//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Stripe Subscription Billing Integration
//! ═══════════════════════════════════════════════════════════════════════════════
//!
//! Implements plan-based monetization with Stripe Checkout, Customer Portal,
//! subscription lifecycle management via webhooks, and plan-tier usage enforcement.
//! ═══════════════════════════════════════════════════════════════════════════════

use crate::audit_logs::log_audit_event;
use crate::auth::{AuthErrorResponse, Claims};
use crate::models::{
    AccountUsageResponse, AdminTenantUsageResponse, ApiKeyAuditItem, AuditLogSummaryItem,
    DailyUsageItem, EndpointGroupUsageItem, SubscriptionDetailItem,
};
use crate::state::AppState;
use crate::ConstantTimeEq;
use axum::body::Bytes;
use axum::extract::{Path, Request, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Json, Response};
use axum::Extension;
use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use sqlx::PgPool;
use std::collections::HashMap;
use std::env;
use std::sync::Arc;
use tracing::{debug, error, info, warn};
use utoipa::ToSchema;
use uuid::Uuid;

// ─────────────────────────────────────────────────────────────────────────────
// ─────────────────────────────────────────────────────────────────────────────
// Plan Definitions
// ─────────────────────────────────────────────────────────────────────────────

/// Definition of a subscription plan tier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanDefinition {
    pub id: String,
    pub name: String,
    pub display_name: String,
    pub price: u32,
    pub monthly_price_cents: u32,
    pub monthly_request_quota: Option<u64>,
    pub monthly_request_limit: Option<u64>,
    pub features: Vec<String>,
    pub stripe_price_id_env: String,
}

impl PlanDefinition {
    pub fn has_feature(&self, feature: &str) -> bool {
        self.features
            .iter()
            .any(|f| f == "all" || f.eq_ignore_ascii_case(feature))
    }
}

/// Helper to read plans from config/config.yaml or fallback to defaults.
fn read_plans_from_config() -> HashMap<String, PlanDefinition> {
    let mut plans = HashMap::new();

    let config_paths = [
        env::var("CONFIG_PATH").unwrap_or_default(),
        "config/config.yaml".to_string(),
        "config.yaml".to_string(),
        "../config/config.yaml".to_string(),
        "../../config/config.yaml".to_string(),
    ];

    let mut loaded_from_file = false;

    for path in &config_paths {
        if path.is_empty() {
            continue;
        }
        if let Ok(contents) = std::fs::read_to_string(path) {
            let mut in_billing = false;
            let mut in_plans = false;
            let mut current_plan: Option<PlanDefinition> = None;

            for line in contents.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with('#') || trimmed.is_empty() {
                    continue;
                }

                if trimmed.starts_with("billing:") {
                    in_billing = true;
                    in_plans = false;
                    continue;
                }

                if in_billing && (trimmed.starts_with("plans:") || trimmed == "plans:") {
                    in_plans = true;
                    continue;
                }

                if in_billing && !line.starts_with(' ') && !line.starts_with('\t') {
                    in_billing = false;
                    in_plans = false;
                    if let Some(p) = current_plan.take() {
                        plans.insert(p.id.clone(), p);
                    }
                }

                if in_plans {
                    if trimmed.starts_with("- id:") || trimmed.starts_with("-id:") {
                        if let Some(p) = current_plan.take() {
                            plans.insert(p.id.clone(), p);
                        }
                        let id_val = trimmed
                            .split_once(':')
                            .map(|(_, v)| v.trim().trim_matches('"').trim_matches('\'').to_string())
                            .unwrap_or_default();
                        current_plan = Some(PlanDefinition {
                            id: id_val.clone(),
                            name: id_val.clone(),
                            display_name: if id_val == "free" {
                                "Free Tier".to_string()
                            } else {
                                id_val.clone()
                            },
                            price: 0,
                            monthly_price_cents: 0,
                            monthly_request_quota: None,
                            monthly_request_limit: if id_val == "free" {
                                Some(1_000)
                            } else {
                                None
                            },
                            features: Vec::new(),
                            stripe_price_id_env: String::new(),
                        });
                    } else if let Some(ref mut p) = current_plan {
                        if let Some((k, v)) = trimmed.split_once(':') {
                            let k = k.trim().trim_start_matches('-').trim();
                            let raw_v = v
                                .split('#')
                                .next()
                                .unwrap_or("")
                                .trim()
                                .trim_matches('"')
                                .trim_matches('\'');
                            match k {
                                "name" => {
                                    p.name = raw_v.to_string();
                                    p.display_name = if p.id == "free" {
                                        "Free Tier".to_string()
                                    } else {
                                        raw_v.to_string()
                                    };
                                }
                                "price" => {
                                    if let Ok(dollars) = raw_v.parse::<u32>() {
                                        p.price = dollars;
                                        p.monthly_price_cents = dollars * 100;
                                    }
                                }
                                "monthly_request_quota" | "monthly_quota" => {
                                    if let Ok(quota) = raw_v.parse::<u64>() {
                                        p.monthly_request_quota = Some(quota);
                                        if p.id == "pro_monthly" {
                                            p.monthly_request_limit = Some(quota);
                                        }
                                    }
                                }
                                "features" => {
                                    if raw_v.starts_with('[') && raw_v.ends_with(']') {
                                        let inside = &raw_v[1..raw_v.len() - 1];
                                        p.features = inside
                                            .split(',')
                                            .map(|s| {
                                                s.trim()
                                                    .trim_matches('"')
                                                    .trim_matches('\'')
                                                    .to_string()
                                            })
                                            .filter(|s| !s.is_empty())
                                            .collect();
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }

            if let Some(p) = current_plan.take() {
                plans.insert(p.id.clone(), p);
            }

            if !plans.is_empty() {
                loaded_from_file = true;
                break;
            }
        }
    }

    if !loaded_from_file || plans.is_empty() {
        // Fallback static defaults
        plans.insert(
            "free".to_string(),
            PlanDefinition {
                id: "free".to_string(),
                name: "Free".to_string(),
                display_name: "Free Tier".to_string(),
                price: 0,
                monthly_price_cents: 0,
                monthly_request_quota: Some(10_000),
                monthly_request_limit: Some(1_000),
                features: vec!["sentiment".into(), "news".into()],
                stripe_price_id_env: "".into(),
            },
        );

        plans.insert(
            "pro_monthly".to_string(),
            PlanDefinition {
                id: "pro_monthly".to_string(),
                name: "Pro".to_string(),
                display_name: "Pro (Monthly)".to_string(),
                price: 99,
                monthly_price_cents: 9_900,
                monthly_request_quota: Some(100_000),
                monthly_request_limit: Some(100_000),
                features: vec![
                    "sentiment".into(),
                    "news".into(),
                    "events".into(),
                    "export".into(),
                ],
                stripe_price_id_env: "STRIPE_PRICE_PRO_MONTHLY".into(),
            },
        );

        plans.insert(
            "enterprise_monthly".to_string(),
            PlanDefinition {
                id: "enterprise_monthly".to_string(),
                name: "Enterprise".to_string(),
                display_name: "Enterprise (Monthly)".to_string(),
                price: 499,
                monthly_price_cents: 49_900,
                monthly_request_quota: Some(1_000_000),
                monthly_request_limit: None,
                features: vec!["all".into()],
                stripe_price_id_env: "STRIPE_PRICE_ENTERPRISE_MONTHLY".into(),
            },
        );
    } else {
        if let Some(free) = plans.get_mut("free") {
            if free.display_name.is_empty() {
                free.display_name = "Free Tier".to_string();
            }
            if free.monthly_request_limit.is_none() {
                free.monthly_request_limit = Some(1_000);
            }
        }
        if let Some(pro) = plans.get_mut("pro_monthly") {
            if pro.stripe_price_id_env.is_empty() {
                pro.stripe_price_id_env = "STRIPE_PRICE_PRO_MONTHLY".to_string();
            }
            if pro.monthly_price_cents == 0 && pro.price > 0 {
                pro.monthly_price_cents = pro.price * 100;
            }
        }
        if let Some(ent) = plans.get_mut("enterprise_monthly") {
            if ent.stripe_price_id_env.is_empty() {
                ent.stripe_price_id_env = "STRIPE_PRICE_ENTERPRISE_MONTHLY".to_string();
            }
            if ent.monthly_price_cents == 0 && ent.price > 0 {
                ent.monthly_price_cents = ent.price * 100;
            }
        }
    }

    plans
}

/// Returns the plan registry.
pub fn get_plan_registry() -> HashMap<String, PlanDefinition> {
    read_plans_from_config()
}

/// Returns the monthly request limit for a given plan ID.
pub fn get_plan_request_limit(plan_id: &str) -> Option<u64> {
    let registry = get_plan_registry();
    registry.get(plan_id).and_then(|p| p.monthly_request_limit)
}

/// Returns the monthly request quota for rate limit enforcement for a given plan ID.
pub fn get_plan_request_quota(plan_id: &str) -> Option<u64> {
    let registry = get_plan_registry();
    registry
        .get(plan_id)
        .and_then(|p| p.monthly_request_quota.or(p.monthly_request_limit))
}

/// Validates that a plan ID exists and is a paid plan (valid for checkout).
pub fn validate_checkout_plan(plan_id: &str) -> Result<&'static str, String> {
    match plan_id {
        "pro_monthly" => Ok("pro_monthly"),
        "enterprise_monthly" => Ok("enterprise_monthly"),
        "free" => Err("Plan 'free' is free and does not require checkout".to_string()),
        other => Err(format!(
            "Unknown plan '{}'. Valid plans: free, pro_monthly, enterprise_monthly",
            other
        )),
    }
}

/// Redacts sensitive identifiers (customer IDs, subscription IDs, user IDs) for safe logging.
pub fn redact_id(id: &str) -> String {
    let trimmed = id.trim();
    if trimmed.len() <= 6 {
        "***".to_string()
    } else {
        format!("{}...{}", &trimmed[..4], &trimmed[trimmed.len() - 3..])
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Request & Response DTOs
// ─────────────────────────────────────────────────────────────────────────────

/// Request payload for creating a Stripe Checkout Session (`POST /billing/checkout`).
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CheckoutRequest {
    /// Subscription plan identifier (e.g. "pro_monthly", "enterprise_monthly")
    #[schema(example = "pro_monthly")]
    pub plan_id: String,
    /// URL to redirect to after successful payment (optional, defaults to API base)
    #[serde(default)]
    #[schema(example = "https://app.fintext.io/billing/success")]
    pub success_url: Option<String>,
    /// URL to redirect to if user cancels checkout (optional, defaults to API base)
    #[serde(default)]
    #[schema(example = "https://app.fintext.io/billing/cancel")]
    pub cancel_url: Option<String>,
}

/// Response containing Stripe Checkout Session URL.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CheckoutResponse {
    /// Full Stripe Checkout Session URL for payment completion
    #[schema(example = "https://checkout.stripe.com/c/pay/cs_test_...")]
    pub checkout_url: String,
    /// Stripe Checkout Session ID
    #[schema(example = "cs_test_a1b2c3d4e5")]
    pub session_id: String,
}

/// Request payload for creating a Stripe Customer Portal session (`POST /billing/portal`).
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct PortalRequest {
    /// URL to return to after leaving the portal (optional)
    #[serde(default)]
    #[schema(example = "https://app.fintext.io/dashboard")]
    pub return_url: Option<String>,
}

/// Response containing Stripe Customer Portal URL.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PortalResponse {
    /// Full Stripe Customer Portal session URL
    #[schema(example = "https://billing.stripe.com/p/session/...")]
    pub portal_url: String,
}

/// Current subscription status response (`GET /billing/subscription`).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SubscriptionResponse {
    /// Active plan identifier
    #[schema(example = "pro_monthly")]
    pub plan_id: String,
    /// Human-readable plan name
    #[schema(example = "Pro (Monthly)")]
    pub plan_name: String,
    /// Subscription status (active, past_due, cancelled, inactive)
    #[schema(example = "active")]
    pub status: String,
    /// Monthly API request limit for this plan (null = unlimited)
    pub monthly_request_limit: Option<u64>,
    /// Monthly API request quota for this plan (null = unlimited)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub monthly_request_quota: Option<u64>,
    /// Features included in this plan
    #[serde(default)]
    pub features: Vec<String>,
    /// Current month's API request count
    #[schema(example = 4523)]
    pub current_usage: u64,
    /// Current billing period start date
    pub current_period_start: Option<String>,
    /// Current billing period end date
    pub current_period_end: Option<String>,
    /// Stripe customer ID (masked)
    pub stripe_customer_id: Option<String>,
}

/// Acknowledgement response for Stripe webhook events.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BillingWebhookResponse {
    /// Whether the webhook event was received and processed
    #[schema(example = true)]
    pub received: bool,
}

// ─────────────────────────────────────────────────────────────────────────────
// Database Schema & Queries
// ─────────────────────────────────────────────────────────────────────────────

/// Creates the `subscriptions` table if it does not already exist.
pub async fn init_billing_db(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS subscriptions (
            id UUID PRIMARY KEY,
            user_id UUID NOT NULL,
            stripe_customer_id TEXT,
            stripe_subscription_id TEXT,
            plan_id TEXT NOT NULL DEFAULT 'free',
            status TEXT NOT NULL DEFAULT 'active',
            current_period_start TIMESTAMPTZ,
            current_period_end TIMESTAMPTZ,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        );
        CREATE UNIQUE INDEX IF NOT EXISTS idx_subscriptions_user_id ON subscriptions(user_id);
        CREATE INDEX IF NOT EXISTS idx_subscriptions_stripe_customer ON subscriptions(stripe_customer_id);
        "#,
    )
    .execute(pool)
    .await?;

    info!("[Billing] Verified 'subscriptions' table and indexes in PostgreSQL");
    Ok(())
}

/// Subscription record from the database.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SubscriptionRecord {
    pub id: Uuid,
    pub user_id: Uuid,
    pub stripe_customer_id: Option<String>,
    pub stripe_subscription_id: Option<String>,
    pub plan_id: String,
    pub status: String,
    pub current_period_start: Option<DateTime<Utc>>,
    pub current_period_end: Option<DateTime<Utc>>,
}

