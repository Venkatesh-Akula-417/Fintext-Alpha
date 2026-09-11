//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Webhook Notification & Dispatch Architecture
//! ═══════════════════════════════════════════════════════════════════════════════

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use axum::Extension;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use hex::ToHex;
use hmac::{Hmac, Mac};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinHandle;
use tokio::time::sleep;
use tracing::{debug, error, info, warn};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::audit_logs::log_audit_event;
use crate::auth::{AuthErrorResponse, Claims};
use crate::state::AppState;

type HmacSha256 = Hmac<Sha256>;

pub const DEFAULT_WEBHOOK_DELIVERY_TIMEOUT_MS: u64 = 5000;
pub const DEFAULT_WEBHOOK_MAX_RETRIES: usize = 3;

/// Full stored webhook subscription model.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct WebhookSubscription {
    /// Unique Webhook subscription identifier UUID
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: Uuid,
    /// Owner user identifier
    #[schema(example = "quant_fund_01")]
    pub user_id: String,
    /// Destination endpoint URL
    #[schema(example = "https://webhook.site/quant-notifications")]
    pub url: String,
    /// Subscribed event categories
    #[schema(example = json!(["sentiment", "spillover"]))]
    pub events: Vec<String>,
    /// Secret key used to sign webhook payloads with HMAC-SHA256
    #[schema(example = "a3f8c7e1d2b409681273981a...")]
    pub secret: String,
    /// Creation timestamp (UTC)
    pub created_at: DateTime<Utc>,
}

/// Request payload for registering a new webhook (`POST /webhooks`).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateWebhookRequest {
    /// Destination webhook receiver URL (must be HTTPS, or HTTP for localhost/127.0.0.1)
    #[schema(example = "https://quant.fund.com/api/v1/webhook")]
    pub url: String,
    /// List of event categories to subscribe to (e.g., ["sentiment", "spillover", "sec_filing"])
    #[schema(example = json!(["sentiment", "spillover"]))]
    pub events: Vec<String>,
}

/// Response payload upon webhook registration or lookup.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct WebhookResponse {
    /// Unique Webhook subscription identifier UUID
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: Uuid,
    /// Owner user identifier
    #[schema(example = "quant_fund_01")]
    pub user_id: String,
    /// Destination endpoint URL
    #[schema(example = "https://quant.fund.com/api/v1/webhook")]
    pub url: String,
    /// Subscribed event categories
    #[schema(example = json!(["sentiment", "spillover"]))]
    pub events: Vec<String>,
    /// Secret key for HMAC-SHA256 verification (save this securely)
    #[schema(example = "a3f8c7e1d2b409681273981a5c6d7e8f...")]
    pub secret: String,
    /// ISO-8601 registration timestamp
    pub created_at: DateTime<Utc>,
}

impl From<WebhookSubscription> for WebhookResponse {
    fn from(sub: WebhookSubscription) -> Self {
        Self {
            id: sub.id,
            user_id: sub.user_id,
            url: sub.url,
            events: sub.events,
            secret: sub.secret,
            created_at: sub.created_at,
        }
    }
}

/// Response payload for listing registered webhooks (`GET /webhooks`).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct ListWebhooksResponse {
    pub webhooks: Vec<WebhookResponse>,
    pub count: usize,
}

/// Response payload for deleting a webhook (`DELETE /webhooks/{id}`).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct DeleteWebhookResponse {
    pub id: Uuid,
    pub status: String,
    pub message: String,
}

/// Outbound HTTP POST JSON payload dispatched to registered webhook receivers.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct WebhookPayload {
    /// Event category name (e.g., "sentiment", "spillover", "sec_filing")
    #[schema(example = "sentiment")]
    pub event_type: String,
    /// Event publication timestamp in microseconds since Unix epoch
    #[schema(example = 1787940389786186u64)]
    pub timestamp_us: u64,
    /// Event-specific data object
    pub data: serde_json::Value,
    /// HMAC-SHA256 hex signature computed with the subscription's secret
    #[schema(example = "d8a1c9e4f2b7...")]
    pub signature: String,
}

/// In-memory and PostgreSQL synchronized Webhook registry.
#[derive(Debug, Clone, Default)]
pub struct WebhookRegistry {
    entries: Arc<DashMap<Uuid, WebhookSubscription>>,
}

