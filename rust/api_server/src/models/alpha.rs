//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Alpha Validation Report & Strategy Performance DTO Models
//! ═══════════════════════════════════════════════════════════════════════════════

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

fn default_benchmark() -> Option<String> {
    Some("SPY".to_string())
}

fn default_initial_capital() -> Option<f64> {
    Some(1_000_000.0)
}

/// Strategy Signal Generation Parameters.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct AlphaSignalConfig {
    /// Signal type indicator, e.g. 'sentiment', 'sentiment_anomaly', 'esg'
    #[schema(example = "sentiment")]
    pub signal_type: String,

    /// Sentiment score threshold to enter a Long position (e.g. 0.2)
    #[schema(example = 0.2)]
    pub threshold_long: f64,

    /// Sentiment score threshold to enter a Short position (e.g. -0.2)
    #[schema(example = -0.2)]
    pub threshold_short: f64,

    /// Minimum holding horizon in calendar days before taking profit/rebalancing
    #[schema(example = 5)]
    pub holding_days: u32,

    /// Optional rolling moving average smoothing window in days (e.g. 3)
    #[schema(example = 3)]
    #[serde(default)]
    pub smoothing_window_days: Option<u32>,
}

impl Default for AlphaSignalConfig {
    fn default() -> Self {
        Self {
            signal_type: "sentiment".to_string(),
            threshold_long: 0.2,
            threshold_short: -0.2,
            holding_days: 5,
            smoothing_window_days: None,
        }
    }
}

/// Alpha Strategy Validation Backtest Request.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AlphaReportRequest {
    /// Target constituent ticker symbols (1 to 10 tickers)
    #[schema(example = json!(["AAPL", "MSFT", "NVDA"]))]
    pub tickers: Vec<String>,

    /// Start date for historical evaluation in YYYY-MM-DD format
    #[schema(example = "2024-01-01")]
    pub start_date: String,

    /// End date for historical evaluation in YYYY-MM-DD format
    #[schema(example = "2024-12-31")]
    pub end_date: String,

    /// Signal generation and threshold configuration
    pub signal_config: AlphaSignalConfig,

    /// Benchmark asset ticker for relative performance attribution (default: 'SPY')
    #[schema(example = "SPY")]
    #[serde(default = "default_benchmark")]
    pub benchmark_ticker: Option<String>,

    /// Initial portfolio capital balance in USD (default: $1,000,000.00)
    #[schema(example = 1000000.0)]
    #[serde(default = "default_initial_capital")]
    pub initial_capital: Option<f64>,
}

/// Comprehensive Institutional Performance and Risk Attribution Metrics.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct PerformanceMetrics {
    /// Total cumulative strategy return over the backtest period (e.g. 0.245 = +24.5%)
    #[schema(example = 0.2450)]
    pub total_return: f64,

    /// Compound annualized strategy return (252 trading days per year basis)
    #[schema(example = 0.2475)]
    pub annualized_return: f64,

    /// Annualized strategy standard deviation / volatility
    #[schema(example = 0.1420)]
    pub annualized_volatility: f64,

    /// Annualized Sharpe ratio (risk-free rate = 0.0)
    #[schema(example = 1.74)]
    pub sharpe_ratio: f64,

    /// Annualized Sortino ratio (downside risk deviation basis)
    #[schema(example = 2.45)]
    pub sortino_ratio: f64,

    /// Maximum peak-to-trough portfolio equity drawdown (e.g. 0.068 = -6.8%)
    #[schema(example = 0.0680)]
    pub max_drawdown: f64,

    /// Percentage of closed round-trip trades with positive net returns
    #[schema(example = 64.5)]
    pub win_rate: f64,

    /// Ratio of gross winning profits to gross losing losses
    #[schema(example = 1.85)]
    pub profit_factor: f64,

    /// Total number of position transitions / trades executed
    #[schema(example = 28)]
    pub total_trades: usize,

    /// Average holding period per trade in calendar days
    #[schema(example = 4.8)]
    pub avg_holding_period_days: f64,

    /// Benchmark ticker symbol used for relative comparison
    #[schema(example = "SPY")]
    pub benchmark_ticker: String,

    /// Total cumulative benchmark return over the period
    #[schema(example = 0.1250)]
    pub benchmark_total_return: f64,

    /// Compound annualized benchmark return
    #[schema(example = 0.1265)]
    pub benchmark_annualized_return: f64,

    /// Annualized benchmark standard deviation / volatility
    #[schema(example = 0.1580)]
    pub benchmark_annualized_volatility: f64,

    /// Net excess alpha return over benchmark (Strategy Total Return - Benchmark Total Return)
    #[schema(example = 0.1200)]
    pub alpha: f64,

    /// Systematic market sensitivity beta from OLS linear regression vs. benchmark
    #[schema(example = 0.82)]
    pub beta: f64,

    /// Information Ratio (Annualized Active Return / Annualized Tracking Error)
    #[schema(example = 1.15)]
    pub information_ratio: f64,

    /// Annualized standard deviation of excess daily returns (Tracking Error)
    #[schema(example = 0.0980)]
    pub tracking_error: f64,
}

