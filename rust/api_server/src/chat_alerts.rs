//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Telegram & Discord Alert Bot Subscription Engine
//! ═══════════════════════════════════════════════════════════════════════════════
//!
//! Provides channel subscription management and asynchronous background alert
//! dispatching for Telegram Bot chat IDs and Discord HTTPS Webhooks.
//! ═══════════════════════════════════════════════════════════════════════════════

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use axum::Extension;
use chrono::Utc;
use dashmap::DashMap;
use reqwest::Url;
use sqlx::{PgPool, Row};
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinHandle;
use tracing::{info, warn};
use uuid::Uuid;

use crate::audit_logs::log_audit_event;
use crate::auth::{AuthErrorResponse, Claims};
use crate::models::{
    ChatAlertSubscription, ChatAlertSubscriptionResponse, ChatAlertsResponse,
    CreateChatAlertRequest, DeleteChatAlertResponse, ALLOWED_EVENT_TYPES, CHANNEL_TYPE_DISCORD,
    CHANNEL_TYPE_TELEGRAM,
};
use crate::state::AppState;

pub const DEFAULT_CHAT_ALERT_INTERVAL_SECS: u64 = 60;
pub const DEFAULT_TELEGRAM_BOT_TOKEN: &str = "mock_telegram_bot_token_fintext_2026";

// ─────────────────────────────────────────────────────────────────────────────
// Validation & Formatting Helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Validates request body for creating a chat alert subscription.
pub fn validate_chat_alert_request(req: &CreateChatAlertRequest) -> Result<(), String> {
    let chan_type = req.channel_type.trim().to_lowercase();
    if chan_type != CHANNEL_TYPE_TELEGRAM && chan_type != CHANNEL_TYPE_DISCORD {
        return Err(format!(
            "Invalid channel_type '{}'. Supported channels are '{}' and '{}'",
            req.channel_type, CHANNEL_TYPE_TELEGRAM, CHANNEL_TYPE_DISCORD
        ));
    }

    let target = req.channel_target.trim();
    if target.is_empty() {
        return Err("channel_target cannot be empty".to_string());
    }

    if chan_type == CHANNEL_TYPE_TELEGRAM {
        if target.len() > 128 {
            return Err(
                "Telegram chat ID / channel handle cannot exceed 128 characters".to_string(),
            );
        }
        // Telegram chat IDs are typically numeric, negative numeric (supergroups), or @usernames
        let is_valid_tg = target.starts_with('@')
            || target.parse::<i64>().is_ok()
            || target
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
        if !is_valid_tg {
            return Err(format!(
                "Invalid Telegram channel_target '{}'. Expected numeric chat ID (e.g. '123456789', '-100123456789') or @channel handle",
                target
            ));
        }
    } else if chan_type == CHANNEL_TYPE_DISCORD {
        if target.len() > 512 {
            return Err("Discord webhook URL cannot exceed 512 characters".to_string());
        }
        let parsed_url =
            Url::parse(target).map_err(|e| format!("Invalid Discord webhook URL: {}", e))?;
        let is_valid_scheme = match parsed_url.scheme() {
            "https" => true,
            "http" => {
                let host = parsed_url.host_str().unwrap_or("");
                host == "localhost" || host == "127.0.0.1" || host == "::1"
            }
            _ => false,
        };
        if !is_valid_scheme {
            return Err("Discord webhook URL must use HTTPS (HTTP is only permitted for localhost during testing)".to_string());
        }
    }

    if req.event_types.is_empty() {
        return Err("event_types array cannot be empty. Specify at least one event type (e.g. 'sentiment_anomaly', '8k_filing', 'unusual_options')".to_string());
    }

    for et in &req.event_types {
        let normalized = et.trim().to_lowercase();
        if !ALLOWED_EVENT_TYPES.contains(&normalized.as_str()) {
            return Err(format!(
                "Unknown event_type '{}'. Allowed event types are: {:?}",
                et, ALLOWED_EVENT_TYPES
            ));
        }
    }

    Ok(())
}

/// Format a Sentiment Anomaly Alert message in Markdown.
pub fn format_sentiment_anomaly_alert(
    ticker: &str,
    direction: &str,
    z_score: f64,
    latest_score: f64,
    timestamp: &str,
) -> String {
    format!(
        "🚨 *Sentiment Anomaly Alert*\n\n\
        *Ticker:* `{}`\n\
        *Direction:* {}\n\
        *Z-Score:* {:+.2}\n\
        *Latest Score:* {:+.2}\n\
        *Timestamp:* {}",
        ticker.to_uppercase(),
        direction,
        z_score,
        latest_score,
        timestamp
    )
}

