//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Market Regime Detection Data Models
//! ═══════════════════════════════════════════════════════════════════════════════

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use utoipa::{IntoParams, ToSchema};

/// Query parameters for Market Regime Detection (`GET /market/regime`).
#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct MarketRegimeParams {
    /// Number of lookback days for computing aggregate market statistics (default: 5, min: 1, max: 30).
    pub lookback_days: Option<i64>,

    /// Optional sector weighting string: e.g. "Technology:0.3,Financials:0.2,Healthcare:0.2"
    /// or comma-separated float values matching canonical sector order. Defaults to equal weighting.
    pub sector_weights: Option<String>,

    /// Minimum number of sentiment records required for high statistical confidence (default: 50, min: 1, max: 1000).
    pub min_data_points: Option<usize>,
}

/// Granular underlying components contributing to market regime synthesis.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct RegimeComponents {
    /// Average sentiment score per GICS sector over the lookback window.
    pub sector_sentiments: HashMap<String, f64>,

    /// Total number of raw sentiment records evaluated in the analysis window.
    pub total_data_points: usize,

    /// Ratio of evaluated tickers with strictly positive sentiment (> 0.0).
    pub positive_sentiment_ratio: f64,

    /// Number of tickers with positive aggregate sentiment (> 0.0).
    pub bullish_tickers_count: usize,

    /// Number of tickers with negative aggregate sentiment (< 0.0).
    pub bearish_tickers_count: usize,

    /// Number of tickers with neutral aggregate sentiment (== 0.0).
    pub neutral_tickers_count: usize,
}

/// Response payload for Market Regime Detection (`GET /market/regime`).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct MarketRegimeResponse {
    /// Classified market regime: "Bullish", "Bearish", "Neutral", or "High Volatility".
    pub regime: String,

    /// Statistical confidence score between 0.0 and 1.0 based on data coverage and signal clarity.
    pub confidence: f64,

    /// Composite cross-sector aggregate market sentiment score between -1.0 and 1.0.
    pub market_sentiment: f64,

    /// Market breadth: proportion of universe assets with positive sentiment (0.0 to 1.0).
    pub breadth: f64,

    /// Average pairwise absolute spillover correlation across representative universe (0.0 to 1.0).
    pub avg_spillover_corr: f64,

    /// Proxy measure for market sentiment dispersion and implied volatility.
    pub volatility_proxy: f64,

    /// Number of lookback days utilized in the evaluation.
    pub lookback_days: i64,

    /// ISO-8601 UTC timestamp when this regime analysis was synthesized.
    pub generated_at: String,

    /// Underlying sector sentiment and market breadth components.
    pub components: RegimeComponents,
}
