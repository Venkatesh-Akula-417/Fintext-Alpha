//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Asynchronous Non-Blocking Usage Metering Pipeline
//! ═══════════════════════════════════════════════════════════════════════════════

use crate::auth::Claims;
use crate::state::AppState;
use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::Response;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, QueryBuilder};
use std::time::Instant;
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tracing::{debug, error, info, trace, warn};

/// Default capacity for the asynchronous bounded metering channel.
pub const DEFAULT_METERING_CHANNEL_CAPACITY: usize = 10_000;
/// Default batch size for PostgreSQL bulk insertions.
pub const DEFAULT_METERING_BATCH_SIZE: usize = 100;
/// Default flush interval in milliseconds.
pub const DEFAULT_METERING_FLUSH_INTERVAL_MS: u64 = 1_000;

/// Represents a single recorded API usage event.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UsageEvent {
    pub user_id: String,
    pub endpoint: String,
    pub method: String,
    pub status_code: u16,
    pub latency_ms: f32,
    pub created_at: DateTime<Utc>,
}

/// Configuration options for the background metering batch worker.
#[derive(Debug, Clone)]
pub struct MeteringWorkerConfig {
    pub batch_size: usize,
    pub flush_interval_ms: u64,
    pub channel_capacity: usize,
}

impl Default for MeteringWorkerConfig {
    fn default() -> Self {
        Self {
            batch_size: DEFAULT_METERING_BATCH_SIZE,
            flush_interval_ms: DEFAULT_METERING_FLUSH_INTERVAL_MS,
            channel_capacity: DEFAULT_METERING_CHANNEL_CAPACITY,
        }
    }
}

/// Creates the PostgreSQL schema and indexing for usage metering if they do not already exist.
pub async fn init_db(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS usage_events (
            id BIGSERIAL PRIMARY KEY,
            user_id TEXT NOT NULL,
            endpoint TEXT NOT NULL,
            method TEXT NOT NULL,
            status_code SMALLINT NOT NULL,
            latency_ms REAL NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        );
        CREATE INDEX IF NOT EXISTS idx_usage_events_user_time ON usage_events(user_id, created_at DESC);
        "#,
    )
    .execute(pool)
    .await?;

    info!("[Usage Metering] Verified 'usage_events' table and indexes in PostgreSQL");
    Ok(())
}

/// Creates a new bounded metering channel.
pub fn create_metering_channel(capacity: usize) -> (Sender<UsageEvent>, Receiver<UsageEvent>) {
    channel(capacity)
}

/// Bulk inserts a batch of UsageEvents into PostgreSQL using `sqlx::QueryBuilder`.
pub async fn flush_batch(batch: &mut Vec<UsageEvent>, pool: Option<&PgPool>) {
    if batch.is_empty() {
        return;
    }

    if let Some(pool) = pool {
        let mut query_builder: QueryBuilder<sqlx::Postgres> = QueryBuilder::new(
            "INSERT INTO usage_events (user_id, endpoint, method, status_code, latency_ms, created_at) ",
        );

        query_builder.push_values(batch.iter(), |mut b, event| {
            b.push_bind(&event.user_id)
                .push_bind(&event.endpoint)
                .push_bind(&event.method)
                .push_bind(event.status_code as i16)
                .push_bind(event.latency_ms)
                .push_bind(event.created_at);
        });

        let query = query_builder.build();
        if let Err(e) = query.execute(pool).await {
            error!(
                "[Usage Metering] Failed to bulk insert batch of {} usage events into PostgreSQL: {}",
                batch.len(),
                e
            );
        } else {
            debug!(
                "[Usage Metering] Successfully persisted batch of {} usage events into PostgreSQL",
                batch.len()
            );
        }
    } else {
        trace!(
            "[Usage Metering] Discarded {} usage events (PostgreSQL pool unconfigured)",
            batch.len()
        );
    }

    batch.clear();
}

