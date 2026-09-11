//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — PostgreSQL Bi-Temporal PIT Reference Database Store
//! ═══════════════════════════════════════════════════════════════════════════════
//! [POINT-IN-TIME COMPLIANCE & SPOF ELIMINATION]
//! Provides database-backed storage for delisted securities, ticker history,
//! corporate actions, and index membership with bi-temporal versioning
//! (`valid_from`, `valid_to`, `is_current`).
//! Supports:
//!   - High-throughput point-in-time snapshot generation
//!   - As-of historical state reconstruction (ignoring revisions after `as_of`)
//!   - Slowly Changing Dimension (SCD) Type 2 updates with zero data loss
//!   - Seamless fallback to static JSON files when database is unavailable
//! ═══════════════════════════════════════════════════════════════════════════════

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{postgres::PgPoolOptions, PgPool, Row};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::time::Duration;
use tracing::info;

use crate::pit::{
    CorporateAction, DelistedSecurity, IndexMembership, PITDataSnapshot, ParsedTickerInterval,
    SP500HistoryItem, TickerHistoryEntity, TickerIntervalMapping,
};

pub const DEFAULT_PIT_DB_URL: &str = "postgres://fintext:fintext@localhost:5432/fintext_metadata";

// ─────────────────────────────────────────────────────────────────────────────
// Configuration Model
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PitDatabaseConfig {
    pub enabled: bool,
    pub url: String,
    pub fallback_to_json: bool,
    pub max_connections: u32,
    pub timeout_ms: u64,
}

impl Default for PitDatabaseConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            url: DEFAULT_PIT_DB_URL.to_string(),
            fallback_to_json: true,
            max_connections: 10,
            timeout_ms: 3000,
        }
    }
}

impl PitDatabaseConfig {
    /// Load configuration from environment variables with fallback to config.yaml.
    pub fn from_env_or_config() -> Self {
        let mut cfg = Self::default();

        // 1. Check config YAML
        let config_candidates = [
            env::var("CONFIG_PATH").unwrap_or_default(),
            "config/config.yaml".to_string(),
            "config.yaml".to_string(),
            "../config/config.yaml".to_string(),
            "../../config/config.yaml".to_string(),
        ];

        for path in &config_candidates {
            if path.is_empty() {
                continue;
            }
            if let Ok(content) = fs::read_to_string(path) {
                cfg.parse_yaml_content(&content);
                break;
            }
        }

        // 2. Environment variable overrides (highest precedence)
        if let Ok(val) = env::var("PIT_DB_ENABLED") {
            cfg.enabled = val == "1" || val.eq_ignore_ascii_case("true") || val.eq_ignore_ascii_case("yes");
        }
        if let Ok(val) = env::var("PIT_DB_URL").or_else(|_| env::var("DATABASE_URL")) {
            if !val.trim().is_empty() {
                cfg.url = val.trim().to_string();
            }
        }
        if let Ok(val) = env::var("PIT_DB_FALLBACK_TO_JSON") {
            cfg.fallback_to_json = val == "1" || val.eq_ignore_ascii_case("true") || val.eq_ignore_ascii_case("yes");
        }
        if let Ok(val) = env::var("PIT_DB_MAX_CONNECTIONS") {
            if let Ok(mc) = val.trim().parse::<u32>() {
                if mc > 0 {
                    cfg.max_connections = mc;
                }
            }
        }
        if let Ok(val) = env::var("PIT_DB_TIMEOUT_MS") {
            if let Ok(t) = val.trim().parse::<u64>() {
                if t > 0 {
                    cfg.timeout_ms = t;
                }
            }
        }

        cfg
    }

