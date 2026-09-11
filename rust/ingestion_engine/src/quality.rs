//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Data Source Quality Scoring & Ingestion Gates
//! ═══════════════════════════════════════════════════════════════════════════════
//!
//! Provides an automated, institutional-grade data quality governance subsystem:
//! 1. Per-source quality scoring (0.0 to 1.0) based on historical accuracy,
//!    freshness, completeness, and provider reputation.
//! 2. Four-stage validation gates:
//!    SCHEMA_VALIDATION -> BUSINESS_RULES -> DUPLICATE_DETECTION -> SOURCE_QUALITY_THRESHOLD
//!    routing records to ACCEPT, QUARANTINE, or REJECT.
//! 3. Atomic quarantine storage (`data/quarantine/<source>/<date>/<uuid>.json`).
//! ═══════════════════════════════════════════════════════════════════════════════

use crate::pipeline::RawDocument;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use tracing::warn;
use uuid::Uuid;

/// Configuration options for data source quality scoring and validation gates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataQualityConfig {
    /// Minimum source quality score required to accept records (default: 0.60)
    pub source_quality_threshold: f64,
    /// Maximum number of hashes tracked in the 24-hour duplicate detection cache
    pub duplicate_cache_size: usize,
    /// Base directory where quarantined records are stored
    pub quarantine_path: String,
    /// Whether quality metrics collection is enabled
    pub quality_metrics_enabled: bool,
    /// Maximum clock skew tolerance in seconds for future timestamps (default: 300s / 5 min)
    pub future_skew_tolerance_secs: i64,
}

impl Default for DataQualityConfig {
    fn default() -> Self {
        Self {
            source_quality_threshold: 0.60,
            duplicate_cache_size: 10_000,
            quarantine_path: "data/quarantine".to_string(),
            quality_metrics_enabled: true,
            future_skew_tolerance_secs: 300,
        }
    }
}

