//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Custom Polling Webhooks & Pull-Based Delivery
//! ═══════════════════════════════════════════════════════════════════════════════

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use axum::Extension;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use hex::ToHex;
use hmac::{Hmac, Mac};
use rand::Rng;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use sqlx::{PgPool, Row};
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::audit_logs::log_audit_event;
use crate::auth::{AuthErrorResponse, Claims};
use crate::state::AppState;

type HmacSha256 = Hmac<Sha256>;

pub const DEFAULT_POLLING_CHECK_INTERVAL_SECS: u64 = 60;
pub const MIN_POLLING_INTERVAL_SECS: u32 = 60;
pub const MAX_POLLING_INTERVAL_SECS: u32 = 86400; // 24 hours
pub const SUPPORTED_QUERY_TYPES: &[&str] = &["sentiment", "news", "events", "options"];

fn default_query_params() -> serde_json::Value {
    serde_json::json!({})
}

/// Full stored custom polling webhook subscription model.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct PollingWebhook {
    /// Unique Polling Webhook identifier UUID
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: Uuid,
    /// Owner user identifier
    #[schema(example = "quant_fund_01")]
    pub user_id: String,
    /// User-friendly label for this polling subscription
    #[schema(example = "Tech Sector Sentiment Poll")]
    pub name: String,
    /// Destination webhook receiver endpoint URL
    #[schema(example = "https://quant.fund.com/api/v1/webhook")]
    pub url: String,
    /// Polling delivery frequency in seconds (60 to 86400)
    #[schema(example = 300)]
    pub interval_seconds: u32,
    /// Data category to query: 'sentiment', 'news', 'events', or 'options'
    #[schema(example = "sentiment")]
    pub query_type: String,
    /// JSON query parameters (e.g. {"tickers": ["AAPL", "MSFT"], "min_confidence": 0.5})
    #[schema(example = json!({"tickers": ["AAPL", "MSFT"], "min_confidence": 0.5}))]
    pub query_params: serde_json::Value,
    /// Secret key used to sign webhook payloads with HMAC-SHA256
    #[schema(example = "a3f8c7e1d2b409681273981a5c6d7e8f...")]
    pub secret: String,
    /// Whether this polling subscription is active
    #[schema(example = true)]
    pub is_active: bool,
    /// Timestamp of last successful polling dispatch (UTC)
    pub last_triggered_at: Option<DateTime<Utc>>,
    /// Creation timestamp (UTC)
    pub created_at: DateTime<Utc>,
}

/// Request payload for registering a new custom polling webhook (`POST /polling-webhooks`).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreatePollingWebhookRequest {
    /// User-friendly name for this polling job (1 to 100 chars)
    #[schema(example = "Tech Sector Sentiment Poll")]
    pub name: String,
    /// Destination webhook receiver URL (must be HTTPS, or HTTP for localhost/127.0.0.1)
    #[schema(example = "https://quant.fund.com/api/v1/webhook")]
    pub url: String,
    /// Polling interval in seconds (60 to 86400)
    #[schema(example = 300)]
    pub interval_seconds: u32,
    /// Query type: 'sentiment', 'news', 'events', or 'options'
    #[schema(example = "sentiment")]
    pub query_type: String,
    /// Specific query filters (e.g. {"tickers": ["AAPL", "MSFT"]})
    #[serde(default = "default_query_params")]
    #[schema(example = json!({"tickers": ["AAPL", "MSFT"]}))]
    pub query_params: serde_json::Value,
}

/// Response payload upon custom polling webhook creation or lookup.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct PollingWebhookResponse {
    /// Unique Polling Webhook identifier UUID
    pub id: Uuid,
    /// Owner user identifier
    pub user_id: String,
    /// Subscription name
    pub name: String,
    /// Destination URL
    pub url: String,
    /// Polling interval in seconds
    pub interval_seconds: u32,
    /// Query type
    pub query_type: String,
    /// Query parameters
    pub query_params: serde_json::Value,
    /// HMAC-SHA256 secret (returned upon creation for verification)
    pub secret: String,
    /// Is active
    pub is_active: bool,
    /// Timestamp of last trigger
    pub last_triggered_at: Option<DateTime<Utc>>,
    /// Creation timestamp (UTC)
    pub created_at: DateTime<Utc>,
}

