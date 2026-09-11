//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Unified Cross-Domain Search API Data Models
//! ═══════════════════════════════════════════════════════════════════════════════

use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

/// Query parameters for Unified Cross-Domain Search (`GET /search`).
#[derive(Debug, Clone, Deserialize, IntoParams, ToSchema)]
pub struct SearchParams {
    /// Search query term, keyword, phrase, or ticker symbol (required, non-empty)
    #[schema(example = "AAPL")]
    pub q: String,

    /// Comma-separated data domains to include (default: "all"):
    /// `news`, `sentiment`, `transcripts`, `filings`, `events`, `insider`, `supply_chain`, `options`
    #[schema(example = "news,transcripts,filings")]
    pub types: Option<String>,

    /// Filter items occurring on or after this date (RFC3339 or YYYY-MM-DD)
    #[schema(example = "2025-01-01")]
    pub start_date: Option<String>,

    /// Filter items occurring on or before this date (RFC3339 or YYYY-MM-DD)
    #[schema(example = "2026-12-31")]
    pub end_date: Option<String>,

    /// Maximum total results to return (min: 1, max: 100, default: 20)
    #[schema(example = 20)]
    pub limit: Option<usize>,

    /// Pagination offset (min: 0, default: 0)
    #[schema(example = 0)]
    pub offset: Option<usize>,
}

/// Normalized search result item across all data domains.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct SearchResultItem {
    /// Unique item identifier (UUID, accession number, or deterministic ID)
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: String,

    /// Data domain type (e.g., "news", "transcript", "filing", "event", "sentiment", "insider", "supply_chain", "options")
    #[schema(example = "news")]
    #[serde(rename = "type")]
    pub item_type: String,

    /// Primary associated ticker symbol
    #[schema(example = "AAPL")]
    pub ticker: String,

    /// Descriptive title or headline of the matching record
    #[schema(example = "Apple reports record quarterly earnings")]
    pub title: String,

    /// Excerpt or snippet text preview (up to 200 characters)
    #[schema(example = "Apple Inc. announced record quarterly revenue of $94.9 billion...")]
    pub snippet: String,

    /// Date or timestamp of the record (ISO format YYYY-MM-DD or RFC3339)
    #[schema(example = "2025-07-28")]
    pub date: String,

    /// Match relevance ranking score between 0.0 and 1.0
    #[schema(example = 0.95)]
    pub score: f64,
}

/// Unified Search response payload.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct SearchResponse {
    /// The search query that was executed
    #[schema(example = "AAPL")]
    pub query: String,

    /// List of data types queried
    #[schema(example = json!(["news", "sentiment", "transcripts", "filings"]))]
    pub types: Vec<String>,

    /// Total number of results returned in this response page
    #[schema(example = 15)]
    pub count: usize,

    /// Consolidated, ranked list of matching items
    pub results: Vec<SearchResultItem>,

    /// UTC timestamp when this search was executed
    #[schema(example = "2026-08-30T12:00:00Z")]
    pub generated_at: String,
}
