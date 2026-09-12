//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Polygon.io Real-Time Options WebSocket Streamer
//! ═══════════════════════════════════════════════════════════════════════════════
//!
//! Provides ultra-low-latency streaming of options trades and aggregates from
//! Polygon.io (`wss://socket.polygon.io/options`) directly into the VPIN
//! and Dealer Gamma Exposure (GEX) microstructure engines.
//! ═══════════════════════════════════════════════════════════════════════════════

use crate::sources::polygon::{parse_options_ticker, OptionTrade};
use chrono::Utc;
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

/// Polygon Options WebSocket configuration parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolygonWsConfig {
    pub ws_url: String,
    pub api_key: String,
    pub subscriptions: Vec<String>,
    pub reconnect_base_delay_ms: u64,
    pub reconnect_max_delay_ms: u64,
    pub mock_mode: bool,
}

impl Default for PolygonWsConfig {
    fn default() -> Self {
        let ws_url = env::var("POLYGON_WS_URL")
            .unwrap_or_else(|_| "wss://socket.polygon.io/options".to_string());
        let api_key = env::var("POLYGON_API_KEY").unwrap_or_default();
        let subs_str = env::var("POLYGON_WS_SUBSCRIPTIONS").unwrap_or_else(|_| "T.*".to_string());
        let subscriptions = subs_str
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let reconnect_base_delay_ms = env::var("POLYGON_WS_RECONNECT_BASE_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(1000);
        let reconnect_max_delay_ms = env::var("POLYGON_WS_RECONNECT_MAX_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(60000);
        let mock_mode = if crate::is_production_mode() {
            false
        } else {
            env::var("POLYGON_MOCK_MODE").as_deref() == Ok("1")
        };

        Self {
            ws_url,
            api_key,
            subscriptions,
            reconnect_base_delay_ms,
            reconnect_max_delay_ms,
            mock_mode,
        }
    }
}

/// Raw trade item payload received from Polygon options WebSocket feed.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PolygonWsRawTrade {
    #[serde(default)]
    pub ev: String,
    #[serde(default)]
    pub sym: String,
    #[serde(default)]
    pub x: Option<i32>,
    #[serde(default)]
    pub p: f64,
    #[serde(default)]
    pub s: u64,
    #[serde(default)]
    pub c: Vec<i32>,
    #[serde(default)]
    pub t: i64, // nanoseconds since Unix epoch
}

impl PolygonWsRawTrade {
    /// Converts a raw Polygon trade frame into a validated `OptionTrade` domain model.
    pub fn to_option_trade(&self) -> Result<OptionTrade, String> {
        let (underlying, expiry, option_type, strike) = parse_options_ticker(&self.sym)?;
        let timestamp_us = if self.t > 1_000_000_000_000_000 {
            self.t / 1_000 // Convert SIP nanoseconds to microseconds
        } else if self.t > 1_000_000_000_000 {
            self.t * 1_000 // Convert milliseconds to microseconds
        } else {
            Utc::now().timestamp_micros()
        };

        Ok(OptionTrade {
            options_ticker: self.sym.clone(),
            underlying,
            expiry,
            option_type,
            strike,
            price: self.p,
            size: self.s,
            timestamp_us,
            exchange: self.x,
            conditions: self.c.clone(),
        })
    }
}

/// Generic wrapper for Polygon WebSocket messages (status, trades, aggregates).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PolygonWsMessage {
    Status {
        ev: String,
        status: String,
        message: Option<String>,
    },
    Trades(Vec<PolygonWsRawTrade>),
    Other(serde_json::Value),
}

/// Polygon Options WebSocket streaming client with automatic authentication and reconnection.
pub struct PolygonWsClient {
    config: PolygonWsConfig,
}

impl PolygonWsClient {
    pub fn new(config: PolygonWsConfig) -> Self {
        Self { config }
    }

