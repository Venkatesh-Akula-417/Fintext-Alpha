//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Portfolio Optimization Data Models
//! ═══════════════════════════════════════════════════════════════════════════════

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Optimization constraints configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct PortfolioConstraints {
    /// Enforce long-only portfolio weights (w_i >= 0, sum(w) = 1.0). Default: true.
    #[serde(default = "default_true")]
    #[schema(example = true)]
    pub long_only: Option<bool>,
}

fn default_true() -> Option<bool> {
    Some(true)
}

impl Default for PortfolioConstraints {
    fn default() -> Self {
        Self {
            long_only: Some(true),
        }
    }
}

/// Request payload for `POST /portfolio/optimize`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PortfolioOptimizeRequest {
    /// Universe of asset ticker symbols (between 2 and 20 constituents).
    #[schema(example = json!(["AAPL", "MSFT", "NVDA"]))]
    pub tickers: Vec<String>,
    /// Historical window start date (YYYY-MM-DD).
    #[schema(example = "2025-01-01")]
    pub start_date: String,
    /// Historical window end date (YYYY-MM-DD).
    #[schema(example = "2025-06-30")]
    pub end_date: String,
    /// Optimization objective ("max_sharpe" or "risk_parity"). Default: "max_sharpe".
    #[serde(default = "default_opt_type")]
    #[schema(example = "max_sharpe")]
    pub optimization_type: Option<String>,
    /// Annualized risk-free rate used for Sharpe ratio calculation (0.0 to 0.10). Default: 0.0.
    #[serde(default)]
    #[schema(example = 0.05)]
    pub risk_free_rate: Option<f64>,
    /// Optional portfolio construction constraints.
    #[serde(default)]
    pub constraints: Option<PortfolioConstraints>,
}

fn default_opt_type() -> Option<String> {
    Some("max_sharpe".to_string())
}

/// Individual constituent asset weight allocation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct PortfolioWeight {
    /// Asset ticker symbol.
    #[schema(example = "AAPL")]
    pub ticker: String,
    /// Optimized portfolio weight allocation (0.0 to 1.0).
    #[schema(example = 0.35)]
    pub weight: f64,
}

/// Response payload for `POST /portfolio/optimize`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct PortfolioOptimizeResponse {
    /// Ordered universe of constituent stock tickers.
    #[schema(example = json!(["AAPL", "MSFT", "NVDA"]))]
    pub tickers: Vec<String>,
    /// Historical analysis window start date (YYYY-MM-DD).
    #[schema(example = "2025-01-01")]
    pub start_date: String,
    /// Historical analysis window end date (YYYY-MM-DD).
    #[schema(example = "2025-06-30")]
    pub end_date: String,
    /// Optimization objective applied ("max_sharpe" or "risk_parity").
    #[schema(example = "max_sharpe")]
    pub optimization_type: String,
    /// Annualized risk-free rate applied.
    #[schema(example = 0.05)]
    pub risk_free_rate: f64,
    /// Optimal asset weight allocations summing to 1.0.
    pub weights: Vec<PortfolioWeight>,
    /// Expected annualized portfolio return ($w^T \mu$).
    #[schema(example = 0.12)]
    pub expected_annual_return: f64,
    /// Expected annualized portfolio volatility ($\sqrt{w^T \Sigma w}$).
    #[schema(example = 0.18)]
    pub expected_annual_volatility: f64,
    /// Annualized Sharpe Ratio ($\frac{w^T \mu - r_f}{\sqrt{w^T \Sigma w}}$).
    #[schema(example = 0.67)]
    pub sharpe_ratio: f64,
    /// ISO 8601 UTC timestamp of calculation.
    #[schema(example = "2025-07-01T12:00:00Z")]
    pub generated_at: String,
}
