//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — User API Usage Statistics & Consumption Data Models
//! ═══════════════════════════════════════════════════════════════════════════════

use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

/// Query parameters for filtering and grouping user API usage statistics.
#[derive(Debug, Clone, Deserialize, IntoParams, ToSchema)]
pub struct UsageStatsParams {
    /// Start date for usage window in ISO YYYY-MM-DD format (defaults to 30 days ago)
    #[param(example = "2026-07-30")]
    #[schema(example = "2026-07-30")]
    pub start_date: Option<String>,

    /// End date for usage window in ISO YYYY-MM-DD format (defaults to today)
    #[param(example = "2026-08-29")]
    #[schema(example = "2026-08-29")]
    pub end_date: Option<String>,

    /// Dimension to group breakdown metrics by: "day", "endpoint", "method", "status_code" (defaults to "day")
    #[param(example = "day")]
    #[schema(example = "day")]
    pub group_by: Option<String>,

    /// Maximum number of breakdown groups to return (1..=1000, defaults to 100)
    #[param(example = 100)]
    #[schema(example = 100)]
    pub limit: Option<u32>,
}

/// Aggregated summary statistics for an authenticated user over the queried time window.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct UsageStatsSummary {
    /// Total number of requests recorded in the window
    #[schema(example = 1250)]
    pub total_requests: u64,

    /// Number of successful requests (HTTP status 200..=299)
    #[schema(example = 1205)]
    pub successful_requests: u64,

    /// Number of failed requests (HTTP status >= 400)
    #[schema(example = 45)]
    pub failed_requests: u64,

    /// Number of rate-limited requests (HTTP status 429)
    #[schema(example = 12)]
    pub rate_limited_requests: u64,

    /// Mean request processing latency in milliseconds
    #[schema(example = 4.85)]
    pub average_latency_ms: f64,

    /// 95th percentile request processing latency in milliseconds
    #[schema(example = 14.20)]
    pub p95_latency_ms: f64,

    /// Maximum observed request processing latency in milliseconds
    #[schema(example = 45.60)]
    pub max_latency_ms: f64,
}

/// Breakdown entry grouped by the requested dimension.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct UsageGroupItem {
    /// Group identifier key (e.g. "2026-08-28" for day, "/sentiment" for endpoint, "GET" for method, "200" for status_code)
    #[schema(example = "2026-08-28")]
    pub key: String,

    /// Total requests within this group
    #[schema(example = 150)]
    pub count: u64,

    /// Successful requests (2xx) in this group
    #[schema(example = 145)]
    pub successful_requests: u64,

    /// Failed requests (>= 400) in this group
    #[schema(example = 5)]
    pub failed_requests: u64,

    /// Rate-limited requests (429) in this group
    #[schema(example = 1)]
    pub rate_limited_requests: u64,

    /// Average latency in milliseconds for requests in this group
    #[schema(example = 3.92)]
    pub average_latency_ms: f64,

    /// 95th percentile latency in milliseconds for this group
    #[schema(example = 11.50)]
    pub p95_latency_ms: f64,

    /// Maximum latency in milliseconds in this group
    #[schema(example = 38.20)]
    pub max_latency_ms: f64,
}

/// Response payload containing aggregate usage statistics and dimensional breakdown.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct UsageStatsResponse {
    /// Authenticated user identifier (`sub` claim)
    #[schema(example = "usr_institutional_hedgefund_01")]
    pub user_id: String,

    /// Applied start date filter (YYYY-MM-DD)
    #[schema(example = "2026-07-30")]
    pub start_date: String,

    /// Applied end date filter (YYYY-MM-DD)
    #[schema(example = "2026-08-29")]
    pub end_date: String,

    /// Applied grouping dimension ("day", "endpoint", "method", "status_code")
    #[schema(example = "day")]
    pub group_by: String,

    /// Summary metrics aggregated over the entire window
    pub summary: UsageStatsSummary,

    /// List of breakdown groups sorted chronologically (for day) or by request volume descending
    pub breakdown: Vec<UsageGroupItem>,

    /// Diagnostic and operational status message
    #[schema(example = "Usage statistics aggregated successfully")]
    pub message: String,
}