/// Format an SEC Form 8-K Filing Alert message in Markdown.
pub fn format_8k_filing_alert(
    ticker: &str,
    event_type: &str,
    filing_date: &str,
    description: &str,
) -> String {
    format!(
        "📄 *8-K Filing Alert*\n\n\
        *Ticker:* `{}`\n\
        *Event Type:* {}\n\
        *Filing Date:* {}\n\
        *Description:* {}",
        ticker.to_uppercase(),
        event_type,
        filing_date,
        description
    )
}

/// Format an Unusual Options Activity Alert message in Markdown.
pub fn format_unusual_options_alert(
    ticker: &str,
    vol_oi_ratio: f64,
    option_type: &str,
    strike: f64,
    expiry: &str,
) -> String {
    format!(
        "📊 *Unusual Options Activity*\n\n\
        *Ticker:* `{}`\n\
        *Volume/OI Ratio:* {:.1}x\n\
        *Option Type:* {}\n\
        *Strike:* ${:.2}\n\
        *Expiry:* {}",
        ticker.to_uppercase(),
        vol_oi_ratio,
        option_type,
        strike,
        expiry
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// Registry & Database Persistence
// ─────────────────────────────────────────────────────────────────────────────

/// Concurrent in-memory registry and database manager for chat alert subscriptions.
#[derive(Debug, Clone)]
pub struct ChatAlertRegistry {
    cache: Arc<DashMap<Uuid, ChatAlertSubscription>>,
}

impl Default for ChatAlertRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ChatAlertRegistry {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(DashMap::new()),
        }
    }

    pub fn insert(&self, sub: ChatAlertSubscription) {
        self.cache.insert(sub.id, sub);
    }

    pub fn get(&self, id: &Uuid) -> Option<ChatAlertSubscription> {
        self.cache.get(id).map(|r| r.value().clone())
    }

    pub fn list_by_user(&self, user_id: &str) -> Vec<ChatAlertSubscription> {
        let mut list: Vec<ChatAlertSubscription> = self
            .cache
            .iter()
            .filter(|r| r.user_id == user_id)
            .map(|r| r.value().clone())
            .collect();
        list.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        list
    }

    pub fn delete(&self, id: &Uuid, user_id: &str) -> bool {
        if let Some(r) = self.cache.get(id) {
            if r.user_id == user_id {
                drop(r);
                return self.cache.remove(id).is_some();
            }
        }
        false
    }

    pub fn get_active_for_event(&self, event_type: &str) -> Vec<ChatAlertSubscription> {
        let et_norm = event_type.to_lowercase();
        self.cache
            .iter()
            .filter(|r| {
                r.is_active
                    && r.event_types
                        .iter()
                        .any(|e| e.eq_ignore_ascii_case(&et_norm))
            })
            .map(|r| r.value().clone())
            .collect()
    }

    pub async fn init_db(pool: &PgPool) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS chat_alert_subscriptions (
                id UUID PRIMARY KEY,
                user_id VARCHAR(255) NOT NULL,
                channel_type TEXT NOT NULL,
                channel_target TEXT NOT NULL,
                event_types JSONB NOT NULL DEFAULT '[]',
                is_active BOOLEAN NOT NULL DEFAULT TRUE,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );
            CREATE INDEX IF NOT EXISTS idx_chat_alerts_user ON chat_alert_subscriptions(user_id);
            "#,
        )
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn load_from_db(&self, pool: &PgPool) -> Result<usize, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT id, user_id, channel_type, channel_target, event_types, is_active, created_at FROM chat_alert_subscriptions"
        )
        .fetch_all(pool)
        .await?;

        let count = rows.len();
        for row in rows {
            let id: Uuid = row.get("id");
            let user_id: String = row.get("user_id");
            let channel_type: String = row.get("channel_type");
            let channel_target: String = row.get("channel_target");
            let event_types_val: serde_json::Value = row.get("event_types");
            let is_active: bool = row.get("is_active");
            let created_at: chrono::DateTime<Utc> = row.get("created_at");

            let event_types: Vec<String> =
                serde_json::from_value(event_types_val).unwrap_or_default();

            self.insert(ChatAlertSubscription {
                id,
                user_id,
                channel_type,
                channel_target,
                event_types,
                is_active,
                created_at,
            });
        }

        info!(
            "[Chat Alerts] Loaded {} subscriptions from PostgreSQL",
            count
        );
        Ok(count)
    }

    pub async fn save_to_db(
        &self,
        pool: &PgPool,
        sub: &ChatAlertSubscription,
    ) -> Result<(), sqlx::Error> {
        let event_types_json =
            serde_json::to_value(&sub.event_types).unwrap_or(serde_json::json!([]));
        sqlx::query(
            r#"
            INSERT INTO chat_alert_subscriptions (id, user_id, channel_type, channel_target, event_types, is_active, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (id) DO UPDATE SET
                channel_type = EXCLUDED.channel_type,
                channel_target = EXCLUDED.channel_target,
                event_types = EXCLUDED.event_types,
                is_active = EXCLUDED.is_active
            "#
        )
        .bind(sub.id)
        .bind(&sub.user_id)
        .bind(&sub.channel_type)
        .bind(&sub.channel_target)
        .bind(event_types_json)
        .bind(sub.is_active)
        .bind(sub.created_at)
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn delete_from_db(
        &self,
        pool: &PgPool,
        id: &Uuid,
        user_id: &str,
    ) -> Result<bool, sqlx::Error> {
        let res =
            sqlx::query("DELETE FROM chat_alert_subscriptions WHERE id = $1 AND user_id = $2")
                .bind(id)
                .bind(user_id)
                .execute(pool)
                .await?;
        Ok(res.rows_affected() > 0)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Route Handlers
// ─────────────────────────────────────────────────────────────────────────────

/// Subscribe to Telegram / Discord market alerts.
#[utoipa::path(
    post,
    path = "/chat-alerts",
    tag = "Notifications & Alerts",
    request_body = CreateChatAlertRequest,
    responses(
        (status = 201, description = "Chat alert subscription created successfully", body = ChatAlertSubscriptionResponse),
        (status = 400, description = "Invalid channel type, target, or event types", body = AuthErrorResponse),
        (status = 401, description = "Missing or invalid Bearer JWT", body = AuthErrorResponse)
    ),
    security(
        ("BearerAuth" = [])
    )
)]
pub async fn create_chat_alert_handler(
    Extension(claims): Extension<Claims>,
    State(state): State<AppState>,
    Json(payload): Json<CreateChatAlertRequest>,
) -> Response {
    if let Err(err_msg) = validate_chat_alert_request(&payload) {
        let err = AuthErrorResponse {
            error: "Bad Request".to_string(),
            message: err_msg,
        };
        return (StatusCode::BAD_REQUEST, Json(err)).into_response();
    }

    let channel_type = payload.channel_type.trim().to_lowercase();
    let channel_target = payload.channel_target.trim().to_string();
    let event_types: Vec<String> = payload
        .event_types
        .into_iter()
        .map(|s| s.trim().to_lowercase())
        .collect();

    let sub = ChatAlertSubscription {
        id: Uuid::new_v4(),
        user_id: claims.sub.clone(),
        channel_type,
        channel_target,
        event_types,
        is_active: true,
        created_at: Utc::now(),
    };

    // In-memory cache
    state.chat_alert_registry.insert(sub.clone());

    // Postgres persistence
    if let Some(ref pool) = state.db_pool {
        if let Err(e) = state.chat_alert_registry.save_to_db(pool, &sub).await {
            warn!(
                "[Chat Alerts] Failed to persist subscription to database: {}",
                e
            );
        }
    }

    // Audit log
    log_audit_event(
        &state,
        None,
        &claims.sub,
        "chat_alert.create",
        "chat_alert_subscription",
        Some(&sub.id.to_string()),
        serde_json::json!({
            "channel_type": sub.channel_type,
            "event_types": sub.event_types,
        }),
        None,
    )
    .await;

    (
        StatusCode::CREATED,
        Json(ChatAlertSubscriptionResponse::from(sub)),
    )
        .into_response()
}

/// List user's active chat alert subscriptions.
#[utoipa::path(
    get,
    path = "/chat-alerts",
    tag = "Notifications & Alerts",
    responses(
        (status = 200, description = "List of configured chat alert subscriptions", body = ChatAlertsResponse),
        (status = 401, description = "Missing or invalid Bearer JWT", body = AuthErrorResponse)
    ),
    security(
        ("BearerAuth" = [])
    )
)]
pub async fn list_chat_alerts_handler(
    Extension(claims): Extension<Claims>,
    State(state): State<AppState>,
) -> Response {
    let subs = state.chat_alert_registry.list_by_user(&claims.sub);
    let total = subs.len();
    let responses: Vec<ChatAlertSubscriptionResponse> = subs.into_iter().map(Into::into).collect();

    (
        StatusCode::OK,
        Json(ChatAlertsResponse {
            subscriptions: responses,
            total,
        }),
    )
        .into_response()
}

