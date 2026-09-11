//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — News Article Data Models
//! ═══════════════════════════════════════════════════════════════════════════════

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

/// Query parameters for listing news articles.
#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct ListNewsArticlesQuery {
    /// Filter articles by stock ticker (e.g. 'AAPL', 'MSFT', 'NVDA').
    pub ticker: Option<String>,
    /// Filter articles published on or after this date (YYYY-MM-DD or RFC3339).
    pub start_date: Option<String>,
    /// Filter articles published on or before this date (YYYY-MM-DD or RFC3339).
    pub end_date: Option<String>,
    /// Filter articles by news source (e.g. 'Institutional Wire', 'SEC EDGAR', 'Finnhub').
    pub source: Option<String>,
    /// Maximum number of articles to return (default: 20, max: 100).
    pub limit: Option<usize>,
    /// Number of articles to skip for pagination (default: 0).
    pub offset: Option<usize>,
}

/// Metadata and preview snippet for a news article in list views.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct NewsArticleMetadata {
    /// Unique identifier for the news article.
    #[schema(value_type = String, format = "uuid", example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: Uuid,
    /// Stock ticker symbol associated with the news.
    #[schema(example = "AAPL")]
    pub ticker: String,
    /// Headline title of the news article.
    #[schema(example = "Apple Unveils Next-Gen Neural Engine & Services Growth Outlook")]
    pub title: String,
    /// Publishing news source or agency.
    #[schema(example = "Institutional Wire")]
    pub source: String,
    /// Timestamp when the article was published in UTC.
    #[schema(example = "2026-08-30T10:15:00Z")]
    pub published_utc: DateTime<Utc>,
    /// Quantized sentiment score between -1.0 (bearish) and +1.0 (bullish).
    #[schema(example = 0.84)]
    pub sentiment_score: Option<f64>,
    /// Qualitative sentiment label (positive, negative, neutral).
    #[schema(example = "positive")]
    pub sentiment_label: Option<String>,
    /// Sentiment prediction confidence score (0.0 to 1.0).
    #[schema(example = 0.95)]
    pub confidence: Option<f64>,
    /// Reliability and completeness score of source content (0.0 to 1.0).
    #[schema(example = 0.98)]
    pub data_quality_score: Option<f64>,
    /// Truncated snippet of the article content (maximum 200 characters).
    #[schema(
        example = "Apple Inc. announced significant expansion of its enterprise cloud compute partnerships and next-generation neural processing capabilities during its institutional investor briefing..."
    )]
    pub snippet: String,
}

/// Full article content and detailed NLP signals for a specific news item.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct NewsArticleFull {
    /// Unique identifier for the news article.
    #[schema(value_type = String, format = "uuid", example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: Uuid,
    /// Stock ticker symbol associated with the news.
    #[schema(example = "AAPL")]
    pub ticker: String,
    /// Headline title of the news article.
    #[schema(example = "Apple Unveils Next-Gen Neural Engine & Services Growth Outlook")]
    pub title: String,
    /// Publishing news source or agency.
    #[schema(example = "Institutional Wire")]
    pub source: String,
    /// Timestamp when the article was published in UTC.
    #[schema(example = "2026-08-30T10:15:00Z")]
    pub published_utc: DateTime<Utc>,
    /// Full unedited text body of the financial news article.
    #[schema(
        example = "Apple Inc. announced significant expansion of its enterprise cloud compute partnerships and next-generation neural processing capabilities during its institutional investor briefing in Cupertino. Senior leadership emphasized double-digit growth in high-margin services revenue and accelerated enterprise hardware refresh cycles."
    )]
    pub full_text: String,
    /// Quantized sentiment score between -1.0 (bearish) and +1.0 (bullish).
    #[schema(example = 0.84)]
    pub sentiment_score: Option<f64>,
    /// Qualitative sentiment label (positive, negative, neutral).
    #[schema(example = "positive")]
    pub sentiment_label: Option<String>,
    /// Sentiment prediction confidence score (0.0 to 1.0).
    #[schema(example = 0.95)]
    pub confidence: Option<f64>,
    /// Reliability and completeness score of source content (0.0 to 1.0).
    #[schema(example = 0.98)]
    pub data_quality_score: Option<f64>,
    /// Timestamp when the record was indexed into FinText Alpha Vectorizer.
    #[schema(example = "2026-08-30T10:15:05Z")]
    pub created_at: DateTime<Utc>,
}

impl NewsArticleFull {
    /// Generate a `NewsArticleMetadata` preview item with snippet truncated to 200 chars.
    pub fn to_metadata(&self) -> NewsArticleMetadata {
        let snippet = if self.full_text.chars().count() <= 200 {
            self.full_text.clone()
        } else {
            let truncated: String = self.full_text.chars().take(197).collect();
            format!("{}...", truncated)
        };

        NewsArticleMetadata {
            id: self.id,
            ticker: self.ticker.clone(),
            title: self.title.clone(),
            source: self.source.clone(),
            published_utc: self.published_utc,
            sentiment_score: self.sentiment_score,
            sentiment_label: self.sentiment_label.clone(),
            confidence: self.confidence,
            data_quality_score: self.data_quality_score,
            snippet,
        }
    }
}

/// Paginated response container for listing news articles.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct NewsArticlesListResponse {
    /// Array of article metadata items matching the query.
    pub articles: Vec<NewsArticleMetadata>,
    /// Total count of matching articles available across all pages.
    #[schema(example = 42)]
    pub total: usize,
    /// Limit applied to this page query.
    #[schema(example = 20)]
    pub limit: usize,
    /// Offset applied to this page query.
    #[schema(example = 0)]
    pub offset: usize,
}
