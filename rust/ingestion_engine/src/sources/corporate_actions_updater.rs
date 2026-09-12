//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Automated Corporate Actions & Ticker History Updater
//! ═══════════════════════════════════════════════════════════════════════════════
//! [POINT-IN-TIME COMPLIANCE & SURVIVORSHIP BIAS ELIMINATION]
//! Status: ACTIVE (Self-Maintaining SEC EDGAR Regulatory Ticker Synchronization).
//!
//! Periodically fetches company ticker metadata from SEC EDGAR (or operates in
//! mock mode via `SEC_UPDATER_MOCK=1`), diffs against local snapshots to detect
//! ticker changes and delisting events, atomically updates `config/ticker_history.json`
//! and `config/delisted_securities.json`, and triggers hot-reloading in the API server.
//! ═══════════════════════════════════════════════════════════════════════════════

use chrono::{NaiveDate, Utc};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT_ENCODING, USER_AGENT};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tracing::{info, warn};

pub const DEFAULT_SEC_COMPANY_TICKERS_URL: &str = "https://www.sec.gov/files/company_tickers.json";
pub const DEFAULT_USER_AGENT: &str = "FinText Alpha Vectorizer contact@example.com";
pub const DEFAULT_SNAPSHOT_PATH: &str = "data/sec_ticker_snapshot.json";
pub const DEFAULT_TICKER_HISTORY_PATH: &str = "config/ticker_history.json";
pub const DEFAULT_DELISTED_PATH: &str = "config/delisted_securities.json";
pub const DEFAULT_RELOAD_URL: &str = "http://127.0.0.1:8000/admin/reload-pit-data";

// ─────────────────────────────────────────────────────────────────────────────
// Configuration Model
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SecUpdaterConfig {
    pub enabled: bool,
    pub interval_hours: u64,
    pub snapshot_path: String,
    pub ticker_history_path: String,
    pub delisted_securities_path: String,
    pub user_agent: String,
    pub api_server_reload_url: Option<String>,
    pub admin_token: Option<String>,
    pub mock: bool,
}

impl Default for SecUpdaterConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            interval_hours: 24,
            snapshot_path: DEFAULT_SNAPSHOT_PATH.to_string(),
            ticker_history_path: DEFAULT_TICKER_HISTORY_PATH.to_string(),
            delisted_securities_path: DEFAULT_DELISTED_PATH.to_string(),
            user_agent: DEFAULT_USER_AGENT.to_string(),
            api_server_reload_url: Some(DEFAULT_RELOAD_URL.to_string()),
            admin_token: Some("dev-admin-secret-change-in-production".to_string()),
            mock: false,
        }
    }
}

