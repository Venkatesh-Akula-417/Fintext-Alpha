//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Crypto News Sentiment Analytics Models
//! ═══════════════════════════════════════════════════════════════════════════════

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

fn default_crypto_asset() -> Option<String> {
    Some("BTC".to_string())
}

fn default_limit() -> Option<usize> {
    Some(10)
}

/// Query parameters for GET /crypto/sentiment.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CryptoSentimentParams {
    /// Target cryptocurrency asset symbol (default: "BTC").
    /// Supported: "BTC", "ETH", "SOL", "BNB", "XRP", "ADA".
    #[serde(default = "default_crypto_asset")]
    pub asset: Option<String>,
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

/// Key driving news article for cryptocurrency sentiment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct CryptoSentimentArticle {
    /// Article UUID identifier.
    pub id: String,
    /// Article headline title.
    pub title: String,
    /// Publication source (e.g. "CoinDesk", "CoinTelegraph", "Bloomberg Crypto", "Decrypt").
    pub source: String,
    /// Publication timestamp (ISO-8601 UTC).
    pub published_utc: String,
    /// Quant sentiment score (-1.0 to 1.0).
    pub sentiment_score: f64,
    /// Sentiment model confidence score (0.0 to 1.0).
    pub confidence: f64,
    /// Source URL or web link.
    pub url: String,
}

/// Aggregated sentiment metrics for a cryptocurrency asset.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct CryptoSentimentSummary {
    /// Confidence-weighted average sentiment score (-1.0 to 1.0).
    pub avg_sentiment: f64,
    /// Total number of relevant news articles mentioning cryptocurrency keywords.
    pub mention_count: usize,
    /// Proportion of matching articles with positive sentiment (> 0.1).
    pub positive_ratio: f64,
    /// Proportion of matching articles with negative sentiment (< -0.1).
    pub negative_ratio: f64,
    /// Publication date/timestamp of the most recent matching article.
    pub latest_article_date: Option<String>,
}

/// Complete response payload for the Crypto News Sentiment endpoint.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct CryptoSentimentResponse {
    /// Evaluated canonical cryptocurrency symbol (e.g. "BTC", "ETH").
    pub asset: String,
    /// Effective start date for analyzed articles (YYYY-MM-DD).
    pub start_date: String,
    /// Effective end date for analyzed articles (YYYY-MM-DD).
    pub end_date: String,
    /// Applied confidence filter threshold.
    pub min_confidence: f64,
    /// Aggregated sentiment summary statistics.
    pub summary: CryptoSentimentSummary,
    /// Top driving news articles matching cryptocurrency keywords.
    pub top_articles: Vec<CryptoSentimentArticle>,
    /// ISO-8601 UTC timestamp of response generation.
    pub generated_at: String,
}
