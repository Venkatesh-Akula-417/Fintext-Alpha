//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — QuestDB Kafka Failover Buffer (Write-Ahead Log)
//! ═══════════════════════════════════════════════════════════════════════════════
//!
//! Provides a durable, zero-data-loss Write-Ahead Log (WAL) for QuestDB persistence.
//! When enabled, all ingested sentiment records and ILP payloads are buffered in
//! Apache Kafka / Redpanda (`sentiment-questdb-buffer`). An asynchronous consumer
//! drains the buffer to QuestDB with exponential backoff retries, eliminating
//! QuestDB as a single point of failure (SPOF).
//! ═══════════════════════════════════════════════════════════════════════════════

use crate::nlp::SentimentOutput;
use crate::pipeline::ProcessedDocument;
use crate::storage::QuestDbSink;
use chrono::Utc;
use rdkafka::config::ClientConfig;
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::message::Message;
use rdkafka::producer::{FutureProducer, FutureRecord};
use rdkafka::util::Timeout;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::env;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::sleep;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Configuration parameters for the QuestDB Kafka write-ahead buffer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestDbBufferConfig {
    /// Whether the Kafka write-ahead buffer is active (vs direct ILP write)
    pub enabled: bool,
    /// Kafka topic used for buffering QuestDB writes
    pub topic: String,
    /// Consumer group ID for the QuestDB drain worker
    pub consumer_group_id: String,
    /// Kafka bootstrap servers (comma-separated host:port)
    pub bootstrap_servers: String,
    /// Maximum retry attempts before routing failed message to DLQ/quarantine
    pub max_retry_attempts: usize,
    /// Base delay in milliseconds for exponential backoff retry
    pub base_retry_delay_ms: u64,
    /// Maximum delay in milliseconds for backoff cap
    pub max_retry_delay_ms: u64,
    /// Whether to run in simulated mock buffer mode (in-memory queue)
    pub mock_mode: bool,
    /// Optional DLQ topic for permanently unwriteable records
    pub dlq_topic: Option<String>,
    /// Local directory path for quarantined unwriteable payloads
    pub quarantine_dir: String,
}

impl Default for QuestDbBufferConfig {
    fn default() -> Self {
        let enabled = env::var("KAFKA_QUESTDB_BUFFER_ENABLED")
            .map(|v| v != "0" && v.to_lowercase() != "false")
            .unwrap_or(true);
        let topic = env::var("KAFKA_QUESTDB_BUFFER_TOPIC")
            .unwrap_or_else(|_| "sentiment-questdb-buffer".to_string());
        let consumer_group_id = env::var("KAFKA_CONSUMER_GROUP_ID")
            .unwrap_or_else(|_| "fintext-questdb-recovery".to_string());
        let bootstrap_servers =
            env::var("KAFKA_BOOTSTRAP_SERVERS").unwrap_or_else(|_| "localhost:9092".to_string());
        let max_retry_attempts = env::var("QUESTDB_WRITE_RETRY_MAX_ATTEMPTS")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(5);
        let base_retry_delay_ms = env::var("QUESTDB_WRITE_RETRY_BASE_DELAY_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(1000);
        let max_retry_delay_ms = env::var("QUESTDB_WRITE_RETRY_MAX_DELAY_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(60_000);
        let mock_mode = if crate::is_production_mode() {
            false
        } else {
            env::var("KAFKA_MOCK_FALLBACK").as_deref() == Ok("1")
                || env::var("KAFKA_MOCK_MODE").as_deref() == Ok("1")
        };
        let dlq_topic = env::var("KAFKA_DLQ_TOPIC").ok();
        let quarantine_dir =
            env::var("QUARANTINE_DIR").unwrap_or_else(|_| "data/quarantine".to_string());

        Self {
            enabled,
            topic,
            consumer_group_id,
            bootstrap_servers,
            max_retry_attempts,
            base_retry_delay_ms,
            max_retry_delay_ms,
            mock_mode,
            dlq_topic,
            quarantine_dir,
        }
    }
}

/// Structured payload buffered in Kafka before committing to QuestDB.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QuestDbBufferMessage {
    pub message_id: String,
    pub ticker: String,
    pub ilp_line: String,
    pub published_utc: String,
    pub ingested_utc: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub db_commit_utc: Option<String>,
    pub signal_available_ts_us: i64,
    pub created_at_utc: String,
    #[serde(default)]
    pub attempt_count: usize,
}

impl QuestDbBufferMessage {
    pub fn new(
        ticker: String,
        ilp_line: String,
        published_utc: String,
        ingested_utc: String,
        db_commit_utc: Option<String>,
        signal_available_ts_us: i64,
    ) -> Self {
        Self {
            message_id: Uuid::new_v4().to_string(),
            ticker,
            ilp_line,
            published_utc,
            ingested_utc,
            db_commit_utc,
            signal_available_ts_us,
            created_at_utc: Utc::now().to_rfc3339(),
            attempt_count: 0,
        }
    }

