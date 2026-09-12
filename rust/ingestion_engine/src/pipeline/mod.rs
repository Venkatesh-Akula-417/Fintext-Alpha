pub mod concurrency;
pub mod preprocessor;
pub mod resilience;

pub use concurrency::{
    create_bounded_ingestion_channel, BackpressureMetrics, BackpressureMetricsSnapshot,
    IngestionConcurrencyConfig, IngestionSendError, IngestionSender, WorkerPool,
    DEFAULT_INPUT_QUEUE_CAPACITY, DEFAULT_MAX_CONCURRENT_TASKS, DEFAULT_TASK_TIMEOUT_MS,
};
pub use preprocessor::{Preprocessor, ProcessedDocument, RawDocument, SignalLatencyMetrics};
pub use resilience::{
    CircuitBreakerConfig, DbCircuitBreaker, DbError, STATE_CLOSED, STATE_HALF_OPEN, STATE_OPEN,
};
