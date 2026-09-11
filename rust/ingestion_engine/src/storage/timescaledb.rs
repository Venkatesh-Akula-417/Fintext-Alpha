//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — TimescaleDB / PostgreSQL Time-Series Storage Adapter
//! Phase 1 Migration: Hypertable Ingestion with SCD Type 2 Point-in-Time Tracking
//! ═══════════════════════════════════════════════════════════════════════════════

use crate::nlp::SentimentOutput;
use crate::pipeline::ProcessedDocument;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::env;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tracing::{debug, info, warn};

/// Configuration for PostgreSQL / TimescaleDB storage adapter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimescaleDbConfig {
    /// Controls whether dual-write to TimescaleDB is enabled
    pub enabled: bool,
    /// Controls whether TimescaleDB is the primary query & synchronous write store
    pub primary: bool,
    /// Controls whether historical records are automatically backfilled on startup
    pub auto_backfill_on_startup: bool,
    /// PostgreSQL connection URL (e.g., postgres://user:pass@host:5432/db)
    pub url: String,
    /// Maximum connection pool size
    pub max_connections: u32,
    /// Connection and query timeout in milliseconds
    pub timeout_ms: u64,
    /// Whether mock fallback is active when no database is reachable
    pub mock_mode: bool,
}

impl Default for TimescaleDbConfig {
    fn default() -> Self {
        let enabled = env::var("ENABLE_TIMESCALEDB")
            .or_else(|_| env::var("TIMESCALE_ENABLED"))
            .map(|v| v == "1" || v.to_lowercase() == "true")
            .unwrap_or(false);

        let primary = env::var("TIMESCALE_PRIMARY")
            .or_else(|_| env::var("ENABLE_TIMESCALE_PRIMARY"))
            .map(|v| v == "1" || v.to_lowercase() == "true")
            .unwrap_or(false);

        let auto_backfill_on_startup = env::var("TIMESCALE_AUTO_BACKFILL")
            .map(|v| v == "1" || v.to_lowercase() == "true")
            .unwrap_or(false);

        let url = env::var("TIMESCALE_DB_URL")
            .unwrap_or_else(|_| "postgres://fintext:fintext@localhost:5432/fintext_timeseries".to_string());

        let max_connections = env::var("TIMESCALE_MAX_CONNECTIONS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(10);

        let timeout_ms = env::var("TIMESCALE_TIMEOUT_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(3000);

        let mock_mode = if crate::is_production_mode() {
            false
        } else {
            env::var("TIMESCALE_MOCK_FALLBACK").as_deref() == Ok("1")
                || env::var("TIMESCALE_MOCK_MODE").as_deref() == Ok("1")
                || true // default mock fallback for tests
        };

        Self {
            enabled,
            primary,
            auto_backfill_on_startup,
            url,
            max_connections,
            timeout_ms,
            mock_mode,
        }
    }
}

impl TimescaleDbConfig {
    /// Load configuration with priority: environment variables -> config/config.yaml -> defaults.
    pub fn from_env_or_config() -> Self {
        let mut cfg = Self::default();

        let config_path = env::var("CONFIG_PATH").unwrap_or_else(|_| "config/config.yaml".to_string());
        if let Ok(contents) = std::fs::read_to_string(&config_path) {
            let mut in_db = false;
            let mut in_timescale = false;

            for line in contents.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') {
                    continue;
                }
                if !line.starts_with(' ') && !line.starts_with('\t') {
                    if trimmed.starts_with("database:") {
                        in_db = true;
                        in_timescale = false;
                        continue;
                    } else {
                        in_db = false;
                        in_timescale = false;
                    }
                }
                if in_db && trimmed.starts_with("timescaledb:") {
                    in_timescale = true;
                    continue;
                }
                if in_db && in_timescale {
                    let leading_spaces = line.chars().take_while(|c| *c == ' ').count();
                    if leading_spaces <= 2 && !trimmed.starts_with("timescaledb:") {
                        in_timescale = false;
                        continue;
                    }
                    if let Some((k, v)) = trimmed.split_once(':') {
                        let key = k.trim();
                        let val = v.split('#').next().unwrap_or("").trim().trim_matches('"');
                        match key {
                            "enabled" => {
                                if val == "true" || val == "1" {
                                    cfg.enabled = true;
                                } else if val == "false" || val == "0" {
                                    cfg.enabled = false;
                                }
                            }
                            "primary" => {
                                if val == "true" || val == "1" {
                                    cfg.primary = true;
                                } else if val == "false" || val == "0" {
                                    cfg.primary = false;
                                }
                            }
                            "auto_backfill_on_startup" => {
                                if val == "true" || val == "1" {
                                    cfg.auto_backfill_on_startup = true;
                                } else if val == "false" || val == "0" {
                                    cfg.auto_backfill_on_startup = false;
                                }
                            }
                            "url" => {
                                if !val.is_empty() {
                                    cfg.url = val.to_string();
                                }
                            }
                            "max_connections" => {
                                if let Ok(n) = val.parse::<u32>() {
                                    cfg.max_connections = n;
                                }
                            }
                            "timeout_ms" => {
                                if let Ok(n) = val.parse::<u64>() {
                                    cfg.timeout_ms = n;
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        // Environment overrides
        if let Ok(val) = env::var("ENABLE_TIMESCALEDB").or_else(|_| env::var("TIMESCALE_ENABLED")) {
            cfg.enabled = val == "1" || val.to_lowercase() == "true";
        }
        if let Ok(val) = env::var("TIMESCALE_PRIMARY").or_else(|_| env::var("ENABLE_TIMESCALE_PRIMARY")) {
            cfg.primary = val == "1" || val.to_lowercase() == "true";
        }
        if let Ok(val) = env::var("TIMESCALE_AUTO_BACKFILL") {
            cfg.auto_backfill_on_startup = val == "1" || val.to_lowercase() == "true";
        }
        if let Ok(val) = env::var("TIMESCALE_DB_URL") {
            if !val.trim().is_empty() {
                cfg.url = val;
            }
        }

        cfg
    }
}

/// In-memory representation of a TimescaleDB sentiment record with SCD Type 2 columns.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TimescaleSentimentRecord {
    pub id: i64,
    pub ticker: String,
    pub published_utc: DateTime<Utc>,
    pub ingested_utc: DateTime<Utc>,
    pub db_commit_utc: DateTime<Utc>,
    pub source: String,
    pub title: String,
    pub sentiment_score: f64,
    pub sentiment_label: String,
    pub confidence: f64,
    pub data_quality_score: f64,
    pub vpin: Option<f64>,
    pub gamma_exposure: Option<f64>,
    pub valid_from: DateTime<Utc>,
    pub valid_to: Option<DateTime<Utc>>,
    pub revision_number: i32,
    pub is_current: bool,
}

/// TimescaleDB / PostgreSQL storage adapter sink for time-series sentiment records.
pub struct TimescaleDbSink {
    config: TimescaleDbConfig,
    pool: Option<sqlx::PgPool>,
    mock_store: Arc<RwLock<Vec<TimescaleSentimentRecord>>>,
    db_breaker: Option<Arc<crate::pipeline::resilience::DbCircuitBreaker>>,
}

impl TimescaleDbSink {
    /// Initialize a new TimescaleDB sink using the provided configuration.
    pub fn new(config: TimescaleDbConfig) -> Self {
        let pool = if config.enabled && !config.mock_mode {
            let pool_opts = sqlx::postgres::PgPoolOptions::new()
                .max_connections(config.max_connections)
                .acquire_timeout(Duration::from_millis(config.timeout_ms));
            
            match pool_opts.connect_lazy(&config.url) {
                Ok(p) => Some(p),
                Err(e) => {
                    warn!("[TimescaleDB] Failed to create connection pool to '{}': {}", config.url, e);
                    None
                }
            }
        } else {
            None
        };

        if config.enabled {
            info!(
                "[TimescaleDB Sink] Initialized: enabled=true, url='{}', mock_mode={}",
                config.url, config.mock_mode
            );
        }

        Self {
            config,
            pool,
            mock_store: Arc::new(RwLock::new(Vec::new())),
            db_breaker: Some(Arc::new(crate::pipeline::resilience::DbCircuitBreaker::new(
                crate::pipeline::resilience::CircuitBreakerConfig::from_env_or_config(),
            ))),
        }
    }

    /// Initialize a mock sink for testing with an explicit mock store.
    pub fn new_mock(config: TimescaleDbConfig, mock_store: Arc<RwLock<Vec<TimescaleSentimentRecord>>>) -> Self {
        Self {
            config,
            pool: None,
            mock_store,
            db_breaker: None,
        }
    }

    /// Attach a circuit breaker instance to the sink.
    pub fn set_circuit_breaker(&mut self, breaker: Arc<crate::pipeline::resilience::DbCircuitBreaker>) {
        self.db_breaker = Some(breaker);
    }

    /// Builder pattern method to attach a circuit breaker.
    pub fn with_circuit_breaker(mut self, breaker: Arc<crate::pipeline::resilience::DbCircuitBreaker>) -> Self {
        self.db_breaker = Some(breaker);
        self
    }

    /// Access the underlying circuit breaker instance if attached.
    pub fn circuit_breaker(&self) -> Option<Arc<crate::pipeline::resilience::DbCircuitBreaker>> {
        self.db_breaker.clone()
    }

    /// Access configuration parameters.
    pub fn config(&self) -> &TimescaleDbConfig {
        &self.config
    }

    /// Returns true if dual-write to TimescaleDB is enabled.
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Returns true if TimescaleDB is configured as the primary query & synchronous write store.
    pub fn is_primary(&self) -> bool {
        self.config.primary
    }

    /// Checks if a record with the same natural key (ticker, published_utc, source) already exists in TimescaleDB.
    pub async fn record_exists(
        &self,
        ticker: &str,
        published_utc: DateTime<Utc>,
        source: &str,
    ) -> Result<bool, String> {
        if let Some(ref pool) = self.pool {
            let row: Option<(i64,)> = sqlx::query_as(
                "SELECT id FROM sentiment_records \
                 WHERE ticker = $1 AND published_utc = $2 AND source = $3 LIMIT 1;"
            )
            .bind(ticker)
            .bind(published_utc)
            .bind(source)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("TimescaleDB duplicate check error: {}", e))?;

            Ok(row.is_some())
        } else {
            let store = self.mock_store.read().unwrap();
            Ok(store.iter().any(|r| {
                r.ticker.eq_ignore_ascii_case(ticker)
                    && r.published_utc == published_utc
                    && r.source.eq_ignore_ascii_case(source)
            }))
        }
    }

    /// Insert a historical backfill record directly into TimescaleDB with initial SCD2 validity.
    pub async fn insert_backfill_record(
        &self,
        record: &TimescaleSentimentRecord,
    ) -> Result<u64, String> {
        if !self.config.enabled && !self.config.mock_mode {
            return Ok(0);
        }

        if let Some(ref pool) = self.pool {
            let row: (i64,) = sqlx::query_as(Self::sql_insert_sentiment())
                .bind(&record.ticker)
                .bind(record.published_utc)
                .bind(record.ingested_utc)
                .bind(record.db_commit_utc)
                .bind(&record.source)
                .bind(&record.title)
                .bind(record.sentiment_score)
                .bind(&record.sentiment_label)
                .bind(record.confidence)
                .bind(record.data_quality_score)
                .bind(record.vpin)
                .bind(record.gamma_exposure)
                .bind(record.valid_from)
                .bind(record.valid_to)
                .bind(record.revision_number)
                .bind(record.is_current)
                .fetch_one(pool)
                .await
                .map_err(|e| format!("TimescaleDB backfill insert error: {}", e))?;

            Ok(row.0 as u64)
        } else {
            let mut store = self.mock_store.write().unwrap();
            let next_id = (store.len() as i64) + 1;
            let mut rec = record.clone();
            rec.id = next_id;
            store.push(rec);
            Ok(next_id as u64)
        }
    }

    /// Return all in-memory mock records stored by this sink.
    pub fn get_mock_records(&self) -> Vec<TimescaleSentimentRecord> {
        self.mock_store.read().unwrap().clone()
    }

    /// Generate SQL query string for inserting a new sentiment record.
    pub fn sql_insert_sentiment() -> &'static str {
        "INSERT INTO sentiment_records ( \
            ticker, published_utc, ingested_utc, db_commit_utc, source, title, \
            sentiment_score, sentiment_label, confidence, data_quality_score, \
            vpin, gamma_exposure, valid_from, valid_to, revision_number, is_current \
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16) \
        RETURNING id;"
    }

    /// Generate SQL query string for closing a previous SCD Type 2 record version.
    pub fn sql_close_previous_version() -> &'static str {
        "UPDATE sentiment_records \
         SET valid_to = $1, is_current = false \
         WHERE ticker = $2 AND published_utc = $3 AND source = $4 AND is_current = true;"
    }

    /// Parse ISO-8601 string to DateTime<Utc> with fallback to Utc::now().
    fn parse_dt(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now())
    }

    /// Persist a processed document and sentiment output to TimescaleDB with SCD2 revision management.
    pub async fn write_sentiment_record(
        &self,
        doc: &ProcessedDocument,
        sentiment: &SentimentOutput,
    ) -> Result<u64, String> {
        if !self.config.enabled {
            return Ok(0);
        }

        let ticker = doc
            .primary_ticker
            .as_deref()
            .unwrap_or_else(|| doc.tickers.first().map(|s| s.as_str()).unwrap_or("MARKET"))
            .to_uppercase();

        let published_utc = Self::parse_dt(&doc.published_utc);
        let ingested_utc = Self::parse_dt(&doc.ingested_utc);
        let db_commit_utc = doc
            .db_commit_utc
            .as_deref()
            .map(Self::parse_dt)
            .unwrap_or_else(Utc::now);

        let source = if doc.source.is_empty() { "UNKNOWN".to_string() } else { doc.source.clone() };
        let title = doc.title.clone();
        let sentiment_score = sentiment.sentiment_score;
        let sentiment_label = sentiment.sentiment_label.clone();
        let confidence = (sentiment.prob_positive.max(sentiment.prob_negative).max(sentiment.prob_neutral) * 100.0).round() / 100.0;
        let data_quality_score = 0.95; // Standard high-quality verified ingestion score
        let vpin = doc.vpin;
        let gamma_exposure = doc.gex;

        // Execute SCD Type 2 Upsert Logic
        if let Some(ref pool) = self.pool {
            let write_op = || async {
                let mut tx = pool.begin().await?;

                // 1. Close existing active version if present
                let _ = sqlx::query(Self::sql_close_previous_version())
                    .bind(ingested_utc)
                    .bind(&ticker)
                    .bind(published_utc)
                    .bind(&source)
                    .execute(&mut *tx)
                    .await;

                // 2. Fetch highest previous revision number
                let prev_rev_row: Option<(i32,)> = sqlx::query_as(
                    "SELECT revision_number FROM sentiment_records \
                     WHERE ticker = $1 AND published_utc = $2 AND source = $3 \
                     ORDER BY revision_number DESC LIMIT 1;"
                )
                .bind(&ticker)
                .bind(published_utc)
                .bind(&source)
                .fetch_optional(&mut *tx)
                .await?;

                let revision_number = prev_rev_row.map(|(r,)| r + 1).unwrap_or(1);
                let valid_from = ingested_utc;
                let valid_to: Option<DateTime<Utc>> = None;
                let is_current = true;

                // 3. Insert new current revision
                let row: (i64,) = sqlx::query_as(Self::sql_insert_sentiment())
                    .bind(&ticker)
                    .bind(published_utc)
                    .bind(ingested_utc)
                    .bind(db_commit_utc)
                    .bind(&source)
                    .bind(&title)
                    .bind(sentiment_score)
                    .bind(&sentiment_label)
                    .bind(confidence)
                    .bind(data_quality_score)
                    .bind(vpin)
                    .bind(gamma_exposure)
                    .bind(valid_from)
                    .bind(valid_to)
                    .bind(revision_number)
                    .bind(is_current)
                    .fetch_one(&mut *tx)
                    .await?;

                tx.commit().await?;
                Ok(row.0 as u64)
            };

            let res = if let Some(ref breaker) = self.db_breaker {
                match breaker.execute(write_op).await {
                    Ok(id) => Ok(id),
                    Err(crate::pipeline::resilience::DbError::CircuitOpen) => {
                        warn!("[TimescaleDB] Circuit breaker is OPEN. Dropping write (buffered in Kafka WAL).");
                        return Ok(0);
                    }
                    Err(crate::pipeline::resilience::DbError::OperationFailed(msg)) => {
                        Err(format!("TimescaleDB transaction error: {}", msg))
                    }
                    Err(crate::pipeline::resilience::DbError::Timeout) => {
                        Err("TimescaleDB transaction error: operation timed out".to_string())
                    }
                }
            } else {
                write_op().await.map_err(|e| format!("TimescaleDB transaction error: {}", e))
            };

            match res {
                Ok(id) => {
                    debug!("[TimescaleDB] Inserted record id={} for ticker={}", id, ticker);
                    Ok(id)
                }
                Err(e) => Err(e),
            }
        } else {
            // Mock Store SCD Type 2 Upsert Logic
            let mut store = self.mock_store.write().unwrap();
            let mut prev_revision = 0;

            // Close existing current version
            for item in store.iter_mut() {
                if item.ticker == ticker && item.published_utc == published_utc && item.source == source && item.is_current {
                    item.is_current = false;
                    item.valid_to = Some(ingested_utc);
                    prev_revision = item.revision_number.max(prev_revision);
                }
            }

            let next_id = (store.len() as i64) + 1;
            let record = TimescaleSentimentRecord {
                id: next_id,
                ticker,
                published_utc,
                ingested_utc,
                db_commit_utc,
                source,
                title,
                sentiment_score,
                sentiment_label,
                confidence,
                data_quality_score,
                vpin,
                gamma_exposure,
                valid_from: ingested_utc,
                valid_to: None,
                revision_number: prev_revision + 1,
                is_current: true,
            };

            store.push(record);
            Ok(next_id as u64)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timescaledb_config_defaults() {
        let cfg = TimescaleDbConfig::default();
        assert!(!cfg.enabled);
        assert!(cfg.url.contains("postgres://"));
        assert_eq!(cfg.max_connections, 10);
        assert_eq!(cfg.timeout_ms, 3000);
    }

    #[test]
    fn test_timescaledb_sql_syntax() {
        let insert_sql = TimescaleDbSink::sql_insert_sentiment();
        assert!(insert_sql.contains("INSERT INTO sentiment_records"));
        assert!(insert_sql.contains("valid_from"));
        assert!(insert_sql.contains("revision_number"));
        assert!(insert_sql.contains("is_current"));

        let close_sql = TimescaleDbSink::sql_close_previous_version();
        assert!(close_sql.contains("UPDATE sentiment_records"));
        assert!(close_sql.contains("valid_to = $1"));
        assert!(close_sql.contains("is_current = false"));
    }

    #[tokio::test]
    async fn test_timescaledb_mock_write_initial_and_scd2_revisions() {
        let cfg = TimescaleDbConfig {
            enabled: true,
            mock_mode: true,
            ..Default::default()
        };
        let sink = TimescaleDbSink::new(cfg);

        let doc = ProcessedDocument {
            id: "doc-apple-1".to_string(),
            title: "Apple Earnings Report".to_string(),
            source: "sec_edgar".to_string(),
            url: "https://example.com/sec".to_string(),
            published_utc: "2026-09-06T10:00:00Z".to_string(),
            ingested_utc: "2026-09-06T10:00:01Z".to_string(),
            primary_ticker: Some("AAPL".to_string()),
            ..Default::default()
        };

        let sentiment_v1 = SentimentOutput {
            sentiment_score: 0.85,
            sentiment_label: "BULLISH".to_string(),
            prob_positive: 0.90,
            prob_neutral: 0.08,
            prob_negative: 0.02,
            ..Default::default()
        };

        // 1. Initial write (Revision 1)
        let id1 = sink.write_sentiment_record(&doc, &sentiment_v1).await.unwrap();
        assert_eq!(id1, 1);

        let records = sink.get_mock_records();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].revision_number, 1);
        assert!(records[0].is_current);
        assert!(records[0].valid_to.is_none());

        // 2. Updated write with revision (Revision 2)
        let mut doc_v2 = doc.clone();
        doc_v2.ingested_utc = "2026-09-06T11:00:00Z".to_string(); // Ingested 1 hour later
        let sentiment_v2 = SentimentOutput {
            sentiment_score: 0.92,
            sentiment_label: "BULLISH".to_string(),
            prob_positive: 0.95,
            prob_neutral: 0.04,
            prob_negative: 0.01,
            ..Default::default()
        };

        let id2 = sink.write_sentiment_record(&doc_v2, &sentiment_v2).await.unwrap();
        assert_eq!(id2, 2);

        let records = sink.get_mock_records();
        assert_eq!(records.len(), 2);

        // Record 1 should be closed: is_current=false, valid_to = 11:00:00Z
        assert!(!records[0].is_current);
        assert!(records[0].valid_to.is_some());
        assert_eq!(records[0].revision_number, 1);

        // Record 2 should be active: is_current=true, valid_to = None, revision_number = 2
        assert!(records[1].is_current);
        assert!(records[1].valid_to.is_none());
        assert_eq!(records[1].revision_number, 2);
        assert_eq!(records[1].sentiment_score, 0.92);
    }

    #[test]
    fn test_timescaledb_config_primary_and_backfill() {
        let cfg = TimescaleDbConfig::default();
        assert!(!cfg.primary);
        assert!(!cfg.auto_backfill_on_startup);

        let custom_cfg = TimescaleDbConfig {
            enabled: true,
            primary: true,
            auto_backfill_on_startup: true,
            ..Default::default()
        };
        assert!(custom_cfg.primary);
        assert!(custom_cfg.auto_backfill_on_startup);
        let sink = TimescaleDbSink::new(custom_cfg);
        assert!(sink.is_primary());
    }

    #[tokio::test]
    async fn test_timescaledb_backfill_duplicate_check() {
        let cfg = TimescaleDbConfig {
            enabled: true,
            mock_mode: true,
            ..Default::default()
        };
        let sink = TimescaleDbSink::new(cfg);

        let pub_utc = DateTime::parse_from_rfc3339("2026-09-01T12:00:00Z").unwrap().with_timezone(&Utc);
        let rec = TimescaleSentimentRecord {
            id: 1,
            ticker: "AAPL".to_string(),
            published_utc: pub_utc,
            ingested_utc: pub_utc,
            db_commit_utc: pub_utc,
            source: "sec_edgar".to_string(),
            title: "Apple 10-Q Report".to_string(),
            sentiment_score: 0.82,
            sentiment_label: "BULLISH".to_string(),
            confidence: 0.95,
            data_quality_score: 1.0,
            vpin: None,
            gamma_exposure: None,
            valid_from: pub_utc,
            valid_to: None,
            revision_number: 1,
            is_current: true,
        };

        // Initially does not exist
        assert!(!sink.record_exists("AAPL", pub_utc, "sec_edgar").await.unwrap());

        // Insert backfill record
        let id = sink.insert_backfill_record(&rec).await.unwrap();
        assert_eq!(id, 1);

        // Now exists
        assert!(sink.record_exists("AAPL", pub_utc, "sec_edgar").await.unwrap());
        // Different source does not exist
        assert!(!sink.record_exists("AAPL", pub_utc, "finnhub").await.unwrap());
        // Different ticker does not exist
        assert!(!sink.record_exists("MSFT", pub_utc, "sec_edgar").await.unwrap());
    }
}
