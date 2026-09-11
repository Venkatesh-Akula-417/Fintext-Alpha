//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Options Implied Volatility & Greeks Models
//! ═══════════════════════════════════════════════════════════════════════════════

use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

/// Parameters for querying options implied volatility and Black-Scholes Greeks.
#[derive(Debug, Clone, Deserialize, IntoParams, ToSchema)]
pub struct OptionsIvParams {
    /// Target underlying equity ticker symbol (e.g. 'AAPL', 'NVDA', 'SPY')
    pub ticker: String,
    /// Option contract expiration date in ISO YYYY-MM-DD format
    pub expiration_date: String,
    /// Optional option type filter ('call', 'put', 'all', default: 'all')
    #[serde(default = "default_option_type")]
    pub option_type: Option<String>,
    /// Optional specific strike price in USD (if omitted, returns a chain around ATM)
    pub strike: Option<f64>,
    /// Annualized risk-free interest rate (e.g., 0.05 for 5%, default: 0.05, allowed: 0.0 to 0.20)
    #[serde(default = "default_risk_free_rate")]
    pub risk_free_rate: Option<f64>,
    /// Annualized underlying continuous dividend yield (e.g., 0.005 for 0.5%, default: 0.0, allowed: 0.0 to 0.10)
    #[serde(default = "default_dividend_yield")]
    pub dividend_yield: Option<f64>,
}

fn default_option_type() -> Option<String> {
    Some("all".to_string())
}

fn default_risk_free_rate() -> Option<f64> {
    Some(0.05)
}

fn default_dividend_yield() -> Option<f64> {
    Some(0.0)
}

/// Normalized institutional option contract metrics with Black-Scholes Greeks.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct OptionContract {
    /// Option contract identifier or ticker (e.g., 'AAPL251219C00250000')
    pub ticker: String,
    /// Underlying equity symbol (e.g., 'AAPL')
    pub underlying_ticker: String,
    /// Expiration date formatted as YYYY-MM-DD
    pub expiration_date: String,
    /// Strike price in USD
    pub strike: f64,
    /// Option contract type ('CALL' or 'PUT')
    pub option_type: String,
    /// Best bid price in USD
    pub bid: f64,
    /// Best ask price in USD
    pub ask: f64,
    /// Last executed trade price in USD
    pub last: f64,
    /// Daily contract trading volume
    pub volume: u64,
    /// Open interest (total outstanding open contracts)
    pub open_interest: u64,
    /// Solved Black-Scholes implied volatility (annualized decimal, e.g. 0.285 for 28.5%)
    pub implied_volatility: f64,
    /// Option Delta Δ (rate of change of option price with respect to underlying spot)
    pub delta: f64,
    /// Option Gamma Γ (rate of change of delta with respect to underlying spot)
    pub gamma: f64,
    /// Option Theta Θ (daily time decay in dollars per calendar day)
    pub theta: f64,
    /// Option Vega ν (dollar change per 1 percentage point change in volatility)
    pub vega: f64,
    /// Option Rho ρ (dollar change per 1 percentage point change in interest rate)
    pub rho: f64,
}

/// Options Implied Volatility and Greeks query response.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct OptionsIvResponse {
    /// Underlying equity ticker symbol
    pub ticker: String,
    /// Option expiration date (YYYY-MM-DD)
    pub expiration_date: String,
    /// Current underlying asset spot price in USD
    pub underlying_price: f64,
    /// Applied annualized risk-free interest rate
    pub risk_free_rate: f64,
    /// Applied annualized dividend yield
    pub dividend_yield: f64,
    /// Total number of option contracts returned
    pub count: usize,
    /// Array of option contracts with implied volatility and Black-Scholes Greeks
    pub contracts: Vec<OptionContract>,
    /// Operational or diagnostic message
    pub message: String,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Unusual Options Activity (UOA) Detection Models
// ═══════════════════════════════════════════════════════════════════════════════

