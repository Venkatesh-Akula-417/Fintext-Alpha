//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Market Breadth & Advance/Decline Models
//! ═══════════════════════════════════════════════════════════════════════════════

use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

/// Query parameters for Market Breadth & Advance/Decline endpoint (`GET /market/breadth`).
#[derive(Debug, Clone, Deserialize, IntoParams, ToSchema)]
pub struct MarketBreadthParams {
    /// Start date for historical breadth window (YYYY-MM-DD)
    #[schema(example = "2025-01-01")]
    pub start_date: String,

    /// End date for historical breadth window (YYYY-MM-DD)
    #[schema(example = "2025-03-31")]
    pub end_date: String,

    /// Filter universe: 'all' (default), 'sp500', or comma-separated list of tickers (max 100)
    #[serde(default = "default_universe")]
    #[schema(example = "all")]
    pub universe: Option<String>,

    /// Maximum number of observation days to return (1 to 200, default: 50)
    #[serde(default = "default_breadth_limit")]
    #[schema(example = 50)]
    pub limit: Option<usize>,

    /// Whether to calculate and include new 52-week highs and lows (default: true)
    #[serde(default = "default_include_new_highs_lows")]
    #[schema(example = true)]
    pub include_new_highs_lows: Option<bool>,
}

fn default_universe() -> Option<String> {
    Some("all".to_string())
}

fn default_breadth_limit() -> Option<usize> {
    Some(50)
}

fn default_include_new_highs_lows() -> Option<bool> {
    Some(true)
}

/// A single daily observation point of market breadth metrics.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct MarketBreadthPoint {
    /// Observation date in YYYY-MM-DD format
    #[schema(example = "2025-01-02")]
    pub date: String,

    /// Number of tickers whose close price increased from previous session
    #[schema(example = 300)]
    pub advancers: usize,

    /// Number of tickers whose close price decreased from previous session
    #[schema(example = 180)]
    pub decliners: usize,

    /// Number of tickers whose close price remained unchanged
    #[schema(example = 20)]
    pub unchanged: usize,

    /// Advance/Decline ratio: advancers / (advancers + decliners)
    #[schema(example = 0.625)]
    pub advance_decline_ratio: f64,

    /// Market breadth index: (advancers - decliners) / total_tickers (scaled -1.0 to +1.0)
    #[schema(example = 0.24)]
    pub breadth_index: f64,

    /// Number of tickers achieving a new 52-week high
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = 25)]
    pub new_52w_highs: Option<usize>,

    /// Number of tickers achieving a new 52-week low
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = 10)]
    pub new_52w_lows: Option<usize>,
}

/// Response payload for Market Breadth & Advance/Decline (`GET /market/breadth`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct MarketBreadthResponse {
    /// Start date for analyzed window (YYYY-MM-DD)
    #[schema(example = "2025-01-01")]
    pub start_date: String,

    /// End date for analyzed window (YYYY-MM-DD)
    #[schema(example = "2025-03-31")]
    pub end_date: String,

    /// Evaluated constituent universe ('all', 'sp500', or custom list)
    #[schema(example = "all")]
    pub universe: String,

    /// Total count of evaluated tickers in the universe
    #[schema(example = 500)]
    pub total_tickers: usize,

    /// Array of daily market breadth data points
    pub points: Vec<MarketBreadthPoint>,

    /// Number of observation points returned
    #[schema(example = 50)]
    pub count: usize,

    /// ISO-8601 UTC timestamp of calculation
    #[schema(example = "2026-08-31T15:00:00.000000Z")]
    pub generated_at: String,
}
