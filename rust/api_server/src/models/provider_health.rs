//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Provider Health & Operational Transparency Models
//! ═══════════════════════════════════════════════════════════════════════════════

use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

/// Query parameters for provider health inspection (`GET /providers/health`).
#[derive(Debug, Clone, Deserialize, IntoParams, ToSchema)]
pub struct ProviderHealthQuery {
    /// Target provider filter: 'sec_edgar', 'finnhub', 'polygon', or 'all' (default: 'all')
    #[param(example = "all")]
    pub provider: Option<String>,
    /// Lookback aggregation window in minutes (1 to 1440, default: 60)
    #[param(example = 60)]
    pub window_minutes: Option<u32>,
}

/// Operational health metric and SLA telemetry for an individual data source provider.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct ProviderHealthItem {
    /// Canonical provider identifier (e.g. 'sec_edgar', 'finnhub', 'polygon')
    #[schema(example = "sec_edgar")]
    pub provider: String,
    /// Current operational health status: 'healthy', 'degraded', 'outage'
    #[schema(example = "healthy")]
    pub status: String,
    /// Total outbound requests dispatched within lookback window
    #[schema(example = 120)]
    pub requests_total: u64,
    /// Successful requests completed within lookback window
    #[schema(example = 118)]
    pub requests_success: u64,
    /// Service request success rate percentage
    #[schema(example = 98.33)]
    pub success_rate_pct: f64,
    /// Average request round-trip latency in milliseconds
    #[schema(example = 250.5)]
    pub avg_latency_ms: f64,
    /// 95th percentile request latency in milliseconds
    #[schema(example = 480.0)]
    pub p95_latency_ms: f64,
    /// Number of operational/network errors encountered in the last hour
    #[schema(example = 2)]
    pub error_count_last_hour: u64,
    /// ISO-8601 UTC timestamp of the most recent successful sync
    #[schema(example = "2025-09-05T10:30:00Z")]
    pub last_success_timestamp: String,
    /// Detailed diagnostic message of the last encountered error, if any
    #[schema(example = "Timeout after 5000ms")]
    pub last_error_message: Option<String>,
    /// Data quality governance score (0.0 to 1.0) based on accuracy, freshness, and completeness
    #[schema(example = 0.98)]
    #[serde(default = "default_quality_score")]
    pub quality_score: f64,
    /// Number of records currently held in quarantine due to rule violations or low quality
    #[schema(example = 0)]
    #[serde(default)]
    pub quarantine_count: u64,
}

fn default_quality_score() -> f64 {
    1.0
}


/// Response payload for provider health status inquiry (`GET /providers/health`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct ProviderHealthResponse {
    /// Telemetry and health metrics for the requested active providers
    pub providers: Vec<ProviderHealthItem>,
    /// ISO-8601 UTC timestamp when the health assessment was compiled
    #[schema(example = "2025-09-05T11:00:00Z")]
    pub generated_at: String,
}
