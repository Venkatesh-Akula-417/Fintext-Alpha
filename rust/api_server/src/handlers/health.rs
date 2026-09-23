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

/// Administrative Backup & Disaster Recovery Telemetry Probe.
#[utoipa::path(
    get,
    path = "/admin/backup/status",
    tag = "Admin",
    responses(
        (status = 200, description = "Backup & Disaster Recovery operational telemetry", body = BackupStatus)
    )
)]
pub async fn backup_status_handler(
    State(state): State<AppState>,
) -> Json<crate::state::BackupStatus> {
    Json(state.get_backup_status())
}

/// Prometheus Metrics Exporter Endpoint (/metrics).
pub async fn prometheus_metrics_handler(
    State(state): State<AppState>,
) -> (
    StatusCode,
    [(axum::http::header::HeaderName, &'static str); 1],
    String,
) {
    let status = state.get_backup_status();
    let questdb_up = if state
        .questdb_health_up
        .load(std::sync::atomic::Ordering::Relaxed)
    {
        1
    } else {
        0
    };
    let fallback_total = state.get_timescale_fallback_count();

    let output = format!(
        "# HELP fintext_backup_last_success_timestamp Unix timestamp of latest successful backup\n\
         # TYPE fintext_backup_last_success_timestamp gauge\n\
         fintext_backup_last_success_timestamp {}\n\
         # HELP fintext_backup_age_hours Hours elapsed since latest successful backup\n\
         # TYPE fintext_backup_age_hours gauge\n\
         fintext_backup_age_hours {}\n\
         # HELP fintext_backup_size_bytes Compressed size of latest backup archive in bytes\n\
         # TYPE fintext_backup_size_bytes gauge\n\
         fintext_backup_size_bytes {}\n\
         # HELP fintext_backup_duration_seconds Execution duration of latest backup in seconds\n\
         # TYPE fintext_backup_duration_seconds gauge\n\
         fintext_backup_duration_seconds {}\n\
         # HELP fintext_restore_test_last_success_timestamp Unix timestamp of latest successful restore drill\n\
         # TYPE fintext_restore_test_last_success_timestamp gauge\n\
         fintext_restore_test_last_success_timestamp {}\n\
         # HELP fintext_restore_test_duration_seconds Measured RTO duration of latest restore drill\n\
         # TYPE fintext_restore_test_duration_seconds gauge\n\
         fintext_restore_test_duration_seconds {}\n\
         # HELP fintext_restore_test_age_hours Hours elapsed since latest restore drill\n\
         # TYPE fintext_restore_test_age_hours gauge\n\
         fintext_restore_test_age_hours {}\n\
         # HELP fintext_backup_failure_total Total count of failed backup attempts\n\
         # TYPE fintext_backup_failure_total counter\n\
         fintext_backup_failure_total {}\n\
         # HELP fintext_restore_test_failure_total Total count of failed restore drills\n\
         # TYPE fintext_restore_test_failure_total counter\n\
         fintext_restore_test_failure_total {}\n\
         # HELP fintext_timescale_fallback_total Total count of TimescaleDB fallbacks triggered\n\
         # TYPE fintext_timescale_fallback_total counter\n\
         fintext_timescale_fallback_total {}\n\
         # HELP fintext_questdb_health_up Binary indicator whether QuestDB time-series node is reachable\n\
         # TYPE fintext_questdb_health_up gauge\n\
         fintext_questdb_health_up {}\n",
        status.backup_last_success_timestamp,
        status.backup_age_hours,
        status.backup_size_bytes,
        status.backup_duration_seconds,
        status.restore_test_last_success_timestamp,
        status.restore_test_duration_seconds,
        status.restore_test_age_hours,
        status.backup_failure_count,
        status.restore_test_failure_count,
        fallback_total,
        questdb_up
    );

    (
        StatusCode::OK,
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        output,
    )
}
