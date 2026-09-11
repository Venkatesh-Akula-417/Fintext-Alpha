//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — GICS Sector & Industry Universe Mapping Engine
//! ═══════════════════════════════════════════════════════════════════════════════

use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{info, warn};

/// Global default SectorMap singleton.
pub static GLOBAL_SECTOR_MAP: Lazy<Arc<SectorMap>> = Lazy::new(|| Arc::new(SectorMap::from_env()));

/// In-memory GICS sector mapping index.
#[derive(Debug, Clone, Default)]
pub struct SectorMap {
    /// Normalized lowercase sector name -> list of uppercase stock tickers
    pub sector_to_tickers: HashMap<String, Vec<String>>,
    /// Uppercase ticker -> canonical sector display name
    pub ticker_to_sector: HashMap<String, String>,
    /// Unique canonical sector display names
    pub sectors: Vec<String>,
}

impl SectorMap {
    /// Create an empty sector map.
    pub fn empty() -> Self {
        Self {
            sector_to_tickers: HashMap::new(),
            ticker_to_sector: HashMap::new(),
            sectors: Vec::new(),
        }
    }

    /// Load sector mapping from environment or standard config search paths.
    pub fn from_env() -> Self {
        let candidate_paths = if let Ok(custom_path) = env::var("SECTOR_MAPPING_PATH") {
            vec![PathBuf::from(custom_path)]
        } else {
            vec![
                PathBuf::from("./config/sector_mapping.csv"),
                PathBuf::from("../config/sector_mapping.csv"),
                PathBuf::from("../../config/sector_mapping.csv"),
                PathBuf::from("d:/FinText-Alpha-Vectorizer/config/sector_mapping.csv"),
                PathBuf::from("D:\\FinText-Alpha-Vectorizer\\config\\sector_mapping.csv"),
            ]
        };

        for path in candidate_paths {
            if path.is_file() {
                info!(
                    "[SectorMap] Loading GICS sector mapping from: {}",
                    path.display()
                );
                return Self::load_from_path(&path);
            }
        }

        warn!("[SectorMap] No sector_mapping.csv found. Using default built-in core sectors.");
        Self::fallback_default()
    }

    /// Parse CSV formatted as `TICKER,Sector` into in-memory lookup indices.
    pub fn load_from_path(path: &Path) -> Self {
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                warn!("[SectorMap] Failed to read sector mapping file: {}", e);
                return Self::fallback_default();
            }
        };

        let mut sector_to_tickers: HashMap<String, Vec<String>> = HashMap::new();
        let mut ticker_to_sector: HashMap<String, String> = HashMap::new();
        let mut canonical_sectors: HashMap<String, String> = HashMap::new(); // lowercase -> display

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with("ticker,") {
                continue;
            }

            let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
            if parts.len() >= 2 {
                let ticker = parts[0].to_uppercase();
                let sector_display = parts[1].to_string();
                let sector_key = sector_display.to_lowercase();

                if !ticker.is_empty() && !sector_display.is_empty() {
                    canonical_sectors
                        .entry(sector_key.clone())
                        .or_insert_with(|| sector_display.clone());
                    ticker_to_sector.insert(ticker.clone(), sector_display);
                    sector_to_tickers
                        .entry(sector_key)
                        .or_default()
                        .push(ticker);
                }
            }
        }

        let mut sectors: Vec<String> = canonical_sectors.into_values().collect();
        sectors.sort();

        info!(
            "[SectorMap] Initialized with {} sectors and {} total mapped tickers",
            sectors.len(),
            ticker_to_sector.len()
        );

        Self {
            sector_to_tickers,
            ticker_to_sector,
            sectors,
        }
    }

    /// Hardcoded fallback with standard mega-cap institutional sectors.
    pub fn fallback_default() -> Self {
        let mut sm = Self::empty();
        let default_mappings = vec![
            ("AAPL", "Technology"),
            ("MSFT", "Technology"),
            ("NVDA", "Technology"),
            ("AMZN", "Technology"),
            ("GOOGL", "Technology"),
            ("META", "Technology"),
            ("JPM", "Financials"),
            ("BAC", "Financials"),
            ("GS", "Financials"),
            ("JNJ", "Healthcare"),
            ("UNH", "Healthcare"),
            ("LLY", "Healthcare"),
            ("XOM", "Energy"),
            ("CVX", "Energy"),
            ("TSLA", "Consumer Cyclical"),
            ("BA", "Industrials"),
            ("CAT", "Industrials"),
            ("DIS", "Communication Services"),
            ("NEE", "Utilities"),
            ("PLD", "Real Estate"),
            ("LIN", "Materials"),
        ];

        for (t, s) in default_mappings {
            let ticker = t.to_string();
            let sector_display = s.to_string();
            let sector_key = s.to_lowercase();

            sm.ticker_to_sector
                .insert(ticker.clone(), sector_display.clone());
            sm.sector_to_tickers
                .entry(sector_key)
                .or_default()
                .push(ticker);
            if !sm.sectors.contains(&sector_display) {
                sm.sectors.push(sector_display);
            }
        }
        sm.sectors.sort();
        sm
    }

    /// Look up all tickers belonging to a sector (case-insensitive).
    pub fn get_tickers_by_sector(&self, sector: &str) -> Option<&Vec<String>> {
        let key = sector.trim().to_lowercase();
        self.sector_to_tickers.get(&key)
    }

    /// Retrieve the canonical display name for a sector string.
    pub fn get_canonical_name(&self, sector: &str) -> Option<String> {
        let key = sector.trim().to_lowercase();
        self.sector_to_tickers.get(&key).and_then(|tickers| {
            tickers
                .first()
                .and_then(|t| self.ticker_to_sector.get(t).cloned())
        })
    }

    /// Retrieve the list of all available sector names.
    pub fn list_sectors(&self) -> &[String] {
        &self.sectors
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fallback_sector_map() {
        let sm = SectorMap::fallback_default();
        let tech_tickers = sm.get_tickers_by_sector("Technology").unwrap();
        assert!(tech_tickers.contains(&"AAPL".to_string()));
        assert!(tech_tickers.contains(&"NVDA".to_string()));

        // Case-insensitivity check
        let fin_tickers = sm.get_tickers_by_sector("financials").unwrap();
        assert!(fin_tickers.contains(&"JPM".to_string()));

        assert!(sm.get_tickers_by_sector("NonExistentSector").is_none());
    }
}
