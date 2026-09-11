//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Data Lineage & Provenance Tracking Engine
//! ═══════════════════════════════════════════════════════════════════════════════
//!
//! Provides granular record-level data provenance and audit tracking linking
//! sentiment scores and news articles to their source content, machine learning
//! model versions, pipeline components, quality scores, and processing history.

use chrono::Utc;
use dashmap::DashMap;
use sqlx::PgPool;
use tracing::info;
use uuid::Uuid;

use crate::models::provenance::{DataProvenanceItem, ProcessingStep};

/// In-memory and cached registry for data provenance audit records.
#[derive(Debug, Clone)]
pub struct ProvenanceRegistry {
    /// Maps `"{record_type}:{record_id}"` to a list of provenance entries.
    entries: DashMap<String, Vec<DataProvenanceItem>>,
}

impl Default for ProvenanceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ProvenanceRegistry {
    /// Creates a new `ProvenanceRegistry` pre-populated with deterministic records
    /// for common tickers and sample articles.
    pub fn new() -> Self {
        let registry = Self {
            entries: DashMap::new(),
        };

        // Pre-populate sample sentiment records
        let sample_tickers = ["AAPL", "MSFT", "NVDA", "TSLA", "AMZN", "GOOGL", "META"];
        for ticker in sample_tickers {
            let record_id = format!("{}_2026-08-30T10:15:00Z", ticker);
            let item = Self::generate_deterministic_mock_provenance(
                "sentiment",
                &record_id,
                "finbert-minilm-v2.1",
                "2.0.0",
            );
            registry.record_provenance(item);
        }

        // Pre-populate sample news records
        let sample_articles = [
            "550e8400-e29b-41d4-a716-446655440000",
            "550e8400-e29b-41d4-a716-446655440001",
            "550e8400-e29b-41d4-a716-446655440002",
        ];
        for art_id in sample_articles {
            let item = Self::generate_deterministic_mock_provenance(
                "news",
                art_id,
                "finbert-minilm-v2.1",
                "2.0.0",
            );
            registry.record_provenance(item);
        }

        registry
    }

    /// Key helper format `"{record_type}:{record_id}"`
    fn make_key(record_type: &str, record_id: &str) -> String {
        format!("{}:{}", record_type.to_lowercase().trim(), record_id.trim())
    }

    /// Records a new provenance entry into the in-memory registry.
    pub fn record_provenance(&self, item: DataProvenanceItem) {
        let key = Self::make_key(&item.record_type, &item.record_id);
        self.entries.entry(key).or_default().push(item);
    }

    /// Retrieves all recorded provenance entries for a given record type and ID.
    pub fn get_provenance(&self, record_type: &str, record_id: &str) -> Vec<DataProvenanceItem> {
        let key = Self::make_key(record_type, record_id);
        if let Some(list) = self.entries.get(&key) {
            list.clone()
        } else {
            Vec::new()
        }
    }

