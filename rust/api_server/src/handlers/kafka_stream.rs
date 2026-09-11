//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Streaming Kafka Topic Access Route Handlers
//! ═══════════════════════════════════════════════════════════════════════════════

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use axum::Extension;
use chrono::Utc;
use serde_json::json;
use std::env;
use tracing::{error, info};
use uuid::Uuid;

use crate::audit_logs::log_audit_event;
use crate::auth::{AuthErrorResponse, Claims};
use crate::kafka_stream::{
    get_available_kafka_topics, is_valid_kafka_topic, DEFAULT_KAFKA_BOOTSTRAP_SERVERS,
    DEFAULT_KAFKA_TTL_MINUTES, MAX_KAFKA_TTL_MINUTES, MIN_KAFKA_TTL_MINUTES,
};
use crate::models::{
    GetKafkaCredentialsQuery, KafkaTopicsResponse, RevokeKafkaCredentialsResponse,
};
use crate::state::AppState;

/// Verify that the caller's plan tier is eligible for real-time Kafka streaming.
async fn verify_streaming_plan_eligibility(
    state: &AppState,
    claims: &Claims,
) -> Result<(), (StatusCode, Json<AuthErrorResponse>)> {
    // 1. Role-based bypass for institutional and admin accounts
    if claims.role.eq_ignore_ascii_case("admin")
        || claims.role.eq_ignore_ascii_case("institutional")
    {
        return Ok(());
    }

    // 2. Explicit free/retail role rejection
    if claims.role.eq_ignore_ascii_case("free") || claims.role.eq_ignore_ascii_case("retail") {
        return Err((
            StatusCode::FORBIDDEN,
            Json(AuthErrorResponse {
                error: "Forbidden".to_string(),
                message: "Streaming access requires Pro or Enterprise plan".to_string(),
            }),
        ));
    }

    // 3. Database subscription plan check
    if let Some(pool) = &state.db_pool {
        if let Ok(Some(sub)) = crate::billing::get_user_subscription(pool, &claims.sub).await {
            if sub.plan_id == "free" || sub.status != "active" {
                return Err((
                    StatusCode::FORBIDDEN,
                    Json(AuthErrorResponse {
                        error: "Forbidden".to_string(),
                        message: "Streaming access requires Pro or Enterprise plan".to_string(),
                    }),
                ));
            }
        }
    }

    Ok(())
}

