//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Earnings Call Transcript Data Models
//! ═══════════════════════════════════════════════════════════════════════════════

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

/// Request payload for manually storing an earnings call transcript (`POST /transcripts`).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateTranscriptRequest {
    /// Asset ticker symbol (e.g. 'AAPL', 'NVDA')
    #[schema(example = "AAPL")]
    pub ticker: String,
    /// Fiscal quarter (1 to 4)
    #[schema(example = 4)]
    pub quarter: Option<u8>,
    /// Fiscal year (e.g. 2024, 2025)
    #[schema(example = 2024)]
    pub year: Option<i32>,
    /// Date of earnings call formatted as YYYY-MM-DD
    #[schema(example = "2024-10-31")]
    pub call_date: Option<String>,
    /// Full text body of the earnings call transcript
    #[schema(
        example = "Apple Inc. (AAPL) Q4 2024 Earnings Call Transcript. Tim Cook -- Chief Executive Officer..."
    )]
    pub transcript_text: String,
    /// Originating source identifier (e.g. 'manual', 'edgar_8k', 'whisper_asr')
    #[schema(example = "manual")]
    pub source: Option<String>,
    /// Optional pre-computed sentiment score [-1.0, 1.0]
    #[schema(example = 0.68)]
    pub sentiment_score: Option<f64>,
    /// Optional pre-computed sentiment label ('BULLISH', 'BEARISH', 'NEUTRAL')
    #[schema(example = "BULLISH")]
    pub sentiment_label: Option<String>,
    /// Optional model prediction confidence [0.0, 1.0]
    #[schema(example = 0.89)]
    pub confidence: Option<f64>,
}

/// Query parameters for filtering and paginating transcripts (`GET /transcripts`).
#[derive(Debug, Clone, Deserialize, IntoParams, ToSchema)]
pub struct TranscriptListParams {
    /// Filter by asset ticker symbol
    #[schema(example = "AAPL")]
    pub ticker: Option<String>,
    /// Filter by start call date (YYYY-MM-DD)
    #[schema(example = "2024-01-01")]
    pub start_date: Option<String>,
    /// Filter by end call date (YYYY-MM-DD)
    #[schema(example = "2024-12-31")]
    pub end_date: Option<String>,
    /// Filter by fiscal quarter (1 to 4)
    #[schema(example = 4)]
    pub quarter: Option<u8>,
    /// Filter by fiscal year (e.g. 2024)
    #[schema(example = 2024)]
    pub year: Option<i32>,
    /// Pagination limit (default: 20, max: 100)
    #[schema(example = 20)]
    pub limit: Option<usize>,
    /// Pagination offset (default: 0)
    #[schema(example = 0)]
    pub offset: Option<usize>,
}

/// Lightweight transcript summary metadata returned in list queries.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct TranscriptMetadata {
    /// Unique transcript UUID
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: Uuid,
    /// Owner user identifier
    #[schema(example = "quant_fund_01")]
    pub user_id: String,
    /// Asset ticker symbol
    #[schema(example = "AAPL")]
    pub ticker: String,
    /// Fiscal quarter (1 to 4)
    #[schema(example = 4)]
    pub quarter: Option<u8>,
    /// Fiscal year (e.g. 2024)
    #[schema(example = 2024)]
    pub year: Option<i32>,
    /// Date of earnings call (YYYY-MM-DD)
    #[schema(example = "2024-10-31")]
    pub call_date: Option<String>,
    /// Originating source
    #[schema(example = "manual")]
    pub source: String,
    /// Quantitative sentiment score [-1.0, 1.0]
    #[schema(example = 0.68)]
    pub sentiment_score: Option<f64>,
    /// Categorical sentiment label ('BULLISH', 'BEARISH', 'NEUTRAL')
    #[schema(example = "BULLISH")]
    pub sentiment_label: Option<String>,
    /// Model confidence score [0.0, 1.0]
    #[schema(example = 0.89)]
    pub confidence: Option<f64>,
    /// Total word count in transcript text
    #[schema(example = 8540)]
    pub word_count: usize,
    /// Record creation timestamp (UTC)
    pub created_at: DateTime<Utc>,
}

/// Full earnings call transcript response payload including complete text body.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct TranscriptResponse {
    /// Unique transcript UUID
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: Uuid,
    /// Owner user identifier
    #[schema(example = "quant_fund_01")]
    pub user_id: String,
    /// Asset ticker symbol
    #[schema(example = "AAPL")]
    pub ticker: String,
    /// Fiscal quarter (1 to 4)
    #[schema(example = 4)]
    pub quarter: Option<u8>,
    /// Fiscal year (e.g. 2024)
    #[schema(example = 2024)]
    pub year: Option<i32>,
    /// Date of earnings call (YYYY-MM-DD)
    #[schema(example = "2024-10-31")]
    pub call_date: Option<String>,
    /// Full transcript text body
    #[schema(example = "Apple Inc. (AAPL) Q4 2024 Earnings Call Transcript...")]
    pub transcript_text: String,
    /// Originating source
    #[schema(example = "manual")]
    pub source: String,
    /// Quantitative sentiment score [-1.0, 1.0]
    #[schema(example = 0.68)]
    pub sentiment_score: Option<f64>,
    /// Categorical sentiment label ('BULLISH', 'BEARISH', 'NEUTRAL')
    #[schema(example = "BULLISH")]
    pub sentiment_label: Option<String>,
    /// Model confidence score [0.0, 1.0]
    #[schema(example = 0.89)]
    pub confidence: Option<f64>,
    /// Total word count in transcript text
    #[schema(example = 8540)]
    pub word_count: usize,
    /// Record creation timestamp (UTC)
    pub created_at: DateTime<Utc>,
}

/// Paginated list of earnings call transcript metadata.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct TranscriptListResponse {
    /// Total matching transcripts count in database
    #[schema(example = 42)]
    pub total: usize,
    /// Number of transcripts in current page
    #[schema(example = 20)]
    pub count: usize,
    /// Limit applied
    #[schema(example = 20)]
    pub limit: usize,
    /// Offset applied
    #[schema(example = 0)]
    pub offset: usize,
    /// Array of transcript metadata summaries
    pub items: Vec<TranscriptMetadata>,
}

/// Response payload for deleting an earnings call transcript (`DELETE /transcripts/{id}`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct DeleteTranscriptResponse {
    /// Deleted transcript UUID
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: Uuid,
    /// Deletion confirmation flag
    #[schema(example = true)]
    pub deleted: bool,
    /// Human-readable outcome message
    #[schema(example = "Transcript successfully deleted")]
    pub message: String,
}
