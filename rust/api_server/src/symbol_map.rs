//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Permanent Security Identifier Mapping Engine
//! ═══════════════════════════════════════════════════════════════════════════════

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{info, warn};
use utoipa::ToSchema;

/// Global default SymbolMap singleton.
pub static GLOBAL_SYMBOL_MAP: Lazy<Arc<SymbolMap>> = Lazy::new(|| Arc::new(SymbolMap::from_env()));

/// Canonical financial security identifiers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema, Default)]
pub struct SecurityIdentifiers {
    /// Stock asset ticker symbol (e.g., 'AAPL', 'MSFT')
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "AAPL")]
    pub ticker: Option<String>,
    /// Bloomberg Financial Instrument Global Identifier (FIGI)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "BBG000B9XRY4")]
    pub figi: Option<String>,
    /// Committee on Uniform Securities Identification Procedures (CUSIP)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "037833100")]
    pub cusip: Option<String>,
    /// International Securities Identification Number (ISIN)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "US0378331005")]
    pub isin: Option<String>,
}

/// Raw record format in `permanent_identifiers.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawSecurityIdentifier {
    pub ticker: String,
    pub figi: Option<String>,
    pub cusip: Option<String>,
    pub isin: Option<String>,
    pub effective_date: Option<String>,
    pub end_date: Option<String>,
}

/// In-memory cross-identifier lookup index supporting O(1) bi-directional resolution.
#[derive(Debug, Clone, Default)]
pub struct SymbolMap {
    /// Ticker (uppercase) -> Full Identifiers
    pub ticker_to_ids: HashMap<String, SecurityIdentifiers>,
    /// FIGI (uppercase) -> Full Identifiers
    pub figi_to_ids: HashMap<String, SecurityIdentifiers>,
    /// CUSIP (uppercase) -> Full Identifiers
    pub cusip_to_ids: HashMap<String, SecurityIdentifiers>,
    /// ISIN (uppercase) -> Full Identifiers
    pub isin_to_ids: HashMap<String, SecurityIdentifiers>,
}

