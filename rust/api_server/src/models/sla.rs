//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Latency SLA Analytics Models
//! ═══════════════════════════════════════════════════════════════════════════════

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use utoipa::{IntoParams, ToSchema};

/// Query parameters for `GET /sla/status`
#[derive(Debug, Clone, Deserialize, Serialize, IntoParams, ToSchema)]
pub struct SLAStatusParams {
    /// Start date filter in YYYY-MM-DD format (defaults to 30 days prior to end_date).
    #[param(example = "2025-08-01")]
    pub start_date: Option<String>,

    /// End date filter in YYYY-MM-DD format (defaults to current date).
    #[param(example = "2025-08-31")]
    pub end_date: Option<String>,

    /// Comma-separated latency percentiles to calculate (allowed values: 1 to 100, default: "50,95,99").
    #[param(example = "50,95,99")]
    pub percentiles: Option<String>,

    /// Latency SLA target threshold in milliseconds (allowed values: 10 to 5000, default: 100).
    #[param(example = 100)]
    pub sla_target_ms: Option<u32>,
}

/// Latency SLA Status and Compliance Report Response
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SLAStatusResponse {
    /// Start date for the aggregated window in YYYY-MM-DD format.
    #[schema(example = "2025-08-01")]
    pub start_date: String,

    /// End date for the aggregated window in YYYY-MM-DD format.
    #[schema(example = "2025-08-31")]
    pub end_date: String,

    /// Total number of requests processed in the specified window.
    #[schema(example = 10000)]
    pub total_requests: u64,

    /// Volume-weighted arithmetic mean latency in milliseconds.
    #[schema(example = 52.3)]
    pub average_latency_ms: f64,

    /// Latency percentiles computed via linear interpolation on ordered latency series.
    pub percentiles: BTreeMap<String, f64>,

    /// Latency SLA target threshold in milliseconds.
    #[schema(example = 100)]
    pub sla_target_ms: u32,

    /// Count of requests executed with latency <= sla_target_ms.
    #[schema(example = 9980)]
    pub compliant_requests: u64,

    /// SLA compliance percentage ((compliant_requests / total_requests) * 100.0).
    #[schema(example = 99.8)]
    pub sla_compliance_rate: f64,

    /// Overall SLA compliance status: 'met' if sla_compliance_rate >= threshold (default 99.9%), else 'breached'.
    #[schema(example = "met")]
    pub sla_status: String,

    /// ISO-8601 timestamp when this SLA compliance report was generated.
    pub generated_at: DateTime<Utc>,
}

/// Query parameters for `GET /sla/latency` (Pipeline Signal Latency SLA Analytics)
#[derive(Debug, Clone, Deserialize, Serialize, IntoParams, ToSchema)]
pub struct SLALatencyParams {
    /// Optional ticker symbol filter (e.g. "AAPL", "NVDA"). If omitted, aggregates across all tickers.
    #[param(example = "AAPL")]
    pub ticker: Option<String>,

    /// Start date filter in YYYY-MM-DD format (defaults to 30 days prior to end_date).
    #[param(example = "2025-08-01")]
    pub start_date: Option<String>,

    /// End date filter in YYYY-MM-DD format (defaults to current date).
    #[param(example = "2025-08-31")]
    pub end_date: Option<String>,

    /// Comma-separated latency percentiles to calculate (allowed values: 1 to 100, default: "50,95,99").
    #[param(example = "50,95,99")]
    pub percentiles: Option<String>,

    /// Signal latency SLA target threshold in milliseconds (allowed values: 10 to 10000, default: 500).
    #[param(example = 500)]
    pub sla_target_ms: Option<u32>,
}

/// Stage-by-stage average latency breakdown in milliseconds.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct StageBreakdown {
    /// Average latency to fetch raw document via HTTP or WebSocket in milliseconds.
    #[schema(example = 18.5)]
    pub fetch_latency_ms: f64,

    /// Average in-process preprocessing runtime (HTML sanitization, ticker extraction, spam filter, event classification) in milliseconds.
    #[schema(example = 3.2)]
    pub normalization_latency_ms: f64,

    /// Average ONNX sentiment inference runtime (including chunking) in milliseconds.
    #[schema(example = 14.8)]
    pub inference_latency_ms: f64,

    /// Average database ILP write / buffer commit runtime in milliseconds.
    #[schema(example = 5.1)]
    pub write_latency_ms: f64,
}

/// Signal Ingestion and Processing Pipeline Latency SLA Compliance Report Response
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SLALatencyResponse {
    /// Filtered ticker symbol or None for universe-wide aggregation.
    #[schema(example = "AAPL")]
    pub ticker: Option<String>,

    /// Start date for the aggregated window in YYYY-MM-DD format.
    #[schema(example = "2025-08-01")]
    pub start_date: String,

    /// End date for the aggregated window in YYYY-MM-DD format.
    #[schema(example = "2025-08-31")]
    pub end_date: String,

    /// Total number of processed signals analyzed in the window.
    #[schema(example = 15420)]
    pub total_signals: u64,

    /// Volume-weighted arithmetic mean total signal freshness latency in milliseconds.
    #[schema(example = 41.6)]
    pub average_latency_ms: f64,

    /// Total signal latency percentiles computed via linear interpolation on ordered latency series.
    pub percentiles: BTreeMap<String, f64>,

    /// Maximum signal latency recorded in the window in milliseconds.
    #[schema(example = 184.2)]
    pub max_latency_ms: f64,

    /// Target SLA latency threshold in milliseconds.
    #[schema(example = 500)]
    pub sla_target_ms: u32,

    /// Count of signals with total latency <= sla_target_ms.
    #[schema(example = 15390)]
    pub sla_compliant_signals: u64,

    /// SLA compliance percentage ((sla_compliant_signals / total_signals) * 100.0).
    #[schema(example = 99.81)]
    pub sla_compliance_rate: f64,

    /// Overall SLA compliance status: 'met' if sla_compliance_rate >= 99.0%, else 'breached'.
    #[schema(example = "met")]
    pub sla_status: String,

    /// Average latency breakdown across pipeline stages.
    pub stage_breakdown: StageBreakdown,

    /// ISO-8601 timestamp when this report was generated.
    pub generated_at: DateTime<Utc>,
}