/// Point-in-Time Daily Equity and Position State.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct EquityCurvePoint {
    /// Date of observation in YYYY-MM-DD format
    #[schema(example = "2024-01-02")]
    pub date: String,

    /// Total mark-to-market strategy portfolio equity value in USD
    #[schema(example = 1002500.0)]
    pub portfolio_value: f64,

    /// Mark-to-market benchmark equity value in USD
    #[schema(example = 1001200.0)]
    pub benchmark_value: f64,

    /// Net daily portfolio strategy return
    #[schema(example = 0.0025)]
    pub strategy_daily_return: f64,

    /// Daily benchmark asset return
    #[schema(example = 0.0012)]
    pub benchmark_daily_return: f64,

    /// Aggregated portfolio position state (-1 = Short, 0 = Flat, 1 = Long)
    #[schema(example = 1)]
    pub position: i32,
}

/// Response payload containing complete Alpha Validation Report, Risk Metrics, and Equity Curves.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AlphaReportResponse {
    /// Portfolio constituent ticker symbols analyzed
    pub tickers: Vec<String>,

    /// Start date of backtest evaluation (YYYY-MM-DD)
    pub start_date: String,

    /// End date of backtest evaluation (YYYY-MM-DD)
    pub end_date: String,

    /// Evaluated signal configuration
    pub signal_config: AlphaSignalConfig,

    /// Starting capital allocation
    pub initial_capital: f64,

    /// Summary risk and return performance metrics
    pub metrics: PerformanceMetrics,

    /// Complete daily equity curve and active position timeseries
    pub equity_curve: Vec<EquityCurvePoint>,

    /// ISO-8601 UTC timestamp when this report was compiled
    pub generated_at: String,

    /// Operational diagnostic and data source attribution message
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alpha_models_serialization() {
        let req = AlphaReportRequest {
            tickers: vec!["AAPL".to_string(), "MSFT".to_string()],
            start_date: "2024-01-01".to_string(),
            end_date: "2024-12-31".to_string(),
            signal_config: AlphaSignalConfig {
                signal_type: "sentiment".to_string(),
                threshold_long: 0.25,
                threshold_short: -0.25,
                holding_days: 7,
                smoothing_window_days: Some(3),
            },
            benchmark_ticker: Some("SPY".to_string()),
            initial_capital: Some(500_000.0),
        };

        let json_str = serde_json::to_string(&req).expect("Failed to serialize request");
        let parsed: AlphaReportRequest =
            serde_json::from_str(&json_str).expect("Failed to deserialize request");

        assert_eq!(parsed.tickers.len(), 2);
        assert_eq!(parsed.signal_config.holding_days, 7);
        assert_eq!(parsed.signal_config.smoothing_window_days, Some(3));
    }
}
