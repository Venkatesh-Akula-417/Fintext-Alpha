//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Sandbox Environment API Handlers
//! ═══════════════════════════════════════════════════════════════════════════════

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Json;
use axum::Extension;
use tracing::info;

use crate::auth::{AuthErrorResponse, Claims};
use crate::models::sandbox::SandboxStatusResponse;
use crate::sandbox::SandboxRegistry;
use crate::state::AppState;

/// Activate Sandbox Environment Mode.
///
/// Enables isolated mock simulation for the authenticated user. All subsequent API calls
/// will be served using synthetic mock data models, bypassing production billing and usage limits.
#[utoipa::path(
    post,
    path = "/sandbox/activate",
    tag = "Sandbox & Testing",
    responses(
        (status = 200, description = "Sandbox mode successfully activated", body = SandboxStatusResponse),
        (status = 401, description = "Unauthorized (missing or invalid Bearer token)", body = AuthErrorResponse)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn activate_sandbox_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<SandboxStatusResponse>, (StatusCode, Json<AuthErrorResponse>)> {
    let user_id = claims.sub.trim();
    let info = state.sandbox_registry.activate(user_id);
    info!(
        "[Sandbox Handler] Activated sandbox mode for user '{}'",
        user_id
    );

    let response = SandboxRegistry::to_response(&info);
    Ok(Json(response))
}

/// Deactivate Sandbox Environment Mode.
///
/// Disables sandbox mode, routing future API requests back to production live data pipelines.
#[utoipa::path(
    post,
    path = "/sandbox/deactivate",
    tag = "Sandbox & Testing",
    responses(
        (status = 200, description = "Sandbox mode successfully deactivated", body = SandboxStatusResponse),
        (status = 401, description = "Unauthorized (missing or invalid Bearer token)", body = AuthErrorResponse)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn deactivate_sandbox_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<SandboxStatusResponse>, (StatusCode, Json<AuthErrorResponse>)> {
    let user_id = claims.sub.trim();
    let info = state.sandbox_registry.deactivate(user_id);
    info!(
        "[Sandbox Handler] Deactivated sandbox mode for user '{}'",
        user_id
    );

    let response = SandboxRegistry::to_response(&info);
    Ok(Json(response))
}

/// Get Sandbox Environment Status.
///
/// Retrieves current sandbox activation status, mock version, and list of supported sandbox endpoints.
#[utoipa::path(
    get,
    path = "/sandbox/status",
    tag = "Sandbox & Testing",
    responses(
        (status = 200, description = "Sandbox status retrieved successfully", body = SandboxStatusResponse),
        (status = 401, description = "Unauthorized (missing or invalid Bearer token)", body = AuthErrorResponse)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_sandbox_status_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<SandboxStatusResponse>, (StatusCode, Json<AuthErrorResponse>)> {
    let user_id = claims.sub.trim();
    let mut info = state.sandbox_registry.get_status(user_id);

    // If JWT token is explicitly scoped with sandbox: true claim, reflect active status
    if claims.sandbox == Some(true) && !info.active {
        info.active = true;
    }

    let response = SandboxRegistry::to_response(&info);
    Ok(Json(response))
}
