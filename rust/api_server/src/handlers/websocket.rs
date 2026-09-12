//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Real-Time WebSocket Signal & Anomaly Streamer
//! ═══════════════════════════════════════════════════════════════════════════════

use crate::state::AppState;
use axum::extract::ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::sync::broadcast::error::RecvError;
use tracing::{debug, error, info, warn};

/// Negotiation format for WebSocket streaming payload serialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StreamEncoding {
    Json,
    MsgPack,
}

impl StreamEncoding {
    /// Determines the streaming serialization format from query parameters.
    pub fn from_query(format: Option<&str>, encoding: Option<&str>) -> Self {
        let raw = format.or(encoding).unwrap_or("json").trim().to_lowercase();
        match raw.as_str() {
            "msgpack" | "messagepack" | "mp" => StreamEncoding::MsgPack,
            _ => StreamEncoding::Json,
        }
    }

    /// Returns the canonical string representation for telemetry and handshakes.
    pub fn as_str(&self) -> &'static str {
        match self {
            StreamEncoding::Json => "json",
            StreamEncoding::MsgPack => "msgpack",
        }
    }
}

/// Helper to serialize a data structure according to the client's negotiated encoding.
pub fn encode_frame<T: Serialize>(
    data: &T,
    encoding: StreamEncoding,
) -> Result<Message, Box<dyn std::error::Error + Send + Sync>> {
    match encoding {
        StreamEncoding::Json => {
            let text = serde_json::to_string(data)?;
            Ok(Message::Text(text))
        }
        StreamEncoding::MsgPack => {
            let bytes = rmp_serde::to_vec_named(data)?;
            Ok(Message::Binary(bytes))
        }
    }
}

/// Query parameters for WebSocket connection upgrade.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct WsQueryParams {
    /// Optional JWT or API Key token in query parameter
    pub token: Option<String>,
    /// Comma-separated list of target stream topics (e.g. "anomalies", "sentiment", "all")
    pub streams: Option<String>,
    /// Serialization format: "json" (default) or "msgpack" / "messagepack"
    pub format: Option<String>,
    /// Serialization encoding alias (e.g. "msgpack" or "json")
    pub encoding: Option<String>,
}

/// WebSocket upgrade handler for `/ws`.
pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    Query(query): Query<WsQueryParams>,
    State(state): State<AppState>,
) -> Response {
    if state.kafka_consumer.active_connections_count()
        >= state.kafka_consumer.config().max_connections
    {
        warn!(
            "[WebSocket] Connection rejected: exceeded maximum concurrent connections ({})",
            state.kafka_consumer.config().max_connections
        );
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "Maximum concurrent WebSocket connections reached",
        )
            .into_response();
    }

    ws.on_upgrade(move |socket| handle_socket(socket, query, state))
}