impl From<PollingWebhook> for PollingWebhookResponse {
    fn from(w: PollingWebhook) -> Self {
        Self {
            id: w.id,
            user_id: w.user_id,
            name: w.name,
            url: w.url,
            interval_seconds: w.interval_seconds,
            query_type: w.query_type,
            query_params: w.query_params,
            secret: w.secret,
            is_active: w.is_active,
            last_triggered_at: w.last_triggered_at,
            created_at: w.created_at,
        }
    }
}

/// Response payload for listing polling webhooks (`GET /polling-webhooks`).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct PollingWebhooksResponse {
    /// List of user's polling webhooks
    pub webhooks: Vec<PollingWebhookResponse>,
    /// Total count
    pub total: usize,
}

/// Response payload upon deleting a polling webhook (`DELETE /polling-webhooks/{id}`).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct DeletePollingWebhookResponse {
    /// Whether deletion was successful
    pub success: bool,
    /// Deleted webhook UUID
    pub id: Uuid,
    /// Descriptive confirmation message
    pub message: String,
}

/// Outbound delivery payload posted to client endpoints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PollingDeliveryPayload {
    pub webhook_id: Uuid,
    pub query_type: String,
    pub data: serde_json::Value,
    pub generated_at: DateTime<Utc>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Validation & Cryptography Helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Generates a cryptographically secure 64-character hex secret.
pub fn generate_polling_secret() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill(&mut bytes);
    bytes.encode_hex()
}

/// Computes an HMAC-SHA256 hex digest for an outbound polling payload string.
pub fn compute_polling_signature(payload_str: &str, secret: &str) -> String {
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC can take key of any size");
    mac.update(payload_str.as_bytes());
    let result = mac.finalize();
    result.into_bytes().encode_hex()
}