impl DataQualityConfig {
    /// Load configuration from environment variables or `config/config.yaml`.
    pub fn from_env_or_config() -> Self {
        let mut cfg = Self::default();

        if let Ok(val) = std::env::var("DATA_QUALITY_THRESHOLD") {
            if let Ok(num) = val.parse::<f64>() {
                cfg.source_quality_threshold = num.clamp(0.0, 1.0);
            }
        }
        if let Ok(val) = std::env::var("DATA_QUALITY_QUARANTINE_PATH") {
            if !val.trim().is_empty() {
                cfg.quarantine_path = val;
            }
        }
        if let Ok(val) = std::env::var("DATA_QUALITY_CACHE_SIZE") {
            if let Ok(num) = val.parse::<usize>() {
                cfg.duplicate_cache_size = num.max(100);
            }
        }

        // Attempt reading from config.yaml if present
        let config_path = std::env::var("CONFIG_PATH").unwrap_or_else(|_| "config/config.yaml".to_string());
        if let Ok(contents) = std::fs::read_to_string(&config_path) {
            let mut in_dq = false;
            for line in contents.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("data_quality:") {
                    in_dq = true;
                    continue;
                }
                if in_dq && !line.starts_with(" ") && !line.starts_with("\t") && !trimmed.is_empty() {
                    break;
                }
                if in_dq {
                    if let Some((k, v)) = trimmed.split_once(':') {
                        let key = k.trim();
                        let val = v.split('#').next().unwrap_or("").trim().trim_matches('"');
                        match key {
                            "source_quality_threshold" => {
                                if let Ok(n) = val.parse::<f64>() {
                                    cfg.source_quality_threshold = n.clamp(0.0, 1.0);
                                }
                            }
                            "duplicate_cache_size" => {
                                if let Ok(n) = val.parse::<usize>() {
                                    cfg.duplicate_cache_size = n.max(100);
                                }
                            }
                            "quarantine_path" => {
                                if !val.is_empty() {
                                    cfg.quarantine_path = val.to_string();
                                }
                            }
                            "quality_metrics_enabled" => {
                                if val == "true" || val == "1" {
                                    cfg.quality_metrics_enabled = true;
                                } else if val == "false" || val == "0" {
                                    cfg.quality_metrics_enabled = false;
                                }
                            }
                            "future_skew_tolerance_secs" => {
                                if let Ok(n) = val.parse::<i64>() {
                                    cfg.future_skew_tolerance_secs = n.max(0);
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        cfg
    }
}

/// Outcome of evaluating an ingested document against data quality gates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum QualityGateResult {
    /// Document meets all schema, business, duplicate, and source quality criteria.
    Accept,
    /// Document violated business rules, duplicate detection, or source quality threshold.
    Quarantine {
        rule: String,
        reason: String,
    },
    /// Document has fundamental schema violations (missing fields, invalid types/dates).
    Reject {
        rule: String,
        reason: String,
    },
}

/// Telemetry metrics tracked per upstream data source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceMetrics {
    pub source: String,
    pub total_records: u64,
    pub accepted_records: u64,
    pub rejected_records: u64,
    pub quarantine_count: u64,
    pub total_freshness_ms: f64,
    pub average_freshness_ms: f64,
    pub last_success_timestamp: Option<String>,
    pub error_count_last_hour: u64,
    pub total_fields_checked: u64,
    pub present_fields_checked: u64,
    pub reliability_factor: f64,
    pub quality_score: f64,
}

impl SourceMetrics {
    pub fn new(source: &str, reliability: f64) -> Self {
        Self {
            source: source.to_string(),
            total_records: 0,
            accepted_records: 0,
            rejected_records: 0,
            quarantine_count: 0,
            total_freshness_ms: 0.0,
            average_freshness_ms: 0.0,
            last_success_timestamp: None,
            error_count_last_hour: 0,
            total_fields_checked: 0,
            present_fields_checked: 0,
            reliability_factor: reliability.clamp(0.0, 1.0),
            quality_score: reliability.clamp(0.0, 1.0),
        }
    }

    /// Recompute composite quality score based on current metrics:
    /// score = 0.4 * (accepted/total) + 0.2 * freshness + 0.2 * completeness + 0.2 * reliability
    pub fn recompute_score(&mut self) {
        // 1. Acceptance ratio (0.4 weight)
        let acceptance_ratio = if self.total_records == 0 {
            1.0
        } else {
            (self.accepted_records as f64 / self.total_records as f64).clamp(0.0, 1.0)
        };

        // 2. Freshness factor (0.2 weight): 1.0 if < 5m (300k ms), decreasing to 0.5 at 30m (1800k ms)
        let freshness_factor = if self.total_records == 0 || self.average_freshness_ms <= 300_000.0 {
            1.0
        } else if self.average_freshness_ms >= 1_800_000.0 {
            0.5
        } else {
            let progress = (self.average_freshness_ms - 300_000.0) / (1_800_000.0 - 300_000.0);
            (1.0 - 0.5 * progress).clamp(0.5, 1.0)
        };

        // 3. Completeness factor (0.2 weight): fraction of required fields present
        let completeness_factor = if self.total_fields_checked == 0 {
            1.0
        } else {
            (self.present_fields_checked as f64 / self.total_fields_checked as f64).clamp(0.0, 1.0)
        };

        // 4. Reliability factor (0.2 weight): inherent reputation
        let reliability = self.reliability_factor.clamp(0.0, 1.0);

        let composite = (0.4 * acceptance_ratio)
            + (0.2 * freshness_factor)
            + (0.2 * completeness_factor)
            + (0.2 * reliability);

        self.quality_score = (composite * 10_000.0).round() / 10_000.0;
    }
}

/// Thread-safe in-memory store tracking quality metrics across all data sources.
pub struct SourceQualityStore {
    metrics: Arc<RwLock<HashMap<String, SourceMetrics>>>,
}

impl Default for SourceQualityStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SourceQualityStore {
    pub fn new() -> Self {
        let mut map = HashMap::new();
        // Canonical reputation factors: SEC=1.0, Polygon=0.9, Finnhub=0.8, mock=0.5
        map.insert("sec_edgar".to_string(), SourceMetrics::new("sec_edgar", 1.0));
        map.insert("sec".to_string(), SourceMetrics::new("sec", 1.0));
        map.insert("polygon".to_string(), SourceMetrics::new("polygon", 0.9));
        map.insert("finnhub".to_string(), SourceMetrics::new("finnhub", 0.8));
        map.insert("mock".to_string(), SourceMetrics::new("mock", 0.5));

        Self {
            metrics: Arc::new(RwLock::new(map)),
        }
    }

    /// Retrieve the default reliability factor for a source identifier.
    pub fn default_reliability_for_source(source: &str) -> f64 {
        match source.to_lowercase().as_str() {
            "sec_edgar" | "sec" => 1.0,
            "polygon" => 0.9,
            "finnhub" => 0.8,
            "mock" | "synthetic" => 0.5,
            _ => 0.7,
        }
    }

    /// Obtain the current quality score for a given source.
    pub fn get_quality_score(&self, source: &str) -> f64 {
        let norm = source.to_lowercase();
        let map = self.metrics.read().unwrap();
        if let Some(m) = map.get(&norm) {
            m.quality_score
        } else {
            Self::default_reliability_for_source(source)
        }
    }

    /// Retrieve detailed telemetry for a given source.
    pub fn get_metrics(&self, source: &str) -> SourceMetrics {
        let norm = source.to_lowercase();
        let mut map = self.metrics.write().unwrap();
        map.entry(norm.clone())
            .or_insert_with(|| SourceMetrics::new(&norm, Self::default_reliability_for_source(&norm)))
            .clone()
    }

    /// Retrieve telemetry for all tracked sources.
    pub fn get_all_metrics(&self) -> Vec<SourceMetrics> {
        let map = self.metrics.read().unwrap();
        map.values().cloned().collect()
    }

    /// Record a successfully accepted document.
    pub fn record_acceptance(
        &self,
        source: &str,
        freshness_ms: f64,
        present_fields: u64,
        total_fields: u64,
    ) {
        let norm = source.to_lowercase();
        let mut map = self.metrics.write().unwrap();
        let entry = map
            .entry(norm.clone())
            .or_insert_with(|| SourceMetrics::new(&norm, Self::default_reliability_for_source(&norm)));

        entry.total_records += 1;
        entry.accepted_records += 1;
        entry.total_freshness_ms += freshness_ms.max(0.0);
        entry.average_freshness_ms = entry.total_freshness_ms / (entry.accepted_records as f64);
        entry.present_fields_checked += present_fields;
        entry.total_fields_checked += total_fields;
        entry.last_success_timestamp = Some(Utc::now().to_rfc3339());
        entry.recompute_score();
    }

    /// Record a document quarantined due to business rules or low quality score.
    pub fn record_quarantine(&self, source: &str, _reason: &str) {
        let norm = source.to_lowercase();
        let mut map = self.metrics.write().unwrap();
        let entry = map
            .entry(norm.clone())
            .or_insert_with(|| SourceMetrics::new(&norm, Self::default_reliability_for_source(&norm)));

        entry.total_records += 1;
        entry.quarantine_count += 1;
        entry.recompute_score();
    }

    /// Record a document rejected due to schema invalidity.
    pub fn record_rejection(&self, source: &str, _reason: &str) {
        let norm = source.to_lowercase();
        let mut map = self.metrics.write().unwrap();
        let entry = map
            .entry(norm.clone())
            .or_insert_with(|| SourceMetrics::new(&norm, Self::default_reliability_for_source(&norm)));

        entry.total_records += 1;
        entry.rejected_records += 1;
        entry.error_count_last_hour += 1;
        entry.recompute_score();
    }
}

/// 24-hour LRU / time-based cache for deduplication of incoming financial documents.
pub struct DuplicateDetector {
    cache: Arc<RwLock<HashMap<u64, DateTime<Utc>>>>,
    max_size: usize,
}

impl DuplicateDetector {
    pub fn new(max_size: usize) -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::with_capacity(max_size.min(10_000)))),
            max_size,
        }
    }

