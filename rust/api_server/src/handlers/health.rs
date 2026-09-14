//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Health Check Handler
//! ═══════════════════════════════════════════════════════════════════════════════

use crate::models::HealthResponse;
use crate::state::AppState;
use crate::storage::{QuestDbClient, QuestDbClientConfig};
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Json;
use once_cell::sync::Lazy;
use serde_json::json;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::warn;

static QUESTDB_CLIENT: Lazy<QuestDbClient> =
    Lazy::new(|| QuestDbClient::new(QuestDbClientConfig::default()));

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

/// Kubernetes & Orchestrator Readiness Probe (/readyz).
///
/// Checks live connectivity to PostgreSQL, QuestDB hot time-series store,
/// and Kafka event bus. Fails closed with 503 if any configured dependency
/// is degraded or unreachable.
#[utoipa::path(
    get,
    path = "/readyz",
    tag = "System",
    responses(
        (status = 200, description = "System ready to serve traffic"),
        (status = 503, description = "System not ready / dependencies degraded")
    )
)]
pub async fn readyz_handler(
    State(state): State<AppState>,
) -> (StatusCode, Json<serde_json::Value>) {
    let mut degraded: Vec<String> = Vec::new();

    // 1. Check PostgreSQL (metadata DB pool, if present/configured)
    if let Some(pool) = state.db_pool.as_ref() {
        let probe = sqlx::query("SELECT 1").fetch_one(pool);
        match tokio::time::timeout(Duration::from_secs(2), probe).await {
            Ok(Ok(_)) => {}
            Ok(Err(e)) => degraded.push(format!("postgres: {}", e)),
            Err(_) => degraded.push("postgres: timeout".to_string()),
        }
    } else {
        warn!("[readyz] PostgreSQL pool is not configured; skipping probe");
    }

    // 2. Check QuestDB (hot time-series via existing client, if configured)
    if std::env::var("QUESTDB_URL").is_ok() {
        let probe = QUESTDB_CLIENT.exec_raw_query("SELECT 1");
        match tokio::time::timeout(Duration::from_secs(2), probe).await {
            Ok(Ok(_)) => {}
            Ok(Err(e)) => degraded.push(format!("questdb: {}", e)),
            Err(_) => degraded.push("questdb: timeout".to_string()),
        }
    } else {
        warn!("[readyz] QuestDB URL is not configured; skipping probe");
    }

    // 3. Check Kafka (event bus, if configured)
    if state.kafka_consumer.is_connected() {
        // Consumer actively receiving / connected to broker
    } else if std::env::var("KAFKA_BOOTSTRAP_SERVERS").is_ok()
        && !state.kafka_consumer.config().mock_mode
    {
        let brokers = state.kafka_consumer.config().bootstrap_servers.clone();
        let probe = tokio::task::spawn_blocking(move || {
            use rdkafka::consumer::Consumer;
            let consumer: Result<rdkafka::consumer::BaseConsumer, _> =
                rdkafka::config::ClientConfig::new()
                    .set("bootstrap.servers", &brokers)
                    .set("group.id", "fintext-readyz-probe")
                    .set("socket.timeout.ms", "2000")
                    .create();
            match consumer {
                Ok(c) => match c.fetch_metadata(None, Duration::from_secs(2)) {
                    Ok(_) => Ok(()),
                    Err(e) => Err(format!("{}", e)),
                },
                Err(e) => Err(format!("{}", e)),
            }
        });

        match tokio::time::timeout(Duration::from_secs(2), probe).await {
            Ok(Ok(Ok(_))) => {}
            Ok(Ok(Err(e))) => degraded.push(format!("kafka: {}", e)),
            Ok(Err(e)) => degraded.push(format!("kafka: probe join error: {}", e)),
            Err(_) => degraded.push("kafka: timeout".to_string()),
        }
    } else {
        warn!("[readyz] Kafka broker is not configured; skipping probe");
    }

    if degraded.is_empty() {
        (StatusCode::OK, Json(json!({"status": "ready"})))
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({
                "status": "not_ready",
                "degraded": degraded
            })),
        )
    }
}