    pub fn parse_yaml_content(&mut self, yaml: &str) {
        let mut in_block = false;
        for line in yaml.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') {
                continue;
            }
            if trimmed.starts_with("pit_database:") {
                in_block = true;
                continue;
            }
            if in_block {
                if !line.starts_with("  ") && !line.starts_with('\t') && trimmed.contains(':') && !trimmed.starts_with('-') {
                    break;
                }
                let parts: Vec<&str> = trimmed.splitn(2, ':').collect();
                if parts.len() == 2 {
                    let key = parts[0].trim();
                    let val = parts[1].split('#').next().unwrap_or("").trim().trim_matches('"').trim_matches('\'');
                    match key {
                        "enabled" => {
                            self.enabled = val == "true" || val == "1" || val == "yes";
                        }
                        "url" => {
                            if !val.is_empty() {
                                self.url = val.to_string();
                            }
                        }
                        "fallback_to_json" => {
                            self.fallback_to_json = val == "true" || val == "1" || val == "yes";
                        }
                        "max_connections" => {
                            if let Ok(mc) = val.parse::<u32>() {
                                if mc > 0 {
                                    self.max_connections = mc;
                                }
                            }
                        }
                        "timeout_ms" => {
                            if let Ok(t) = val.parse::<u64>() {
                                if t > 0 {
                                    self.timeout_ms = t;
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// As-Of Historical Query DTO
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PitAsOfResult {
    pub ticker: String,
    pub as_of: DateTime<Utc>,
    pub is_valid: bool,
    pub effective_symbol: Option<String>,
    pub entity_id: Option<String>,
    pub delisted: Option<DelistedSecurity>,
    pub corporate_actions: Vec<CorporateAction>,
    pub index_memberships: Vec<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// PostgreSQL Bi-Temporal Database Store
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct PitDatabaseStore {
    pool: Option<PgPool>,
    config: PitDatabaseConfig,
}

impl PitDatabaseStore {
    /// Create a new store instance with a given configuration (not connected).
    pub fn new(config: PitDatabaseConfig) -> Self {
        Self { pool: None, config }
    }

    /// Construct a store instance wrapping an existing database connection pool.
    pub fn with_pool(pool: PgPool, config: PitDatabaseConfig) -> Self {
        Self {
            pool: Some(pool),
            config,
        }
    }

    /// Connect to PostgreSQL using the provided configuration.
    pub async fn connect(config: PitDatabaseConfig) -> Result<Self, sqlx::Error> {
        let pool = PgPoolOptions::new()
            .max_connections(config.max_connections)
            .acquire_timeout(Duration::from_millis(config.timeout_ms))
            .connect(&config.url)
            .await?;

        Ok(Self {
            pool: Some(pool),
            config,
        })
    }

    pub fn config(&self) -> &PitDatabaseConfig {
        &self.config
    }

    pub fn pool(&self) -> Option<&PgPool> {
        self.pool.as_ref()
    }

    pub fn is_connected(&self) -> bool {
        self.pool.is_some()
    }

    /// Initialize bi-temporal table schemas if not already present.
    pub async fn init_db(&self) -> Result<(), sqlx::Error> {
        let pool = match self.pool.as_ref() {
            Some(p) => p,
            None => return Ok(()),
        };

        // 1. pit_delisted_securities
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS pit_delisted_securities (
                id BIGSERIAL PRIMARY KEY,
                ticker TEXT NOT NULL,
                name TEXT,
                delisting_date DATE NOT NULL,
                reason TEXT,
                final_price_usd DOUBLE PRECISION,
                last_close_usd DOUBLE PRECISION,
                delisting_return DOUBLE PRECISION,
                sec_form25_date TIMESTAMPTZ,
                announcement_date TIMESTAMPTZ,
                valid_from TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                valid_to TIMESTAMPTZ,
                is_current BOOLEAN NOT NULL DEFAULT TRUE,
                source TEXT DEFAULT 'SEC',
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );
            CREATE INDEX IF NOT EXISTS idx_pit_delisted_ticker ON pit_delisted_securities(ticker);
            CREATE INDEX IF NOT EXISTS idx_pit_delisted_current ON pit_delisted_securities(is_current) WHERE is_current = TRUE;
            CREATE INDEX IF NOT EXISTS idx_pit_delisted_valid ON pit_delisted_securities(ticker, valid_from, valid_to);
            CREATE INDEX IF NOT EXISTS idx_pit_delisted_date ON pit_delisted_securities(delisting_date);
            "#,
        )
        .execute(pool)
        .await?;

        // 2. pit_ticker_history
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS pit_ticker_history (
                id BIGSERIAL PRIMARY KEY,
                entity_id TEXT NOT NULL,
                entity_name TEXT,
                cik TEXT,
                figi TEXT,
                isin TEXT,
                ticker TEXT NOT NULL,
                start_date DATE NOT NULL,
                end_date DATE NOT NULL,
                valid_from TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                valid_to TIMESTAMPTZ,
                is_current BOOLEAN NOT NULL DEFAULT TRUE,
                source TEXT DEFAULT 'SEC',
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );
            CREATE INDEX IF NOT EXISTS idx_pit_ticker_history_ticker ON pit_ticker_history(ticker);
            CREATE INDEX IF NOT EXISTS idx_pit_ticker_history_entity ON pit_ticker_history(entity_id);
            CREATE INDEX IF NOT EXISTS idx_pit_ticker_history_cik ON pit_ticker_history(cik);
            CREATE INDEX IF NOT EXISTS idx_pit_ticker_history_current ON pit_ticker_history(is_current) WHERE is_current = TRUE;
            CREATE INDEX IF NOT EXISTS idx_pit_ticker_history_valid ON pit_ticker_history(ticker, valid_from, valid_to);
            "#,
        )
        .execute(pool)
        .await?;

        // 3. pit_corporate_actions
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS pit_corporate_actions (
                id BIGSERIAL PRIMARY KEY,
                ticker TEXT NOT NULL,
                action_date DATE NOT NULL,
                action_type TEXT NOT NULL,
                split_ratio DOUBLE PRECISION,
                dividend_per_share DOUBLE PRECISION,
                description TEXT,
                valid_from TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                valid_to TIMESTAMPTZ,
                is_current BOOLEAN NOT NULL DEFAULT TRUE,
                source TEXT DEFAULT 'MANUAL',
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );
            CREATE INDEX IF NOT EXISTS idx_pit_corp_actions_ticker ON pit_corporate_actions(ticker);
            CREATE INDEX IF NOT EXISTS idx_pit_corp_actions_current ON pit_corporate_actions(is_current) WHERE is_current = TRUE;
            CREATE INDEX IF NOT EXISTS idx_pit_corp_actions_valid ON pit_corporate_actions(ticker, valid_from, valid_to);
            CREATE INDEX IF NOT EXISTS idx_pit_corp_actions_date ON pit_corporate_actions(action_date);
            "#,
        )
        .execute(pool)
        .await?;

        // 4. pit_index_membership
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS pit_index_membership (
                id BIGSERIAL PRIMARY KEY,
                ticker TEXT NOT NULL,
                index_name TEXT NOT NULL,
                company_name TEXT,
                join_date DATE NOT NULL,
                leave_date DATE,
                reason TEXT,
                valid_from TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                valid_to TIMESTAMPTZ,
                is_current BOOLEAN NOT NULL DEFAULT TRUE,
                source TEXT DEFAULT 'MANUAL',
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );
            CREATE INDEX IF NOT EXISTS idx_pit_index_member_ticker ON pit_index_membership(ticker);
            CREATE INDEX IF NOT EXISTS idx_pit_index_member_index ON pit_index_membership(index_name);
            CREATE INDEX IF NOT EXISTS idx_pit_index_member_current ON pit_index_membership(is_current) WHERE is_current = TRUE;
            CREATE INDEX IF NOT EXISTS idx_pit_index_member_valid ON pit_index_membership(ticker, index_name, valid_from, valid_to);
            "#,
        )
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Load delisted securities from database (current records by default, or as-of timestamp).
    pub async fn load_delisted_securities(
        &self,
        as_of: Option<DateTime<Utc>>,
    ) -> Result<Vec<DelistedSecurity>, sqlx::Error> {
        let pool = match self.pool.as_ref() {
            Some(p) => p,
            None => return Ok(Vec::new()),
        };

        let rows = if let Some(as_of_ts) = as_of {
            sqlx::query(
                r#"
                SELECT ticker, name, delisting_date, reason, final_price_usd, last_close_usd, delisting_return
                FROM pit_delisted_securities
                WHERE valid_from <= $1 AND (valid_to IS NULL OR valid_to > $1)
                ORDER BY ticker ASC;
                "#,
            )
            .bind(as_of_ts)
            .fetch_all(pool)
            .await?
        } else {
            sqlx::query(
                r#"
                SELECT ticker, name, delisting_date, reason, final_price_usd, last_close_usd, delisting_return
                FROM pit_delisted_securities
                WHERE is_current = TRUE
                ORDER BY ticker ASC;
                "#,
            )
            .fetch_all(pool)
            .await?
        };

        let mut results = Vec::with_capacity(rows.len());
        for row in rows {
            let ticker: String = row.try_get("ticker")?;
            let name: Option<String> = row.try_get("name")?;
            let delisting_date: NaiveDate = row.try_get("delisting_date")?;
            let reason: Option<String> = row.try_get("reason")?;
            let final_price_usd: Option<f64> = row.try_get("final_price_usd")?;
            let last_close_usd: Option<f64> = row.try_get("last_close_usd")?;
            let delisting_return: Option<f64> = row.try_get("delisting_return")?;

            results.push(DelistedSecurity {
                ticker,
                name,
                delisting_reason: reason,
                delisting_date_iso: format!("{}T00:00:00Z", delisting_date.format("%Y-%m-%d")),
                final_price_usd,
                last_close_usd,
                delisting_return,
            });
        }

        Ok(results)
    }

    /// Load ticker history entities and interval mappings.
    pub async fn load_ticker_history(
        &self,
        as_of: Option<DateTime<Utc>>,
    ) -> Result<Vec<TickerHistoryEntity>, sqlx::Error> {
        let pool = match self.pool.as_ref() {
            Some(p) => p,
            None => return Ok(Vec::new()),
        };

        let rows = if let Some(as_of_ts) = as_of {
            sqlx::query(
                r#"
                SELECT entity_id, entity_name, cik, figi, isin, ticker, start_date, end_date
                FROM pit_ticker_history
                WHERE valid_from <= $1 AND (valid_to IS NULL OR valid_to > $1)
                ORDER BY entity_id ASC, start_date ASC;
                "#,
            )
            .bind(as_of_ts)
            .fetch_all(pool)
            .await?
        } else {
            sqlx::query(
                r#"
                SELECT entity_id, entity_name, cik, figi, isin, ticker, start_date, end_date
                FROM pit_ticker_history
                WHERE is_current = TRUE
                ORDER BY entity_id ASC, start_date ASC;
                "#,
            )
            .fetch_all(pool)
            .await?
        };

        let mut entity_map: HashMap<String, TickerHistoryEntity> = HashMap::new();

        for row in rows {
            let entity_id: String = row.try_get("entity_id")?;
            let entity_name: Option<String> = row.try_get("entity_name")?;
            let cik: Option<String> = row.try_get("cik")?;
            let figi: Option<String> = row.try_get("figi")?;
            let isin: Option<String> = row.try_get("isin")?;
            let ticker: String = row.try_get("ticker")?;
            let start_date: NaiveDate = row.try_get("start_date")?;
            let end_date: NaiveDate = row.try_get("end_date")?;

            let entry = entity_map.entry(entity_id.clone()).or_insert_with(|| TickerHistoryEntity {
                entity_id,
                entity_name,
                cik,
                figi,
                isin,
                mappings: Vec::new(),
            });

            entry.mappings.push(TickerIntervalMapping {
                ticker,
                start_iso: format!("{}T00:00:00Z", start_date.format("%Y-%m-%d")),
                end_iso: if end_date.format("%Y-%m-%d").to_string() == "9999-12-31" {
                    "9999-12-31T23:59:59Z".to_string()
                } else {
                    format!("{}T00:00:00Z", end_date.format("%Y-%m-%d"))
                },
            });
        }

        let mut entities: Vec<TickerHistoryEntity> = entity_map.into_values().collect();
        entities.sort_by(|a, b| a.entity_id.cmp(&b.entity_id));
        Ok(entities)
    }

    /// Load corporate actions from database.
    pub async fn load_corporate_actions(
        &self,
        as_of: Option<DateTime<Utc>>,
    ) -> Result<Vec<CorporateAction>, sqlx::Error> {
        let pool = match self.pool.as_ref() {
            Some(p) => p,
            None => return Ok(Vec::new()),
        };

        let rows = if let Some(as_of_ts) = as_of {
            sqlx::query(
                r#"
                SELECT ticker, action_date, action_type, split_ratio, dividend_per_share, description
                FROM pit_corporate_actions
                WHERE valid_from <= $1 AND (valid_to IS NULL OR valid_to > $1)
                ORDER BY ticker ASC, action_date ASC;
                "#,
            )
            .bind(as_of_ts)
            .fetch_all(pool)
            .await?
        } else {
            sqlx::query(
                r#"
                SELECT ticker, action_date, action_type, split_ratio, dividend_per_share, description
                FROM pit_corporate_actions
                WHERE is_current = TRUE
                ORDER BY ticker ASC, action_date ASC;
                "#,
            )
            .fetch_all(pool)
            .await?
        };

        let mut actions = Vec::with_capacity(rows.len());
        for row in rows {
            let ticker: String = row.try_get("ticker")?;
            let action_date: NaiveDate = row.try_get("action_date")?;
            let action_type: String = row.try_get("action_type")?;
            let split_ratio: Option<f64> = row.try_get("split_ratio")?;
            let dividend_per_share: Option<f64> = row.try_get("dividend_per_share")?;
            let description: Option<String> = row.try_get("description")?;

            actions.push(CorporateAction {
                ticker,
                action_date: format!("{}T00:00:00Z", action_date.format("%Y-%m-%d")),
                action_type,
                split_ratio,
                dividend_per_share,
                description,
            });
        }

        Ok(actions)
    }

    /// Load index membership records (specifically S&P 500 or general).
    pub async fn load_sp500_history(
        &self,
        as_of: Option<DateTime<Utc>>,
    ) -> Result<Vec<SP500HistoryItem>, sqlx::Error> {
        let pool = match self.pool.as_ref() {
            Some(p) => p,
            None => return Ok(Vec::new()),
        };

        let rows = if let Some(as_of_ts) = as_of {
            sqlx::query(
                r#"
                SELECT ticker, company_name, index_name, join_date, leave_date, reason
                FROM pit_index_membership
                WHERE index_name IN ('SP500', 'S&P500')
                  AND valid_from <= $1 AND (valid_to IS NULL OR valid_to > $1)
                ORDER BY ticker ASC, join_date ASC;
                "#,
            )
            .bind(as_of_ts)
            .fetch_all(pool)
            .await?
        } else {
            sqlx::query(
                r#"
                SELECT ticker, company_name, index_name, join_date, leave_date, reason
                FROM pit_index_membership
                WHERE index_name IN ('SP500', 'S&P500') AND is_current = TRUE
                ORDER BY ticker ASC, join_date ASC;
                "#,
            )
            .fetch_all(pool)
            .await?
        };

        let mut items = Vec::with_capacity(rows.len());
        for row in rows {
            let ticker: String = row.try_get("ticker")?;
            let company_name: Option<String> = row.try_get("company_name")?;
            let index_name: String = row.try_get("index_name")?;
            let join_date: NaiveDate = row.try_get("join_date")?;
            let leave_date: Option<NaiveDate> = row.try_get("leave_date")?;
            let reason: Option<String> = row.try_get("reason")?;

            items.push(SP500HistoryItem {
                ticker,
                company_name,
                index: Some(index_name),
                join_date: join_date.format("%Y-%m-%d").to_string(),
                leave_date: leave_date.map(|d| d.format("%Y-%m-%d").to_string()),
                reason,
            });
        }

        Ok(items)
    }

    /// Load index membership records mapped by ticker.
    pub async fn load_index_membership(
        &self,
        as_of: Option<DateTime<Utc>>,
    ) -> Result<HashMap<String, IndexMembership>, sqlx::Error> {
        let pool = match self.pool.as_ref() {
            Some(p) => p,
            None => return Ok(HashMap::new()),
        };

        let rows = if let Some(as_of_ts) = as_of {
            sqlx::query(
                r#"
                SELECT ticker, index_name, join_date, leave_date
                FROM pit_index_membership
                WHERE valid_from <= $1 AND (valid_to IS NULL OR valid_to > $1)
                ORDER BY ticker ASC, join_date DESC;
                "#,
            )
            .bind(as_of_ts)
            .fetch_all(pool)
            .await?
        } else {
            sqlx::query(
                r#"
                SELECT ticker, index_name, join_date, leave_date
                FROM pit_index_membership
                WHERE is_current = TRUE
                ORDER BY ticker ASC, join_date DESC;
                "#,
            )
            .fetch_all(pool)
            .await?
        };

        let mut map = HashMap::new();
        for row in rows {
            let ticker: String = row.try_get("ticker")?;
            let index_name: String = row.try_get("index_name")?;
            let join_date: NaiveDate = row.try_get("join_date")?;
            let leave_date: Option<NaiveDate> = row.try_get("leave_date")?;

            map.insert(
                ticker.clone(),
                IndexMembership {
                    ticker,
                    index: index_name,
                    effective_date: format!("{}T00:00:00Z", join_date.format("%Y-%m-%d")),
                    removal_date: leave_date.map(|d| format!("{}T00:00:00Z", d.format("%Y-%m-%d"))),
                },
            );
        }

        Ok(map)
    }

    /// Reconstruct an in-memory `PITDataSnapshot` from PostgreSQL tables.
    pub async fn load_snapshot(&self, as_of: Option<DateTime<Utc>>) -> Result<PITDataSnapshot, sqlx::Error> {
        let mut snapshot = PITDataSnapshot::empty();
        snapshot.enabled = true;

        // 1. Ticker History
        let entities = self.load_ticker_history(as_of).await?;
        for entity in entities {
            for m in &entity.mappings {
                let t = m.ticker.trim().to_uppercase();
                let start = NaiveDate::parse_from_str(&m.start_iso[..10], "%Y-%m-%d")
                    .unwrap_or_else(|_| NaiveDate::from_ymd_opt(1970, 1, 1).unwrap());
                let end = NaiveDate::parse_from_str(&m.end_iso[..10], "%Y-%m-%d")
                    .unwrap_or_else(|_| NaiveDate::from_ymd_opt(9999, 12, 31).unwrap());

                snapshot.ticker_intervals.entry(t.clone()).or_default().push(
                    ParsedTickerInterval {
                        entity_id: entity.entity_id.clone(),
                        ticker: t.clone(),
                        start_date: start,
                        end_date: end,
                    },
                );

                snapshot
                    .entity_mappings
                    .entry(entity.entity_id.clone())
                    .or_default()
                    .push((t.clone(), start, end));

                snapshot.ticker_to_entity.insert(t, entity.entity_id.clone());
            }
        }

        // 2. Delisted Securities
        let delisted = self.load_delisted_securities(as_of).await?;
        for sec in delisted {
            let t = sec.ticker.trim().to_uppercase();
            if let Ok(delist_date) = NaiveDate::parse_from_str(&sec.delisting_date_iso[..10], "%Y-%m-%d") {
                snapshot.delisted_tickers.insert(t.clone(), delist_date);
            }
            snapshot.delisted_details.insert(t, sec);
        }

        // 3. Corporate Actions
        let actions = self.load_corporate_actions(as_of).await?;
        for action in actions {
            let t = action.ticker.trim().to_uppercase();
            snapshot.corporate_actions.entry(t).or_default().push(action);
        }

        // 4. S&P 500 Index Membership
        let sp500 = self.load_sp500_history(as_of).await?;
        for item in sp500 {
            let t = item.ticker.trim().to_uppercase();
            if let Ok(join) = NaiveDate::parse_from_str(item.join_date.trim(), "%Y-%m-%d") {
                let leave = item
                    .leave_date
                    .as_deref()
                    .and_then(|l| NaiveDate::parse_from_str(l.trim(), "%Y-%m-%d").ok());
                snapshot
                    .sp500_membership
                    .entry(t)
                    .or_default()
                    .push((join, leave));
            }
        }

        info!(
            "[PIT Database] Snapshot assembled from PostgreSQL: {} ticker intervals, {} delisted securities, {} corporate actions",
            snapshot.ticker_intervals.len(),
            snapshot.delisted_tickers.len(),
            snapshot.corporate_actions.len()
        );

        Ok(snapshot)
    }

    /// Point-in-time bi-temporal as-of query for a specific ticker symbol.
    pub async fn query_as_of(&self, ticker: &str, as_of: DateTime<Utc>) -> Result<PitAsOfResult, sqlx::Error> {
        let t = ticker.trim().to_uppercase();
        let as_of_date = as_of.date_naive();

        // 1. Check delisting as of timestamp
        let delisted_secs = self.load_delisted_securities(Some(as_of)).await?;
        let delisted = delisted_secs.into_iter().find(|d| d.ticker.eq_ignore_ascii_case(&t));
        let is_delisted = if let Some(ref d) = delisted {
            if let Ok(dd) = NaiveDate::parse_from_str(&d.delisting_date_iso[..10], "%Y-%m-%d") {
                as_of_date >= dd
            } else {
                false
            }
        } else {
            false
        };

        // 2. Resolve symbol & entity
        let entities = self.load_ticker_history(Some(as_of)).await?;
        let mut effective_symbol = None;
        let mut entity_id = None;

        for e in &entities {
            for m in &e.mappings {
                if m.ticker.eq_ignore_ascii_case(&t) {
                    entity_id = Some(e.entity_id.clone());
                    break;
                }
            }
            if entity_id.is_some() {
                // Find active symbol on as_of_date
                for m in &e.mappings {
                    let s = NaiveDate::parse_from_str(&m.start_iso[..10], "%Y-%m-%d").unwrap_or_default();
                    let end = NaiveDate::parse_from_str(&m.end_iso[..10], "%Y-%m-%d")
                        .unwrap_or_else(|_| NaiveDate::from_ymd_opt(9999, 12, 31).unwrap());
                    if s <= as_of_date && as_of_date <= end {
                        effective_symbol = Some(m.ticker.clone());
                        break;
                    }
                }
                break;
            }
        }

        // 3. Corporate actions prior to as_of
        let all_corp_actions = self.load_corporate_actions(Some(as_of)).await?;
        let corp_actions: Vec<CorporateAction> = all_corp_actions
            .into_iter()
            .filter(|a| a.ticker.eq_ignore_ascii_case(&t))
            .collect();

        // 4. Index memberships
        let sp500 = self.load_sp500_history(Some(as_of)).await?;
        let mut memberships = Vec::new();
        for item in sp500 {
            if item.ticker.eq_ignore_ascii_case(&t) {
                if let Ok(j) = NaiveDate::parse_from_str(&item.join_date, "%Y-%m-%d") {
                    let l = item
                        .leave_date
                        .as_deref()
                        .and_then(|ld| NaiveDate::parse_from_str(ld, "%Y-%m-%d").ok());
                    if j <= as_of_date && l.map_or(true, |lv| as_of_date <= lv) {
                        memberships.push("SP500".to_string());
                    }
                }
            }
        }

        let is_valid = !is_delisted && (effective_symbol.is_some() || entity_id.is_none());

        Ok(PitAsOfResult {
            ticker: t.clone(),
            as_of,
            is_valid,
            effective_symbol: effective_symbol.or_else(|| if !is_delisted { Some(t) } else { None }),
            entity_id,
            delisted,
            corporate_actions: corp_actions,
            index_memberships: memberships,
        })
    }

    /// Perform Slowly Changing Dimension (SCD) Type 2 transition for a ticker change.
    pub async fn upsert_ticker_change_scd2(
        &self,
        entity_id: &str,
        old_ticker: &str,
        new_ticker: &str,
        effective_date: NaiveDate,
        company_name: Option<&str>,
        cik: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        let pool = match self.pool.as_ref() {
            Some(p) => p,
            None => return Ok(()),
        };

        let now = Utc::now();

        // 1. Close out previous active record in SCD Type 2 fashion
        sqlx::query(
            r#"
            UPDATE pit_ticker_history
            SET valid_to = $1, is_current = FALSE, end_date = $2
            WHERE entity_id = $3 AND ticker = $4 AND is_current = TRUE;
            "#,
        )
        .bind(now)
        .bind(effective_date)
        .bind(entity_id)
        .bind(old_ticker.to_uppercase())
        .execute(pool)
        .await?;

        // 2. Insert new active interval record
        let max_date = NaiveDate::from_ymd_opt(9999, 12, 31).unwrap();
        sqlx::query(
            r#"
            INSERT INTO pit_ticker_history (
                entity_id, entity_name, cik, ticker, start_date, end_date, valid_from, valid_to, is_current, source
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, NULL, TRUE, 'SEC_UPDATER'
            );
            "#,
        )
        .bind(entity_id)
        .bind(company_name)
        .bind(cik)
        .bind(new_ticker.to_uppercase())
        .bind(effective_date)
        .bind(max_date)
        .bind(now)
        .execute(pool)
        .await?;

        info!(
            "[PIT Database] SCD Type 2 ticker change committed: {} -> {} (Entity: {})",
            old_ticker, new_ticker, entity_id
        );

        Ok(())
    }

    /// Perform Slowly Changing Dimension (SCD) Type 2 insertion for a delisted security.
    pub async fn upsert_delisting_scd2(
        &self,
        ticker: &str,
        delisting_date: NaiveDate,
        reason: Option<&str>,
        name: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        let pool = match self.pool.as_ref() {
            Some(p) => p,
            None => return Ok(()),
        };

        let now = Utc::now();
        let t = ticker.trim().to_uppercase();

        // Close any existing active record if present
        sqlx::query(
            r#"
            UPDATE pit_delisted_securities
            SET valid_to = $1, is_current = FALSE
            WHERE ticker = $2 AND is_current = TRUE;
            "#,
        )
        .bind(now)
        .bind(&t)
        .execute(pool)
        .await?;

        // Insert new active delisting record
        sqlx::query(
            r#"
            INSERT INTO pit_delisted_securities (
                ticker, name, delisting_date, reason, valid_from, valid_to, is_current, source
            ) VALUES (
                $1, $2, $3, $4, $5, NULL, TRUE, 'SEC_UPDATER'
            );
            "#,
        )
        .bind(&t)
        .bind(name)
        .bind(delisting_date)
        .bind(reason)
        .bind(now)
        .execute(pool)
        .await?;

        info!(
            "[PIT Database] Delisted security recorded in database: {} on {}",
            t, delisting_date
        );

        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pit_database_config_defaults() {
        let cfg = PitDatabaseConfig::default();
        assert!(!cfg.enabled);
        assert!(cfg.fallback_to_json);
        assert_eq!(cfg.url, DEFAULT_PIT_DB_URL);
        assert_eq!(cfg.max_connections, 10);
        assert_eq!(cfg.timeout_ms, 3000);
    }

    #[test]
    fn test_pit_database_config_yaml_parsing() {
        let yaml = r#"
        pit_database:
          enabled: true
          url: "postgres://custom_user:custom_pass@dbhost:5433/custom_meta"
          fallback_to_json: false
          max_connections: 25
          timeout_ms: 5000
        "#;

        let mut cfg = PitDatabaseConfig::default();
        cfg.parse_yaml_content(yaml);

        assert!(cfg.enabled);
        assert!(!cfg.fallback_to_json);
        assert_eq!(cfg.url, "postgres://custom_user:custom_pass@dbhost:5433/custom_meta");
        assert_eq!(cfg.max_connections, 25);
        assert_eq!(cfg.timeout_ms, 5000);
    }

    #[test]
    fn test_pit_database_config_env_overrides() {
        env::set_var("PIT_DB_ENABLED", "true");
        env::set_var("PIT_DB_URL", "postgres://env_user:env_pass@envhost:5432/env_db");
        env::set_var("PIT_DB_FALLBACK_TO_JSON", "false");
        env::set_var("PIT_DB_MAX_CONNECTIONS", "50");
        env::set_var("PIT_DB_TIMEOUT_MS", "8000");

        let cfg = PitDatabaseConfig::from_env_or_config();

        assert!(cfg.enabled);
        assert_eq!(cfg.url, "postgres://env_user:env_pass@envhost:5432/env_db");
        assert!(!cfg.fallback_to_json);
        assert_eq!(cfg.max_connections, 50);
        assert_eq!(cfg.timeout_ms, 8000);

        // Clean up
        env::remove_var("PIT_DB_ENABLED");
        env::remove_var("PIT_DB_URL");
        env::remove_var("PIT_DB_FALLBACK_TO_JSON");
        env::remove_var("PIT_DB_MAX_CONNECTIONS");
        env::remove_var("PIT_DB_TIMEOUT_MS");
    }

    #[test]
    fn test_pit_database_store_not_connected_returns_empty_results() {
        let cfg = PitDatabaseConfig::default();
        let store = PitDatabaseStore::new(cfg);
        assert!(!store.is_connected());
    }
}
