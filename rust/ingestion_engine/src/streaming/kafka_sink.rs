//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — High-Throughput Kafka Producer & Unified Messaging Sink
//! ═══════════════════════════════════════════════════════════════════════════════
//!
//! Publishes preprocessed, ONNX-scored financial sentiment events to Apache Kafka /
//! Redpanda topics for both durable event streaming (`sentiment-events`)
//! and ultra-low-latency real-time distribution (`sentiment-updates`) to API server
//! WebSocket clients and trading execution algorithms.
//! ═══════════════════════════════════════════════════════════════════════════════

use crate::nlp::SentimentOutput;
use crate::pipeline::ProcessedDocument;
use rdkafka::config::ClientConfig;
use rdkafka::producer::{FutureProducer, FutureRecord};
use rdkafka::util::Timeout;
use serde::{Deserialize, Serialize};
use std::env;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, info, warn};

/// Structured durable sentiment event payload published to Kafka (`sentiment-events`).
/// Standardized JSON schema for persistent event streaming and downstream subscribers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KafkaSentimentEvent {
    pub article_id: String,
    pub title: String,
    pub source: String,
    pub published_utc: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ingested_utc: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub db_commit_utc: Option<String>,
    pub clean_text: String,
    pub tickers: Vec<String>,
    pub primary_ticker: Option<String>,
    pub sentiment_score: f64,
    pub sentiment_label: String,
    pub prob_positive: f64,
    pub prob_negative: f64,
    pub prob_neutral: f64,
    pub event_category: String,
    pub signal_available_ts_us: i64,
    #[serde(default)]
    pub entities: String,
    #[serde(default)]
    pub pitch_mean: f64,
    #[serde(default)]
    pub pitch_std: f64,
    #[serde(default)]
    pub energy_mean: f64,
    #[serde(default)]
    pub energy_std: f64,
    #[serde(default)]
    pub pause_ratio: f64,
    #[serde(default)]
    pub speech_rate: f64,
    #[serde(default)]
    pub vpin: f64,
    #[serde(default)]
    pub gex: f64,
    #[serde(default)]
    pub gex_positive: f64,
    #[serde(default)]
    pub gex_negative: f64,
}

impl KafkaSentimentEvent {
    /// Constructs a `KafkaSentimentEvent` from processed document and sentiment classification.
    pub fn from_components(
        doc: &ProcessedDocument,
        sentiment: &SentimentOutput,
        signal_avail_ts_us: i64,
    ) -> Self {
        let entities = serde_json::to_string(&doc.entities).unwrap_or_else(|_| "[]".to_string());
        let (pitch_mean, pitch_std, energy_mean, energy_std, pause_ratio, speech_rate) =
            if let Some(ref af) = doc.audio_features {
                (
                    af.pitch_mean,
                    af.pitch_std,
                    af.energy_mean,
                    af.energy_std,
                    af.pause_ratio,
                    af.speech_rate,
                )
            } else {
                (0.0, 0.0, 0.0, 0.0, 0.0, 0.0)
            };

        let vpin = doc.vpin.unwrap_or(0.0);
        let gex = doc.gex.unwrap_or(0.0);
        let gex_positive = doc.gex_positive.unwrap_or(0.0);
        let gex_negative = doc.gex_negative.unwrap_or(0.0);

        Self {
            article_id: doc.id.clone(),
            title: doc.title.clone(),
            source: doc.source.clone(),
            published_utc: doc.published_utc.clone(),
            ingested_utc: Some(doc.ingested_utc.clone()),
            db_commit_utc: doc.db_commit_utc.clone(),
            clean_text: doc.clean_text.clone(),
            tickers: doc.tickers.clone(),
            primary_ticker: doc.primary_ticker.clone(),
            sentiment_score: sentiment.sentiment_score,
            sentiment_label: sentiment.sentiment_label.clone(),
            prob_positive: sentiment.prob_positive,
            prob_negative: sentiment.prob_negative,
            prob_neutral: sentiment.prob_neutral,
            event_category: doc.event_category.clone(),
            signal_available_ts_us: signal_avail_ts_us,
            entities,
            pitch_mean,
            pitch_std,
            energy_mean,
            energy_std,
            pause_ratio,
            speech_rate,
            vpin,
            gex,
            gex_positive,
            gex_negative,
        }
    }