    /// Generates a realistic, deterministic mock provenance trace for any queried record.
    pub fn generate_deterministic_mock_provenance(
        record_type: &str,
        record_id: &str,
        model_version: &str,
        pipeline_version: &str,
    ) -> DataProvenanceItem {
        let norm_type = record_type.to_lowercase();
        let mut bytes = [0u8; 16];
        let key_bytes = format!("{}:{}", norm_type, record_id).into_bytes();
        for (i, b) in key_bytes.iter().enumerate() {
            bytes[i % 16] ^= b;
        }
        bytes[6] = (bytes[6] & 0x0f) | 0x40; // Version 4
        bytes[8] = (bytes[8] & 0x3f) | 0x80; // Variant RFC4122
        let base_id = Uuid::from_bytes(bytes);
        let now_str = Utc::now().to_rfc3339();

        let (source_type, source_id, quality_score, steps) = if norm_type == "sentiment" {
            let s_type = "finnhub".to_string();
            let s_id = Some(format!("doc_{}", &base_id.to_string()[..8]));
            let q_score = Some(0.98);

            let steps = vec![
                ProcessingStep {
                    step: "fetch_article".to_string(),
                    timestamp: "2026-08-30T10:15:00.100Z".to_string(),
                    description: "Ingested raw multi-source news/transcript payload from primary data feed".to_string(),
                    details: Some(serde_json::json!({
                        "provider": "Finnhub Institutional News Wire",
                        "http_status": 200,
                        "content_length_bytes": 4820
                    })),
                },
                ProcessingStep {
                    step: "clean_text".to_string(),
                    timestamp: "2026-08-30T10:15:00.220Z".to_string(),
                    description: "Normalized Unicode, stripped HTML boilerplate, and extracted financial sentences".to_string(),
                    details: Some(serde_json::json!({
                        "cleaning_rules_applied": ["strip_tags", "normalize_whitespace", "remove_disclaimers"],
                        "retained_sentence_count": 18
                    })),
                },
                ProcessingStep {
                    step: "tokenize".to_string(),
                    timestamp: "2026-08-30T10:15:00.310Z".to_string(),
                    description: "Tokenized financial lexicon with BPE tokenizer for FinBERT model".to_string(),
                    details: Some(serde_json::json!({
                        "token_count": 342,
                        "vocab_size": 30522,
                        "truncated": false
                    })),
                },
                ProcessingStep {
                    step: "infer_sentiment".to_string(),
                    timestamp: "2026-08-30T10:15:00.540Z".to_string(),
                    description: format!("Executed transformer forward pass on {} sentiment model", model_version),
                    details: Some(serde_json::json!({
                        "model_tag": model_version,
                        "logits": [0.08, 0.12, 0.80],
                        "inference_latency_ms": 1.42
                    })),
                },
                ProcessingStep {
                    step: "quality_audit".to_string(),
                    timestamp: "2026-08-30T10:15:00.610Z".to_string(),
                    description: "Validated sentiment bounds [-1.0, 1.0] and computed confidence interval".to_string(),
                    details: Some(serde_json::json!({
                        "quality_score": 0.98,
                        "confidence": 0.95,
                        "passed_all_invariants": true
                    })),
                },
            ];

            (s_type, s_id, q_score, steps)
        } else {
            // news article
            let s_type = "sec_edgar".to_string();
            let s_id = Some(format!("0000320193-26-{}", &base_id.to_string()[..6]));
            let q_score = Some(0.99);

            let steps = vec![
                ProcessingStep {
                    step: "fetch_source".to_string(),
                    timestamp: "2026-08-30T10:10:00.050Z".to_string(),
                    description: "Polled RSS/REST wire endpoint and verified SSL signature"
                        .to_string(),
                    details: Some(serde_json::json!({
                        "endpoint": "https://www.sec.gov/edgar/searchedgar/companysearch",
                        "compression": "gzip",
                        "size_bytes": 12840
                    })),
                },
                ProcessingStep {
                    step: "deduplication".to_string(),
                    timestamp: "2026-08-30T10:10:00.150Z".to_string(),
                    description: "Calculated MinHash LSH and confirmed document novelty score"
                        .to_string(),
                    details: Some(serde_json::json!({
                        "lsh_similarity": 0.04,
                        "is_duplicate": false
                    })),
                },
                ProcessingStep {
                    step: "entity_extraction".to_string(),
                    timestamp: "2026-08-30T10:10:00.280Z".to_string(),
                    description:
                        "Identified primary company tickers, executive names, and corporate actions"
                            .to_string(),
                    details: Some(serde_json::json!({
                        "entities_detected": ["Apple Inc.", "Tim Cook", "Services Division"],
                        "ticker": "AAPL"
                    })),
                },
                ProcessingStep {
                    step: "sentiment_tagging".to_string(),
                    timestamp: "2026-08-30T10:10:00.410Z".to_string(),
                    description:
                        "Computed paragraph-level sentiment dispersion and aggregated score"
                            .to_string(),
                    details: Some(serde_json::json!({
                        "paragraph_count": 6,
                        "overall_sentiment": 0.84,
                        "pipeline_version": pipeline_version
                    })),
                },
                ProcessingStep {
                    step: "publish_index".to_string(),
                    timestamp: "2026-08-30T10:10:00.520Z".to_string(),
                    description: "Indexed full text and metadata into search and time-series store"
                        .to_string(),
                    details: Some(serde_json::json!({
                        "indexed_at": "2026-08-30T10:10:00.520Z",
                        "replicated": true
                    })),
                },
            ];

            (s_type, s_id, q_score, steps)
        };

        DataProvenanceItem {
            id: base_id.to_string(),
            record_type: norm_type,
            record_id: record_id.to_string(),
            source_type,
            source_id,
            model_version: model_version.to_string(),
            pipeline_version: pipeline_version.to_string(),
            data_quality_score: quality_score,
            processing_steps: steps,
            created_at: now_str,
        }
    }
}