    /// Compute 64-bit hash of natural key (source_id, ticker, published_utc).
    pub fn compute_key(source_id: &str, ticker: &str, published_utc: &str) -> u64 {
        let mut hasher = DefaultHasher::new();
        source_id.hash(&mut hasher);
        ticker.hash(&mut hasher);
        published_utc.hash(&mut hasher);
        hasher.finish()
    }

    /// Check if key was seen within 24 hours. If not, records it and returns false.
    pub fn check_and_insert(&self, source_id: &str, ticker: &str, published_utc: &str) -> bool {
        let key = Self::compute_key(source_id, ticker, published_utc);
        let now = Utc::now();
        let cutoff = now - Duration::hours(24);

        let mut map = self.cache.write().unwrap();

        // 1. Check if already present and fresh
        if let Some(&ts) = map.get(&key) {
            if ts >= cutoff {
                return true; // Duplicate detected!
            }
        }

        // 2. Periodic pruning if cache exceeds threshold
        if map.len() >= self.max_size {
            map.retain(|_, ts| *ts >= cutoff);
            if map.len() >= self.max_size {
                // If still full, clear oldest elements
                let keys_to_remove: Vec<u64> = map.keys().take(map.len() / 4).cloned().collect();
                for k in keys_to_remove {
                    map.remove(&k);
                }
            }
        }

        // 3. Record new entry
        map.insert(key, now);
        false
    }
}

