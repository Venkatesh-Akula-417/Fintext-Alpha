//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Data Lineage & Provenance Tracking Models
//! ═══════════════════════════════════════════════════════════════════════════════

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// A granular processing step recorded in a record's data provenance lineage trace.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct ProcessingStep {
    /// Step stage identifier (e.g. "fetch_article", "clean_text", "tokenize", "infer_sentiment", "quality_audit")
    #[schema(example = "fetch_article")]
    pub step: String,
    /// UTC timestamp when the processing step was performed
    #[schema(example = "2026-08-30T10:15:00.123Z")]
    pub timestamp: String,
    /// Human-readable explanation of what this processing step achieved
    #[schema(example = "Fetched raw HTML/JSON payload from upstream news source provider")]
    pub description: String,
    /// Optional structured details, parameters, or sub-metrics
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

/// A complete data provenance and lineage audit record.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct DataProvenanceItem {
    /// Unique identifier for the provenance record (UUID)
    #[schema(value_type = String, format = "uuid", example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: String,
    /// Record entity type (e.g. 'sentiment', 'news')
    #[schema(example = "sentiment")]
    pub record_type: String,
    /// Unique identifier of the target record (e.g. 'AAPL_2026-08-30T10:15:00Z' or article UUID)
    #[schema(example = "AAPL_2026-08-30T10:15:00Z")]
    pub record_id: String,
    /// Source origin provider ('sec_edgar', 'finnhub', 'polygon', 'manual', 'mock')
    #[schema(example = "finnhub")]
    pub source_type: String,
    /// Optional upstream source ID or SEC filing accession number
    #[schema(example = "art_987654321")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_id: Option<String>,
    /// Machine learning / Transformer model version tag
    #[schema(example = "finbert-minilm-v2.1")]
    pub model_version: String,
    /// Feature extraction and processing pipeline semantic version
    #[schema(example = "2.0.0")]
    pub pipeline_version: String,
    /// Quantitative data quality score between 0.0 and 1.0
    #[schema(example = 0.98)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_quality_score: Option<f64>,
    /// Granular chronological execution steps comprising the processing history
    pub processing_steps: Vec<ProcessingStep>,
    /// UTC timestamp when the provenance entry was recorded
    #[schema(example = "2026-08-30T10:15:01.456Z")]
    pub created_at: String,
}

/// Provenance trace query response for `GET /provenance/{record_type}/{record_id}`
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct DataProvenanceResponse {
    /// Queried record type
    #[schema(example = "sentiment")]
    pub record_type: String,
    /// Queried record identifier
    #[schema(example = "AAPL_2026-08-30T10:15:00Z")]
    pub record_id: String,
    /// Provenance records matching the query
    pub provenance_entries: Vec<DataProvenanceItem>,
    /// Total count of matching provenance records
    #[schema(example = 1)]
    pub total_entries: usize,
    /// UTC timestamp of the retrieval query
    #[schema(example = "2026-09-01T12:00:00Z")]
    pub retrieved_at: String,
}
