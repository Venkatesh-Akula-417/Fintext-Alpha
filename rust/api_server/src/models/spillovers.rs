use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Deserialize)]
pub struct SpilloverQuery {
    pub ticker: String,
    pub limit: Option<usize>,
    pub min_correlation: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct SpilloverItem {
    /// Associated correlated asset ticker
    #[schema(example = "MSFT")]
    pub related_ticker: String,
    /// Time lag in hours between asset movements (positive = leading, negative = lagging)
    #[schema(example = 1)]
    pub lag_hours: i64,
    /// Lead-lag Pearson correlation coefficient
    #[schema(example = 0.745)]
    pub correlation: f64,
    /// Human-readable relationship summary
    #[schema(example = "AAPL LEADS MSFT by 1h")]
    pub relationship: String,
    /// ISO-8601 update timestamp
    #[schema(example = "2026-08-28T23:59:00Z")]
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct SpilloverResponse {
    /// Query target ticker symbol
    #[schema(example = "AAPL")]
    pub ticker: String,
    /// Ranked list of cross-asset spillover correlations
    pub spillovers: Vec<SpilloverItem>,
    /// Number of spillover items returned
    #[schema(example = 2)]
    pub count: usize,
    /// Query execution status
    #[schema(example = "ok")]
    pub status: String,
    /// Diagnostic summary message
    #[schema(example = "Cross-asset spillover graph computed successfully")]
    pub message: String,
}

/// Query parameters for cross-asset spillover correlation matrix (`GET /spillovers/matrix`).
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct SpilloverMatrixParams {
    /// Optional comma-separated list of stock tickers (e.g. 'AAPL,MSFT,NVDA,AMZN'). Max 50 tickers.
    #[schema(example = "AAPL,MSFT,NVDA")]
    pub tickers: Option<String>,
    /// Start date in ISO format YYYY-MM-DD (e.g. '2025-01-01')
    #[schema(example = "2025-01-01")]
    pub start_date: String,
    /// End date in ISO format YYYY-MM-DD (e.g. '2025-03-31')
    #[schema(example = "2025-03-31")]
    pub end_date: String,
    /// Optional minimum absolute correlation threshold (-1.0 to 1.0, default: 0.0)
    #[schema(example = 0.5)]
    pub min_correlation: Option<f64>,
    /// Optional maximum lead-lag offset window in hours (default: 24, max: 168)
    #[schema(example = 24)]
    pub max_lag_hours: Option<i64>,
}

/// Pairwise cross-asset spillover correlation relationship.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct SpilloverMatrixItem {
    /// Primary asset ticker symbol
    #[schema(example = "AAPL")]
    pub ticker_a: String,
    /// Associated correlated asset ticker symbol
    #[schema(example = "MSFT")]
    pub ticker_b: String,
    /// Pearson cross-correlation coefficient at optimal lag
    #[schema(example = 0.75)]
    pub correlation: f64,
    /// Optimal lead-lag offset in hours (positive => A leads B; negative => B leads A)
    #[schema(example = 1)]
    pub lag_hours: i64,
    /// Lead-lag direction tag (e.g. 'AAPL_leads_MSFT', 'NVDA_leads_AAPL', 'contemporaneous', 'self')
    #[schema(example = "AAPL_leads_MSFT")]
    pub direction: String,
}

/// Response payload for cross-asset spillover matrix query (`GET /spillovers/matrix`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct SpilloverMatrixResponse {
    /// Evaluated list of stock ticker symbols
    #[schema(example = "[\"AAPL\", \"MSFT\", \"NVDA\"]")]
    pub tickers: Vec<String>,
    /// Start date of analyzed window
    #[schema(example = "2025-01-01")]
    pub start_date: String,
    /// End date of analyzed window
    #[schema(example = "2025-03-31")]
    pub end_date: String,
    /// Minimum correlation threshold applied
    #[schema(example = 0.5)]
    pub min_correlation: f64,
    /// Maximum lag window in hours applied
    #[schema(example = 24)]
    pub max_lag_hours: i64,
    /// Computed pairwise lead-lag correlation matrix items
    pub matrix: Vec<SpilloverMatrixItem>,
    /// Number of pairwise matrix relationships returned
    #[schema(example = 3)]
    pub count: usize,
    /// ISO-8601 generation timestamp
    #[schema(example = "2026-08-28T20:15:00Z")]
    pub generated_at: String,
}