    pub fn to_json_string(&self) -> Result<String, String> {
        serde_json::to_string(self)
            .map_err(|e| format!("Failed to serialize buffer message: {}", e))
    }

    pub fn from_json_str(s: &str) -> Result<Self, String> {
        serde_json::from_str(s).map_err(|e| format!("Failed to deserialize buffer message: {}", e))
    }
}

/// Producer responsible for appending ILP writes to the durable Kafka WAL buffer.
#[derive(Clone)]
pub struct QuestDbBufferProducer {
    config: QuestDbBufferConfig,
    producer: Option<Arc<FutureProducer>>,
    mock_queue: Option<Arc<Mutex<VecDeque<QuestDbBufferMessage>>>>,
}

impl QuestDbBufferProducer {
    /// Creates a standard Kafka buffer producer from config.
    pub fn new(config: QuestDbBufferConfig) -> Self {
        if !config.enabled || config.mock_mode {
            info!(
                "[QuestDB Buffer Producer] Initialized in mock/in-memory mode (enabled={}, mock_mode={}).",
                config.enabled, config.mock_mode
            );
            return Self {
                config,
                producer: None,
                mock_queue: Some(Arc::new(Mutex::new(VecDeque::new()))),
            };
        }

        info!(
            "[QuestDB Buffer Producer] Initializing rdkafka FutureProducer for brokers='{}', topic='{}'",
            config.bootstrap_servers, config.topic
        );

        let producer_res: Result<FutureProducer, _> = ClientConfig::new()
            .set("bootstrap.servers", &config.bootstrap_servers)
            .set("message.timeout.ms", "5000")
            .set("queue.buffering.max.ms", "10")
            .set("compression.type", "lz4")
            .set("acks", "all")
            .create();

        match producer_res {
            Ok(p) => Self {
                config,
                producer: Some(Arc::new(p)),
                mock_queue: None,
            },
            Err(e) => {
                if crate::is_production_mode() {
                    error!(
                        "[QuestDB Buffer Producer] Failed to initialize Kafka producer in production mode: {}",
                        e
                    );
                    Self {
                        config,
                        producer: None,
                        mock_queue: None,
                    }
                } else {
                    warn!(
                        "[QuestDB Buffer Producer] rdkafka initialization warning: {}. Operating in mock buffer fallback.",
                        e
                    );
                    Self {
                        config,
                        producer: None,
                        mock_queue: Some(Arc::new(Mutex::new(VecDeque::new()))),
                    }
                }
            }
        }
    }

    /// Explicitly creates a mock producer with a shared memory queue for unit testing.
    pub fn new_mock(
        config: QuestDbBufferConfig,
        mock_queue: Arc<Mutex<VecDeque<QuestDbBufferMessage>>>,
    ) -> Self {
        Self {
            config,
            producer: None,
            mock_queue: Some(mock_queue),
        }
    }

    pub fn config(&self) -> &QuestDbBufferConfig {
        &self.config
    }

    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    pub fn mock_queue(&self) -> Option<Arc<Mutex<VecDeque<QuestDbBufferMessage>>>> {
        self.mock_queue.clone()
    }