/// Parameters for querying unusual options activity (UOA) — contracts with
/// abnormally high volume relative to open interest.
#[derive(Debug, Clone, Deserialize, IntoParams, ToSchema)]
pub struct UnusualOptionsParams {
    /// Optional underlying ticker filter (e.g. 'AAPL'). If omitted, scans all tickers.
    pub ticker: Option<String>,
    /// Minimum volume / open_interest ratio threshold (default: 2.0)
    #[serde(default = "default_min_volume_oi_ratio")]
    pub min_volume_oi_ratio: Option<f64>,
    /// Minimum absolute contract volume to consider (default: 100)
    #[serde(default = "default_min_volume")]
    pub min_volume: Option<u64>,
    /// Lookback period in trading days for average volume computation (default: 1, max: 7)
    #[serde(default = "default_days")]
    pub days: Option<u32>,
    /// Maximum number of results to return (default: 20, max: 100)
    #[serde(default = "default_limit")]
    pub limit: Option<u32>,
}

fn default_min_volume_oi_ratio() -> Option<f64> {
    Some(2.0)
}

fn default_min_volume() -> Option<u64> {
    Some(100)
}

fn default_days() -> Option<u32> {
    Some(1)
}

fn default_limit() -> Option<u32> {
    Some(20)
}

/// A single options contract flagged for unusual activity.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct UnusualOptionItem {
    /// Option contract identifier or ticker (e.g. 'O:AAPL251219C00250000')
    pub ticker: String,
    /// Underlying equity symbol (e.g. 'AAPL')
    pub underlying_ticker: String,
    /// Expiration date formatted as YYYY-MM-DD
    pub expiration_date: String,
    /// Strike price in USD
    pub strike: f64,
    /// Option contract type ('CALL' or 'PUT')
    pub option_type: String,
    /// Current session trading volume
    pub volume: u64,
    /// Total outstanding open interest
    pub open_interest: u64,
    /// Average daily volume over the lookback window
    pub avg_volume: f64,
    /// Volume divided by max(open_interest, 1)
    pub volume_oi_ratio: f64,
    /// (volume - avg_volume) / max(stddev, 1) — standard deviations above mean
    pub volume_zscore: f64,
    /// Composite unusualness score = volume_oi_ratio × max(volume_zscore, 0.1)
    pub score: f64,
    /// Snapshot timestamp (ISO 8601 UTC)
    pub timestamp: String,
}

/// Response envelope for unusual options activity scan.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct UnusualOptionsResponse {
    /// Ticker filter applied (or "ALL" if scanning universe)
    pub ticker: String,
    /// Applied minimum volume/OI ratio threshold
    pub min_volume_oi_ratio: f64,
    /// Applied minimum absolute volume threshold
    pub min_volume: u64,
    /// Lookback window in trading days
    pub days: u32,
    /// Number of flagged contracts returned
    pub count: usize,
    /// Array of unusual options items, sorted by composite score descending
    pub items: Vec<UnusualOptionItem>,
    /// Operational or diagnostic message
    pub message: String,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Options Volatility Surface Models
// ═══════════════════════════════════════════════════════════════════════════════

/// Parameters for querying the 2D options implied volatility surface grid.
#[derive(Debug, Clone, Deserialize, IntoParams, ToSchema)]
pub struct OptionsVolSurfaceParams {
    /// Target underlying equity ticker symbol (e.g. 'AAPL', 'NVDA', 'SPY')
    pub ticker: String,
    /// Earliest expiration date to include (ISO YYYY-MM-DD, default: today)
    pub start_date: Option<String>,
    /// Latest expiration date to include (ISO YYYY-MM-DD, default: +6 months)
    pub end_date: Option<String>,
    /// Range of strikes as percentage/multiplier of current underlying spot price (e.g. '0.8-1.2', default: '0.8-1.2')
    #[serde(default = "default_strike_range")]
    pub strike_range: Option<String>,
    /// Number of symmetric strikes to include centered around ATM (default: 9, odd integer between 3 and 15)
    #[serde(default = "default_strike_count")]
    pub strike_count: Option<usize>,
    /// Annualized risk-free interest rate (e.g. 0.05 for 5%, default: 0.05, allowed: 0.0 to 0.20)
    #[serde(default = "default_risk_free_rate")]
    pub risk_free_rate: Option<f64>,
    /// Annualized continuous underlying dividend yield (e.g. 0.005 for 0.5%, default: 0.0, allowed: 0.0 to 0.10)
    #[serde(default = "default_dividend_yield")]
    pub dividend_yield: Option<f64>,
}

