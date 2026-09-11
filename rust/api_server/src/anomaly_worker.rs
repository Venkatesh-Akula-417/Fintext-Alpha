//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Real-Time Sentiment Anomaly Detection & WebSocket Broadcaster
//! ═══════════════════════════════════════════════════════════════════════════════
//!
//! Spawns background worker task periodically computing statistical sentiment z-score
//! deviations across tracked equity tickers and fanning out real-time alerts to
//! connected WebSocket subscriber sessions.
//! ═══════════════════════════════════════════════════════════════════════════════

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::env;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::time::interval;
use tracing::{debug, info};

use crate::handlers::sentiment_anomalies::{compute_anomalies, generate_mock_anomaly_data};
use crate::models::SentimentAnomalyAlert;
use crate::state::AppState;
use crate::storage::{QuestDbClient, QuestDbClientConfig};

pub const DEFAULT_ANOMALY_SCAN_INTERVAL_SECS: u64 = 60;
pub const DEFAULT_ANOMALY_BROADCAST_CAPACITY: usize = 1024;
pub const DEFAULT_ANOMALY_LOOKBACK_DAYS: u32 = 30;
pub const DEFAULT_ANOMALY_ZSCORE_THRESHOLD: f64 = 2.0;
pub const DEFAULT_ANOMALY_MIN_RECORDS: usize = 20;
pub const DEFAULT_ANOMALY_LIMIT: usize = 20;

/// Configuration options for the anomaly background detector and broadcaster.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyWorkerConfig {
    pub scan_interval_secs: u64,
    pub channel_capacity: usize,
    pub lookback_days: u32,
    pub zscore_threshold: f64,
    pub min_records: usize,
    pub limit: usize,
    pub enabled: bool,
}

impl Default for AnomalyWorkerConfig {
    fn default() -> Self {
        let scan_interval_secs = env::var("ANOMALY_SCAN_INTERVAL_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(DEFAULT_ANOMALY_SCAN_INTERVAL_SECS);

        let channel_capacity = env::var("ANOMALY_BROADCAST_CAPACITY")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(DEFAULT_ANOMALY_BROADCAST_CAPACITY);

        let lookback_days = env::var("ANOMALY_LOOKBACK_DAYS")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(DEFAULT_ANOMALY_LOOKBACK_DAYS);

        let zscore_threshold = env::var("ANOMALY_ZSCORE_THRESHOLD")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(DEFAULT_ANOMALY_ZSCORE_THRESHOLD);

        let min_records = env::var("ANOMALY_MIN_RECORDS")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(DEFAULT_ANOMALY_MIN_RECORDS);

        let limit = env::var("ANOMALY_LIMIT")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(DEFAULT_ANOMALY_LIMIT);

        let enabled = env::var("ANOMALY_WORKER_ENABLED")
            .map(|v| v != "0" && v.to_lowercase() != "false")
            .unwrap_or(true);

        Self {
            scan_interval_secs,
            channel_capacity,
            lookback_days,
            zscore_threshold,
            min_records,
            limit,
            enabled,
        }
    }
}

/// Thread-safe anomaly fan-out broadcaster to WebSocket clients.
#[derive(Clone)]
pub struct AnomalyBroadcaster {
    tx: broadcast::Sender<SentimentAnomalyAlert>,
}

impl AnomalyBroadcaster {
    /// Creates a new `AnomalyBroadcaster` with the given capacity.
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self { tx }
    }

    /// Obtains a new receiver subscription for a WebSocket client.
    pub fn subscribe(&self) -> broadcast::Receiver<SentimentAnomalyAlert> {
        self.tx.subscribe()
    }

    /// Broadcasts a single anomaly alert frame.
    pub fn broadcast(&self, alert: SentimentAnomalyAlert) -> usize {
        self.tx.send(alert).unwrap_or(0)
    }

    /// Broadcasts multiple anomaly alert frames in batch.
    pub fn broadcast_batch(&self, alerts: &[SentimentAnomalyAlert]) -> usize {
        let mut sent = 0;
        for alert in alerts {
            if self.tx.send(alert.clone()).is_ok() {
                sent += 1;
            }
        }
        sent
    }

    /// Returns the number of active subscriber receivers.
    pub fn receiver_count(&self) -> usize {
        self.tx.receiver_count()
    }
}

impl Default for AnomalyBroadcaster {
    fn default() -> Self {
        Self::new(DEFAULT_ANOMALY_BROADCAST_CAPACITY)
    }
}

/// Default ticker universe scanned for statistical sentiment anomalies.
pub fn get_default_scan_tickers() -> Vec<String> {
    vec![
        "AAPL".to_string(),
        "NVDA".to_string(),
        "MSFT".to_string(),
        "AMZN".to_string(),
        "GOOGL".to_string(),
        "META".to_string(),
        "TSLA".to_string(),
        "JPM".to_string(),
        "GS".to_string(),
        "JNJ".to_string(),
        "PFE".to_string(),
        "XOM".to_string(),
        "CVX".to_string(),
    ]
}

