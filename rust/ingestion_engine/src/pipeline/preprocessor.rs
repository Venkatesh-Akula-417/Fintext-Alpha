//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — In-Process Rust Preprocessing Pipeline
//! ═══════════════════════════════════════════════════════════════════════════════

use crate::audio::AudioFeatures;
use crate::nlp::Entity;
use fintext_event_classifier::classify_event;
use fintext_html_sanitizer::{clean_html, extract_links};
use fintext_spam_detector::RustSpamDetector;
use fintext_ticker_extractor::extract_tickers_with_confidence;
use serde::{Deserialize, Serialize};
use std::time::Instant;

pub fn default_utc_now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawDocument {
    pub id: String,
    pub title: String,
    pub source: String,
    pub url: String,
    pub published_utc: String,
    pub raw_content: String,
    #[serde(default)]
    pub audio_path: Option<String>,
    #[serde(default = "default_utc_now_rfc3339")]
    pub ingested_utc: String,
    #[serde(default)]
    pub source_id: Option<String>,
    #[serde(default)]
    pub revision_number: Option<i32>,
    #[serde(default)]
    pub is_revision: Option<bool>,
}

/// Detailed stage-by-stage and end-to-end signal latency metrics.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SignalLatencyMetrics {
    /// Latency of source fetching (HTTP / WebSocket) in milliseconds.
    pub fetch_latency_ms: f64,
    /// In-process normalization runtime (HTML, tickers, spam, classification) in milliseconds.
    pub normalization_latency_ms: f64,
    /// ONNX sentiment inference runtime (including chunking) in milliseconds.
    pub inference_latency_ms: f64,
    /// Database ILP / WAL buffer write runtime in milliseconds.
    pub write_latency_ms: f64,
    /// Total customer-visible signal freshness latency (db_commit_ts - source_event_ts) in milliseconds.
    pub total_signal_latency_ms: f64,
    /// Target SLA threshold in milliseconds.
    pub sla_target_ms: u32,
    /// True if total_signal_latency_ms <= sla_target_ms.
    pub is_sla_compliant: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProcessedDocument {
    pub id: String,
    pub title: String,
    pub source: String,
    pub url: String,
    pub published_utc: String,
    #[serde(default = "default_utc_now_rfc3339")]
    pub ingested_utc: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub db_commit_utc: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_content: Option<String>,
    pub clean_text: String,
    pub extracted_links: Vec<String>,
    pub tickers: Vec<String>,
    pub primary_ticker: Option<String>,
    pub ticker_confidences: Vec<(String, f64)>,
    pub is_spam: bool,
    pub spam_reason: String,
    pub event_category: String,
    pub preprocessing_latency_us: u64,
    #[serde(default)]
    pub entities: Vec<Entity>,
    #[serde(default)]
    pub audio_transcript: Option<String>,
    #[serde(default)]
    pub audio_features: Option<AudioFeatures>,
    #[serde(default)]
    pub vpin: Option<f64>,
    #[serde(default)]
    pub gex: Option<f64>,
    #[serde(default)]
    pub gex_positive: Option<f64>,
    #[serde(default)]
    pub gex_negative: Option<f64>,
    #[serde(default)]
    pub latency_metrics: Option<SignalLatencyMetrics>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_to: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision_number: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_current: Option<bool>,
}

impl Default for RawDocument {
    fn default() -> Self {
        Self {
            id: String::new(),
            title: String::new(),
            source: String::new(),
            url: String::new(),
            published_utc: String::new(),
            raw_content: String::new(),
            audio_path: None,
            ingested_utc: default_utc_now_rfc3339(),
            source_id: None,
            revision_number: None,
            is_revision: None,
        }
    }
}

impl Default for ProcessedDocument {
    fn default() -> Self {
        Self {
            id: String::new(),
            title: String::new(),
            source: String::new(),
            url: String::new(),
            published_utc: String::new(),
            ingested_utc: default_utc_now_rfc3339(),
            db_commit_utc: None,
            raw_content: None,
            clean_text: String::new(),
            extracted_links: Vec::new(),
            tickers: Vec::new(),
            primary_ticker: None,
            ticker_confidences: Vec::new(),
            is_spam: false,
            spam_reason: String::new(),
            event_category: "GENERAL".to_string(),
            preprocessing_latency_us: 0,
            entities: Vec::new(),
            audio_transcript: None,
            audio_features: None,
            vpin: None,
            gex: None,
            gex_positive: None,
            gex_negative: None,
            latency_metrics: None,
            source_id: None,
            valid_from: None,
            valid_to: None,
            revision_number: Some(1),
            is_current: Some(true),
        }
    }
}

