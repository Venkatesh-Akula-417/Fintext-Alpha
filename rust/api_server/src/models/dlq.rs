//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Dead Letter Queue (DLQ) Models & DTOs
//! ═══════════════════════════════════════════════════════════════════════════════

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

/// Summary item representing a failed event in DLQ list queries.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DLQEventItem {
    /// Internal unique identifier UUID for the DLQ record
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: Uuid,
    /// Upstream source event identifier string
    #[schema(example = "evt_sentiment_nvda_20260831_101")]
    pub event_id: String,
    /// Originating subsystem/pipeline source
    #[schema(example = "sentiment")]
    pub source: String,
    /// Error classification category
    #[schema(example = "TimeoutError")]
    pub error_type: Option<String>,
    /// Narrative description of failure reason
    #[schema(example = "Connection timeout while writing ILP metrics to QuestDB")]
    pub error_message: Option<String>,
    /// Truncated preview snippet of the event payload
    #[schema(
        example = "{\"ticker\":\"NVDA\",\"sentiment\":0.85,\"source\":\"Institutional Wire\"}"
    )]
    pub payload_preview: String,
    /// Timestamp when failure occurred in UTC
    #[schema(example = "2026-08-31T12:00:00Z")]
    pub failed_at: DateTime<Utc>,
    /// Total number of automated or manual retry attempts
    #[schema(example = 3)]
    pub retry_count: i32,
    /// Lifecycle status of the DLQ item: 'failed', 'retrying', 'reprocessed', 'purged'
    #[schema(example = "failed")]
    pub status: String,
    /// Creation timestamp in UTC
    #[schema(example = "2026-08-31T12:00:00Z")]
    pub created_at: DateTime<Utc>,
    /// Last update timestamp in UTC
    #[schema(example = "2026-08-31T12:05:00Z")]
    pub updated_at: DateTime<Utc>,
}

/// Comprehensive details of a specific DLQ event, including full un-truncated payload.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DLQEventDetail {
    /// Internal unique identifier UUID for the DLQ record
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: Uuid,
    /// Upstream source event identifier string
    #[schema(example = "evt_sentiment_nvda_20260831_101")]
    pub event_id: String,
    /// Originating subsystem/pipeline source
    #[schema(example = "sentiment")]
    pub source: String,
    /// Error classification category
    #[schema(example = "TimeoutError")]
    pub error_type: Option<String>,
    /// Full narrative description of failure reason
    #[schema(example = "Connection timeout while writing ILP metrics to QuestDB: pool exhausted")]
    pub error_message: Option<String>,
    /// Full original message payload JSON structure
    pub payload: serde_json::Value,
    /// Timestamp when failure occurred in UTC
    #[schema(example = "2026-08-31T12:00:00Z")]
    pub failed_at: DateTime<Utc>,
    /// Total number of automated or manual retry attempts
    #[schema(example = 3)]
    pub retry_count: i32,
    /// Lifecycle status of the DLQ item: 'failed', 'retrying', 'reprocessed', 'purged'
    #[schema(example = "failed")]
    pub status: String,
    /// Creation timestamp in UTC
    #[schema(example = "2026-08-31T12:00:00Z")]
    pub created_at: DateTime<Utc>,
    /// Last update timestamp in UTC
    #[schema(example = "2026-08-31T12:05:00Z")]
    pub updated_at: DateTime<Utc>,
}

/// Paginated list response envelope for DLQ events query.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DLQEventsListResponse {
    /// Array of matching DLQ event summary items
    pub events: Vec<DLQEventItem>,
    /// Total count of matching records across all pages
    #[schema(example = 42)]
    pub total: usize,
    /// Maximum limit of items returned in current page
    #[schema(example = 20)]
    pub limit: usize,
    /// Pagination offset applied
    #[schema(example = 0)]
    pub offset: usize,
}

/// Query parameters for filtering and paginating DLQ events (`GET /dlq/events`).
#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct DLQEventsQueryParams {
    /// Optional filter by originating event source (e.g. 'ingestion', 'sentiment', 'options', 'fix')
    pub source: Option<String>,
    /// Optional filter by error classification
    pub error_type: Option<String>,
    /// Optional filter by status: 'failed', 'retrying', 'reprocessed', 'purged', or 'all' (default: 'failed')
    pub status: Option<String>,
    /// Optional start date boundary (YYYY-MM-DD)
    pub start_date: Option<String>,
    /// Optional end date boundary (YYYY-MM-DD)
    pub end_date: Option<String>,
    /// Maximum items to return per page (1..=200, default: 20)
    pub limit: Option<usize>,
    /// Number of items to skip for pagination (default: 0)
    pub offset: Option<usize>,
}

/// Response payload confirming immediate reprocessing initiation for a DLQ event.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ReprocessDLQResponse {
    /// Unique DLQ record identifier UUID
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: Uuid,
    /// Upstream source event identifier string
    #[schema(example = "evt_sentiment_nvda_20260831_101")]
    pub event_id: String,
    /// Updated lifecycle status after reprocessing ('reprocessed' or 'retrying')
    #[schema(example = "reprocessed")]
    pub status: String,
    /// Updated total retry count
    #[schema(example = 4)]
    pub retry_count: i32,
    /// Operational status message
    #[schema(example = "Event reprocessed successfully and republished to processing pipeline")]
    pub message: String,
    /// Reprocessing timestamp in UTC
    #[schema(example = "2026-08-31T12:10:00Z")]
    pub reprocessed_at: DateTime<Utc>,
}

/// Response payload confirming permanent purge of a DLQ event.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PurgeDLQResponse {
    /// Unique DLQ record identifier UUID
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: Uuid,
    /// Upstream source event identifier string
    #[schema(example = "evt_sentiment_nvda_20260831_101")]
    pub event_id: String,
    /// Final purged lifecycle status
    #[schema(example = "purged")]
    pub status: String,
    /// Confirmation message
    #[schema(example = "Event purged from quarantine and marked as purged")]
    pub message: String,
    /// Purge timestamp in UTC
    #[schema(example = "2026-08-31T12:15:00Z")]
    pub purged_at: DateTime<Utc>,
}