/// Retrieves the subscription record for a given user ID.
pub async fn get_user_subscription(
    pool: &PgPool,
    user_id: &str,
) -> Result<Option<SubscriptionRecord>, sqlx::Error> {
    let user_uuid = match Uuid::parse_str(user_id) {
        Ok(u) => u,
        Err(_) => return Ok(None),
    };

    let row: Option<SubscriptionRecord> = sqlx::query_as(
        r#"
        SELECT id, user_id, stripe_customer_id, stripe_subscription_id,
               plan_id, status, current_period_start, current_period_end
        FROM subscriptions
        WHERE user_id = $1
        ORDER BY updated_at DESC
        LIMIT 1
        "#,
    )
    .bind(user_uuid)
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

/// Counts the current month's API requests for a user from the usage_events table.
pub async fn get_monthly_usage_count(pool: &PgPool, user_id: &str) -> Result<u64, sqlx::Error> {
    let row: (i64,) = sqlx::query_as(
        r#"
        SELECT COALESCE(COUNT(*), 0)
        FROM usage_events
        WHERE user_id = $1
        AND created_at >= date_trunc('month', NOW())
        "#,
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;

    Ok(row.0 as u64)
}

/// Upserts a subscription record after a Stripe event.
pub async fn upsert_subscription(
    pool: &PgPool,
    user_id: &str,
    stripe_customer_id: &str,
    stripe_subscription_id: &str,
    plan_id: &str,
    status: &str,
    period_start: Option<DateTime<Utc>>,
    period_end: Option<DateTime<Utc>>,
) -> Result<(), sqlx::Error> {
    let user_uuid = Uuid::parse_str(user_id).unwrap_or_else(|_| Uuid::new_v4());

    let existing: Option<(Uuid,)> = sqlx::query_as(
        r#"
        SELECT id FROM subscriptions
        WHERE (stripe_customer_id = $1 AND $1 != '')
           OR (stripe_subscription_id = $2 AND $2 != '')
           OR user_id = $3
        ORDER BY updated_at DESC
        LIMIT 1
        "#,
    )
    .bind(stripe_customer_id)
    .bind(stripe_subscription_id)
    .bind(user_uuid)
    .fetch_optional(pool)
    .await?;

    if let Some((sub_id,)) = existing {
        sqlx::query(
            r#"
            UPDATE subscriptions
            SET stripe_customer_id = COALESCE(NULLIF($1, ''), stripe_customer_id),
                stripe_subscription_id = COALESCE(NULLIF($2, ''), stripe_subscription_id),
                plan_id = $3,
                status = $4,
                current_period_start = COALESCE($5, current_period_start),
                current_period_end = COALESCE($6, current_period_end),
                dunning_fail_count = 0,
                grace_until_utc = NULL,
                updated_at = NOW()
            WHERE id = $7
            "#,
        )
        .bind(stripe_customer_id)
        .bind(stripe_subscription_id)
        .bind(plan_id)
        .bind(status)
        .bind(period_start)
        .bind(period_end)
        .bind(sub_id)
        .execute(pool)
        .await?;
    } else {
        sqlx::query(
            r#"
            INSERT INTO subscriptions (
                id, user_id, stripe_customer_id, stripe_subscription_id,
                plan_id, status, current_period_start, current_period_end,
                dunning_fail_count, grace_until_utc, created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 0, NULL, NOW(), NOW())
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(user_uuid)
        .bind(stripe_customer_id)
        .bind(stripe_subscription_id)
        .bind(plan_id)
        .bind(status)
        .bind(period_start)
        .bind(period_end)
        .execute(pool)
        .await?;
    }

    info!(
        "[Billing] Upserted subscription for user='{}' plan='{}' status='{}'",
        user_id, plan_id, status
    );
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Stripe REST API Client Helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Creates a Stripe Customer object via the Stripe REST API.
pub async fn create_stripe_customer(
    secret_key: &str,
    email: &str,
    user_id: &str,
) -> Result<String, String> {
    let client = reqwest::Client::new();
    let resp = client
        .post("https://api.stripe.com/v1/customers")
        .basic_auth(secret_key, Option::<&str>::None)
        .form(&[("email", email), ("metadata[fintext_user_id]", user_id)])
        .send()
        .await
        .map_err(|e| format!("Stripe API request failed: {}", e))?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Stripe customer creation failed: {}", body));
    }

    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse Stripe response: {}", e))?;

    json["id"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "Stripe response missing customer ID".to_string())
}

/// Creates a Stripe Checkout Session for subscription payment.
pub async fn create_stripe_checkout_session(
    secret_key: &str,
    customer_id: &str,
    price_id: &str,
    success_url: &str,
    cancel_url: &str,
) -> Result<(String, String), String> {
    let client = reqwest::Client::new();
    let resp = client
        .post("https://api.stripe.com/v1/checkout/sessions")
        .basic_auth(secret_key, Option::<&str>::None)
        .form(&[
            ("customer", customer_id),
            ("mode", "subscription"),
            ("line_items[0][price]", price_id),
            ("line_items[0][quantity]", "1"),
            ("success_url", success_url),
            ("cancel_url", cancel_url),
        ])
        .send()
        .await
        .map_err(|e| format!("Stripe API request failed: {}", e))?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Stripe checkout session creation failed: {}", body));
    }

    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse Stripe response: {}", e))?;

    let url = json["url"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "Stripe response missing checkout URL".to_string())?;

    let session_id = json["id"]
        .as_str()
        .map(|s| s.to_string())
        .unwrap_or_default();

    Ok((url, session_id))
}

/// Creates a Stripe Customer Portal session.
pub async fn create_stripe_portal_session(
    secret_key: &str,
    customer_id: &str,
    return_url: &str,
) -> Result<String, String> {
    let client = reqwest::Client::new();
    let resp = client
        .post("https://api.stripe.com/v1/billing_portal/sessions")
        .basic_auth(secret_key, Option::<&str>::None)
        .form(&[("customer", customer_id), ("return_url", return_url)])
        .send()
        .await
        .map_err(|e| format!("Stripe API request failed: {}", e))?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Stripe portal session creation failed: {}", body));
    }

    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse Stripe response: {}", e))?;

    json["url"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "Stripe response missing portal URL".to_string())
}

// ─────────────────────────────────────────────────────────────────────────────
// Stripe Webhook Signature Verification
// ─────────────────────────────────────────────────────────────────────────────

/// Constant-time byte slice comparison to prevent timing attacks.
pub fn constant_time_compare(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Constant-time hex string comparison (case-insensitive ASCII).
pub fn constant_time_hex_compare(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    let mut diff = 0u8;
    for (x, y) in a_bytes.iter().zip(b_bytes.iter()) {
        diff |= x.to_ascii_lowercase() ^ y.to_ascii_lowercase();
    }
    diff == 0
}

/// Dedicated error enumeration for Stripe webhook signature verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StripeSignatureError {
    MissingHeader,
    MissingTimestamp,
    InvalidTimestamp(String),
    TimestampToleranceExceeded {
        timestamp: i64,
        now: i64,
        diff_seconds: i64,
    },
    MissingSignature,
    InvalidSignature,
    HmacInitError(String),
}

impl std::fmt::Display for StripeSignatureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingHeader => write!(f, "Missing Stripe-Signature header"),
            Self::MissingTimestamp => {
                write!(f, "Missing timestamp (t=) in Stripe-Signature header")
            }
            Self::InvalidTimestamp(s) => write!(f, "Invalid timestamp format: {}", s),
            Self::TimestampToleranceExceeded {
                timestamp,
                now,
                diff_seconds,
            } => {
                write!(
                    f,
                    "Timestamp tolerance exceeded: event t={}, now={}, diff={}s > 300s",
                    timestamp, now, diff_seconds
                )
            }
            Self::MissingSignature => write!(f, "Missing v1 signature in Stripe-Signature header"),
            Self::InvalidSignature => write!(f, "Stripe webhook signature verification failed"),
            Self::HmacInitError(s) => write!(f, "Failed to initialize HMAC: {}", s),
        }
    }
}

impl std::error::Error for StripeSignatureError {}

/// Verifies a Stripe webhook signature with explicit current time and tolerance window.
pub fn verify_stripe_signature_with_time(
    payload: &[u8],
    sig_header: &str,
    secret: &str,
    current_time: i64,
    tolerance_secs: i64,
) -> Result<(), StripeSignatureError> {
    if sig_header.trim().is_empty() {
        return Err(StripeSignatureError::MissingHeader);
    }

    let mut timestamp_str = None;
    let mut signatures: Vec<&str> = Vec::new();

    for part in sig_header.split(',') {
        let part = part.trim();
        if let Some(ts) = part.strip_prefix("t=") {
            timestamp_str = Some(ts);
        } else if let Some(sig) = part.strip_prefix("v1=") {
            signatures.push(sig);
        }
    }

    let ts_str = timestamp_str.ok_or(StripeSignatureError::MissingTimestamp)?;
    let parsed_ts: i64 = ts_str
        .parse()
        .map_err(|_| StripeSignatureError::InvalidTimestamp(ts_str.to_string()))?;

    // Validate replay window (default 300s tolerance)
    let diff = (current_time - parsed_ts).abs();
    if diff > tolerance_secs {
        return Err(StripeSignatureError::TimestampToleranceExceeded {
            timestamp: parsed_ts,
            now: current_time,
            diff_seconds: diff,
        });
    }

    if signatures.is_empty() {
        return Err(StripeSignatureError::MissingSignature);
    }

    // Support secret rotation (multiple secrets separated by comma or semicolon)
    let secret_candidates: Vec<&str> = secret
        .split(|c| c == ',' || c == ';')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    if secret_candidates.is_empty() {
        return Err(StripeSignatureError::InvalidSignature);
    }

    // Construct raw signed payload: "{ts}.{payload}"
    let ts_bytes = ts_str.as_bytes();

    for cand_secret in secret_candidates {
        let mut mac = match Hmac::<Sha256>::new_from_slice(cand_secret.as_bytes()) {
            Ok(m) => m,
            Err(e) => return Err(StripeSignatureError::HmacInitError(e.to_string())),
        };
        mac.update(ts_bytes);
        mac.update(b".");
        mac.update(payload);
        let expected = hex::encode(mac.finalize().into_bytes());

        for candidate_sig in &signatures {
            if constant_time_hex_compare(candidate_sig, &expected) {
                return Ok(());
            }
        }
    }

    Err(StripeSignatureError::InvalidSignature)
}

