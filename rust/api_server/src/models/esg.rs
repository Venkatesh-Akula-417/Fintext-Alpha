//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — ESG Sentiment & Sustainability Analytics Models
//! ═══════════════════════════════════════════════════════════════════════════════

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Query parameters for GET /esg/scores.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ESGScoresParams {
    /// Optional equity ticker filter (e.g. "AAPL", "MSFT", "TSLA").
    #[serde(default)]
    pub ticker: Option<String>,
    /// Optional GICS sector name filter (e.g. "Technology", "Energy", "Financials").
    #[serde(default)]
    pub sector: Option<String>,
    /// Historical window start date (YYYY-MM-DD). Defaults to 90 days ago.
    #[serde(default)]
    pub start_date: Option<String>,
    /// Historical window end date (YYYY-MM-DD). Defaults to current date.
    #[serde(default)]
    pub end_date: Option<String>,
    /// Minimum model confidence threshold (0.0 to 1.0, default: 0.0).
    #[serde(default)]
    pub min_confidence: Option<f64>,
}

/// Quantitative metrics for a single ESG pillar dimension (Environmental, Social, Governance).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct ESGDimensionScore {
    /// Confidence-weighted average sentiment score across matching articles (-1.0 to 1.0).
    pub score: f64,
    /// Total number of articles mentioning this dimension's keywords.
    pub mention_count: usize,
    /// Proportion of matching articles with positive sentiment (> 0.1).
    pub positive_ratio: f64,
    /// Proportion of matching articles with negative sentiment (< -0.1).
    pub negative_ratio: f64,
}

/// Composite container for the three ESG pillars.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct ESGDimensions {
    /// Environmental dimension score and metrics.
    pub environmental: ESGDimensionScore,
    /// Social dimension score and metrics.
    pub social: ESGDimensionScore,
    /// Governance dimension score and metrics.
    pub governance: ESGDimensionScore,
}

/// Complete response payload for the ESG Sentiment Scores endpoint.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct ESGScoresResponse {
    /// Target stock ticker symbol, if ticker-level query.
    pub ticker: Option<String>,
    /// Target sector name, if sector-level query.
    pub sector: Option<String>,
    /// Effective start date for analyzed articles (YYYY-MM-DD).
    pub start_date: String,
    /// Effective end date for analyzed articles (YYYY-MM-DD).
    pub end_date: String,
    /// Applied confidence filter threshold (0.0 to 1.0).
    pub min_confidence: f64,
    /// Overall composite ESG score on a standardized 0 to 100 scale.
    pub overall_esg_score: f64,
    /// Pillar-by-pillar ESG dimension breakdown.
    pub dimensions: ESGDimensions,
    /// ISO-8601 UTC timestamp of score computation.
    pub generated_at: String,
}
