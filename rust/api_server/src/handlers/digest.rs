//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Email Digest Route Handlers
//! ═══════════════════════════════════════════════════════════════════════════════

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use axum::Extension;
use chrono::Utc;
use serde_json::json;
use tracing::{error, info};
use uuid::Uuid;

use crate::audit_logs::log_audit_event;
use crate::auth::{AuthErrorResponse, Claims};
use crate::digest::{compose_digest_email, validate_digest_request, DigestSubscriptionRegistry};
use crate::models::{
    CreateDigestRequest, DeleteDigestResponse, DigestSubscription, DigestSubscriptionResponse,
    TriggerDigestRequest, TriggerDigestResponse,
};
use crate::state::AppState;

/// Retrieve the authenticated user's current email digest subscription (`GET /digest/subscription`).
#[utoipa::path(
    get,
    path = "/digest/subscription",
    tag = "Email Digest",
    responses(
        (status = 200, description = "Email digest subscription retrieved successfully", body = DigestSubscriptionResponse),
        (status = 401, description = "Unauthorized (missing or invalid Bearer JWT)", body = AuthErrorResponse),
        (status = 404, description = "No email digest subscription configured for user", body = AuthErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::rate_limit::RateLimitErrorResponse)
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn get_digest_subscription_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Response {
    match state.digest_registry.get_by_user(&claims.sub) {
        Some(sub) => (
            StatusCode::OK,
            Json(DigestSubscriptionResponse {
                status: "ok".to_string(),
                message: "Email digest subscription retrieved successfully".to_string(),
                subscription: sub,
            }),
        )
            .into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(AuthErrorResponse {
                error: "Not Found".to_string(),
                message: format!(
                    "No email digest subscription configured for user '{}'",
                    claims.sub
                ),
            }),
        )
            .into_response(),
    }
}

/// Create or update an email digest subscription (`POST /digest/subscription`).
#[utoipa::path(
    post,
    path = "/digest/subscription",
    tag = "Email Digest",
    request_body = CreateDigestRequest,
    responses(
        (status = 200, description = "Email digest subscription created or updated successfully", body = DigestSubscriptionResponse),
        (status = 400, description = "Invalid frequency, event types, or ticker symbols", body = AuthErrorResponse),
        (status = 401, description = "Unauthorized (missing or invalid Bearer JWT)", body = AuthErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::rate_limit::RateLimitErrorResponse)
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn create_digest_subscription_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(payload): Json<CreateDigestRequest>,
) -> Response {
    // 1. Validate request payload
    if let Err(err_msg) = validate_digest_request(&payload) {
        return (
            StatusCode::BAD_REQUEST,
            Json(AuthErrorResponse {
                error: "Bad Request".to_string(),
                message: err_msg,
            }),
        )
            .into_response();
    }

    let now = Utc::now();
    let existing = state.digest_registry.get_by_user(&claims.sub);
    let (id, created_at, action) = match existing {
        Some(prev) => (prev.id, prev.created_at, "digest.subscription_updated"),
        None => (Uuid::new_v4(), now, "digest.subscription_created"),
    };

    let sub = DigestSubscription {
        id,
        user_id: claims.sub.clone(),
        frequency: payload.frequency.trim().to_lowercase(),
        tickers: payload
            .tickers
            .into_iter()
            .map(|t| t.trim().to_uppercase())
            .collect(),
        sectors: payload
            .sectors
            .into_iter()
            .map(|s| s.trim().to_string())
            .collect(),
        event_types: payload
            .event_types
            .into_iter()
            .map(|e| e.trim().to_lowercase())
            .collect(),
        is_active: payload.is_active,
        created_at,
        updated_at: now,
    };

    // 2. Persist to in-memory cache and DB
    if let Err(e) = state
        .digest_registry
        .upsert(sub.clone(), state.db_pool.as_ref())
        .await
    {
        error!("[Email Digest] Failed to persist subscription: {}", e);
    }

    // 3. Record compliance audit log
    let org_uuid = claims
        .org_id
        .as_deref()
        .and_then(|id| Uuid::parse_str(id).ok());
    log_audit_event(
        &state,
        org_uuid,
        &claims.sub,
        action,
        "digest_subscription",
        Some(&id.to_string()),
        json!({
            "frequency": sub.frequency,
            "tickers": sub.tickers,
            "sectors": sub.sectors,
            "event_types": sub.event_types,
            "is_active": sub.is_active,
        }),
        None,
    )
    .await;

    info!(
        "[Email Digest] User '{}' {} digest subscription (freq: '{}', tickers: {:?})",
        claims.sub,
        if action == "digest.subscription_created" {
            "created"
        } else {
            "updated"
        },
        sub.frequency,
        sub.tickers
    );

    (
        StatusCode::OK,
        Json(DigestSubscriptionResponse {
            status: "ok".to_string(),
            message: format!(
                "Email digest subscription {} successfully",
                if action == "digest.subscription_created" {
                    "created"
                } else {
                    "updated"
                }
            ),
            subscription: sub,
        }),
    )
        .into_response()
}