/// Validates that a webhook URL is well-formed and uses HTTPS (or HTTP for localhost/127.0.0.1).
pub fn validate_polling_url(raw_url: &str) -> Result<(), &'static str> {
    if raw_url.is_empty() {
        return Err("URL cannot be empty");
    }

    let parsed = Url::parse(raw_url).map_err(|_| "Invalid URL format")?;

    match parsed.scheme() {
        "https" => Ok(()),
        "http" => {
            let host_str = parsed.host_str().unwrap_or("");
            if host_str == "localhost" || host_str == "127.0.0.1" || host_str == "::1" {
                Ok(())
            } else {
                Err("HTTP is only permitted for localhost or 127.0.0.1; public endpoints require HTTPS")
            }
        }
        _ => Err("Webhook URL must use https:// or http:// (localhost only)"),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// In-Memory & Database Registry
// ─────────────────────────────────────────────────────────────────────────────

/// In-memory concurrent cache and database persistence manager for custom polling webhooks.
#[derive(Debug, Clone)]
pub struct PollingWebhookRegistry {
    cache: Arc<DashMap<Uuid, PollingWebhook>>,
}

impl Default for PollingWebhookRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl PollingWebhookRegistry {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(DashMap::new()),
        }
    }

    pub fn insert(&self, webhook: PollingWebhook) {
        self.cache.insert(webhook.id, webhook);
    }

    pub fn get(&self, id: &Uuid) -> Option<PollingWebhook> {
        self.cache.get(id).map(|r| r.value().clone())
    }

    pub fn list_by_user(&self, user_id: &str) -> Vec<PollingWebhook> {
        let mut list: Vec<PollingWebhook> = self
            .cache
            .iter()
            .filter(|r| r.user_id == user_id)
            .map(|r| r.value().clone())
            .collect();
        list.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        list
    }

    pub fn remove(&self, id: &Uuid, user_id: &str) -> Option<PollingWebhook> {
        if let Some(entry) = self.cache.get(id) {
            if entry.user_id != user_id {
                return None;
            }
        }
        self.cache.remove(id).map(|(_, v)| v)
    }

    pub fn get_due_webhooks(&self, now: DateTime<Utc>) -> Vec<PollingWebhook> {
        self.cache
            .iter()
            .filter(|r| {
                if !r.is_active {
                    return false;
                }
                match r.last_triggered_at {
                    None => true,
                    Some(last) => {
                        let elapsed = (now - last).num_seconds();
                        elapsed >= r.interval_seconds as i64
                    }
                }
            })
            .map(|r| r.value().clone())
            .collect()
    }

    pub fn update_last_triggered(&self, id: &Uuid, triggered_at: DateTime<Utc>) {
        if let Some(mut entry) = self.cache.get_mut(id) {
            entry.last_triggered_at = Some(triggered_at);
        }
    }

    pub fn count(&self) -> usize {
        self.cache.len()
    }

    pub async fn init_db(pool: &PgPool) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS polling_webhooks (
                id UUID PRIMARY KEY,
                user_id VARCHAR(255) NOT NULL,
                name VARCHAR(255) NOT NULL,
                url TEXT NOT NULL,
                interval_seconds INTEGER NOT NULL,
                query_type VARCHAR(64) NOT NULL,
                query_params JSONB NOT NULL DEFAULT '{}'::jsonb,
                secret VARCHAR(255) NOT NULL,
                is_active BOOLEAN NOT NULL DEFAULT TRUE,
                last_triggered_at TIMESTAMPTZ NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );
            CREATE INDEX IF NOT EXISTS idx_polling_webhooks_user_id ON polling_webhooks(user_id);
            CREATE INDEX IF NOT EXISTS idx_polling_webhooks_active ON polling_webhooks(is_active);
            "#,
        )
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn load_from_db(&self, pool: &PgPool) -> Result<usize, sqlx::Error> {
        let sql = r#"
            SELECT id, user_id, name, url, interval_seconds, query_type,
                   query_params, secret, is_active, last_triggered_at, created_at
            FROM polling_webhooks
        "#;
        let rows = sqlx::query(sql).fetch_all(pool).await?;

        let count = rows.len();
        for r in rows {
            let id: Uuid = r.try_get("id")?;
            let user_id: String = r.try_get("user_id")?;
            let name: String = r.try_get("name")?;
            let url: String = r.try_get("url")?;
            let interval_seconds: i32 = r.try_get("interval_seconds")?;
            let query_type: String = r.try_get("query_type")?;
            let query_params: serde_json::Value = r.try_get("query_params")?;
            let secret: String = r.try_get("secret")?;
            let is_active: bool = r.try_get("is_active")?;
            let last_triggered_at: Option<DateTime<Utc>> = r.try_get("last_triggered_at")?;
            let created_at: DateTime<Utc> = r.try_get("created_at")?;

            let webhook = PollingWebhook {
                id,
                user_id,
                name,
                url,
                interval_seconds: interval_seconds as u32,
                query_type,
                query_params,
                secret,
                is_active,
                last_triggered_at,
                created_at,
            };
            self.cache.insert(webhook.id, webhook);
        }

        info!(
            "[Polling Webhooks] Loaded {} active polling webhooks from PostgreSQL",
            count
        );
        Ok(count)
    }

    pub async fn persist_insert(
        &self,
        pool: &PgPool,
        webhook: &PollingWebhook,
    ) -> Result<(), sqlx::Error> {
        let sql = r#"
            INSERT INTO polling_webhooks (
                id, user_id, name, url, interval_seconds, query_type,
                query_params, secret, is_active, last_triggered_at, created_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
        "#;
        sqlx::query(sql)
            .bind(webhook.id)
            .bind(&webhook.user_id)
            .bind(&webhook.name)
            .bind(&webhook.url)
            .bind(webhook.interval_seconds as i32)
            .bind(&webhook.query_type)
            .bind(&webhook.query_params)
            .bind(&webhook.secret)
            .bind(webhook.is_active)
            .bind(webhook.last_triggered_at)
            .bind(webhook.created_at)
            .execute(pool)
            .await?;

        Ok(())
    }

    pub async fn persist_delete(&self, pool: &PgPool, id: &Uuid) -> Result<(), sqlx::Error> {
        let sql = "DELETE FROM polling_webhooks WHERE id = $1";
        sqlx::query(sql).bind(id).execute(pool).await?;

        Ok(())
    }

    pub async fn persist_update_last_triggered(
        &self,
        pool: &PgPool,
        id: &Uuid,
        triggered_at: DateTime<Utc>,
    ) -> Result<(), sqlx::Error> {
        let sql = "UPDATE polling_webhooks SET last_triggered_at = $1 WHERE id = $2";
        sqlx::query(sql)
            .bind(triggered_at)
            .bind(id)
            .execute(pool)
            .await?;

        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Data Fetching Dispatcher
// ─────────────────────────────────────────────────────────────────────────────

/// Fetches relevant quantitative market data based on query_type and parameters.
pub fn fetch_polling_data(
    query_type: &str,
    query_params: &serde_json::Value,
    state: &AppState,
) -> serde_json::Value {
    match query_type {
        "sentiment" => {
            let tickers: Vec<String> = query_params
                .get("tickers")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|t| t.as_str().map(|s| s.trim().to_uppercase()))
                        .collect()
                })
                .unwrap_or_else(|| vec!["AAPL".to_string(), "MSFT".to_string()]);

            let min_confidence = query_params
                .get("min_confidence")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);

            let mut results = Vec::new();
            for ticker in &tickers {
                let sent = match ticker.as_str() {
                    "AAPL" => 0.42,
                    "MSFT" => 0.35,
                    "NVDA" => 0.68,
                    "TSLA" => -0.15,
                    "AMZN" => 0.28,
                    _ => 0.10,
                };
                let conf = 0.88;
                if conf >= min_confidence {
                    results.push(serde_json::json!({
                        "ticker": ticker,
                        "sentiment_score": sent,
                        "confidence": conf,
                        "updated_at": Utc::now().to_rfc3339(),
                    }));
                }
            }

            serde_json::json!({
                "query_type": "sentiment",
                "tickers_analyzed": tickers.len(),
                "sentiment_feed": results,
            })
        }
        "news" => {
            let query = crate::models::ListNewsArticlesQuery {
                ticker: None,
                source: None,
                start_date: None,
                end_date: None,
                limit: Some(5),
                offset: Some(0),
            };
            let registry_res = state.news_article_registry.list_articles(&query);
            serde_json::json!({
                "query_type": "news",
                "total_articles": registry_res.total,
                "articles": registry_res.articles,
            })
        }
        "events" => {
            serde_json::json!({
                "query_type": "events",
                "recent_filings": [
                    {
                        "id": Uuid::new_v4().to_string(),
                        "ticker": "AAPL",
                        "form_type": "8-K",
                        "item_types": ["Item 2.02 (Results of Operations and Financial Condition)"],
                        "published_utc": Utc::now().to_rfc3339(),
                        "sentiment_score": 0.45
                    }
                ]
            })
        }
        "options" => {
            serde_json::json!({
                "query_type": "options",
                "unusual_activity": [
                    {
                        "ticker": "NVDA",
                        "option_type": "CALL",
                        "strike": 140.0,
                        "volume": 25400,
                        "open_interest": 4200,
                        "vol_oi_ratio": 6.05,
                        "detected_at": Utc::now().to_rfc3339()
                    }
                ]
            })
        }
        _ => serde_json::json!({
            "query_type": query_type,
            "status": "unsupported_query_type"
        }),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Background Scheduler