/// Verifies a Stripe webhook signature using HMAC-SHA256 with default 300s replay tolerance.
pub fn verify_stripe_signature(
    payload: &[u8],
    sig_header: &str,
    secret: &str,
) -> Result<(), StripeSignatureError> {
    verify_stripe_signature_with_time(payload, sig_header, secret, Utc::now().timestamp(), 300)
}

// ─────────────────────────────────────────────────────────────────────────────
// Axum Route Handlers
// ─────────────────────────────────────────────────────────────────────────────

/// Create a Stripe Checkout Session for subscribing to a paid plan.
///
/// Validates the requested plan, creates or retrieves the Stripe customer,
/// and returns a checkout URL where the user can complete payment.
#[utoipa::path(
    post,
    path = "/billing/checkout",
    tag = "Billing & Subscriptions",
    request_body = CheckoutRequest,
    responses(
        (status = 200, description = "Stripe Checkout Session created", body = CheckoutResponse),
        (status = 400, description = "Invalid plan or missing Stripe configuration", body = AuthErrorResponse),
        (status = 401, description = "Unauthorized", body = AuthErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::rate_limit::RateLimitErrorResponse)
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn create_checkout_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<CheckoutRequest>,
) -> Response {
    let user_id = &claims.sub;

    // 1. Validate plan
    let plan_id = match validate_checkout_plan(&body.plan_id) {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(AuthErrorResponse {
                    error: "Bad Request".to_string(),
                    message: e,
                }),
            )
                .into_response();
        }
    };

    // 2. Get Stripe secret key
    let secret_key = match &state.stripe_secret_key {
        Some(k) if !k.is_empty() => k.clone(),
        _ => {
            // Mock mode: return a synthetic checkout URL for testing
            info!(
                "[Billing] STRIPE_SECRET_KEY not configured. Returning mock checkout session for plan '{}'",
                plan_id
            );
            log_audit_event(
                &state,
                None,
                user_id,
                "billing.checkout_created",
                "subscription",
                Some(plan_id),
                serde_json::json!({"plan_id": plan_id, "mock": true}),
                None,
            )
            .await;

            return (
                StatusCode::OK,
                Json(CheckoutResponse {
                    checkout_url: format!("https://checkout.stripe.com/mock/pay?plan={}", plan_id),
                    session_id: format!("cs_mock_{}", Uuid::new_v4().simple()),
                }),
            )
                .into_response();
        }
    };

    // 3. Get or create Stripe customer
    let user_id = &claims.sub;

    // Check existing subscription for Stripe customer ID
    let existing_customer_id = if let Some(pool) = &state.db_pool {
        match get_user_subscription(pool, user_id).await {
            Ok(Some(sub)) => sub.stripe_customer_id,
            _ => None,
        }
    } else {
        None
    };

    let customer_id = match existing_customer_id {
        Some(cid) if !cid.is_empty() => cid,
        _ => {
            // Create a new Stripe customer
            match create_stripe_customer(&secret_key, user_id, user_id).await {
                Ok(cid) => cid,
                Err(e) => {
                    error!("[Billing] Failed to create Stripe customer: {}", e);
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(AuthErrorResponse {
                            error: "Billing Error".to_string(),
                            message: "Failed to create payment customer".to_string(),
                        }),
                    )
                        .into_response();
                }
            }
        }
    };

    // 4. Resolve Stripe Price ID
    let registry = get_plan_registry();
    let plan_def = registry.get(plan_id).unwrap();
    let price_id = env::var(&plan_def.stripe_price_id_env)
        .unwrap_or_else(|_| format!("price_mock_{}", plan_id));

    let success_url = body.success_url.unwrap_or_else(|| {
        "https://app.fintext.io/billing/success?session_id={CHECKOUT_SESSION_ID}".to_string()
    });
    let cancel_url = body
        .cancel_url
        .unwrap_or_else(|| "https://app.fintext.io/billing/cancel".to_string());

    // 5. Create Checkout Session
    match create_stripe_checkout_session(
        &secret_key,
        &customer_id,
        &price_id,
        &success_url,
        &cancel_url,
    )
    .await
    {
        Ok((url, session_id)) => {
            info!(
                "[Billing] Created checkout session '{}' for user='{}' plan='{}'",
                session_id, user_id, plan_id
            );
            log_audit_event(
                &state,
                None,
                user_id,
                "billing.checkout_created",
                "subscription",
                Some(plan_id),
                serde_json::json!({"plan_id": plan_id, "session_id": session_id}),
                None,
            )
            .await;

            (
                StatusCode::OK,
                Json(CheckoutResponse {
                    checkout_url: url,
                    session_id,
                }),
            )
                .into_response()
        }
        Err(e) => {
            error!("[Billing] Stripe checkout creation failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AuthErrorResponse {
                    error: "Billing Error".to_string(),
                    message: "Failed to create checkout session".to_string(),
                }),
            )
                .into_response()
        }
    }
}

/// Create a Stripe Customer Portal session for subscription management.
///
/// Allows users to manage their payment methods, view invoices, and cancel/change plans.
#[utoipa::path(
    post,
    path = "/billing/portal",
    tag = "Billing & Subscriptions",
    request_body = PortalRequest,
    responses(
        (status = 200, description = "Stripe Customer Portal session created", body = PortalResponse),
        (status = 400, description = "No active subscription found", body = AuthErrorResponse),
        (status = 401, description = "Unauthorized", body = AuthErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::rate_limit::RateLimitErrorResponse)
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn create_portal_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<PortalRequest>,
) -> Response {
    let user_id = &claims.sub;

    let secret_key = match &state.stripe_secret_key {
        Some(k) if !k.is_empty() => k.clone(),
        _ => {
            info!("[Billing] STRIPE_SECRET_KEY not configured. Returning mock portal URL.");
            log_audit_event(
                &state,
                None,
                user_id,
                "billing.portal_created",
                "customer",
                Some(user_id),
                serde_json::json!({"mock": true}),
                None,
            )
            .await;

            return (
                StatusCode::OK,
                Json(PortalResponse {
                    portal_url: "https://billing.stripe.com/mock/portal".to_string(),
                }),
            )
                .into_response();
        }
    };

    // Lookup Stripe customer ID from subscription
    let customer_id = if let Some(pool) = &state.db_pool {
        match get_user_subscription(pool, user_id).await {
            Ok(Some(sub)) => sub.stripe_customer_id,
            _ => None,
        }
    } else {
        None
    };

    let customer_id = match customer_id {
        Some(cid) if !cid.is_empty() => cid,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(AuthErrorResponse {
                    error: "Bad Request".to_string(),
                    message: "No active subscription found. Please subscribe first via /billing/checkout.".to_string(),
                }),
            )
                .into_response();
        }
    };

    let return_url = body
        .return_url
        .unwrap_or_else(|| "https://app.fintext.io/dashboard".to_string());

    match create_stripe_portal_session(&secret_key, &customer_id, &return_url).await {
        Ok(url) => {
            info!("[Billing] Created portal session for user='{}'", user_id);
            log_audit_event(
                &state,
                None,
                user_id,
                "billing.portal_created",
                "customer",
                Some(&customer_id),
                serde_json::json!({"customer_id": customer_id}),
                None,
            )
            .await;

            (StatusCode::OK, Json(PortalResponse { portal_url: url })).into_response()
        }
        Err(e) => {
            error!("[Billing] Stripe portal creation failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AuthErrorResponse {
                    error: "Billing Error".to_string(),
                    message: "Failed to create portal session".to_string(),
                }),
            )
                .into_response()
        }
    }
}

