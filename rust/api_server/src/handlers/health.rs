//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Health Check Handler
//! ═══════════════════════════════════════════════════════════════════════════════

use crate::models::HealthResponse;
use axum::response::Json;
use std::time::{SystemTime, UNIX_EPOCH};

/// System Health and Liveness Probe.
///
/// Returns the operational status, version identifier, and microsecond-level system timestamp.
/// Used by Kubernetes/Docker orchestrators for liveness and readiness checks.
#[utoipa::path(
    get,
    path = "/health",
    tag = "System",
    responses(
        (status = 200, description = "System operational", body = HealthResponse)
    )
)]
pub async fn health_check_handler() -> Json<HealthResponse> {
    let now_us = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_micros() as u64)
        .unwrap_or(0);

    Json(HealthResponse {
        status: "ok".to_string(),
        version: "2.0.0-institutional".to_string(),
        timestamp_us: now_us,
    })
}