/// Manages an individual WebSocket client session.
async fn handle_socket(socket: WebSocket, query: WsQueryParams, state: AppState) {
    let (mut ws_sender, mut ws_receiver) = socket.split();

    let conn_id = state
        .kafka_consumer
        .register_connection()
        .unwrap_or_else(|_| state.kafka_consumer.active_connections_count());

    let encoding = StreamEncoding::from_query(query.format.as_deref(), query.encoding.as_deref());

    info!(
        "[WebSocket] Client connected (Active session #{}, total: {}, encoding: {:?})",
        conn_id,
        state.kafka_consumer.active_connections_count(),
        encoding
    );

    // Parse initial stream subscriptions from query parameter
    let mut subscribed_sentiment = true;
    let mut subscribed_anomalies = false;

    if let Some(ref streams_raw) = query.streams {
        let lower = streams_raw.to_lowercase();
        let tokens: Vec<&str> = lower.split(',').map(|s| s.trim()).collect();

        if tokens.contains(&"all") {
            subscribed_sentiment = true;
            subscribed_anomalies = true;
        } else {
            subscribed_anomalies = tokens.contains(&"anomalies") || tokens.contains(&"anomaly");
            subscribed_sentiment = tokens.contains(&"sentiment")
                || (!tokens.contains(&"anomalies") && !tokens.contains(&"anomaly"));
        }
    }

    let mut bcast_rx = state.kafka_consumer.subscribe_client();
    let mut anomaly_rx = state.anomaly_broadcaster.subscribe();

    // Production Mode Guard: If real-time data source is unavailable, close connection with error
    if state.production_mode && !state.kafka_consumer.is_connected() {
        warn!("[WebSocket] Closing connection in production mode: real-time Kafka data source unavailable.");
        let err_msg = serde_json::json!({
            "error": "Service Unavailable",
            "message": "Required real-time data source unavailable in production mode."
        });
        if let Ok(frame) = encode_frame(&err_msg, encoding) {
            let _ = ws_sender.send(frame).await;
        }
        let _ = ws_sender
            .send(Message::Close(Some(CloseFrame {
                code: 1011,
                reason: "Required real-time data source unavailable in production mode.".into(),
            })))
            .await;
        state.kafka_consumer.unregister_connection();
        return;
    }

    // Send initial handshake / welcome frame in negotiated encoding
    let welcome = serde_json::json!({
        "type": "connected",
        "server": "fintext-api",
        "version": "2.0.0-institutional",
        "encoding": encoding.as_str(),
        "topic": state.kafka_consumer.config().topic,
        "subject": state.kafka_consumer.config().topic,
        "active_connections": state.kafka_consumer.active_connections_count(),
        "subscribed_streams": {
            "sentiment": subscribed_sentiment,
            "anomalies": subscribed_anomalies,
        }
    });

    if let Ok(welcome_msg) = encode_frame(&welcome, encoding) {
        if let Err(e) = ws_sender.send(welcome_msg).await {
            error!("[WebSocket] Failed to send handshake message: {}", e);
            state.kafka_consumer.unregister_connection();
            return;
        }
    }

    let mut ping_interval = tokio::time::interval(Duration::from_secs(30));

    loop {
        tokio::select! {
            // 1. Periodic keep-alive heartbeat
            _ = ping_interval.tick() => {
                if let Err(_) = ws_sender.send(Message::Ping(vec![].into())).await {
                    break;
                }
            }

            // 2. Real-time sentiment update from Kafka broadcast channel
            broadcast_res = bcast_rx.recv(), if subscribed_sentiment => {
                match broadcast_res {
                    Ok(payload) => {
                        let send_res = match encoding {
                            StreamEncoding::Json => ws_sender.send(Message::Text(payload)).await,
                            StreamEncoding::MsgPack => {
                                if let Ok(parsed_val) = serde_json::from_str::<serde_json::Value>(&payload) {
                                    if let Ok(msgpack_bytes) = rmp_serde::to_vec_named(&parsed_val) {
                                        ws_sender.send(Message::Binary(msgpack_bytes)).await
                                    } else {
                                        ws_sender.send(Message::Text(payload)).await
                                    }
                                } else {
                                    ws_sender.send(Message::Text(payload)).await
                                }
                            }
                        };
                        if let Err(e) = send_res {
                            warn!("[WebSocket] Failed to push sentiment payload to client: {}. Disconnecting client.", e);
                            break;
                        }
                    }
                    Err(RecvError::Lagged(skipped)) => {
                        warn!("[WebSocket] Client lagged behind by {} sentiment messages. Skipping dropped frames.", skipped);
                    }
                    Err(RecvError::Closed) => {
                        info!("[WebSocket] Sentiment broadcast channel closed.");
                        break;
                    }
                }
            }

            // 3. Real-time sentiment anomaly alert from AnomalyBroadcaster
            anomaly_res = anomaly_rx.recv(), if subscribed_anomalies => {
                match anomaly_res {
                    Ok(alert) => {
                        if let Ok(frame) = encode_frame(&alert, encoding) {
                            if let Err(e) = ws_sender.send(frame).await {
                                warn!("[WebSocket] Failed to push anomaly alert to client: {}. Disconnecting client.", e);
                                break;
                            }
                        }
                    }
                    Err(RecvError::Lagged(skipped)) => {
                        warn!("[WebSocket] Client lagged behind by {} anomaly alerts. Skipping dropped frames.", skipped);
                    }
                    Err(RecvError::Closed) => {
                        info!("[WebSocket] Anomaly broadcast channel closed.");
                        break;
                    }
                }
            }

            // 4. Incoming client frames (Pong, Close, or client control requests in JSON or MsgPack)
            client_frame = ws_receiver.next() => {
                let maybe_cmd: Option<serde_json::Value> = match client_frame {
                    Some(Ok(Message::Close(_))) => {
                        info!("[WebSocket] Client sent explicit Close frame.");
                        break;
                    }
                    Some(Ok(Message::Ping(payload))) => {
                        let _ = ws_sender.send(Message::Pong(payload)).await;
                        None
                    }
                    Some(Ok(Message::Text(text))) => {
                        serde_json::from_str::<serde_json::Value>(&text).ok()
                    }
                    Some(Ok(Message::Binary(bytes))) => {
                        rmp_serde::from_slice::<serde_json::Value>(&bytes).ok()
                    }
                    Some(Err(e)) => {
                        warn!("[WebSocket] Client read error: {}. Closing connection.", e);
                        break;
                    }
                    None => {
                        // Client closed stream
                        break;
                    }
                    _ => None,
                };

                if let Some(cmd) = maybe_cmd {
                    if let Some(action) = cmd.get("action").and_then(|v| v.as_str()) {
                        let stream = cmd.get("stream").and_then(|v| v.as_str()).unwrap_or("");
                        match (action, stream) {
                            ("subscribe", "anomalies") | ("subscribe", "anomaly") => {
                                subscribed_anomalies = true;
                                let ack = serde_json::json!({
                                    "type": "subscribed",
                                    "stream": "anomalies",
                                    "status": "active"
                                });
                                if let Ok(frame) = encode_frame(&ack, encoding) {
                                    let _ = ws_sender.send(frame).await;
                                }
                            }
                            ("unsubscribe", "anomalies") | ("unsubscribe", "anomaly") => {
                                subscribed_anomalies = false;
                                let ack = serde_json::json!({
                                    "type": "unsubscribed",
                                    "stream": "anomalies",
                                    "status": "inactive"
                                });
                                if let Ok(frame) = encode_frame(&ack, encoding) {
                                    let _ = ws_sender.send(frame).await;
                                }
                            }
                            ("subscribe", "sentiment") => {
                                subscribed_sentiment = true;
                                let ack = serde_json::json!({
                                    "type": "subscribed",
                                    "stream": "sentiment",
                                    "status": "active"
                                });
                                if let Ok(frame) = encode_frame(&ack, encoding) {
                                    let _ = ws_sender.send(frame).await;
                                }
                            }
                            ("unsubscribe", "sentiment") => {
                                subscribed_sentiment = false;
                                let ack = serde_json::json!({
                                    "type": "unsubscribed",
                                    "stream": "sentiment",
                                    "status": "inactive"
                                });
                                if let Ok(frame) = encode_frame(&ack, encoding) {
                                    let _ = ws_sender.send(frame).await;
                                }
                            }
                            ("ping", _) => {
                                let pong = serde_json::json!({"type": "pong"});
                                if let Ok(frame) = encode_frame(&pong, encoding) {
                                    let _ = ws_sender.send(frame).await;
                                }
                            }
                            _ => {
                                debug!("[WebSocket] Inbound unhandled client command: {:?}", cmd);
                            }
                        }
                    }
                }
            }
        }
    }

    let remaining = state.kafka_consumer.unregister_connection();
    info!(
        "[WebSocket] Client disconnected cleanly. Remaining active connections: {}",
        remaining
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_encoding_negotiation() {
        assert_eq!(
            StreamEncoding::from_query(Some("msgpack"), None),
            StreamEncoding::MsgPack
        );
        assert_eq!(
            StreamEncoding::from_query(Some("messagepack"), None),
            StreamEncoding::MsgPack
        );
        assert_eq!(
            StreamEncoding::from_query(Some("mp"), None),
            StreamEncoding::MsgPack
        );
        assert_eq!(
            StreamEncoding::from_query(None, Some("msgpack")),
            StreamEncoding::MsgPack
        );
        assert_eq!(
            StreamEncoding::from_query(Some("json"), None),
            StreamEncoding::Json
        );
        assert_eq!(StreamEncoding::from_query(None, None), StreamEncoding::Json);
        assert_eq!(
            StreamEncoding::from_query(Some("unknown"), None),
            StreamEncoding::Json
        );
    }

    #[test]
    fn test_encode_frame_json_and_msgpack() {
        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        struct TestAlert {
            ticker: String,
            z_score: f64,
            is_anomaly: bool,
        }

        let alert = TestAlert {
            ticker: "NVDA".to_string(),
            z_score: 3.42,
            is_anomaly: true,
        };

        // 1. JSON Frame
        let json_msg =
            encode_frame(&alert, StreamEncoding::Json).expect("Failed to encode JSON frame");
        match json_msg {
            Message::Text(text) => {
                let parsed: TestAlert = serde_json::from_str(&text).expect("Failed to decode JSON");
                assert_eq!(parsed, alert);
            }
            _ => panic!("Expected Message::Text for JSON encoding"),
        }

        // 2. MessagePack Frame
        let msgpack_msg =
            encode_frame(&alert, StreamEncoding::MsgPack).expect("Failed to encode MsgPack frame");
        match msgpack_msg {
            Message::Binary(bytes) => {
                let parsed: TestAlert =
                    rmp_serde::from_slice(&bytes).expect("Failed to decode MsgPack");
                assert_eq!(parsed, alert);
            }
            _ => panic!("Expected Message::Binary for MsgPack encoding"),
        }
    }

    #[test]
    fn test_msgpack_binary_frame_decoding() {
        let cmd = serde_json::json!({
            "action": "subscribe",
            "stream": "anomalies"
        });

        let bytes = rmp_serde::to_vec_named(&cmd).expect("Failed to serialize MsgPack");
        let decoded: serde_json::Value =
            rmp_serde::from_slice(&bytes).expect("Failed to deserialize MsgPack");
        assert_eq!(decoded["action"], "subscribe");
        assert_eq!(decoded["stream"], "anomalies");
    }
}
