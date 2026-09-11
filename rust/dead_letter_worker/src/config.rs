//! Configuration module for the Dead Letter Queue (DLQ) Auto-Reprocessing Worker.

use std::env;

#[derive(Debug, Clone)]
pub struct DlqConfig {
    /// Kafka bootstrap servers (e.g., "localhost:9092")
    pub kafka_bootstrap_servers: String,
    /// Kafka topic for failed DLQ messages (e.g., "sentiment-dlq")
    pub dlq_topic: String,
    /// Kafka topic for re-routed successful reprocessing (e.g., "sentiment-updates")
    pub reprocess_topic: String,
    /// Kafka consumer group ID for DLQ worker
    pub group_id: String,
    /// AWS S3 quarantine bucket name for permanently failed messages
    pub s3_quarantine_bucket: String,
    /// S3 key prefix for quarantined messages
    pub s3_quarantine_prefix: String,
    /// Local fallback directory when S3 is unavailable or unconfigured
    pub local_quarantine_dir: String,
    /// Maximum retry attempts before routing to quarantine (default: 5)
    pub max_retries: usize,
    /// Initial backoff delay in milliseconds for exponential backoff (default: 100ms)
    pub initial_backoff_ms: u64,
    /// Maximum backoff ceiling in milliseconds (default: 5000ms)
    pub max_backoff_ms: u64,
    /// Mock mode flag for offline testing and verification without external infrastructure
    pub mock_mode: bool,
    /// Maximum concurrent message processing workers
    pub concurrency_limit: usize,
}

impl Default for DlqConfig {
    fn default() -> Self {
        Self {
            kafka_bootstrap_servers: "localhost:9092".to_string(),
            dlq_topic: "sentiment-dlq".to_string(),
            reprocess_topic: "sentiment-updates".to_string(),
            group_id: "fintext-dlq-worker".to_string(),
            s3_quarantine_bucket: "fintext-dlq-quarantine".to_string(),
            s3_quarantine_prefix: "dlq/".to_string(),
            local_quarantine_dir: "data/quarantine/".to_string(),
            max_retries: 5,
            initial_backoff_ms: 100,
            max_backoff_ms: 5000,
            mock_mode: false,
            concurrency_limit: 4,
        }
    }
}

impl DlqConfig {
    /// Load configuration from environment variables with safe defaults.
    pub fn from_env() -> Self {
        let defaults = Self::default();

        let kafka_bootstrap_servers = env::var("KAFKA_BOOTSTRAP_SERVERS")
            .unwrap_or(defaults.kafka_bootstrap_servers);
        let dlq_topic = env::var("KAFKA_DLQ_TOPIC")
            .or_else(|_| env::var("DLQ_TOPIC"))
            .unwrap_or(defaults.dlq_topic);
        let reprocess_topic = env::var("KAFKA_REPROCESS_TOPIC")
            .or_else(|_| env::var("DLQ_REPROCESS_TOPIC"))
            .unwrap_or(defaults.reprocess_topic);
        let group_id = env::var("KAFKA_GROUP_ID")
            .or_else(|_| env::var("DLQ_GROUP_ID"))
            .unwrap_or(defaults.group_id);

        let s3_quarantine_bucket =
            env::var("S3_QUARANTINE_BUCKET").unwrap_or(defaults.s3_quarantine_bucket);
        let s3_quarantine_prefix =
            env::var("S3_QUARANTINE_PREFIX").unwrap_or(defaults.s3_quarantine_prefix);
        let local_quarantine_dir =
            env::var("LOCAL_QUARANTINE_DIR").unwrap_or(defaults.local_quarantine_dir);

        let max_retries = env::var("DLQ_MAX_RETRIES")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(defaults.max_retries);

        let initial_backoff_ms = env::var("DLQ_INITIAL_BACKOFF_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(defaults.initial_backoff_ms);

        let max_backoff_ms = env::var("DLQ_MAX_BACKOFF_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(defaults.max_backoff_ms);

        let mock_mode = env::var("KAFKA_MOCK_FALLBACK").as_deref() == Ok("1")
            || env::var("KAFKA_MOCK_MODE").as_deref() == Ok("1")
            || env::var("DLQ_MOCK_MODE").map(|v| v == "1" || v.eq_ignore_ascii_case("true")).unwrap_or(false);

        let concurrency_limit = env::var("DLQ_CONCURRENCY_LIMIT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(defaults.concurrency_limit);

        Self {
            kafka_bootstrap_servers,
            dlq_topic,
            reprocess_topic,
            group_id,
            s3_quarantine_bucket,
            s3_quarantine_prefix,
            local_quarantine_dir,
            max_retries,
            initial_backoff_ms,
            max_backoff_ms,
            mock_mode,
            concurrency_limit,
        }
    }
}
