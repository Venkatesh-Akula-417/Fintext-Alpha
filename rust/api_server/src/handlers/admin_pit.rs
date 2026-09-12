use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    Json,
};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};
use utoipa::ToSchema;

use crate::auth::AuthErrorResponse;
use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ReloadPitDataResponse {
    pub status: String,
    pub message: String,
    pub ticker_intervals_count: usize,
    pub delisted_tickers_count: usize,
    pub corporate_actions_count: usize,
    pub timestamp_utc: String,
}

#[derive(Debug, Clone, Deserialize, ToSchema, Default)]
pub struct ReloadPitDataRequest {
    /// Optional directory path containing updated JSON files. If None, uses default/env paths.
    #[serde(default)]
    pub config_dir: Option<String>,
}

/// Reload Point-in-Time Corporate Actions and Ticker History Cache.
///
/// Reloads the in-memory Point-in-Time data snapshot from configuration files on disk.
/// Protected by the `X-Admin-Token` header or valid Admin token.
#[utoipa::path(
    post,
    path = "/admin/reload-pit-data",
    tag = "Admin",
    params(
        ("x-admin-token" = Option<String>, Header, description = "Administrative secret token authorizing PIT data cache reload")
    ),
    request_body = Option<ReloadPitDataRequest>,
    responses(
        (status = 200, description = "PIT data cache successfully reloaded", body = ReloadPitDataResponse),
        (status = 401, description = "Unauthorized (invalid or missing X-Admin-Token)", body = AuthErrorResponse),
        (status = 500, description = "Internal server error reloading PIT data", body = AuthErrorResponse)
    )
)]
pub async fn reload_pit_data_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    payload: Option<Json<ReloadPitDataRequest>>,
) -> Result<Json<ReloadPitDataResponse>, (StatusCode, Json<AuthErrorResponse>)> {
    // 1. Validate administrative authorization
    let is_authorized = if state.admin_token.trim().is_empty() {
        true
    } else {
        let admin_header = headers
            .get("x-admin-token")
            .and_then(|h| h.to_str().ok())
            .unwrap_or("")
            .trim();

        let bearer_header = headers
            .get("authorization")
            .and_then(|h| h.to_str().ok())
            .and_then(|h| {
                h.strip_prefix("Bearer ")
                    .or_else(|| h.strip_prefix("bearer "))
            })
            .unwrap_or("")
            .trim();

        admin_header == state.admin_token.trim() || bearer_header == state.admin_token.trim()
    };

    if !is_authorized {
        warn!("[Admin PIT] Unauthorized reload attempt");
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(AuthErrorResponse {
                error: "Unauthorized".to_string(),
                message: "Invalid or missing X-Admin-Token header".to_string(),
            }),
        ));
    }

    // 2. Perform reload
    let config_dir_opt = payload.as_ref().and_then(|p| p.config_dir.as_deref());
    let reload_res = if let Some(dir_str) = config_dir_opt {
        let path = std::path::Path::new(dir_str);
        state.pit_data.reload(path)
    } else {
        state.pit_data.reload_from_env()
    };

    match reload_res {
        Ok(()) => {
            let snap = state.pit_data.snapshot();
            info!(
                "[Admin PIT] Reload succeeded: intervals={}, delisted={}, corporate_actions={}",
                snap.ticker_intervals.len(),
                snap.delisted_tickers.len(),
                snap.corporate_actions.len()
            );
            Ok(Json(ReloadPitDataResponse {
                status: "success".to_string(),
                message: "Point-in-Time cache reloaded successfully".to_string(),
                ticker_intervals_count: snap.ticker_intervals.len(),
                delisted_tickers_count: snap.delisted_tickers.len(),
                corporate_actions_count: snap.corporate_actions.len(),
                timestamp_utc: chrono::Utc::now().to_rfc3339(),
            }))
        }
        Err(err) => {
            warn!("[Admin PIT] Reload failed: {}", err);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AuthErrorResponse {
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to reload PIT data: {}", err),
                }),
            ))
        }
    }
}
