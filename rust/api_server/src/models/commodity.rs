//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Commodity News Sentiment Analytics Models
//! ═══════════════════════════════════════════════════════════════════════════════

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

fn default_commodity() -> Option<String> {
    Some("crude_oil".to_string())
}

fn default_limit() -> Option<usize> {
    Some(10)
}

/// Query parameters for GET /commodities/sentiment.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CommoditySentimentParams {
    /// Target commodity asset (default: "crude_oil").
    /// Supported: "crude_oil", "gold", "copper", "natural_gas", "wheat", "silver".
    #[serde(default = "default_commodity")]
    pub commodity: Option<String>,
    /// Start date for news analysis window (YYYY-MM-DD). Defaults to 30 days ago.
    #[serde(default)]
    pub start_date: Option<String>,
    /// End date for news analysis window (YYYY-MM-DD). Defaults to current date.
    #[serde(default)]
    pub end_date: Option<String>,
    /// Minimum model confidence threshold (0.0 to 1.0, default: 0.0).
    #[serde(default)]
    pub min_confidence: Option<f64>,
    /// Maximum number of top driving news articles to return (1 to 50, default: 10).
    #[serde(default = "default_limit")]
    pub limit: Option<usize>,
}

/// Key driving news article for commodity sentiment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct CommoditySentimentArticle {
    /// Article UUID identifier.
    pub id: String,
    /// Article headline title.
    pub title: String,
    /// Publication source (e.g. "Reuters Commodities", "Bloomberg Energy", "Platts").
    pub source: String,
    /// Publication timestamp (ISO-8601 UTC).
    pub published_utc: String,
    /// Quant sentiment score (-1.0 to 1.0).
    pub sentiment_score: f64,
    /// Sentiment model confidence score (0.0 to 1.0).
    pub confidence: f64,
    /// Source URL or institutional wire link.
    pub url: String,
}

/// Aggregated sentiment metrics for a commodity asset.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct CommoditySentimentSummary {
    /// Confidence-weighted average sentiment score (-1.0 to 1.0).
    pub avg_sentiment: f64,
    /// Total number of relevant news articles mentioning commodity keywords.
    pub mention_count: usize,
    /// Proportion of matching articles with positive sentiment (> 0.1).
    pub positive_ratio: f64,
    /// Proportion of matching articles with negative sentiment (< -0.1).
    pub negative_ratio: f64,
    /// Publication date/timestamp of the most recent matching article.
    pub latest_article_date: Option<String>,
}

/// Complete response payload for the Commodity News Sentiment endpoint.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct CommoditySentimentResponse {
    /// Evaluated canonical commodity identifier (e.g. "crude_oil", "gold").
    pub commodity: String,
    /// Effective start date for analyzed articles (YYYY-MM-DD).
    pub start_date: String,
    /// Effective end date for analyzed articles (YYYY-MM-DD).
    pub end_date: String,
    /// Applied confidence filter threshold.
    pub min_confidence: f64,
    /// Aggregated sentiment summary statistics.
    pub summary: CommoditySentimentSummary,
    /// Top driving news articles matching commodity keywords.
    pub top_articles: Vec<CommoditySentimentArticle>,
    /// ISO-8601 UTC timestamp of response generation.
    pub generated_at: String,
}