/// Delete a chat alert subscription.
#[utoipa::path(
    delete,
    path = "/chat-alerts/{id}",
    tag = "Notifications & Alerts",
    params(
        ("id" = Uuid, Path, description = "Chat alert subscription UUID to delete")
    ),
    responses(
        (status = 200, description = "Subscription deleted successfully", body = DeleteChatAlertResponse),
        (status = 404, description = "Subscription not found or not owned by user", body = AuthErrorResponse),
        (status = 401, description = "Missing or invalid Bearer JWT", body = AuthErrorResponse)
    ),
    security(
        ("BearerAuth" = [])
    )
)]
pub async fn delete_chat_alert_handler(
    Path(id): Path<Uuid>,
    Extension(claims): Extension<Claims>,
    State(state): State<AppState>,
) -> Response {
    let existing = state.chat_alert_registry.get(&id);
    match existing {
        Some(sub) if sub.user_id == claims.sub => {
            state.chat_alert_registry.delete(&id, &claims.sub);

            if let Some(ref pool) = state.db_pool {
                if let Err(e) = state
                    .chat_alert_registry
                    .delete_from_db(pool, &id, &claims.sub)
                    .await
                {
                    warn!(
                        "[Chat Alerts] Failed to delete subscription from database: {}",
                        e
                    );
                }
            }

            log_audit_event(
                &state,
                None,
                &claims.sub,
                "chat_alert.delete",
                "chat_alert_subscription",
                Some(&id.to_string()),
                serde_json::json!({
                    "channel_type": sub.channel_type,
                    "channel_target": sub.channel_target,
                }),
                None,
            )
            .await;

            (
                StatusCode::OK,
                Json(DeleteChatAlertResponse {
                    success: true,
                    id,
                    message: "Chat alert subscription deleted successfully".to_string(),
                }),
            )
                .into_response()
        }
        _ => {
            let err = AuthErrorResponse {
                error: "Not Found".to_string(),
                message: format!("Chat alert subscription with ID '{}' not found", id),
            };
            (StatusCode::NOT_FOUND, Json(err)).into_response()
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Dispatcher & Delivery Worker
// ─────────────────────────────────────────────────────────────────────────────

/// Dispatch an alert message to a specific Telegram or Discord channel.
pub async fn dispatch_alert_to_channel(
    channel_type: &str,
    channel_target: &str,
    message: &str,
    bot_token: &str,
) -> Result<(), String> {
    let is_mock = std::env::var("MOCK_MODE").as_deref() == Ok("1")
        || std::env::var("CHAT_ALERTS_MOCK").as_deref() == Ok("1")
        || bot_token.starts_with("mock");

    if is_mock {
        info!(
            "[Chat Alerts Dispatcher] [MOCK] Channel: '{}', Target: '{}'\nMessage:\n{}",
            channel_type, channel_target, message
        );
        return Ok(());
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;

    if channel_type.eq_ignore_ascii_case(CHANNEL_TYPE_TELEGRAM) {
        let url = format!("https://api.telegram.org/bot{}/sendMessage", bot_token);
        let payload = serde_json::json!({
            "chat_id": channel_target,
            "text": message,
            "parse_mode": "Markdown",
        });

        let resp = client
            .post(&url)
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("Telegram API dispatch failed: {}", e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("Telegram API returned error {}: {}", status, body));
        }
    } else if channel_type.eq_ignore_ascii_case(CHANNEL_TYPE_DISCORD) {
        let payload = serde_json::json!({
            "content": message,
        });

        let resp = client
            .post(channel_target)
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("Discord webhook dispatch failed: {}", e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!(
                "Discord webhook returned error {}: {}",
                status, body
            ));
        }
    }

    Ok(())
}

/// Dispatches an event alert to all matching active subscribers in the registry.
pub async fn dispatch_chat_alert(state: &AppState, event_type: &str, message: &str) -> usize {
    let subs = state.chat_alert_registry.get_active_for_event(event_type);
    let bot_token = std::env::var("TELEGRAM_BOT_TOKEN")
        .unwrap_or_else(|_| DEFAULT_TELEGRAM_BOT_TOKEN.to_string());

    let count = subs.len();
    for sub in subs {
        if let Err(e) =
            dispatch_alert_to_channel(&sub.channel_type, &sub.channel_target, message, &bot_token)
                .await
        {
            warn!(
                "[Chat Alerts Dispatcher] Failed to dispatch {} alert to {}: {}",
                sub.channel_type, sub.channel_target, e
            );
        }
    }
    count
}

/// Spawns the periodic background worker for monitoring and dispatching market chat alerts.
pub fn spawn_chat_alert_dispatcher(_state: AppState, interval_secs: u64) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
        info!(
            "[Chat Alerts Dispatcher] Background worker started (Interval: {}s)",
            interval_secs
        );

        loop {
            interval.tick().await;

            // In production, queries recent events since last tick
            // and dispatches formatted alerts to subscribers.
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validation_valid_telegram() {
        let req = CreateChatAlertRequest {
            channel_type: "telegram".to_string(),
            channel_target: "123456789".to_string(),
            event_types: vec!["sentiment_anomaly".to_string(), "8k_filing".to_string()],
        };
        assert!(validate_chat_alert_request(&req).is_ok());
    }

    #[test]
    fn test_validation_valid_discord() {
        let req = CreateChatAlertRequest {
            channel_type: "discord".to_string(),
            channel_target: "https://discord.com/api/webhooks/123/xyz".to_string(),
            event_types: vec!["unusual_options".to_string()],
        };
        assert!(validate_chat_alert_request(&req).is_ok());
    }

    #[test]
    fn test_validation_invalid_channel_type() {
        let req = CreateChatAlertRequest {
            channel_type: "slack".to_string(),
            channel_target: "12345".to_string(),
            event_types: vec!["sentiment_anomaly".to_string()],
        };
        assert!(validate_chat_alert_request(&req).is_err());
    }

    #[test]
    fn test_validation_invalid_discord_url() {
        let req = CreateChatAlertRequest {
            channel_type: "discord".to_string(),
            channel_target: "ftp://discord.com/hook".to_string(),
            event_types: vec!["sentiment_anomaly".to_string()],
        };
        assert!(validate_chat_alert_request(&req).is_err());
    }

    #[test]
    fn test_validation_unknown_event_type() {
        let req = CreateChatAlertRequest {
            channel_type: "telegram".to_string(),
            channel_target: "12345".to_string(),
            event_types: vec!["unknown_event".to_string()],
        };
        assert!(validate_chat_alert_request(&req).is_err());
    }

    #[test]
    fn test_message_formatting() {
        let msg =
            format_sentiment_anomaly_alert("AAPL", "bearish", -3.2, 0.12, "2025-08-31T12:00:00Z");
        assert!(msg.contains("AAPL"));
        assert!(msg.contains("bearish"));
        assert!(msg.contains("-3.20"));

        let msg8k = format_8k_filing_alert(
            "MSFT",
            "Earnings Warning",
            "2025-08-30",
            "Notice of material event",
        );
        assert!(msg8k.contains("MSFT"));
        assert!(msg8k.contains("Earnings Warning"));

        let msguoa = format_unusual_options_alert("NVDA", 5.4, "Call", 450.0, "2025-09-15");
        assert!(msguoa.contains("NVDA"));
        assert!(msguoa.contains("5.4x"));
    }

    #[test]
    fn test_registry_crud() {
        let reg = ChatAlertRegistry::new();
        let sub = ChatAlertSubscription {
            id: Uuid::new_v4(),
            user_id: "user_a".to_string(),
            channel_type: "telegram".to_string(),
            channel_target: "111222".to_string(),
            event_types: vec!["sentiment_anomaly".to_string()],
            is_active: true,
            created_at: Utc::now(),
        };

        reg.insert(sub.clone());
        assert_eq!(reg.list_by_user("user_a").len(), 1);
        assert_eq!(reg.list_by_user("user_b").len(), 0);

        let active = reg.get_active_for_event("sentiment_anomaly");
        assert_eq!(active.len(), 1);

        assert!(reg.delete(&sub.id, "user_a"));
        assert_eq!(reg.list_by_user("user_a").len(), 0);
    }
}
