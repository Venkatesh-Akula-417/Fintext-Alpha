//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Data Retention Policy Route Handlers
//! ═══════════════════════════════════════════════════════════════════════════════

use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use chrono::Utc;
use serde_json::json;
use tracing::info;
use uuid::Uuid;

use crate::audit_logs::log_audit_event;
use crate::auth::Claims;
use crate::models::{
    CreateRetentionPolicyRequest, DeleteRetentionPolicyResponse, RetentionPoliciesResponse,
};
use crate::retention::{
    is_valid_data_category, MAX_RETENTION_DAYS, MIN_RETENTION_DAYS, VALID_DATA_CATEGORIES,
};
use crate::state::AppState;

/// GET /retention/policies
///
/// List active data retention policies for the authenticated user and organization.
#[utoipa::path(
    get,
    path = "/retention/policies",
    tag = "Data Retention & Compliance",
    responses(
        (status = 200, description = "Active data retention policies listed successfully", body = RetentionPoliciesResponse),
        (status = 401, description = "Unauthorized - Missing or invalid Bearer JWT"),
    ),
    security(("BearerAuth" = []))
)]
pub async fn list_retention_policies_handler(
    Extension(claims): Extension<Claims>,
    State(state): State<AppState>,
) -> Response {
    let org_uuid = claims
        .org_id
        .as_deref()
        .and_then(|s| Uuid::parse_str(s).ok());
    let policies = state
        .retention_registry
        .list_policies(&claims.sub, org_uuid);

    Json(RetentionPoliciesResponse {
        total_policies: policies.len(),
        policies,
    })
    .into_response()
}

/// POST /retention/policies
///
/// Create or update a data retention policy for a specific data category.
#[utoipa::path(
    post,
    path = "/retention/policies",
    tag = "Data Retention & Compliance",
    request_body = CreateRetentionPolicyRequest,
    responses(
        (status = 200, description = "Retention policy created or updated successfully", body = RetentionPolicy),
        (status = 400, description = "Invalid data category or retention_days out of range (1-3650)"),
        (status = 401, description = "Unauthorized - Missing or invalid Bearer JWT"),
    ),
    security(("BearerAuth" = []))
)]
pub async fn create_retention_policy_handler(
    Extension(claims): Extension<Claims>,
    State(state): State<AppState>,
    Json(payload): Json<CreateRetentionPolicyRequest>,
) -> Response {
    let clean_category = payload.data_category.trim().to_lowercase();
    if !is_valid_data_category(&clean_category) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Bad Request",
                "message": format!(
                    "Invalid data_category '{}'. Allowed: {:?}",
                    payload.data_category, VALID_DATA_CATEGORIES
                )
            })),
        )
            .into_response();
    }

    if payload.retention_days < MIN_RETENTION_DAYS || payload.retention_days > MAX_RETENTION_DAYS {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Bad Request",
                "message": format!(
                    "retention_days must be between {} and {} (got {})",
                    MIN_RETENTION_DAYS, MAX_RETENTION_DAYS, payload.retention_days
                )
            })),
        )
            .into_response();
    }

    let is_active = payload.is_active.unwrap_or(true);

    // If org_id is present in token claims and role is admin, assign to org scope
    let org_uuid = claims
        .org_id
        .as_deref()
        .and_then(|s| Uuid::parse_str(s).ok());
    let target_org_id = if claims.role == "admin" || claims.role == "org_admin" {
        org_uuid
    } else {
        None
    };

    match state
        .retention_registry
        .upsert_policy(
            &claims.sub,
            target_org_id,
            &clean_category,
            payload.retention_days,
            is_active,
            state.db_pool.as_ref(),
        )
        .await
    {
        Ok(policy) => {
            // Log audit event
            log_audit_event(
                &state,
                policy.org_id,
                &claims.sub,
                "retention.policy_updated",
                "data_retention_policy",
                Some(&policy.id.to_string()),
                json!({
                    "data_category": policy.data_category,
                    "retention_days": policy.retention_days,
                    "is_active": policy.is_active,
                    "org_id": policy.org_id,
                }),
                None,
            )
            .await;

            info!(
                "[Data Retention] Policy updated by user='{}' for category='{}'",
                claims.sub, policy.data_category
            );
            (StatusCode::OK, Json(policy)).into_response()
        }
        Err(err) => (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Bad Request",
                "message": err
            })),
        )
            .into_response(),
    }
}

/// DELETE /retention/policies/{id}
///
/// Deactivate or delete a data retention policy.
#[utoipa::path(
    delete,
    path = "/retention/policies/{id}",
    tag = "Data Retention & Compliance",
    params(
        ("id" = Uuid, Path, description = "Retention policy UUID to delete")
    ),
    responses(
        (status = 200, description = "Retention policy deleted successfully", body = DeleteRetentionPolicyResponse),
        (status = 401, description = "Unauthorized - Missing or invalid Bearer JWT"),
        (status = 404, description = "Retention policy not found or unauthorized"),
    ),
    security(("BearerAuth" = []))
)]
pub async fn delete_retention_policy_handler(
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
    State(state): State<AppState>,
) -> Response {
    let org_uuid = claims
        .org_id
        .as_deref()
        .and_then(|s| Uuid::parse_str(s).ok());
    match state
        .retention_registry
        .delete_policy(id, &claims.sub, org_uuid, state.db_pool.as_ref())
        .await
    {
        Ok(Some(policy)) => {
            // Log audit event
            log_audit_event(
                &state,
                policy.org_id,
                &claims.sub,
                "retention.policy_deleted",
                "data_retention_policy",
                Some(&policy.id.to_string()),
                json!({
                    "data_category": policy.data_category,
                    "retention_days": policy.retention_days,
                    "org_id": policy.org_id,
                }),
                None,
            )
            .await;

            (
                StatusCode::OK,
                Json(DeleteRetentionPolicyResponse {
                    status: "deleted".to_string(),
                    message: "Retention policy deleted successfully".to_string(),
                    id,
                    deleted_at: Utc::now(),
                }),
            )
                .into_response()
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": "Not Found",
                "message": format!("Retention policy '{}' not found", id)
            })),
        )
            .into_response(),
        Err(_) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": "Not Found",
                "message": format!("Retention policy '{}' not found or access denied", id)
            })),
        )
            .into_response(),
    }
}
