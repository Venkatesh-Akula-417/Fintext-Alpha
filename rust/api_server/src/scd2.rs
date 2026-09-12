//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Slowly Changing Dimension Type 2 (SCD2) Registry
//! ═══════════════════════════════════════════════════════════════════════════════
//!
//! Manages point-in-time revision history for sentiment signals. Ensures true
//! as-of querying, historical preservation of amended or corrected filings,
//! and complete elimination of look-ahead bias in financial backtesting.

use chrono::{DateTime, Duration, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::info;
use utoipa::ToSchema;

use crate::models::SentimentRecord;

/// DTO for ingesting or triggering a sentiment record revision.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RevisionIngestRequest {
    /// Target stock ticker symbol (e.g., 'AAPL', 'NVDA')
    #[schema(example = "AAPL")]
    pub ticker: String,
    /// Revised headline title or amendment filing summary
    #[schema(example = "Apple Q3 Guidance Revised Higher in SEC Form 8-K/A Amendment")]
    pub title: String,
    /// Revised point-in-time sentiment score (-1.0 to 1.0)
    #[schema(example = 0.72)]
    pub sentiment_score: f64,
    /// Originating source or agency
    #[schema(example = "SEC EDGAR")]
    pub source: Option<String>,
    /// Optional natural key or filing accession identifier
    #[schema(example = "0000320193-25-000055")]
    pub source_id: Option<String>,
    /// Ingestion timestamp (ISO-8601 UTC). Defaults to current time.
    #[schema(example = "2026-08-26T09:15:00.050000Z")]
    pub ingested_utc: Option<String>,
    /// Database commit timestamp (ISO-8601 UTC). Defaults to current time.
    #[schema(example = "2026-08-26T09:15:00.100000Z")]
    pub db_commit_utc: Option<String>,
}

/// DTO response after applying an SCD2 revision.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RevisionIngestResponse {
    /// Target stock ticker
    #[schema(example = "AAPL")]
    pub ticker: String,
    /// Message summarizing the SCD2 operation
    #[schema(example = "Successfully created revision 2, superseded revision 1")]
    pub message: String,
    /// The newly created active revision
    pub active_revision: SentimentRecord,
    /// The previous revision that was superseded (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub superseded_revision: Option<SentimentRecord>,
}

/// DTO response for inspecting all revisions of a ticker.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RevisionHistoryListResponse {
    /// Target stock ticker
    #[schema(example = "AAPL")]
    pub ticker: String,
    /// Total count of revision versions
    #[schema(example = 2)]
    pub total_revisions: usize,
    /// Complete ordered lineage of revision snapshots
    pub revisions: Vec<SentimentRecord>,
}

/// Thread-safe in-memory registry for SCD Type 2 revision history.
#[derive(Debug, Clone)]
pub struct Scd2RevisionRegistry {
    /// Maps uppercase ticker symbol to ordered list of revisions (Version 1, Version 2, etc.)
    records: Arc<DashMap<String, Vec<SentimentRecord>>>,
    /// Fast memory-bounded TTL cache for current active revisions
    active_cache: Arc<crate::cache::TtlCache<String, SentimentRecord>>,
}

