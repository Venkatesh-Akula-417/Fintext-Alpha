//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Dead Letter Queue (DLQ) Registry & Database Manager
//! ═══════════════════════════════════════════════════════════════════════════════

use chrono::{DateTime, NaiveDate, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use tracing::{error, info};
use uuid::Uuid;

use crate::models::dlq::{DLQEventDetail, DLQEventItem, DLQEventsQueryParams};

/// Internal domain model for a DLQ record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DlqEvent {
    pub id: Uuid,
    pub event_id: String,
    pub source: String,
    pub error_type: Option<String>,
    pub error_message: Option<String>,
    pub payload: serde_json::Value,
    pub failed_at: DateTime<Utc>,
    pub retry_count: i32,
    pub status: String, // "failed", "retrying", "reprocessed", "purged"
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl DlqEvent {
    /// Convert to summary list item with truncated payload preview.
    pub fn to_item(&self) -> DLQEventItem {
        let payload_str = serde_json::to_string(&self.payload).unwrap_or_else(|_| "{}".to_string());
        let payload_preview = if payload_str.len() > 180 {
            format!("{}...", &payload_str[..180])
        } else {
            payload_str
        };

        DLQEventItem {
            id: self.id,
            event_id: self.event_id.clone(),
            source: self.source.clone(),
            error_type: self.error_type.clone(),
            error_message: self.error_message.clone(),
            payload_preview,
            failed_at: self.failed_at,
            retry_count: self.retry_count,
            status: self.status.clone(),
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }

    /// Convert to full detail view with unabridged payload JSON.
    pub fn to_detail(&self) -> DLQEventDetail {
        DLQEventDetail {
            id: self.id,
            event_id: self.event_id.clone(),
            source: self.source.clone(),
            error_type: self.error_type.clone(),
            error_message: self.error_message.clone(),
            payload: self.payload.clone(),
            failed_at: self.failed_at,
            retry_count: self.retry_count,
            status: self.status.clone(),
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

/// Concurrent thread-safe registry for DLQ events with PostgreSQL persistence.
pub struct DlqRegistry {
    events: DashMap<Uuid, DlqEvent>,
}

impl DlqRegistry {
    /// Create a new empty DLQ registry with seeded mock data for local testing.
    pub fn new() -> Self {
        let registry = Self {
            events: DashMap::new(),
        };
        registry.seed_mock_data();
        registry
    }

    /// Seed sample DLQ events for mock testing and verification.
    pub fn seed_mock_data(&self) {
        if !self.events.is_empty() {
            return;
        }

        let now = Utc::now();
        let samples = vec![
            DlqEvent {
                id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440101").unwrap(),
                event_id: "evt_sentiment_nvda_101".to_string(),
                source: "sentiment".to_string(),
                error_type: Some("TimeoutError".to_string()),
                error_message: Some(
                    "Connection timeout while writing ILP metrics to QuestDB".to_string(),
                ),
                payload: serde_json::json!({
                    "ticker": "NVDA",
                    "sentiment_score": 0.88,
                    "confidence": 0.94,
                    "headline": "NVIDIA announces next-generation AI accelerators with record efficiency",
                    "source": "Bloomberg Wire"
                }),
                failed_at: now - chrono::Duration::hours(2),
                retry_count: 2,
                status: "failed".to_string(),
                created_at: now - chrono::Duration::hours(2),
                updated_at: now - chrono::Duration::hours(2),
            },
            DlqEvent {
                id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440102").unwrap(),
                event_id: "evt_ingestion_aapl_102".to_string(),
                source: "ingestion".to_string(),
                error_type: Some("ValidationError".to_string()),
                error_message: Some(
                    "Invalid timestamp header encountered in audio stream segment".to_string(),
                ),
                payload: serde_json::json!({
                    "ticker": "AAPL",
                    "audio_segment_id": "seg_9921",
                    "call_id": "aapl_q3_2026",
                    "sample_rate": 16000
                }),
                failed_at: now - chrono::Duration::hours(5),
                retry_count: 1,
                status: "failed".to_string(),
                created_at: now - chrono::Duration::hours(5),
                updated_at: now - chrono::Duration::hours(5),
            },
            DlqEvent {
                id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440103").unwrap(),
                event_id: "evt_options_tsla_103".to_string(),
                source: "options".to_string(),
                error_type: Some("DownstreamUnavailable".to_string()),
                error_message: Some(
                    "Options Greeks surface calculator service returned 503".to_string(),
                ),
                payload: serde_json::json!({
                    "symbol": "TSLA260918C00250000",
                    "underlying": "TSLA",
                    "strike": 250.0,
                    "implied_volatility": 0.45
                }),
                failed_at: now - chrono::Duration::hours(12),
                retry_count: 5,
                status: "failed".to_string(),
                created_at: now - chrono::Duration::hours(12),
                updated_at: now - chrono::Duration::hours(12),
            },
            DlqEvent {
                id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440104").unwrap(),
                event_id: "evt_fix_msft_104".to_string(),
                source: "fix".to_string(),
                error_type: Some("ProtocolError".to_string()),
                error_message: Some(
                    "Invalid tag value delimiter received from broker gateway".to_string(),
                ),
                payload: serde_json::json!({
                    "cl_ord_id": "CL-ORD-MSFT-881",
                    "symbol": "MSFT",
                    "side": "1",
                    "quantity": 500
                }),
                failed_at: now - chrono::Duration::hours(24),
                retry_count: 3,
                status: "failed".to_string(),
                created_at: now - chrono::Duration::hours(24),
                updated_at: now - chrono::Duration::hours(24),
            },
            DlqEvent {
                id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440105").unwrap(),
                event_id: "evt_sentiment_amzn_105".to_string(),
                source: "sentiment".to_string(),
                error_type: Some("RateLimitExceeded".to_string()),
                error_message: Some(
                    "External NLP vendor rate limit exceeded (HTTP 429)".to_string(),
                ),
                payload: serde_json::json!({
                    "ticker": "AMZN",
                    "text": "Amazon Web Services expands global datacenter presence in APAC",
                    "source": "Reuters"
                }),
                failed_at: now - chrono::Duration::days(2),
                retry_count: 1,
                status: "reprocessed".to_string(),
                created_at: now - chrono::Duration::days(2),
                updated_at: now - chrono::Duration::days(1),
            },
        ];

        for s in samples {
            self.events.insert(s.id, s);
        }
    }

    /// Initialize PostgreSQL schema for `dlq_events`.
    pub async fn init_db(pool: &PgPool) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS dlq_events (
                id UUID PRIMARY KEY,
                event_id TEXT NOT NULL,
                source TEXT NOT NULL,
                error_type TEXT,
                error_message TEXT,
                payload JSONB NOT NULL,
                failed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                retry_count INTEGER NOT NULL DEFAULT 0,
                status TEXT NOT NULL DEFAULT 'failed',
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );

            CREATE INDEX IF NOT EXISTS idx_dlq_events_status ON dlq_events(status);
            CREATE INDEX IF NOT EXISTS idx_dlq_events_failed_at ON dlq_events(failed_at);
            CREATE INDEX IF NOT EXISTS idx_dlq_events_source ON dlq_events(source);
            CREATE INDEX IF NOT EXISTS idx_dlq_events_event_id ON dlq_events(event_id);
            "#,
        )
        .execute(pool)
        .await?;

        info!("[DLQ Registry] Initialized PostgreSQL `dlq_events` table and indices.");
        Ok(())
    }

    /// Hydrate in-memory registry from PostgreSQL database on startup.
    pub async fn load_from_db(&self, pool: &PgPool) -> Result<usize, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT id, event_id, source, error_type, error_message, payload, failed_at, retry_count, status, created_at, updated_at
            FROM dlq_events
            ORDER BY failed_at DESC
            "#
        )
        .fetch_all(pool)
        .await?;

        let count = rows.len();
        if count > 0 {
            self.events.clear();
        }

        for row in rows {
            let id: Uuid = row.get("id");
            let event_id: String = row.get("event_id");
            let source: String = row.get("source");
            let error_type: Option<String> = row.get("error_type");
            let error_message: Option<String> = row.get("error_message");
            let payload: serde_json::Value = row.get("payload");
            let failed_at: DateTime<Utc> = row.get("failed_at");
            let retry_count: i32 = row.get("retry_count");
            let status: String = row.get("status");
            let created_at: DateTime<Utc> = row.get("created_at");
            let updated_at: DateTime<Utc> = row.get("updated_at");

            let event = DlqEvent {
                id,
                event_id,
                source,
                error_type,
                error_message,
                payload,
                failed_at,
                retry_count,
                status,
                created_at,
                updated_at,
            };

            self.events.insert(id, event);
        }

        info!(
            count = count,
            "[DLQ Registry] Loaded events from PostgreSQL."
        );
        Ok(count)
    }

    /// Insert or record a new failed DLQ event.
    pub async fn insert(
        &self,
        event: DlqEvent,
        pool_opt: Option<&PgPool>,
    ) -> Result<DlqEvent, String> {
        if let Some(pool) = pool_opt {
            let res = sqlx::query(
                r#"
                INSERT INTO dlq_events (id, event_id, source, error_type, error_message, payload, failed_at, retry_count, status, created_at, updated_at)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
                ON CONFLICT (id) DO UPDATE SET
                    retry_count = EXCLUDED.retry_count,
                    status = EXCLUDED.status,
                    error_message = EXCLUDED.error_message,
                    updated_at = EXCLUDED.updated_at
                "#
            )
            .bind(event.id)
            .bind(&event.event_id)
            .bind(&event.source)
            .bind(&event.error_type)
            .bind(&event.error_message)
            .bind(&event.payload)
            .bind(event.failed_at)
            .bind(event.retry_count)
            .bind(&event.status)
            .bind(event.created_at)
            .bind(event.updated_at)
            .execute(pool)
            .await;

            if let Err(e) = res {
                error!("[DLQ Registry] Failed to insert DLQ event into DB: {}", e);
            }
        }

        self.events.insert(event.id, event.clone());
        Ok(event)
    }

    /// Retrieve an individual DLQ event by UUID.
    pub fn get(&self, id: &Uuid) -> Option<DlqEvent> {
        self.events.get(id).map(|r| r.value().clone())
    }

    /// Query and paginate DLQ events according to query parameters.
    pub fn list(&self, params: &DLQEventsQueryParams) -> (Vec<DLQEventItem>, usize) {
        let mut matched: Vec<DlqEvent> = self
            .events
            .iter()
            .map(|r| r.value().clone())
            .filter(|e| {
                // Filter by source
                if let Some(ref src) = params.source {
                    if !e.source.eq_ignore_ascii_case(src) {
                        return false;
                    }
                }

                // Filter by error_type
                if let Some(ref err_t) = params.error_type {
                    if let Some(ref e_err) = e.error_type {
                        if !e_err.eq_ignore_ascii_case(err_t) {
                            return false;
                        }
                    } else {
                        return false;
                    }
                }

                // Filter by status (default: "failed" if not provided; "all" includes everything)
                let filter_status = params.status.as_deref().unwrap_or("failed");
                if !filter_status.eq_ignore_ascii_case("all") {
                    if !e.status.eq_ignore_ascii_case(filter_status) {
                        return false;
                    }
                }

                // Filter by start_date (YYYY-MM-DD)
                if let Some(ref start_s) = params.start_date {
                    if let Ok(sd) = NaiveDate::parse_from_str(start_s, "%Y-%m-%d") {
                        if e.failed_at.date_naive() < sd {
                            return false;
                        }
                    }
                }

                // Filter by end_date (YYYY-MM-DD)
                if let Some(ref end_s) = params.end_date {
                    if let Ok(ed) = NaiveDate::parse_from_str(end_s, "%Y-%m-%d") {
                        if e.failed_at.date_naive() > ed {
                            return false;
                        }
                    }
                }

                true
            })
            .collect();

        // Sort descending by failed_at
        matched.sort_by(|a, b| b.failed_at.cmp(&a.failed_at));

        let total = matched.len();
        let limit = params.limit.unwrap_or(20).clamp(1, 200);
        let offset = params.offset.unwrap_or(0);

        let paged: Vec<DLQEventItem> = matched
            .into_iter()
            .skip(offset)
            .take(limit)
            .map(|e| e.to_item())
            .collect();

        (paged, total)
    }

    /// Update status and retry count for a DLQ event.
    pub async fn update_status(
        &self,
        id: &Uuid,
        status: &str,
        retry_count: i32,
        pool_opt: Option<&PgPool>,
    ) -> Result<DlqEvent, String> {
        let mut entry = self
            .events
            .get_mut(id)
            .ok_or_else(|| format!("DLQ event '{}' not found", id))?;
        let now = Utc::now();
        entry.status = status.to_string();
        entry.retry_count = retry_count;
        entry.updated_at = now;

        let updated = entry.clone();
        drop(entry);

        if let Some(pool) = pool_opt {
            let res = sqlx::query(
                r#"
                UPDATE dlq_events
                SET status = $2, retry_count = $3, updated_at = $4
                WHERE id = $1
                "#,
            )
            .bind(*id)
            .bind(status)
            .bind(retry_count)
            .bind(now)
            .execute(pool)
            .await;

            if let Err(e) = res {
                error!("[DLQ Registry] Failed to update DLQ event in DB: {}", e);
            }
        }

        Ok(updated)
    }

    /// Purge a DLQ event permanently: marks as 'purged' and removes quarantine file.
    pub async fn purge(&self, id: &Uuid, pool_opt: Option<&PgPool>) -> Result<DlqEvent, String> {
        let mut entry = self
            .events
            .get_mut(id)
            .ok_or_else(|| format!("DLQ event '{}' not found", id))?;
        let now = Utc::now();
        entry.status = "purged".to_string();
        entry.updated_at = now;

        let updated = entry.clone();
        drop(entry);

        if let Some(pool) = pool_opt {
            let res = sqlx::query(
                r#"
                UPDATE dlq_events
                SET status = 'purged', updated_at = $2
                WHERE id = $1
                "#,
            )
            .bind(*id)
            .bind(now)
            .execute(pool)
            .await;

            if let Err(e) = res {
                error!("[DLQ Registry] Failed to purge DLQ event in DB: {}", e);
            }
        }

        Ok(updated)
    }
}
