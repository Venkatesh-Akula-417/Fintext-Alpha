//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Real-Time Sentiment Anomaly Models
//! ═══════════════════════════════════════════════════════════════════════════════

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::models::sentiment::SentimentAnomalyItem;

/// Real-time sentiment anomaly alert frame pushed over WebSocket.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct SentimentAnomalyAlert {
    /// Event type identifier
    #[serde(rename = "type")]
    #[schema(example = "sentiment_anomaly")]
    pub event_type: String,

    /// Target stock ticker symbol
    #[schema(example = "AAPL")]
    pub ticker: String,

    /// Most recent observed sentiment score (-1.0 to +1.0)
    #[schema(example = 0.85)]
    pub latest_score: f64,

    /// Historical rolling sample mean sentiment score
    #[schema(example = 0.12)]
    pub mean_score: f64,

    /// Historical rolling sample standard deviation
    #[schema(example = 0.21)]
    pub stddev: f64,

    /// Statistical z-score deviation
    #[schema(example = 3.48)]
    pub zscore: f64,

    /// Anomaly movement direction ("bullish" or "bearish")
    #[schema(example = "bullish")]
    pub direction: String,

    /// ISO 8601 UTC timestamp of latest observation
    #[schema(example = "2025-08-31T12:00:00Z")]
    pub timestamp: String,
}

impl SentimentAnomalyAlert {
    /// Constructs a `SentimentAnomalyAlert` from a detected `SentimentAnomalyItem`.
    pub fn from_anomaly_item(item: &SentimentAnomalyItem) -> Self {
        Self {
            event_type: "sentiment_anomaly".to_string(),
            ticker: item.ticker.clone(),
            latest_score: item.latest_score,
            mean_score: item.mean_score,
            stddev: item.stddev,
            zscore: item.zscore,
            direction: item.direction.clone(),
            timestamp: item.latest_timestamp.clone(),
        }
    }
}

/// Response envelope for manual anomaly scan trigger.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct AnomalyScanResponse {
    /// Total number of anomalies detected in this scan
    #[schema(example = 4)]
    pub anomalies_found: usize,

    /// Total number of alert messages broadcast to active WebSocket subscribers
    #[schema(example = 4)]
    pub alerts_broadcasted: usize,

    /// List of detected anomaly alerts
    pub anomalies: Vec<SentimentAnomalyAlert>,

    /// ISO 8601 UTC timestamp when scan completed
    #[schema(example = "2026-09-01T12:00:00Z")]
    pub scanned_at: String,

    /// Diagnostic status message
    #[schema(example = "Anomaly scan completed: 4 anomalies broadcast to subscribers")]
    pub message: String,
}
