//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — FX Sentiment & Currency Pair Analytics Models
//! ═══════════════════════════════════════════════════════════════════════════════

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

fn default_currency_pair() -> Option<String> {
    Some("EUR/USD".to_string())
}

fn default_limit() -> Option<usize> {
    Some(20)
}

/// Query parameters for GET /fx/sentiment.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct FXSentimentParams {
    /// Target major currency pair (default: "EUR/USD").
    /// Supported: "EUR/USD", "USD/JPY", "GBP/USD", "USD/CHF", "AUD/USD", "USD/CAD", "NZD/USD".
    #[serde(default = "default_currency_pair")]
    pub currency_pair: Option<String>,
    /// Start date for news analysis window (YYYY-MM-DD). Defaults to 30 days ago.
    #[serde(default)]
    pub start_date: Option<String>,
    /// End date for news analysis window (YYYY-MM-DD). Defaults to current date.
    #[serde(default)]
    pub end_date: Option<String>,
    /// Minimum model confidence threshold (0.0 to 1.0, default: 0.0).
    #[serde(default)]
    pub min_confidence: Option<f64>,
    /// Maximum number of top driving news articles to return (1 to 100, default: 20).
    #[serde(default = "default_limit")]
    pub limit: Option<usize>,
}

/// Key driving news article for currency pair sentiment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct FXSentimentArticle {
    /// Article UUID identifier.
    pub id: String,
    /// Article headline title.
    pub title: String,
    /// Publication source (e.g. "Bloomberg", "Reuters FX", "Financial Times").
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

/// Aggregated sentiment metrics for a currency pair.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct FXSentimentSummary {
    /// Confidence-weighted average sentiment score (-1.0 to 1.0).
    pub avg_sentiment: f64,
    /// Total number of relevant news articles mentioning currency pair keywords.
    pub mention_count: usize,
    /// Proportion of matching articles with positive sentiment (> 0.1).
    pub positive_ratio: f64,
    /// Proportion of matching articles with negative sentiment (< -0.1).
    pub negative_ratio: f64,
    /// Publication date/timestamp of the most recent matching article.
    pub latest_article_date: Option<String>,
}

/// Complete response payload for the FX Sentiment Feed endpoint.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct FXSentimentResponse {
    /// Evaluated canonical currency pair symbol (e.g. "EUR/USD", "USD/JPY").
    pub currency_pair: String,
    /// Effective start date for analyzed articles (YYYY-MM-DD).
    pub start_date: String,
    /// Effective end date for analyzed articles (YYYY-MM-DD).
    pub end_date: String,
    /// Applied confidence filter threshold.
    pub min_confidence: f64,
    /// Aggregated sentiment summary statistics.
    pub summary: FXSentimentSummary,
    /// Top driving news articles matching currency pair keywords.
    pub top_articles: Vec<FXSentimentArticle>,
    /// ISO-8601 UTC timestamp of response generation.
    pub generated_at: String,
}