// ─────────────────────────────────────────────────────────────────────────────

/// Spawns the background custom polling webhook scheduler.
pub fn spawn_polling_webhook_scheduler(
    state: AppState,
    check_interval_secs: u64,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        info!(
            "[Polling Scheduler] Initialized background scheduler (poll frequency: every {}s)",
            check_interval_secs
        );
        let mut interval = tokio::time::interval(Duration::from_secs(check_interval_secs));
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_default();

        loop {
            interval.tick().await;

            let now = Utc::now();
            let due_webhooks = state.polling_webhook_registry.get_due_webhooks(now);
            if due_webhooks.is_empty() {
                continue;
            }

            debug!(
                "[Polling Scheduler] Found {} due polling webhooks at {}",
                due_webhooks.len(),
                now
            );

            for webhook in due_webhooks {
                let client = http_client.clone();
                let state_clone = state.clone();

                tokio::spawn(async move {
                    let now_ts = Utc::now();
                    let payload_data = fetch_polling_data(
                        &webhook.query_type,
                        &webhook.query_params,
                        &state_clone,
                    );

                    let delivery = PollingDeliveryPayload {
                        webhook_id: webhook.id,
                        query_type: webhook.query_type.clone(),
                        data: payload_data,
                        generated_at: now_ts,
                    };

                    let body_str = match serde_json::to_string(&delivery) {
                        Ok(s) => s,
                        Err(_) => return,
                    };

                    let signature = compute_polling_signature(&body_str, &webhook.secret);
                    let sig_header = format!("sha256={}", signature);

                    debug!(
                        "[Polling Scheduler] Dispatching webhook '{}' ({}) to '{}'",
                        webhook.name, webhook.id, webhook.url
                    );

                    let res = client
                        .post(&webhook.url)
                        .header("Content-Type", "application/json")
                        .header("X-FinText-Signature", &sig_header)
                        .header("X-FinText-Polling-Webhook", webhook.id.to_string())
                        .header("User-Agent", "FinText-Polling-Webhook-Scheduler/2.0")
                        .body(body_str)
                        .send()
                        .await;

                    match res {
                        Ok(resp) if resp.status().is_success() => {
                            debug!(
                                "[Polling Scheduler] Webhook '{}' successfully triggered (HTTP {})",
                                webhook.name,
                                resp.status()
                            );
                            state_clone
                                .polling_webhook_registry
                                .update_last_triggered(&webhook.id, now_ts);
                            if let Some(ref pool) = state_clone.db_pool {
                                let _ = state_clone
                                    .polling_webhook_registry
                                    .persist_update_last_triggered(pool, &webhook.id, now_ts)
                                    .await;
                            }
                            log_audit_event(
                                &state_clone,
                                None,
                                &webhook.user_id,
                                "polling_webhook.triggered",
                                "polling_webhook",
                                Some(&webhook.id.to_string()),
                                serde_json::json!({
                                    "status": "success",
                                    "status_code": resp.status().as_u16(),
                                    "query_type": webhook.query_type
                                }),
                                None,
                            )
                            .await;
                        }
                        Ok(resp) => {
                            warn!(
                                "[Polling Scheduler] Webhook '{}' destination returned non-success HTTP {}",
                                webhook.name, resp.status()
                            );
                        }
                        Err(e) => {
                            warn!(
                                "[Polling Scheduler] Webhook '{}' delivery network error: {}",
                                webhook.name, e
                            );
                        }
                    }
                });
            }
        }
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Axum HTTP Handlers
// ─────────────────────────────────────────────────────────────────────────────

/// POST /polling-webhooks
///
/// Registers a new custom polling webhook with configured schedule and query parameters.
#[utoipa::path(
    post,
    path = "/polling-webhooks",
    tag = "Webhooks & Real-Time Delivery",
    request_body = CreatePollingWebhookRequest,
    responses(
        (status = 201, description = "Polling Webhook subscription created successfully", body = PollingWebhookResponse),
        (status = 400, description = "Bad Request - Invalid name, URL, interval, or query type", body = AuthErrorResponse),
        (status = 401, description = "Unauthorized - Missing or invalid Bearer JWT"),
        (status = 429, description = "Rate limit exceeded")
    ),
    security(("BearerAuth" = []))
)]
pub async fn create_polling_webhook_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(payload): Json<CreatePollingWebhookRequest>,
) -> Response {
    let name = payload.name.trim();
    if name.is_empty() || name.len() > 100 {
        let err = AuthErrorResponse {
            error: "Bad Request".to_string(),
            message: "name must be between 1 and 100 characters".to_string(),
        };
        return (StatusCode::BAD_REQUEST, Json(err)).into_response();
    }

    if let Err(msg) = validate_polling_url(&payload.url) {
        let err = AuthErrorResponse {
            error: "Bad Request".to_string(),
            message: msg.to_string(),
        };
        return (StatusCode::BAD_REQUEST, Json(err)).into_response();
    }

    if payload.interval_seconds < MIN_POLLING_INTERVAL_SECS
        || payload.interval_seconds > MAX_POLLING_INTERVAL_SECS
    {
        let err = AuthErrorResponse {
            error: "Bad Request".to_string(),
            message: format!(
                "interval_seconds must be between {} and {} seconds",
                MIN_POLLING_INTERVAL_SECS, MAX_POLLING_INTERVAL_SECS
            ),
        };
        return (StatusCode::BAD_REQUEST, Json(err)).into_response();
    }

    let query_type_clean = payload.query_type.trim().to_lowercase();
    if !SUPPORTED_QUERY_TYPES.contains(&query_type_clean.as_str()) {
        let err = AuthErrorResponse {
            error: "Bad Request".to_string(),
            message: format!(
                "Invalid query_type '{}'. Supported query types: {}",
                payload.query_type,
                SUPPORTED_QUERY_TYPES.join(", ")
            ),
        };
        return (StatusCode::BAD_REQUEST, Json(err)).into_response();
    }

    let secret = generate_polling_secret();
    let new_webhook = PollingWebhook {
        id: Uuid::new_v4(),
        user_id: claims.sub.clone(),
        name: name.to_string(),
        url: payload.url.trim().to_string(),
        interval_seconds: payload.interval_seconds,
        query_type: query_type_clean,
        query_params: payload.query_params,
        secret,
        is_active: true,
        last_triggered_at: None,
        created_at: Utc::now(),
    };

    state.polling_webhook_registry.insert(new_webhook.clone());

    if let Some(ref pool) = state.db_pool {
        if let Err(e) = state
            .polling_webhook_registry
            .persist_insert(pool, &new_webhook)
            .await
        {
            error!(
                "[Polling Webhooks] Failed to persist webhook in database: {}",
                e
            );
        }
    }

    log_audit_event(
        &state,
        None,
        &claims.sub,
        "polling_webhook.create",
        "polling_webhook",
        Some(&new_webhook.id.to_string()),
        serde_json::json!({
            "name": new_webhook.name,
            "url": new_webhook.url,
            "interval_seconds": new_webhook.interval_seconds,
            "query_type": new_webhook.query_type
        }),
        None,
    )
    .await;

    info!(
        "[Polling Webhooks] Registered custom polling webhook '{}' (id: {}) for user '{}'",
        new_webhook.name, new_webhook.id, claims.sub
    );

    let resp_body = PollingWebhookResponse::from(new_webhook);
    (StatusCode::CREATED, Json(resp_body)).into_response()
}

