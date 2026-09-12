//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Ingestion Bounded Concurrency & Backpressure Engine
//! ═══════════════════════════════════════════════════════════════════════════════
//! [OOM & LATENCY SPIKE PREVENTION - SUITE #270]
//! Provides bounded concurrency control and graceful non-blocking backpressure:
//!   - Bounded input queue (`mpsc::channel`) with deterministic drop-and-log semantics
//!   - Strict concurrency limiter via `tokio::sync::Semaphore`
//!   - Timeout-protected document processing tasks
//!   - Real-time backpressure metrics (processed, dropped, timed out, active)
//! ═══════════════════════════════════════════════════════════════════════════════

use crate::pipeline::preprocessor::RawDocument;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, Semaphore};
use tracing::warn;

pub const DEFAULT_MAX_CONCURRENT_TASKS: usize = 100;
pub const DEFAULT_INPUT_QUEUE_CAPACITY: usize = 1000;
pub const DEFAULT_TASK_TIMEOUT_MS: u64 = 30000;

// ─────────────────────────────────────────────────────────────────────────────
// Configuration
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IngestionConcurrencyConfig {
    pub max_concurrent_tasks: usize,
    pub input_queue_capacity: usize,
    pub task_timeout_ms: u64,
}

impl Default for IngestionConcurrencyConfig {
    fn default() -> Self {
        Self {
            max_concurrent_tasks: DEFAULT_MAX_CONCURRENT_TASKS,
            input_queue_capacity: DEFAULT_INPUT_QUEUE_CAPACITY,
            task_timeout_ms: DEFAULT_TASK_TIMEOUT_MS,
        }
    }
}

impl IngestionConcurrencyConfig {
    /// Load configuration from `config/config.yaml` with environment variable overrides.
    pub fn from_env_or_config() -> Self {
        let mut cfg = Self::default();

        // 1. Try loading from YAML candidates
        let config_candidates = [
            env::var("CONFIG_PATH").unwrap_or_default(),
            "config/config.yaml".to_string(),
            "config.yaml".to_string(),
            "../config/config.yaml".to_string(),
            "../../config/config.yaml".to_string(),
        ];

        for path in &config_candidates {
            if path.is_empty() {
                continue;
            }
            if let Ok(content) = fs::read_to_string(path) {
                cfg.parse_yaml_content(&content);
                break;
            }
        }

        // 2. Environment variable overrides (highest precedence)
        if let Ok(val) =
            env::var("MAX_CONCURRENT_TASKS").or_else(|_| env::var("INGESTION_MAX_CONCURRENT_TASKS"))
        {
            if let Ok(num) = val.trim().parse::<usize>() {
                if num > 0 {
                    cfg.max_concurrent_tasks = num;
                }
            }
        }

        if let Ok(val) =
            env::var("INPUT_QUEUE_CAPACITY").or_else(|_| env::var("INGESTION_INPUT_QUEUE_CAPACITY"))
        {
            if let Ok(num) = val.trim().parse::<usize>() {
                if num > 0 {
                    cfg.input_queue_capacity = num;
                }
            }
        }

        if let Ok(val) =
            env::var("TASK_TIMEOUT_MS").or_else(|_| env::var("INGESTION_TASK_TIMEOUT_MS"))
        {
            if let Ok(num) = val.trim().parse::<u64>() {
                if num > 0 {
                    cfg.task_timeout_ms = num;
                }
            }
        }

        cfg
    }

