use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Single daily progression point on the portfolio equity curve.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct EquityPoint {
    /// Date of the equity calculation (YYYY-MM-DD)
    #[schema(example = "2025-01-01")]
    pub date: String,
    /// Portfolio cash/equity valuation in USD
    #[schema(example = 1000000.0)]
    pub portfolio_value: f64,
    /// Net daily percentage return of the portfolio
    #[serde(default)]
    #[schema(example = 0.0045)]
    pub daily_return: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BacktestRequest {
    /// Target single asset ticker symbol (legacy / single-asset compatibility)
    #[serde(default)]
    #[schema(example = "AAPL")]
    pub ticker: Option<String>,
    /// Portfolio ticker symbols (1 to 10 tickers; overrides or supplements single ticker)
    #[serde(default)]
    #[schema(example = json!(["AAPL", "NVDA", "MSFT"]))]
    pub tickers: Option<Vec<String>>,
    /// Portfolio weight allocation per ticker (sum should equal 1.0; defaults to equal weight)
    #[serde(default)]
    #[schema(example = json!([0.4, 0.3, 0.3]))]
    pub weights: Option<Vec<f64>>,
    /// Benchmark asset ticker for relative performance and alpha tracking (default: "SPY")
    #[serde(default = "default_benchmark_ticker_opt")]
    #[schema(example = "SPY")]
    pub benchmark_ticker: Option<String>,
    /// Transaction cost in basis points (bps) per trade/turnover (default: 5.0 bps = 0.05%, max: 100 bps)
    #[serde(default = "default_transaction_cost_bps_opt")]
    #[schema(example = 5.0)]
    pub transaction_cost_bps: Option<f64>,
    /// Backtest simulation start date (YYYY-MM-DD)
    #[schema(example = "2025-01-01")]
    pub start_date: String,
    /// Backtest simulation end date (YYYY-MM-DD)
    #[schema(example = "2025-03-31")]
    pub end_date: String,
    /// Sentiment score threshold to enter a LONG position (default: 0.2)
    #[serde(default = "default_long_threshold")]
    #[schema(example = 0.2)]
    pub long_threshold: f64,
    /// Sentiment score threshold to enter a SHORT position (default: -0.2)
    #[serde(default = "default_short_threshold")]
    #[schema(example = -0.2)]
    pub short_threshold: f64,
    /// Maximum days to hold a position before reverting to flat (default: 5)
    #[serde(default = "default_holding_days")]
    #[schema(example = 5)]
    pub holding_days: i64,
    /// Starting portfolio cash in USD (default: $1,000,000.0)
    #[serde(default = "default_initial_capital")]
    #[schema(example = 1000000.0)]
    pub initial_capital: f64,
}

fn default_benchmark_ticker_opt() -> Option<String> {
    Some("SPY".to_string())
}

fn default_transaction_cost_bps_opt() -> Option<f64> {
    Some(5.0)
}

fn default_benchmark_ticker_val() -> String {
    "SPY".to_string()
}

fn default_transaction_cost_bps_val() -> f64 {
    5.0
}

fn default_profit_factor() -> f64 {
    1.0
}

fn default_long_threshold() -> f64 {
    0.2
}

fn default_short_threshold() -> f64 {
    -0.2
}

fn default_holding_days() -> i64 {
    5
}

fn default_initial_capital() -> f64 {
    1_000_000.0
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct BacktestResponse {
    /// Target asset ticker symbol (or primary ticker / portfolio summary)
    #[schema(example = "AAPL")]
    pub ticker: String,
    /// List of portfolio ticker symbols evaluated
    #[serde(default)]
    #[schema(example = json!(["AAPL", "NVDA", "MSFT"]))]
    pub tickers: Vec<String>,
    /// Portfolio weight allocation per ticker
    #[serde(default)]
    #[schema(example = json!([0.4, 0.3, 0.3]))]
    pub weights: Vec<f64>,
    /// Benchmark asset ticker symbol
    #[serde(default = "default_benchmark_ticker_val")]
    #[schema(example = "SPY")]
    pub benchmark_ticker: String,
    /// Backtest simulation start date
    #[schema(example = "2025-01-01")]
    pub start_date: String,
    /// Backtest simulation end date
    #[schema(example = "2025-03-31")]
    pub end_date: String,
    /// Net total portfolio return percentage (e.g., 0.185 = +18.5%)
    #[schema(example = 0.142)]
    pub total_return: f64,
    /// Annualized portfolio return assuming 252 trading days per year
    #[schema(example = 0.568)]
    pub annualized_return: f64,
    /// Annualized Sharpe Ratio (risk-free rate = 0.0)
    #[schema(example = 2.15)]
    pub sharpe_ratio: f64,
    /// Annualized Sortino Ratio (downside deviation with target return = 0.0)
    #[serde(default)]
    #[schema(example = 3.42)]
    pub sortino_ratio: f64,
    /// Maximum peak-to-trough equity drawdown percentage
    #[schema(example = -0.048)]
    pub max_drawdown: f64,
    /// Total number of simulated trade positions/rebalances executed
    #[schema(example = 13)]
    pub num_trades: usize,
    /// Percentage of closed trades with positive net returns (0.0 to 100.0)
    #[schema(example = 69.2)]
    pub win_rate: f64,
    /// Profit Factor (Gross Profits / Gross Losses)
    #[serde(default = "default_profit_factor")]
    #[schema(example = 2.45)]
    pub profit_factor: f64,
    /// Transaction cost in basis points (bps) applied per trade
    #[serde(default = "default_transaction_cost_bps_val")]
    #[schema(example = 5.0)]
    pub transaction_cost_bps: f64,
    /// Daily portfolio equity progression values starting from initial_capital
    #[schema(example = json!([1000000.0, 1004500.0, 1012000.0, 1025000.0, 1142000.0]))]
    pub equity_curve: Vec<f64>,
    /// Daily detailed equity points with date, portfolio value, and daily return
    #[serde(default)]
    pub equity_points: Vec<EquityPoint>,
    /// Benchmark cumulative equity progression values starting from initial_capital
    #[serde(default)]
    pub benchmark_curve: Vec<f64>,
    /// Benchmark total return percentage over the simulation horizon
    #[serde(default)]
    #[schema(example = 0.085)]
    pub benchmark_total_return: f64,
    /// Excess return (Alpha) generated relative to benchmark
    #[serde(default)]
    #[schema(example = 0.057)]
    pub alpha: f64,
    /// Summary and institutional notes on backtest methodology
    #[schema(
        example = "Institutional point-in-time multi-asset portfolio backtest simulated successfully"
    )]
    pub message: String,
}