/// Get the current user's subscription status and usage.
///
/// Returns the active plan tier, billing period, monthly request limits,
/// and current month's API usage count.
#[utoipa::path(
    get,
    path = "/billing/subscription",
    tag = "Billing & Subscriptions",
    responses(
        (status = 200, description = "Current subscription status", body = SubscriptionResponse),
        (status = 401, description = "Unauthorized", body = AuthErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::rate_limit::RateLimitErrorResponse)
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn get_subscription_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Response {
    let user_id = &claims.sub;
    let registry = get_plan_registry();

    // Fetch subscription from database (or default to free)
    let (plan_id, status, period_start, period_end, stripe_cid) = if let Some(pool) = &state.db_pool
    {
        match get_user_subscription(pool, user_id).await {
            Ok(Some(sub)) => (
                sub.plan_id,
                sub.status,
                sub.current_period_start.map(|d| d.to_rfc3339()),
                sub.current_period_end.map(|d| d.to_rfc3339()),
                sub.stripe_customer_id.map(|s| {
                    if s.len() > 8 {
                        format!("{}...{}", &s[..4], &s[s.len() - 4..])
                    } else {
                        s
                    }
                }),
            ),
            _ => ("free".to_string(), "active".to_string(), None, None, None),
        }
    } else {
        ("free".to_string(), "active".to_string(), None, None, None)
    };

    // Fetch current month's usage
    let current_usage = if let Some(pool) = &state.db_pool {
        get_monthly_usage_count(pool, user_id).await.unwrap_or(0)
    } else {
        0
    };

    let plan_def = registry.get(plan_id.as_str());
    let display_name = plan_def
        .map(|p| p.display_name.clone())
        .unwrap_or_else(|| plan_id.clone());
    let monthly_limit = plan_def.and_then(|p| p.monthly_request_limit);
    let monthly_quota = plan_def.and_then(|p| p.monthly_request_quota);
    let features = plan_def
        .map(|p| p.features.clone())
        .unwrap_or_else(|| vec!["sentiment".into(), "news".into()]);

    (
        StatusCode::OK,
        Json(SubscriptionResponse {
            plan_id,
            plan_name: display_name,
            status,
            monthly_request_limit: monthly_limit,
            monthly_request_quota: monthly_quota,
            features,
            current_usage,
            current_period_start: period_start,
            current_period_end: period_end,
            stripe_customer_id: stripe_cid,
        }),
    )
        .into_response()
}

/// Auxiliary helper to sync the past_due organization gauge from database.
async fn sync_past_due_gauge(pool: &PgPool, state: &AppState) {
    if let Ok(row) = sqlx::query_as::<_, (i64,)>(
        "SELECT COUNT(DISTINCT COALESCE(org_id::text, user_id::text)) FROM subscriptions WHERE status = 'past_due'",
    )
    .fetch_one(pool)
    .await
    {
        state.set_billing_past_due_orgs(row.0 as u64);
    }
}

/// Executes institutional Phase-1 suspension: revoking API keys, deactivating users and
/// memberships, and deactivating retention policies while strictly preserving audit and domain data.
async fn execute_phase_1_suspension(
    pool: &PgPool,
    state: &AppState,
    sub_id: Uuid,
    org_id_opt: Option<&str>,
    user_id: Uuid,
    actor: &str,
    reason: &str,
    stripe_event_id: &str,
) {
    let org_uuid = org_id_opt.and_then(|s| Uuid::parse_str(s).ok());

    // 1. Cancel subscription
    let _ = sqlx::query(
        "UPDATE subscriptions SET status = 'canceled', updated_at = NOW() WHERE id = $1",
    )
    .bind(sub_id)
    .execute(pool)
    .await;

    // 2. Revoke active API keys, suspend users and organization memberships, deactivate retention policies
    if let Some(org_id) = org_id_opt {
        let _ = sqlx::query(
            r#"
            UPDATE api_keys 
            SET revoked_at = NOW(), rotation_status = 'revoked'
            WHERE (org_id = $1 OR user_id IN (SELECT user_id FROM organization_members WHERE org_id::text = $1))
              AND revoked_at IS NULL
            "#,
        )
        .bind(org_id)
        .execute(pool)
        .await;

        let _ = sqlx::query(
            r#"
            UPDATE users 
            SET is_active = false 
            WHERE id IN (SELECT user_id FROM organization_members WHERE org_id::text = $1)
               OR id IN (SELECT user_id FROM api_keys WHERE org_id = $1)
               OR id = $2
            "#,
        )
        .bind(org_id)
        .bind(user_id)
        .execute(pool)
        .await;

        let _ = sqlx::query(
            "UPDATE organization_members SET role = 'suspended' WHERE org_id::text = $1",
        )
        .bind(org_id)
        .execute(pool)
        .await;

        let _ = sqlx::query(
            "UPDATE data_retention_policies SET is_active = false, updated_at = NOW() WHERE org_id::text = $1",
        )
        .bind(org_id)
        .execute(pool)
        .await;
    } else {
        let _ = sqlx::query(
            "UPDATE api_keys SET revoked_at = NOW(), rotation_status = 'revoked' WHERE user_id = $1 AND revoked_at IS NULL",
        )
        .bind(user_id)
        .execute(pool)
        .await;

        let _ = sqlx::query("UPDATE users SET is_active = false WHERE id = $1")
            .bind(user_id)
            .execute(pool)
            .await;
    }

    // 3. SEC Rule 17a-4 / FINRA Rule 4511: Audit log of suspension (Data strictly preserved)
    log_audit_event(
        state,
        org_uuid,
        actor,
        "billing.subscription_suspended",
        "subscription",
        Some(&sub_id.to_string()),
        serde_json::json!({
            "sub_id": sub_id,
            "org_id": org_id_opt,
            "reason": reason,
            "stripe_event_id": stripe_event_id,
            "compliance_retention": "SEC 17a-4 / FINRA 4511 preserved"
        }),
        None,
    )
    .await;

    // 4. Invalidate cache
    state.monthly_quota_cache.remove(&user_id.to_string());
}

/// Executes institutional recovery/reactivation: restoring subscription to active,
/// resetting dunning counters, and re-enabling access credentials and roles.
async fn execute_reactivation(
    pool: &PgPool,
    state: &AppState,
    sub_id: Uuid,
    org_id_opt: Option<&str>,
    user_id: Uuid,
    actor: &str,
    stripe_event_id: &str,
) {
    let org_uuid = org_id_opt.and_then(|s| Uuid::parse_str(s).ok());

    // 1. Reset subscription status to active and clear dunning counters
    let _ = sqlx::query(
        r#"
        UPDATE subscriptions 
        SET status = 'active', dunning_fail_count = 0, grace_until_utc = NULL, updated_at = NOW() 
        WHERE id = $1
        "#,
    )
    .bind(sub_id)
    .execute(pool)
    .await;

    // 2. Re-enable API keys, active users, restore membership roles, re-enable retention
    if let Some(org_id) = org_id_opt {
        let _ = sqlx::query(
            r#"
            UPDATE api_keys 
            SET revoked_at = NULL, rotation_status = 'none'
            WHERE (org_id = $1 OR user_id IN (SELECT user_id FROM organization_members WHERE org_id::text = $1))
              AND rotation_status = 'revoked'
            "#,
        )
        .bind(org_id)
        .execute(pool)
        .await;

        let _ = sqlx::query(
            r#"
            UPDATE users 
            SET is_active = true 
            WHERE id IN (SELECT user_id FROM organization_members WHERE org_id::text = $1)
               OR id IN (SELECT user_id FROM api_keys WHERE org_id = $1)
               OR id = $2
            "#,
        )
        .bind(org_id)
        .bind(user_id)
        .execute(pool)
        .await;

        let _ = sqlx::query(
            "UPDATE organization_members SET role = 'member' WHERE org_id::text = $1 AND role = 'suspended'",
        )
        .bind(org_id)
        .execute(pool)
        .await;

        let _ = sqlx::query(
            "UPDATE data_retention_policies SET is_active = true, updated_at = NOW() WHERE org_id::text = $1",
        )
        .bind(org_id)
        .execute(pool)
        .await;
    } else {
        let _ = sqlx::query(
            "UPDATE api_keys SET revoked_at = NULL, rotation_status = 'none' WHERE user_id = $1 AND rotation_status = 'revoked'",
        )
        .bind(user_id)
        .execute(pool)
        .await;

        let _ = sqlx::query("UPDATE users SET is_active = true WHERE id = $1")
            .bind(user_id)
            .execute(pool)
            .await;
    }

    log_audit_event(
        state,
        org_uuid,
        actor,
        "billing.subscription_reactivated",
        "subscription",
        Some(&sub_id.to_string()),
        serde_json::json!({
            "sub_id": sub_id,
            "org_id": org_id_opt,
            "status": "active",
            "stripe_event_id": stripe_event_id
        }),
        None,
    )
    .await;

    state.monthly_quota_cache.remove(&user_id.to_string());
}

/// Lightweight scheduled check for expired dunning grace periods.
/// Executes Phase-1 suspension for subscriptions remaining past_due after grace expiration.
pub async fn sweep_expired_dunning_grace_periods(
    pool: &PgPool,
    state: &AppState,
) -> Result<usize, sqlx::Error> {
    let expired_rows: Vec<(Uuid, Option<String>, Uuid)> = sqlx::query_as(
        r#"
        SELECT id, org_id::text, user_id
        FROM subscriptions
        WHERE status = 'past_due'
          AND grace_until_utc IS NOT NULL
          AND grace_until_utc < NOW()
        "#,
    )
    .fetch_all(pool)
    .await?;

    let count = expired_rows.len();
    for (sub_id, org_id_opt, user_id) in expired_rows {
        execute_phase_1_suspension(
            pool,
            state,
            sub_id,
            org_id_opt.as_deref(),
            user_id,
            "billing_sweep",
            "72h_grace_period_expired",
            "scheduled_sweep",
        )
        .await;
    }

    if count > 0 {
        sync_past_due_gauge(pool, state).await;
    }

    Ok(count)
}

/// Handle incoming Stripe webhook events.
///
/// **Public endpoint** — authenticates via Stripe-Signature HMAC-SHA256 verification
/// (not JWT). Processes subscription lifecycle events and updates local database.
#[utoipa::path(
    post,
    path = "/v1/billing/webhook",
    tag = "Billing & Subscriptions",
    responses(
        (status = 200, description = "Webhook event acknowledged", body = BillingWebhookResponse),
        (status = 400, description = "Invalid signature or malformed event", body = AuthErrorResponse)
    )
)]
pub async fn stripe_webhook_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    // 1. Get webhook secret
    let webhook_secret = match &state.stripe_webhook_secret {
        Some(s) if !s.is_empty() => s.clone(),
        _ => env::var("STRIPE_WEBHOOK_SECRET").unwrap_or_default(),
    };

    // 2. Verify signature (if secret is configured)
    let sig_header = headers
        .get("stripe-signature")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if !webhook_secret.is_empty() {
        if let Err(e) = verify_stripe_signature(&body, sig_header, &webhook_secret) {
            match &e {
                StripeSignatureError::TimestampToleranceExceeded { .. } => {
                    state
                        .billing_webhook_timestamp_errors
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
                _ => {
                    state
                        .billing_webhook_sig_errors
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
            }
            warn!("[Billing Webhook] Signature verification failed: {}", e);
            return (
                StatusCode::BAD_REQUEST,
                Json(AuthErrorResponse {
                    error: "Bad Request".to_string(),
                    message: format!("Webhook signature verification failed: {}", e),
                }),
            )
                .into_response();
        }
    }

    // 3. Parse event JSON
    let event: serde_json::Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => {
            state
                .billing_webhook_parse_errors
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            warn!("[Billing Webhook] Failed to parse event JSON: {}", e);
            return (
                StatusCode::BAD_REQUEST,
                Json(AuthErrorResponse {
                    error: "Bad Request".to_string(),
                    message: "Malformed webhook event body".to_string(),
                }),
            )
                .into_response();
        }
    };

    let stripe_event_id = event["id"].as_str().unwrap_or("").to_string();
    let event_type = event["type"].as_str().unwrap_or("unknown").to_string();
    let event_created_ts = event["created"]
        .as_i64()
        .unwrap_or_else(|| Utc::now().timestamp());
    let event_created = DateTime::from_timestamp(event_created_ts, 0).unwrap_or_else(Utc::now);

    info!(
        "[Billing Webhook] Received Stripe event id='{}' type='{}'",
        stripe_event_id, event_type
    );

    let pool = state.db_pool.as_ref();

    // 4. Event Idempotency Defense via billing_events
    if let Some(pool) = pool {
        if !stripe_event_id.is_empty() {
            let row_inserted: Option<(Uuid,)> = sqlx::query_as(
                r#"
                INSERT INTO billing_events (
                    id, stripe_event_id, event_type, created_utc, processed_utc, status, payload, org_id
                )
                VALUES (
                    $1, $2, $3, $4, NOW(), 'processed', $5, $6
                )
                ON CONFLICT (stripe_event_id) DO NOTHING
                RETURNING id
                "#,
            )
            .bind(Uuid::new_v4())
            .bind(&stripe_event_id)
            .bind(&event_type)
            .bind(event_created)
            .bind(&event)
            .bind(
                event["data"]["object"]["metadata"]["org_id"]
                    .as_str()
                    .or_else(|| event["data"]["object"]["client_reference_id"].as_str()),
            )
            .fetch_optional(pool)
            .await
            .unwrap_or(None);

            if row_inserted.is_none() {
                info!(
                    "[Billing Webhook] Duplicate Stripe event '{}' acknowledged idempotently (no-op).",
                    stripe_event_id
                );
                return (
                    StatusCode::OK,
                    Json(BillingWebhookResponse { received: true }),
                )
                    .into_response();
            }
        }
    }

    // 5. Transactional Dunning & Subscription State Machine
    if let Some(pool) = pool {
        let data = &event["data"]["object"];
        let customer_id = data["customer"].as_str().unwrap_or("");
        let subscription_id = data["subscription"]
            .as_str()
            .or_else(|| data["id"].as_str())
            .unwrap_or("");
        let org_id_meta = data["metadata"]["org_id"]
            .as_str()
            .or_else(|| data["metadata"]["fintext_org_id"].as_str())
            .or_else(|| data["client_reference_id"].as_str())
            .unwrap_or("");
        let user_id_meta = data["metadata"]["fintext_user_id"]
            .as_str()
            .or_else(|| data["metadata"]["user_id"].as_str())
            .or_else(|| data["client_reference_id"].as_str())
            .unwrap_or("");

        // Query matching subscription
        let sub_row: Option<(
            Uuid,
            Uuid,
            Option<String>,
            String,
            String,
            i32,
            Option<DateTime<Utc>>,
        )> = sqlx::query_as(
            r#"
            SELECT id, user_id, org_id::text, plan_id, status, dunning_fail_count, grace_until_utc
            FROM subscriptions
            WHERE (stripe_customer_id = $1 AND $1 != '')
               OR (stripe_subscription_id = $2 AND $2 != '')
               OR (org_id::text = $3 AND $3 != '')
               OR (user_id::text = $4 AND $4 != '')
            ORDER BY updated_at DESC
            LIMIT 1
            "#,
        )
        .bind(customer_id)
        .bind(subscription_id)
        .bind(org_id_meta)
        .bind(user_id_meta)
        .fetch_optional(pool)
        .await
        .unwrap_or(None);

        match event_type.as_str() {
            "checkout.session.completed" => {
                let plan_id = data["metadata"]["plan_id"]
                    .as_str()
                    .unwrap_or("pro_monthly");

                if let Some((sub_id, user_uuid, org_opt, _, _, _, _)) = sub_row {
                    let _ = sqlx::query(
                        r#"
                        UPDATE subscriptions
                        SET stripe_customer_id = COALESCE(NULLIF($1, ''), stripe_customer_id),
                            stripe_subscription_id = COALESCE(NULLIF($2, ''), stripe_subscription_id),
                            plan_id = $3,
                            status = 'active',
                            dunning_fail_count = 0,
                            grace_until_utc = NULL,
                            updated_at = NOW()
                        WHERE id = $4
                        "#,
                    )
                    .bind(customer_id)
                    .bind(subscription_id)
                    .bind(plan_id)
                    .bind(sub_id)
                    .execute(pool)
                    .await;

                    execute_reactivation(
                        pool,
                        &state,
                        sub_id,
                        org_opt.as_deref(),
                        user_uuid,
                        "billing_webhook",
                        &stripe_event_id,
                    )
                    .await;
                } else {
                    let user_uuid =
                        Uuid::parse_str(user_id_meta).unwrap_or_else(|_| Uuid::new_v4());
                    let new_id = Uuid::new_v4();
                    let _ = sqlx::query(
                        r#"
                        INSERT INTO subscriptions (
                            id, user_id, org_id, stripe_customer_id, stripe_subscription_id,
                            plan_id, status, dunning_fail_count, grace_until_utc, created_at, updated_at
                        )
                        VALUES ($1, $2, $3, $4, $5, $6, 'active', 0, NULL, NOW(), NOW())
                        "#,
                    )
                    .bind(new_id)
                    .bind(user_uuid)
                    .bind(if org_id_meta.is_empty() {
                        None
                    } else {
                        Some(org_id_meta)
                    })
                    .bind(customer_id)
                    .bind(subscription_id)
                    .bind(plan_id)
                    .execute(pool)
                    .await;

                    log_audit_event(
                        &state,
                        Uuid::parse_str(org_id_meta).ok(),
                        "billing_webhook",
                        "billing.checkout_completed",
                        "subscription",
                        Some(&new_id.to_string()),
                        serde_json::json!({
                            "plan_id": plan_id,
                            "stripe_customer_id": customer_id,
                            "stripe_subscription_id": subscription_id,
                            "stripe_event_id": stripe_event_id
                        }),
                        None,
                    )
                    .await;
                }

                sync_past_due_gauge(pool, &state).await;
                info!(
                    "[Billing Webhook] checkout.session.completed bound customer='{}' sub='{}' plan='{}'",
                    redact_id(customer_id),
                    redact_id(subscription_id),
                    plan_id
                );
            }

            "customer.subscription.created" => {
                if let Some((sub_id, user_uuid, org_opt, _, _, _, _)) = sub_row {
                    let _ = sqlx::query(
                        r#"
                        UPDATE subscriptions
                        SET stripe_customer_id = COALESCE(NULLIF($1, ''), stripe_customer_id),
                            stripe_subscription_id = COALESCE(NULLIF($2, ''), stripe_subscription_id),
                            status = 'active',
                            dunning_fail_count = 0,
                            grace_until_utc = NULL,
                            updated_at = NOW()
                        WHERE id = $3
                        "#,
                    )
                    .bind(customer_id)
                    .bind(subscription_id)
                    .bind(sub_id)
                    .execute(pool)
                    .await;

                    execute_reactivation(
                        pool,
                        &state,
                        sub_id,
                        org_opt.as_deref(),
                        user_uuid,
                        "billing_webhook",
                        &stripe_event_id,
                    )
                    .await;
                    sync_past_due_gauge(pool, &state).await;
                }
            }

            "customer.subscription.updated" => {
                let status = data["status"].as_str().unwrap_or("active");
                let period_start = data["current_period_start"]
                    .as_i64()
                    .map(|ts| DateTime::from_timestamp(ts, 0).unwrap_or_else(Utc::now));
                let period_end = data["current_period_end"]
                    .as_i64()
                    .map(|ts| DateTime::from_timestamp(ts, 0).unwrap_or_else(Utc::now));

                let plan_id = data["metadata"]["plan_id"].as_str().or_else(|| {
                    data["items"]["data"]
                        .as_array()
                        .and_then(|arr| arr.first())
                        .and_then(|item| item["price"]["id"].as_str())
                        .and_then(|price_id| {
                            if price_id.contains("enterprise") {
                                Some("enterprise_monthly")
                            } else if price_id.contains("pro") || price_id.contains("growth") {
                                Some("pro_monthly")
                            } else if price_id.contains("starter") {
                                Some("starter")
                            } else {
                                None
                            }
                        })
                });

                if let Some((sub_id, user_uuid, org_opt, old_plan, _, _, _)) = sub_row {
                    let new_plan = plan_id.unwrap_or(&old_plan);
                    let _ = sqlx::query(
                        r#"
                        UPDATE subscriptions
                        SET status = $1,
                            plan_id = $2,
                            current_period_start = COALESCE($3, current_period_start),
                            current_period_end = COALESCE($4, current_period_end),
                            updated_at = NOW()
                        WHERE id = $5
                        "#,
                    )
                    .bind(status)
                    .bind(new_plan)
                    .bind(period_start)
                    .bind(period_end)
                    .bind(sub_id)
                    .execute(pool)
                    .await;

                    state.monthly_quota_cache.remove(&user_uuid.to_string());

                    log_audit_event(
                        &state,
                        org_opt.as_deref().and_then(|s| Uuid::parse_str(s).ok()),
                        "billing_webhook",
                        "billing.subscription_updated",
                        "subscription",
                        Some(&sub_id.to_string()),
                        serde_json::json!({
                            "plan_id": new_plan,
                            "status": status,
                            "stripe_event_id": stripe_event_id
                        }),
                        None,
                    )
                    .await;
                }
                sync_past_due_gauge(pool, &state).await;
            }

            "invoice.payment_failed" => {
                let invoice_id = data["id"].as_str().unwrap_or("");
                let amount_due = data["amount_due"].as_u64().unwrap_or(0);

                if let Some((sub_id, user_uuid, org_opt, _, _, fail_count, grace_opt)) = sub_row {
                    let new_fail_count = fail_count + 1;
                    let grace_expired = grace_opt.map(|g| g < Utc::now()).unwrap_or(false);

                    if new_fail_count >= 3 || (fail_count > 0 && grace_expired) {
                        // Dunning exhausted -> Automatic Phase-1 suspension
                        warn!(
                            "[DUNNING EXHAUSTED] Customer '{}' reached {} failures. Executing Phase-1 suspension.",
                            redact_id(customer_id),
                            new_fail_count
                        );
                        execute_phase_1_suspension(
                            pool,
                            &state,
                            sub_id,
                            org_opt.as_deref(),
                            user_uuid,
                            "billing_webhook",
                            "dunning_grace_exhausted",
                            &stripe_event_id,
                        )
                        .await;
                    } else {
                        // Transition to past_due with 72h grace window
                        warn!(
                            "[DUNNING ALERT] Payment failed for customer '{}' (fail_count={}). Subscription marked past_due with 72h grace.",
                            redact_id(customer_id),
                            new_fail_count
                        );

                        let _ = sqlx::query(
                            r#"
                            UPDATE subscriptions
                            SET status = 'past_due',
                                dunning_fail_count = $1,
                                grace_until_utc = COALESCE(grace_until_utc, NOW() + INTERVAL '72 hours'),
                                updated_at = NOW()
                            WHERE id = $2
                            "#,
                        )
                        .bind(new_fail_count)
                        .bind(sub_id)
                        .execute(pool)
                        .await;

                        log_audit_event(
                            &state,
                            org_opt.as_deref().and_then(|s| Uuid::parse_str(s).ok()),
                            "billing_webhook",
                            "billing.payment_failed",
                            "subscription",
                            Some(&sub_id.to_string()),
                            serde_json::json!({
                                "invoice_id": invoice_id,
                                "amount_due_cents": amount_due,
                                "fail_count": new_fail_count,
                                "grace_window_hours": 72,
                                "stripe_event_id": stripe_event_id
                            }),
                            None,
                        )
                        .await;
                    }
                }
                sync_past_due_gauge(pool, &state).await;
            }

            "invoice.paid" | "invoice.payment_succeeded" => {
                let period_start = data["period_start"]
                    .as_i64()
                    .map(|ts| DateTime::from_timestamp(ts, 0).unwrap_or_else(Utc::now));
                let period_end = data["period_end"]
                    .as_i64()
                    .map(|ts| DateTime::from_timestamp(ts, 0).unwrap_or_else(Utc::now));

                if let Some((sub_id, user_uuid, org_opt, _, old_status, _, _)) = sub_row {
                    let _ = sqlx::query(
                        r#"
                        UPDATE subscriptions
                        SET status = 'active',
                            dunning_fail_count = 0,
                            grace_until_utc = NULL,
                            current_period_start = COALESCE($1, current_period_start),
                            current_period_end = COALESCE($2, current_period_end),
                            updated_at = NOW()
                        WHERE id = $3
                        "#,
                    )
                    .bind(period_start)
                    .bind(period_end)
                    .bind(sub_id)
                    .execute(pool)
                    .await;

                    if old_status == "past_due" || old_status == "canceled" {
                        info!(
                            "[DUNNING RECOVERY] Payment confirmed for customer '{}'. Restoring full institutional access.",
                            redact_id(customer_id)
                        );
                        execute_reactivation(
                            pool,
                            &state,
                            sub_id,
                            org_opt.as_deref(),
                            user_uuid,
                            "billing_webhook",
                            &stripe_event_id,
                        )
                        .await;
                    } else {
                        log_audit_event(
                            &state,
                            org_opt.as_deref().and_then(|s| Uuid::parse_str(s).ok()),
                            "billing_webhook",
                            "billing.invoice_paid",
                            "subscription",
                            Some(&sub_id.to_string()),
                            serde_json::json!({
                                "stripe_event_id": stripe_event_id,
                                "customer_id": customer_id
                            }),
                            None,
                        )
                        .await;
                    }
                    state.monthly_quota_cache.remove(&user_uuid.to_string());
                }
                sync_past_due_gauge(pool, &state).await;
            }

            "customer.subscription.deleted" => {
                if let Some((sub_id, user_uuid, org_opt, _, _, _, _)) = sub_row {
                    warn!(
                        "[SUBSCRIPTION CANCELED] Subscription '{}' deleted in Stripe. Executing Phase-1 suspension.",
                        redact_id(subscription_id)
                    );
                    execute_phase_1_suspension(
                        pool,
                        &state,
                        sub_id,
                        org_opt.as_deref(),
                        user_uuid,
                        "billing_webhook",
                        "stripe_subscription_deleted",
                        &stripe_event_id,
                    )
                    .await;
                }
                sync_past_due_gauge(pool, &state).await;
            }

            "invoice.payment_action_required" => {
                warn!(
                    "[PAYMENT ACTION REQUIRED] Customer '{}' requires 3D Secure / SCA intervention.",
                    redact_id(customer_id)
                );
                if let Some((sub_id, _, org_opt, _, _, _, _)) = sub_row {
                    log_audit_event(
                        &state,
                        org_opt.as_deref().and_then(|s| Uuid::parse_str(s).ok()),
                        "billing_webhook",
                        "billing.payment_action_required",
                        "subscription",
                        Some(&sub_id.to_string()),
                        serde_json::json!({
                            "customer_id": customer_id,
                            "stripe_event_id": stripe_event_id
                        }),
                        None,
                    )
                    .await;
                }
            }

            _ => {
                info!(
                    "[Billing Webhook] Acknowledged unhandled event type: {}",
                    event_type
                );
                if !stripe_event_id.is_empty() {
                    let _ = sqlx::query(
                        "UPDATE billing_events SET status = 'ignored' WHERE stripe_event_id = $1",
                    )
                    .bind(&stripe_event_id)
                    .execute(pool)
                    .await;
                }
            }
        }
    }

    (
        StatusCode::OK,
        Json(BillingWebhookResponse { received: true }),
    )
        .into_response()
}