fn config_dir() -> PathBuf {
    std::env::var("FINTEXT_CONFIG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("config"))
}

impl SymbolMap {
    /// Create an empty symbol map.
    pub fn empty() -> Self {
        Self {
            ticker_to_ids: HashMap::new(),
            figi_to_ids: HashMap::new(),
            cusip_to_ids: HashMap::new(),
            isin_to_ids: HashMap::new(),
        }
    }

    /// Load symbol mapping from environment or standard config search paths.
    pub fn from_env() -> Self {
        let candidate_paths = if let Ok(custom_path) = env::var("PERMANENT_IDENTIFIERS_PATH") {
            vec![PathBuf::from(custom_path)]
        } else {
            vec![
                config_dir().join("permanent_identifiers.json"),
                PathBuf::from("./config/permanent_identifiers.json"),
                PathBuf::from("../config/permanent_identifiers.json"),
                PathBuf::from("../../config/permanent_identifiers.json"),
            ]
        };

        for path in candidate_paths {
            if path.is_file() {
                info!(
                    "[SymbolMap] Loading permanent identifiers from: {}",
                    path.display()
                );
                return Self::load_from_path(&path);
            }
        }

        warn!("[SymbolMap] No permanent_identifiers.json found. Using built-in core identifiers fallback.");
        Self::fallback_default()
    }

    /// Parse JSON array of security identifiers into multi-key lookup indices.
    pub fn load_from_path(path: &Path) -> Self {
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                warn!("[SymbolMap] Failed to read identifier file: {}", e);
                return Self::fallback_default();
            }
        };

        let records: Vec<RawSecurityIdentifier> = match serde_json::from_str(&content) {
            Ok(r) => r,
            Err(e) => {
                warn!(
                    "[SymbolMap] Failed to parse permanent_identifiers.json: {}",
                    e
                );
                return Self::fallback_default();
            }
        };

        let mut ticker_to_ids = HashMap::new();
        let mut figi_to_ids = HashMap::new();
        let mut cusip_to_ids = HashMap::new();
        let mut isin_to_ids = HashMap::new();

        for rec in records {
            let ids = SecurityIdentifiers {
                ticker: Some(rec.ticker.clone()),
                figi: rec.figi.clone(),
                cusip: rec.cusip.clone(),
                isin: rec.isin.clone(),
            };

            let ticker_key = rec.ticker.trim().to_uppercase();
            ticker_to_ids.insert(ticker_key, ids.clone());

            if let Some(ref figi) = rec.figi {
                let figi_key = figi.trim().to_uppercase();
                if !figi_key.is_empty() {
                    figi_to_ids.insert(figi_key, ids.clone());
                }
            }

            if let Some(ref cusip) = rec.cusip {
                let cusip_key = cusip.trim().to_uppercase();
                if !cusip_key.is_empty() {
                    cusip_to_ids.insert(cusip_key, ids.clone());
                }
            }

            if let Some(ref isin) = rec.isin {
                let isin_key = isin.trim().to_uppercase();
                if !isin_key.is_empty() {
                    isin_to_ids.insert(isin_key, ids.clone());
                }
            }
        }

        info!(
            "[SymbolMap] Loaded {} tickers, {} FIGIs, {} CUSIPs, {} ISINs",
            ticker_to_ids.len(),
            figi_to_ids.len(),
            cusip_to_ids.len(),
            isin_to_ids.len()
        );

        Self {
            ticker_to_ids,
            figi_to_ids,
            cusip_to_ids,
            isin_to_ids,
        }
    }

    /// Hardcoded fallback with core institutional identifiers.
    pub fn fallback_default() -> Self {
        let entries = vec![
            ("AAPL", "BBG000B9XRY4", "037833100", "US0378331005"),
            ("MSFT", "BBG000BPH459", "594918104", "US5949181045"),
            ("NVDA", "BBG000BBJQV0", "67066G104", "US67066G1040"),
            ("TSLA", "BBG000N9MNX3", "88160R101", "US88160R1014"),
            ("AMZN", "BBG000BVPV84", "023135106", "US0231351067"),
            ("META", "BBG000MM2P62", "30303M102", "US30303M1027"),
            ("FB", "BBG000MM2P62", "30303M102", "US30303M1027"),
            ("GOOGL", "BBG009S39JX6", "02079K305", "US02079K3059"),
            ("AMD", "BBG000BBQCY0", "007903107", "US0079031078"),
            ("INTC", "BBG000C12848", "458140100", "US4581401001"),
            ("JPM", "BBG000GZQJ27", "46625H100", "US46625H1005"),
        ];

        let mut ticker_to_ids = HashMap::new();
        let mut figi_to_ids = HashMap::new();
        let mut cusip_to_ids = HashMap::new();
        let mut isin_to_ids = HashMap::new();

        for (ticker, figi, cusip, isin) in entries {
            let ids = SecurityIdentifiers {
                ticker: Some(ticker.to_string()),
                figi: Some(figi.to_string()),
                cusip: Some(cusip.to_string()),
                isin: Some(isin.to_string()),
            };

            ticker_to_ids.insert(ticker.to_uppercase(), ids.clone());
            figi_to_ids.insert(figi.to_uppercase(), ids.clone());
            cusip_to_ids.insert(cusip.to_uppercase(), ids.clone());
            isin_to_ids.insert(isin.to_uppercase(), ids.clone());
        }

        Self {
            ticker_to_ids,
            figi_to_ids,
            cusip_to_ids,
            isin_to_ids,
        }
    }

    /// Infer security identifier type from format patterns.
    pub fn infer_identifier_type(raw: &str) -> &'static str {
        let trimmed = raw.trim().to_uppercase();
        let len = trimmed.len();

        if len == 12 && trimmed.starts_with("BBG") {
            "figi"
        } else if len == 12
            && trimmed.chars().take(2).all(|c| c.is_ascii_alphabetic())
            && trimmed.chars().all(|c| c.is_ascii_alphanumeric())
        {
            "isin"
        } else if len == 9 && trimmed.chars().all(|c| c.is_ascii_alphanumeric()) {
            "cusip"
        } else {
            "ticker"
        }
    }

    /// Resolve an identifier against the in-memory multi-key index.
    pub fn lookup(
        &self,
        identifier: &str,
        input_type: Option<&str>,
    ) -> Option<(SecurityIdentifiers, &'static str)> {
        let trimmed = identifier.trim().to_uppercase();
        if trimmed.is_empty() {
            return None;
        }

        let type_mode = input_type
            .map(|s| s.trim().to_lowercase())
            .unwrap_or_else(|| "auto".to_string());

        let target_type = if type_mode == "auto" {
            Self::infer_identifier_type(&trimmed)
        } else {
            match type_mode.as_str() {
                "ticker" => "ticker",
                "figi" => "figi",
                "cusip" => "cusip",
                "isin" => "isin",
                _ => return None,
            }
        };

        // Primary lookup based on resolved type
        let primary_result = match target_type {
            "ticker" => self.ticker_to_ids.get(&trimmed).cloned(),
            "figi" => self.figi_to_ids.get(&trimmed).cloned(),
            "cusip" => self.cusip_to_ids.get(&trimmed).cloned(),
            "isin" => self.isin_to_ids.get(&trimmed).cloned(),
            _ => None,
        };

        if let Some(res) = primary_result {
            return Some((res, target_type));
        }

        // If auto mode, try remaining indices as fallback
        if type_mode == "auto" {
            if let Some(res) = self.ticker_to_ids.get(&trimmed).cloned() {
                return Some((res, "ticker"));
            }
            if let Some(res) = self.isin_to_ids.get(&trimmed).cloned() {
                return Some((res, "isin"));
            }
            if let Some(res) = self.figi_to_ids.get(&trimmed).cloned() {
                return Some((res, "figi"));
            }
            if let Some(res) = self.cusip_to_ids.get(&trimmed).cloned() {
                return Some((res, "cusip"));
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_inference() {
        assert_eq!(SymbolMap::infer_identifier_type("AAPL"), "ticker");
        assert_eq!(SymbolMap::infer_identifier_type("BBG000B9XRY4"), "figi");
        assert_eq!(SymbolMap::infer_identifier_type("037833100"), "cusip");
        assert_eq!(SymbolMap::infer_identifier_type("US0378331005"), "isin");
    }

    #[test]
    fn test_lookup_auto_mode() {
        let map = SymbolMap::fallback_default();

        let (res_aapl, type_aapl) = map.lookup("AAPL", Some("auto")).unwrap();
        assert_eq!(type_aapl, "ticker");
        assert_eq!(res_aapl.figi.as_deref(), Some("BBG000B9XRY4"));
        assert_eq!(res_aapl.cusip.as_deref(), Some("037833100"));
        assert_eq!(res_aapl.isin.as_deref(), Some("US0378331005"));

        let (res_isin, type_isin) = map.lookup("US5949181045", None).unwrap();
        assert_eq!(type_isin, "isin");
        assert_eq!(res_isin.ticker.as_deref(), Some("MSFT"));

        let (res_figi, type_figi) = map.lookup("BBG000BBJQV0", Some("auto")).unwrap();
        assert_eq!(type_figi, "figi");
        assert_eq!(res_figi.ticker.as_deref(), Some("NVDA"));
    }

    #[test]
    fn test_lookup_explicit_mode() {
        let map = SymbolMap::fallback_default();

        let (res, t) = map.lookup("037833100", Some("cusip")).unwrap();
        assert_eq!(t, "cusip");
        assert_eq!(res.ticker.as_deref(), Some("AAPL"));

        assert!(map.lookup("AAPL", Some("isin")).is_none());
    }
}
