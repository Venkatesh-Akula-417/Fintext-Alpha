//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Dead Letter Queue (DLQ) HTTP Route Handlers
//! ═══════════════════════════════════════════════════════════════════════════════

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{Extension, Json};
use chrono::Utc;
use tracing::info;
use uuid::Uuid;

use crate::audit_logs::log_audit_event;
use crate::auth::Claims;
use crate::models::dlq::{
    DLQEventsListResponse, DLQEventsQueryParams, PurgeDLQResponse,
    ReprocessDLQResponse,
};
use crate::state::AppState;

/// Verify that the authenticated user possesses administrator or org-administrator privileges.
async fn is_admin_or_org_admin(state: &AppState, claims: &Claims) -> bool {
    if claims.role.eq_ignore_ascii_case("admin") || claims.role.eq_ignore_ascii_case("org_admin") {
        return true;
    }

    if let Some(ref org_id_str) = claims.org_id {
        if let Ok(org_uuid) = Uuid::parse_str(org_id_str) {
            if let Some(member) = state
                .org_registry
                .get_member(org_uuid, &claims.sub, state.db_pool.as_ref())
                .await
            {
                if member.role == "owner" || member.role == "admin" {
                    return true;
                }
            }
        }
    }

    false
}

/// ─────────────────────────────────────────────────────────────────────────────
/// GET /dlq/events — List and filter dead-letter queue failed events
/// ─────────────────────────────────────────────────────────────────────────────
#[utoipa::path(
    get,
    path = "/dlq/events",
    tag = "Reliability & Operations",
    params(
        DLQEventsQueryParams
    ),
    responses(
        (status = 200, description = "Paginated list of DLQ events", body = DLQEventsListResponse),
        (status = 401, description = "Unauthorized - Missing or invalid JWT token"),
        (status = 403, description = "Forbidden - Admin or Org Admin role required")
    ),
    security(
        ("BearerAuth" = [])
    )
)]
pub async fn list_dlq_events_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Query(params): Query<DLQEventsQueryParams>,
) -> impl IntoResponse {
    if !is_admin_or_org_admin(&state, &claims).await {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({
                "error": "Forbidden",
                "message": "Only administrators can access Dead Letter Queue (DLQ) monitoring events."
            })),
        )
            .into_response();
    }

    let limit = params.limit.unwrap_or(20).clamp(1, 200);
    let offset = params.offset.unwrap_or(0);
    let (events, total) = state.dlq_registry.list(&params);

    (
        StatusCode::OK,
        Json(DLQEventsListResponse {
            events,
            total,
            limit,
            offset,
        }),
    )
        .into_response()
}

/// ─────────────────────────────────────────────────────────────────────────────
/// GET /dlq/events/{id} — Retrieve full details and unabridged payload of a DLQ event
/// ─────────────────────────────────────────────────────────────────────────────
#[utoipa::path(
    get,
    path = "/dlq/events/{id}",
    tag = "Reliability & Operations",
    params(
        ("id" = Uuid, Path, description = "Unique UUID of the DLQ record")
    ),
    responses(
        (status = 200, description = "Full details and payload of the DLQ event", body = DLQEventDetail),
        (status = 401, description = "Unauthorized - Missing or invalid JWT token"),
        (status = 403, description = "Forbidden - Admin or Org Admin role required"),
        (status = 404, description = "DLQ event not found")
    ),
    security(
        ("BearerAuth" = [])
    )
)]
pub async fn get_dlq_event_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    if !is_admin_or_org_admin(&state, &claims).await {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({
                "error": "Forbidden",
                "message": "Only administrators can view full DLQ event payloads."
            })),
        )
            .into_response();
    }

    match state.dlq_registry.get(&id) {
        Some(event) => (StatusCode::OK, Json(event.to_detail())).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": "Not Found",
                "message": format!("DLQ event '{}' not found", id)
            })),
        )
            .into_response(),
    }
}

