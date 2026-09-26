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

// ─────────────────────────────────────────────────────────────────────────────
// Problem #11: Per-Tenant Usage & API-Key Audit Surfaces
// ─────────────────────────────────────────────────────────────────────────────

/// Daily usage aggregate item for 30-day time series.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct DailyUsageItem {
    /// UTC date in YYYY-MM-DD format
    #[schema(example = "2026-09-01")]
    pub date: String,
    /// Number of recorded requests on this date
    #[schema(example = 450)]
    pub requests: u64,
}

/// Request breakdown by functional endpoint group.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct EndpointGroupUsageItem {
    /// Functional route group (sentiment, alpha, pit, analytics, other)
    #[schema(example = "sentiment")]
    pub group: String,
    /// Total request count in the current billing period
    #[schema(example = 5230)]
    pub requests: u64,
}

/// Sanitized API key audit entry (strictly zero hash or secret exposure).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct ApiKeyAuditItem {
    /// Safe public prefix (e.g. "ak_live_a1b2")
    #[schema(example = "ak_live_a1b2")]
    pub prefix: String,
    /// Friendly label or identifier
    #[schema(example = "Production Signal Ingestion Key")]
    pub name: String,
    /// Key generation timestamp in UTC RFC3339 format
    #[schema(example = "2026-09-01T00:00:00Z")]
    pub created_utc: String,
    /// Last observed usage timestamp in UTC RFC3339 format
    #[schema(example = "2026-09-26T04:15:00Z")]
    pub last_seen_utc: Option<String>,
    /// Whether the key is currently valid and unrevoked
    #[schema(example = true)]
    pub active: bool,
}

/// Sanitized tenant-scoped audit log summary item.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct AuditLogSummaryItem {
    /// Event occurrence timestamp in UTC RFC3339 format
    #[schema(example = "2026-09-26T04:20:00Z")]
    pub ts: String,
    /// Audit action type (e.g. "api_key.create", "ip_whitelist.add")
    #[serde(rename = "type")]
    #[schema(example = "api_key.create")]
    pub event_type: String,
    /// Actor identifier or user initiating the action
    #[schema(example = "usr_institutional_hedgefund_01")]
    pub actor: String,
}

/// Tenant self-service usage & quota audit payload (`GET /v1/account/usage`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct AccountUsageResponse {
    /// Confined organization identifier
    #[schema(example = "org_quant_fund_alpha")]
    pub org_id: String,
    /// Current UTC billing month ("YYYY-MM")
    #[schema(example = "2026-09")]
    pub period_utc: String,
    /// Active subscription plan tier ("growth", "starter", "enterprise", "free")
    #[schema(example = "growth")]
    pub plan: String,
    /// Maximum billable requests quota allowed under active plan
    #[schema(example = 500000)]
    pub plan_limit: Option<u64>,
    /// Total recorded requests consumed in current UTC month
    #[schema(example = 12345)]
    pub requests_total: u64,
    /// Remaining headroom percentage before 429 quota exhaustion (0.0% to 100.0%)
    #[schema(example = 97.53)]
    pub headroom_pct: f64,
    /// Daily request time series for the trailing 30 days
    pub daily: Vec<DailyUsageItem>,
    /// Request breakdown grouped by functional endpoint family
    pub by_endpoint_group: Vec<EndpointGroupUsageItem>,
    /// Sanitized list of tenant API keys (prefix, name, created, last_seen, active only)
    pub keys: Vec<ApiKeyAuditItem>,
    /// Recent tenant audit events (last 50 events scoped strictly to this organization)
    pub recent_audit: Vec<AuditLogSummaryItem>,
    /// Active IP / CIDR whitelist rules enforced for this tenant
    #[schema(example = json!(["192.168.1.0/24", "10.0.0.1/32"]))]
    pub ip_whitelist: Vec<String>,
    /// Telemetry generation timestamp in UTC RFC3339 format
    #[schema(example = "2026-09-26T04:30:00Z")]
    pub generated_utc: String,
}

/// Subscription and dunning status item for administrative diagnostics.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct SubscriptionDetailItem {
    /// Active plan identifier
    #[schema(example = "growth")]
    pub plan: String,
    /// Current lifecycle status ("active", "past_due", "canceled", "incomplete")
    #[schema(example = "active")]
    pub status: String,
    /// Consecutive failed billing charge attempts
    #[schema(example = 0)]
    pub dunning_fail_count: i32,
    /// Expiration timestamp of active 72h dunning grace period
    pub grace_until_utc: Option<String>,
}

/// Administrative tenant usage & subscription diagnostic payload (`GET /v1/admin/tenants/{org_id}/usage`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct AdminTenantUsageResponse {
    /// Base tenant usage and quota metrics
    #[serde(flatten)]
    pub usage: AccountUsageResponse,
    /// Commercial subscription and dunning health status
    pub subscription: Option<SubscriptionDetailItem>,
}