impl WebhookRegistry {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(DashMap::new()),
        }
    }

    pub fn insert(&self, sub: WebhookSubscription) {
        self.entries.insert(sub.id, sub);
    }

    pub fn list_by_user(&self, user_id: &str) -> Vec<WebhookSubscription> {
        self.entries
            .iter()
            .filter(|e| e.user_id == user_id)
            .map(|e| e.value().clone())
            .collect()
    }

    pub fn get(&self, id: &Uuid) -> Option<WebhookSubscription> {
        self.entries.get(id).map(|e| e.value().clone())
    }

    pub fn remove(&self, id: &Uuid, user_id: &str) -> Option<WebhookSubscription> {
        if let Some(entry) = self.entries.get(id) {
            if entry.user_id == user_id {
                drop(entry);
                return self.entries.remove(id).map(|(_, v)| v);
            }
        }
        None
    }

    pub fn find_subscribers_for_event(&self, event_type: &str) -> Vec<WebhookSubscription> {
        self.entries
            .iter()
            .filter(|e| {
                e.events
                    .iter()
                    .any(|ev| ev.eq_ignore_ascii_case(event_type) || ev == "*")
            })
            .map(|e| e.value().clone())
            .collect()
    }

    pub fn count(&self) -> usize {
        self.entries.len()
    }
}

/// Initialize the PostgreSQL `webhooks` table idempotently.
pub async fn init_webhooks_db(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS webhooks (
            id UUID PRIMARY KEY,
            user_id TEXT NOT NULL,
            url TEXT NOT NULL,
            events JSONB NOT NULL DEFAULT '[]',
            secret TEXT NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        );
        CREATE INDEX IF NOT EXISTS idx_webhooks_user_id ON webhooks(user_id);
        "#,
    )
    .execute(pool)
    .await?;

    info!("[Webhooks] PostgreSQL 'webhooks' table and index verified.");
    Ok(())
}

/// Generates a cryptographically random 256-bit (64 hex characters) webhook secret.
pub fn generate_webhook_secret() -> String {
    let u1 = Uuid::new_v4().as_u128();
    let u2 = Uuid::new_v4().as_u128();
    format!("{:032x}{:032x}", u1, u2)
}

/// Computes the HMAC-SHA256 hex signature for a webhook payload string using the given secret.
pub fn compute_signature(payload_json: &str, secret: &str) -> String {
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC can take key of any size");
    mac.update(payload_json.as_bytes());
    let result = mac.finalize();
    result.into_bytes().encode_hex::<String>()
}