/// Spawns the background asynchronous metering batch worker.
pub fn spawn_metering_worker(
    mut rx: Receiver<UsageEvent>,
    pool: Option<PgPool>,
    config: MeteringWorkerConfig,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        info!(
            "[Usage Metering] Background batch worker started (Batch size: {}, Flush interval: {}ms)",
            config.batch_size, config.flush_interval_ms
        );

        let mut batch: Vec<UsageEvent> = Vec::with_capacity(config.batch_size);
        let mut interval =
            tokio::time::interval(tokio::time::Duration::from_millis(config.flush_interval_ms));

        loop {
            tokio::select! {
                maybe_event = rx.recv() => {
                    match maybe_event {
                        Some(event) => {
                            batch.push(event);
                            if batch.len() >= config.batch_size {
                                flush_batch(&mut batch, pool.as_ref()).await;
                            }
                        }
                        None => {
                            // Channel closed during application shutdown
                            if !batch.is_empty() {
                                flush_batch(&mut batch, pool.as_ref()).await;
                            }
                            info!("[Usage Metering] Background batch worker gracefully stopped");
                            break;
                        }
                    }
                }
                _ = interval.tick() => {
                    if !batch.is_empty() {
                        flush_batch(&mut batch, pool.as_ref()).await;
                    }
                }
            }
        }
    })
}

/// Axum middleware that captures authenticated request metrics and dispatches `UsageEvent` non-blockingly.
pub async fn metering_middleware(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Response {
    let start = Instant::now();
    let method = req.method().to_string();
    let endpoint = req.uri().path().to_string();
    let user_id = req
        .extensions()
        .get::<Claims>()
        .map(|c| c.sub.clone())
        .unwrap_or_else(|| "anonymous".to_string());

    let is_sandbox = req
        .extensions()
        .get::<crate::sandbox::SandboxContext>()
        .map(|ctx| ctx.is_sandbox)
        .unwrap_or_else(|| {
            req.extensions()
                .get::<Claims>()
                .map(|c| c.sandbox.unwrap_or(false) || state.sandbox_registry.is_active(&c.sub))
                .unwrap_or(false)
        });

    let response = next.run(req).await;

    // Sandbox requests are completely isolated from production usage events and quota metering
    if is_sandbox {
        return response;
    }

    let latency_ms = start.elapsed().as_secs_f32() * 1000.0;
    let status_code = response.status().as_u16();

    let event = UsageEvent {
        user_id,
        endpoint,
        method,
        status_code,
        latency_ms,
        created_at: Utc::now(),
    };

    if let Some(tx) = &state.metering_tx {
        match tx.try_send(event) {
            Ok(_) => {}
            Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
                warn!("[Usage Metering] Queue capacity exceeded. Dropping usage event.");
            }
            Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
                warn!("[Usage Metering] Queue sender closed. Dropping usage event.");
            }
        }
    }

    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_metering_channel_send_and_receive() {
        let (tx, mut rx) = create_metering_channel(10);

        let event = UsageEvent {
            user_id: "quant_tester".to_string(),
            endpoint: "/sentiment".to_string(),
            method: "GET".to_string(),
            status_code: 200,
            latency_ms: 1.25,
            created_at: Utc::now(),
        };

        tx.try_send(event.clone()).expect("Should enqueue event");

        let received = rx.recv().await.expect("Should receive event");
        assert_eq!(received.user_id, "quant_tester");
        assert_eq!(received.endpoint, "/sentiment");
        assert_eq!(received.status_code, 200);
    }

    #[tokio::test]
    async fn test_metering_channel_full_drop() {
        let (tx, _rx) = create_metering_channel(2);

        let event = UsageEvent {
            user_id: "quant_tester".to_string(),
            endpoint: "/sentiment".to_string(),
            method: "GET".to_string(),
            status_code: 200,
            latency_ms: 0.5,
            created_at: Utc::now(),
        };

        assert!(tx.try_send(event.clone()).is_ok());
        assert!(tx.try_send(event.clone()).is_ok());

        // 3rd send on capacity 2 must be TrySendError::Full
        let result = tx.try_send(event);
        assert!(matches!(
            result,
            Err(tokio::sync::mpsc::error::TrySendError::Full(_))
        ));
    }

    #[tokio::test]
    async fn test_flush_batch_without_pool_clears_batch() {
        let mut batch = vec![UsageEvent {
            user_id: "trader_1".to_string(),
            endpoint: "/backtest".to_string(),
            method: "POST".to_string(),
            status_code: 200,
            latency_ms: 15.4,
            created_at: Utc::now(),
        }];

        flush_batch(&mut batch, None).await;
        assert!(batch.is_empty());
    }
}
