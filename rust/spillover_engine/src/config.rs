//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Cross-Asset Spillover Engine Configuration
//! ═══════════════════════════════════════════════════════════════════════════════

use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpilloverConfig {
    /// Base URL of the QuestDB instance (HTTP REST endpoint)
    pub questdb_url: String,
    /// Periodic loop interval in seconds (default: 3600s = 1 hour)
    pub interval_secs: u64,
    /// Historical lookback window in days (default: 30 days)
    pub lookback_days: i64,
    /// Maximum lead/lag scan window in hours (default: 3 hours, i.e., [-3, +3])
    pub max_lag_hours: i64,
    /// Minimum absolute correlation coefficient to retain a spillover relationship (default: 0.3)
    pub min_correlation_threshold: f64,
    /// Minimum distinct active hourly buckets required for a ticker to enter analysis (default: 10)
    pub min_active_hours: usize,
    /// Maximum active tickers to analyze simultaneously for strict memory containment < 2GB (default: 500)
    pub max_tickers: usize,
    /// HTTP request timeout in milliseconds
    pub timeout_ms: u64,
    /// Max HTTP retries for QuestDB connections
    pub max_retries: usize,
    /// Mock mode flag for offline testing without a live QuestDB container
    pub mock_mode: bool,
}

impl Default for SpilloverConfig {
    fn default() -> Self {
        Self::from_env()
    }
}

impl SpilloverConfig {
    /// Constructs `SpilloverConfig` populated from environment variables with safe defaults.
    pub fn from_env() -> Self {
        let questdb_url =
            env::var("QUESTDB_URL").unwrap_or_else(|_| "http://127.0.0.1:9000".to_string());

        let interval_secs = env::var("SPILLOVER_INTERVAL_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(3600);

        let lookback_days = env::var("SPILLOVER_LOOKBACK_DAYS")
            .ok()
            .and_then(|v| v.parse::<i64>().ok())
            .unwrap_or(30);

        let max_lag_hours = env::var("SPILLOVER_MAX_LAG_HOURS")
            .ok()
            .and_then(|v| v.parse::<i64>().ok())
            .unwrap_or(3);

        let min_correlation_threshold = env::var("SPILLOVER_CORR_THRESHOLD")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(0.3);

        let min_active_hours = env::var("SPILLOVER_MIN_HOURS")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(10);

        let max_tickers = env::var("SPILLOVER_MAX_TICKERS")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(500);

        let timeout_ms = env::var("SPILLOVER_TIMEOUT_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(5000);

        let max_retries = env::var("SPILLOVER_MAX_RETRIES")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(3);

        let mock_mode = env::var("SPILLOVER_MOCK_MODE").as_deref() == Ok("1")
            || env::var("QUESTDB_MOCK_FALLBACK").as_deref() == Ok("1");

        Self {
            questdb_url,
            interval_secs,
            lookback_days,
            max_lag_hours,
            min_correlation_threshold,
            min_active_hours,
            max_tickers,
            timeout_ms,
            max_retries,
            mock_mode,
        }
    }
}
