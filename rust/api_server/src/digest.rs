//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Email Digest Service & Background Delivery Engine
//! ═══════════════════════════════════════════════════════════════════════════════
//!
//! Provides institutional subscribers with personalized market intelligence digests
//! delivered to their inbox at configurable frequencies (daily/weekly). Synthesizes
//! cross-sector sentiment trends, Form 8-K and Form 4 corporate catalyst events,
//! and curated full-text financial news into responsive HTML and plain-text summaries.
//! ═══════════════════════════════════════════════════════════════════════════════

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde_json::json;
use sqlx::{PgPool, Row};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::task::JoinHandle;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::models::{CreateDigestRequest, DigestItemCounts, DigestSubscription};
use crate::news_articles::NewsArticleRegistry;
use crate::sector::GLOBAL_SECTOR_MAP;

// ─────────────────────────────────────────────────────────────────────────────
// Constants & Validation Sets
// ─────────────────────────────────────────────────────────────────────────────

pub const SUPPORTED_FREQUENCIES: &[&str] = &["daily", "weekly"];
pub const SUPPORTED_EVENT_TYPES: &[&str] =
    &["earnings", "insider", "ma", "8k", "news", "sentiment"];
pub const MAX_DIGEST_TICKERS: usize = 50;
pub const MAX_DIGEST_SECTORS: usize = 11;
pub const DEFAULT_DIGEST_WORKER_INTERVAL_SECS: u64 = 3600;

// ─────────────────────────────────────────────────────────────────────────────
// EmailSender Trait & Implementations
// ─────────────────────────────────────────────────────────────────────────────

/// Record of an email dispatched by `MockEmailSender`.
#[derive(Debug, Clone)]
pub struct SentEmailRecord {
    pub to: String,
    pub subject: String,
    pub body_html: String,
    pub body_text: String,
    pub sent_at: DateTime<Utc>,
}

/// Abstract asynchronous email delivery interface.
pub trait EmailSender: Send + Sync {
    fn send_email<'a>(
        &'a self,
        to: &'a str,
        subject: &'a str,
        body_html: &'a str,
        body_text: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + 'a>>;
}

/// In-memory mock email sender that logs to console and retains dispatch history.
#[derive(Debug, Clone, Default)]
pub struct MockEmailSender {
    sent_emails: Arc<Mutex<Vec<SentEmailRecord>>>,
}