    /// Serializes the event into a compact JSON string.
    pub fn to_json_string(&self) -> Result<String, String> {
        serde_json::to_string(self)
            .map_err(|e| format!("Failed to serialize Kafka event to JSON: {}", e))
    }
}

/// Compact real-time sentiment event payload for WebSocket & algorithmic subscribers (`sentiment-updates`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RealtimeSentimentEvent {
    pub article_id: String,
    pub ticker: String,
    pub sentiment_score: f64,
    pub sentiment_label: String,
    pub signal_available_ts_us: i64,
    pub published_utc: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ingested_utc: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub db_commit_utc: Option<String>,
    pub title: String,
    pub source: String,
    #[serde(default)]
    pub entities: String,
    #[serde(default)]
    pub pitch_mean: f64,
    #[serde(default)]
    pub pitch_std: f64,
    #[serde(default)]
    pub energy_mean: f64,
    #[serde(default)]
    pub energy_std: f64,
    #[serde(default)]
    pub pause_ratio: f64,
    #[serde(default)]
    pub speech_rate: f64,
    #[serde(default)]
    pub vpin: f64,
    #[serde(default)]
    pub gex: f64,
    #[serde(default)]
    pub gex_positive: f64,
    #[serde(default)]
    pub gex_negative: f64,
}

impl RealtimeSentimentEvent {
    /// Constructs a `RealtimeSentimentEvent` from processed document and sentiment classification.
    pub fn from_components(
        doc: &ProcessedDocument,
        sentiment: &SentimentOutput,
        signal_avail_ts_us: i64,
    ) -> Self {
        let ticker = doc
            .primary_ticker
            .clone()
            .unwrap_or_else(|| "UNKNOWN".to_string());

        let entities = serde_json::to_string(&doc.entities).unwrap_or_else(|_| "[]".to_string());

        let (pitch_mean, pitch_std, energy_mean, energy_std, pause_ratio, speech_rate) =
            if let Some(ref af) = doc.audio_features {
                (
                    af.pitch_mean,
                    af.pitch_std,
                    af.energy_mean,
                    af.energy_std,
                    af.pause_ratio,
                    af.speech_rate,
                )
            } else {
                (0.0, 0.0, 0.0, 0.0, 0.0, 0.0)
            };

        let vpin = doc.vpin.unwrap_or(0.0);
        let gex = doc.gex.unwrap_or(0.0);
        let gex_positive = doc.gex_positive.unwrap_or(0.0);
        let gex_negative = doc.gex_negative.unwrap_or(0.0);

        Self {
            article_id: doc.id.clone(),
            ticker,
            sentiment_score: sentiment.sentiment_score,
            sentiment_label: sentiment.sentiment_label.clone(),
            signal_available_ts_us: signal_avail_ts_us,
            published_utc: doc.published_utc.clone(),
            ingested_utc: Some(doc.ingested_utc.clone()),
            db_commit_utc: doc.db_commit_utc.clone(),
            title: doc.title.clone(),
            source: doc.source.clone(),
            entities,
            pitch_mean,
            pitch_std,
            energy_mean,
            energy_std,
            pause_ratio,
            speech_rate,
            vpin,
            gex,
            gex_positive,
            gex_negative,
        }
    }

    /// Serializes the event into a compact JSON string (<1KB).
    pub fn to_json_string(&self) -> Result<String, String> {
        serde_json::to_string(self).map_err(|e| {
            format!(
                "Failed to serialize realtime sentiment event to JSON: {}",
                e
            )
        })
    }
}

/// Configuration settings for the unified Kafka streaming producer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KafkaSinkConfig {
    pub bootstrap_servers: String,
    pub topic: String,
    pub realtime_topic: String,
    pub delivery_timeout_ms: u64,
    pub max_retries: usize,
    pub enabled: bool,
    pub mock_mode: bool,
}