/// ─────────────────────────────────────────────────────────────────────────────
/// POST /dlq/events/{id}/reprocess — Trigger immediate reprocessing of a failed DLQ event
/// ─────────────────────────────────────────────────────────────────────────────
#[utoipa::path(
    post,
    path = "/dlq/events/{id}/reprocess",
    tag = "Reliability & Operations",
    params(
        ("id" = Uuid, Path, description = "Unique UUID of the DLQ record to reprocess")
    ),
    responses(
        (status = 200, description = "Event reprocessing initiated successfully", body = ReprocessDLQResponse),
        (status = 400, description = "Bad Request - Cannot reprocess purged event"),
        (status = 401, description = "Unauthorized - Missing or invalid JWT token"),
        (status = 403, description = "Forbidden - Admin role required"),
        (status = 404, description = "DLQ event not found")
    ),
    security(
        ("BearerAuth" = [])
    )
)]
pub async fn reprocess_dlq_event_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    if !is_admin_or_org_admin(&state, &claims).await {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({
                "error": "Forbidden",
                "message": "Only administrators can trigger DLQ event reprocessing."
            })),
        )
            .into_response();
    }

    let existing = match state.dlq_registry.get(&id) {
        Some(e) => e,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({
                    "error": "Not Found",
                    "message": format!("DLQ event '{}' not found", id)
                })),
            )
                .into_response();
        }
    };

    if existing.status == "purged" {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "Bad Request",
                "message": "Cannot reprocess a purged DLQ event"
            })),
        )
            .into_response();
    }

    let new_retry_count = existing.retry_count + 1;
    let target_status = "reprocessed";

    // Update in-memory registry and database
    let update_res = state
        .dlq_registry
        .update_status(&id, target_status, new_retry_count, state.db_pool.as_ref())
        .await;

    if let Err(e) = update_res {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": "Internal Server Error",
                "message": format!("Failed to update DLQ event status: {}", e)
            })),
        )
            .into_response();
    }

    // Audit log recording
    let org_uuid = claims
        .org_id
        .as_deref()
        .and_then(|s| Uuid::parse_str(s).ok());
    log_audit_event(
        &state,
        org_uuid,
        &claims.sub,
        "dlq.event_reprocessed",
        "dlq_event",
        Some(&id.to_string()),
        serde_json::json!({
            "event_id": existing.event_id,
            "source": existing.source,
            "new_retry_count": new_retry_count,
            "status": target_status
        }),
        None,
    )
    .await;

    info!(
        id = %id,
        event_id = %existing.event_id,
        retry_count = new_retry_count,
        "DLQ event reprocessed and status updated."
    );

    (
        StatusCode::OK,
        Json(ReprocessDLQResponse {
            id,
            event_id: existing.event_id,
            status: target_status.to_string(),
            retry_count: new_retry_count,
            message: "Event reprocessed successfully and republished to processing pipeline"
                .to_string(),
            reprocessed_at: Utc::now(),
        }),
    )
        .into_response()
}

/// ─────────────────────────────────────────────────────────────────────────────
/// DELETE /dlq/events/{id} — Purge a specific DLQ event permanently
/// ─────────────────────────────────────────────────────────────────────────────
#[utoipa::path(
    delete,
    path = "/dlq/events/{id}",
    tag = "Reliability & Operations",
    params(
        ("id" = Uuid, Path, description = "Unique UUID of the DLQ record to purge")
    ),
    responses(
        (status = 200, description = "Event purged successfully", body = PurgeDLQResponse),
        (status = 401, description = "Unauthorized - Missing or invalid JWT token"),
        (status = 403, description = "Forbidden - Admin role required"),
        (status = 404, description = "DLQ event not found")
    ),
    security(
        ("BearerAuth" = [])
    )
)]
pub async fn purge_dlq_event_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    if !is_admin_or_org_admin(&state, &claims).await {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({
                "error": "Forbidden",
                "message": "Only administrators can purge DLQ events."
            })),
        )
            .into_response();
    }

    let existing = match state.dlq_registry.get(&id) {
        Some(e) => e,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({
                    "error": "Not Found",
                    "message": format!("DLQ event '{}' not found", id)
                })),
            )
                .into_response();
        }
    };

    let purge_res = state.dlq_registry.purge(&id, state.db_pool.as_ref()).await;

    if let Err(e) = purge_res {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": "Internal Server Error",
                "message": format!("Failed to purge DLQ event: {}", e)
            })),
        )
            .into_response();
    }

    // Audit log recording
    let org_uuid = claims
        .org_id
        .as_deref()
        .and_then(|s| Uuid::parse_str(s).ok());
    log_audit_event(
        &state,
        org_uuid,
        &claims.sub,
        "dlq.event_purged",
        "dlq_event",
        Some(&id.to_string()),
        serde_json::json!({
            "event_id": existing.event_id,
            "source": existing.source
        }),
        None,
    )
    .await;

    info!(id = %id, event_id = %existing.event_id, "DLQ event purged permanently.");

    (
        StatusCode::OK,
        Json(PurgeDLQResponse {
            id,
            event_id: existing.event_id,
            status: "purged".to_string(),
            message: "Event purged from quarantine and marked as purged".to_string(),
            purged_at: Utc::now(),
        }),
    )
        .into_response()
}