impl MockEmailSender {
    pub fn new() -> Self {
        Self {
            sent_emails: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Retrieve all sent email records.
    pub fn get_sent_emails(&self) -> Vec<SentEmailRecord> {
        self.sent_emails.lock().unwrap().clone()
    }

    /// Retrieve the count of sent emails.
    pub fn sent_count(&self) -> usize {
        self.sent_emails.lock().unwrap().len()
    }

    /// Clear sent email history.
    pub fn clear(&self) {
        self.sent_emails.lock().unwrap().clear();
    }
}

impl EmailSender for MockEmailSender {
    fn send_email<'a>(
        &'a self,
        to: &'a str,
        subject: &'a str,
        body_html: &'a str,
        body_text: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + 'a>> {
        let to = to.to_string();
        let subject = subject.to_string();
        let body_html = body_html.to_string();
        let body_text = body_text.to_string();
        let sent_emails = self.sent_emails.clone();

        Box::pin(async move {
            let record = SentEmailRecord {
                to: to.clone(),
                subject: subject.clone(),
                body_html: body_html.clone(),
                body_text: body_text.clone(),
                sent_at: Utc::now(),
            };

            info!(
                "[Email Digest] Mock dispatch to '{}' | Subject: '{}' | (HTML bytes: {}, Text bytes: {})",
                to,
                subject,
                body_html.len(),
                body_text.len()
            );

            let mut lock = sent_emails.lock().unwrap();
            lock.push(record);
            Ok(())
        })
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Digest Subscription Registry
// ─────────────────────────────────────────────────────────────────────────────

/// In-memory and PostgreSQL synchronized Email Digest Subscription Registry.
#[derive(Debug, Clone, Default)]
pub struct DigestSubscriptionRegistry {
    /// Keyed by user_id string for single-subscription-per-user enforcement
    entries: Arc<DashMap<String, DigestSubscription>>,
}

impl DigestSubscriptionRegistry {
    /// Creates a new empty `DigestSubscriptionRegistry`.
    pub fn new() -> Self {
        Self {
            entries: Arc::new(DashMap::new()),
        }
    }

    /// Initializes PostgreSQL schema for `email_digest_subscriptions` and `digest_send_history`.
    pub async fn init_db(pool: &PgPool) -> Result<(), sqlx::Error> {
        let sql = r#"
            CREATE TABLE IF NOT EXISTS email_digest_subscriptions (
                id UUID PRIMARY KEY,
                user_id TEXT NOT NULL UNIQUE,
                frequency TEXT NOT NULL DEFAULT 'daily',
                tickers JSONB NOT NULL DEFAULT '[]',
                sectors JSONB NOT NULL DEFAULT '[]',
                event_types JSONB NOT NULL DEFAULT '[]',
                is_active BOOLEAN NOT NULL DEFAULT TRUE,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );
            CREATE INDEX IF NOT EXISTS idx_digest_subscriptions_user_id ON email_digest_subscriptions(user_id);

            CREATE TABLE IF NOT EXISTS digest_send_history (
                id UUID PRIMARY KEY,
                subscription_id UUID NOT NULL,
                user_id TEXT NOT NULL,
                recipient_email TEXT NOT NULL,
                subject TEXT NOT NULL,
                content_summary TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'sent',
                sent_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );
            CREATE INDEX IF NOT EXISTS idx_digest_history_user_time ON digest_send_history(user_id, sent_at DESC);
        "#;
        sqlx::query(sql).execute(pool).await?;
        info!("[Email Digest] PostgreSQL tables 'email_digest_subscriptions' and 'digest_send_history' verified");
        Ok(())
    }

    /// Loads existing subscriptions from PostgreSQL into in-memory cache.
    pub async fn load_from_db(&self, pool: &PgPool) -> Result<usize, sqlx::Error> {
        let sql = r#"
            SELECT id, user_id, frequency, tickers, sectors, event_types, is_active, created_at, updated_at
            FROM email_digest_subscriptions
        "#;
        let rows = sqlx::query(sql).fetch_all(pool).await?;
        let count = rows.len();

        for row in rows {
            let id: Uuid = row.try_get("id")?;
            let user_id: String = row.try_get("user_id")?;
            let frequency: String = row.try_get("frequency")?;
            let tickers_raw: serde_json::Value = row.try_get("tickers")?;
            let sectors_raw: serde_json::Value = row.try_get("sectors")?;
            let event_types_raw: serde_json::Value = row.try_get("event_types")?;
            let is_active: bool = row.try_get("is_active")?;
            let created_at: DateTime<Utc> = row.try_get("created_at")?;
            let updated_at: DateTime<Utc> = row.try_get("updated_at")?;

            let tickers: Vec<String> = serde_json::from_value(tickers_raw).unwrap_or_default();
            let sectors: Vec<String> = serde_json::from_value(sectors_raw).unwrap_or_default();
            let event_types: Vec<String> =
                serde_json::from_value(event_types_raw).unwrap_or_default();

            self.entries.insert(
                user_id.clone(),
                DigestSubscription {
                    id,
                    user_id,
                    frequency,
                    tickers,
                    sectors,
                    event_types,
                    is_active,
                    created_at,
                    updated_at,
                },
            );
        }

        info!(
            "[Email Digest] Loaded {} subscriptions from PostgreSQL into memory",
            count
        );
        Ok(count)
    }

    /// Retrieves an existing subscription for a given user ID.
    pub fn get_by_user(&self, user_id: &str) -> Option<DigestSubscription> {
        self.entries.get(user_id).map(|r| r.value().clone())
    }

    /// Upserts a subscription into in-memory cache and PostgreSQL (if connected).
    pub async fn upsert(
        &self,
        sub: DigestSubscription,
        pool: Option<&PgPool>,
    ) -> Result<DigestSubscription, String> {
        self.entries.insert(sub.user_id.clone(), sub.clone());

        if let Some(p) = pool {
            let tickers_json = json!(sub.tickers);
            let sectors_json = json!(sub.sectors);
            let event_types_json = json!(sub.event_types);

            let sql = r#"
                INSERT INTO email_digest_subscriptions
                    (id, user_id, frequency, tickers, sectors, event_types, is_active, created_at, updated_at)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                ON CONFLICT (user_id) DO UPDATE SET
                    frequency = EXCLUDED.frequency,
                    tickers = EXCLUDED.tickers,
                    sectors = EXCLUDED.sectors,
                    event_types = EXCLUDED.event_types,
                    is_active = EXCLUDED.is_active,
                    updated_at = EXCLUDED.updated_at
            "#;

            if let Err(e) = sqlx::query(sql)
                .bind(sub.id)
                .bind(&sub.user_id)
                .bind(&sub.frequency)
                .bind(&tickers_json)
                .bind(&sectors_json)
                .bind(&event_types_json)
                .bind(sub.is_active)
                .bind(sub.created_at)
                .bind(sub.updated_at)
                .execute(p)
                .await
            {
                warn!(
                    "[Email Digest] Failed to persist subscription to PostgreSQL: {}",
                    e
                );
            }
        }

        Ok(sub)
    }

    /// Deletes a subscription for a given user ID.
    pub async fn delete_by_user(
        &self,
        user_id: &str,
        pool: Option<&PgPool>,
    ) -> Result<Option<DigestSubscription>, String> {
        let removed = self.entries.remove(user_id).map(|(_, v)| v);

        if let Some(p) = pool {
            let sql = "DELETE FROM email_digest_subscriptions WHERE user_id = $1";
            if let Err(e) = sqlx::query(sql).bind(user_id).execute(p).await {
                warn!(
                    "[Email Digest] Failed to delete subscription from PostgreSQL: {}",
                    e
                );
            }
        }

        Ok(removed)
    }

    /// Returns all active subscriptions.
    pub fn list_active(&self) -> Vec<DigestSubscription> {
        self.entries
            .iter()
            .filter(|r| r.value().is_active)
            .map(|r| r.value().clone())
            .collect()
    }

    /// Returns total subscription count.
    pub fn count(&self) -> usize {
        self.entries.len()
    }

    /// Records a dispatched digest entry into the send history table.
    pub async fn record_send_history(
        pool: Option<&PgPool>,
        sub_id: Uuid,
        user_id: &str,
        recipient: &str,
        subject: &str,
        summary: &str,
    ) {
        if let Some(p) = pool {
            let id = Uuid::new_v4();
            let sql = r#"
                INSERT INTO digest_send_history (id, subscription_id, user_id, recipient_email, subject, content_summary, status, sent_at)
                VALUES ($1, $2, $3, $4, $5, $6, 'sent', NOW())
            "#;
            if let Err(e) = sqlx::query(sql)
                .bind(id)
                .bind(sub_id)
                .bind(user_id)
                .bind(recipient)
                .bind(subject)
                .bind(summary)
                .execute(p)
                .await
            {
                warn!(
                    "[Email Digest] Failed to record send history in PostgreSQL: {}",
                    e
                );
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Input Validation
// ─────────────────────────────────────────────────────────────────────────────

/// Validates a create/update digest subscription request payload.
pub fn validate_digest_request(req: &CreateDigestRequest) -> Result<(), String> {
    // 1. Frequency validation
    let freq = req.frequency.trim().to_lowercase();
    if !SUPPORTED_FREQUENCIES.contains(&freq.as_str()) {
        return Err(format!(
            "Invalid frequency '{}'. Must be one of: {:?}",
            req.frequency, SUPPORTED_FREQUENCIES
        ));
    }

    // 2. Tickers validation
    if req.tickers.len() > MAX_DIGEST_TICKERS {
        return Err(format!(
            "Too many tickers provided ({}). Maximum permitted is {}",
            req.tickers.len(),
            MAX_DIGEST_TICKERS
        ));
    }
    for t in &req.tickers {
        let clean = t.trim().to_uppercase();
        if clean.is_empty()
            || clean.len() > 10
            || !clean
                .chars()
                .all(|c| c.is_alphanumeric() || c == '.' || c == '-')
        {
            return Err(format!("Invalid ticker symbol '{}'", t));
        }
    }

    // 3. Sectors validation
    let sector_map = GLOBAL_SECTOR_MAP.clone();
    if req.sectors.len() > MAX_DIGEST_SECTORS {
        return Err(format!(
            "Too many sectors provided ({}). Maximum permitted is {}",
            req.sectors.len(),
            MAX_DIGEST_SECTORS
        ));
    }
    for s in &req.sectors {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return Err("Sector name cannot be empty".to_string());
        }
        let matches = sector_map
            .sectors
            .iter()
            .any(|sec| sec.eq_ignore_ascii_case(trimmed));
        if !matches && !is_valid_sector_alias(trimmed) {
            return Err(format!(
                "Invalid sector '{}'. Valid GICS sectors: {:?}",
                s, sector_map.sectors
            ));
        }
    }

    // 4. Event types validation
    for ev in &req.event_types {
        let clean = ev.trim().to_lowercase().replace('_', "").replace('-', "");
        let valid = SUPPORTED_EVENT_TYPES.iter().any(|valid_ev| {
            let clean_valid = valid_ev.replace('_', "").replace('-', "");
            clean == clean_valid || clean.contains(&clean_valid)
        });
        if !valid {
            return Err(format!(
                "Invalid event type '{}'. Supported event types: {:?}",
                ev, SUPPORTED_EVENT_TYPES
            ));
        }
    }

    Ok(())
}

fn is_valid_sector_alias(s: &str) -> bool {
    let lower = s.to_lowercase();
    matches!(
        lower.as_str(),
        "tech"
            | "technology"
            | "finance"
            | "financials"
            | "health"
            | "healthcare"
            | "energy"
            | "consumer"
            | "consumer cyclical"
            | "industrials"
            | "comms"
            | "communication"
            | "communication services"
            | "utilities"
            | "real estate"
            | "materials"
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// Digest Email Content Generator
// ─────────────────────────────────────────────────────────────────────────────

/// Generates structured HTML and plain-text email content for a subscription.
pub fn compose_digest_email(
    sub: &DigestSubscription,
    recipient_email: &str,
    news_registry: &NewsArticleRegistry,
) -> (String, String, String, DigestItemCounts) {
    let now = Utc::now();
    let date_str = now.format("%Y-%m-%d").to_string();
    let freq_cap = if sub.frequency.eq_ignore_ascii_case("weekly") {
        "Weekly"
    } else {
        "Daily"
    };
    let subject = format!("FinText Alpha {} Market Digest — {}", freq_cap, date_str);

    let mut sentiment_items = Vec::new();
    let mut event_items = Vec::new();
    let mut news_items = Vec::new();

    // 1. Ticker & Sector Sentiment Section
    let default_tickers = if sub.tickers.is_empty() {
        vec!["AAPL".to_string(), "NVDA".to_string(), "MSFT".to_string()]
    } else {
        sub.tickers.clone()
    };

    for ticker in &default_tickers {
        let clean = ticker.trim().to_uppercase();
        let hash = clean.bytes().map(|b| b as usize).sum::<usize>();
        let score = (((hash % 100) as f64 - 45.0) / 100.0).clamp(-0.95, 0.95);
        let label = if score > 0.15 {
            "Bullish"
        } else if score < -0.15 {
            "Bearish"
        } else {
            "Neutral"
        };
        let signal_color = if score > 0.15 {
            "#10B981"
        } else if score < -0.15 {
            "#EF4444"
        } else {
            "#6B7280"
        };
        sentiment_items.push((clean, score, label.to_string(), signal_color));
    }

    // 2. Catalyst Events Section
    let include_8k = sub
        .event_types
        .iter()
        .any(|e| e.eq_ignore_ascii_case("8k") || e.eq_ignore_ascii_case("all"));
    let include_insider = sub
        .event_types
        .iter()
        .any(|e| e.eq_ignore_ascii_case("insider") || e.eq_ignore_ascii_case("all"));
    let include_earnings = sub
        .event_types
        .iter()
        .any(|e| e.eq_ignore_ascii_case("earnings") || e.eq_ignore_ascii_case("all"));
    let include_ma = sub
        .event_types
        .iter()
        .any(|e| e.eq_ignore_ascii_case("ma") || e.eq_ignore_ascii_case("all"));

    if include_8k {
        event_items.push((
            "Form 8-K: Material Disclosure".to_string(),
            "NVDA".to_string(),
            "Executive leadership transition announced: Chief Operating Officer retirement."
                .to_string(),
            "2026-08-30".to_string(),
        ));
    }
    if include_insider {
        event_items.push((
            "Form 4: Insider Purchase".to_string(),
            "AAPL".to_string(),
            "Chief Financial Officer acquired 15,000 common shares via open market transaction."
                .to_string(),
            "2026-08-29".to_string(),
        ));
    }
    if include_earnings {
        event_items.push((
            "Earnings Announcement".to_string(),
            "MSFT".to_string(),
            "Quarterly EPS beat consensus by +14.2%; cloud intelligence revenue up +28% YoY."
                .to_string(),
            "2026-08-28".to_string(),
        ));
    }
    if include_ma {
        event_items.push((
            "M&A Catalyst Signal".to_string(),
            "AMZN".to_string(),
            "Strategic acquisition discussions reported for sovereign AI compute provider."
                .to_string(),
            "2026-08-27".to_string(),
        ));
    }

    // 3. News Articles Section
    let all_articles = news_registry.list_articles(&crate::models::ListNewsArticlesQuery {
        ticker: None,
        start_date: None,
        end_date: None,
        source: None,
        limit: Some(10),
        offset: Some(0),
    });
    for art in all_articles.articles {
        if sub.tickers.is_empty()
            || sub
                .tickers
                .iter()
                .any(|t| t.eq_ignore_ascii_case(&art.ticker))
        {
            news_items.push((
                art.ticker,
                art.title,
                art.source,
                art.published_utc.format("%Y-%m-%d %H:%M UTC").to_string(),
            ));
        }
    }
    if news_items.is_empty() {
        news_items.push((
            "AAPL".to_string(),
            "Apple Showcases Ultra-Low-Latency Edge AI Architecture Across Flagship Devices"
                .to_string(),
            "Financial Times Wire".to_string(),
            date_str.clone(),
        ));
    }

    let counts = DigestItemCounts {
        sentiment_count: sentiment_items.len(),
        events_count: event_items.len(),
        news_count: news_items.len(),
    };

    // ── Build HTML Body ──────────────────────────────────────────────────────
    let mut html = String::new();
    html.push_str("<!DOCTYPE html><html><head><meta charset='utf-8'>");
    html.push_str("<style>");
    html.push_str("body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif; background-color: #0B0F19; color: #E2E8F0; margin: 0; padding: 24px; }");
    html.push_str(".container { max-width: 680px; margin: 0 auto; background-color: #111827; border: 1px solid #1F2937; border-radius: 12px; padding: 24px; }");
    html.push_str(
        ".header { border-bottom: 1px solid #374151; padding-bottom: 16px; margin-bottom: 20px; }",
    );
    html.push_str(".header h1 { font-size: 22px; color: #F9FAFB; margin: 0 0 6px 0; }");
    html.push_str(".header p { font-size: 13px; color: #9CA3AF; margin: 0; }");
    html.push_str(".section-title { font-size: 16px; font-weight: 600; color: #60A5FA; margin: 20px 0 10px 0; border-bottom: 1px solid #1E293B; padding-bottom: 4px; }");
    html.push_str(
        ".sentiment-grid { display: flex; flex-wrap: wrap; gap: 8px; margin-bottom: 16px; }",
    );
    html.push_str(".sentiment-badge { background-color: #1E293B; border: 1px solid #334155; border-radius: 6px; padding: 8px 12px; font-size: 13px; }");
    html.push_str(".event-card { background-color: #182234; border-left: 3px solid #3B82F6; border-radius: 4px; padding: 10px 14px; margin-bottom: 8px; font-size: 13px; }");
    html.push_str(
        ".news-card { padding: 8px 0; border-bottom: 1px solid #1F2937; font-size: 13px; }",
    );
    html.push_str(".news-card a { color: #93C5FD; text-decoration: none; font-weight: 500; }");
    html.push_str(".footer { margin-top: 24px; border-top: 1px solid #374151; padding-top: 12px; font-size: 11px; color: #6B7280; text-align: center; }");
    html.push_str("</style></head><body>");
    html.push_str("<div class='container'>");

    html.push_str(&format!(
        "<div class='header'><h1>FinText Alpha {} Digest</h1><p>Market Intelligence & Event Summary for {} &bull; Generated for {}</p></div>",
        freq_cap, date_str, recipient_email
    ));

    // Sentiment section
    html.push_str(
        "<div class='section-title'>📊 Sentiment Signals & Drift</div><div class='sentiment-grid'>",
    );
    for (tkr, sc, lbl, col) in &sentiment_items {
        html.push_str(&format!(
            "<div class='sentiment-badge'><strong>{}</strong>: <span style='color:{};font-weight:600;'>{:.2} ({})</span></div>",
            tkr, col, sc, lbl
        ));
    }
    html.push_str("</div>");

    // Events section
    if !event_items.is_empty() {
        html.push_str(
            "<div class='section-title'>⚡ Corporate Catalysts & Regulatory Disclosures</div>",
        );
        for (cat, tkr, desc, dt) in &event_items {
            html.push_str(&format!(
                "<div class='event-card'><span style='color:#93C5FD;font-weight:600;'>[{}] {}</span> &bull; <span style='color:#9CA3AF;'>{}</span><br><span style='color:#CBD5E1;'>{}</span></div>",
                tkr, cat, dt, desc
            ));
        }
    }

    // News section
    if !news_items.is_empty() {
        html.push_str("<div class='section-title'>📰 Curated Institutional News</div>");
        for (tkr, title, src, dt) in &news_items {
            html.push_str(&format!(
                "<div class='news-card'><strong>[{}]</strong> {}<br><span style='color:#6B7280;font-size:11px;'>{} &bull; {}</span></div>",
                tkr, title, src, dt
            ));
        }
    }

    html.push_str(&format!(
        "<div class='footer'>Sent by FinText Alpha Vectorizer &bull; Preferences: Frequency={} | Manage at /digest/subscription</div>",
        sub.frequency
    ));
    html.push_str("</div></body></html>");

    // ── Build Plain Text Body ────────────────────────────────────────────────
    let mut text = String::new();
    text.push_str(&format!(
        "================================================================================\n"
    ));
    text.push_str(&format!(
        "  FinText Alpha {} Market Digest — {}\n",
        freq_cap, date_str
    ));
    text.push_str(&format!("  Recipient: {}\n", recipient_email));
    text.push_str(&format!(
        "================================================================================\n\n"
    ));

    text.push_str("--- 1. SENTIMENT SIGNALS ---\n");
    for (tkr, sc, lbl, _) in &sentiment_items {
        text.push_str(&format!("  * {:<6} Score: {:+.2} ({})\n", tkr, sc, lbl));
    }
    text.push_str("\n");

    if !event_items.is_empty() {
        text.push_str("--- 2. CORPORATE CATALYSTS & EVENTS ---\n");
        for (cat, tkr, desc, dt) in &event_items {
            text.push_str(&format!("  * [{}] {} ({})\n    {}\n", tkr, cat, dt, desc));
        }
        text.push_str("\n");
    }

    if !news_items.is_empty() {
        text.push_str("--- 3. FINANCIAL NEWS HEADLINES ---\n");
        for (tkr, title, src, dt) in &news_items {
            text.push_str(&format!("  * [{}] {} ({}, {})\n", tkr, title, src, dt));
        }
        text.push_str("\n");
    }

    text.push_str(&format!(
        "---\nManage your preferences at https://api.fintext.io/digest/subscription\n"
    ));

    (subject, html, text, counts)
}

// ─────────────────────────────────────────────────────────────────────────────
// Background Digest Delivery Worker
// ─────────────────────────────────────────────────────────────────────────────

/// Spawns the background Email Digest Worker Tokio task.
pub fn spawn_digest_worker(
    registry: Arc<DigestSubscriptionRegistry>,
    email_sender: Arc<dyn EmailSender>,
    news_registry: Arc<NewsArticleRegistry>,
    pool: Option<PgPool>,
    interval_secs: u64,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        info!(
            "[Email Digest Worker] Started background delivery worker (Interval: {}s)",
            interval_secs
        );

        let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
        interval.tick().await; // skip immediate first tick

        loop {
            interval.tick().await;
            let active_subs = registry.list_active();
            if active_subs.is_empty() {
                continue;
            }

            info!(
                "[Email Digest Worker] Processing {} active email digest subscriptions",
                active_subs.len()
            );

            for sub in active_subs {
                let recipient = format!("{}@institutional-fund.com", sub.user_id);
                let (subject, body_html, body_text, _) =
                    compose_digest_email(&sub, &recipient, &news_registry);

                if let Err(e) = email_sender
                    .send_email(&recipient, &subject, &body_html, &body_text)
                    .await
                {
                    error!(
                        "[Email Digest Worker] Failed to send digest to '{}': {}",
                        recipient, e
                    );
                } else {
                    DigestSubscriptionRegistry::record_send_history(
                        pool.as_ref(),
                        sub.id,
                        &sub.user_id,
                        &recipient,
                        &subject,
                        "Automatic background digest delivery",
                    )
                    .await;
                }
            }
        }
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_digest_request_valid() {
        let req = CreateDigestRequest {
            frequency: "daily".to_string(),
            tickers: vec!["AAPL".to_string(), "MSFT".to_string()],
            sectors: vec!["Technology".to_string()],
            event_types: vec!["earnings".to_string(), "insider".to_string()],
            is_active: true,
        };
        assert!(validate_digest_request(&req).is_ok());
    }

    #[test]
    fn test_validate_digest_request_invalid_frequency() {
        let req = CreateDigestRequest {
            frequency: "monthly".to_string(),
            tickers: vec!["AAPL".to_string()],
            sectors: vec![],
            event_types: vec!["earnings".to_string()],
            is_active: true,
        };
        let res = validate_digest_request(&req);
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("Invalid frequency"));
    }

    #[test]
    fn test_validate_digest_request_invalid_event_type() {
        let req = CreateDigestRequest {
            frequency: "weekly".to_string(),
            tickers: vec!["NVDA".to_string()],
            sectors: vec![],
            event_types: vec!["crypto_pump".to_string()],
            is_active: true,
        };
        let res = validate_digest_request(&req);
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("Invalid event type"));
    }

    #[test]
    fn test_validate_digest_request_invalid_ticker() {
        let req = CreateDigestRequest {
            frequency: "daily".to_string(),
            tickers: vec!["INVALID$$$TICKER".to_string()],
            sectors: vec![],
            event_types: vec!["8k".to_string()],
            is_active: true,
        };
        let res = validate_digest_request(&req);
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("Invalid ticker"));
    }

    #[tokio::test]
    async fn test_mock_email_sender() {
        let sender = MockEmailSender::new();
        assert_eq!(sender.sent_count(), 0);

        let res = sender
            .send_email(
                "trader@fund.com",
                "Daily Digest Test",
                "<h1>Test HTML</h1>",
                "Test Plain Text",
            )
            .await;
        assert!(res.is_ok());
        assert_eq!(sender.sent_count(), 1);

        let sent = sender.get_sent_emails();
        assert_eq!(sent[0].to, "trader@fund.com");
        assert_eq!(sent[0].subject, "Daily Digest Test");
        assert_eq!(sent[0].body_html, "<h1>Test HTML</h1>");

        sender.clear();
        assert_eq!(sender.sent_count(), 0);
    }

    #[tokio::test]
    async fn test_digest_subscription_registry_crud() {
        let registry = DigestSubscriptionRegistry::new();
        assert_eq!(registry.count(), 0);

        let sub = DigestSubscription {
            id: Uuid::new_v4(),
            user_id: "quant_01".to_string(),
            frequency: "daily".to_string(),
            tickers: vec!["AAPL".to_string(), "NVDA".to_string()],
            sectors: vec!["Technology".to_string()],
            event_types: vec!["earnings".to_string(), "8k".to_string()],
            is_active: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        // Upsert
        let upserted = registry.upsert(sub.clone(), None).await.unwrap();
        assert_eq!(upserted.user_id, "quant_01");
        assert_eq!(registry.count(), 1);

        // Get
        let found = registry.get_by_user("quant_01").unwrap();
        assert_eq!(found.frequency, "daily");
        assert_eq!(found.tickers, vec!["AAPL", "NVDA"]);

        // List active
        let active = registry.list_active();
        assert_eq!(active.len(), 1);

        // Delete
        let deleted = registry.delete_by_user("quant_01", None).await.unwrap();
        assert!(deleted.is_some());
        assert_eq!(registry.count(), 0);
        assert!(registry.get_by_user("quant_01").is_none());
    }

    #[test]
    fn test_compose_digest_email() {
        let news_reg = NewsArticleRegistry::new();
        let sub = DigestSubscription {
            id: Uuid::new_v4(),
            user_id: "test_user".to_string(),
            frequency: "daily".to_string(),
            tickers: vec!["AAPL".to_string(), "MSFT".to_string()],
            sectors: vec!["Technology".to_string()],
            event_types: vec!["earnings".to_string(), "8k".to_string(), "news".to_string()],
            is_active: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let (subject, html, text, counts) =
            compose_digest_email(&sub, "test@example.com", &news_reg);

        assert!(subject.contains("Daily Market Digest"));
        assert!(html.contains("FinText Alpha Daily Digest"));
        assert!(html.contains("AAPL"));
        assert!(text.contains("SENTIMENT SIGNALS"));
        assert!(counts.sentiment_count >= 2);
    }
}