impl Default for KafkaSinkConfig {
    fn default() -> Self {
        let bootstrap_servers =
            env::var("KAFKA_BOOTSTRAP_SERVERS").unwrap_or_else(|_| "localhost:9092".to_string());
        let topic = env::var("KAFKA_TOPIC").unwrap_or_else(|_| "sentiment-events".to_string());
        let realtime_topic = env::var("KAFKA_TOPIC_REALTIME")
            .or_else(|_| env::var("KAFKA_REALTIME_TOPIC"))
            .unwrap_or_else(|_| "sentiment-updates".to_string());
        let delivery_timeout_ms = env::var("KAFKA_DELIVERY_TIMEOUT_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(5000);
        let max_retries = env::var("KAFKA_MAX_RETRIES")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(3);
        let enabled = env::var("KAFKA_ENABLED")
            .map(|v| v != "0" && v.to_lowercase() != "false")
            .unwrap_or(true);
        let mock_mode = if crate::is_production_mode() {
            false
        } else {
            env::var("KAFKA_MOCK_FALLBACK").as_deref() == Ok("1")
                || env::var("KAFKA_MOCK_MODE").as_deref() == Ok("1")
        };

        Self {
            bootstrap_servers,
            topic,
            realtime_topic,
            delivery_timeout_ms,
            max_retries,
            enabled,
            mock_mode,
        }
    }
}

/// High-throughput asynchronous Kafka producer sink.
#[derive(Clone)]
pub struct KafkaSink {
    config: KafkaSinkConfig,
    producer: Option<Arc<FutureProducer>>,
}

impl KafkaSink {
    /// Creates a new `KafkaSink` with the specified configuration.
    pub fn new(config: KafkaSinkConfig) -> Self {
        if !config.enabled || config.mock_mode {
            info!(
                "[Kafka Sink] Running without active live producer (enabled={}, mock_mode={}).",
                config.enabled, config.mock_mode
            );
            return Self {
                config,
                producer: None,
            };
        }

        info!(
            "[Kafka Sink] Initializing rdkafka FutureProducer for brokers='{}', topics: events='{}', realtime='{}' (timeout={}ms)",
            config.bootstrap_servers, config.topic, config.realtime_topic, config.delivery_timeout_ms
        );

        let producer_res: Result<FutureProducer, _> = ClientConfig::new()
            .set("bootstrap.servers", &config.bootstrap_servers)
            .set(
                "message.timeout.ms",
                &config.delivery_timeout_ms.to_string(),
            )
            .set("queue.buffering.max.ms", "20")
            .set("compression.type", "lz4")
            .set("acks", "1")
            .create();

        match producer_res {
            Ok(p) => Self {
                config,
                producer: Some(Arc::new(p)),
            },
            Err(e) => {
                warn!("[Kafka Sink] Producer initialization warning: {}. Producer will operate in fallback mode.", e);
                Self {
                    config,
                    producer: None,
                }
            }
        }
    }

    /// Helper constructor loading from environment variables.
    pub fn from_env() -> Self {
        Self::new(KafkaSinkConfig::default())
    }

    /// Access the underlying configuration.
    pub fn config(&self) -> &KafkaSinkConfig {
        &self.config
    }

    /// Sends a durable processed sentiment event to Kafka topic (`sentiment-events`) with retry logic.
    pub async fn send_event(
        &self,
        doc: &ProcessedDocument,
        sentiment: &SentimentOutput,
        signal_avail_ts_us: i64,
    ) -> Result<(), String> {
        let event = KafkaSentimentEvent::from_components(doc, sentiment, signal_avail_ts_us);
        let payload = event.to_json_string()?;
        let partition_key = doc.primary_ticker.as_deref().unwrap_or(&doc.id).to_string();

        self.publish_raw(&self.config.topic, &partition_key, &payload)
            .await
    }

    /// Publishes a compact real-time sentiment event to Kafka topic (`sentiment-updates`) for WebSocket fan-out.
    pub async fn publish_realtime_event(
        &self,
        doc: &ProcessedDocument,
        sentiment: &SentimentOutput,
        signal_avail_ts_us: i64,
    ) -> Result<(), String> {
        let event = RealtimeSentimentEvent::from_components(doc, sentiment, signal_avail_ts_us);
        let payload = event.to_json_string()?;
        let partition_key = doc.primary_ticker.clone().unwrap_or_else(|| doc.id.clone());

        self.publish_raw(&self.config.realtime_topic, &partition_key, &payload)
            .await
    }

