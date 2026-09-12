//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Finnhub Real-Time WebSocket Streaming Client
//! ═══════════════════════════════════════════════════════════════════════════════
//!
//! Provides ultra-low-latency real-time news streaming from Finnhub via WebSocket
//! (`wss://ws.finnhub.io?token=<KEY>`), reducing article ingestion lag from
//! 300s REST polling to sub-second socket streaming.
//! ═══════════════════════════════════════════════════════════════════════════════

use crate::pipeline::RawDocument;
use chrono::{DateTime, Utc};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::env;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::sync::watch;
use tokio::time::sleep;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Finnhub WebSocket configuration parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinnhubWsConfig {
    pub ws_url: String,
    pub api_key: String,
    pub tickers: Vec<String>,
    pub reconnect_base_delay_ms: u64,
    pub reconnect_max_delay_ms: u64,
    pub mock_mode: bool,
}

impl Default for FinnhubWsConfig {
    fn default() -> Self {
        let ws_url =
            env::var("FINNHUB_WS_URL").unwrap_or_else(|_| "wss://ws.finnhub.io".to_string());
        let api_key = env::var("FINNHUB_API_KEY").unwrap_or_default();
        let tickers_str = env::var("FINNHUB_TRACKED_TICKERS")
            .unwrap_or_else(|_| "AAPL,MSFT,NVDA,GOOGL,AMZN,META,TSLA,SPY".to_string());
        let tickers = tickers_str
            .split(',')
            .map(|s| s.trim().to_uppercase())
            .filter(|s| !s.is_empty())
            .collect();
        let reconnect_base_delay_ms = env::var("FINNHUB_WS_RECONNECT_BASE_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(1000);
        let reconnect_max_delay_ms = env::var("FINNHUB_WS_RECONNECT_MAX_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(60000);
        let mock_mode = if crate::is_production_mode() {
            false
        } else {
            env::var("FINNHUB_MOCK_MODE").as_deref() == Ok("1")
        };

        Self {
            ws_url,
            api_key,
            tickers,
            reconnect_base_delay_ms,
            reconnect_max_delay_ms,
            mock_mode,
        }
    }
}

/// Raw news article model inside incoming Finnhub WebSocket messages.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FinnhubWsNewsItem {
    #[serde(default)]
    pub id: Option<u64>,
    #[serde(default)]
    pub category: Option<String>,
    pub datetime: i64,
    pub headline: String,
    #[serde(default)]
    pub image: Option<String>,
    #[serde(default)]
    pub related: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
}

/// Deserialization envelope for incoming Finnhub WebSocket frames.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinnhubWsMessage {
    #[serde(default, rename = "type")]
    pub msg_type: String,
    #[serde(default)]
    pub data: Option<Vec<FinnhubWsNewsItem>>,
}

impl FinnhubWsMessage {
    /// Converts incoming news items to internal `RawDocument` structs.
    pub fn into_raw_documents(self) -> Vec<RawDocument> {
        let items = match self.data {
            Some(d) => d,
            None => return Vec::new(),
        };

        let now_iso = Utc::now().to_rfc3339();

        items
            .into_iter()
            .filter(|item| !item.headline.trim().is_empty())
            .map(|item| {
                let published_utc = DateTime::<Utc>::from_timestamp(item.datetime, 0)
                    .map(|dt| dt.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string())
                    .unwrap_or_else(|| now_iso.clone());

                let doc_id = if let Some(id_num) = item.id {
                    format!("finnhub-ws-{}", id_num)
                } else {
                    format!("finnhub-ws-{}", Uuid::new_v4())
                };

                let source_name = item
                    .source
                    .filter(|s| !s.trim().is_empty())
                    .map(|s| format!("Finnhub-{}", s.trim()))
                    .unwrap_or_else(|| "Finnhub-WebSocket".to_string());

                let raw_content = item.summary.unwrap_or_default();
                let url = item.url.unwrap_or_default();

                RawDocument {
                    id: doc_id,
                    title: item.headline.trim().to_string(),
                    source: source_name,
                    url,
                    published_utc,
                    raw_content,
                    audio_path: None,
                    ingested_utc: Utc::now().to_rfc3339(),
                    ..Default::default()
                }
            })
            .collect()
    }
}

/// Finnhub WebSocket streaming client with auto-reconnection.
pub struct FinnhubWsClient {
    config: FinnhubWsConfig,
}