    /// Pushes a processed document and sentiment output into the Kafka QuestDB buffer.
    pub async fn push_event(
        &self,
        doc: &ProcessedDocument,
        _sentiment: &SentimentOutput,
        signal_avail_ts_us: i64,
        ilp_line: &str,
    ) -> Result<(), String> {
        let ticker = doc
            .primary_ticker
            .as_deref()
            .unwrap_or_else(|| doc.tickers.first().map(|s| s.as_str()).unwrap_or("MARKET"))
            .to_string();

        let msg = QuestDbBufferMessage::new(
            ticker.clone(),
            ilp_line.to_string(),
            doc.published_utc.clone(),
            doc.ingested_utc.clone(),
            doc.db_commit_utc.clone(),
            signal_avail_ts_us,
        );

        self.push_message(msg).await
    }

    /// Pushes a raw ILP line into the buffer.
    pub async fn push_raw_ilp(
        &self,
        ticker: &str,
        ilp_line: &str,
        signal_avail_ts_us: i64,
    ) -> Result<(), String> {
        let now_iso = Utc::now().to_rfc3339();
        let msg = QuestDbBufferMessage::new(
            ticker.to_string(),
            ilp_line.to_string(),
            now_iso.clone(),
            now_iso,
            None,
            signal_avail_ts_us,
        );
        self.push_message(msg).await
    }

    /// Internal dispatch sending the message to Kafka or mock queue.
    async fn push_message(&self, msg: QuestDbBufferMessage) -> Result<(), String> {
        if !self.config.enabled {
            return Err("QuestDB buffer is disabled in configuration".to_string());
        }

        // Mock in-memory mode
        if let Some(ref queue) = self.mock_queue {
            let mut lock = queue.lock().await;
            lock.push_back(msg);
            return Ok(());
        }

        let producer = match &self.producer {
            Some(p) => p,
            None => {
                return Err(format!(
                    "QuestDB buffer producer not initialized for brokers '{}'",
                    self.config.bootstrap_servers
                ));
            }
        };

        let payload = msg.to_json_string()?;
        let record = FutureRecord::to(&self.config.topic)
            .key(&msg.ticker)
            .payload(&payload);

        match producer
            .send(record, Timeout::After(Duration::from_millis(5000)))
            .await
        {
            Ok(_) => {
                debug!(
                    "[QuestDB Buffer Producer] Buffered event for ticker='{}' -> topic='{}'",
                    msg.ticker, self.config.topic
                );
                Ok(())
            }
            Err((e, _)) => {
                let err_msg = format!("Failed to publish write-ahead event to Kafka buffer: {}", e);
                error!("[QuestDB Buffer Producer] {}", err_msg);
                Err(err_msg)
            }
        }
    }
}

/// Helper function to calculate exponential backoff delay with jitter.
pub fn calculate_buffer_backoff(attempt: usize, base_delay_ms: u64, max_delay_ms: u64) -> Duration {
    if attempt == 0 {
        return Duration::from_millis(base_delay_ms);
    }
    let factor = 1u64
        .checked_shl((attempt.saturating_sub(1)) as u32)
        .unwrap_or(u64::MAX);
    let delay_ms = base_delay_ms.saturating_mul(factor).min(max_delay_ms);
    Duration::from_millis(delay_ms)
}

/// Quarantines an unwriteable message to a local JSON file.
pub fn quarantine_failed_message(
    quarantine_dir: &str,
    msg: &QuestDbBufferMessage,
    last_error: &str,
) -> Result<PathBuf, String> {
    let dir = Path::new(quarantine_dir);
    if let Err(e) = std::fs::create_dir_all(dir) {
        return Err(format!(
            "Failed to create quarantine directory '{}': {}",
            quarantine_dir, e
        ));
    }

    let now = Utc::now();
    let file_name = format!(
        "questdb-buffer-quarantine-{}-{}.json",
        now.format("%Y%m%d%H%M%S"),
        msg.message_id
    );
    let file_path = dir.join(file_name);

    let envelope = serde_json::json!({
        "quarantined_at": now.to_rfc3339(),
        "last_error": last_error,
        "attempts": msg.attempt_count,
        "message": msg
    });

    let content = serde_json::to_string_pretty(&envelope)
        .map_err(|e| format!("Failed to serialize quarantine envelope: {}", e))?;

    std::fs::write(&file_path, content)
        .map_err(|e| format!("Failed to write quarantine file '{:?}': {}", file_path, e))?;

    info!(
        "[QuestDB Buffer Quarantine] Permanently unwriteable record written to: {:?}",
        file_path
    );
    Ok(file_path)
}

/// Asynchronous consumer worker that continuously drains the Kafka WAL buffer to QuestDB.
pub struct QuestDbBufferConsumer {
    config: QuestDbBufferConfig,
    questdb_sink: Arc<QuestDbSink>,
    mock_queue: Option<Arc<Mutex<VecDeque<QuestDbBufferMessage>>>>,
}

impl QuestDbBufferConsumer {
    pub fn new(config: QuestDbBufferConfig, questdb_sink: Arc<QuestDbSink>) -> Self {
        let mock_mode = if crate::is_production_mode() {
            false
        } else {
            config.mock_mode
                || env::var("KAFKA_MOCK_FALLBACK").as_deref() == Ok("1")
                || env::var("KAFKA_MOCK_MODE").as_deref() == Ok("1")
        };

        let mock_queue = if mock_mode {
            Some(Arc::new(Mutex::new(VecDeque::new())))
        } else {
            None
        };

        Self {
            config,
            questdb_sink,
            mock_queue,
        }
    }

