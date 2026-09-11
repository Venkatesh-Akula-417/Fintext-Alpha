//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Factor Exposure & Quantitative Risk Models
//! ═══════════════════════════════════════════════════════════════════════════════

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Query parameters for GET /risk/factor-exposure.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct FactorExposureParams {
    /// Stock ticker symbol to analyze (e.g. "AAPL", "MSFT", "NVDA").
    pub ticker: String,
    /// Historical window start date (YYYY-MM-DD).
    pub start_date: String,
    /// Historical window end date (YYYY-MM-DD).
    pub end_date: String,
    /// Comma-separated list of risk factors (default: "market,momentum,sentiment,volatility").
    /// Allowed: "market", "momentum", "sentiment", "volatility", "size", "value".
    #[serde(default)]
    pub factors: Option<String>,
    /// Benchmark ticker symbol used for the market factor (default: "SPY").
    #[serde(default)]
    pub benchmark_ticker: Option<String>,
}

/// Exposure coefficient and statistical significance for a single risk factor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct FactorExposureItem {
    /// Name of the risk factor (e.g. "market", "momentum", "sentiment", "volatility").
    pub factor: String,
    /// Estimated OLS regression beta coefficient (sensitivity).
    pub beta: f64,
    /// Student's t-statistic for the null hypothesis beta == 0.
    pub t_stat: f64,
    /// Two-tailed p-value corresponding to the t-statistic.
    pub p_value: f64,
}

/// Ordinary Least Squares (OLS) regression goodness-of-fit and summary statistics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct OLSStatistics {
    /// Coefficient of determination R^2 (proportion of variance explained by model).
    pub r_squared: f64,
    /// Adjusted R^2 penalized for the number of factor predictors.
    pub adjusted_r_squared: f64,
    /// Number of daily observation periods included in regression estimation.
    pub num_observations: usize,
    /// Overall regression F-statistic.
    pub f_statistic: f64,
    /// Statistical p-value of the overall regression model.
    pub p_value: f64,
}

/// Complete response payload for the Factor Exposure Report endpoint.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct FactorExposureResponse {
    /// Analyzed stock ticker symbol.
    pub ticker: String,
    /// Start date of the analyzed historical window (YYYY-MM-DD).
    pub start_date: String,
    /// End date of the analyzed historical window (YYYY-MM-DD).
    pub end_date: String,
    /// Benchmark ticker symbol used for the market factor.
    pub benchmark_ticker: String,
    /// List of risk factors included in the multiple regression model.
    pub factors_included: Vec<String>,
    /// Model-level OLS summary statistics.
    pub ols_summary: OLSStatistics,
    /// Individual factor exposure betas and statistical significances.
    pub exposures: Vec<FactorExposureItem>,
    /// ISO 8601 UTC timestamp of report generation.
    pub generated_at: String,
}

fn default_lookback_days() -> Option<u32> {
    Some(30)
}

fn default_true() -> Option<bool> {
    Some(true)
}

/// Query parameters for GET /risk/bankruptcy.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BankruptcyRiskParams {
    /// Target equity ticker symbol (e.g. "AAPL", "TSLA").
    pub ticker: String,
    /// Lookback observation window in calendar days (default: 30, min: 1, max: 90).
    #[serde(default = "default_lookback_days")]
    pub lookback_days: Option<u32>,
    /// Whether to include detailed multi-factor component breakdowns (default: true).
    #[serde(default = "default_true")]
    pub include_components: Option<bool>,
}

/// Six-pillar distress component breakdowns.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct BankruptcyComponents {
    /// SEC Form 8-K distress filing score (0.0 to 40.0 pts).
    pub eight_k_distress_score: f64,
    /// Sentiment deterioration z-score penalty (0.0 to 20.0 pts).
    pub sentiment_deterioration_score: f64,
    /// Options put/call volume ratio bearish distress score (0.0 to 15.0 pts).
    pub put_call_ratio_score: f64,
    /// ATM options implied volatility distress score (0.0 to 15.0 pts).
    pub implied_volatility_score: f64,
    /// Supplier / customer contagion graph propagation score (0.0 to 10.0 pts).
    pub supply_chain_risk_score: f64,
    /// Net insider selling pressure score (0.0 to 10.0 pts).
    pub insider_selling_score: f64,
}

/// Complete response payload for the Bankruptcy Risk Signals endpoint.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct BankruptcyRiskResponse {
    /// Target stock ticker symbol.
    pub ticker: String,
    /// Effective lookback period in calendar days.
    pub lookback_days: u32,
    /// Composite bankruptcy risk score on a standardized 0 to 100 scale.
    pub bankruptcy_risk_score: f64,
    /// Risk severity classification: "LOW", "MODERATE", "HIGH", "CRITICAL".
    pub risk_category: String,
    /// Optional breakdown of underlying distress signal components.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub components: Option<BankruptcyComponents>,
    /// ISO-8601 UTC timestamp of risk calculation.
    pub generated_at: String,
}

/// Request payload for POST /risk/portfolio-factor-exposure.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PortfolioFactorExposureRequest {
    /// Portfolio constituent stock ticker symbols (2 to 20 tickers).
    #[schema(example = json!(["AAPL", "MSFT", "NVDA"]))]
    pub tickers: Vec<String>,
    /// Portfolio allocation weights corresponding to tickers (must sum to 1.0 ± 0.05).
    #[schema(example = json!([0.4, 0.3, 0.3]))]
    pub weights: Vec<f64>,
    /// Historical estimation window start date (YYYY-MM-DD).
    #[schema(example = "2025-01-01")]
    pub start_date: String,
    /// Historical estimation window end date (YYYY-MM-DD).
    #[schema(example = "2025-06-30")]
    pub end_date: String,
    /// Benchmark ticker symbol used for the market factor (default: "SPY").
    #[serde(default)]
    #[schema(example = "SPY")]
    pub benchmark_ticker: Option<String>,
    /// Comma-separated list of risk factors (default: "market,momentum,sentiment,volatility").
    /// Allowed: "market", "momentum", "sentiment", "volatility", "size", "value".
    #[serde(default)]
    #[schema(example = "market,momentum,sentiment,volatility")]
    pub factors: Option<String>,
}

/// Complete response payload for the Portfolio Factor Exposure endpoint.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct PortfolioFactorExposureResponse {
    /// Portfolio constituent ticker symbols.
    pub tickers: Vec<String>,
    /// Normalized portfolio allocation weights summing to 1.0.
    pub weights: Vec<f64>,
    /// Start date of the analyzed historical estimation window (YYYY-MM-DD).
    pub start_date: String,
    /// End date of the analyzed historical estimation window (YYYY-MM-DD).
    pub end_date: String,
    /// Benchmark ticker symbol used for the market factor.
    pub benchmark_ticker: String,
    /// List of risk factors included in the multiple regression model.
    pub factors_included: Vec<String>,
    /// Model-level OLS summary statistics.
    pub ols_summary: OLSStatistics,
    /// Portfolio-level factor exposure betas and statistical significances.
    pub exposures: Vec<FactorExposureItem>,
    /// ISO 8601 UTC timestamp of report generation.
    pub generated_at: String,
}