impl Default for Scd2RevisionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl Scd2RevisionRegistry {
    /// Create a new registry pre-populated with deterministic benchmark revisions.
    pub fn new() -> Self {
        let cache_cfg = crate::cache::CacheConfig::from_env_or_config();
        let registry = Self {
            records: Arc::new(DashMap::new()),
            active_cache: Arc::new(crate::cache::TtlCache::with_ttl_secs(
                cache_cfg.default_ttl_secs,
                cache_cfg.default_max_capacity,
            )),
        };

        // Seed AAPL with a deterministic 2-stage revision history:
        // Version 1: Released on 2025-06-15, valid until 2025-06-16T10:00:00Z
        // Version 2: SEC 8-K/A amendment released 2025-06-16T10:00:00Z, valid onwards
        let aapl_v1 = SentimentRecord {
            published_utc: "2025-06-15T14:30:00.000000Z".to_string(),
            ticker: "AAPL".to_string(),
            source: "SEC EDGAR".to_string(),
            title: "Apple Q3 Guidance Beat with Services Growth".to_string(),
            sentiment_score: 0.45,
            vpin: 0.48,
            gamma_exposure: 150000.0,
            data_quality_score: 0.92,
            model_version: Some("finbert-minilm-v2.1".to_string()),
            pipeline_version: Some("2.0.0".to_string()),
            data_provenance: Some(vec![
                "SEC EDGAR".to_string(),
                "Finnhub".to_string(),
                "Polygon".to_string(),
            ]),
            language: "english".to_string(),
            ingested_utc: Some("2025-06-15T14:30:00.050000Z".to_string()),
            db_commit_utc: Some("2025-06-15T14:30:00.100000Z".to_string()),
            valid_from: Some("2025-06-15T14:30:00.100000Z".to_string()),
            valid_to: Some("2025-06-16T10:00:00.000000Z".to_string()),
            revision_number: Some(1),
            is_current: Some(false),
        };

        let aapl_v2 = SentimentRecord {
            published_utc: "2025-06-16T09:59:00.000000Z".to_string(),
            ticker: "AAPL".to_string(),
            source: "SEC EDGAR".to_string(),
            title: "Apple Q3 Guidance Revised Higher in SEC Form 8-K/A Amendment".to_string(),
            sentiment_score: 0.72,
            vpin: 0.52,
            gamma_exposure: 220000.0,
            data_quality_score: 0.95,
            model_version: Some("finbert-minilm-v2.1".to_string()),
            pipeline_version: Some("2.0.0".to_string()),
            data_provenance: Some(vec![
                "SEC EDGAR".to_string(),
                "Finnhub".to_string(),
                "Polygon".to_string(),
            ]),
            language: "english".to_string(),
            ingested_utc: Some("2025-06-16T09:59:30.000000Z".to_string()),
            db_commit_utc: Some("2025-06-16T10:00:00.000000Z".to_string()),
            valid_from: Some("2025-06-16T10:00:00.000000Z".to_string()),
            valid_to: None,
            revision_number: Some(2),
            is_current: Some(true),
        };

        registry
            .records
            .insert("AAPL".to_string(), vec![aapl_v1, aapl_v2]);

        // Seed MSFT baseline
        let msft_v1 = SentimentRecord {
            published_utc: "2025-06-15T14:30:00.000000Z".to_string(),
            ticker: "MSFT".to_string(),
            source: "Institutional Wire".to_string(),
            title: "Microsoft Cloud Revenue Expands 29% YoY".to_string(),
            sentiment_score: 0.58,
            vpin: 0.42,
            gamma_exposure: 180000.0,
            data_quality_score: 0.90,
            model_version: Some("finbert-minilm-v2.1".to_string()),
            pipeline_version: Some("2.0.0".to_string()),
            data_provenance: Some(vec![
                "SEC EDGAR".to_string(),
                "Finnhub".to_string(),
                "Polygon".to_string(),
            ]),
            language: "english".to_string(),
            ingested_utc: Some("2025-06-15T14:30:00.050000Z".to_string()),
            db_commit_utc: Some("2025-06-15T14:30:00.100000Z".to_string()),
            valid_from: Some("2025-06-15T14:30:00.100000Z".to_string()),
            valid_to: None,
            revision_number: Some(1),
            is_current: Some(true),
        };
        registry.records.insert("MSFT".to_string(), vec![msft_v1]);

        registry
    }

    /// Retrieve all revision versions for a ticker.
    pub fn get_revisions_for_ticker(&self, ticker: &str) -> Vec<SentimentRecord> {
        let ticker_upper = ticker.trim().to_uppercase();
        self.records
            .get(&ticker_upper)
            .map(|entry| entry.clone())
            .unwrap_or_default()
    }

