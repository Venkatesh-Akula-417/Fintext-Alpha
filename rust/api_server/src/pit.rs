use chrono::{DateTime, NaiveDate};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use tracing::{info, warn};

/// Global default PITData singleton for fallback / static handler access.
pub static GLOBAL_PIT_DATA: Lazy<Arc<PITData>> = Lazy::new(|| Arc::new(PITData::from_env()));

// ─────────────────────────────────────────────────────────────────────────────
// Raw Config DTO Models
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TickerHistoryEntity {
    pub entity_id: String,
    pub entity_name: Option<String>,
    pub cik: Option<String>,
    pub figi: Option<String>,
    pub isin: Option<String>,
    pub mappings: Vec<TickerIntervalMapping>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TickerIntervalMapping {
    pub ticker: String,
    pub start_iso: String,
    pub end_iso: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelistedSecurity {
    pub ticker: String,
    pub name: Option<String>,
    pub delisting_reason: Option<String>,
    pub delisting_date_iso: String,
    pub final_price_usd: Option<f64>,
    pub last_close_usd: Option<f64>,
    pub delisting_return: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SP500HistoryItem {
    pub ticker: String,
    pub company_name: Option<String>,
    pub index: Option<String>,
    pub join_date: String,
    pub leave_date: Option<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorporateAction {
    pub ticker: String,
    pub action_date: String,
    #[serde(rename = "type")]
    pub action_type: String,
    pub split_ratio: Option<f64>,
    pub dividend_per_share: Option<f64>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermanentIdentifier {
    pub ticker: String,
    pub figi: Option<String>,
    pub cusip: Option<String>,
    pub isin: Option<String>,
    pub effective_date: String,
    pub end_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexMembership {
    pub ticker: String,
    pub index: String,
    pub effective_date: String,
    pub removal_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TickerUniverseItem {
    pub ticker: String,
    pub status: String,
}

// ─────────────────────────────────────────────────────────────────────────────
// Parsed In-Memory Structures for O(1) Index Lookups
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedTickerInterval {
    pub entity_id: String,
    pub ticker: String,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
}

#[derive(Debug, Clone, Default)]
pub struct PITDataSnapshot {
    pub enabled: bool,
    /// Map of uppercase ticker symbol -> list of valid active date intervals
    pub ticker_intervals: HashMap<String, Vec<ParsedTickerInterval>>,
    /// Map of entity_id -> list of (ticker, start_date, end_date)
    pub entity_mappings: HashMap<String, Vec<(String, NaiveDate, NaiveDate)>>,
    /// Map of ticker -> entity_id
    pub ticker_to_entity: HashMap<String, String>,
    /// Map of delisted ticker -> delisting date
    pub delisted_tickers: HashMap<String, NaiveDate>,
    /// Map of delisted ticker -> metadata struct
    pub delisted_details: HashMap<String, DelistedSecurity>,
    /// Map of ticker -> intervals of S&P 500 index membership (join_date, Option<leave_date>)
    pub sp500_membership: HashMap<String, Vec<(NaiveDate, Option<NaiveDate>)>>,
    /// Map of ticker -> list of corporate actions
    pub corporate_actions: HashMap<String, Vec<CorporateAction>>,
    /// Map of ticker -> permanent identifier record
    pub permanent_identifiers: HashMap<String, PermanentIdentifier>,
    /// Set of active tickers in default universe
    pub active_universe: HashSet<String>,
}

impl PITDataSnapshot {
    /// Create a new empty `PITData` container.
    pub fn empty() -> Self {
        Self {
            enabled: true,
            ticker_intervals: HashMap::new(),
            entity_mappings: HashMap::new(),
            ticker_to_entity: HashMap::new(),
            delisted_tickers: HashMap::new(),
            delisted_details: HashMap::new(),
            sp500_membership: HashMap::new(),
            corporate_actions: HashMap::new(),
            permanent_identifiers: HashMap::new(),
            active_universe: HashSet::new(),
        }
    }

    /// Load PIT configuration from environment or search default config paths.
    pub fn from_env() -> Self {
        let enabled_var = env::var("PIT_DATA_ENABLED").unwrap_or_else(|_| "1".to_string());
        let enabled = enabled_var == "1"
            || enabled_var.eq_ignore_ascii_case("true")
            || enabled_var.eq_ignore_ascii_case("yes");

        if !enabled {
            info!("[PIT Engine] Point-in-Time data enforcement is DISABLED via PIT_DATA_ENABLED=0");
            let mut p = Self::empty();
            p.enabled = false;
            return p;
        }

        // Search directory candidates
        let candidate_dirs = if let Ok(custom_dir) = env::var("PIT_DATA_DIR") {
            vec![PathBuf::from(custom_dir)]
        } else {
            vec![
                PathBuf::from("./config"),
                PathBuf::from("../config"),
                PathBuf::from("../../config"),
                PathBuf::from("d:/FinText-Alpha-Vectorizer/config"),
                PathBuf::from("D:\\FinText-Alpha-Vectorizer\\config"),
            ]
        };

        for dir in candidate_dirs {
            if dir.is_dir()
                && (dir.join("ticker_history.json").exists()
                    || dir.join("delisted_securities.json").exists())
            {
                info!(
                    "[PIT Engine] Loading Point-in-Time configuration from: {}",
                    dir.display()
                );
                return Self::load_from_dir(&dir);
            }
        }

        warn!("[PIT Engine] No valid PIT config directory found. Gracefully degrading to default empty PITData.");
        Self::empty()
    }

    /// Load and index JSON configuration files from the specified directory.
    pub fn load_from_dir(dir: &Path) -> Self {
        let mut pit = Self::empty();
        pit.enabled = true;

        // 1. ticker_history.json
        let ticker_history_path = dir.join("ticker_history.json");
        if let Ok(content) = fs::read_to_string(&ticker_history_path) {
            if let Ok(entities) = serde_json::from_str::<Vec<TickerHistoryEntity>>(&content) {
                for entity in entities {
                    for m in &entity.mappings {
                        let t = m.ticker.trim().to_uppercase();
                        let start = parse_iso_date(&m.start_iso)
                            .unwrap_or(NaiveDate::from_ymd_opt(1970, 1, 1).unwrap());
                        let end = parse_iso_date(&m.end_iso)
                            .unwrap_or(NaiveDate::from_ymd_opt(9999, 12, 31).unwrap());

                        pit.ticker_intervals.entry(t.clone()).or_default().push(
                            ParsedTickerInterval {
                                entity_id: entity.entity_id.clone(),
                                ticker: t.clone(),
                                start_date: start,
                                end_date: end,
                            },
                        );

                        pit.entity_mappings
                            .entry(entity.entity_id.clone())
                            .or_default()
                            .push((t.clone(), start, end));

                        pit.ticker_to_entity.insert(t, entity.entity_id.clone());
                    }
                }
            }
        }

        // 2. delisted_securities.json
        let delisted_path = dir.join("delisted_securities.json");
        if let Ok(content) = fs::read_to_string(&delisted_path) {
            if let Ok(securities) = serde_json::from_str::<Vec<DelistedSecurity>>(&content) {
                for sec in securities {
                    let t = sec.ticker.trim().to_uppercase();
                    if let Some(delist_date) = parse_iso_date(&sec.delisting_date_iso) {
                        pit.delisted_tickers.insert(t.clone(), delist_date);
                    }
                    pit.delisted_details.insert(t, sec);
                }
            }
        }

        // 3. sp500_history.json
        let sp500_path = dir.join("sp500_history.json");
        if let Ok(content) = fs::read_to_string(&sp500_path) {
            if let Ok(items) = serde_json::from_str::<Vec<SP500HistoryItem>>(&content) {
                for item in items {
                    let t = item.ticker.trim().to_uppercase();
                    if let Ok(join) = NaiveDate::parse_from_str(item.join_date.trim(), "%Y-%m-%d") {
                        let leave = item
                            .leave_date
                            .as_deref()
                            .and_then(|l| NaiveDate::parse_from_str(l.trim(), "%Y-%m-%d").ok());
                        pit.sp500_membership
                            .entry(t)
                            .or_default()
                            .push((join, leave));
                    }
                }
            }
        }

        // 4. corporate_actions.json
        let corp_path = dir.join("corporate_actions.json");
        if let Ok(content) = fs::read_to_string(&corp_path) {
            if let Ok(actions) = serde_json::from_str::<Vec<CorporateAction>>(&content) {
                for action in actions {
                    let t = action.ticker.trim().to_uppercase();
                    pit.corporate_actions.entry(t).or_default().push(action);
                }
            }
        }

        // 5. permanent_identifiers.json
        let perm_path = dir.join("permanent_identifiers.json");
        if let Ok(content) = fs::read_to_string(&perm_path) {
            if let Ok(perms) = serde_json::from_str::<Vec<PermanentIdentifier>>(&content) {
                for perm in perms {
                    let t = perm.ticker.trim().to_uppercase();
                    pit.permanent_identifiers.insert(t, perm);
                }
            }
        }

        // 6. ticker_universe.json
        let universe_path = dir.join("ticker_universe.json");
        if let Ok(content) = fs::read_to_string(&universe_path) {
            if let Ok(univ) = serde_json::from_str::<Vec<TickerUniverseItem>>(&content) {
                for item in univ {
                    if item.status.eq_ignore_ascii_case("active") {
                        pit.active_universe
                            .insert(item.ticker.trim().to_uppercase());
                    }
                }
            }
        }

        info!(
            "[PIT Engine] Initialized: {} ticker intervals, {} delisted securities, {} S&P 500 members",
            pit.ticker_intervals.len(),
            pit.delisted_tickers.len(),
            pit.sp500_membership.len()
        );

        pit
    }

    /// Check if Point-in-Time data enforcement is active.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Validate if a given ticker symbol was listed, active, and not delisted on a specific date.
    pub fn is_valid_ticker(&self, ticker: &str, date: NaiveDate) -> bool {
        if !self.enabled {
            return true;
        }

        let t = ticker.trim().to_uppercase();

        // 1. Check delisting
        if let Some(&delist_date) = self.delisted_tickers.get(&t) {
            if date >= delist_date {
                return false;
            }
        }

        // 2. Check ticker history interval if explicitly tracked
        if let Some(intervals) = self.ticker_intervals.get(&t) {
            let matches_interval = intervals
                .iter()
                .any(|iv| iv.start_date <= date && date <= iv.end_date);
            if !matches_interval {
                return false;
            }
        }

        true
    }

    /// Resolve a ticker symbol to its effective historical trading symbol on a specific date.
    /// Returns `Some(symbol)` if the company was listed and trading on `date`, or `None` if not.
    pub fn resolve_symbol(&self, ticker: &str, date: NaiveDate) -> Option<String> {
        if !self.enabled {
            return Some(ticker.trim().to_uppercase());
        }

        let t = ticker.trim().to_uppercase();

        // If the queried symbol was valid on date, return it
        if self.is_valid_ticker(&t, date) {
            return Some(t);
        }

        // If queried symbol wasn't valid, check if its underlying entity traded under another symbol on `date`
        if let Some(entity_id) = self.ticker_to_entity.get(&t) {
            if let Some(mappings) = self.entity_mappings.get(entity_id) {
                for (mapped_ticker, start, end) in mappings {
                    if *start <= date && date <= *end {
                        return Some(mapped_ticker.clone());
                    }
                }
            }
            // Entity exists but was not listed on `date`
            return None;
        }

        // Ticker is delisted before `date`
        if let Some(&delist_date) = self.delisted_tickers.get(&t) {
            if date >= delist_date {
                return None;
            }
        }

        // Untracked generic ticker: assume valid
        Some(t)
    }

    /// Retrieve the official delisting date for a ticker if known.
    pub fn get_delisting_date(&self, ticker: &str) -> Option<NaiveDate> {
        let t = ticker.trim().to_uppercase();
        self.delisted_tickers.get(&t).copied()
    }

    /// Clamp a requested `[start_date, end_date]` query window to prevent querying post-delisting
    /// or post-symbol change data. Returns `Some((effective_start, effective_end))` or `None` if invalid throughout.
    pub fn clamp_query_range(
        &self,
        ticker: &str,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Option<(NaiveDate, NaiveDate)> {
        if !self.enabled {
            return Some((start_date, end_date));
        }

        let t = ticker.trim().to_uppercase();

        // Must be valid as of start_date
        if !self.is_valid_ticker(&t, start_date) {
            return None;
        }

        let mut effective_end = end_date;

        // Clamp to delisting date if delisted within window
        if let Some(&delist_date) = self.delisted_tickers.get(&t) {
            if delist_date < start_date {
                return None;
            }
            if delist_date <= effective_end {
                effective_end = delist_date;
            }
        }

        // Clamp to interval end date if symbol ended within window
        if let Some(intervals) = self.ticker_intervals.get(&t) {
            for iv in intervals {
                if iv.start_date <= start_date && start_date <= iv.end_date {
                    if iv.end_date < effective_end && iv.end_date.year() < 9000 {
                        effective_end = iv.end_date;
                    }
                }
            }
        }

        if start_date > effective_end {
            return None;
        }

        Some((start_date, effective_end))
    }

    /// Filter a slice of tickers, retaining only those that were valid on `date`.
    pub fn filter_universe_by_date(&self, tickers: &[String], date: NaiveDate) -> Vec<String> {
        if !self.enabled {
            return tickers.to_vec();
        }
        tickers
            .iter()
            .filter(|t| self.is_valid_ticker(t, date))
            .cloned()
            .collect()
    }

    /// Check if a ticker was a constituent of the S&P 500 index on a specific date.
    pub fn is_in_sp500(&self, ticker: &str, date: NaiveDate) -> bool {
        if !self.enabled {
            return true;
        }

        let t = ticker.trim().to_uppercase();
        if let Some(intervals) = self.sp500_membership.get(&t) {
            return intervals
                .iter()
                .any(|(join, leave)| *join <= date && leave.map_or(true, |l| date <= l));
        }

        // If not in sp500 history file, assume false for strict index filtering
        false
    }

    /// Retrieve metadata for a delisted security if known.
    pub fn get_delisted_detail(&self, ticker: &str) -> Option<DelistedSecurity> {
        let t = ticker.trim().to_uppercase();
        self.delisted_details.get(&t).cloned()
    }
}

/// Thread-safe Point-in-Time data container supporting atomic zero-downtime hot-reloads.
#[derive(Debug, Clone)]
pub struct PITData {
    pub(crate) inner: Arc<RwLock<Arc<PITDataSnapshot>>>,
}

impl Default for PITData {
    fn default() -> Self {
        Self::empty()
    }
}

impl PITData {
    /// Create a new empty `PITData` container.
    pub fn empty() -> Self {
        Self {
            inner: Arc::new(RwLock::new(Arc::new(PITDataSnapshot::empty()))),
        }
    }

    /// Construct `PITData` from a pre-built snapshot.
    pub fn from_snapshot(snapshot: PITDataSnapshot) -> Self {
        Self {
            inner: Arc::new(RwLock::new(Arc::new(snapshot))),
        }
    }

    /// Load PIT configuration from environment or search default config paths.
    pub fn from_env() -> Self {
        let snapshot = PITDataSnapshot::from_env();
        Self::from_snapshot(snapshot)
    }

    /// Load and index JSON configuration files from the specified directory.
    pub fn load_from_dir(dir: &Path) -> Self {
        let snapshot = PITDataSnapshot::load_from_dir(dir);
        Self::from_snapshot(snapshot)
    }

    /// Reload PIT data from a specific directory atomically.
    pub fn reload(&self, dir: &Path) -> Result<(), String> {
        let new_snapshot = PITDataSnapshot::load_from_dir(dir);
        let mut guard = self.inner.write().map_err(|e| e.to_string())?;
        *guard = Arc::new(new_snapshot);
        info!(
            "[PIT Engine] Point-in-Time cache reloaded successfully from {}",
            dir.display()
        );
        Ok(())
    }

    /// Reload PIT data from environment / candidate paths atomically.
    pub fn reload_from_env(&self) -> Result<(), String> {
        let new_snapshot = PITDataSnapshot::from_env();
        let mut guard = self.inner.write().map_err(|e| e.to_string())?;
        *guard = Arc::new(new_snapshot);
        info!("[PIT Engine] Point-in-Time cache reloaded successfully from environment / default paths");
        Ok(())
    }

    /// Reload PIT data from a pre-built snapshot atomically.
    pub fn reload_from_snapshot(&self, snapshot: PITDataSnapshot) -> Result<(), String> {
        let mut guard = self.inner.write().map_err(|e| e.to_string())?;
        *guard = Arc::new(snapshot);
        info!("[PIT Engine] Point-in-Time cache reloaded successfully from external snapshot");
        Ok(())
    }

    /// Retrieve an immutable reference-counted clone of the current in-memory snapshot.
    pub fn snapshot(&self) -> Arc<PITDataSnapshot> {
        self.inner.read().unwrap().clone()
    }

    /// Check if Point-in-Time data enforcement is active.
    pub fn is_enabled(&self) -> bool {
        self.inner.read().unwrap().is_enabled()
    }

    /// Validate if a given ticker symbol was listed, active, and not delisted on a specific date.
    pub fn is_valid_ticker(&self, ticker: &str, date: NaiveDate) -> bool {
        self.inner.read().unwrap().is_valid_ticker(ticker, date)
    }

    /// Resolve a ticker symbol to its effective historical trading symbol on a specific date.
    pub fn resolve_symbol(&self, ticker: &str, date: NaiveDate) -> Option<String> {
        self.inner.read().unwrap().resolve_symbol(ticker, date)
    }

    /// Retrieve the official delisting date for a ticker if known.
    pub fn get_delisting_date(&self, ticker: &str) -> Option<NaiveDate> {
        self.inner.read().unwrap().get_delisting_date(ticker)
    }

    /// Retrieve metadata for a delisted security if known.
    pub fn get_delisted_detail(&self, ticker: &str) -> Option<DelistedSecurity> {
        self.inner.read().unwrap().get_delisted_detail(ticker)
    }

    /// Clamp a requested `[start_date, end_date]` query window to prevent querying post-delisting
    /// or post-symbol change data. Returns `Some((effective_start, effective_end))` or `None` if invalid throughout.
    pub fn clamp_query_range(
        &self,
        ticker: &str,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Option<(NaiveDate, NaiveDate)> {
        self.inner
            .read()
            .unwrap()
            .clamp_query_range(ticker, start_date, end_date)
    }

    /// Filter a slice of tickers, retaining only those that were valid on `date`.
    pub fn filter_universe_by_date(&self, tickers: &[String], date: NaiveDate) -> Vec<String> {
        self.inner
            .read()
            .unwrap()
            .filter_universe_by_date(tickers, date)
    }

    /// Check if a ticker was a constituent of the S&P 500 index on a specific date.
    pub fn is_in_sp500(&self, ticker: &str, date: NaiveDate) -> bool {
        self.inner.read().unwrap().is_in_sp500(ticker, date)
    }

    pub fn ticker_intervals_count(&self) -> usize {
        self.inner.read().unwrap().ticker_intervals.len()
    }

    pub fn delisted_tickers_count(&self) -> usize {
        self.inner.read().unwrap().delisted_tickers.len()
    }

    pub fn corporate_actions_count(&self) -> usize {
        self.inner.read().unwrap().corporate_actions.len()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Date Parsing Utilities
// ─────────────────────────────────────────────────────────────────────────────

fn parse_iso_date(iso_str: &str) -> Option<NaiveDate> {
    let s = iso_str.trim();
    if s.len() >= 10 {
        if let Ok(d) = NaiveDate::parse_from_str(&s[..10], "%Y-%m-%d") {
            return Some(d);
        }
    }
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.date_naive());
    }
    None
}

trait YearExtractor {
    fn year(&self) -> i32;
}

impl YearExtractor for NaiveDate {
    fn year(&self) -> i32 {
        use chrono::Datelike;
        Datelike::year(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_pit_data() -> PITData {
        let mut snap = PITDataSnapshot::empty();
        snap.enabled = true;

        // Meta / FB mapping
        let fb_start = NaiveDate::from_ymd_opt(2012, 5, 18).unwrap();
        let fb_end = NaiveDate::from_ymd_opt(2022, 6, 9).unwrap();
        let meta_start = NaiveDate::from_ymd_opt(2022, 6, 9).unwrap();
        let meta_end = NaiveDate::from_ymd_opt(9999, 12, 31).unwrap();

        snap.ticker_intervals.insert(
            "FB".to_string(),
            vec![ParsedTickerInterval {
                entity_id: "PERM_META".to_string(),
                ticker: "FB".to_string(),
                start_date: fb_start,
                end_date: fb_end,
            }],
        );

        snap.ticker_intervals.insert(
            "META".to_string(),
            vec![ParsedTickerInterval {
                entity_id: "PERM_META".to_string(),
                ticker: "META".to_string(),
                start_date: meta_start,
                end_date: meta_end,
            }],
        );

        snap.entity_mappings.insert(
            "PERM_META".to_string(),
            vec![
                ("FB".to_string(), fb_start, fb_end),
                ("META".to_string(), meta_start, meta_end),
            ],
        );
        snap.ticker_to_entity
            .insert("FB".to_string(), "PERM_META".to_string());
        snap.ticker_to_entity
            .insert("META".to_string(), "PERM_META".to_string());

        // Delisted: TWTR on 2022-10-28, SIVB on 2023-03-28
        let twtr_delist = NaiveDate::from_ymd_opt(2022, 10, 28).unwrap();
        let sivb_delist = NaiveDate::from_ymd_opt(2023, 3, 28).unwrap();
        snap.delisted_tickers
            .insert("TWTR".to_string(), twtr_delist);
        snap.delisted_tickers
            .insert("SIVB".to_string(), sivb_delist);

        snap.delisted_details.insert(
            "TWTR".to_string(),
            DelistedSecurity {
                ticker: "TWTR".to_string(),
                name: Some("Twitter, Inc.".to_string()),
                delisting_reason: Some("Acquisition by X Corp".to_string()),
                delisting_date_iso: "2022-10-28".to_string(),
                final_price_usd: Some(54.20),
                last_close_usd: Some(53.70),
                delisting_return: Some(0.0),
            },
        );

        // SP500 membership: TSLA joined 2020-12-21
        let tsla_join = NaiveDate::from_ymd_opt(2020, 12, 21).unwrap();
        snap.sp500_membership
            .insert("TSLA".to_string(), vec![(tsla_join, None)]);

        PITData::from_snapshot(snap)
    }

    #[test]
    fn test_ticker_validity_dates() {
        let pit = sample_pit_data();

        let date_2021 = NaiveDate::from_ymd_opt(2021, 6, 1).unwrap();
        let date_2023 = NaiveDate::from_ymd_opt(2023, 6, 1).unwrap();

        // In 2021: FB is valid, META is not valid
        assert!(pit.is_valid_ticker("FB", date_2021));
        assert!(!pit.is_valid_ticker("META", date_2021));

        // In 2023: FB is not valid, META is valid
        assert!(!pit.is_valid_ticker("FB", date_2023));
        assert!(pit.is_valid_ticker("META", date_2023));
    }

    #[test]
    fn test_delisted_ticker_validity() {
        let pit = sample_pit_data();

        let date_before_delist = NaiveDate::from_ymd_opt(2022, 1, 1).unwrap();
        let date_after_delist = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();

        assert!(pit.is_valid_ticker("TWTR", date_before_delist));
        assert!(!pit.is_valid_ticker("TWTR", date_after_delist));

        assert!(pit.is_valid_ticker("SIVB", date_before_delist));
        assert!(!pit.is_valid_ticker("SIVB", date_after_delist));
    }

    #[test]
    fn test_symbol_resolution() {
        let pit = sample_pit_data();

        let date_2021 = NaiveDate::from_ymd_opt(2021, 6, 1).unwrap();
        let date_2023 = NaiveDate::from_ymd_opt(2023, 6, 1).unwrap();

        // In 2021, META entity traded as FB
        assert_eq!(pit.resolve_symbol("FB", date_2021), Some("FB".to_string()));
        assert_eq!(
            pit.resolve_symbol("META", date_2021),
            Some("FB".to_string())
        );

        // In 2023, FB entity trades as META
        assert_eq!(
            pit.resolve_symbol("FB", date_2023),
            Some("META".to_string())
        );
        assert_eq!(
            pit.resolve_symbol("META", date_2023),
            Some("META".to_string())
        );

        // Delisted TWTR in 2023 has no active symbol
        assert_eq!(pit.resolve_symbol("TWTR", date_2023), None);
    }

    #[test]
    fn test_clamp_query_range() {
        let pit = sample_pit_data();

        let start_2022 = NaiveDate::from_ymd_opt(2022, 1, 1).unwrap();
        let end_2022 = NaiveDate::from_ymd_opt(2022, 12, 31).unwrap();

        // TWTR was delisted 2022-10-28 -> clamped to 2022-10-28
        let twtr_clamped = pit.clamp_query_range("TWTR", start_2022, end_2022);
        assert_eq!(
            twtr_clamped,
            Some((start_2022, NaiveDate::from_ymd_opt(2022, 10, 28).unwrap()))
        );

        // Querying TWTR in 2023 should return None
        let start_2023 = NaiveDate::from_ymd_opt(2023, 1, 1).unwrap();
        let end_2023 = NaiveDate::from_ymd_opt(2023, 6, 1).unwrap();
        assert_eq!(pit.clamp_query_range("TWTR", start_2023, end_2023), None);
    }

    #[test]
    fn test_sp500_membership_check() {
        let pit = sample_pit_data();

        let before_tsla = NaiveDate::from_ymd_opt(2020, 6, 1).unwrap();
        let after_tsla = NaiveDate::from_ymd_opt(2021, 6, 1).unwrap();

        assert!(!pit.is_in_sp500("TSLA", before_tsla));
        assert!(pit.is_in_sp500("TSLA", after_tsla));
    }

    #[test]
    fn test_pit_get_delisted_detail() {
        let pit = sample_pit_data();
        let twtr_detail = pit.get_delisted_detail("TWTR");
        assert!(twtr_detail.is_some());
        assert_eq!(
            twtr_detail.unwrap().delisting_reason.as_deref(),
            Some("Acquisition by X Corp")
        );

        let unknown = pit.get_delisted_detail("UNKNOWN_TICKER");
        assert!(unknown.is_none());
    }

    #[test]
    fn test_pit_data_hot_reload() {
        let pit = sample_pit_data();
        let date_2021 = NaiveDate::from_ymd_opt(2021, 6, 1).unwrap();
        assert!(pit.is_valid_ticker("FB", date_2021));

        // Hot reload with empty snapshot
        let mut new_snap = PITDataSnapshot::empty();
        new_snap.enabled = true;
        *pit.inner.write().unwrap() = Arc::new(new_snap);

        assert_eq!(pit.ticker_intervals_count(), 0);
    }
}