pub struct Preprocessor {
    spam_detector: RustSpamDetector,
}

impl Preprocessor {
    pub fn new() -> Self {
        Self {
            spam_detector: RustSpamDetector::new(Some(60.0), Some(3), Some(5)),
        }
    }

    pub fn process(&self, doc: RawDocument) -> ProcessedDocument {
        let t0 = Instant::now();

        // 1. In-process HTML sanitization
        let clean_text = clean_html(&doc.raw_content);
        let extracted_links = extract_links(&doc.raw_content);

        // Combine title and clean text for analysis
        let combined_text = if clean_text.is_empty() {
            doc.title.clone()
        } else {
            format!("{} {}", doc.title, clean_text)
        };

        // 2. In-process Ticker Extraction with disambiguation
        let scored_tickers = extract_tickers_with_confidence(&combined_text);
        let tickers: Vec<String> = scored_tickers.iter().map(|(t, _)| t.clone()).collect();
        let primary_ticker = tickers.first().cloned();

        // 3. In-process Spam Detection
        let (is_spam, spam_reason) = self.spam_detector.check_spam(&doc.title, &doc.source, None);

        // 4. In-process Event Taxonomy Classification
        let event_category = classify_event(&combined_text);

        let latency_us = t0.elapsed().as_micros() as u64;

        ProcessedDocument {
            id: doc.id,
            title: doc.title,
            source: doc.source,
            url: doc.url,
            published_utc: doc.published_utc,
            ingested_utc: doc.ingested_utc,
            db_commit_utc: None,
            raw_content: Some(doc.raw_content),
            clean_text,
            extracted_links,
            tickers,
            primary_ticker,
            ticker_confidences: scored_tickers,
            is_spam,
            spam_reason,
            event_category,
            preprocessing_latency_us: latency_us,
            entities: Vec::new(),
            audio_transcript: None,
            audio_features: None,
            vpin: None,
            gex: None,
            gex_positive: None,
            gex_negative: None,
            latency_metrics: None,
            source_id: doc.source_id,
            valid_from: None,
            valid_to: None,
            revision_number: doc.revision_number.or(Some(1)),
            is_current: Some(true),
        }
    }

    /// Evaluates data quality gates for a raw document before downstream sentiment processing.
    pub fn evaluate_quality_gates(
        &self,
        doc: &RawDocument,
        gate: &crate::quality::DataQualityGate,
        source_quality_score: f64,
    ) -> crate::quality::QualityGateResult {
        // Extract preliminary tickers from title + raw_content to check ticker format & duplicates
        let combined_text = if doc.raw_content.is_empty() {
            doc.title.clone()
        } else {
            format!("{} {}", doc.title, doc.raw_content)
        };
        let scored_tickers = extract_tickers_with_confidence(&combined_text);
        let primary_ticker = scored_tickers.first().map(|(t, _)| t.as_str());

        gate.evaluate(doc, primary_ticker, source_quality_score)
    }

    /// Process document with integrated quality gate enforcement and telemetry updates.
    pub fn process_with_quality(
        &self,
        doc: RawDocument,
        gate: &crate::quality::DataQualityGate,
        quality_store: &crate::quality::SourceQualityStore,
    ) -> Result<ProcessedDocument, crate::quality::QualityGateResult> {
        let score = quality_store.get_quality_score(&doc.source);
        let gate_res = self.evaluate_quality_gates(&doc, gate, score);

        match &gate_res {
            crate::quality::QualityGateResult::Accept => {
                let freshness_ms = crate::quality::calculate_freshness_ms(&doc.published_utc);
                let (present, total) = crate::quality::count_document_fields(&doc);
                quality_store.record_acceptance(&doc.source, freshness_ms, present, total);
                Ok(self.process(doc))
            }
            crate::quality::QualityGateResult::Quarantine { reason, .. } => {
                quality_store.record_quarantine(&doc.source, reason);
                Err(gate_res)
            }
            crate::quality::QualityGateResult::Reject { reason, .. } => {
                quality_store.record_rejection(&doc.source, reason);
                Err(gate_res)
            }
        }
    }
}
