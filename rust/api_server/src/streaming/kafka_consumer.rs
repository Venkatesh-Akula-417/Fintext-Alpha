//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Kafka Real-Time Stream Consumer & Broadcaster
//! ═══════════════════════════════════════════════════════════════════════════════
//!
//! Subscribes to Kafka topic 'sentiment-updates' (consumer group 'fintext-api-websocket')
//! and fans out streaming JSON events to all active WebSocket clients with
//! sub-millisecond distribution and connection throttling.
//! ═══════════════════════════════════════════════════════════════════════════════

use futures_util::StreamExt;
use rdkafka::config::ClientConfig;
use rdkafka::consumer::stream_consumer::StreamConsumer;
use rdkafka::consumer::Consumer;
use rdkafka::message::Message;
use serde::{Deserialize, Serialize};
use std::env;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::time::sleep;
use tracing::{error, info, warn};

/// Configuration settings for the Kafka consumer and WebSocket broadcaster.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KafkaSubscriberConfig {
    pub bootstrap_servers: String,
    pub topic: String,
    pub group_id: String,
    pub timeout_ms: u64,
    pub channel_capacity: usize,
    pub max_connections: usize,
    pub enabled: bool,
    pub mock_mode: bool,
}

impl Default for KafkaSubscriberConfig {
    fn default() -> Self {
        let bootstrap_servers =
            env::var("KAFKA_BOOTSTRAP_SERVERS").unwrap_or_else(|_| "localhost:9092".to_string());
        let topic = env::var("KAFKA_TOPIC_REALTIME")
            .or_else(|_| env::var("KAFKA_REALTIME_TOPIC"))
            .or_else(|_| env::var("KAFKA_TOPIC"))
            .unwrap_or_else(|_| "sentiment-updates".to_string());
        let group_id = env::var("KAFKA_GROUP_ID")
            .or_else(|_| env::var("KAFKA_CONSUMER_GROUP_ID"))
            .unwrap_or_else(|_| "fintext-api-websocket".to_string());
        let timeout_ms = env::var("KAFKA_TIMEOUT_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(2000);
        let channel_capacity = env::var("WS_CHANNEL_CAPACITY")
            .or_else(|_| env::var("KAFKA_CHANNEL_CAPACITY"))
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(1024);
        let max_connections = env::var("WS_MAX_CONNECTIONS")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(1000);
        let enabled = env::var("KAFKA_ENABLED")
            .map(|v| v != "0" && v.to_lowercase() != "false")
            .unwrap_or(true);
        let mock_mode = crate::state::is_kafka_mock_fallback_enabled();

        Self {
            bootstrap_servers,
            topic,
            group_id,
            timeout_ms,
            channel_capacity,
            max_connections,
            enabled,
            mock_mode,
        }
    }
}

/// Thread-safe Kafka real-time consumer and fan-out broadcaster for WebSocket clients.
#[derive(Clone)]
pub struct KafkaSubscriber {
    config: KafkaSubscriberConfig,
    tx: broadcast::Sender<String>,
    active_connections: Arc<AtomicUsize>,
    is_connected: Arc<AtomicBool>,
}

impl KafkaSubscriber {
    /// Creates a new `KafkaSubscriber` with the given configuration.
    pub fn new(config: KafkaSubscriberConfig) -> Self {
        let (tx, _) = broadcast::channel(config.channel_capacity);
        Self {
            config,
            tx,
            active_connections: Arc::new(AtomicUsize::new(0)),
            is_connected: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Helper constructor loading from environment variables.
    pub fn from_env() -> Self {
        Self::new(KafkaSubscriberConfig::default())
    }

    /// Access configuration parameters.
    pub fn config(&self) -> &KafkaSubscriberConfig {
        &self.config
    }

    /// Returns true if the Kafka subscriber has an active live connection to the broker.
    pub fn is_connected(&self) -> bool {
        self.is_connected.load(Ordering::SeqCst)
    }

    /// Obtains a new receiver handle for a connecting WebSocket client.
    pub fn subscribe_client(&self) -> broadcast::Receiver<String> {
        self.tx.subscribe()
    }

    /// Direct local broadcast method (used for mock injection, testing, or internal signals).
    pub fn broadcast_message(&self, message: &str) -> usize {
        self.tx.send(message.to_string()).unwrap_or(0)
    }

    /// Current number of registered WebSocket connections.
    pub fn active_connections_count(&self) -> usize {
        self.active_connections.load(Ordering::Relaxed)
    }

    /// Registers a new active connection if below the configured limit.
    pub fn register_connection(&self) -> Result<usize, &'static str> {
        let count = self.active_connections.fetch_add(1, Ordering::SeqCst) + 1;
        if count > self.config.max_connections {
            self.active_connections.fetch_sub(1, Ordering::SeqCst);
            Err("Maximum concurrent WebSocket connections reached")
        } else {
            Ok(count)
        }
    }

    /// Unregisters an active connection upon disconnect.
    pub fn unregister_connection(&self) -> usize {
        let prev = self.active_connections.fetch_sub(1, Ordering::SeqCst);
        if prev == 0 {
            self.active_connections.store(0, Ordering::SeqCst);
            0
        } else {
            prev - 1
        }
    }

    /// Starts the background Kafka stream consumption worker.
    ///
    /// Spawns a Tokio task that connects to Kafka/Redpanda, joins the consumer group,
    /// subscribes to the topic, and forwards streaming payloads into the broadcast channel.
    pub fn start_subscription(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            if !self.config.enabled || self.config.mock_mode {
                self.is_connected.store(false, Ordering::SeqCst);
                info!(
                    "[Kafka Subscriber] Running in offline/mock mode (enabled={}, mock_mode={}).",
                    self.config.enabled, self.config.mock_mode
                );
                return;
            }

            info!(
                "[Kafka Subscriber] Connecting to Kafka brokers='{}', topic='{}', group='{}'",
                self.config.bootstrap_servers, self.config.topic, self.config.group_id
            );

            let mut backoff = Duration::from_millis(100);

            loop {
                // 1. Initialize StreamConsumer with configuration
                let consumer_res: Result<StreamConsumer, _> = ClientConfig::new()
                    .set("bootstrap.servers", &self.config.bootstrap_servers)
                    .set("group.id", &self.config.group_id)
                    .set("enable.auto.commit", "true")
                    .set("auto.offset.reset", "latest")
                    .set("session.timeout.ms", "6000")
                    .set("enable.partition.eof", "false")
                    .create();

                let consumer = match consumer_res {
                    Ok(c) => {
                        info!(
                            "[Kafka Subscriber] Connected to broker '{}' with group '{}'",
                            self.config.bootstrap_servers, self.config.group_id
                        );
                        backoff = Duration::from_millis(100);
                        c
                    }
                    Err(e) => {
                        self.is_connected.store(false, Ordering::SeqCst);
                        warn!(
                            "[Kafka Subscriber] Consumer creation warning for '{}': {}. Retrying in {:?}...",
                            self.config.bootstrap_servers, e, backoff
                        );
                        sleep(backoff).await;
                        backoff = (backoff * 2).min(Duration::from_secs(5));
                        continue;
                    }
                };

                // 2. Subscribe to the real-time sentiment updates topic
                if let Err(e) = consumer.subscribe(&[&self.config.topic]) {
                    self.is_connected.store(false, Ordering::SeqCst);
                    error!(
                        "[Kafka Subscriber] Failed to subscribe to topic '{}': {}. Retrying in {:?}...",
                        self.config.topic, e, backoff
                    );
                    sleep(backoff).await;
                    backoff = (backoff * 2).min(Duration::from_secs(5));
                    continue;
                }

                self.is_connected.store(true, Ordering::SeqCst);
                info!(
                    "[Kafka Subscriber] Subscribed to topic '{}'",
                    self.config.topic
                );

                // 3. Continuous message consumption loop
                let mut message_stream = consumer.stream();
                while let Some(msg_result) = message_stream.next().await {
                    match msg_result {
                        Ok(msg) => {
                            if let Some(payload_bytes) = msg.payload() {
                                let payload = String::from_utf8_lossy(payload_bytes).to_string();
                                let client_count = self.tx.receiver_count();
                                if client_count > 0 {
                                    let _ = self.tx.send(payload);
                                }
                            }
                        }
                        Err(e) => {
                            warn!("[Kafka Subscriber] Kafka stream error: {}", e);
                        }
                    }
                }

                self.is_connected.store(false, Ordering::SeqCst);
                warn!("[Kafka Subscriber] Stream consumption loop terminated. Attempting reconnect...");
                sleep(Duration::from_secs(1)).await;
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_kafka_subscriber_broadcast_fanout() {
        let config = KafkaSubscriberConfig {
            bootstrap_servers: "127.0.0.1:9092".to_string(),
            topic: "test.sentiment.updates".to_string(),
            group_id: "test-group".to_string(),
            timeout_ms: 500,
            channel_capacity: 16,
            max_connections: 10,
            enabled: true,
            mock_mode: true,
        };

        let subscriber = Arc::new(KafkaSubscriber::new(config));

        // Create 2 simulated client receivers
        let mut rx1 = subscriber.subscribe_client();
        let mut rx2 = subscriber.subscribe_client();

        let sample_payload = r#"{"article_id":"test-001","ticker":"NVDA","sentiment_score":0.95}"#;
        let delivered = subscriber.broadcast_message(sample_payload);
        assert_eq!(delivered, 2, "Message should be delivered to 2 receivers");

        let msg1 = rx1.recv().await.expect("Client 1 should receive message");
        let msg2 = rx2.recv().await.expect("Client 2 should receive message");

        assert_eq!(msg1, sample_payload);
        assert_eq!(msg2, sample_payload);
    }

    #[test]
    fn test_kafka_connection_limit_counter() {
        let config = KafkaSubscriberConfig {
            bootstrap_servers: "127.0.0.1:9092".to_string(),
            topic: "test.sentiment.updates".to_string(),
            group_id: "test-group".to_string(),
            timeout_ms: 500,
            channel_capacity: 16,
            max_connections: 2,
            enabled: true,
            mock_mode: true,
        };

        let subscriber = KafkaSubscriber::new(config);

        assert_eq!(subscriber.register_connection(), Ok(1));
        assert_eq!(subscriber.register_connection(), Ok(2));
        assert!(
            subscriber.register_connection().is_err(),
            "Exceeding max connections should error"
        );

        assert_eq!(subscriber.unregister_connection(), 1);
        assert_eq!(subscriber.register_connection(), Ok(2));
        assert_eq!(subscriber.unregister_connection(), 1);
        assert_eq!(subscriber.unregister_connection(), 0);
    }
}