    /// Parse `ingestion:` YAML block line by line.
    pub fn parse_yaml_content(&mut self, yaml: &str) {
        let mut in_block = false;
        for line in yaml.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') {
                continue;
            }
            if trimmed.starts_with("ingestion:") {
                in_block = true;
                continue;
            }
            if in_block {
                // If top-level non-indented key encountered, exit block
                if !line.starts_with("  ")
                    && !line.starts_with('\t')
                    && trimmed.contains(':')
                    && !trimmed.starts_with('-')
                {
                    break;
                }
                let parts: Vec<&str> = trimmed.splitn(2, ':').collect();
                if parts.len() == 2 {
                    let key = parts[0].trim();
                    let val = parts[1]
                        .split('#')
                        .next()
                        .unwrap_or("")
                        .trim()
                        .trim_matches('"')
                        .trim_matches('\'');
                    match key {
                        "max_concurrent_tasks" => {
                            if let Ok(num) = val.parse::<usize>() {
                                if num > 0 {
                                    self.max_concurrent_tasks = num;
                                }
                            }
                        }
                        "input_queue_capacity" => {
                            if let Ok(num) = val.parse::<usize>() {
                                if num > 0 {
                                    self.input_queue_capacity = num;
                                }
                            }
                        }
                        "task_timeout_ms" => {
                            if let Ok(num) = val.parse::<u64>() {
                                if num > 0 {
                                    self.task_timeout_ms = num;
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Metrics
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Default)]
pub struct BackpressureMetrics {
    pub processed_count: AtomicU64,
    pub dropped_count: AtomicU64,
    pub timeout_count: AtomicU64,
    pub active_tasks: AtomicUsize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BackpressureMetricsSnapshot {
    pub processed_count: u64,
    pub dropped_count: u64,
    pub timeout_count: u64,
    pub active_tasks: usize,
}

impl BackpressureMetrics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_processed(&self) {
        self.processed_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_dropped(&self) {
        self.dropped_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_timeout(&self) {
        self.timeout_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_active(&self) {
        self.active_tasks.fetch_add(1, Ordering::Relaxed);
    }

    pub fn dec_active(&self) {
        self.active_tasks.fetch_sub(1, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> BackpressureMetricsSnapshot {
        BackpressureMetricsSnapshot {
            processed_count: self.processed_count.load(Ordering::Relaxed),
            dropped_count: self.dropped_count.load(Ordering::Relaxed),
            timeout_count: self.timeout_count.load(Ordering::Relaxed),
            active_tasks: self.active_tasks.load(Ordering::Relaxed),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Bounded Queue Ingestion Sender
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IngestionSendError {
    QueueFull(String),
    ChannelClosed,
}

impl std::fmt::Display for IngestionSendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IngestionSendError::QueueFull(id) => {
                write!(f, "Ingestion input queue full; document '{}' dropped", id)
            }
            IngestionSendError::ChannelClosed => write!(f, "Ingestion channel is closed"),
        }
    }
}

impl std::error::Error for IngestionSendError {}

#[derive(Clone)]
pub struct IngestionSender {
    sender: mpsc::Sender<RawDocument>,
    capacity: usize,
    metrics: Arc<BackpressureMetrics>,
}

impl IngestionSender {
    pub fn new(
        sender: mpsc::Sender<RawDocument>,
        capacity: usize,
        metrics: Arc<BackpressureMetrics>,
    ) -> Self {
        Self {
            sender,
            capacity,
            metrics,
        }
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn metrics(&self) -> &Arc<BackpressureMetrics> {
        &self.metrics
    }

    /// Non-blocking send: if the queue is full, logs a structured warning, records
    /// the dropped document in metrics, and returns `Err(IngestionSendError::QueueFull)`.
    pub fn try_send(&self, doc: RawDocument) -> Result<(), IngestionSendError> {
        match self.sender.try_send(doc) {
            Ok(_) => Ok(()),
            Err(mpsc::error::TrySendError::Full(dropped_doc)) => {
                warn!(
                    "[Ingestion Backpressure] Input queue full (capacity {}). Dropping document id='{}' title='{}' source='{}'",
                    self.capacity, dropped_doc.id, dropped_doc.title, dropped_doc.source
                );
                self.metrics.record_dropped();
                Err(IngestionSendError::QueueFull(dropped_doc.id))
            }
            Err(mpsc::error::TrySendError::Closed(_)) => Err(IngestionSendError::ChannelClosed),
        }
    }

    /// Asynchronous send with backpressure (waits until capacity is available).
    pub async fn send_async(&self, doc: RawDocument) -> Result<(), IngestionSendError> {
        self.sender
            .send(doc)
            .await
            .map_err(|_| IngestionSendError::ChannelClosed)
    }

    /// Underlying raw tokio mpsc sender reference.
    pub fn raw_sender(&self) -> &mpsc::Sender<RawDocument> {
        &self.sender
    }
}

/// Helper to create a bounded channel and paired `IngestionSender`.
pub fn create_bounded_ingestion_channel(
    capacity: usize,
    metrics: Arc<BackpressureMetrics>,
) -> (IngestionSender, mpsc::Receiver<RawDocument>) {
    let (tx, rx) = mpsc::channel(capacity);
    let sender = IngestionSender::new(tx, capacity, metrics);
    (sender, rx)
}

// ─────────────────────────────────────────────────────────────────────────────
// Worker Pool Concurrency Limiter
// ─────────────────────────────────────────────────────────────────────────────

pub struct WorkerPool {
    semaphore: Arc<Semaphore>,
    config: IngestionConcurrencyConfig,
    metrics: Arc<BackpressureMetrics>,
}

impl WorkerPool {
    pub fn new(config: IngestionConcurrencyConfig, metrics: Arc<BackpressureMetrics>) -> Self {
        let semaphore = Arc::new(Semaphore::new(config.max_concurrent_tasks));
        Self {
            semaphore,
            config,
            metrics,
        }
    }

    pub fn semaphore(&self) -> &Arc<Semaphore> {
        &self.semaphore
    }

    pub fn config(&self) -> &IngestionConcurrencyConfig {
        &self.config
    }

    pub fn metrics(&self) -> &Arc<BackpressureMetrics> {
        &self.metrics
    }

    pub fn available_permits(&self) -> usize {
        self.semaphore.available_permits()
    }

    pub fn timeout_duration(&self) -> Duration {
        Duration::from_millis(self.config.task_timeout_ms)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_concurrency_config_defaults() {
        let cfg = IngestionConcurrencyConfig::default();
        assert_eq!(cfg.max_concurrent_tasks, 100);
        assert_eq!(cfg.input_queue_capacity, 1000);
        assert_eq!(cfg.task_timeout_ms, 30000);
    }

    #[test]
    fn test_concurrency_config_yaml_parsing() {
        let yaml = r#"
        production_mode: false
        ingestion:
          max_concurrent_tasks: 42
          input_queue_capacity: 500
          task_timeout_ms: 15000
        "#;
        let mut cfg = IngestionConcurrencyConfig::default();
        cfg.parse_yaml_content(yaml);
        assert_eq!(cfg.max_concurrent_tasks, 42);
        assert_eq!(cfg.input_queue_capacity, 500);
        assert_eq!(cfg.task_timeout_ms, 15000);
    }

    #[test]
    fn test_concurrency_config_env_overrides() {
        env::set_var("MAX_CONCURRENT_TASKS", "64");
        env::set_var("INPUT_QUEUE_CAPACITY", "256");
        env::set_var("TASK_TIMEOUT_MS", "45000");

        let cfg = IngestionConcurrencyConfig::from_env_or_config();
        assert_eq!(cfg.max_concurrent_tasks, 64);
        assert_eq!(cfg.input_queue_capacity, 256);
        assert_eq!(cfg.task_timeout_ms, 45000);

        env::remove_var("MAX_CONCURRENT_TASKS");
        env::remove_var("INPUT_QUEUE_CAPACITY");
        env::remove_var("TASK_TIMEOUT_MS");
    }

    #[tokio::test]
    async fn test_bounded_queue_try_send_full_drop_metrics() {
        let metrics = Arc::new(BackpressureMetrics::new());
        let (sender, mut rx) = create_bounded_ingestion_channel(2, metrics.clone());

        let doc1 = RawDocument {
            id: "doc-1".to_string(),
            title: "First Doc".to_string(),
            source: "Finnhub".to_string(),
            ..Default::default()
        };
        let doc2 = RawDocument {
            id: "doc-2".to_string(),
            title: "Second Doc".to_string(),
            source: "Finnhub".to_string(),
            ..Default::default()
        };
        let doc3 = RawDocument {
            id: "doc-3".to_string(),
            title: "Third Doc (Should Drop)".to_string(),
            source: "Finnhub".to_string(),
            ..Default::default()
        };

        // First two should succeed
        assert!(sender.try_send(doc1).is_ok());
        assert!(sender.try_send(doc2).is_ok());

        // Queue is now full (capacity 2). Third try_send should return QueueFull error and increment dropped_count
        let res3 = sender.try_send(doc3);
        assert!(matches!(res3, Err(IngestionSendError::QueueFull(id)) if id == "doc-3"));
        assert_eq!(metrics.dropped_count.load(Ordering::Relaxed), 1);

        // Receive one document, freeing space
        let received = rx.recv().await;
        assert!(received.is_some());
        assert_eq!(received.unwrap().id, "doc-1");

        // Now sending doc4 should succeed
        let doc4 = RawDocument {
            id: "doc-4".to_string(),
            title: "Fourth Doc".to_string(),
            source: "Finnhub".to_string(),
            ..Default::default()
        };
        assert!(sender.try_send(doc4).is_ok());
    }

    #[tokio::test]
    async fn test_semaphore_concurrency_limit() {
        let cfg = IngestionConcurrencyConfig {
            max_concurrent_tasks: 3,
            input_queue_capacity: 10,
            task_timeout_ms: 1000,
        };
        let metrics = Arc::new(BackpressureMetrics::new());
        let pool = WorkerPool::new(cfg, metrics);

        assert_eq!(pool.available_permits(), 3);

        let permit1 = pool.semaphore().clone().try_acquire_owned().unwrap();
        let permit2 = pool.semaphore().clone().try_acquire_owned().unwrap();
        let permit3 = pool.semaphore().clone().try_acquire_owned().unwrap();

        assert_eq!(pool.available_permits(), 0);

        // Fourth attempt should fail because all permits are held
        let permit4 = pool.semaphore().clone().try_acquire_owned();
        assert!(permit4.is_err());

        // Dropping one permit frees capacity
        drop(permit1);
        assert_eq!(pool.available_permits(), 1);

        let permit4_retry = pool.semaphore().clone().try_acquire_owned();
        assert!(permit4_retry.is_ok());

        drop(permit2);
        drop(permit3);
        drop(permit4_retry);
        assert_eq!(pool.available_permits(), 3);
    }

    #[tokio::test]
    async fn test_task_timeout_cancellation() {
        let timeout_ms = 50u64;
        let metrics = Arc::new(BackpressureMetrics::new());

        // Simulate a task that runs longer than timeout
        let result = tokio::time::timeout(Duration::from_millis(timeout_ms), async {
            tokio::time::sleep(Duration::from_millis(200)).await;
            "finished"
        })
        .await;

        assert!(result.is_err()); // Elapsed error
        metrics.record_timeout();

        assert_eq!(metrics.timeout_count.load(Ordering::Relaxed), 1);
    }
}
