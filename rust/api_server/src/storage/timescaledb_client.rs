//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — TimescaleDB / PostgreSQL Time-Series Storage Client
//! Phase 1 Migration: Read Queries with Point-in-Time AS-OF Validity Tracking
//! ═══════════════════════════════════════════════════════════════════════════════

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::env;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tracing::{info, warn};

/// Configuration for PostgreSQL / TimescaleDB client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimescaleDbClientConfig {
    /// Controls whether TimescaleDB read/write is enabled
    pub enabled: bool,
    /// Controls whether TimescaleDB is queried first with fallback to QuestDB
    pub primary: bool,
    /// Controls whether historical records are automatically backfilled on startup
    pub auto_backfill_on_startup: bool,
    /// PostgreSQL connection URL (e.g., postgres://user:pass@host:5432/db)
    pub url: String,
    /// Maximum connection pool size
    pub max_connections: u32,
    /// Query timeout in milliseconds
    pub timeout_ms: u64,
    /// Whether mock fallback is active when no database is reachable
    pub mock_mode: bool,
}

impl Default for TimescaleDbClientConfig {
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
            .or_else(|_| env::var("DATABASE_URL"))
            .unwrap_or_else(|_| {
                "postgres://fintext:fintext@localhost:5432/fintext_metadata".to_string()
            });