/// Delete or deactivate an email digest subscription (`DELETE /digest/subscription`).
#[utoipa::path(
    delete,
    path = "/digest/subscription",
    tag = "Email Digest",
    responses(
        (status = 200, description = "Email digest subscription deleted successfully", body = DeleteDigestResponse),
        (status = 401, description = "Unauthorized (missing or invalid Bearer JWT)", body = AuthErrorResponse),
        (status = 404, description = "No email digest subscription found to delete", body = AuthErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::rate_limit::RateLimitErrorResponse)
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn delete_digest_subscription_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Response {
    let removed = state
        .digest_registry
        .delete_by_user(&claims.sub, state.db_pool.as_ref())
        .await;

    match removed {
        Ok(Some(sub)) => {
            let org_uuid = claims
                .org_id
                .as_deref()
                .and_then(|id| Uuid::parse_str(id).ok());
            log_audit_event(
                &state,
                org_uuid,
                &claims.sub,
                "digest.subscription_deleted",
                "digest_subscription",
                Some(&sub.id.to_string()),
                json!({ "user_id": claims.sub }),
                None,
            )
            .await;

            info!(
                "[Email Digest] User '{}' deleted digest subscription",
                claims.sub
            );

            (
                StatusCode::OK,
                Json(DeleteDigestResponse {
                    status: "deleted".to_string(),
                    message: "Email digest subscription deleted successfully".to_string(),
                }),
            )
                .into_response()
        }
        _ => (
            StatusCode::NOT_FOUND,
            Json(AuthErrorResponse {
                error: "Not Found".to_string(),
                message: format!(
                    "No email digest subscription found to delete for user '{}'",
                    claims.sub
                ),
            }),
        )
            .into_response(),
    }
}

/// Trigger on-demand email digest compilation and delivery (`POST /digest/trigger`).
#[utoipa::path(
    post,
    path = "/digest/trigger",
    tag = "Email Digest",
    request_body = Option<TriggerDigestRequest>,
    responses(
        (status = 200, description = "Email digest compiled and delivered successfully", body = TriggerDigestResponse),
        (status = 401, description = "Unauthorized (missing or invalid Bearer JWT)", body = AuthErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::rate_limit::RateLimitErrorResponse)
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn trigger_digest_send_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    payload: Option<Json<TriggerDigestRequest>>,
) -> Response {
    let req = payload.map(|Json(p)| p).unwrap_or_default();

    // 1. Resolve subscription (or default configuration if none stored)
    let sub = state
        .digest_registry
        .get_by_user(&claims.sub)
        .unwrap_or_else(|| DigestSubscription {
            id: Uuid::new_v4(),
            user_id: claims.sub.clone(),
            frequency: "daily".to_string(),
            tickers: vec!["AAPL".to_string(), "NVDA".to_string(), "MSFT".to_string()],
            sectors: vec!["Technology".to_string()],
            event_types: vec![
                "earnings".to_string(),
                "insider".to_string(),
                "8k".to_string(),
                "news".to_string(),
            ],
            is_active: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        });

    let recipient = req
        .recipient_email
        .unwrap_or_else(|| format!("{}@institutional-fund.com", claims.sub));

    // 2. Compose digest email
    let (subject, body_html, body_text, counts) =
        compose_digest_email(&sub, &recipient, &state.news_article_registry);

    // 3. Dispatch via configured email sender
    if let Err(e) = state
        .email_sender
        .send_email(&recipient, &subject, &body_html, &body_text)
        .await
    {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(AuthErrorResponse {
                error: "Email Delivery Error".to_string(),
                message: format!("Failed to dispatch email digest: {}", e),
            }),
        )
            .into_response();
    }

    // 4. Record history & audit
    DigestSubscriptionRegistry::record_send_history(
        state.db_pool.as_ref(),
        sub.id,
        &claims.sub,
        &recipient,
        &subject,
        "Manual on-demand digest trigger",
    )
    .await;

    let org_uuid = claims
        .org_id
        .as_deref()
        .and_then(|id| Uuid::parse_str(id).ok());
    log_audit_event(
        &state,
        org_uuid,
        &claims.sub,
        "digest.triggered",
        "digest_subscription",
        Some(&sub.id.to_string()),
        json!({
            "recipient": recipient,
            "subject": subject,
            "sentiment_count": counts.sentiment_count,
            "events_count": counts.events_count,
            "news_count": counts.news_count,
        }),
        None,
    )
    .await;

    info!(
        "[Email Digest] User '{}' manually triggered digest to '{}' (Sent {} items)",
        claims.sub,
        recipient,
        counts.sentiment_count + counts.events_count + counts.news_count
    );

    (
        StatusCode::OK,
        Json(TriggerDigestResponse {
            status: "sent".to_string(),
            recipient,
            subject,
            preview_html: body_html,
            preview_text: body_text,
            item_counts: counts,
            sent_at: Utc::now(),
        }),
    )
        .into_response()
}