/// Validates webhook target URL (HTTPS enforced except for localhost/127.0.0.1).
pub fn validate_webhook_url(url_str: &str) -> Result<String, String> {
    let trimmed = url_str.trim();
    if trimmed.is_empty() {
        return Err("Webhook URL cannot be empty".to_string());
    }

    let parsed = Url::parse(trimmed).map_err(|e| format!("Invalid webhook URL format: {}", e))?;

    let host = parsed
        .host_str()
        .ok_or_else(|| "Webhook URL must contain a valid host".to_string())?;

    let is_local = host == "localhost" || host == "127.0.0.1" || host == "::1";

    match parsed.scheme() {
        "https" => Ok(trimmed.to_string()),
        "http" if is_local => Ok(trimmed.to_string()),
        "http" => Err("Production webhook URLs must use HTTPS (HTTP is only permitted for localhost/127.0.0.1)".to_string()),
        scheme => Err(format!("Unsupported URL scheme '{}', expected https://", scheme)),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Route Handlers
// ─────────────────────────────────────────────────────────────────────────────

/// Register a new Webhook endpoint.
///
/// Registers a new webhook subscription URL with selected event filters for the authenticated user.
/// Returns the webhook UUID and unique HMAC-SHA256 signing secret.
#[utoipa::path(
    post,
    path = "/webhooks",
    tag = "Webhooks",
    request_body = CreateWebhookRequest,
    responses(
        (status = 201, description = "Webhook registered successfully", body = WebhookResponse),
        (status = 400, description = "Invalid request payload or URL format", body = AuthErrorResponse),
        (status = 401, description = "Unauthorized", body = AuthErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::rate_limit::RateLimitErrorResponse)
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn register_webhook_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(payload): Json<CreateWebhookRequest>,
) -> Response {
    let url = match validate_webhook_url(&payload.url) {
        Ok(u) => u,
        Err(err) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(AuthErrorResponse {
                    error: "Bad Request".to_string(),
                    message: err,
                }),
            )
                .into_response();
        }
    };

    let events: Vec<String> = payload
        .events
        .into_iter()
        .map(|e| e.trim().to_lowercase())
        .filter(|e| !e.is_empty())
        .collect();

    if events.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(AuthErrorResponse {
                error: "Bad Request".to_string(),
                message: "Must specify at least one event type (e.g. ['sentiment', 'spillover'])"
                    .to_string(),
            }),
        )
            .into_response();
    }

    let sub = WebhookSubscription {
        id: Uuid::new_v4(),
        user_id: claims.sub.clone(),
        url,
        events,
        secret: generate_webhook_secret(),
        created_at: Utc::now(),
    };

    // Store in PostgreSQL if connected
    if let Some(pool) = &state.db_pool {
        let events_json = serde_json::to_value(&sub.events).unwrap_or(serde_json::json!([]));
        let res = sqlx::query(
            r#"
            INSERT INTO webhooks (id, user_id, url, events, secret, created_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(sub.id)
        .bind(&sub.user_id)
        .bind(&sub.url)
        .bind(events_json)
        .bind(&sub.secret)
        .bind(sub.created_at)
        .execute(pool)
        .await;

        if let Err(e) = res {
            error!("[Webhooks] Failed to persist webhook to PostgreSQL: {}", e);
        }
    }

    // Also update in-memory registry for ultra-low-latency real-time dispatch
    state.webhook_registry.insert(sub.clone());

    info!(
        "[Webhooks] User '{}' registered webhook '{}' (id: {}) for events {:?}",
        claims.sub, sub.url, sub.id, sub.events
    );

    log_audit_event(
        &state,
        None,
        &claims.sub,
        "webhook.registered",
        "webhook",
        Some(&sub.id.to_string()),
        serde_json::json!({"url": sub.url, "events": sub.events}),
        None,
    )
    .await;

    (StatusCode::CREATED, Json(WebhookResponse::from(sub))).into_response()
}

/// List all Webhooks for the authenticated user.
///
/// Returns all active webhook subscriptions owned by the caller.
#[utoipa::path(
    get,
    path = "/webhooks",
    tag = "Webhooks",
    responses(
        (status = 200, description = "List of registered webhooks", body = ListWebhooksResponse),
        (status = 401, description = "Unauthorized", body = AuthErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::rate_limit::RateLimitErrorResponse)
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn list_webhooks_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Response {
    let mut webhooks: Vec<WebhookResponse> = state
        .webhook_registry
        .list_by_user(&claims.sub)
        .into_iter()
        .map(WebhookResponse::from)
        .collect();

    // If in-memory is empty but DB pool exists, query PostgreSQL
    if webhooks.is_empty() {
        if let Some(pool) = &state.db_pool {
            let rows = sqlx::query_as::<_, (Uuid, String, String, serde_json::Value, String, DateTime<Utc>)>(
                "SELECT id, user_id, url, events, secret, created_at FROM webhooks WHERE user_id = $1 ORDER BY created_at DESC",
            )
            .bind(&claims.sub)
            .fetch_all(pool)
            .await;

            if let Ok(records) = rows {
                for (id, user_id, url, events_val, secret, created_at) in records {
                    let events: Vec<String> =
                        serde_json::from_value(events_val).unwrap_or_default();
                    let sub = WebhookSubscription {
                        id,
                        user_id,
                        url,
                        events,
                        secret,
                        created_at,
                    };
                    state.webhook_registry.insert(sub.clone());
                    webhooks.push(WebhookResponse::from(sub));
                }
            }
        }
    }

    let count = webhooks.len();
    (
        StatusCode::OK,
        Json(ListWebhooksResponse { webhooks, count }),
    )
        .into_response()
}

/// Delete a Webhook subscription.
///
/// Removes an active webhook subscription belonging to the authenticated user.
#[utoipa::path(
    delete,
    path = "/webhooks/{id}",
    tag = "Webhooks",
    params(
        ("id" = Uuid, Path, description = "Webhook subscription UUID to delete")
    ),
    responses(
        (status = 200, description = "Webhook deleted successfully", body = DeleteWebhookResponse),
        (status = 401, description = "Unauthorized", body = AuthErrorResponse),
        (status = 404, description = "Webhook not found or owned by another user", body = AuthErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::rate_limit::RateLimitErrorResponse)
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn delete_webhook_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
) -> Response {
    // Remove from in-memory registry
    let in_memory_removed = state.webhook_registry.remove(&id, &claims.sub);

    // Delete from PostgreSQL if connected
    let mut db_deleted = false;
    if let Some(pool) = &state.db_pool {
        let res = sqlx::query("DELETE FROM webhooks WHERE id = $1 AND user_id = $2")
            .bind(id)
            .bind(&claims.sub)
            .execute(pool)
            .await;

        if let Ok(r) = res {
            if r.rows_affected() > 0 {
                db_deleted = true;
            }
        }
    }

    if in_memory_removed.is_none() && !db_deleted {
        return (
            StatusCode::NOT_FOUND,
            Json(AuthErrorResponse {
                error: "Not Found".to_string(),
                message: format!(
                    "Webhook subscription '{}' not found or owned by another user",
                    id
                ),
            }),
        )
            .into_response();
    }

    info!(
        "[Webhooks] User '{}' deleted webhook subscription '{}'",
        claims.sub, id
    );

    log_audit_event(
        &state,
        None,
        &claims.sub,
        "webhook.deleted",
        "webhook",
        Some(&id.to_string()),
        serde_json::json!({"webhook_id": id}),
        None,
    )
    .await;

    (
        StatusCode::OK,
        Json(DeleteWebhookResponse {
            id,
            status: "deleted".to_string(),
            message: "Webhook subscription deleted successfully".to_string(),
        }),
    )
        .into_response()
}

// ─────────────────────────────────────────────────────────────────────────────
// Webhook Background Dispatcher
// ─────────────────────────────────────────────────────────────────────────────

/// Spawns the background Webhook Dispatcher Tokio task.
pub fn spawn_webhook_dispatcher(state: AppState) -> JoinHandle<()> {
    tokio::spawn(async move {
        info!("[Webhook Dispatcher] Initialized real-time Kafka event dispatcher worker.");
        let mut rx = state.kafka_consumer.subscribe_client();
        let timeout_ms = std::env::var("WEBHOOK_DELIVERY_TIMEOUT_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(DEFAULT_WEBHOOK_DELIVERY_TIMEOUT_MS);
        let max_retries = std::env::var("WEBHOOK_MAX_RETRIES")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(DEFAULT_WEBHOOK_MAX_RETRIES);

        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_millis(timeout_ms))
            .build()
            .unwrap_or_default();

        while let Ok(msg) = rx.recv().await {
            let parsed_json: serde_json::Value = match serde_json::from_str(&msg) {
                Ok(v) => v,
                Err(_) => continue,
            };

            // Detect event type
            let event_type = if parsed_json.get("event_type").is_some() {
                parsed_json["event_type"]
                    .as_str()
                    .unwrap_or("general")
                    .to_string()
            } else if parsed_json.get("sentiment_score").is_some() {
                "sentiment".to_string()
            } else if parsed_json.get("spillovers").is_some()
                || parsed_json.get("correlation").is_some()
            {
                "spillover".to_string()
            } else if parsed_json.get("form_type").is_some()
                || parsed_json.get("filing_type").is_some()
            {
                "sec_filing".to_string()
            } else {
                "general".to_string()
            };

            let matching_subs = state
                .webhook_registry
                .find_subscribers_for_event(&event_type);
            if matching_subs.is_empty() {
                continue;
            }

            debug!(
                "[Webhook Dispatcher] Dispatching event '{}' to {} matching webhooks",
                event_type,
                matching_subs.len()
            );

            for sub in matching_subs {
                let client = http_client.clone();
                let sub_clone = sub.clone();
                let event_type_clone = event_type.clone();
                let data_clone = parsed_json.clone();

                tokio::spawn(async move {
                    dispatch_single_webhook(
                        sub_clone,
                        &event_type_clone,
                        data_clone,
                        max_retries,
                        client,
                    )
                    .await;
                });
            }
        }
    })
}

/// Dispatches a single webhook notification with exponential backoff retries.
pub async fn dispatch_single_webhook(
    sub: WebhookSubscription,
    event_type: &str,
    data: serde_json::Value,
    max_retries: usize,
    client: reqwest::Client,
) -> bool {
    let now_us = Utc::now().timestamp_micros() as u64;

    // 1. Build unsigned body
    let mut payload_val = serde_json::json!({
        "event_type": event_type,
        "timestamp_us": now_us,
        "data": data,
    });

    // 2. Compute HMAC-SHA256 signature over stringified JSON
    let body_str = payload_val.to_string();
    let signature = compute_signature(&body_str, &sub.secret);

    // 3. Attach signature to payload
    payload_val["signature"] = serde_json::Value::String(signature.clone());
    let final_body = payload_val.to_string();

    let sig_header = format!("sha256={}", signature);

    // 4. Retry loop with exponential backoff
    for attempt in 1..=max_retries {
        let req = client
            .post(&sub.url)
            .header("Content-Type", "application/json")
            .header("X-FinText-Signature", &sig_header)
            .header("X-FinText-Event", event_type)
            .header("User-Agent", "FinText-Webhook-Dispatcher/2.0")
            .body(final_body.clone());

        match req.send().await {
            Ok(resp) if resp.status().is_success() => {
                debug!(
                    "[Webhook Dispatcher] Successfully delivered event '{}' to '{}' (HTTP {})",
                    event_type,
                    sub.url,
                    resp.status()
                );
                return true;
            }
            Ok(resp) => {
                warn!(
                    "[Webhook Dispatcher] Attempt {}/{} failed for '{}': HTTP {}",
                    attempt,
                    max_retries,
                    sub.url,
                    resp.status()
                );
            }
            Err(err) => {
                warn!(
                    "[Webhook Dispatcher] Attempt {}/{} network error for '{}': {}",
                    attempt, max_retries, sub.url, err
                );
            }
        }

        if attempt < max_retries {
            let backoff_ms = 50 * (1 << attempt);
            sleep(Duration::from_millis(backoff_ms)).await;
        }
    }

    error!(
        "[Webhook Dispatcher] Failed to deliver event '{}' to '{}' after {} attempts",
        event_type, sub.url, max_retries
    );
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secret_generation_and_signature_computation() {
        let secret = generate_webhook_secret();
        assert_eq!(secret.len(), 64);

        let payload = r#"{"event_type":"sentiment","ticker":"AAPL","sentiment_score":0.75}"#;
        let sig1 = compute_signature(payload, &secret);
        let sig2 = compute_signature(payload, &secret);

        assert_eq!(sig1, sig2);
        assert_eq!(sig1.len(), 64);

        // Different payload -> different signature
        let sig3 = compute_signature(r#"{"event_type":"sentiment","ticker":"MSFT"}"#, &secret);
        assert_ne!(sig1, sig3);
    }

    #[test]
    fn test_webhook_url_validation() {
        // Valid HTTPS
        assert!(validate_webhook_url("https://quant.fund.com/webhook").is_ok());

        // Valid Localhost HTTP
        assert!(validate_webhook_url("http://localhost:8080/webhook").is_ok());
        assert!(validate_webhook_url("http://127.0.0.1:9000/webhook").is_ok());

        // Invalid Public HTTP (must be HTTPS)
        assert!(validate_webhook_url("http://insecure.fund.com/webhook").is_err());

        // Invalid Malformed
        assert!(validate_webhook_url("not_a_valid_url").is_err());
        assert!(validate_webhook_url("").is_err());
    }

    #[test]
    fn test_webhook_registry_crud_and_event_matching() {
        let registry = WebhookRegistry::new();
        let sub1 = WebhookSubscription {
            id: Uuid::new_v4(),
            user_id: "fund_01".to_string(),
            url: "https://fund1.com/webhook".to_string(),
            events: vec!["sentiment".to_string(), "spillover".to_string()],
            secret: generate_webhook_secret(),
            created_at: Utc::now(),
        };
        let sub2 = WebhookSubscription {
            id: Uuid::new_v4(),
            user_id: "fund_02".to_string(),
            url: "https://fund2.com/webhook".to_string(),
            events: vec!["sec_filing".to_string()],
            secret: generate_webhook_secret(),
            created_at: Utc::now(),
        };

        registry.insert(sub1.clone());
        registry.insert(sub2.clone());

        assert_eq!(registry.count(), 2);

        // List by user
        let user1_subs = registry.list_by_user("fund_01");
        assert_eq!(user1_subs.len(), 1);
        assert_eq!(user1_subs[0].id, sub1.id);

        // Find subscribers for event
        let sentiment_subs = registry.find_subscribers_for_event("sentiment");
        assert_eq!(sentiment_subs.len(), 1);
        assert_eq!(sentiment_subs[0].id, sub1.id);

        let sec_subs = registry.find_subscribers_for_event("sec_filing");
        assert_eq!(sec_subs.len(), 1);
        assert_eq!(sec_subs[0].id, sub2.id);

        // Remove
        assert!(registry.remove(&sub1.id, "fund_01").is_some());
        assert_eq!(registry.count(), 1);
        assert_eq!(registry.list_by_user("fund_01").len(), 0);
    }
}