/// Evaluates incoming raw documents across sequential validation gates.
pub struct DataQualityGate {
    config: DataQualityConfig,
    duplicate_detector: Arc<DuplicateDetector>,
}

impl DataQualityGate {
    pub fn new(config: DataQualityConfig) -> Self {
        let duplicate_detector = Arc::new(DuplicateDetector::new(config.duplicate_cache_size));
        Self {
            config,
            duplicate_detector,
        }
    }

    pub fn config(&self) -> &DataQualityConfig {
        &self.config
    }

    /// Evaluate document against:
    /// 1. SCHEMA_VALIDATION -> Reject
    /// 2. BUSINESS_RULES -> Quarantine
    /// 3. DUPLICATE_DETECTION -> Quarantine
    /// 4. SOURCE_QUALITY_THRESHOLD -> Quarantine
    /// 5. All pass -> Accept
    pub fn evaluate(
        &self,
        doc: &RawDocument,
        primary_ticker: Option<&str>,
        source_quality_score: f64,
    ) -> QualityGateResult {
        // ── Gate 1: SCHEMA_VALIDATION ────────────────────────────────────────
        if doc.id.trim().is_empty() {
            return QualityGateResult::Reject {
                rule: "SCHEMA_VALIDATION".to_string(),
                reason: "Document 'id' field is missing or empty".to_string(),
            };
        }
        if doc.title.trim().is_empty() {
            return QualityGateResult::Reject {
                rule: "SCHEMA_VALIDATION".to_string(),
                reason: "Document 'title' field is missing or empty".to_string(),
            };
        }
        if doc.source.trim().is_empty() {
            return QualityGateResult::Reject {
                rule: "SCHEMA_VALIDATION".to_string(),
                reason: "Document 'source' field is missing or empty".to_string(),
            };
        }
        if doc.raw_content.trim().is_empty() && doc.title.trim().is_empty() {
            return QualityGateResult::Reject {
                rule: "SCHEMA_VALIDATION".to_string(),
                reason: "Document content and title are both empty".to_string(),
            };
        }
        let published_dt = match DateTime::parse_from_rfc3339(&doc.published_utc) {
            Ok(dt) => dt.with_timezone(&Utc),
            Err(_) => {
                return QualityGateResult::Reject {
                    rule: "SCHEMA_VALIDATION".to_string(),
                    reason: format!(
                        "Invalid RFC-3339 published timestamp '{}'",
                        doc.published_utc
                    ),
                };
            }
        };

        // ── Gate 2: BUSINESS_RULES ───────────────────────────────────────────
        // Ticker format validation (if ticker extracted)
        if let Some(ticker) = primary_ticker {
            let clean = ticker.trim();
            let is_valid_format = !clean.is_empty()
                && clean.len() <= 10
                && clean
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-');
            if !is_valid_format {
                return QualityGateResult::Quarantine {
                    rule: "BUSINESS_RULES".to_string(),
                    reason: format!("Invalid ticker format '{}'", clean),
                };
            }
        }

        // Timestamp not in the future (with clock skew tolerance)
        let now = Utc::now();
        let max_allowed_time = now + Duration::seconds(self.config.future_skew_tolerance_secs);
        if published_dt > max_allowed_time {
            return QualityGateResult::Quarantine {
                rule: "BUSINESS_RULES".to_string(),
                reason: format!(
                    "Published timestamp '{}' is in the future (current: '{}', tolerance: {}s)",
                    doc.published_utc,
                    now.to_rfc3339(),
                    self.config.future_skew_tolerance_secs
                ),
            };
        }

        // ── Gate 3: DUPLICATE_DETECTION ──────────────────────────────────────
        let source_id = doc.source_id.as_deref().unwrap_or(doc.id.as_str());
        let ticker_key = primary_ticker.unwrap_or("");
        if self
            .duplicate_detector
            .check_and_insert(source_id, ticker_key, &doc.published_utc)
        {
            return QualityGateResult::Quarantine {
                rule: "DUPLICATE_DETECTION".to_string(),
                reason: "duplicate".to_string(),
            };
        }

        // ── Gate 4: SOURCE_QUALITY_THRESHOLD ─────────────────────────────────
        if source_quality_score < self.config.source_quality_threshold {
            return QualityGateResult::Quarantine {
                rule: "SOURCE_QUALITY_THRESHOLD".to_string(),
                reason: format!(
                    "low_source_quality: score {:.3} below threshold {:.3}",
                    source_quality_score, self.config.source_quality_threshold
                ),
            };
        }

        QualityGateResult::Accept
    }
}