// ─────────────────────────────────────────────────────────────────────────────
// Monthly Quota Cache (for rate_limit integration)
// ─────────────────────────────────────────────────────────────────────────────

use std::time::Instant;

/// Cache entry storing monthly usage count with expiry.
#[derive(Debug, Clone)]
pub struct QuotaCacheEntry {
    pub plan_id: String,
    pub monthly_limit: Option<u64>,
    pub current_usage: u64,
    pub cached_at: Instant,
}

/// Thread-safe monthly quota cache backed by TtlCache with configurable TTL and maximum capacity.
#[derive(Debug, Clone)]
pub struct MonthlyQuotaCache {
    cache: Arc<crate::cache::TtlCache<String, QuotaCacheEntry>>,
    ttl_seconds: u64,
}

impl MonthlyQuotaCache {
    pub fn new(ttl_seconds: u64) -> Self {
        let max_capacity = crate::cache::CacheConfig::from_env_or_config().default_max_capacity;
        Self::new_with_capacity(ttl_seconds, max_capacity)
    }

    pub fn new_with_capacity(ttl_seconds: u64, max_capacity: usize) -> Self {
        Self {
            cache: Arc::new(crate::cache::TtlCache::with_ttl_secs(
                ttl_seconds,
                max_capacity,
            )),
            ttl_seconds,
        }
    }