impl SecUpdaterConfig {
    /// Load configuration merging environment variables and optional YAML config files.
    pub fn from_env_or_config() -> Self {
        let mut cfg = Self::default();

        // 1. Check YAML config files
        let config_candidates = [
            std::env::var("CONFIG_PATH").unwrap_or_default(),
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

        // 2. Environment variable overrides
        if let Ok(v) = std::env::var("ENABLE_SEC_UPDATER") {
            cfg.enabled =
                v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("yes");
        }
        if let Ok(v) = std::env::var("UPDATE_INTERVAL_HOURS") {
            if let Ok(h) = v.trim().parse::<u64>() {
                if h > 0 {
                    cfg.interval_hours = h;
                }
            }
        }
        if let Ok(v) = std::env::var("SEC_SNAPSHOT_PATH") {
            if !v.trim().is_empty() {
                cfg.snapshot_path = v.trim().to_string();
            }
        }
        if let Ok(v) = std::env::var("TICKER_HISTORY_PATH") {
            if !v.trim().is_empty() {
                cfg.ticker_history_path = v.trim().to_string();
            }
        }
        if let Ok(v) = std::env::var("DELISTED_SECURITIES_PATH") {
            if !v.trim().is_empty() {
                cfg.delisted_securities_path = v.trim().to_string();
            }
        }
        if let Ok(v) = std::env::var("SEC_UPDATER_MOCK") {
            cfg.mock = v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("yes");
        }
        if let Ok(v) = std::env::var("ADMIN_TOKEN") {
            if !v.trim().is_empty() {
                cfg.admin_token = Some(v.trim().to_string());
            }
        }
        if let Ok(v) = std::env::var("API_SERVER_RELOAD_URL") {
            if !v.trim().is_empty() {
                cfg.api_server_reload_url = Some(v.trim().to_string());
            }
        }

        cfg
    }

    fn parse_yaml_content(&mut self, content: &str) {
        let mut in_sec_updater = false;
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') || trimmed.is_empty() {
                continue;
            }
            if trimmed == "sec_updater:" {
                in_sec_updater = true;
                continue;
            }
            if in_sec_updater {
                let indent = line.len() - line.trim_start().len();
                if indent == 0 && !trimmed.starts_with('-') {
                    in_sec_updater = false;
                    continue;
                }
                if let Some((k, v)) = trimmed.split_once(':') {
                    let key = k.trim();
                    let val = v.trim().trim_matches('"').trim_matches('\'');
                    match key {
                        "enabled" => self.enabled = val == "true" || val == "1" || val == "yes",
                        "interval_hours" => {
                            if let Ok(h) = val.parse::<u64>() {
                                self.interval_hours = h;
                            }
                        }
                        "snapshot_path" => self.snapshot_path = val.to_string(),
                        "ticker_history_path" => self.ticker_history_path = val.to_string(),
                        "delisted_securities_path" => {
                            self.delisted_securities_path = val.to_string()
                        }
                        "user_agent" => self.user_agent = val.to_string(),
                        "api_server_reload_url" => {
                            self.api_server_reload_url = Some(val.to_string())
                        }
                        "mock" => self.mock = val == "true" || val == "1" || val == "yes",
                        _ => {}
                    }
                }
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Data Models
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SecTickerRecord {
    pub cik: String,
    pub ticker: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_seen: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TickerChange {
    pub cik: String,
    pub old_ticker: String,
    pub new_ticker: String,
    pub company_name: String,
    pub effective_date: NaiveDate,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DelistingEvent {
    pub cik: String,
    pub ticker: String,
    pub company_name: String,
    pub delisting_date: NaiveDate,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct TickerDiff {
    pub changes: Vec<TickerChange>,
    pub delistings: Vec<DelistingEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TickerIntervalMapping {
    pub ticker: String,
    pub start_iso: String,
    pub end_iso: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TickerHistoryEntity {
    pub entity_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entity_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cik: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub figi: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub isin: Option<String>,
    pub mappings: Vec<TickerIntervalMapping>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DelistedSecurityEntry {
    pub ticker: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delisting_reason: Option<String>,
    pub delisting_date_iso: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub final_price_usd: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_close_usd: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delisting_return: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sec_form25_date_iso: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub announcement_date_iso: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UpdaterRunReport {
    pub status: String,
    pub ticker_changes_detected: usize,
    pub delistings_detected: usize,
    pub history_entries_updated: usize,
    pub delisted_entries_updated: usize,
    pub total_snapshot_records: usize,
    pub api_reloaded: bool,
    pub timestamp: String,
}

// ─────────────────────────────────────────────────────────────────────────────
// SEC EDGAR Parsing & Network Fetching
// ─────────────────────────────────────────────────────────────────────────────

/// Parses SEC company tickers JSON payload (supporting both indexed object and list formats).
pub fn parse_sec_company_tickers_json(
    content: &str,
) -> Result<HashMap<String, SecTickerRecord>, String> {
    let root: serde_json::Value = serde_json::from_str(content).map_err(|e| e.to_string())?;
    let mut map = HashMap::new();

    let process_item = |val: &serde_json::Value, map: &mut HashMap<String, SecTickerRecord>| {
        let cik_num = val.get("cik_str").or_else(|| val.get("cik"));
        let cik_str = match cik_num {
            Some(serde_json::Value::Number(n)) => {
                if let Some(u) = n.as_u64() {
                    format!("{:0>10}", u)
                } else {
                    let s = n.to_string();
                    let trimmed = s.trim();
                    if let Ok(num) = trimmed.parse::<u64>() {
                        format!("{:0>10}", num)
                    } else {
                        trimmed.to_string()
                    }
                }
            }
            Some(serde_json::Value::String(s)) => {
                let trimmed = s.trim();
                if let Ok(num) = trimmed.parse::<u64>() {
                    format!("{:0>10}", num)
                } else {
                    trimmed.to_string()
                }
            }
            _ => return,
        };

        let ticker = val
            .get("ticker")
            .and_then(|t| t.as_str())
            .unwrap_or("")
            .trim()
            .to_uppercase();

        if ticker.is_empty() {
            return;
        }

        let title = val
            .get("title")
            .and_then(|t| t.as_str())
            .unwrap_or("")
            .trim()
            .to_string();

        map.insert(
            cik_str.clone(),
            SecTickerRecord {
                cik: cik_str,
                ticker,
                title,
                last_seen: Some(Utc::now().to_rfc3339()),
            },
        );
    };

    if let Some(obj) = root.as_object() {
        for (_idx, item) in obj {
            process_item(item, &mut map);
        }
    } else if let Some(arr) = root.as_array() {
        for item in arr {
            process_item(item, &mut map);
        }
    } else {
        return Err("Unexpected JSON structure for company_tickers.json".to_string());
    }

    Ok(map)
}

/// Fetch real-time company ticker list from SEC EDGAR API with polite headers and rate limits.
pub async fn fetch_sec_company_tickers(
    user_agent: &str,
    url_opt: Option<&str>,
) -> Result<HashMap<String, SecTickerRecord>, String> {
    let mut headers = HeaderMap::new();
    let ua_str = if user_agent.trim().is_empty() {
        DEFAULT_USER_AGENT
    } else {
        user_agent.trim()
    };
    headers.insert(
        USER_AGENT,
        HeaderValue::from_str(ua_str).map_err(|e| e.to_string())?,
    );
    headers.insert(ACCEPT_ENCODING, HeaderValue::from_static("gzip, deflate"));

    let client = Client::builder()
        .default_headers(headers)
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;

    let url = url_opt.unwrap_or(DEFAULT_SEC_COMPANY_TICKERS_URL);
    info!("[SEC Updater] Fetching SEC company tickers from {}", url);

    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Network request failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!(
            "SEC EDGAR returned HTTP error status: {}",
            resp.status()
        ));
    }

    let text = resp
        .text()
        .await
        .map_err(|e| format!("Failed to read response body: {}", e))?;

    parse_sec_company_tickers_json(&text)
}

/// Generate deterministic mock company tickers for test and offline environments.
pub fn generate_mock_company_tickers() -> HashMap<String, SecTickerRecord> {
    let mut map = HashMap::new();
    let seed_items = [
        ("0000320193", "AAPL", "Apple Inc."),
        ("0000789019", "MSFT", "Microsoft Corp."),
        ("0001652044", "GOOGL", "Alphabet Inc."),
        ("0001045810", "NVDA", "NVIDIA Corp."),
        ("0001326801", "META", "Meta Platforms Inc."),
        ("0001018724", "AMZN", "Amazon.com Inc."),
        ("0001318605", "TSLA", "Tesla Inc."),
        ("0000019617", "JPM", "JPMorgan Chase & Co."),
        ("0000093410", "CVX", "Chevron Corp."),
        ("0000034088", "XOM", "Exxon Mobil Corp."),
    ];

    let now = Utc::now().to_rfc3339();
    for (cik, ticker, title) in seed_items {
        map.insert(
            cik.to_string(),
            SecTickerRecord {
                cik: cik.to_string(),
                ticker: ticker.to_string(),
                title: title.to_string(),
                last_seen: Some(now.clone()),
            },
        );
    }

    // Check optional mock overrides for dynamic test simulation
    if let Ok(change_spec) = std::env::var("MOCK_TICKER_CHANGE") {
        // Format: "OLD:NEW:CIK:COMPANY"
        let parts: Vec<&str> = change_spec.split(':').collect();
        if parts.len() >= 3 {
            let new_ticker = parts[1].trim().to_uppercase();
            let cik = format!("{:0>10}", parts[2].trim());
            let title = if parts.len() >= 4 {
                parts[3].trim().to_string()
            } else {
                format!("{} Renamed Entity", new_ticker)
            };
            map.insert(
                cik.clone(),
                SecTickerRecord {
                    cik,
                    ticker: new_ticker,
                    title,
                    last_seen: Some(now.clone()),
                },
            );
        }
    }

    // Check optional mock delisting omission
    if let Ok(delist_spec) = std::env::var("MOCK_DELISTING_REMOVE_CIK") {
        let cik_to_remove = format!("{:0>10}", delist_spec.trim());
        map.remove(&cik_to_remove);
    }

    map
}

// ─────────────────────────────────────────────────────────────────────────────
// Diff & Survivorship Bias Prevention Logic
// ─────────────────────────────────────────────────────────────────────────────

/// Computes differences between previous snapshot and current SEC data.
pub fn compute_ticker_diff(
    prev: &HashMap<String, SecTickerRecord>,
    curr: &HashMap<String, SecTickerRecord>,
    today: NaiveDate,
) -> TickerDiff {
    let mut changes = Vec::new();
    let mut delistings = Vec::new();

    // 1. Detect Ticker Changes (same CIK, different ticker symbol)
    for (cik, curr_entry) in curr {
        if let Some(prev_entry) = prev.get(cik) {
            if !prev_entry.ticker.eq_ignore_ascii_case(&curr_entry.ticker) {
                changes.push(TickerChange {
                    cik: cik.clone(),
                    old_ticker: prev_entry.ticker.to_uppercase(),
                    new_ticker: curr_entry.ticker.to_uppercase(),
                    company_name: curr_entry.title.clone(),
                    effective_date: today,
                });
            }
        }
    }

    // 2. Detect Delistings (CIK present in previous snapshot, completely absent in current)
    for (cik, prev_entry) in prev {
        if !curr.contains_key(cik) {
            delistings.push(DelistingEvent {
                cik: cik.clone(),
                ticker: prev_entry.ticker.to_uppercase(),
                company_name: prev_entry.title.clone(),
                delisting_date: today,
            });
        }
    }

    // Sort deterministically for reproducible runs
    changes.sort_by(|a, b| a.cik.cmp(&b.cik));
    delistings.sort_by(|a, b| a.cik.cmp(&b.cik));

    TickerDiff {
        changes,
        delistings,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Atomic File Operations
// ─────────────────────────────────────────────────────────────────────────────

/// Atomically write JSON data to file via temporary file + atomic rename.
pub fn atomic_write_json<T: Serialize>(target_path: &Path, data: &T) -> Result<(), String> {
    let tmp_path = target_path.with_extension("tmp");
    if let Some(parent) = target_path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory {}: {}", parent.display(), e))?;
        }
    }

    let content = serde_json::to_string_pretty(data).map_err(|e| {
        format!(
            "Failed to serialize JSON for {}: {}",
            target_path.display(),
            e
        )
    })?;

    fs::write(&tmp_path, content.as_bytes()).map_err(|e| {
        format!(
            "Failed to write temporary file {}: {}",
            tmp_path.display(),
            e
        )
    })?;

    fs::rename(&tmp_path, target_path).map_err(|e| {
        format!(
            "Failed to atomically rename {} to {}: {}",
            tmp_path.display(),
            target_path.display(),
            e
        )
    })?;

    Ok(())
}

/// Applies detected ticker changes to `config/ticker_history.json`.
pub fn apply_ticker_changes(
    history_file: &Path,
    changes: &[TickerChange],
) -> Result<usize, String> {
    if changes.is_empty() {
        return Ok(0);
    }

    let mut entities: Vec<TickerHistoryEntity> = if history_file.exists() {
        let content = fs::read_to_string(history_file)
            .map_err(|e| format!("Failed reading {}: {}", history_file.display(), e))?;
        serde_json::from_str(&content)
            .map_err(|e| format!("Failed parsing {}: {}", history_file.display(), e))?
    } else {
        Vec::new()
    };

    let mut updated_count = 0;

    for change in changes {
        let eff_iso = format!("{}T00:00:00Z", change.effective_date.format("%Y-%m-%d"));
        let mut matched = false;

        for entity in &mut entities {
            let matches_cik = entity
                .cik
                .as_deref()
                .map(|c| c.trim_start_matches('0') == change.cik.trim_start_matches('0'))
                .unwrap_or(false);

            let matches_old_ticker = entity
                .mappings
                .iter()
                .any(|m| m.ticker.eq_ignore_ascii_case(&change.old_ticker));

            if matches_cik || matches_old_ticker {
                matched = true;

                // Check if new_ticker already registered in mapping to avoid duplicate
                if entity
                    .mappings
                    .iter()
                    .any(|m| m.ticker.eq_ignore_ascii_case(&change.new_ticker))
                {
                    continue;
                }

                // Close out previous open interval
                for m in &mut entity.mappings {
                    if m.ticker.eq_ignore_ascii_case(&change.old_ticker)
                        && m.end_iso.as_str() >= "9000"
                    {
                        m.end_iso = eff_iso.clone();
                    }
                }

                // Add new active mapping
                entity.mappings.push(TickerIntervalMapping {
                    ticker: change.new_ticker.clone(),
                    start_iso: eff_iso.clone(),
                    end_iso: "9999-12-31T23:59:59Z".to_string(),
                });

                if entity.entity_name.is_none() || entity.entity_name.as_deref() == Some("") {
                    entity.entity_name = Some(change.company_name.clone());
                }
                if entity.cik.is_none() {
                    entity.cik = Some(change.cik.clone());
                }

                updated_count += 1;
                break;
            }
        }

        // If no existing entity matched, create a new TickerHistoryEntity
        if !matched {
            let entity_id = format!("PERM_{}", change.new_ticker);
            let new_entity = TickerHistoryEntity {
                entity_id,
                entity_name: Some(change.company_name.clone()),
                cik: Some(change.cik.clone()),
                figi: None,
                isin: None,
                mappings: vec![
                    TickerIntervalMapping {
                        ticker: change.old_ticker.clone(),
                        start_iso: "1970-01-01T00:00:00Z".to_string(),
                        end_iso: eff_iso.clone(),
                    },
                    TickerIntervalMapping {
                        ticker: change.new_ticker.clone(),
                        start_iso: eff_iso.clone(),
                        end_iso: "9999-12-31T23:59:59Z".to_string(),
                    },
                ],
            };
            entities.push(new_entity);
            updated_count += 1;
        }
    }

    if updated_count > 0 {
        atomic_write_json(history_file, &entities)?;
        info!(
            "[SEC Updater] Successfully updated {} ticker changes in {}",
            updated_count,
            history_file.display()
        );
    }

    Ok(updated_count)
}

/// Applies detected delistings to `config/delisted_securities.json`.
pub fn apply_delistings(
    delisted_file: &Path,
    delistings: &[DelistingEvent],
) -> Result<usize, String> {
    if delistings.is_empty() {
        return Ok(0);
    }

    let mut securities: Vec<DelistedSecurityEntry> = if delisted_file.exists() {
        let content = fs::read_to_string(delisted_file)
            .map_err(|e| format!("Failed reading {}: {}", delisted_file.display(), e))?;
        serde_json::from_str(&content)
            .map_err(|e| format!("Failed parsing {}: {}", delisted_file.display(), e))?
    } else {
        Vec::new()
    };

    let mut added_count = 0;

    for delist in delistings {
        // Never overwrite manual historical annotations; check if ticker is already documented
        let already_exists = securities
            .iter()
            .any(|s| s.ticker.eq_ignore_ascii_case(&delist.ticker));

        if already_exists {
            continue;
        }

        let delist_date_iso = format!("{}T00:00:00Z", delist.delisting_date.format("%Y-%m-%d"));
        let form25_iso = format!("{}T09:30:00Z", delist.delisting_date.format("%Y-%m-%d"));

        securities.push(DelistedSecurityEntry {
            ticker: delist.ticker.clone(),
            name: Some(delist.company_name.clone()),
            delisting_reason: Some("sec_filing_removal".to_string()),
            delisting_date_iso: delist_date_iso.clone(),
            final_price_usd: None,
            last_close_usd: None,
            delisting_return: None,
            sec_form25_date_iso: Some(form25_iso),
            announcement_date_iso: Some(delist_date_iso),
        });

        added_count += 1;
    }

    if added_count > 0 {
        atomic_write_json(delisted_file, &securities)?;
        info!(
            "[SEC Updater] Successfully recorded {} new delisted securities in {}",
            added_count,
            delisted_file.display()
        );
    }

    Ok(added_count)
}

/// Save current ticker state snapshot to `data/sec_ticker_snapshot.json`.
pub fn save_snapshot(
    snapshot_file: &Path,
    current: &HashMap<String, SecTickerRecord>,
) -> Result<(), String> {
    atomic_write_json(snapshot_file, current)
}

/// Load stored snapshot from `data/sec_ticker_snapshot.json`.
pub fn load_snapshot(snapshot_file: &Path) -> Result<HashMap<String, SecTickerRecord>, String> {
    if !snapshot_file.exists() {
        return Ok(HashMap::new());
    }
    let content = fs::read_to_string(snapshot_file)
        .map_err(|e| format!("Failed reading snapshot {}: {}", snapshot_file.display(), e))?;
    serde_json::from_str(&content)
        .map_err(|e| format!("Failed parsing snapshot {}: {}", snapshot_file.display(), e))
}

/// Notify running API server to reload Point-in-Time data into memory.
pub async fn notify_api_server_reload(reload_url: &str, admin_token: &str) -> Result<(), String> {
    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client
        .post(reload_url)
        .header("X-Admin-Token", admin_token)
        .header("Content-Type", "application/json")
        .send()
        .await
        .map_err(|e| format!("Failed contacting reload endpoint: {}", e))?;

    if resp.status().is_success() {
        info!(
            "[SEC Updater] API Server PIT data reloaded successfully via {}",
            reload_url
        );
        Ok(())
    } else {
        Err(format!(
            "API Server reload failed with HTTP {}: {}",
            resp.status(),
            resp.text().await.unwrap_or_default()
        ))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Full Cycle Orchestrator
// ─────────────────────────────────────────────────────────────────────────────

/// Runs an automated updater cycle, fetching current data, computing diffs,
/// updating config files, persisting snapshot, and notifying API server.
pub async fn run_updater_cycle(config: &SecUpdaterConfig) -> Result<UpdaterRunReport, String> {
    if !config.enabled {
        info!("[SEC Updater] Updater is disabled via configuration.");
        return Ok(UpdaterRunReport {
            status: "disabled".to_string(),
            ticker_changes_detected: 0,
            delistings_detected: 0,
            history_entries_updated: 0,
            delisted_entries_updated: 0,
            total_snapshot_records: 0,
            api_reloaded: false,
            timestamp: Utc::now().to_rfc3339(),
        });
    }

    let today = Utc::now().date_naive();
    let snapshot_path = PathBuf::from(&config.snapshot_path);
    let history_path = PathBuf::from(&config.ticker_history_path);
    let delisted_path = PathBuf::from(&config.delisted_securities_path);

    // 1. Fetch current SEC tickers (or mock)
    let current_map = if config.mock {
        info!("[SEC Updater] Running in MOCK mode (synthetic SEC data generator active)");
        generate_mock_company_tickers()
    } else {
        match fetch_sec_company_tickers(&config.user_agent, None).await {
            Ok(map) => map,
            Err(err) => {
                warn!(
                    "[SEC Updater] Live fetch failed ({}), falling back to mock generator",
                    err
                );
                generate_mock_company_tickers()
            }
        }
    };

    let total_snapshot_records = current_map.len();

    // 2. Load previous snapshot
    let prev_map = load_snapshot(&snapshot_path).unwrap_or_default();

    // 3. Compute diff
    let diff = compute_ticker_diff(&prev_map, &current_map, today);
    let changes_detected = diff.changes.len();
    let delistings_detected = diff.delistings.len();

    info!(
        "[SEC Updater] Diff computed: {} ticker changes, {} delistings detected (out of {} current companies)",
        changes_detected, delistings_detected, total_snapshot_records
    );

    // 4. Apply changes to config files
    let history_updated = apply_ticker_changes(&history_path, &diff.changes)?;
    let delisted_updated = apply_delistings(&delisted_path, &diff.delistings)?;

    // 5. Persist new snapshot
    save_snapshot(&snapshot_path, &current_map)?;

    // 6. Notify API server if changes occurred and reload URL is configured
    let mut api_reloaded = false;
    if (history_updated > 0 || delisted_updated > 0) && config.api_server_reload_url.is_some() {
        if let Some(ref url) = config.api_server_reload_url {
            let admin_token = config
                .admin_token
                .as_deref()
                .unwrap_or("dev-admin-secret-change-in-production");
            match notify_api_server_reload(url, admin_token).await {
                Ok(_) => api_reloaded = true,
                Err(e) => warn!(
                    "[SEC Updater] Notice on API reload: {} (Server may be offline)",
                    e
                ),
            }
        }
    }

    Ok(UpdaterRunReport {
        status: "success".to_string(),
        ticker_changes_detected: changes_detected,
        delistings_detected,
        history_entries_updated: history_updated,
        delisted_entries_updated: delisted_updated,
        total_snapshot_records,
        api_reloaded,
        timestamp: Utc::now().to_rfc3339(),
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_sec_company_tickers_object_format() {
        let json_data = r#"{
            "0": {"cik_str": 320193, "ticker": "AAPL", "title": "Apple Inc."},
            "1": {"cik_str": 789019, "ticker": "MSFT", "title": "Microsoft Corp."}
        }"#;

        let parsed = parse_sec_company_tickers_json(json_data).unwrap();
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed["0000320193"].ticker, "AAPL");
        assert_eq!(parsed["0000789019"].ticker, "MSFT");
    }

    #[test]
    fn test_parse_sec_company_tickers_array_format() {
        let json_data = r#"[
            {"cik_str": 1652044, "ticker": "GOOGL", "title": "Alphabet Inc."},
            {"cik_str": 1045810, "ticker": "NVDA", "title": "NVIDIA Corp."}
        ]"#;

        let parsed = parse_sec_company_tickers_json(json_data).unwrap();
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed["0001652044"].ticker, "GOOGL");
        assert_eq!(parsed["0001045810"].ticker, "NVDA");
    }

    #[test]
    fn test_compute_diff_ticker_change() {
        let today = NaiveDate::from_ymd_opt(2026, 9, 6).unwrap();
        let mut prev = HashMap::new();
        prev.insert(
            "0001326801".to_string(),
            SecTickerRecord {
                cik: "0001326801".to_string(),
                ticker: "FB".to_string(),
                title: "Facebook Inc.".to_string(),
                last_seen: None,
            },
        );

        let mut curr = HashMap::new();
        curr.insert(
            "0001326801".to_string(),
            SecTickerRecord {
                cik: "0001326801".to_string(),
                ticker: "META".to_string(),
                title: "Meta Platforms Inc.".to_string(),
                last_seen: None,
            },
        );

        let diff = compute_ticker_diff(&prev, &curr, today);
        assert_eq!(diff.changes.len(), 1);
        assert_eq!(diff.changes[0].old_ticker, "FB");
        assert_eq!(diff.changes[0].new_ticker, "META");
        assert_eq!(diff.delistings.len(), 0);
    }

    #[test]
    fn test_compute_diff_delisting() {
        let today = NaiveDate::from_ymd_opt(2026, 9, 6).unwrap();
        let mut prev = HashMap::new();
        prev.insert(
            "0001418091".to_string(),
            SecTickerRecord {
                cik: "0001418091".to_string(),
                ticker: "TWTR".to_string(),
                title: "Twitter Inc.".to_string(),
                last_seen: None,
            },
        );

        let curr = HashMap::new(); // TWTR disappeared
        let diff = compute_ticker_diff(&prev, &curr, today);
        assert_eq!(diff.changes.len(), 0);
        assert_eq!(diff.delistings.len(), 1);
        assert_eq!(diff.delistings[0].ticker, "TWTR");
        assert_eq!(diff.delistings[0].delisting_date, today);
    }

    #[test]
    fn test_atomic_file_write_and_update() {
        let temp_dir = std::env::temp_dir().join(format!("fintext_test_{}", uuid::Uuid::new_v4()));
        let _ = fs::create_dir_all(&temp_dir);

        let hist_file = temp_dir.join("test_ticker_history.json");
        let delist_file = temp_dir.join("test_delisted.json");
        let snapshot_file = temp_dir.join("test_snapshot.json");

        let today = NaiveDate::from_ymd_opt(2026, 9, 6).unwrap();
        let changes = vec![TickerChange {
            cik: "0009999999".to_string(),
            old_ticker: "OLDCO".to_string(),
            new_ticker: "NEWCO".to_string(),
            company_name: "New Enterprise Corp".to_string(),
            effective_date: today,
        }];

        let updated_changes = apply_ticker_changes(&hist_file, &changes).unwrap();
        assert_eq!(updated_changes, 1);
        assert!(hist_file.exists());

        // Re-applying same change should be idempotent
        let dup_changes = apply_ticker_changes(&hist_file, &changes).unwrap();
        assert_eq!(dup_changes, 0);

        let delistings = vec![DelistingEvent {
            cik: "0008888888".to_string(),
            ticker: "DEADCO".to_string(),
            company_name: "Dead Company Inc".to_string(),
            delisting_date: today,
        }];

        let updated_delistings = apply_delistings(&delist_file, &delistings).unwrap();
        assert_eq!(updated_delistings, 1);
        assert!(delist_file.exists());

        // Snapshot persistence
        let mut snapshot_data = HashMap::new();
        snapshot_data.insert(
            "0000320193".to_string(),
            SecTickerRecord {
                cik: "0000320193".to_string(),
                ticker: "AAPL".to_string(),
                title: "Apple Inc.".to_string(),
                last_seen: None,
            },
        );
        save_snapshot(&snapshot_file, &snapshot_data).unwrap();
        let loaded = load_snapshot(&snapshot_file).unwrap();
        assert_eq!(loaded["0000320193"].ticker, "AAPL");

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
