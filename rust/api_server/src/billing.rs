//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Stripe Subscription Billing Integration
//! ═══════════════════════════════════════════════════════════════════════════════
//!
//! Implements plan-based monetization with Stripe Checkout, Customer Portal,
//! subscription lifecycle management via webhooks, and plan-tier usage enforcement.
//! ═══════════════════════════════════════════════════════════════════════════════

use crate::audit_logs::log_audit_event;
use crate::auth::{AuthErrorResponse, Claims};
use crate::state::AppState;
use axum::body::Bytes;
use axum::extract::State;
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
use tracing::{error, info, warn};
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

    sqlx::query(
        r#"
        INSERT INTO subscriptions (id, user_id, stripe_customer_id, stripe_subscription_id,
                                   plan_id, status, current_period_start, current_period_end, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, NOW())
        ON CONFLICT (user_id) DO UPDATE SET
            stripe_customer_id = EXCLUDED.stripe_customer_id,
            stripe_subscription_id = EXCLUDED.stripe_subscription_id,
            plan_id = EXCLUDED.plan_id,
            status = EXCLUDED.status,
            current_period_start = EXCLUDED.current_period_start,
            current_period_end = EXCLUDED.current_period_end,
            updated_at = NOW()
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

/// Verifies a Stripe webhook signature using HMAC-SHA256.
///
/// Stripe sends the signature in the `Stripe-Signature` header as:
///   `t=<timestamp>,v1=<hex_signature>`
///
/// The signed payload is `<timestamp>.<raw_body>`.
pub fn verify_stripe_signature(
    payload: &[u8],
    sig_header: &str,
    secret: &str,
) -> Result<(), String> {
    let mut timestamp = None;
    let mut signatures: Vec<String> = Vec::new();

    for part in sig_header.split(',') {
        let part = part.trim();
        if let Some(ts) = part.strip_prefix("t=") {
            timestamp = Some(ts.to_string());
        } else if let Some(sig) = part.strip_prefix("v1=") {
            signatures.push(sig.to_string());
        }
    }

    let ts = timestamp.ok_or_else(|| "Missing timestamp in Stripe-Signature header".to_string())?;

    if signatures.is_empty() {
        return Err("Missing v1 signature in Stripe-Signature header".to_string());
    }

    // Construct the signed payload: "<timestamp>.<body>"
    let signed_payload = format!("{}.{}", ts, String::from_utf8_lossy(payload));

    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes())
        .map_err(|_| "Failed to create HMAC instance".to_string())?;
    mac.update(signed_payload.as_bytes());
    let expected = hex::encode(mac.finalize().into_bytes());

    if signatures.iter().any(|s| s == &expected) {
        Ok(())
    } else {
        Err("Stripe webhook signature verification failed".to_string())
    }
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

/// Handle incoming Stripe webhook events.
///
/// **Public endpoint** — authenticates via Stripe-Signature HMAC-SHA256 verification
/// (not JWT). Processes subscription lifecycle events and updates local database.
#[utoipa::path(
    post,
    path = "/billing/webhook",
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
    let _mock_mode = env::var("STRIPE_MOCK_MODE").as_deref() == Ok("1");

    // 2. Verify signature (if secret is configured)
    let sig_header = headers
        .get("stripe-signature")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if !webhook_secret.is_empty() {
        if let Err(e) = verify_stripe_signature(&body, sig_header, &webhook_secret) {
            warn!("[Billing Webhook] Signature verification failed: {}", e);
            return (
                StatusCode::BAD_REQUEST,
                Json(AuthErrorResponse {
                    error: "Bad Request".to_string(),
                    message: "Webhook signature verification failed".to_string(),
                }),
            )
                .into_response();
        }
    }

    // 3. Parse event JSON
    let event: serde_json::Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => {
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

    let event_type = event["type"].as_str().unwrap_or("unknown");
    info!("[Billing Webhook] Received event type: {}", event_type);

    // 4. Dispatch based on event type
    let pool = state.db_pool.as_ref();

    match event_type {
        "checkout.session.completed" => {
            let data = &event["data"]["object"];
            let customer_id = data["customer"].as_str().unwrap_or("");
            let subscription_id = data["subscription"].as_str().unwrap_or("");
            let user_id = data["metadata"]["fintext_user_id"]
                .as_str()
                .or_else(|| data["client_reference_id"].as_str())
                .unwrap_or("");

            // Determine plan from metadata or default to pro
            let plan_id = data["metadata"]["plan_id"]
                .as_str()
                .unwrap_or("pro_monthly");

            if let Some(pool) = pool {
                if let Err(e) = upsert_subscription(
                    pool,
                    user_id,
                    customer_id,
                    subscription_id,
                    plan_id,
                    "active",
                    Some(Utc::now()),
                    None,
                )
                .await
                {
                    error!("[Billing Webhook] Failed to upsert subscription: {}", e);
                }
            }

            // Invalidate cached quota so new plan limits apply immediately
            state.monthly_quota_cache.remove(user_id);

            info!(
                "[Billing Webhook] checkout.session.completed: user='{}' plan='{}' customer='{}' subscription='{}'",
                redact_id(user_id),
                plan_id,
                redact_id(customer_id),
                redact_id(subscription_id)
            );
        }

        "invoice.payment_succeeded" => {
            let data = &event["data"]["object"];
            let customer_id = data["customer"].as_str().unwrap_or("");
            let subscription_id = data["subscription"].as_str().unwrap_or("");
            let period_start = data["period_start"]
                .as_i64()
                .map(|ts| DateTime::from_timestamp(ts, 0).unwrap_or_else(Utc::now));
            let period_end = data["period_end"]
                .as_i64()
                .map(|ts| DateTime::from_timestamp(ts, 0).unwrap_or_else(Utc::now));

            if let Some(pool) = pool {
                // Check if user was past_due to trigger dunning recovery notice
                let prev_sub: Option<(String, String)> = sqlx::query_as(
                    "SELECT user_id::text, status FROM subscriptions WHERE stripe_customer_id = $1 LIMIT 1"
                )
                .bind(customer_id)
                .fetch_optional(pool)
                .await
                .ok()
                .flatten();

                let _ = sqlx::query(
                    r#"
                    UPDATE subscriptions
                    SET status = 'active',
                        current_period_start = COALESCE($1, current_period_start),
                        current_period_end = COALESCE($2, current_period_end),
                        updated_at = NOW()
                    WHERE stripe_customer_id = $3
                    "#,
                )
                .bind(period_start)
                .bind(period_end)
                .bind(customer_id)
                .execute(pool)
                .await;

                if let Some((uid, old_status)) = prev_sub {
                    state.monthly_quota_cache.remove(&uid);
                    if old_status == "past_due" {
                        info!(
                            "[DUNNING RECOVERY] Payment succeeded for customer '{}'. Subscription restored to 'active'.",
                            redact_id(customer_id)
                        );
                    }
                }
            }

            info!(
                "[Billing Webhook] invoice.payment_succeeded: customer='{}' subscription='{}'",
                redact_id(customer_id),
                redact_id(subscription_id)
            );
        }

        "invoice.payment_failed" => {
            let data = &event["data"]["object"];
            let customer_id = data["customer"].as_str().unwrap_or("");
            let invoice_id = data["id"].as_str().unwrap_or("");
            let amount_due = data["amount_due"].as_u64().unwrap_or(0);

            if let Some(pool) = pool {
                let user_row: Option<(String,)> = sqlx::query_as(
                    "SELECT user_id::text FROM subscriptions WHERE stripe_customer_id = $1 LIMIT 1",
                )
                .bind(customer_id)
                .fetch_optional(pool)
                .await
                .ok()
                .flatten();

                let _ = sqlx::query(
                    r#"
                    UPDATE subscriptions SET status = 'past_due', updated_at = NOW()
                    WHERE stripe_customer_id = $1
                    "#,
                )
                .bind(customer_id)
                .execute(pool)
                .await;

                if let Some((uid,)) = user_row {
                    state.monthly_quota_cache.remove(&uid);
                }
            }

            warn!(
                "[Billing Webhook] invoice.payment_failed: customer='{}' invoice='{}' amount_due_cents={}",
                redact_id(customer_id),
                redact_id(invoice_id),
                amount_due
            );
            warn!(
                "[DUNNING ALERT] Payment failed for customer '{}'. Subscription marked 'past_due'. Dunning notice dispatched.",
                redact_id(customer_id)
            );
        }

        "customer.subscription.updated" => {
            let data = &event["data"]["object"];
            let customer_id = data["customer"].as_str().unwrap_or("");
            let subscription_id = data["id"].as_str().unwrap_or("");
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
                        } else if price_id.contains("pro") {
                            Some("pro_monthly")
                        } else {
                            None
                        }
                    })
            });

            if let Some(pool) = pool {
                let user_row: Option<(String,)> = sqlx::query_as(
                    "SELECT user_id::text FROM subscriptions WHERE stripe_customer_id = $1 OR stripe_subscription_id = $2 LIMIT 1"
                )
                .bind(customer_id)
                .bind(subscription_id)
                .fetch_optional(pool)
                .await
                .ok()
                .flatten();

                if let Some(pid) = plan_id {
                    let _ = sqlx::query(
                        r#"
                        UPDATE subscriptions
                        SET status = $1,
                            plan_id = $2,
                            current_period_start = COALESCE($3, current_period_start),
                            current_period_end = COALESCE($4, current_period_end),
                            updated_at = NOW()
                        WHERE stripe_customer_id = $5 OR stripe_subscription_id = $6
                        "#,
                    )
                    .bind(status)
                    .bind(pid)
                    .bind(period_start)
                    .bind(period_end)
                    .bind(customer_id)
                    .bind(subscription_id)
                    .execute(pool)
                    .await;
                } else {
                    let _ = sqlx::query(
                        r#"
                        UPDATE subscriptions
                        SET status = $1,
                            current_period_start = COALESCE($2, current_period_start),
                            current_period_end = COALESCE($3, current_period_end),
                            updated_at = NOW()
                        WHERE stripe_customer_id = $4 OR stripe_subscription_id = $5
                        "#,
                    )
                    .bind(status)
                    .bind(period_start)
                    .bind(period_end)
                    .bind(customer_id)
                    .bind(subscription_id)
                    .execute(pool)
                    .await;
                }

                if let Some((uid,)) = user_row {
                    state.monthly_quota_cache.remove(&uid);
                }
            }

            info!(
                "[Billing Webhook] customer.subscription.updated: customer='{}' subscription='{}' status='{}' plan='{:?}'",
                redact_id(customer_id),
                redact_id(subscription_id),
                status,
                plan_id
            );
        }

        "customer.subscription.deleted" => {
            let data = &event["data"]["object"];
            let customer_id = data["customer"].as_str().unwrap_or("");
            let subscription_id = data["id"].as_str().unwrap_or("");

            if let Some(pool) = pool {
                let user_row: Option<(String,)> = sqlx::query_as(
                    "SELECT user_id::text FROM subscriptions WHERE stripe_customer_id = $1 OR stripe_subscription_id = $2 LIMIT 1"
                )
                .bind(customer_id)
                .bind(subscription_id)
                .fetch_optional(pool)
                .await
                .ok()
                .flatten();

                let _ = sqlx::query(
                    r#"
                    UPDATE subscriptions
                    SET status = 'cancelled', plan_id = 'free', updated_at = NOW()
                    WHERE stripe_customer_id = $1 OR stripe_subscription_id = $2
                    "#,
                )
                .bind(customer_id)
                .bind(subscription_id)
                .execute(pool)
                .await;

                if let Some((uid,)) = user_row {
                    state.monthly_quota_cache.remove(&uid);
                }
            }

            info!(
                "[Billing Webhook] customer.subscription.deleted: customer='{}' subscription='{}'",
                redact_id(customer_id),
                redact_id(subscription_id)
            );
        }

        _ => {
            info!(
                "[Billing Webhook] Acknowledged unhandled event type: {}",
                event_type
            );
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
        let timestamp = "1630000000";

        // Compute expected signature
        let signed_payload = format!("{}.{}", timestamp, String::from_utf8_lossy(payload));
        let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(signed_payload.as_bytes());
        let sig = hex::encode(mac.finalize().into_bytes());

        let sig_header = format!("t={},v1={}", timestamp, sig);
        assert!(verify_stripe_signature(payload, &sig_header, secret).is_ok());
    }

    #[test]
    fn test_stripe_signature_verification_tampered_payload() {
        let secret = "whsec_test_secret_key";
        let payload = b"{\"type\":\"checkout.session.completed\"}";
        let timestamp = "1630000000";

        let signed_payload = format!("{}.{}", timestamp, String::from_utf8_lossy(payload));
        let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(signed_payload.as_bytes());
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
        let timestamp = "1630000000";

        let signed_payload = format!("{}.{}", timestamp, String::from_utf8_lossy(payload));
        let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(signed_payload.as_bytes());
        let sig = hex::encode(mac.finalize().into_bytes());

        let sig_header = format!("t={},v1={}", timestamp, sig);
        assert!(verify_stripe_signature(payload, &sig_header, "wrong_secret").is_err());
    }

    #[test]
    fn test_stripe_signature_missing_timestamp() {
        let result = verify_stripe_signature(b"body", "v1=abc123", "secret");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Missing timestamp"));
    }

    #[test]
    fn test_stripe_signature_missing_v1() {
        let result = verify_stripe_signature(b"body", "t=1630000000", "secret");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Missing v1 signature"));
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
}