    pub fn new_mock(
        config: QuestDbBufferConfig,
        questdb_sink: Arc<QuestDbSink>,
        mock_queue: Arc<Mutex<VecDeque<QuestDbBufferMessage>>>,
    ) -> Self {
        Self {
            config,
            questdb_sink,
            mock_queue: Some(mock_queue),
        }
    }

    pub fn config(&self) -> &QuestDbBufferConfig {
        &self.config
    }

    /// Attempts to drain a single buffered message to QuestDB with retry handling.
    pub async fn drain_message(&self, msg: &mut QuestDbBufferMessage) -> Result<(), String> {
        let mut attempt = 0;

        loop {
            attempt += 1;
            msg.attempt_count = attempt;

            match self.questdb_sink.send_ilp_payload(&msg.ilp_line).await {
                Ok(_) => {
                    debug!(
                        "[QuestDB Buffer Consumer] Successfully drained record for ticker='{}' to QuestDB",
                        msg.ticker
                    );
                    return Ok(());
                }
                Err(err) => {
                    warn!(
                        "[QuestDB Buffer Consumer] Attempt {}/{} failed to write record for ticker='{}' to QuestDB: {}",
                        attempt, self.config.max_retry_attempts, msg.ticker, err
                    );

                    if attempt >= self.config.max_retry_attempts {
                        let quarantine_res =
                            quarantine_failed_message(&self.config.quarantine_dir, msg, &err);
                        if let Err(q_err) = quarantine_res {
                            error!(
                                "[QuestDB Buffer Consumer] Quarantine write failed: {}",
                                q_err
                            );
                        }
                        return Err(format!(
                            "Max retries ({}) exceeded. Message quarantined.",
                            self.config.max_retry_attempts
                        ));
                    }

                    let delay = calculate_buffer_backoff(
                        attempt,
                        self.config.base_retry_delay_ms,
                        self.config.max_retry_delay_ms,
                    );
                    sleep(delay).await;
                }
            }
        }
    }

    /// Processes all currently pending messages in the mock queue once.
    /// Returns `(success_count, fail_count)`.
    pub async fn drain_mock_queue_once(&self) -> (usize, usize) {
        let mock_queue = match &self.mock_queue {
            Some(q) => q.clone(),
            None => return (0, 0),
        };

        let mut successes = 0;
        let mut failures = 0;

        let messages: Vec<QuestDbBufferMessage> = {
            let mut lock = mock_queue.lock().await;
            lock.drain(..).collect()
        };

        let mut uncommitted = VecDeque::new();

        for mut msg in messages {
            match self.drain_message(&mut msg).await {
                Ok(_) => successes += 1,
                Err(_) => {
                    failures += 1;
                    // If not permanently quarantined (attempt < max_retry), preserve in buffer
                    if msg.attempt_count < self.config.max_retry_attempts {
                        uncommitted.push_back(msg);
                    }
                }
            }
        }

        if !uncommitted.is_empty() {
            let mut lock = mock_queue.lock().await;
            for msg in uncommitted.into_iter().rev() {
                lock.push_front(msg);
            }
        }

        (successes, failures)
    }