/// Atomically commit quarantined documents to disk under `data/quarantine/<source>/<date>/<uuid>.json`.
pub fn quarantine_document(
    quarantine_base: &Path,
    doc: &RawDocument,
    rule: &str,
    reason: &str,
) -> Result<PathBuf, String> {
    let source_sanitized = doc
        .source
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect::<String>();

    let now = Utc::now();
    let date_dir = now.format("%Y-%m-%d").to_string();
    let dir = quarantine_base.join(source_sanitized).join(date_dir);

    if let Err(e) = std::fs::create_dir_all(&dir) {
        return Err(format!(
            "Failed to create quarantine directory '{:?}': {}",
            dir, e
        ));
    }

    let q_uuid = Uuid::new_v4().to_string();
    let final_path = dir.join(format!("{}.json", q_uuid));
    let tmp_path = dir.join(format!("{}.tmp", q_uuid));

    let envelope = serde_json::json!({
        "quarantine_id": q_uuid,
        "quarantined_at": now.to_rfc3339(),
        "rule": rule,
        "reason": reason,
        "source": doc.source,
        "document": doc,
    });

    let content = serde_json::to_string_pretty(&envelope)
        .map_err(|e| format!("Serialization error for quarantine envelope: {}", e))?;

    std::fs::write(&tmp_path, &content)
        .map_err(|e| format!("Failed to write temporary quarantine file '{:?}': {}", tmp_path, e))?;

    std::fs::rename(&tmp_path, &final_path).map_err(|e| {
        format!(
            "Failed to atomically rename '{:?}' to '{:?}': {}",
            tmp_path, final_path, e
        )
    })?;

    warn!(
        "[Data Quality Gate] QUARANTINED document '{}' from '{}' -> {:?} ([{}] {})",
        doc.id, doc.source, final_path, rule, reason
    );

    Ok(final_path)
}

/// Calculate signal freshness in milliseconds based on published timestamp.
pub fn calculate_freshness_ms(published_utc: &str) -> f64 {
    if let Ok(dt) = DateTime::parse_from_rfc3339(published_utc) {
        let diff = Utc::now().signed_duration_since(dt.with_timezone(&Utc));
        diff.num_milliseconds().max(0) as f64
    } else {
        0.0
    }
}