    /// Query a sentiment record as of a given point in time (`as_of_utc`).
    /// If `as_of_utc` is None, returns the current active revision (`is_current = true`).
    pub fn query_as_of(&self, ticker: &str, as_of_utc: Option<&str>) -> Option<SentimentRecord> {
        let ticker_upper = ticker.trim().to_uppercase();
        let versions = self.records.get(&ticker_upper)?;

        match as_of_utc {
            Some(as_of_str) if !as_of_str.trim().is_empty() => {
                let parsed_as_of = DateTime::parse_from_rfc3339(as_of_str.trim())
                    .ok()?
                    .with_timezone(&Utc);

                // Find version valid at as_of: valid_from <= as_of AND (valid_to IS NULL OR valid_to > as_of)
                versions
                    .iter()
                    .filter(|r| {
                        let from_ok = r
                            .valid_from
                            .as_deref()
                            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                            .map(|dt| dt <= parsed_as_of)
                            .unwrap_or(false);

                        let to_ok = match r.valid_to.as_deref() {
                            Some(to_str) => DateTime::parse_from_rfc3339(to_str)
                                .ok()
                                .map(|dt| dt > parsed_as_of)
                                .unwrap_or(false),
                            None => true,
                        };

                        from_ok && to_ok
                    })
                    .last()
                    .cloned()
            }
            _ => {
                // Check active cache first
                if let Some(cached) = self.active_cache.get(&ticker_upper) {
                    return Some(cached);
                }
                // Return latest active version and cache it
                let active = versions
                    .iter()
                    .find(|r| r.is_current == Some(true))
                    .or_else(|| versions.last())
                    .cloned();
                if let Some(ref rec) = active {
                    self.active_cache.insert(ticker_upper, rec.clone());
                }
                active
            }
        }
    }

    /// Insert or append a new revision version for a ticker.
    /// Supersedes the existing active version by setting `valid_to = new_valid_from` and `is_current = false`.
    pub fn insert_revision(
        &self,
        req: RevisionIngestRequest,
    ) -> Result<(SentimentRecord, Option<SentimentRecord>), String> {
        let ticker = req.ticker.trim().to_uppercase();
        if ticker.is_empty() {
            return Err("Ticker cannot be empty".to_string());
        }

        let now = Utc::now();
        let pub_utc = now.to_rfc3339();
        let ing_utc = req
            .ingested_utc
            .unwrap_or_else(|| (now + Duration::milliseconds(50)).to_rfc3339());
        let db_utc = req
            .db_commit_utc
            .unwrap_or_else(|| (now + Duration::milliseconds(100)).to_rfc3339());
        let valid_from = db_utc.clone();

        let source = req.source.unwrap_or_else(|| "SEC EDGAR".to_string());

        let mut entry = self.records.entry(ticker.clone()).or_insert_with(Vec::new);

        // Find currently active version to supersede
        let mut superseded = None;
        let mut next_revision_number = 1;

        for r in entry.iter_mut() {
            if r.is_current == Some(true) {
                r.is_current = Some(false);
                r.valid_to = Some(valid_from.clone());
                superseded = Some(r.clone());
                next_revision_number = r.revision_number.unwrap_or(1) + 1;
            }
        }

        let new_record = SentimentRecord {
            published_utc: pub_utc,
            ticker: ticker.clone(),
            source: source.clone(),
            title: req.title,
            sentiment_score: req.sentiment_score,
            vpin: 0.50,
            gamma_exposure: 100000.0,
            data_quality_score: 0.90,
            model_version: Some("finbert-minilm-v2.1".to_string()),
            pipeline_version: Some("2.0.0".to_string()),
            data_provenance: Some(vec![source]),
            language: "english".to_string(),
            ingested_utc: Some(ing_utc),
            db_commit_utc: Some(db_utc),
            valid_from: Some(valid_from),
            valid_to: None,
            revision_number: Some(next_revision_number),
            is_current: Some(true),
        };

        entry.push(new_record.clone());
        self.active_cache.insert(ticker.clone(), new_record.clone());
        info!(
            "[SCD2 Registry] Created revision {} for ticker {} (superseded rev: {:?})",
            next_revision_number,
            ticker,
            superseded.as_ref().and_then(|s| s.revision_number)
        );

        Ok((new_record, superseded))
    }