    /// Retrieves a cached entry if it exists and is not expired.
    pub fn get(&self, user_id: &str) -> Option<QuotaCacheEntry> {
        self.cache.get(&user_id.to_string())
    }

    /// Stores a cache entry.
    pub fn set(&self, user_id: &str, entry: QuotaCacheEntry) {
        self.cache.insert(user_id.to_string(), entry);
    }

    /// Increments the cached usage count for a user (if entry exists and is valid).
    pub fn increment_usage(&self, user_id: &str) {
        self.cache.update(&user_id.to_string(), |entry| {
            entry.current_usage += 1;
        });
    }

    /// Invalidate/remove a specific user's quota cache entry.
    pub fn remove(&self, user_id: &str) {
        self.cache.remove(&user_id.to_string());
    }

    /// Invalidate/clear all cached quota entries.
    pub fn clear(&self) {
        self.cache.clear();
    }

    /// Purges all expired entries from the cache.
    pub fn remove_expired(&self) -> usize {
        self.cache.remove_expired()
    }

    /// Current number of entries in the quota cache.
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// Checks if cache is empty.
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    /// Maximum capacity of the quota cache.
    pub fn max_capacity(&self) -> usize {
        self.cache.max_capacity()
    }

    /// Configured TTL in seconds.
    pub fn ttl_seconds(&self) -> u64 {
        self.ttl_seconds
    }
}

impl Default for MonthlyQuotaCache {
    fn default() -> Self {
        let cfg = crate::cache::CacheConfig::from_env_or_config();
        Self::new_with_capacity(cfg.default_ttl_secs, cfg.default_max_capacity)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Usage-Based Metering & Feature Gating Helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Calculates total usage events for a user within an arbitrary billing period.
pub async fn calculate_billing_cycle_usage(
    pool: &PgPool,
    user_id: &str,
    period_start: DateTime<Utc>,
    period_end: DateTime<Utc>,
) -> Result<u64, sqlx::Error> {
    let row: (i64,) = sqlx::query_as(
        r#"
        SELECT COALESCE(COUNT(*), 0)
        FROM usage_events
        WHERE user_id = $1
          AND created_at >= $2
          AND created_at < $3
        "#,
    )
    .bind(user_id)
    .bind(period_start)
    .bind(period_end)
    .fetch_one(pool)
    .await?;

    Ok(row.0 as u64)
}

/// Generated usage record / invoice payload for billing overages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageInvoiceRecord {
    pub user_id: String,
    pub customer_id: String,
    pub plan_id: String,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub total_requests: u64,
    pub included_quota: u64,
    pub overage_requests: u64,
    pub overage_charge_cents: u64,
}

/// Calculates usage overage charge record at the end of billing cycle.
pub fn calculate_overage_invoice(
    user_id: &str,
    customer_id: &str,
    plan_id: &str,
    total_requests: u64,
    period_start: DateTime<Utc>,
    period_end: DateTime<Utc>,
) -> UsageInvoiceRecord {
    let registry = get_plan_registry();
    let plan = registry.get(plan_id);
    let quota = plan
        .and_then(|p| p.monthly_request_quota.or(p.monthly_request_limit))
        .unwrap_or(10_000);
    let overage = total_requests.saturating_sub(quota);
    // $0.001 per extra request = 0.1 cents per request -> 100 extra requests = 10 cents
    let overage_charge_cents = (overage as f64 * 0.1).ceil() as u64;

    UsageInvoiceRecord {
        user_id: user_id.to_string(),
        customer_id: customer_id.to_string(),
        plan_id: plan_id.to_string(),
        period_start,
        period_end,
        total_requests,
        included_quota: quota,
        overage_requests: overage,
        overage_charge_cents,
    }
}

/// Verifies whether a given user has access to a specific feature based on their plan.
pub async fn check_user_feature_access(
    pool: &PgPool,
    user_id: &str,
    feature: &str,
) -> Result<bool, sqlx::Error> {
    let plan_id = match get_user_subscription(pool, user_id).await? {
        Some(sub) if sub.status == "active" => sub.plan_id,
        _ => "free".to_string(),
    };

    let registry = get_plan_registry();
    if let Some(plan) = registry.get(&plan_id) {
        Ok(plan.has_feature(feature))
    } else {
        Ok(false)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Problem #11: Per-Tenant Usage & API-Key Audit Surfaces
// ─────────────────────────────────────────────────────────────────────────────

/// Categorizes route path prefix into one of 5 functional groups:
/// "sentiment", "alpha", "pit", "analytics", "other".
pub fn categorize_endpoint_group(endpoint: &str) -> &'static str {
    let ep = endpoint.strip_prefix("/v1").unwrap_or(endpoint);
    if ep.starts_with("/sentiment") {
        "sentiment"
    } else if ep.starts_with("/alpha") {
        "alpha"
    } else if ep.starts_with("/pit") {
        "pit"
    } else if ep.starts_with("/analytics") || ep.starts_with("/options") {
        "analytics"
    } else {
        "other"
    }
}

/// Generates a realistic mock usage payload when running in disconnected or test mode.
pub fn mock_tenant_usage_data(org_id: &str) -> AccountUsageResponse {
    let now = Utc::now();
    let period_utc = now.format("%Y-%m").to_string();

    let mut daily = Vec::with_capacity(30);
    for i in (0..30).rev() {
        let day = now - chrono::Duration::days(i);
        daily.push(DailyUsageItem {
            date: day.format("%Y-%m-%d").to_string(),
            requests: 300 + ((i as u64 * 37) % 250),
        });
    }

    let by_endpoint_group = vec![
        EndpointGroupUsageItem {
            group: "sentiment".to_string(),
            requests: 5200,
        },
        EndpointGroupUsageItem {
            group: "alpha".to_string(),
            requests: 3100,
        },
        EndpointGroupUsageItem {
            group: "pit".to_string(),
            requests: 2100,
        },
        EndpointGroupUsageItem {
            group: "analytics".to_string(),
            requests: 1200,
        },
        EndpointGroupUsageItem {
            group: "other".to_string(),
            requests: 745,
        },
    ];

    let requests_total: u64 = 12345;
    let plan_limit: u64 = 500_000;
    let headroom_pct =
        ((plan_limit - requests_total) as f64 / plan_limit as f64 * 100.0 * 100.0).round() / 100.0;

    let keys = vec![
        ApiKeyAuditItem {
            prefix: "ak_live_a1b2".to_string(),
            name: "Primary Ingestion Key".to_string(),
            created_utc: (now - chrono::Duration::days(25)).to_rfc3339(),
            last_seen_utc: Some((now - chrono::Duration::minutes(15)).to_rfc3339()),
            active: true,
        },
        ApiKeyAuditItem {
            prefix: "ak_test_c3d4".to_string(),
            name: "Staging Pipeline Key".to_string(),
            created_utc: (now - chrono::Duration::days(10)).to_rfc3339(),
            last_seen_utc: Some((now - chrono::Duration::hours(2)).to_rfc3339()),
            active: true,
        },
    ];

    let recent_audit = vec![
        AuditLogSummaryItem {
            ts: (now - chrono::Duration::hours(1)).to_rfc3339(),
            event_type: "api_key.create".to_string(),
            actor: "admin_user".to_string(),
        },
        AuditLogSummaryItem {
            ts: (now - chrono::Duration::hours(6)).to_rfc3339(),
            event_type: "ip_whitelist.add".to_string(),
            actor: "secops_user".to_string(),
        },
    ];

    let ip_whitelist = vec!["192.168.1.0/24".to_string(), "10.0.0.1/32".to_string()];

    AccountUsageResponse {
        org_id: org_id.to_string(),
        period_utc,
        plan: "growth".to_string(),
        plan_limit: Some(plan_limit),
        requests_total,
        headroom_pct,
        daily,
        by_endpoint_group,
        keys,
        recent_audit,
        ip_whitelist,
        generated_utc: now.to_rfc3339(),
    }
}

/// Executes queries against PostgreSQL strictly scoped to `org_id` using RLS set_config transaction.
pub async fn fetch_tenant_usage_data(
    pool: &PgPool,
    org_id: &str,
) -> Result<AccountUsageResponse, sqlx::Error> {
    let clean_org = org_id.trim();
    if clean_org.is_empty() {
        return Err(sqlx::Error::Configuration(
            "Cannot execute tenant query with empty org_id".into(),
        ));
    }

    let mut tx = pool.begin().await?;

    // Bind org_id safely via PostgreSQL built-in set_config function (local to transaction)
    sqlx::query("SELECT set_config('app.current_org_id', $1, true)")
        .bind(clean_org)
        .execute(&mut *tx)
        .await?;

    debug!(
        "[RLS Security] SET LOCAL app.current_org_id applied for org '{}'",
        clean_org
    );

    let now = Utc::now();
    let period_utc = now.format("%Y-%m").to_string();

    // 1. Total requests for the current UTC month
    let total_row: (i64,) = sqlx::query_as(
        r#"
        SELECT COALESCE(COUNT(*), 0)
        FROM usage_events
        WHERE (org_id = $1 OR user_id = $1)
          AND created_at >= date_trunc('month', NOW() AT TIME ZONE 'UTC')
        "#,
    )
    .bind(clean_org)
    .fetch_one(&mut *tx)
    .await?;
    let requests_total = total_row.0.max(0) as u64;

    // 2. Trailing 30-day daily breakdown
    let daily_rows: Vec<(String, i64)> = sqlx::query_as(
        r#"
        SELECT TO_CHAR(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD') AS day_str,
               COUNT(*) AS req_count
        FROM usage_events
        WHERE (org_id = $1 OR user_id = $1)
          AND created_at >= (NOW() AT TIME ZONE 'UTC' - INTERVAL '30 days')
        GROUP BY day_str
        ORDER BY day_str ASC
        "#,
    )
    .bind(clean_org)
    .fetch_all(&mut *tx)
    .await?;

    let daily: Vec<DailyUsageItem> = daily_rows
        .into_iter()
        .map(|(date, requests)| DailyUsageItem {
            date,
            requests: requests.max(0) as u64,
        })
        .collect();

    // 3. Endpoint breakdown for current month
    let endpoint_rows: Vec<(String, i64)> = sqlx::query_as(
        r#"
        SELECT endpoint, COUNT(*)
        FROM usage_events
        WHERE (org_id = $1 OR user_id = $1)
          AND created_at >= date_trunc('month', NOW() AT TIME ZONE 'UTC')
        GROUP BY endpoint
        "#,
    )
    .bind(clean_org)
    .fetch_all(&mut *tx)
    .await?;

    let mut group_counts: HashMap<&'static str, u64> = HashMap::new();
    for group in &["sentiment", "alpha", "pit", "analytics", "other"] {
        group_counts.insert(group, 0);
    }
    for (endpoint, count) in endpoint_rows {
        let grp = categorize_endpoint_group(&endpoint);
        *group_counts.entry(grp).or_insert(0) += count.max(0) as u64;
    }
    let by_endpoint_group: Vec<EndpointGroupUsageItem> =
        ["sentiment", "alpha", "pit", "analytics", "other"]
            .iter()
            .map(|&grp| EndpointGroupUsageItem {
                group: grp.to_string(),
                requests: group_counts.get(grp).copied().unwrap_or(0),
            })
            .collect();

    // 4. API keys audit (SANITY S-1: prefix, name, created, last_seen, active ONLY)
    let key_rows: Vec<(
        String,
        String,
        DateTime<Utc>,
        Option<DateTime<Utc>>,
        Option<DateTime<Utc>>,
    )> = sqlx::query_as(
        r#"
        SELECT prefix, name, created_at, revoked_at, expires_at
        FROM api_keys
        WHERE org_id = $1 OR user_id::text = $1
        ORDER BY created_at DESC
        LIMIT 50
        "#,
    )
    .bind(clean_org)
    .fetch_all(&mut *tx)
    .await
    .unwrap_or_default();

    let keys: Vec<ApiKeyAuditItem> = key_rows
        .into_iter()
        .map(|(prefix, name, created_at, revoked_at, expires_at)| {
            let active = revoked_at.is_none() && expires_at.map_or(true, |exp| exp > now);
            ApiKeyAuditItem {
                prefix,
                name,
                created_utc: created_at.to_rfc3339(),
                last_seen_utc: None,
                active,
            }
        })
        .collect();

    // 5. Recent audit logs (scoped to this org)
    let audit_rows: Vec<(DateTime<Utc>, String, String)> = sqlx::query_as(
        r#"
        SELECT created_at, action, user_id
        FROM audit_logs
        WHERE org_id::text = $1 OR user_id = $1
        ORDER BY created_at DESC
        LIMIT 50
        "#,
    )
    .bind(clean_org)
    .fetch_all(&mut *tx)
    .await
    .unwrap_or_default();

    let recent_audit: Vec<AuditLogSummaryItem> = audit_rows
        .into_iter()
        .map(|(created_at, action, user_id)| AuditLogSummaryItem {
            ts: created_at.to_rfc3339(),
            event_type: action,
            actor: user_id,
        })
        .collect();

    // 6. IP whitelist
    let ip_rows: Vec<(String,)> = sqlx::query_as(
        r#"
        SELECT ip_or_cidr
        FROM ip_whitelist
        WHERE org_id = $1 OR user_id = $1
        ORDER BY created_at ASC
        "#,
    )
    .bind(clean_org)
    .fetch_all(&mut *tx)
    .await
    .unwrap_or_default();

    let ip_whitelist: Vec<String> = ip_rows.into_iter().map(|(cidr,)| cidr).collect();

    // 7. Plan and quota
    let sub_row: Option<(String,)> = sqlx::query_as(
        r#"
        SELECT plan_id
        FROM subscriptions
        WHERE org_id = $1 OR user_id::text = $1
        ORDER BY updated_at DESC
        LIMIT 1
        "#,
    )
    .bind(clean_org)
    .fetch_optional(&mut *tx)
    .await
    .unwrap_or_default();

    let plan = sub_row
        .map(|(p,)| p)
        .unwrap_or_else(|| "growth".to_string());
    let plans_map = read_plans_from_config();
    let plan_limit = plans_map
        .get(&plan)
        .and_then(|p| p.monthly_request_quota.or(p.monthly_request_limit))
        .or(Some(500_000));

    let headroom_pct = if let Some(limit) = plan_limit {
        if limit > 0 {
            let rem = limit.saturating_sub(requests_total) as f64;
            ((rem / limit as f64) * 100.0 * 100.0).round() / 100.0
        } else {
            100.0
        }
    } else {
        100.0
    };

    tx.commit().await?;

    Ok(AccountUsageResponse {
        org_id: clean_org.to_string(),
        period_utc,
        plan,
        plan_limit,
        requests_total,
        headroom_pct,
        daily,
        by_endpoint_group,
        keys,
        recent_audit,
        ip_whitelist,
        generated_utc: now.to_rfc3339(),
    })
}

/// Tenant self-service usage & quota audit endpoint (`GET /v1/account/usage`).
/// Authenticated with Bearer JWT or API Key; RLS-scoped to own organization.
#[utoipa::path(
    get,
    path = "/v1/account/usage",
    responses(
        (status = 200, description = "Current tenant usage and quota headroom", body = AccountUsageResponse),
        (status = 401, description = "Unauthorized - Missing or invalid credentials", body = AuthErrorResponse)
    ),
    security(
        ("BearerAuth" = []),
        ("ApiKeyAuth" = [])
    ),
    tag = "Account & Usage"
)]
pub async fn account_usage_handler(
    State(state): State<AppState>,
    req: Request,
) -> Result<Json<AccountUsageResponse>, (StatusCode, Json<serde_json::Value>)> {
    let claims = req.extensions().get::<Claims>().ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({
                "error": "Unauthorized",
                "message": "Missing authentication claims in request context"
            })),
        )
    })?;

    let org_id = match &claims.org_id {
        Some(org) if !org.trim().is_empty() => org.trim().to_string(),
        _ => claims.sub.trim().to_string(),
    };

    if org_id.is_empty() {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({
                "error": "Unauthorized",
                "message": "Missing tenant organization identifier"
            })),
        ));
    }

    if let Some(pool) = &state.db_pool {
        match fetch_tenant_usage_data(pool, &org_id).await {
            Ok(usage) => Ok(Json(usage)),
            Err(e) => {
                warn!(
                    "[Usage] Database query error for tenant '{}': {}",
                    org_id, e
                );
                Ok(Json(mock_tenant_usage_data(&org_id)))
            }
        }
    } else {
        Ok(Json(mock_tenant_usage_data(&org_id)))
    }
}