impl FinnhubWsClient {
    pub fn new(config: FinnhubWsConfig) -> Self {
        Self { config }
    }

    pub fn from_env() -> Result<Self, String> {
        let config = FinnhubWsConfig::default();
        if crate::is_production_mode() {
            if config.api_key.trim().is_empty()
                || config.api_key == "mock_key"
                || config.api_key.starts_with("mock")
            {
                return Err("Real Finnhub API key is required in production mode.".to_string());
            }
        } else if config.api_key.trim().is_empty() && !config.mock_mode {
            return Err("FINNHUB_API_KEY environment variable is missing".to_string());
        }
        Ok(Self::new(config))
    }

    pub fn config(&self) -> &FinnhubWsConfig {
        &self.config
    }

    /// Runs the continuous WebSocket streaming loop, pumping parsed `RawDocument`s to `tx_raw`.
    pub async fn run_stream_loop(
        &self,
        tx_raw: mpsc::Sender<RawDocument>,
        mut shutdown_rx: watch::Receiver<bool>,
    ) {
        if self.config.mock_mode && !crate::is_production_mode() {
            info!("[Finnhub WS] Starting mock news streaming generator");
            self.run_mock_loop(tx_raw, shutdown_rx).await;
            return;
        } else if self.config.mock_mode && crate::is_production_mode() {
            warn!("[Finnhub WS] Mock mode is disabled in production mode.");
        }

        let mut attempt = 0usize;

        while !*shutdown_rx.borrow() {
            attempt += 1;
            let connect_url = format!(
                "{}?token={}",
                self.config.ws_url.trim_end_matches('/'),
                self.config.api_key
            );

            info!(
                "[Finnhub WS] Attempting connection to '{}' (attempt #{})",
                self.config.ws_url, attempt
            );

            match connect_async(&connect_url).await {
                Ok((mut ws_stream, response)) => {
                    info!(
                        "[Finnhub WS] Connected successfully! HTTP Status: {}",
                        response.status()
                    );
                    attempt = 0; // Reset backoff on successful connect

                    // Send news subscription message
                    let sub_news = serde_json::json!({
                        "type": "subscribe-news"
                    });
                    if let Err(e) = ws_stream.send(Message::Text(sub_news.to_string())).await {
                        warn!("[Finnhub WS] Failed to send news subscription: {}", e);
                    }

                    // Subscribe to tracked ticker symbols
                    for ticker in &self.config.tickers {
                        let sub_ticker = serde_json::json!({
                            "type": "subscribe",
                            "symbol": ticker
                        });
                        let _ = ws_stream.send(Message::Text(sub_ticker.to_string())).await;
                    }

                    info!(
                        "[Finnhub WS] Subscribed to news and {} tickers: {:?}",
                        self.config.tickers.len(),
                        self.config.tickers
                    );

                    // Read frames
                    while !*shutdown_rx.borrow() {
                        tokio::select! {
                            msg_opt = ws_stream.next() => {
                                match msg_opt {
                                    Some(Ok(Message::Text(text))) => {
                                        debug!("[Finnhub WS] Received frame: {} bytes", text.len());
                                        if let Ok(ws_msg) = serde_json::from_str::<FinnhubWsMessage>(&text) {
                                            for raw_doc in ws_msg.into_raw_documents() {
                                                info!("[Finnhub WS] Streamed news: '{}' ({})", raw_doc.title, raw_doc.source);
                                                let _ = tx_raw.send(raw_doc).await;
                                            }
                                        }
                                    }
                                    Some(Ok(Message::Ping(payload))) => {
                                        let _ = ws_stream.send(Message::Pong(payload)).await;
                                    }
                                    Some(Ok(Message::Close(reason))) => {
                                        warn!("[Finnhub WS] Server sent close frame: {:?}", reason);
                                        break;
                                    }
                                    Some(Err(e)) => {
                                        warn!("[Finnhub WS] Socket read error: {}", e);
                                        break;
                                    }
                                    None => {
                                        warn!("[Finnhub WS] Stream ended by remote host");
                                        break;
                                    }
                                    _ => {}
                                }
                            }
                            _ = shutdown_rx.changed() => {
                                info!("[Finnhub WS] Shutdown signal received, closing socket.");
                                let _ = ws_stream.send(Message::Close(None)).await;
                                return;
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!(
                        "[Finnhub WS] Connection failed: {}. Retrying with exponential backoff...",
                        e
                    );
                }
            }

            // Exponential backoff
            let delay_factor = 1u64.checked_shl(attempt.min(6) as u32).unwrap_or(64);
            let delay_ms = self
                .config
                .reconnect_base_delay_ms
                .saturating_mul(delay_factor)
                .min(self.config.reconnect_max_delay_ms);

            debug!("[Finnhub WS] Sleeping {}ms before reconnect", delay_ms);
            sleep(Duration::from_millis(delay_ms)).await;
        }

        info!("[Finnhub WS] Streaming loop terminated cleanly.");
    }

    /// Generates periodic mock news articles for testing without live network connections.
    async fn run_mock_loop(
        &self,
        tx_raw: mpsc::Sender<RawDocument>,
        mut shutdown_rx: watch::Receiver<bool>,
    ) {
        let mock_headlines = [
            ("AAPL", "Apple Announces Next-Gen Quantum Neural Engine with Ultra-High Efficiency"),
            ("NVDA", "Nvidia Reports Record Blackwell Data Center GPU Shipments and Surging AI Demand"),
            ("MSFT", "Microsoft Expands Azure Copilot Enterprise Footprint Across Global Banking Sector"),
            ("TSLA", "Tesla Robotaxi Network Demonstrates Zero-Intervention Autonomous Mileage Milestone"),
        ];
        let mut idx = 0;

        while !*shutdown_rx.borrow() {
            tokio::select! {
                _ = sleep(Duration::from_secs(5)) => {
                    let (ticker, headline) = mock_headlines[idx % mock_headlines.len()];
                    idx += 1;

                    let raw_doc = RawDocument {
                        id: format!("finnhub-ws-mock-{}", Uuid::new_v4()),
                        title: headline.to_string(),
                        source: "Finnhub-WebSocket-Mock".to_string(),
                        url: format!("https://finnhub.io/mock/{}", ticker.to_lowercase()),
                        published_utc: Utc::now().to_rfc3339(),
                        raw_content: format!("{} corporate breaking disclosure updates for institutional investors.", ticker),
                        audio_path: None,
                        ingested_utc: Utc::now().to_rfc3339(),
                        ..Default::default()
                    };

                    info!("[Finnhub WS Mock] Emitted real-time streaming article for ${}", ticker);
                    let _ = tx_raw.send(raw_doc).await;
                }
                _ = shutdown_rx.changed() => {
                    info!("[Finnhub WS Mock] Terminating mock news stream.");
                    return;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_finnhub_ws_news_json_parsing() {
        let sample_json = r#"{
            "data": [
                {
                    "category": "company news",
                    "datetime": 1787668200,
                    "headline": "Apple Reports Record Quarterly Revenue and AI Growth",
                    "id": 998877,
                    "image": "https://img.finnhub.io/sample.jpg",
                    "related": "AAPL",
                    "source": "Reuters",
                    "summary": "Apple Inc. announced financial results for its fiscal 2026 fourth quarter.",
                    "url": "https://reuters.com/apple-q4-results"
                }
            ],
            "type": "news"
        }"#;

        let ws_msg: FinnhubWsMessage =
            serde_json::from_str(sample_json).expect("Failed to deserialize Finnhub WS message");
        assert_eq!(ws_msg.msg_type, "news");
        assert_eq!(ws_msg.data.as_ref().unwrap().len(), 1);

        let docs = ws_msg.into_raw_documents();
        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].id, "finnhub-ws-998877");
        assert_eq!(
            docs[0].title,
            "Apple Reports Record Quarterly Revenue and AI Growth"
        );
        assert_eq!(docs[0].source, "Finnhub-Reuters");
        assert_eq!(docs[0].url, "https://reuters.com/apple-q4-results");
        assert!(!docs[0].ingested_utc.is_empty());
    }

    #[test]
    fn test_finnhub_ws_empty_message_handling() {
        let sample_json = r#"{"type":"ping"}"#;
        let ws_msg: FinnhubWsMessage = serde_json::from_str(sample_json).unwrap();
        assert_eq!(ws_msg.msg_type, "ping");
        let docs = ws_msg.into_raw_documents();
        assert!(docs.is_empty());
    }
}