    /// Purge expired entries from the active revision cache.
    pub fn remove_expired(&self) -> usize {
        self.active_cache.remove_expired()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scd2_registry_seed_and_query_as_of() {
        let registry = Scd2RevisionRegistry::new();

        // 1. Query latest AAPL (should be revision 2)
        let latest = registry
            .query_as_of("AAPL", None)
            .expect("Should find AAPL");
        assert_eq!(latest.revision_number, Some(2));
        assert_eq!(latest.is_current, Some(true));
        assert_eq!(latest.sentiment_score, 0.72);

        // 2. Query AAPL as of 2025-06-15T18:00:00Z (should be revision 1)
        let as_of_v1 = registry
            .query_as_of("AAPL", Some("2025-06-15T18:00:00Z"))
            .expect("Should find AAPL v1");
        assert_eq!(as_of_v1.revision_number, Some(1));
        assert_eq!(as_of_v1.sentiment_score, 0.45);
        assert_eq!(
            as_of_v1.valid_to,
            Some("2025-06-16T10:00:00.000000Z".to_string())
        );

        // 3. Query AAPL as of 2025-06-16T12:00:00Z (should be revision 2)
        let as_of_v2 = registry
            .query_as_of("AAPL", Some("2025-06-16T12:00:00Z"))
            .expect("Should find AAPL v2");
        assert_eq!(as_of_v2.revision_number, Some(2));
        assert_eq!(as_of_v2.sentiment_score, 0.72);

        // 4. Query before published (should be None)
        let before_all = registry.query_as_of("AAPL", Some("2025-06-01T00:00:00Z"));
        assert!(before_all.is_none());
    }

    #[test]
    fn test_scd2_insert_revision_continuity() {
        let registry = Scd2RevisionRegistry::new();

        let req = RevisionIngestRequest {
            ticker: "NVDA".to_string(),
            title: "NVIDIA Preliminary Revenue".to_string(),
            sentiment_score: 0.40,
            source: Some("SEC EDGAR".to_string()),
            source_id: Some("nvda-prelim".to_string()),
            ingested_utc: Some("2026-08-25T14:30:00.050Z".to_string()),
            db_commit_utc: Some("2026-08-25T14:30:00.100Z".to_string()),
        };

        let (v1, old) = registry.insert_revision(req).unwrap();
        assert_eq!(v1.revision_number, Some(1));
        assert_eq!(v1.is_current, Some(true));
        assert!(old.is_none());

        // Now issue revision 2
        let req_v2 = RevisionIngestRequest {
            ticker: "NVDA".to_string(),
            title: "NVIDIA Revised Higher Revenue (Form 8-K/A)".to_string(),
            sentiment_score: 0.85,
            source: Some("SEC EDGAR".to_string()),
            source_id: Some("nvda-amendment".to_string()),
            ingested_utc: Some("2026-08-26T10:00:00.050Z".to_string()),
            db_commit_utc: Some("2026-08-26T10:00:00.100Z".to_string()),
        };

        let (v2, superseded) = registry.insert_revision(req_v2).unwrap();
        assert_eq!(v2.revision_number, Some(2));
        assert_eq!(v2.is_current, Some(true));
        let old_v1 = superseded.expect("Should have superseded v1");
        assert_eq!(old_v1.revision_number, Some(1));
        assert_eq!(old_v1.is_current, Some(false));
        assert_eq!(old_v1.valid_to, v2.valid_from); // Continuity invariant!
    }
}
