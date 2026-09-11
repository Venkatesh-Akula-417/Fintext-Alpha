//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Dataset Export Request Models
//! ═══════════════════════════════════════════════════════════════════════════════

use serde::Deserialize;
use utoipa::ToSchema;

/// Request parameters for historical Apache Parquet dataset export (`GET /export/parquet`).
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct ExportParquetParams {
    /// Target stock ticker symbol (e.g., 'AAPL', 'NVDA', 'MSFT')
    #[schema(example = "AAPL")]
    pub ticker: String,
    /// Start date formatted as YYYY-MM-DD
    #[schema(example = "2025-01-01")]
    pub start_date: String,
    /// End date formatted as YYYY-MM-DD
    #[schema(example = "2025-03-31")]
    pub end_date: String,
    /// Maximum number of records to export (default: 10000, max: 100000)
    #[schema(example = 10000)]
    pub limit: Option<u32>,
    /// Minimum data quality score filter threshold between 0.0 and 1.0 (default: 0.0)
    #[schema(example = 0.70)]
    pub min_quality: Option<f32>,
    /// Whether to left-join daily closing stock prices on date (default: false)
    #[schema(example = true)]
    pub include_prices: Option<bool>,
}
