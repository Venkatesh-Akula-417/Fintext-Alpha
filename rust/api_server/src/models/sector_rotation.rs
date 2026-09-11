//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Sector Rotation Signals Data Models
//! ═══════════════════════════════════════════════════════════════════════════════

use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

/// Query parameters for Sector Rotation Signals (`GET /market/sector-rotation`).
#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct SectorRotationParams {
    /// Number of lookback days for sentiment and price momentum calculation (default: 30, min: 1, max: 90).
    pub lookback_days: Option<i64>,

    /// Whether to include price momentum from daily stock bars (default: true).
    pub include_momentum: Option<bool>,

    /// Number of top and bottom sectors to flag as outperform/underperform (default: 3, min: 1, max: 10).
    pub top_n: Option<usize>,

    /// Minimum sentiment confidence threshold between 0.0 and 1.0 (default: 0.0).
    pub min_confidence: Option<f64>,
}

/// Individual sector rotation signal and relative strength ranking.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct SectorRotationItem {
    /// GICS sector display name (e.g., "Technology", "Financials").
    pub sector: String,

    /// Average normalized sentiment score over the lookback window (-1.0 to 1.0).
    pub sentiment_trend: f64,

    /// Sentiment momentum: recent sub-window average minus baseline sub-window average.
    pub sentiment_momentum: f64,

    /// Average daily return of sector constituents over the lookback window.
    pub price_momentum: f64,

    /// Average pairwise Pearson correlation of daily returns among sector constituents (0.0 to 1.0).
    pub correlation_score: f64,

    /// Composite relative strength score normalized to [0.0, 1.0].
    pub relative_strength: f64,

    /// Ordinal rank by relative strength (1 = strongest).
    pub rank: usize,

    /// Signal flag: "outperform", "underperform", or "neutral".
    pub flag: String,

    /// ML model version tag.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_version: Option<String>,

    /// Feature extraction pipeline version.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pipeline_version: Option<String>,

    /// Upstream data source lineage.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_provenance: Option<Vec<String>>,
}

/// Response payload for Sector Rotation Signals (`GET /market/sector-rotation`).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct SectorRotationResponse {
    /// Number of lookback days used for the analysis.
    pub lookback_days: i64,

    /// Whether price momentum was included in the composite scoring.
    pub include_momentum: bool,

    /// Number of sectors flagged as top/bottom performers.
    pub top_n: usize,

    /// Array of sector rotation items sorted by relative strength descending.
    pub sectors: Vec<SectorRotationItem>,

    /// Sector names flagged as outperform (top_n strongest).
    pub outperform_sectors: Vec<String>,

    /// Sector names flagged as underperform (top_n weakest).
    pub underperform_sectors: Vec<String>,

    /// Macro rotation signal: "risk-on", "risk-off", or "neutral".
    pub market_signal: String,

    /// ISO-8601 UTC timestamp when this analysis was generated.
    pub generated_at: String,
}
