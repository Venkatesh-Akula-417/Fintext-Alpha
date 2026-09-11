use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

/// Query parameters for the Options Put/Call Ratio endpoint.
#[derive(Debug, Clone, Deserialize, IntoParams, ToSchema)]
pub struct PutCallRatioParams {
    /// Underlying equity ticker symbol (e.g., "AAPL"). If omitted, market-wide aggregate ratio is computed.
    #[param(example = "AAPL")]
    pub ticker: Option<String>,

    /// Start date of observation window (YYYY-MM-DD).
    #[param(example = "2025-01-01")]
    pub start_date: String,

    /// End date of observation window (YYYY-MM-DD). Maximum window is 730 days (2 years).
    #[param(example = "2025-03-31")]
    pub end_date: String,

    /// Ratio metric to evaluate: "volume" (default) or "open_interest".
    #[param(example = "volume")]
    pub ratio_type: Option<String>,

    /// Aggregation granularity: "daily" (default, time series) or "total" (period aggregate).
    #[param(example = "daily")]
    pub granularity: Option<String>,
}

/// Daily observation point for Put/Call Ratio time series.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PutCallRatioPoint {
    /// Trading date in YYYY-MM-DD format.
    pub date: String,

    /// Total traded put volume on this date.
    pub put_volume: u64,

    /// Total traded call volume on this date.
    pub call_volume: u64,

    /// Total put open interest on this date (if available).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub put_open_interest: Option<u64>,

    /// Total call open interest on this date (if available).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub call_open_interest: Option<u64>,

    /// Computed put/call ratio (put_metric / call_metric). None if call metric is 0.
    pub ratio: Option<f64>,
}

/// Response payload for the Put/Call Ratio endpoint.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PutCallRatioResponse {
    /// Evaluated underlying ticker symbol (None if market-wide aggregate).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ticker: Option<String>,

    /// Observation start date (YYYY-MM-DD).
    pub start_date: String,

    /// Observation end date (YYYY-MM-DD).
    pub end_date: String,

    /// Evaluated ratio metric: "volume" or "open_interest".
    pub ratio_type: String,

    /// Aggregation granularity: "daily" or "total".
    pub granularity: String,

    /// Time series of daily put/call points (populated when granularity is "daily").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub points: Option<Vec<PutCallRatioPoint>>,

    /// Mean put/call ratio across all valid daily points in the observation period.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub average_ratio: Option<f64>,

    /// Total aggregated put volume across the observation period (populated when granularity is "total").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_put_volume: Option<u64>,

    /// Total aggregated call volume across the observation period (populated when granularity is "total").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_call_volume: Option<u64>,

    /// Total aggregated put open interest across the period (populated when granularity is "total").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_put_open_interest: Option<u64>,

    /// Total aggregated call open interest across the period (populated when granularity is "total").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_call_open_interest: Option<u64>,

    /// Total aggregate put/call ratio across the entire period (populated when granularity is "total").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_ratio: Option<f64>,

    /// ISO 8601 UTC timestamp of calculation.
    pub generated_at: String,
}