/// Count required fields presence for completeness calculation (6 total fields).
pub fn count_document_fields(doc: &RawDocument) -> (u64, u64) {
    let total = 6u64;
    let mut present = 0u64;

    if !doc.id.trim().is_empty() {
        present += 1;
    }
    if !doc.title.trim().is_empty() {
        present += 1;
    }
    if !doc.source.trim().is_empty() {
        present += 1;
    }
    if !doc.url.trim().is_empty() {
        present += 1;
    }
    if !doc.published_utc.trim().is_empty() {
        present += 1;
    }
    if !doc.raw_content.trim().is_empty() {
        present += 1;
    }

    (present, total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_source_quality_score_calculation() {
        let mut metrics = SourceMetrics::new("sec_edgar", 1.0);
        // Initial state: total=0 -> ratio=1.0, freshness=1.0, completeness=1.0, reliability=1.0
        metrics.recompute_score();
        assert_eq!(metrics.quality_score, 1.0);

        // Record 10 accepted documents with 0ms freshness and full completeness
        for _ in 0..10 {
            metrics.total_records += 1;
            metrics.accepted_records += 1;
            metrics.present_fields_checked += 6;
            metrics.total_fields_checked += 6;
        }
        metrics.recompute_score();
        assert_eq!(metrics.quality_score, 1.0);

        // Add 10 rejected records -> accepted ratio = 10/20 = 0.5
        metrics.total_records += 10;
        metrics.rejected_records += 10;
        metrics.recompute_score();
        // score = 0.4 * 0.5 + 0.2 * 1.0 + 0.2 * 1.0 + 0.2 * 1.0 = 0.2 + 0.6 = 0.8
        assert_eq!(metrics.quality_score, 0.8);
    }

    #[test]
    fn test_freshness_factor_interpolation() {
        let mut metrics = SourceMetrics::new("polygon", 0.9);
        metrics.total_records = 1;
        metrics.accepted_records = 1;
        metrics.total_fields_checked = 6;
        metrics.present_fields_checked = 6;

        // Freshness <= 5min (300k ms) -> factor = 1.0
        metrics.average_freshness_ms = 100_000.0;
        metrics.recompute_score();
        // 0.4*1.0 + 0.2*1.0 + 0.2*1.0 + 0.2*0.9 = 0.4 + 0.2 + 0.2 + 0.18 = 0.98
        assert_eq!(metrics.quality_score, 0.98);

        // Freshness >= 30min (1800k ms) -> factor = 0.5
        metrics.average_freshness_ms = 2_000_000.0;
        metrics.recompute_score();
        // 0.4*1.0 + 0.2*0.5 + 0.2*1.0 + 0.2*0.9 = 0.4 + 0.1 + 0.2 + 0.18 = 0.88
        assert_eq!(metrics.quality_score, 0.88);
    }

    #[test]
    fn test_schema_validation_gate_reject() {
        let gate = DataQualityGate::new(DataQualityConfig::default());

        let invalid_doc = RawDocument {
            id: "".to_string(), // Missing ID!
            title: "Apple Reports Earnings".to_string(),
            source: "finnhub".to_string(),
            url: "https://example.com".to_string(),
            published_utc: "2026-09-06T10:00:00Z".to_string(),
            raw_content: "Content".to_string(),
            ..Default::default()
        };

        let res = gate.evaluate(&invalid_doc, Some("AAPL"), 0.9);
        assert!(matches!(res, QualityGateResult::Reject { .. }));
    }

    #[test]
    fn test_business_rules_gate_future_timestamp_quarantine() {
        let gate = DataQualityGate::new(DataQualityConfig::default());

        let future_time = (Utc::now() + Duration::hours(2)).to_rfc3339();
        let future_doc = RawDocument {
            id: "doc-future-1".to_string(),
            title: "Future News".to_string(),
            source: "finnhub".to_string(),
            url: "https://example.com".to_string(),
            published_utc: future_time,
            raw_content: "Future content".to_string(),
            ..Default::default()
        };

        let res = gate.evaluate(&future_doc, Some("AAPL"), 0.9);
        match res {
            QualityGateResult::Quarantine { rule, reason } => {
                assert_eq!(rule, "BUSINESS_RULES");
                assert!(reason.contains("future"));
            }
            other => panic!("Expected Quarantine, got {:?}", other),
        }
    }

    #[test]
    fn test_business_rules_gate_invalid_ticker_quarantine() {
        let gate = DataQualityGate::new(DataQualityConfig::default());

        let doc = RawDocument {
            id: "doc-bad-ticker-1".to_string(),
            title: "Invalid Ticker News".to_string(),
            source: "polygon".to_string(),
            url: "https://example.com".to_string(),
            published_utc: "2026-09-06T10:00:00Z".to_string(),
            raw_content: "Some news text".to_string(),
            ..Default::default()
        };

        let res = gate.evaluate(&doc, Some("INVALID$$$$TOOLONGTICKER123"), 0.9);
        match res {
            QualityGateResult::Quarantine { rule, reason } => {
                assert_eq!(rule, "BUSINESS_RULES");
                assert!(reason.contains("Invalid ticker format"));
            }
            other => panic!("Expected Quarantine, got {:?}", other),
        }
    }

    #[test]
    fn test_duplicate_detection_quarantine() {
        let gate = DataQualityGate::new(DataQualityConfig::default());

        let doc = RawDocument {
            id: "doc-dup-1".to_string(),
            source_id: Some("article-12345".to_string()),
            title: "Duplicate Article".to_string(),
            source: "finnhub".to_string(),
            url: "https://example.com".to_string(),
            published_utc: "2026-09-06T10:00:00Z".to_string(),
            raw_content: "Duplicate content".to_string(),
            ..Default::default()
        };

        // First pass: accepted
        let res1 = gate.evaluate(&doc, Some("AAPL"), 0.9);
        assert_eq!(res1, QualityGateResult::Accept);

        // Second pass with same natural key: quarantined as duplicate!
        let res2 = gate.evaluate(&doc, Some("AAPL"), 0.9);
        match res2 {
            QualityGateResult::Quarantine { rule, reason } => {
                assert_eq!(rule, "DUPLICATE_DETECTION");
                assert_eq!(reason, "duplicate");
            }
            other => panic!("Expected Quarantine duplicate, got {:?}", other),
        }
    }

    #[test]
    fn test_source_quality_threshold_quarantine() {
        let gate = DataQualityGate::new(DataQualityConfig {
            source_quality_threshold: 0.60,
            ..Default::default()
        });

        let doc = RawDocument {
            id: "doc-low-qual-1".to_string(),
            title: "Low Quality Source News".to_string(),
            source: "unreliable_mock".to_string(),
            url: "https://example.com".to_string(),
            published_utc: "2026-09-06T10:00:00Z".to_string(),
            raw_content: "Content".to_string(),
            ..Default::default()
        };

        // Source score 0.45 < threshold 0.60 -> Quarantine
        let res = gate.evaluate(&doc, Some("AAPL"), 0.45);
        match res {
            QualityGateResult::Quarantine { rule, reason } => {
                assert_eq!(rule, "SOURCE_QUALITY_THRESHOLD");
                assert!(reason.contains("low_source_quality"));
            }
            other => panic!("Expected Quarantine low quality, got {:?}", other),
        }
    }

    #[test]
    fn test_atomic_quarantine_storage() {
        let temp_dir = std::env::temp_dir().join(format!("fintext_quarantine_test_{}", Uuid::new_v4()));
        let doc = RawDocument {
            id: "test-quarantine-doc".to_string(),
            title: "Quarantined Report".to_string(),
            source: "test_source".to_string(),
            url: "https://example.com/q".to_string(),
            published_utc: "2026-09-06T10:00:00Z".to_string(),
            raw_content: "Quarantined body".to_string(),
            ..Default::default()
        };

        let path = quarantine_document(&temp_dir, &doc, "BUSINESS_RULES", "Test quarantine write")
            .expect("Quarantine write should succeed");

        assert!(path.exists());
        let content = std::fs::read_to_string(&path).expect("File must be readable");
        assert!(content.contains("Quarantined Report"));
        assert!(content.contains("BUSINESS_RULES"));

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
