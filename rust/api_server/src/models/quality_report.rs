//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Signal Quality Report & Alpha Validation DTO Models
//! ═══════════════════════════════════════════════════════════════════════════════

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use utoipa::ToSchema;

fn default_horizon_days() -> Option<u32> {
    Some(5)
}

fn default_benchmark_ticker() -> Option<String> {
    Some("SPY".to_string())
}

/// Request payload for evaluating historical Signal Quality and Predictive Power.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct SignalQualityReportRequest {
    /// Signal family stream to evaluate ('sentiment', 'spillover', 'gex', 'insider', 'event')
    #[schema(example = "sentiment")]
    pub signal_type: String,

    /// Constituent stock ticker symbols (1 to 20 tickers)
    #[schema(example = json!(["AAPL", "MSFT", "NVDA"]))]
    pub tickers: Vec<String>,

    /// Start date for historical evaluation in YYYY-MM-DD format
    #[schema(example = "2025-01-01")]
    pub start_date: String,

    /// End date for historical evaluation in YYYY-MM-DD format
    #[schema(example = "2025-06-30")]
    pub end_date: String,

    /// Forward return holding horizon in trading days (1 to 20, default: 5)
    #[schema(example = 5)]
    #[serde(default = "default_horizon_days")]
    pub horizon_days: Option<u32>,

    /// Benchmark asset ticker for relative comparison (default: 'SPY')
    #[schema(example = "SPY")]
    #[serde(default = "default_benchmark_ticker")]
    pub benchmark_ticker: Option<String>,
}

/// Information Coefficient (IC) Statistical Summary.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct ICSummary {
    /// Spearman Rank correlation between signal value and forward asset returns
    #[schema(example = 0.042)]
    pub spearman_ic: f64,

    /// Normalized Rank IC across cross-sectional observations
    #[schema(example = 0.042)]
    pub rank_ic: f64,

    /// Total count of valid paired signal and return observations
    #[schema(example = 4500)]
    pub observations: usize,

    /// Information Coefficient Information Ratio (mean IC / sample stddev of ICs across evaluation periods)
    #[schema(example = 0.65)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icir: Option<f64>,
}

/// Point on the Signal Horizon Decay Curve.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct DecayCurvePoint {
    /// Forward return horizon in trading days (e.g., 1, 2, 3, 5, 10, 20)
    #[schema(example = 1)]
    pub horizon_days: u32,

    /// Estimated Information Coefficient (IC) at this forward horizon
    #[schema(example = 0.055)]
    pub ic: f64,
}

/// Market Capitalization Quantile Bias Analytics.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct MarketCapBias {
    /// Average signal strength across top market-cap quartile (Large Cap)
    #[schema(example = 0.012)]
    pub top_quantile_avg: f64,

    /// Average signal strength across bottom market-cap quartile (Small Cap)
    #[schema(example = 0.025)]
    pub bottom_quantile_avg: f64,

    /// Spread between bottom and top quartile averages (Small - Large Cap bias)
    #[schema(example = 0.013)]
    pub spread: f64,
}

/// Comprehensive Signal Quality & Predictive Validity Report.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct SignalQualityReportResponse {
    /// Evaluated signal family stream
    #[schema(example = "sentiment")]
    pub signal_type: String,

    /// Evaluated stock constituent ticker symbols
    pub tickers: Vec<String>,

    /// Backtest evaluation start date (YYYY-MM-DD)
    #[schema(example = "2025-01-01")]
    pub start_date: String,

    /// Backtest evaluation end date (YYYY-MM-DD)
    #[schema(example = "2025-06-30")]
    pub end_date: String,

    /// Forward return holding horizon evaluated
    #[schema(example = 5)]
    pub horizon_days: u32,

    /// Fraction of evaluation days with active signal generation per ticker (percentage)
    #[schema(example = 88.5)]
    pub coverage_pct: f64,

    /// Average signal ingestion latency in milliseconds (db_commit_utc - published_utc)
    #[schema(example = 410.2)]
    pub freshness_avg_ms: f64,

    /// 95th percentile signal ingestion latency in milliseconds
    #[schema(example = 980.0)]
    pub freshness_p95_ms: f64,

    /// Information Coefficient (IC) and Rank IC statistical metrics
    pub ic_summary: ICSummary,

    /// Information coefficient decay trajectory across 1, 2, 3, 5, 10, 20 day horizons
    pub decay_curve: Vec<DecayCurvePoint>,

    /// Estimated exponential signal half-life in trading days
    #[schema(example = 14.0)]
    pub half_life_days: f64,

    /// Percentage of observations where signal direction matched forward return direction
    #[schema(example = 52.3)]
    pub hit_rate_pct: f64,

    /// Percentage of positive signals that resulted in negative forward returns
    #[schema(example = 18.2)]
    pub false_positive_rate_pct: f64,

    /// Systematic sector-level signal bias mapping (Sector Name -> Average Signal Value)
    pub sector_bias: HashMap<String, f64>,

    /// Market capitalization quantile spread and bias
    pub market_cap_bias: MarketCapBias,

    /// ISO-8601 UTC compilation timestamp
    #[schema(example = "2025-07-01T12:00:00Z")]
    pub generated_at: String,

    /// Diagnostic and status message
    #[schema(example = "Signal quality report generated for sentiment stream across 3 tickers")]
    pub message: String,
}
