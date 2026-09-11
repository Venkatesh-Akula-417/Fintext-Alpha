//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Compliance Audit Log Route Handlers
//! ═══════════════════════════════════════════════════════════════════════════════

use axum::{
    extract::{Query, State},
    http::{header, HeaderValue, StatusCode},
    response::Response,
    Extension, Json,
};
use chrono::Utc;
use uuid::Uuid;

use crate::audit_logs::{AuditExportQuery, AuditLogRegistry, AuditLogsQuery, AuditLogsResponse};
use crate::auth::Claims;
use crate::state::AppState;

/// Handler for `GET /audit/logs` — Retrieves paginated compliance audit logs.
#[utoipa::path(
    get,
    path = "/audit/logs",
    tag = "Compliance & Audit",
    params(
        AuditLogsQuery
    ),
    responses(
        (status = 200, description = "Paginated compliance audit log entries", body = AuditLogsResponse),
        (status = 400, description = "Invalid query parameters or date format"),
        (status = 401, description = "Missing or invalid Bearer authentication token"),
        (status = 403, description = "Insufficient permissions to view target audit logs")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_audit_logs_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<AuditLogsQuery>,
) -> Result<Json<AuditLogsResponse>, (StatusCode, Json<serde_json::Value>)> {
    let (enforced_user, enforced_org) = resolve_rbac_filters(&state, &claims, query.org_id).await?;

    match state
        .audit_log_registry
        .query(&query, enforced_user, enforced_org)
    {
        Ok((logs, total)) => {
            let limit = query.limit.unwrap_or(100);
            let offset = query.offset.unwrap_or(0);
            Ok(Json(AuditLogsResponse {
                logs,
                total,
                limit,
                offset,
            }))
        }
        Err(err) => Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "Bad Request",
                "message": err
            })),
        )),
    }
}

/// Handler for `GET /audit/export` — Streams or downloads compliance audit logs in CSV or JSON format.
#[utoipa::path(
    get,
    path = "/audit/export",
    tag = "Compliance & Audit",
    params(
        AuditExportQuery
    ),
    responses(
        (status = 200, description = "Exported audit logs stream as CSV or JSON file", content_type = "text/csv"),
        (status = 400, description = "Invalid query parameters, format, or date window"),
        (status = 401, description = "Missing or invalid Bearer authentication token"),
        (status = 403, description = "Insufficient permissions to export target audit logs")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn export_audit_logs_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<AuditExportQuery>,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    let (enforced_user, enforced_org) = resolve_rbac_filters(&state, &claims, query.org_id).await?;

    let logs = state
        .audit_log_registry
        .query_for_export(&query, enforced_user, enforced_org)
        .map_err(|err| {
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "Bad Request",
                    "message": err
                })),
            )
        })?;

    let format_choice = query
        .format
        .as_deref()
        .unwrap_or("json")
        .trim()
        .to_lowercase();
    let timestamp_str = Utc::now().format("%Y%m%d_%H%M%S");

    match format_choice.as_str() {
        "csv" => {
            let csv_bytes = AuditLogRegistry::export_csv(&logs).map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({
                        "error": "Export Error",
                        "message": e
                    })),
                )
            })?;

            let mut response = Response::new(csv_bytes.into());
            *response.status_mut() = StatusCode::OK;
            response.headers_mut().insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("text/csv; charset=utf-8"),
            );
            response.headers_mut().insert(
                header::CONTENT_DISPOSITION,
                HeaderValue::from_str(&format!(
                    "attachment; filename=\"fintext_audit_logs_{}.csv\"",
                    timestamp_str
                ))
                .unwrap(),
            );

            Ok(response)
        }
        "json" => {
            let json_str = AuditLogRegistry::export_json(&logs).map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({
                        "error": "Export Error",
                        "message": e
                    })),
                )
            })?;

            let mut response = Response::new(json_str.into());
            *response.status_mut() = StatusCode::OK;
            response.headers_mut().insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/json; charset=utf-8"),
            );
            response.headers_mut().insert(
                header::CONTENT_DISPOSITION,
                HeaderValue::from_str(&format!(
                    "attachment; filename=\"fintext_audit_logs_{}.json\"",
                    timestamp_str
                ))
                .unwrap(),
            );

            Ok(response)
        }
        other => Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "Bad Request",
                "message": format!("Invalid export format '{}'. Allowed formats: 'csv', 'json'.", other)
            })),
        )),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// RBAC Resolution Helper
// ─────────────────────────────────────────────────────────────────────────────

async fn resolve_rbac_filters<'a>(
    state: &AppState,
    claims: &'a Claims,
    requested_org_id: Option<Uuid>,
) -> Result<(Option<&'a str>, Option<Uuid>), (StatusCode, Json<serde_json::Value>)> {
    // 1. Global Admin role can query across all users and organizations
    if claims.role == "admin" {
        return Ok((None, requested_org_id));
    }

    // 2. If an organization filter is requested, verify the user is an owner or admin of that org
    if let Some(org_id) = requested_org_id {
        if let Some(member) = state
            .org_registry
            .get_member(org_id, &claims.sub, state.db_pool.as_ref())
            .await
        {
            if member.role == "owner" || member.role == "admin" {
                return Ok((None, Some(org_id)));
            }
        }

        return Err((
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({
                "error": "Forbidden",
                "message": "Only organization administrators or owners can access organization-wide compliance audit logs."
            })),
        ));
    }

    // 3. Regular users without org admin filter are restricted to their own audit logs
    Ok((Some(&claims.sub), None))
}
