//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Return Correlation Matrix Models
//! ═══════════════════════════════════════════════════════════════════════════════

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Query parameters for Return Correlation Matrix endpoint (`GET /market/correlation`).
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct ReturnCorrelationParams {
    /// Comma-separated list of ticker symbols (max 50, e.g., "AAPL,MSFT,NVDA,AMZN")
    #[schema(example = "AAPL,MSFT,NVDA,AMZN")]
    pub tickers: String,

    /// Start date for historical correlation window (YYYY-MM-DD)
    #[schema(example = "2025-01-01")]
    pub start_date: String,

    /// End date for historical correlation window (YYYY-MM-DD)
    #[schema(example = "2025-03-31")]
    pub end_date: String,

    /// Minimum number of overlapping daily returns required to compute correlation (default: 20, min: 10)
    #[schema(example = 20)]
    pub min_periods: Option<usize>,

    /// Whether to include self-correlation pairs (e.g. AAPL-AAPL = 1.0) in the matrix (default: false)
    #[schema(example = false)]
    pub include_self: Option<bool>,
}

/// Pairwise daily return correlation entry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct ReturnCorrelationItem {
    /// First ticker in the pairwise relation
    #[schema(example = "AAPL")]
    pub ticker_a: String,

    /// Second ticker in the pairwise relation
    #[schema(example = "MSFT")]
    pub ticker_b: String,

    /// Pearson correlation coefficient between -1.0 and +1.0 (None/null if insufficient data)
    #[schema(example = 0.72)]
    pub correlation: Option<f64>,

    /// Number of common daily return periods utilized in computation
    #[schema(example = 60)]
    pub periods: usize,
}

/// Response envelope for daily return correlation matrix.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct ReturnCorrelationResponse {
    /// Evaluated list of tickers in the matrix
    #[schema(example = json!(["AAPL", "MSFT", "NVDA", "AMZN"]))]
    pub tickers: Vec<String>,

    /// Applied start date in ISO format (YYYY-MM-DD)
    #[schema(example = "2025-01-01")]
    pub start_date: String,

    /// Applied end date in ISO format (YYYY-MM-DD)
    #[schema(example = "2025-03-31")]
    pub end_date: String,

    /// Minimum overlapping periods required for valid correlation
    #[schema(example = 20)]
    pub min_periods: usize,

    /// Array of pairwise correlation metrics
    pub matrix: Vec<ReturnCorrelationItem>,

    /// ISO 8601 UTC timestamp of matrix generation
    #[schema(example = "2026-08-30T10:30:00.000000Z")]
    pub generated_at: String,
}