    pub fn from_env() -> Result<Self, String> {
        let config = PolygonWsConfig::default();
        if crate::is_production_mode() {
            if config.api_key.trim().is_empty()
                || config.api_key == "mock_key"
                || config.api_key.starts_with("mock")
            {
                return Err("Real Polygon.io API key is required in production mode.".to_string());
            }
        } else if config.api_key.trim().is_empty() && !config.mock_mode {
            return Err("POLYGON_API_KEY environment variable is missing".to_string());
        }
        Ok(Self::new(config))
    }

    pub fn config(&self) -> &PolygonWsConfig {
        &self.config
    }

    /// Runs the continuous options trade streaming loop.
    pub async fn run_stream_loop(
        &self,
        tx_trade: mpsc::Sender<OptionTrade>,
        mut shutdown_rx: watch::Receiver<bool>,
    ) {
        if self.config.mock_mode && !crate::is_production_mode() {
            info!("[Polygon WS] Starting mock options trade streaming generator");
            self.run_mock_loop(tx_trade, shutdown_rx).await;
            return;
        } else if self.config.mock_mode && crate::is_production_mode() {
            warn!("[Polygon WS] Mock mode is disabled in production mode.");
        }

        let mut attempt = 0usize;

        while !*shutdown_rx.borrow() {
            attempt += 1;
            info!(
                "[Polygon WS] Attempting connection to '{}' (attempt #{})",
                self.config.ws_url, attempt
            );

            match connect_async(&self.config.ws_url).await {
                Ok((mut ws_stream, response)) => {
                    info!(
                        "[Polygon WS] Connected successfully! HTTP Status: {}",
                        response.status()
                    );
                    attempt = 0; // Reset backoff

                    // Authenticate with Polygon API Key
                    let auth_payload = serde_json::json!({
                        "action": "auth",
                        "params": self.config.api_key
                    });
                    if let Err(e) = ws_stream
                        .send(Message::Text(auth_payload.to_string()))
                        .await
                    {
                        warn!("[Polygon WS] Failed to send authentication frame: {}", e);
                    }

                    // Subscribe to configured channels (e.g. "T.*" or "T.O:AAPL*")
                    let sub_param = self.config.subscriptions.join(",");
                    let sub_payload = serde_json::json!({
                        "action": "subscribe",
                        "params": sub_param
                    });
                    if let Err(e) = ws_stream.send(Message::Text(sub_payload.to_string())).await {
                        warn!("[Polygon WS] Failed to send subscription frame: {}", e);
                    } else {
                        info!(
                            "[Polygon WS] Subscribed to options channels: '{}'",
                            sub_param
                        );
                    }

                    // Read frames
                    while !*shutdown_rx.borrow() {
                        tokio::select! {
                            msg_opt = ws_stream.next() => {
                                match msg_opt {
                                    Some(Ok(Message::Text(text))) => {
                                        debug!("[Polygon WS] Received frame: {} bytes", text.len());
                                        if let Ok(trades) = serde_json::from_str::<Vec<PolygonWsRawTrade>>(&text) {
                                            for raw_trade in trades {
                                                if raw_trade.ev == "T" {
                                                    match raw_trade.to_option_trade() {
                                                        Ok(opt_trade) => {
                                                            debug!(
                                                                "[Polygon WS] Streamed trade: {} (size: {}, price: ${:.2})",
                                                                opt_trade.options_ticker, opt_trade.size, opt_trade.price
                                                            );
                                                            let _ = tx_trade.send(opt_trade).await;
                                                        }
                                                        Err(parse_err) => {
                                                            debug!("[Polygon WS] Skip non-standard symbol '{}': {}", raw_trade.sym, parse_err);
                                                        }
                                                    }
                                                }
                                            }
                                        } else if text.contains("auth_success") {
                                            info!("[Polygon WS] Authentication confirmed by Polygon server.");
                                        }
                                    }
                                    Some(Ok(Message::Ping(payload))) => {
                                        let _ = ws_stream.send(Message::Pong(payload)).await;
                                    }
                                    Some(Ok(Message::Close(reason))) => {
                                        warn!("[Polygon WS] Server sent close frame: {:?}", reason);
                                        break;
                                    }
                                    Some(Err(e)) => {
                                        warn!("[Polygon WS] Socket read error: {}", e);
                                        break;
                                    }
                                    None => {
                                        warn!("[Polygon WS] Stream closed by remote host");
                                        break;
                                    }
                                    _ => {}
                                }
                            }
                            _ = shutdown_rx.changed() => {
                                info!("[Polygon WS] Shutdown signal received, closing socket.");
                                let _ = ws_stream.send(Message::Close(None)).await;
                                return;
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!(
                        "[Polygon WS] Connection failed: {}. Retrying with exponential backoff...",
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

            debug!("[Polygon WS] Sleeping {}ms before reconnect", delay_ms);
            sleep(Duration::from_millis(delay_ms)).await;
        }

        info!("[Polygon WS] Streaming loop terminated cleanly.");
    }

    /// Emits synthetic realistic options trades for testing VPIN and GEX engines.
    async fn run_mock_loop(
        &self,
        tx_trade: mpsc::Sender<OptionTrade>,
        mut shutdown_rx: watch::Receiver<bool>,
    ) {
        let mock_contracts = [
            (
                "AAPL",
                "O:AAPL240119C00150000",
                "2024-01-19",
                "CALL",
                150.0,
                4.50,
                10,
            ),
            (
                "NVDA",
                "O:NVDA240119C00500000",
                "2024-01-19",
                "CALL",
                500.0,
                18.25,
                25,
            ),
            (
                "MSFT",
                "O:MSFT240119P00380000",
                "2024-01-19",
                "PUT",
                380.0,
                6.10,
                15,
            ),
            (
                "SPY",
                "O:SPY240119C00480000",
                "2024-01-19",
                "CALL",
                480.0,
                3.20,
                100,
            ),
        ];
        let mut idx = 0;

        while !*shutdown_rx.borrow() {
            tokio::select! {
                _ = sleep(Duration::from_millis(500)) => {
                    let (underlying, ticker, expiry, opt_type, strike, price, size) = mock_contracts[idx % mock_contracts.len()];
                    idx += 1;

                    let trade = OptionTrade {
                        options_ticker: ticker.to_string(),
                        underlying: underlying.to_string(),
                        expiry: expiry.to_string(),
                        option_type: opt_type.to_string(),
                        strike,
                        price,
                        size,
                        timestamp_us: Utc::now().timestamp_micros(),
                        exchange: Some(302),
                        conditions: vec![200],
                    };

                    let _ = tx_trade.send(trade).await;
                }
                _ = shutdown_rx.changed() => {
                    info!("[Polygon WS Mock] Terminating mock options stream.");
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
    fn test_polygon_ws_options_trade_parsing() {
        let sample_json = r#"[
            {
                "ev": "T",
                "sym": "O:AAPL240119C00150000",
                "x": 302,
                "p": 5.25,
                "s": 20,
                "c": [200],
                "t": 1787668200000000000
            }
        ]"#;

        let raw_trades: Vec<PolygonWsRawTrade> =
            serde_json::from_str(sample_json).expect("Failed to deserialize Polygon trade frame");
        assert_eq!(raw_trades.len(), 1);

        let trade = raw_trades[0]
            .to_option_trade()
            .expect("Failed to convert to OptionTrade");
        assert_eq!(trade.options_ticker, "O:AAPL240119C00150000");
        assert_eq!(trade.underlying, "AAPL");
        assert_eq!(trade.expiry, "2024-01-19");
        assert_eq!(trade.option_type, "CALL");
        assert_eq!(trade.strike, 150.0);
        assert_eq!(trade.price, 5.25);
        assert_eq!(trade.size, 20);
        assert_eq!(trade.exchange, Some(302));
        assert_eq!(trade.conditions, vec![200]);
        assert_eq!(trade.timestamp_us, 1787668200000000);
    }

    #[test]
    fn test_polygon_ws_status_message_json() {
        let status_json = r#"[{"ev":"status","status":"auth_success","message":"authenticated"}]"#;
        let val: serde_json::Value = serde_json::from_str(status_json).unwrap();
        assert_eq!(val[0]["status"], "auth_success");
    }
}
