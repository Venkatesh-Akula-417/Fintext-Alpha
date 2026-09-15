//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Manual Anomaly Scan & Push Trigger Handler
//! ═══════════════════════════════════════════════════════════════════════════════

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use chrono::Utc;
use tracing::info;

use crate::anomaly_worker::scan_and_broadcast_anomalies;
use crate::models::AnomalyScanResponse;
use crate::state::AppState;

/// Trigger an immediate statistical sentiment anomaly scan and push alerts to WebSocket subscribers.
#[utoipa::path(
    post,
    path = "/anomaly-scan",
    tag = "Sentiment Analysis",
    responses(
        (status = 200, description = "Sentiment anomaly scan completed and broadcast", body = AnomalyScanResponse),
        (status = 401, description = "Unauthorized (missing or invalid Bearer token)", body = crate::auth::AuthErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::rate_limit::RateLimitErrorResponse)
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn post_anomaly_scan_handler(State(state): State<AppState>) -> Response {
    let now = Utc::now().to_rfc3339();
    let anomalies = scan_and_broadcast_anomalies(&state).await;
    let anomalies_found = anomalies.len();
    let alerts_broadcasted = state.anomaly_broadcaster.receiver_count() * anomalies_found;

    info!(
        "[Anomaly Scan Handler] Completed manual anomaly scan: {} anomalies found, broadcast to {} active subscribers",
        anomalies_found, state.anomaly_broadcaster.receiver_count()
    );

    let response = AnomalyScanResponse {
        anomalies_found,
        alerts_broadcasted,
        anomalies,
        scanned_at: now,
        message: format!(
            "Anomaly scan completed: {} anomalies detected and broadcast to {} active WebSocket receivers",
            anomalies_found, state.anomaly_broadcaster.receiver_count()
        ),
    };

    (StatusCode::OK, Json(response)).into_response()
}