/// Initializes the `data_provenance` table and indexes in PostgreSQL idempotently.
pub async fn init_data_provenance_table(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS data_provenance (
            id UUID PRIMARY KEY,
            record_type TEXT NOT NULL,
            record_id TEXT NOT NULL,
            source_type TEXT NOT NULL,
            source_id TEXT,
            model_version TEXT NOT NULL,
            pipeline_version TEXT NOT NULL,
            data_quality_score DOUBLE PRECISION,
            processing_steps JSONB NOT NULL DEFAULT '[]'::jsonb,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        );
        CREATE INDEX IF NOT EXISTS idx_provenance_record ON data_provenance(record_type, record_id);
        "#,
    )
    .execute(pool)
    .await?;

    info!("Initialized data_provenance database table and indexes");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provenance_registry_crud() {
        let registry = ProvenanceRegistry::new();

        // 1. Check pre-populated record
        let entries = registry.get_provenance("sentiment", "AAPL_2026-08-30T10:15:00Z");
        assert!(!entries.is_empty());
        assert_eq!(entries[0].record_type, "sentiment");
        assert_eq!(entries[0].source_type, "finnhub");
        assert_eq!(entries[0].processing_steps.len(), 5);

        // 2. Add custom record
        let custom_item = DataProvenanceItem {
            id: Uuid::new_v4().to_string(),
            record_type: "sentiment".to_string(),
            record_id: "CUSTOM_2026-09-01T00:00:00Z".to_string(),
            source_type: "polygon".to_string(),
            source_id: Some("poly_123".to_string()),
            model_version: "finbert-v3".to_string(),
            pipeline_version: "2.1.0".to_string(),
            data_quality_score: Some(0.99),
            processing_steps: vec![ProcessingStep {
                step: "custom_step".to_string(),
                timestamp: Utc::now().to_rfc3339(),
                description: "Custom step executed".to_string(),
                details: None,
            }],
            created_at: Utc::now().to_rfc3339(),
        };

        registry.record_provenance(custom_item.clone());

        let retrieved = registry.get_provenance("sentiment", "CUSTOM_2026-09-01T00:00:00Z");
        assert_eq!(retrieved.len(), 1);
        assert_eq!(retrieved[0].source_id, Some("poly_123".to_string()));
        assert_eq!(retrieved[0].model_version, "finbert-v3");
    }

    #[test]
    fn test_deterministic_mock_provenance_generation() {
        let p1 = ProvenanceRegistry::generate_deterministic_mock_provenance(
            "sentiment",
            "NVDA_2026-08-30T10:15:00Z",
            "finbert-minilm-v2.1",
            "2.0.0",
        );
        let p2 = ProvenanceRegistry::generate_deterministic_mock_provenance(
            "sentiment",
            "NVDA_2026-08-30T10:15:00Z",
            "finbert-minilm-v2.1",
            "2.0.0",
        );

        assert_eq!(p1.id, p2.id);
        assert_eq!(p1.record_type, "sentiment");
        assert_eq!(p1.processing_steps.len(), 5);
        assert_eq!(p1.processing_steps[0].step, "fetch_article");
        assert_eq!(p1.processing_steps[3].step, "infer_sentiment");
    }

    #[test]
    fn test_news_provenance_generation() {
        let news_prov = ProvenanceRegistry::generate_deterministic_mock_provenance(
            "news",
            "550e8400-e29b-41d4-a716-446655440000",
            "finbert-minilm-v2.1",
            "2.0.0",
        );

        assert_eq!(news_prov.record_type, "news");
        assert_eq!(news_prov.source_type, "sec_edgar");
        assert_eq!(news_prov.processing_steps.len(), 5);
        assert_eq!(news_prov.processing_steps[0].step, "fetch_source");
        assert_eq!(news_prov.processing_steps[4].step, "publish_index");
    }
}