/// Scans the target equity universe for statistical sentiment anomalies and broadcasts alerts.
pub async fn scan_and_broadcast_anomalies(state: &AppState) -> Vec<SentimentAnomalyAlert> {
    let now = Utc::now();
    let lookback_days = DEFAULT_ANOMALY_LOOKBACK_DAYS;
    let zscore_threshold = DEFAULT_ANOMALY_ZSCORE_THRESHOLD;
    let min_records = DEFAULT_ANOMALY_MIN_RECORDS;
    let limit = DEFAULT_ANOMALY_LIMIT;

    let target_tickers = get_default_scan_tickers();
    let start_dt = now - chrono::Duration::days(lookback_days as i64);
    let start_iso = start_dt.to_rfc3339();

    // 1. Try querying QuestDB if available, otherwise generate mock anomaly data
    let force_mock = crate::state::is_questdb_mock_fallback_enabled();
    let mut anomaly_items = Vec::new();

    if !force_mock {
        let questdb = QuestDbClient::new(QuestDbClientConfig::default());
        if let Ok(sql) =
            QuestDbClient::build_sentiment_anomalies_query(Some(&target_tickers), &start_iso)
        {
            let endpoint = format!("{}/exec", questdb.config().url.trim_end_matches('/'));
            let client = reqwest::Client::builder()
                .timeout(Duration::from_millis(questdb.config().timeout_ms))
                .build()
                .unwrap_or_default();

            if let Ok(resp) = client.get(&endpoint).query(&[("query", &sql)]).send().await {
                if resp.status().is_success() {
                    if let Ok(val) = resp.json::<serde_json::Value>().await {
                        if let Ok(records) =
                            QuestDbClient::parse_sentiment_anomalies_exec_response(&val)
                        {
                            let (items, _, _) =
                                compute_anomalies(records, min_records, zscore_threshold, limit);
                            anomaly_items = items;
                        }
                    }
                }
            }
        }
    }

    if anomaly_items.is_empty() && !crate::state::is_production_mode() {
        let mock_records = generate_mock_anomaly_data(&target_tickers, lookback_days, now);
        let (items, _, _) = compute_anomalies(mock_records, min_records, zscore_threshold, limit);
        anomaly_items = items;
    }

    // 2. Convert to SentimentAnomalyAlert frames
    let alerts: Vec<SentimentAnomalyAlert> = anomaly_items
        .iter()
        .map(SentimentAnomalyAlert::from_anomaly_item)
        .collect();

    // 3. Broadcast to all active WebSocket clients subscribed to anomalies
    if !alerts.is_empty() {
        let broadcasted = state.anomaly_broadcaster.broadcast_batch(&alerts);
        info!(
            "[Anomaly Worker] Scanned {} tickers: detected {} anomalies, pushed to {} active WebSocket receivers",
            target_tickers.len(),
            alerts.len(),
            state.anomaly_broadcaster.receiver_count()
        );
        debug!("[Anomaly Worker] Broadcast count: {}", broadcasted);
    }

    alerts
}

/// Spawns the background anomaly detection worker task.
pub fn spawn_anomaly_detection_worker(
    state: AppState,
    config: AnomalyWorkerConfig,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        if !config.enabled {
            info!("[Anomaly Worker] Background anomaly detection disabled via configuration.");
            return;
        }

        info!(
            "[Anomaly Worker] Starting background anomaly detection worker (Interval: {}s, Threshold: |z| >= {:.1})",
            config.scan_interval_secs, config.zscore_threshold
        );

        let mut timer = interval(Duration::from_secs(config.scan_interval_secs.max(1)));
        // Skip immediate first tick to allow server startup and initialization
        timer.tick().await;

        loop {
            timer.tick().await;
            debug!("[Anomaly Worker] Initiating scheduled sentiment anomaly scan...");
            let alerts = scan_and_broadcast_anomalies(&state).await;
            debug!(
                "[Anomaly Worker] Completed scheduled scan: {} alerts generated",
                alerts.len()
            );
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_anomaly_broadcaster_subscribe_and_broadcast() {
        let broadcaster = AnomalyBroadcaster::new(32);
        let mut rx1 = broadcaster.subscribe();
        let mut rx2 = broadcaster.subscribe();

        assert_eq!(broadcaster.receiver_count(), 2);

        let alert = SentimentAnomalyAlert {
            event_type: "sentiment_anomaly".to_string(),
            ticker: "AAPL".to_string(),
            latest_score: 0.85,
            mean_score: 0.12,
            stddev: 0.21,
            zscore: 3.48,
            direction: "bullish".to_string(),
            timestamp: "2026-09-01T12:00:00Z".to_string(),
        };

        let sent = broadcaster.broadcast(alert.clone());
        assert_eq!(sent, 2);

        let msg1 = rx1.recv().await.unwrap();
        assert_eq!(msg1.ticker, "AAPL");
        assert_eq!(msg1.direction, "bullish");
        assert_eq!(msg1.event_type, "sentiment_anomaly");

        let msg2 = rx2.recv().await.unwrap();
        assert_eq!(msg2.ticker, "AAPL");
    }

    #[tokio::test]
    async fn test_anomaly_scan_and_broadcast() {
        let state = AppState::default();
        let mut rx = state.anomaly_broadcaster.subscribe();

        let alerts = scan_and_broadcast_anomalies(&state).await;
        assert!(
            !alerts.is_empty(),
            "Should generate mock anomalies in test mode"
        );

        // First alert should have been received by subscriber
        let received = rx.recv().await.unwrap();
        assert_eq!(received.event_type, "sentiment_anomaly");
        assert!(received.zscore.abs() >= 2.0);
    }
}