fn default_strike_range() -> Option<String> {
    Some("0.8-1.2".to_string())
}

fn default_strike_count() -> Option<usize> {
    Some(9)
}

/// A slice of the volatility surface along a single expiration date across multiple strikes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct VolSurfacePoint {
    /// Option expiration date formatted as YYYY-MM-DD
    pub expiration: String,
    /// Solved Black-Scholes implied volatilities corresponding 1-to-1 with the strikes array
    pub ivs: Vec<f64>,
}

/// Response envelope for the Options Volatility Surface endpoint.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct OptionsVolSurfaceResponse {
    /// Target underlying equity ticker symbol
    pub ticker: String,
    /// Underlying asset spot price in USD used for moneyness calculation
    pub spot: f64,
    /// ISO 8601 UTC timestamp of calculation
    pub generated_at: String,
    /// Array of strike prices in USD (columns of the 2D volatility matrix)
    pub strikes: Vec<f64>,
    /// Array of expiration dates in YYYY-MM-DD format (rows of the 2D volatility matrix)
    pub expirations: Vec<String>,
    /// 2D volatility surface grid rows, where each element contains expiration and IV values
    pub surface: Vec<VolSurfacePoint>,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Options Market Microstructure (VPIN & GEX) Models
// ═══════════════════════════════════════════════════════════════════════════════

fn default_metric() -> Option<String> {
    Some("both".to_string())
}

fn default_interval() -> Option<String> {
    Some("daily".to_string())
}

fn default_microstructure_limit() -> Option<usize> {
    Some(100)
}

/// Query parameters for GET /options/microstructure.
#[derive(Debug, Clone, Deserialize, IntoParams, ToSchema)]
pub struct MicrostructureParams {
    /// Target underlying equity ticker symbol (e.g. 'AAPL', 'NVDA', 'SPY')
    pub ticker: String,
    /// Start date for microstructure analysis window (YYYY-MM-DD)
    pub start_date: String,
    /// End date for microstructure analysis window (YYYY-MM-DD)
    pub end_date: String,
    /// Metric to retrieve: 'vpin', 'gex', or 'both' (default: 'both')
    #[serde(default = "default_metric")]
    pub metric: Option<String>,
    /// Time aggregation interval: 'daily' or 'intraday' (default: 'daily')
    #[serde(default = "default_interval")]
    pub interval: Option<String>,
    /// Maximum number of time points to return (1 to 1000, default: 100)
    #[serde(default = "default_microstructure_limit")]
    pub limit: Option<usize>,
}

/// A single microstructure time series data point.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct MicrostructurePoint {
    /// Timestamp of observation (ISO-8601 UTC)
    pub timestamp: String,
    /// Volume-Synchronized Probability of Informed Trading (VPIN, 0.0 to 1.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vpin: Option<f64>,
    /// Net Dealer Gamma Exposure (GEX in dollars, positive indicates long gamma / volatility dampening)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gex: Option<f64>,
}

/// Response envelope for the Options Market Microstructure endpoint.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct MicrostructureResponse {
    /// Target underlying equity ticker symbol
    pub ticker: String,
    /// Start date for analyzed window (YYYY-MM-DD)
    pub start_date: String,
    /// End date for analyzed window (YYYY-MM-DD)
    pub end_date: String,
    /// Requested metric filter ('vpin', 'gex', or 'both')
    pub metric: String,
    /// Time aggregation interval ('daily' or 'intraday')
    pub interval: String,
    /// Array of microstructure observation points
    pub points: Vec<MicrostructurePoint>,
    /// Total count of data points returned
    pub count: usize,
    /// ISO-8601 UTC timestamp of calculation
    pub generated_at: String,
}