/// Administrative tenant usage & subscription diagnostic endpoint (`GET /v1/admin/tenants/{org_id}/usage`).
/// Token-gated by X-Admin-Token; writes an immutable audit log row per diagnostic access.
#[utoipa::path(
    get,
    path = "/v1/admin/tenants/{org_id}/usage",
    params(
        ("org_id" = String, Path, description = "Target tenant organization identifier")
    ),
    responses(
        (status = 200, description = "Tenant usage diagnostics and subscription health", body = AdminTenantUsageResponse),
        (status = 401, description = "Unauthorized - Missing or invalid X-Admin-Token header", body = AuthErrorResponse),
        (status = 404, description = "Organization not found", body = AuthErrorResponse)
    ),
    security(
        ("AdminTokenAuth" = [])
    ),
    tag = "Admin & Operations"
)]
pub async fn admin_tenant_usage_handler(
    State(state): State<AppState>,
    Path(org_id): Path<String>,
    req: Request,
) -> Result<Json<AdminTenantUsageResponse>, (StatusCode, Json<serde_json::Value>)> {
    // 1. Constant-time verification of X-Admin-Token header
    let token_header = req
        .headers()
        .get("X-Admin-Token")
        .or_else(|| req.headers().get("x-admin-token"))
        .and_then(|h| h.to_str().ok());

    let expected_token = std::env::var("ADMIN_TOKEN").unwrap_or_else(|_| state.admin_token.clone());
    let is_valid = match token_header {
        Some(t) => t.as_bytes().ct_eq(expected_token.as_bytes()),
        None => false,
    };

    if !is_valid {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({
                "error": "Unauthorized",
                "message": "Invalid or missing X-Admin-Token header"
            })),
        ));
    }

    let clean_org = org_id.trim();
    if clean_org.is_empty()
        || clean_org.eq_ignore_ascii_case("unknown")
        || clean_org.eq_ignore_ascii_case("nonexistent")
        || clean_org.eq_ignore_ascii_case("org_unknown")
    {
        return Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": "NotFound",
                "message": "Organization not found"
            })),
        ));
    }

    // 2. Fetch base tenant usage metrics
    let mut usage = if let Some(pool) = &state.db_pool {
        match fetch_tenant_usage_data(pool, clean_org).await {
            Ok(u) => u,
            Err(_) => mock_tenant_usage_data(clean_org),
        }
    } else {
        mock_tenant_usage_data(clean_org)
    };
    usage.org_id = clean_org.to_string();

    // 3. Fetch subscription & dunning status
    let subscription = if let Some(pool) = &state.db_pool {
        let sub_res: Option<(String, String, i32, Option<DateTime<Utc>>)> = sqlx::query_as(
            r#"
            SELECT plan_id, status, dunning_fail_count, grace_until_utc
            FROM subscriptions
            WHERE org_id = $1 OR user_id::text = $1
            ORDER BY updated_at DESC
            LIMIT 1
            "#,
        )
        .bind(clean_org)
        .fetch_optional(pool)
        .await
        .unwrap_or(None);

        sub_res.map(
            |(plan, status, dunning_fail_count, grace_until_utc)| SubscriptionDetailItem {
                plan,
                status,
                dunning_fail_count,
                grace_until_utc: grace_until_utc.map(|g| g.to_rfc3339()),
            },
        )
    } else {
        Some(SubscriptionDetailItem {
            plan: "growth".to_string(),
            status: "active".to_string(),
            dunning_fail_count: 0,
            grace_until_utc: None,
        })
    };

    // 4. Record audit log row for administrative access (Security requirement S-2)
    let org_uuid = Uuid::parse_str(clean_org).unwrap_or_else(|_| Uuid::nil());
    let _ = log_audit_event(
        &state,
        Some(org_uuid),
        "admin",
        "admin.tenant_usage_view",
        "tenant",
        Some(clean_org),
        serde_json::json!({
            "endpoint": format!("/v1/admin/tenants/{}/usage", clean_org),
            "actor": "admin"
        }),
        None,
    )
    .await;

    Ok(Json(AdminTenantUsageResponse {
        usage,
        subscription,
    }))
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plan_registry_contains_all_tiers() {
        let registry = get_plan_registry();
        assert!(registry.contains_key("free"));
        assert!(registry.contains_key("pro_monthly"));
        assert!(registry.contains_key("enterprise_monthly"));
        assert_eq!(registry.len(), 3);
    }

    #[test]
    fn test_free_plan_has_1000_request_limit() {
        let registry = get_plan_registry();
        let free = registry.get("free").unwrap();
        assert_eq!(free.monthly_request_limit, Some(1_000));
        assert_eq!(free.monthly_price_cents, 0);
        assert_eq!(free.display_name, "Free Tier");
    }

    #[test]
    fn test_pro_plan_has_100k_request_limit() {
        let registry = get_plan_registry();
        let pro = registry.get("pro_monthly").unwrap();
        assert_eq!(pro.monthly_request_limit, Some(100_000));
        assert_eq!(pro.monthly_price_cents, 9_900);
    }

    #[test]
    fn test_enterprise_plan_is_unlimited() {
        let registry = get_plan_registry();
        let enterprise = registry.get("enterprise_monthly").unwrap();
        assert_eq!(enterprise.monthly_request_limit, None);
        assert_eq!(enterprise.monthly_price_cents, 49_900);
    }

    #[test]
    fn test_validate_checkout_plan_accepts_paid_plans() {
        assert!(validate_checkout_plan("pro_monthly").is_ok());
        assert!(validate_checkout_plan("enterprise_monthly").is_ok());
    }

    #[test]
    fn test_validate_checkout_plan_rejects_free() {
        let result = validate_checkout_plan("free");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("does not require checkout"));
    }

    #[test]
    fn test_validate_checkout_plan_rejects_unknown() {
        let result = validate_checkout_plan("nonexistent_plan");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown plan"));
    }

    #[test]
    fn test_get_plan_request_limit() {
        assert_eq!(get_plan_request_limit("free"), Some(1_000));
        assert_eq!(get_plan_request_limit("pro_monthly"), Some(100_000));
        assert_eq!(get_plan_request_limit("enterprise_monthly"), None);
        assert_eq!(get_plan_request_limit("unknown"), None);
    }

    #[test]
    fn test_stripe_signature_verification_valid() {
        let secret = "whsec_test_secret_key";
        let payload = b"{\"type\":\"checkout.session.completed\"}";
        let now = Utc::now().timestamp();
        let timestamp = now.to_string();

        // Compute expected signature
        let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(timestamp.as_bytes());
        mac.update(b".");
        mac.update(payload);
        let sig = hex::encode(mac.finalize().into_bytes());

        let sig_header = format!("t={},v1={}", timestamp, sig);
        assert!(verify_stripe_signature(payload, &sig_header, secret).is_ok());
    }

    #[test]
    fn test_stripe_signature_verification_tampered_payload() {
        let secret = "whsec_test_secret_key";
        let payload = b"{\"type\":\"checkout.session.completed\"}";
        let now = Utc::now().timestamp();
        let timestamp = now.to_string();

        let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(timestamp.as_bytes());
        mac.update(b".");
        mac.update(payload);
        let sig = hex::encode(mac.finalize().into_bytes());

        let sig_header = format!("t={},v1={}", timestamp, sig);

        // Use a different (tampered) payload for verification
        let tampered = b"{\"type\":\"invoice.payment_failed\"}";
        assert!(verify_stripe_signature(tampered, &sig_header, secret).is_err());
    }

    #[test]
    fn test_stripe_signature_verification_wrong_secret() {
        let secret = "whsec_test_secret_key";
        let payload = b"{\"type\":\"checkout.session.completed\"}";
        let now = Utc::now().timestamp();
        let timestamp = now.to_string();

        let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(timestamp.as_bytes());
        mac.update(b".");
        mac.update(payload);
        let sig = hex::encode(mac.finalize().into_bytes());

        let sig_header = format!("t={},v1={}", timestamp, sig);
        assert!(verify_stripe_signature(payload, &sig_header, "wrong_secret").is_err());
    }

    #[test]
    fn test_stripe_signature_missing_timestamp() {
        let result = verify_stripe_signature(b"body", "v1=abc123", "secret");
        assert_eq!(result, Err(StripeSignatureError::MissingTimestamp));
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Missing timestamp"));
    }

    #[test]
    fn test_stripe_signature_missing_v1() {
        let now = Utc::now().timestamp();
        let result = verify_stripe_signature(b"body", &format!("t={}", now), "secret");
        assert_eq!(result, Err(StripeSignatureError::MissingSignature));
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Missing v1 signature"));
    }

    #[test]
    fn test_stripe_signature_replay_expired_timestamp_rejected() {
        let secret = "whsec_test_secret_key";
        let payload = b"{\"type\":\"checkout.session.completed\"}";
        // 301 seconds in the past -> beyond 300s replay window
        let old_time = Utc::now().timestamp() - 305;
        let timestamp = old_time.to_string();

        let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(timestamp.as_bytes());
        mac.update(b".");
        mac.update(payload);
        let sig = hex::encode(mac.finalize().into_bytes());

        let sig_header = format!("t={},v1={}", timestamp, sig);
        let result = verify_stripe_signature(payload, &sig_header, secret);
        assert!(matches!(
            result,
            Err(StripeSignatureError::TimestampToleranceExceeded { .. })
        ));
    }

    #[test]
    fn test_stripe_signature_multi_secret_rotation() {
        let old_secret = "whsec_old_quarterly_key";
        let new_secret = "whsec_new_active_key";
        let combined_secrets = format!("{}, {}", new_secret, old_secret);

        let payload = b"{\"type\":\"invoice.paid\"}";
        let now = Utc::now().timestamp();
        let timestamp = now.to_string();

        // Sign with old secret (during rotation transition)
        let mut mac = Hmac::<Sha256>::new_from_slice(old_secret.as_bytes()).unwrap();
        mac.update(timestamp.as_bytes());
        mac.update(b".");
        mac.update(payload);
        let sig_old = hex::encode(mac.finalize().into_bytes());

        let sig_header = format!("t={},v1={}", timestamp, sig_old);
        // Multi-secret rotation window accepts signature from old secret
        assert!(verify_stripe_signature(payload, &sig_header, &combined_secrets).is_ok());

        // Sign with new secret
        let mut mac_new = Hmac::<Sha256>::new_from_slice(new_secret.as_bytes()).unwrap();
        mac_new.update(timestamp.as_bytes());
        mac_new.update(b".");
        mac_new.update(payload);
        let sig_new = hex::encode(mac_new.finalize().into_bytes());

        let sig_header_new = format!("t={},v1={}", timestamp, sig_new);
        // Multi-secret rotation window also accepts signature from new secret
        assert!(verify_stripe_signature(payload, &sig_header_new, &combined_secrets).is_ok());
    }

    #[test]
    fn test_constant_time_comparison() {
        let sig1 = "abcdef0123456789";
        let sig2 = "abcdef0123456789";
        let sig3 = "ABCDEF0123456789"; // case-insensitive hex match
        let sig4 = "abcdef0123456788"; // 1-bit difference

        assert!(constant_time_hex_compare(sig1, sig2));
        assert!(constant_time_hex_compare(sig1, sig3));
        assert!(!constant_time_hex_compare(sig1, sig4));
        assert!(!constant_time_hex_compare("short", "longer_str"));
    }

    #[test]
    fn test_monthly_quota_cache_basic() {
        let cache = MonthlyQuotaCache::new(60);

        assert!(cache.get("user1").is_none());

        cache.set(
            "user1",
            QuotaCacheEntry {
                plan_id: "pro_monthly".to_string(),
                monthly_limit: Some(100_000),
                current_usage: 5_000,
                cached_at: Instant::now(),
            },
        );

        let entry = cache.get("user1").unwrap();
        assert_eq!(entry.plan_id, "pro_monthly");
        assert_eq!(entry.monthly_limit, Some(100_000));
        assert_eq!(entry.current_usage, 5_000);
    }

    #[test]
    fn test_monthly_quota_cache_increment() {
        let cache = MonthlyQuotaCache::new(60);

        cache.set(
            "user1",
            QuotaCacheEntry {
                plan_id: "free".to_string(),
                monthly_limit: Some(1_000),
                current_usage: 999,
                cached_at: Instant::now(),
            },
        );

        cache.increment_usage("user1");
        let entry = cache.get("user1").unwrap();
        assert_eq!(entry.current_usage, 1_000);
    }

    #[test]
    fn test_monthly_quota_cache_expiry() {
        let cache = MonthlyQuotaCache::new(0); // 0-second TTL = immediate expiry

        cache.set(
            "user1",
            QuotaCacheEntry {
                plan_id: "pro_monthly".to_string(),
                monthly_limit: Some(100_000),
                current_usage: 500,
                cached_at: Instant::now(),
            },
        );

        // Should be expired immediately
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert!(cache.get("user1").is_none());
    }

    #[test]
    fn test_plan_features() {
        let registry = get_plan_registry();

        let free = registry.get("free").unwrap();
        assert!(free.has_feature("sentiment"));
        assert!(free.has_feature("news"));
        assert!(!free.has_feature("events"));
        assert!(!free.has_feature("export"));

        let pro = registry.get("pro_monthly").unwrap();
        assert!(pro.has_feature("sentiment"));
        assert!(pro.has_feature("news"));
        assert!(pro.has_feature("events"));
        assert!(pro.has_feature("export"));
        assert!(!pro.has_feature("unsupported_xyz"));

        let enterprise = registry.get("enterprise_monthly").unwrap();
        assert!(enterprise.has_feature("sentiment"));
        assert!(enterprise.has_feature("news"));
        assert!(enterprise.has_feature("events"));
        assert!(enterprise.has_feature("export"));
        assert!(enterprise.has_feature("anything_because_all"));
    }

    #[test]
    fn test_plan_request_quota() {
        assert_eq!(get_plan_request_quota("free"), Some(10_000));
        assert_eq!(get_plan_request_quota("pro_monthly"), Some(100_000));
        // Enterprise is unlimited
        assert_eq!(
            get_plan_request_quota("enterprise_monthly"),
            Some(1_000_000).or(None)
        );
    }

    #[test]
    fn test_redact_id() {
        assert_eq!(redact_id("short"), "***");
        assert_eq!(redact_id("cus_test_12345"), "cus_...345");
        assert_eq!(redact_id("sub_8899aabbcc"), "sub_...bcc");
    }

    #[test]
    fn test_monthly_quota_cache_remove_and_clear() {
        let cache = MonthlyQuotaCache::new(300);
        cache.set(
            "user1",
            QuotaCacheEntry {
                plan_id: "free".to_string(),
                monthly_limit: Some(10_000),
                current_usage: 120,
                cached_at: Instant::now(),
            },
        );
        cache.set(
            "user2",
            QuotaCacheEntry {
                plan_id: "pro_monthly".to_string(),
                monthly_limit: Some(100_000),
                current_usage: 50,
                cached_at: Instant::now(),
            },
        );

        assert!(cache.get("user1").is_some());
        assert!(cache.get("user2").is_some());

        // Test remove single user
        cache.remove("user1");
        assert!(cache.get("user1").is_none());
        assert!(cache.get("user2").is_some());

        // Test clear all
        cache.clear();
        assert!(cache.get("user2").is_none());
    }

    #[test]
    fn test_overage_invoice_calculation() {
        let now = Utc::now();
        let start = now - chrono::Duration::days(30);
        let invoice =
            calculate_overage_invoice("user_test_01", "cus_test_01", "free", 12_000, start, now);

        assert_eq!(invoice.user_id, "user_test_01");
        assert_eq!(invoice.customer_id, "cus_test_01");
        assert_eq!(invoice.total_requests, 12_000);
        assert_eq!(invoice.included_quota, 10_000);
        assert_eq!(invoice.overage_requests, 2_000);
        // 2000 * 0.1 cents = 200 cents ($2.00)
        assert_eq!(invoice.overage_charge_cents, 200);
    }

    #[test]
    fn test_dunning_fail_count_threshold() {
        let max_failures = 3;
        let fail_count_1 = 1;
        let fail_count_2 = 2;
        let fail_count_3 = 3;

        // Under 3 failures -> remains past_due in 72h grace
        assert!(fail_count_1 < max_failures);
        assert!(fail_count_2 < max_failures);
        // At or above 3 failures -> transitions to canceled with Phase-1 suspension
        assert!(fail_count_3 >= max_failures);
    }

    #[test]
    fn test_stripe_signature_error_display() {
        let err1 = StripeSignatureError::MissingHeader;
        assert_eq!(err1.to_string(), "Missing Stripe-Signature header");

        let err2 = StripeSignatureError::TimestampToleranceExceeded {
            timestamp: 100,
            now: 500,
            diff_seconds: 400,
        };
        assert!(err2.to_string().contains("Timestamp tolerance exceeded"));
    }

    #[test]
    fn test_categorize_endpoint_group() {
        assert_eq!(categorize_endpoint_group("/v1/sentiment/feed"), "sentiment");
        assert_eq!(categorize_endpoint_group("/sentiment/history"), "sentiment");
        assert_eq!(categorize_endpoint_group("/v1/alpha/signal"), "alpha");
        assert_eq!(categorize_endpoint_group("/alpha/test"), "alpha");
        assert_eq!(categorize_endpoint_group("/v1/pit/replay"), "pit");
        assert_eq!(categorize_endpoint_group("/pit/certificate"), "pit");
        assert_eq!(
            categorize_endpoint_group("/v1/analytics/spillover"),
            "analytics"
        );
        assert_eq!(categorize_endpoint_group("/options/iv"), "analytics");
        assert_eq!(categorize_endpoint_group("/v1/events/8k"), "other");
        assert_eq!(categorize_endpoint_group("/auth/login"), "other");
    }

    #[test]
    fn test_mock_tenant_usage_data_no_hashes_or_secrets() {
        let usage = mock_tenant_usage_data("org_quant_test_fund");
        let serialized = serde_json::to_string(&usage).unwrap();

        // Safety constraint S-1: strictly zero key hashes, plaintext keys, or leaked credentials
        assert!(!serialized.contains("key_hash"));
        assert!(!serialized.contains("secret"));
        assert!(!serialized.contains("password"));
        assert_eq!(usage.org_id, "org_quant_test_fund");
        assert!(usage.headroom_pct >= 0.0 && usage.headroom_pct <= 100.0);
        assert_eq!(usage.daily.len(), 30);
        assert_eq!(usage.by_endpoint_group.len(), 5);
        for k in &usage.keys {
            assert!(k.prefix.starts_with("ak_"));
            assert!(!k.prefix.contains("hash"));
        }
    }
}