/// List all available Kafka streaming topics with schemas and descriptions (`GET /stream/kafka/topics`).
#[utoipa::path(
    get,
    path = "/stream/kafka/topics",
    tag = "Streaming & Kafka",
    responses(
        (status = 200, description = "List of available Kafka streaming topics", body = KafkaTopicsResponse),
        (status = 401, description = "Unauthorized", body = AuthErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::rate_limit::RateLimitErrorResponse)
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn list_kafka_topics_handler(
    State(_state): State<AppState>,
    Extension(_claims): Extension<Claims>,
) -> Response {
    let topics = get_available_kafka_topics();
    let total_topics = topics.len();
    (
        StatusCode::OK,
        Json(KafkaTopicsResponse {
            topics,
            total_topics,
        }),
    )
        .into_response()
}

/// Generate short-lived Kafka consumer credentials for a specific topic (`GET /stream/kafka/credentials`).
#[utoipa::path(
    get,
    path = "/stream/kafka/credentials",
    tag = "Streaming & Kafka",
    params(GetKafkaCredentialsQuery),
    responses(
        (status = 200, description = "Temporary Kafka consumer credentials issued", body = KafkaCredentials),
        (status = 400, description = "Invalid topic or TTL parameters", body = AuthErrorResponse),
        (status = 401, description = "Unauthorized", body = AuthErrorResponse),
        (status = 403, description = "Streaming access requires Pro or Enterprise plan", body = AuthErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::rate_limit::RateLimitErrorResponse)
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn get_kafka_credentials_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<GetKafkaCredentialsQuery>,
) -> Response {
    // 1. Check plan authorization
    if let Err(forbidden_resp) = verify_streaming_plan_eligibility(&state, &claims).await {
        return forbidden_resp.into_response();
    }

    // 2. Validate topic
    let topic_clean = query.topic.trim().to_lowercase();
    if topic_clean.is_empty() || !is_valid_kafka_topic(&topic_clean) {
        return (
            StatusCode::BAD_REQUEST,
            Json(AuthErrorResponse {
                error: "Bad Request".to_string(),
                message: format!(
                    "Invalid or unsupported topic '{}'. Available topics: {:?}",
                    query.topic,
                    get_available_kafka_topics()
                        .iter()
                        .map(|t| &t.topic)
                        .collect::<Vec<_>>()
                ),
            }),
        )
            .into_response();
    }

    // 3. Validate TTL
    let ttl = query.ttl_minutes.unwrap_or(DEFAULT_KAFKA_TTL_MINUTES);
    if ttl < MIN_KAFKA_TTL_MINUTES || ttl > MAX_KAFKA_TTL_MINUTES {
        return (
            StatusCode::BAD_REQUEST,
            Json(AuthErrorResponse {
                error: "Bad Request".to_string(),
                message: format!(
                    "Field 'ttl_minutes' must be between {} and {} (got {})",
                    MIN_KAFKA_TTL_MINUTES, MAX_KAFKA_TTL_MINUTES, ttl
                ),
            }),
        )
            .into_response();
    }

    // 4. Validate custom consumer group if provided
    if let Some(ref cg) = query.consumer_group {
        let clean_cg = cg.trim();
        if clean_cg.is_empty()
            || clean_cg.len() > 128
            || !clean_cg
                .chars()
                .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.')
        {
            return (
                StatusCode::BAD_REQUEST,
                Json(AuthErrorResponse {
                    error: "Bad Request".to_string(),
                    message: format!("Invalid consumer_group '{}'. Must be 1-128 alphanumeric characters, dots, dashes, or underscores.", cg),
                }),
            )
                .into_response();
        }
    }

    // 5. Resolve broker bootstrap address
    let broker_address = env::var("KAFKA_BOOTSTRAP_SERVERS")
        .unwrap_or_else(|_| DEFAULT_KAFKA_BOOTSTRAP_SERVERS.to_string());

    // 6. Issue credentials
    let creds = match state
        .kafka_credentials_registry
        .issue_credentials(
            &claims.sub,
            &topic_clean,
            ttl,
            query.consumer_group.map(|s| s.trim().to_string()),
            &broker_address,
            state.db_pool.as_ref(),
        )
        .await
    {
        Ok(c) => c,
        Err(e) => {
            error!("[Kafka Streaming] Failed to issue credentials: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AuthErrorResponse {
                    error: "Credential Issuance Error".to_string(),
                    message: "Failed to generate Kafka credentials".to_string(),
                }),
            )
                .into_response();
        }
    };

    // 7. Record compliance audit log
    let org_uuid = claims
        .org_id
        .as_deref()
        .and_then(|id| Uuid::parse_str(id).ok());
    log_audit_event(
        &state,
        org_uuid,
        &claims.sub,
        "kafka.credentials_issued",
        "kafka_credentials",
        Some(&creds.id.to_string()),
        json!({
            "topic": creds.topic,
            "consumer_group": creds.consumer_group,
            "username": creds.username,
            "broker_address": creds.broker_address,
            "ttl_minutes": ttl,
            "expires_at": creds.expires_at,
        }),
        None,
    )
    .await;

    info!(
        "[Kafka Streaming] Issued credentials id='{}' for user='{}' topic='{}'",
        creds.id, claims.sub, creds.topic
    );

    (StatusCode::OK, Json(creds)).into_response()
}

/// Revoke Kafka consumer credentials early (`DELETE /stream/kafka/credentials/{id}`).
#[utoipa::path(
    delete,
    path = "/stream/kafka/credentials/{id}",
    tag = "Streaming & Kafka",
    params(
        ("id" = Uuid, Path, description = "Kafka credential UUID to revoke")
    ),
    responses(
        (status = 200, description = "Kafka credentials revoked successfully", body = RevokeKafkaCredentialsResponse),
        (status = 401, description = "Unauthorized", body = AuthErrorResponse),
        (status = 404, description = "Credentials not found or owned by another user", body = AuthErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::rate_limit::RateLimitErrorResponse)
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn revoke_kafka_credentials_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
) -> Response {
    let result = state
        .kafka_credentials_registry
        .revoke_credentials(id, &claims.sub, state.db_pool.as_ref())
        .await;

    match result {
        Ok(Some(stored)) => {
            let org_uuid = claims
                .org_id
                .as_deref()
                .and_then(|i| Uuid::parse_str(i).ok());
            log_audit_event(
                &state,
                org_uuid,
                &claims.sub,
                "kafka.credentials_revoked",
                "kafka_credentials",
                Some(&id.to_string()),
                json!({
                    "topic": stored.topic,
                    "consumer_group": stored.consumer_group,
                    "username": stored.username,
                    "revoked_at": stored.revoked_at,
                }),
                None,
            )
            .await;

            info!(
                "[Kafka Streaming] User '{}' revoked Kafka credentials id='{}'",
                claims.sub, id
            );

            (
                StatusCode::OK,
                Json(RevokeKafkaCredentialsResponse {
                    status: "revoked".to_string(),
                    message: "Kafka credentials revoked successfully".to_string(),
                    id,
                    revoked_at: stored.revoked_at.unwrap_or_else(Utc::now),
                }),
            )
                .into_response()
        }
        Ok(None) | Err(_) => (
            StatusCode::NOT_FOUND,
            Json(AuthErrorResponse {
                error: "Not Found".to_string(),
                message: format!(
                    "Kafka credentials '{}' not found or owned by another user",
                    id
                ),
            }),
        )
            .into_response(),
    }
}