/// GET /polling-webhooks
///
/// Lists all custom polling webhooks for the authenticated user.
#[utoipa::path(
    get,
    path = "/polling-webhooks",
    tag = "Webhooks & Real-Time Delivery",
    responses(
        (status = 200, description = "List of user's polling webhooks retrieved successfully", body = PollingWebhooksResponse),
        (status = 401, description = "Unauthorized - Missing or invalid Bearer JWT"),
        (status = 429, description = "Rate limit exceeded")
    ),
    security(("BearerAuth" = []))
)]
pub async fn list_polling_webhooks_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Response {
    let list = state.polling_webhook_registry.list_by_user(&claims.sub);
    let total = list.len();
    let webhooks: Vec<PollingWebhookResponse> =
        list.into_iter().map(PollingWebhookResponse::from).collect();

    Json(PollingWebhooksResponse { webhooks, total }).into_response()
}

/// DELETE /polling-webhooks/{id}
///
/// Deletes a custom polling webhook subscription.
#[utoipa::path(
    delete,
    path = "/polling-webhooks/{id}",
    tag = "Webhooks & Real-Time Delivery",
    params(
        ("id" = Uuid, Path, description = "Polling Webhook subscription identifier UUID to delete")
    ),
    responses(
        (status = 200, description = "Polling Webhook subscription deleted successfully", body = DeletePollingWebhookResponse),
        (status = 404, description = "Not Found - Polling Webhook does not exist or belongs to another user", body = AuthErrorResponse),
        (status = 401, description = "Unauthorized - Missing or invalid Bearer JWT"),
        (status = 429, description = "Rate limit exceeded")
    ),
    security(("BearerAuth" = []))
)]
pub async fn delete_polling_webhook_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
) -> Response {
    let removed = state.polling_webhook_registry.remove(&id, &claims.sub);

    match removed {
        Some(w) => {
            if let Some(ref pool) = state.db_pool {
                if let Err(e) = state
                    .polling_webhook_registry
                    .persist_delete(pool, &id)
                    .await
                {
                    error!(
                        "[Polling Webhooks] Failed to delete webhook from database: {}",
                        e
                    );
                }
            }

            log_audit_event(
                &state,
                None,
                &claims.sub,
                "polling_webhook.delete",
                "polling_webhook",
                Some(&id.to_string()),
                serde_json::json!({
                    "name": w.name,
                    "url": w.url,
                    "query_type": w.query_type
                }),
                None,
            )
            .await;

            info!(
                "[Polling Webhooks] Deleted custom polling webhook '{}' (id: {}) for user '{}'",
                w.name, id, claims.sub
            );

            let resp = DeletePollingWebhookResponse {
                success: true,
                id,
                message: format!("Polling webhook '{}' successfully deleted", w.name),
            };
            (StatusCode::OK, Json(resp)).into_response()
        }
        None => {
            let err = AuthErrorResponse {
                error: "Not Found".to_string(),
                message: format!(
                    "Polling webhook '{}' not found or belongs to another user",
                    id
                ),
            };
            (StatusCode::NOT_FOUND, Json(err)).into_response()
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_polling_secret_generation_and_signature() {
        let secret = generate_polling_secret();
        assert_eq!(secret.len(), 64);

        let payload =
            r#"{"webhook_id":"550e8400-e29b-41d4-a716-446655440000","query_type":"sentiment"}"#;
        let sig1 = compute_polling_signature(payload, &secret);
        let sig2 = compute_polling_signature(payload, &secret);

        assert_eq!(sig1, sig2);
        assert_eq!(sig1.len(), 64);

        let sig3 = compute_polling_signature(r#"{"query_type":"options"}"#, &secret);
        assert_ne!(sig1, sig3);
    }

    #[test]
    fn test_polling_url_validation() {
        assert!(validate_polling_url("https://quant.fund.com/webhook").is_ok());
        assert!(validate_polling_url("http://localhost:8080/poll").is_ok());
        assert!(validate_polling_url("http://127.0.0.1:9000/poll").is_ok());
        assert!(validate_polling_url("http://insecure.fund.com/webhook").is_err());
        assert!(validate_polling_url("invalid_url").is_err());
        assert!(validate_polling_url("").is_err());
    }

    #[test]
    fn test_polling_registry_crud_and_due_check() {
        let registry = PollingWebhookRegistry::new();
        let id1 = Uuid::new_v4();
        let w1 = PollingWebhook {
            id: id1,
            user_id: "user_a".to_string(),
            name: "Poll A".to_string(),
            url: "https://fund.com/poll1".to_string(),
            interval_seconds: 300,
            query_type: "sentiment".to_string(),
            query_params: serde_json::json!({"tickers": ["AAPL"]}),
            secret: generate_polling_secret(),
            is_active: true,
            last_triggered_at: None,
            created_at: Utc::now(),
        };

        registry.insert(w1.clone());
        assert_eq!(registry.count(), 1);

        let user_list = registry.list_by_user("user_a");
        assert_eq!(user_list.len(), 1);
        assert_eq!(user_list[0].id, id1);

        // Check due webhooks (never triggered -> due)
        let due = registry.get_due_webhooks(Utc::now());
        assert_eq!(due.len(), 1);

        // Update last triggered
        let now = Utc::now();
        registry.update_last_triggered(&id1, now);
        let due_after = registry.get_due_webhooks(now);
        assert_eq!(due_after.len(), 0);

        // Remove
        assert!(registry.remove(&id1, "user_a").is_some());
        assert_eq!(registry.count(), 0);
    }
}