    /// Sends a payload to an arbitrary topic with backoff and retry handling.
    pub async fn send_to_topic(&self, topic: &str, key: &str, payload: &str) -> Result<(), String> {
        self.publish_raw(topic, key, payload).await
    }

    /// Internal raw publish handler.
    async fn publish_raw(&self, topic: &str, key: &str, payload: &str) -> Result<(), String> {
        if !self.config.enabled {
            let msg = format!(
                "Kafka producer is disabled (enabled=false, brokers: {}). Event dropped from stream.",
                self.config.bootstrap_servers
            );
            warn!("[Kafka Sink] {}", msg);
            return Err(msg);
        }

        if !crate::is_production_mode()
            && (self.config.mock_mode
                || env::var("KAFKA_MOCK_FALLBACK").as_deref() == Ok("1")
                || env::var("KAFKA_MOCK_MODE").as_deref() == Ok("1"))
        {
            info!(
                "[Kafka MOCK] Event serialized ({} bytes) for key='{}' -> topic='{}'",
                payload.len(),
                key,
                topic
            );
            return Ok(());
        }

        let producer = match &self.producer {
            Some(p) => p,
            None => {
                let msg = format!(
                    "Kafka producer not initialized (brokers: {}). Event dropped from stream.",
                    self.config.bootstrap_servers
                );
                warn!("[Kafka Sink] {}", msg);
                return Err(msg);
            }
        };

        let mut backoff = Duration::from_millis(100);

        for attempt in 1..=self.config.max_retries {
            let record = FutureRecord::to(topic).payload(payload).key(key);
            let timeout = Timeout::After(Duration::from_millis(self.config.delivery_timeout_ms));

            match producer.send(record, timeout).await {
                Ok((partition, offset)) => {
                    info!(
                        "[Kafka Sink] Event published -> topic='{}' partition={} offset={} key='{}'",
                        topic, partition, offset, key
                    );
                    return Ok(());
                }
                Err((err, _msg)) => {
                    warn!(
                        "[Kafka Sink] Send attempt {}/{} failed for topic '{}': {}. Backing off {:?}...",
                        attempt, self.config.max_retries, topic, err, backoff
                    );
                }
            }

            if attempt < self.config.max_retries {
                sleep(backoff).await;
                backoff = Duration::from_millis(backoff.as_millis() as u64 * 2);
            }
        }

        let err_msg = format!(
            "Failed to publish event to Kafka topic '{}' after {} attempts (broker unreachable)",
            topic, self.config.max_retries
        );
        error!("[Kafka Sink] {}", err_msg);
        Err(err_msg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_doc() -> ProcessedDocument {
        ProcessedDocument {
            id: "doc-kafka-001".to_string(),
            title: "NVIDIA Reports Record Q4 Revenue of $22.1B on AI Accelerator Demand".to_string(),
            source: "SEC_8K".to_string(),
            url: "https://sec.gov/edgar/nvda".to_string(),
            published_utc: "2026-08-25T16:00:00Z".to_string(),
            ingested_utc: "2026-08-25T16:00:00.050Z".to_string(),
            db_commit_utc: None,
            clean_text: "NVIDIA reported revenue for the fourth quarter of $22.1 billion, up 265% from a year ago.".to_string(),
            extracted_links: vec!["https://nvidia.com".to_string()],
            tickers: vec!["NVDA".to_string()],
            primary_ticker: Some("NVDA".to_string()),
            ticker_confidences: vec![("NVDA".to_string(), 0.99)],
            is_spam: false,
            spam_reason: "Clean".to_string(),
            event_category: "EARNINGS".to_string(),
            preprocessing_latency_us: 380,
            entities: vec![],
            audio_transcript: None,
            audio_features: None,
            vpin: Some(0.45),
            gex: Some(500000.0),
            gex_positive: Some(750000.0),
            gex_negative: Some(250000.0),
            latency_metrics: None,
            ..Default::default()
        }
    }

    fn sample_sentiment() -> SentimentOutput {
        SentimentOutput {
            sentiment_score: 0.92,
            sentiment_label: "POSITIVE".to_string(),
            prob_positive: 0.95,
            prob_negative: 0.02,
            prob_neutral: 0.03,
            ..Default::default()
        }
    }

    #[test]
    fn test_kafka_sentiment_event_serialization() {
        let doc = sample_doc();
        let sentiment = sample_sentiment();
        let signal_ts = 1724658000437000_i64;

        let event = KafkaSentimentEvent::from_components(&doc, &sentiment, signal_ts);

        assert_eq!(event.article_id, "doc-kafka-001");
        assert_eq!(event.title, doc.title);
        assert_eq!(event.source, "SEC_8K");
        assert_eq!(event.primary_ticker, Some("NVDA".to_string()));
        assert_eq!(event.sentiment_score, 0.92);
        assert_eq!(event.sentiment_label, "POSITIVE");
        assert_eq!(event.prob_positive, 0.95);
        assert_eq!(event.signal_available_ts_us, signal_ts);
        assert_eq!(event.entities, "[]");

        // Serialize to JSON string
        let json_str = event.to_json_string().expect("Serialization failed");
        assert!(json_str.contains("\"article_id\":\"doc-kafka-001\""));
        assert!(json_str.contains("\"primary_ticker\":\"NVDA\""));
        assert!(json_str.contains("\"sentiment_label\":\"POSITIVE\""));
        assert!(json_str.contains("\"signal_available_ts_us\":1724658000437000"));
        assert!(json_str.contains("\"entities\":\"[]\""));

        // Round-trip deserialization
        let deserialized: KafkaSentimentEvent =
            serde_json::from_str(&json_str).expect("Deserialization failed");
        assert_eq!(event, deserialized);
    }

    #[test]
    fn test_realtime_sentiment_event_serialization() {
        let doc = sample_doc();
        let sentiment = sample_sentiment();
        let signal_ts = 1724658000437000_i64;

        let event = RealtimeSentimentEvent::from_components(&doc, &sentiment, signal_ts);

        assert_eq!(event.article_id, "doc-kafka-001");
        assert_eq!(event.ticker, "NVDA");
        assert_eq!(event.sentiment_score, 0.92);
        assert_eq!(event.sentiment_label, "POSITIVE");
        assert_eq!(event.signal_available_ts_us, signal_ts);

        let json_str = event.to_json_string().expect("Serialization failed");
        assert!(
            json_str.len() < 1024,
            "Realtime JSON should be compact and <1KB"
        );
        assert!(json_str.contains("\"article_id\":\"doc-kafka-001\""));
        assert!(json_str.contains("\"ticker\":\"NVDA\""));

        let deserialized: RealtimeSentimentEvent =
            serde_json::from_str(&json_str).expect("Deserialization failed");
        assert_eq!(event, deserialized);
    }

    #[tokio::test]
    async fn test_kafka_sink_mock_mode_success() {
        let config = KafkaSinkConfig {
            bootstrap_servers: "127.0.0.1:9092".to_string(),
            topic: "test-sentiment-events".to_string(),
            realtime_topic: "test-sentiment-updates".to_string(),
            delivery_timeout_ms: 1000,
            max_retries: 2,
            enabled: true,
            mock_mode: true,
        };

        let sink = KafkaSink::new(config);
        let doc = sample_doc();
        let sentiment = sample_sentiment();
        let signal_ts = 1724658000000000_i64;

        let res1 = sink.send_event(&doc, &sentiment, signal_ts).await;
        assert!(res1.is_ok(), "Expected mock send_event to succeed");

        let res2 = sink
            .publish_realtime_event(&doc, &sentiment, signal_ts)
            .await;
        assert!(
            res2.is_ok(),
            "Expected mock publish_realtime_event to succeed"
        );
    }

    #[tokio::test]
    async fn test_kafka_sink_disabled_mode() {
        let config = KafkaSinkConfig {
            bootstrap_servers: "127.0.0.1:9092".to_string(),
            topic: "test-sentiment-events".to_string(),
            realtime_topic: "test-sentiment-updates".to_string(),
            delivery_timeout_ms: 500,
            max_retries: 1,
            enabled: false,
            mock_mode: false,
        };

        let sink = KafkaSink::new(config);
        let doc = sample_doc();
        let sentiment = sample_sentiment();

        let res = sink.send_event(&doc, &sentiment, 1000).await;
        assert!(res.is_err(), "Expected error when Kafka sink is disabled");
    }
}