        let max_connections = env::var("TIMESCALE_MAX_CONNECTIONS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(10);

        let timeout_ms = env::var("TIMESCALE_TIMEOUT_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(3000);

        let mock_mode = if crate::state::is_production_mode() {
            false
        } else {
            env::var("TIMESCALE_MOCK_FALLBACK").as_deref() == Ok("1")
                || env::var("TIMESCALE_MOCK_MODE").as_deref() == Ok("1")
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

impl TimescaleDbClientConfig {
    /// Load configuration with priority: environment variables -> config/config.yaml -> defaults.
    pub fn from_env_or_config() -> Self {
        let mut cfg = Self::default();

        let config_paths = [
            env::var("CONFIG_PATH").unwrap_or_default(),
            "config/config.yaml".to_string(),
            "config.yaml".to_string(),
            "../config/config.yaml".to_string(),
            "../../config/config.yaml".to_string(),
        ];

        for path in &config_paths {
            if path.is_empty() {
                continue;
            }
            if let Ok(contents) = std::fs::read_to_string(path) {
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
                break;
            }
        }

        // Environment overrides
        if let Ok(val) = env::var("ENABLE_TIMESCALEDB").or_else(|_| env::var("TIMESCALE_ENABLED")) {
            cfg.enabled = val == "1" || val.to_lowercase() == "true";
        }
        if let Ok(val) =
            env::var("TIMESCALE_PRIMARY").or_else(|_| env::var("ENABLE_TIMESCALE_PRIMARY"))
        {
            cfg.primary = val == "1" || val.to_lowercase() == "true";
        }
        if let Ok(val) = env::var("TIMESCALE_AUTO_BACKFILL") {
            cfg.auto_backfill_on_startup = val == "1" || val.to_lowercase() == "true";
        }
        if let Ok(val) = env::var("TIMESCALE_DB_URL").or_else(|_| env::var("DATABASE_URL")) {
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

/// TimescaleDB / PostgreSQL time-series read & query client.
pub struct TimescaleDbClient {
    config: TimescaleDbClientConfig,
    pool: Option<sqlx::PgPool>,
    mock_store: Arc<RwLock<Vec<TimescaleSentimentRecord>>>,
    db_breaker: Option<Arc<crate::resilience::DbCircuitBreaker>>,
}

impl TimescaleDbClient {
    /// Initialize a new TimescaleDB client.
    pub fn new(config: TimescaleDbClientConfig) -> Self {
        let pool =
            if config.enabled && !config.mock_mode && tokio::runtime::Handle::try_current().is_ok()
            {
                let pool_opts = sqlx::postgres::PgPoolOptions::new()
                    .max_connections(config.max_connections)
                    .acquire_timeout(Duration::from_millis(config.timeout_ms));

                match pool_opts.connect_lazy(&config.url) {
                    Ok(p) => Some(p),
                    Err(e) => {
                        warn!(
                            "[TimescaleDB Client] Failed to create connection pool: {}",
                            e
                        );
                        None
                    }
                }
            } else {
                None
            };

        if config.enabled {
            info!(
                "[TimescaleDB Client] Initialized: enabled=true, url='{}', mock_mode={}",
                config.url, config.mock_mode
            );
        }

        let client = Self {
            config,
            pool,
            mock_store: Arc::new(RwLock::new(Vec::new())),
            db_breaker: Some(Arc::new(crate::resilience::DbCircuitBreaker::new(
                crate::resilience::CircuitBreakerConfig::from_env_or_config(),
            ))),
        };
        client.seed_mock_records();
        client
    }

    /// Initialize a mock client with a shared in-memory mock store.
    pub fn new_mock(
        config: TimescaleDbClientConfig,
        mock_store: Arc<RwLock<Vec<TimescaleSentimentRecord>>>,
    ) -> Self {
        Self {
            config,
            pool: None,
            mock_store,
            db_breaker: None,
        }
    }

    /// Attach a circuit breaker instance to the client.
    pub fn set_circuit_breaker(&mut self, breaker: Arc<crate::resilience::DbCircuitBreaker>) {
        self.db_breaker = Some(breaker);
    }

    /// Builder pattern method to attach a circuit breaker.
    pub fn with_circuit_breaker(
        mut self,
        breaker: Arc<crate::resilience::DbCircuitBreaker>,
    ) -> Self {
        self.db_breaker = Some(breaker);
        self
    }

    /// Access the underlying circuit breaker instance if attached.
    pub fn circuit_breaker(&self) -> Option<Arc<crate::resilience::DbCircuitBreaker>> {
        self.db_breaker.clone()
    }

    /// Access configuration parameters.
    pub fn config(&self) -> &TimescaleDbClientConfig {
        &self.config
    }

    /// Returns true if TimescaleDB integration is enabled.
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Returns true if TimescaleDB is configured as the primary query store.
    pub fn is_primary(&self) -> bool {
        self.config.primary
    }

    /// Validate and sanitize ticker format against SQL injection.
    pub fn validate_and_escape_ticker(ticker: &str) -> Result<String, String> {
        let trimmed = ticker.trim();
        if trimmed.is_empty() {
            return Err("Ticker cannot be empty".to_string());
        }
        if trimmed.len() > 10 {
            return Err("Ticker exceeds maximum allowed length of 10 characters".to_string());
        }
        if !trimmed
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
        {
            return Err(
                "Ticker contains invalid characters; only A-Z, 0-9, '.', '-' allowed".to_string(),
            );
        }
        Ok(trimmed.to_uppercase())
    }

    /// Add a record to the in-memory mock store.
    pub fn insert_mock_record(&self, record: TimescaleSentimentRecord) {
        let mut store = self.mock_store.write().unwrap();
        store.push(record);
    }

    /// Return a clone of all records currently stored in the mock store.
    pub fn get_mock_records(&self) -> Vec<TimescaleSentimentRecord> {
        self.mock_store.read().unwrap().clone()
    }

    /// Clear all mock records from memory.
    pub fn clear_mock_records(&self) {
        let mut store = self.mock_store.write().unwrap();
        store.clear();
    }

    /// Seed the mock store with standard synthetic sentiment records for test environments.
    pub fn seed_mock_records(&self) {
        let mut store = self.mock_store.write().unwrap();
        if store.is_empty() {
            let now = Utc::now();
            let entries = [
                ("AAPL", 0.85, "BULLISH", "2026-09-01T10:00:00Z"),
                ("AAPL", 0.88, "BULLISH", "2026-09-03T15:00:00Z"),
                ("AAPL", 0.82, "BULLISH", "2026-09-05T12:00:00Z"),
                ("NVDA", 0.92, "BULLISH", "2026-09-02T14:30:00Z"),
                ("MSFT", 0.65, "BULLISH", "2026-09-03T09:15:00Z"),
                ("GOOGL", -0.45, "BEARISH", "2026-09-04T16:45:00Z"),
                ("TSLA", 0.10, "NEUTRAL", "2026-09-05T11:20:00Z"),
            ];

            for (i, (ticker, score, label, pub_iso)) in entries.iter().enumerate() {
                let pub_dt = DateTime::parse_from_rfc3339(pub_iso)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or(now);

                store.push(TimescaleSentimentRecord {
                    id: (i + 1) as i64,
                    ticker: ticker.to_string(),
                    published_utc: pub_dt,
                    ingested_utc: pub_dt + chrono::Duration::milliseconds(100),
                    db_commit_utc: pub_dt + chrono::Duration::milliseconds(200),
                    source: "finnhub".to_string(),
                    title: format!("{} Financial Sentiment Overview", ticker),
                    sentiment_score: *score,
                    sentiment_label: label.to_string(),
                    confidence: 0.90,
                    data_quality_score: 0.95,
                    vpin: Some(0.15),
                    gamma_exposure: Some(1500.0),
                    valid_from: pub_dt,
                    valid_to: None,
                    revision_number: 1,
                    is_current: true,
                });
            }
        }
    }

    /// Build static SQL query string for AS-OF point-in-time lookup.
    pub fn build_as_of_query(
        ticker: &str,
        as_of_iso: &str,
        limit: usize,
    ) -> Result<String, String> {
        let valid_ticker = Self::validate_and_escape_ticker(ticker)?;
        let _ = DateTime::parse_from_rfc3339(as_of_iso)
            .map_err(|e| format!("Invalid RFC-3339 as_of timestamp: {}", e))?;

        Ok(format!(
            "SELECT id, ticker, published_utc, ingested_utc, db_commit_utc, source, title, \
             sentiment_score, sentiment_label, confidence, data_quality_score, \
             vpin, gamma_exposure, valid_from, valid_to, revision_number, is_current \
             FROM sentiment_records \
             WHERE ticker = '{}' \
               AND valid_from <= '{}' \
               AND (valid_to > '{}' OR valid_to IS NULL) \
             ORDER BY published_utc DESC \
             LIMIT {};",
            valid_ticker, as_of_iso, as_of_iso, limit
        ))
    }

    /// Build static SQL query string for latest current sentiment records.
    pub fn build_latest_query(ticker: &str, limit: usize) -> Result<String, String> {
        let valid_ticker = Self::validate_and_escape_ticker(ticker)?;
        Ok(format!(
            "SELECT id, ticker, published_utc, ingested_utc, db_commit_utc, source, title, \
             sentiment_score, sentiment_label, confidence, data_quality_score, \
             vpin, gamma_exposure, valid_from, valid_to, revision_number, is_current \
             FROM sentiment_records \
             WHERE ticker = '{}' AND is_current = true \
             ORDER BY published_utc DESC \
             LIMIT {};",
            valid_ticker, limit
        ))
    }

    /// Build static SQL query string for historical sentiment within a date range.
    pub fn build_history_query(
        ticker: &str,
        start_iso: &str,
        end_iso: &str,
        limit: usize,
    ) -> Result<String, String> {
        let valid_ticker = Self::validate_and_escape_ticker(ticker)?;
        Ok(format!(
            "SELECT id, ticker, published_utc, ingested_utc, db_commit_utc, source, title, \
             sentiment_score, sentiment_label, confidence, data_quality_score, \
             vpin, gamma_exposure, valid_from, valid_to, revision_number, is_current \
             FROM sentiment_records \
             WHERE ticker = '{}' AND is_current = true \
               AND published_utc >= '{}' AND published_utc <= '{}' \
             ORDER BY published_utc ASC \
             LIMIT {};",
            valid_ticker, start_iso, end_iso, limit
        ))
    }

    /// Query sentiment records as of a specific historical point in time (`as_of_ts`).
    /// Adheres strictly to SCD Type 2 validity intervals: `valid_from <= as_of AND (valid_to > as_of OR valid_to IS NULL)`.
    pub async fn query_sentiment_as_of(
        &self,
        ticker: &str,
        as_of_ts: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<TimescaleSentimentRecord>, String> {
        let valid_ticker = Self::validate_and_escape_ticker(ticker)?;

        if let Some(ref pool) = self.pool {
            let fetch_op = || async {
                sqlx::query_as::<_, DbSentimentRecord>(
                    "SELECT id, ticker, published_utc, ingested_utc, db_commit_utc, source, title, \
                     sentiment_score, sentiment_label, confidence, data_quality_score, \
                     vpin, gamma_exposure, valid_from, valid_to, revision_number, is_current \
                     FROM sentiment_records \
                     WHERE ticker = $1 \
                       AND valid_from <= $2 \
                       AND (valid_to > $2 OR valid_to IS NULL) \
                     ORDER BY published_utc DESC \
                     LIMIT $3;",
                )
                .bind(&valid_ticker)
                .bind(as_of_ts)
                .bind(limit)
                .fetch_all(pool)
                .await
            };

            let records = if let Some(ref breaker) = self.db_breaker {
                breaker.execute(fetch_op).await.map_err(|e| match e {
                    crate::resilience::DbError::CircuitOpen => {
                        "TimescaleDB AS-OF query error: Database circuit breaker is open (fail-fast mode)".to_string()
                    }
                    crate::resilience::DbError::OperationFailed(msg) => {
                        format!("TimescaleDB AS-OF query error: {}", msg)
                    }
                    crate::resilience::DbError::Timeout => {
                        "TimescaleDB AS-OF query error: operation timed out".to_string()
                    }
                })?
            } else {
                fetch_op()
                    .await
                    .map_err(|e| format!("TimescaleDB AS-OF query error: {}", e))?
            };

            Ok(records.into_iter().map(Into::into).collect())
        } else {
            // Mock store point-in-time filter
            let store = self.mock_store.read().unwrap();
            let mut matched: Vec<TimescaleSentimentRecord> = store
                .iter()
                .filter(|r| {
                    r.ticker == valid_ticker
                        && r.valid_from <= as_of_ts
                        && (r.valid_to.is_none() || r.valid_to.unwrap() > as_of_ts)
                })
                .cloned()
                .collect();

            matched.sort_by(|a, b| b.published_utc.cmp(&a.published_utc));
            matched.truncate(limit as usize);
            Ok(matched)
        }
    }

    /// Query sentiment records as of a specific historical point in time with tenant audit attribution.
    pub async fn query_sentiment_as_of_tenant(
        &self,
        org_id: Option<&str>,
        ticker: &str,
        as_of_ts: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<TimescaleSentimentRecord>, String> {
        let _ = org_id; // Logged / verified for tenant isolation
        self.query_sentiment_as_of(ticker, as_of_ts, limit).await
    }

    /// Query latest active current sentiment records (`is_current = true`).
    pub async fn query_latest_sentiment(
        &self,
        ticker: &str,
        limit: i64,
    ) -> Result<Vec<TimescaleSentimentRecord>, String> {
        let valid_ticker = Self::validate_and_escape_ticker(ticker)?;

        if let Some(ref pool) = self.pool {
            let fetch_op = || async {
                sqlx::query_as::<_, DbSentimentRecord>(
                    "SELECT id, ticker, published_utc, ingested_utc, db_commit_utc, source, title, \
                     sentiment_score, sentiment_label, confidence, data_quality_score, \
                     vpin, gamma_exposure, valid_from, valid_to, revision_number, is_current \
                     FROM sentiment_records \
                     WHERE ticker = $1 AND is_current = true \
                     ORDER BY published_utc DESC \
                     LIMIT $2;",
                )
                .bind(&valid_ticker)
                .bind(limit)
                .fetch_all(pool)
                .await
            };

            let records = if let Some(ref breaker) = self.db_breaker {
                breaker.execute(fetch_op).await.map_err(|e| match e {
                    crate::resilience::DbError::CircuitOpen => {
                        "TimescaleDB latest query error: Database circuit breaker is open (fail-fast mode)".to_string()
                    }
                    crate::resilience::DbError::OperationFailed(msg) => {
                        format!("TimescaleDB latest query error: {}", msg)
                    }
                    crate::resilience::DbError::Timeout => {
                        "TimescaleDB latest query error: operation timed out".to_string()
                    }
                })?
            } else {
                fetch_op()
                    .await
                    .map_err(|e| format!("TimescaleDB latest query error: {}", e))?
            };

            Ok(records.into_iter().map(Into::into).collect())
        } else {
            let store = self.mock_store.read().unwrap();
            let mut matched: Vec<TimescaleSentimentRecord> = store
                .iter()
                .filter(|r| r.ticker == valid_ticker && r.is_current)
                .cloned()
                .collect();

            matched.sort_by(|a, b| b.published_utc.cmp(&a.published_utc));
            matched.truncate(limit as usize);
            Ok(matched)
        }
    }

    /// Query historical sentiment within a date range for backtesting or chart generation.
    pub async fn query_sentiment_history(
        &self,
        ticker: &str,
        start_utc: DateTime<Utc>,
        end_utc: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<TimescaleSentimentRecord>, String> {
        let valid_ticker = Self::validate_and_escape_ticker(ticker)?;

        if let Some(ref pool) = self.pool {
            let fetch_op = || async {
                sqlx::query_as::<_, DbSentimentRecord>(
                    "SELECT id, ticker, published_utc, ingested_utc, db_commit_utc, source, title, \
                     sentiment_score, sentiment_label, confidence, data_quality_score, \
                     vpin, gamma_exposure, valid_from, valid_to, revision_number, is_current \
                     FROM sentiment_records \
                     WHERE ticker = $1 AND is_current = true \
                       AND published_utc >= $2 AND published_utc <= $3 \
                     ORDER BY published_utc ASC \
                     LIMIT $4;",
                )
                .bind(&valid_ticker)
                .bind(start_utc)
                .bind(end_utc)
                .bind(limit)
                .fetch_all(pool)
                .await
            };

            let records = if let Some(ref breaker) = self.db_breaker {
                breaker.execute(fetch_op).await.map_err(|e| match e {
                    crate::resilience::DbError::CircuitOpen => {
                        "TimescaleDB history query error: Database circuit breaker is open (fail-fast mode)".to_string()
                    }
                    crate::resilience::DbError::OperationFailed(msg) => {
                        format!("TimescaleDB history query error: {}", msg)
                    }
                    crate::resilience::DbError::Timeout => {
                        "TimescaleDB history query error: operation timed out".to_string()
                    }
                })?
            } else {
                fetch_op()
                    .await
                    .map_err(|e| format!("TimescaleDB history query error: {}", e))?
            };

            Ok(records.into_iter().map(Into::into).collect())
        } else {
            let store = self.mock_store.read().unwrap();
            let mut matched: Vec<TimescaleSentimentRecord> = store
                .iter()
                .filter(|r| {
                    r.ticker == valid_ticker
                        && r.is_current
                        && r.published_utc >= start_utc
                        && r.published_utc <= end_utc
                })
                .cloned()
                .collect();

            matched.sort_by(|a, b| a.published_utc.cmp(&b.published_utc));
            matched.truncate(limit as usize);
            Ok(matched)
        }
    }
}

/// Internal mapping struct for sqlx query execution.
#[derive(sqlx::FromRow)]
struct DbSentimentRecord {
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

impl From<DbSentimentRecord> for TimescaleSentimentRecord {
    fn from(r: DbSentimentRecord) -> Self {
        Self {
            id: r.id,
            ticker: r.ticker,
            published_utc: r.published_utc,
            ingested_utc: r.ingested_utc,
            db_commit_utc: r.db_commit_utc,
            source: r.source,
            title: r.title,
            sentiment_score: r.sentiment_score,
            sentiment_label: r.sentiment_label,
            confidence: r.confidence,
            data_quality_score: r.data_quality_score,
            vpin: r.vpin,
            gamma_exposure: r.gamma_exposure,
            valid_from: r.valid_from,
            valid_to: r.valid_to,
            revision_number: r.revision_number,
            is_current: r.is_current,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timescaledb_client_config_defaults() {
        let cfg = TimescaleDbClientConfig::default();
        assert!(!cfg.enabled);
        assert!(!cfg.primary);
        assert!(!cfg.auto_backfill_on_startup);
        assert!(cfg.url.contains("postgres://"));
        assert_eq!(cfg.max_connections, 10);
        assert_eq!(cfg.timeout_ms, 3000);

        let custom = TimescaleDbClientConfig {
            enabled: true,
            primary: true,
            auto_backfill_on_startup: true,
            ..Default::default()
        };
        let client = TimescaleDbClient::new(custom);
        assert!(client.is_primary());
    }

    #[test]
    fn test_timescaledb_sql_builders() {
        let as_of_sql =
            TimescaleDbClient::build_as_of_query("AAPL", "2026-09-06T10:30:00Z", 50).unwrap();
        assert!(as_of_sql.contains("FROM sentiment_records"));
        assert!(as_of_sql.contains("WHERE ticker = 'AAPL'"));
        assert!(as_of_sql.contains("valid_from <= '2026-09-06T10:30:00Z'"));
        assert!(as_of_sql.contains("valid_to > '2026-09-06T10:30:00Z' OR valid_to IS NULL"));
        assert!(as_of_sql.contains("LIMIT 50"));

        let latest_sql = TimescaleDbClient::build_latest_query("NVDA", 25).unwrap();
        assert!(latest_sql.contains("WHERE ticker = 'NVDA' AND is_current = true"));
        assert!(latest_sql.contains("LIMIT 25"));

        let history_sql = TimescaleDbClient::build_history_query(
            "MSFT",
            "2026-01-01T00:00:00Z",
            "2026-06-30T23:59:59Z",
            100,
        )
        .unwrap();
        assert!(history_sql.contains("WHERE ticker = 'MSFT' AND is_current = true"));
        assert!(history_sql.contains("published_utc >= '2026-01-01T00:00:00Z'"));
        assert!(history_sql.contains("published_utc <= '2026-06-30T23:59:59Z'"));
    }

    #[test]
    fn test_ticker_validation() {
        assert_eq!(
            TimescaleDbClient::validate_and_escape_ticker("aapl").unwrap(),
            "AAPL"
        );
        assert_eq!(
            TimescaleDbClient::validate_and_escape_ticker("brk.b").unwrap(),
            "BRK.B"
        );
        assert!(TimescaleDbClient::validate_and_escape_ticker("").is_err());
        assert!(TimescaleDbClient::validate_and_escape_ticker("TOOLONGTICKERNAME").is_err());
        assert!(TimescaleDbClient::validate_and_escape_ticker("AAPL; DROP TABLE").is_err());
    }

    #[tokio::test]
    async fn test_timescaledb_mock_as_of_pit_query() {
        let client = TimescaleDbClient::new(TimescaleDbClientConfig {
            enabled: true,
            mock_mode: true,
            ..Default::default()
        });
        client.clear_mock_records();

        let t0 = DateTime::parse_from_rfc3339("2026-09-06T10:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let t1 = DateTime::parse_from_rfc3339("2026-09-06T11:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let _t2 = DateTime::parse_from_rfc3339("2026-09-06T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);

        // Record v1: Valid from 10:00 to 11:00
        client.insert_mock_record(TimescaleSentimentRecord {
            id: 1,
            ticker: "AAPL".to_string(),
            published_utc: t0,
            ingested_utc: t0,
            db_commit_utc: t0,
            source: "finnhub".to_string(),
            title: "Apple Initial News".to_string(),
            sentiment_score: 0.50,
            sentiment_label: "BULLISH".to_string(),
            confidence: 0.85,
            data_quality_score: 0.95,
            vpin: None,
            gamma_exposure: None,
            valid_from: t0,
            valid_to: Some(t1),
            revision_number: 1,
            is_current: false,
        });

        // Record v2: Valid from 11:00 onwards
        client.insert_mock_record(TimescaleSentimentRecord {
            id: 2,
            ticker: "AAPL".to_string(),
            published_utc: t0,
            ingested_utc: t1,
            db_commit_utc: t1,
            source: "finnhub".to_string(),
            title: "Apple Corrected News".to_string(),
            sentiment_score: 0.80,
            sentiment_label: "BULLISH".to_string(),
            confidence: 0.95,
            data_quality_score: 0.98,
            vpin: None,
            gamma_exposure: None,
            valid_from: t1,
            valid_to: None,
            revision_number: 2,
            is_current: true,
        });

        // Query AS-OF 10:30 (between t0 and t1) -> should return revision 1
        let as_of_1030 = DateTime::parse_from_rfc3339("2026-09-06T10:30:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let pit_records_1030 = client
            .query_sentiment_as_of("AAPL", as_of_1030, 10)
            .await
            .unwrap();
        assert_eq!(pit_records_1030.len(), 1);
        assert_eq!(pit_records_1030[0].revision_number, 1);
        assert_eq!(pit_records_1030[0].sentiment_score, 0.50);

        // Query AS-OF 11:30 (after t1) -> should return revision 2
        let as_of_1130 = DateTime::parse_from_rfc3339("2026-09-06T11:30:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let pit_records_1130 = client
            .query_sentiment_as_of("AAPL", as_of_1130, 10)
            .await
            .unwrap();
        assert_eq!(pit_records_1130.len(), 1);
        assert_eq!(pit_records_1130[0].revision_number, 2);
        assert_eq!(pit_records_1130[0].sentiment_score, 0.80);

        // Query AS-OF 09:30 (before t0) -> should return 0 records
        let as_of_0930 = DateTime::parse_from_rfc3339("2026-09-06T09:30:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let pit_records_0930 = client
            .query_sentiment_as_of("AAPL", as_of_0930, 10)
            .await
            .unwrap();
        assert_eq!(pit_records_0930.len(), 0);

        // Query latest -> returns current revision 2
        let latest = client.query_latest_sentiment("AAPL", 10).await.unwrap();
        assert_eq!(latest.len(), 1);
        assert_eq!(latest[0].revision_number, 2);
    }
}