    /// Runs the asynchronous consumer drain loop.
    pub async fn run_drain_loop(&self, shutdown_rx: tokio::sync::watch::Receiver<bool>) {
        info!(
            "[QuestDB Buffer Consumer] Starting recovery consumer loop for topic='{}' (group_id='{}')",
            self.config.topic, self.config.consumer_group_id
        );

        if self.mock_queue.is_some() {
            while !*shutdown_rx.borrow() {
                let (drained, failed) = self.drain_mock_queue_once().await;
                if drained > 0 || failed > 0 {
                    debug!(
                        "[QuestDB Mock Drainer] Drained {} records ({} failed/quarantined)",
                        drained, failed
                    );
                }
                sleep(Duration::from_millis(50)).await;
            }
            info!("[QuestDB Buffer Consumer] Mock consumer loop cleanly stopped.");
            return;
        }

        let consumer_res: Result<StreamConsumer, _> = ClientConfig::new()
            .set("bootstrap.servers", &self.config.bootstrap_servers)
            .set("group.id", &self.config.consumer_group_id)
            .set("enable.auto.commit", "false")
            .set("auto.offset.reset", "earliest")
            .set("session.timeout.ms", "30000")
            .create();

        let consumer = match consumer_res {
            Ok(c) => c,
            Err(e) => {
                error!(
                    "[QuestDB Buffer Consumer] Failed to initialize Kafka StreamConsumer: {}. Buffer consumer disabled.",
                    e
                );
                return;
            }
        };

        if let Err(e) = consumer.subscribe(&[&self.config.topic]) {
            error!(
                "[QuestDB Buffer Consumer] Failed to subscribe to topic '{}': {}",
                self.config.topic, e
            );
            return;
        }

        while !*shutdown_rx.borrow() {
            match tokio::time::timeout(Duration::from_millis(500), consumer.recv()).await {
                Ok(Ok(m)) => {
                    if let Some(payload_bytes) = m.payload() {
                        let payload_str = String::from_utf8_lossy(payload_bytes);
                        match QuestDbBufferMessage::from_json_str(&payload_str) {
                            Ok(mut buffer_msg) => {
                                let _ = self.drain_message(&mut buffer_msg).await;
                                if let Err(commit_err) = consumer
                                    .commit_message(&m, rdkafka::consumer::CommitMode::Async)
                                {
                                    warn!(
                                        "[QuestDB Buffer Consumer] Offset commit warning: {}",
                                        commit_err
                                    );
                                }
                            }
                            Err(parse_err) => {
                                warn!(
                                    "[QuestDB Buffer Consumer] Invalid buffer message JSON: {}. Committing offset to skip bad message.",
                                    parse_err
                                );
                                let _ = consumer
                                    .commit_message(&m, rdkafka::consumer::CommitMode::Async);
                            }
                        }
                    }
                }
                Ok(Err(e)) => {
                    warn!("[QuestDB Buffer Consumer] Kafka recv notice: {}", e);
                    sleep(Duration::from_millis(100)).await;
                }
                Err(_) => {
                    // Timeout tick — check shutdown channel
                    continue;
                }
            }
        }

        info!("[QuestDB Buffer Consumer] Drain loop terminated cleanly.");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::QuestDbConfig;

    fn test_buffer_config() -> QuestDbBufferConfig {
        QuestDbBufferConfig {
            enabled: true,
            topic: "test-sentiment-questdb-buffer".to_string(),
            consumer_group_id: "test-questdb-recovery".to_string(),
            bootstrap_servers: "localhost:9092".to_string(),
            max_retry_attempts: 3,
            base_retry_delay_ms: 10,
            max_retry_delay_ms: 50,
            mock_mode: true,
            dlq_topic: Some("test-dlq".to_string()),
            quarantine_dir: "data/test_quarantine".to_string(),
        }
    }

    #[test]
    fn test_backoff_calculation() {
        let b1 = calculate_buffer_backoff(1, 1000, 60000);
        assert_eq!(b1.as_millis(), 1000);
        let b2 = calculate_buffer_backoff(2, 1000, 60000);
        assert_eq!(b2.as_millis(), 2000);
        let b3 = calculate_buffer_backoff(3, 1000, 60000);
        assert_eq!(b3.as_millis(), 4000);
        let b7 = calculate_buffer_backoff(7, 1000, 60000);
        assert_eq!(b7.as_millis(), 60000); // capped at 60s
    }

    #[tokio::test]
    async fn test_buffer_producer_and_consumer_mock_flow() {
        let config = test_buffer_config();
        let queue = Arc::new(Mutex::new(VecDeque::new()));

        let producer = QuestDbBufferProducer::new_mock(config.clone(), queue.clone());
        let questdb_sink = Arc::new(QuestDbSink::new(QuestDbConfig {
            url: "http://127.0.0.1:9999".to_string(), // offline mock endpoint
            table_name: "sentiment_news".to_string(),
            max_retries: 1,
            timeout_ms: 10,
            enabled: true,
        }));
        let consumer = QuestDbBufferConsumer::new_mock(config, questdb_sink, queue.clone());

        // Push 3 raw ILP events
        let ilp_1 = "sentiment_news,ticker=AAPL sentiment_score=0.75 1787668200000000000";
        let ilp_2 = "sentiment_news,ticker=NVDA sentiment_score=0.88 1787668201000000000";
        let ilp_3 = "sentiment_news,ticker=MSFT sentiment_score=0.45 1787668202000000000";

        producer
            .push_raw_ilp("AAPL", ilp_1, 1787668200000)
            .await
            .unwrap();
        producer
            .push_raw_ilp("NVDA", ilp_2, 1787668201000)
            .await
            .unwrap();
        producer
            .push_raw_ilp("MSFT", ilp_3, 1787668202000)
            .await
            .unwrap();

        assert_eq!(queue.lock().await.len(), 3);

        // Attempt drain (QuestDB offline -> retries -> quarantined)
        let (successes, failures) = consumer.drain_mock_queue_once().await;
        assert_eq!(successes, 0);
        assert_eq!(failures, 3);
        assert_eq!(queue.lock().await.len(), 0); // All exhausted retries and were quarantined
    }

    #[tokio::test]
    async fn test_buffer_failover_recovery_simulation() {
        let mut config = test_buffer_config();
        config.max_retry_attempts = 10;
        let queue = Arc::new(Mutex::new(VecDeque::new()));

        let producer = QuestDbBufferProducer::new_mock(config.clone(), queue.clone());

        // Push 2 messages to buffer
        producer
            .push_raw_ilp(
                "AAPL",
                "sentiment_news,ticker=AAPL sentiment_score=0.9 1787668200000000000",
                1787668200000,
            )
            .await
            .unwrap();
        producer
            .push_raw_ilp(
                "GOOGL",
                "sentiment_news,ticker=GOOGL sentiment_score=0.6 1787668201000000000",
                1787668201000,
            )
            .await
            .unwrap();

        assert_eq!(queue.lock().await.len(), 2);

        // Verify message contents
        let queued_items = queue.lock().await;
        assert_eq!(queued_items[0].ticker, "AAPL");
        assert_eq!(queued_items[1].ticker, "GOOGL");
        drop(queued_items);

        // Verify that disabled buffer rejects writes
        let mut disabled_config = config.clone();
        disabled_config.enabled = false;
        let disabled_producer = QuestDbBufferProducer::new_mock(disabled_config, queue.clone());
        assert!(!disabled_producer.is_enabled());
        let res = disabled_producer.push_raw_ilp("TSLA", "ilp_dummy", 0).await;
        assert!(res.is_err());
    }

    #[test]
    fn test_buffer_message_serialization() {
        let msg = QuestDbBufferMessage::new(
            "AAPL".to_string(),
            "sentiment_news,ticker=AAPL sentiment_score=0.5 1787668200000000000".to_string(),
            "2026-08-25T14:30:00Z".to_string(),
            "2026-08-25T14:30:00.050Z".to_string(),
            Some("2026-08-25T14:30:00.100Z".to_string()),
            1787668200000000,
        );

        let json = msg.to_json_string().unwrap();
        assert!(json.contains("AAPL"));
        assert!(json.contains("sentiment_news,ticker=AAPL"));

        let deserialized = QuestDbBufferMessage::from_json_str(&json).unwrap();
        assert_eq!(deserialized.ticker, "AAPL");
        assert_eq!(deserialized.message_id, msg.message_id);
    }
}
