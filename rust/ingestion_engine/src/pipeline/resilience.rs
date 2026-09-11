//! Database Circuit Breaker & Retry Policy for Ingestion Engine (Suite #271)
//!
//! Provides a resilient wrapper around TimescaleDB/PostgreSQL writes in the ingestion pipeline.
//! Transitions through Closed -> Open -> HalfOpen -> Closed states to prevent cascading failures
//! when the downstream database cluster is unavailable or degraded.

use std::env;
use std::future::Future;
use std::sync::atomic::{AtomicU32, AtomicU8, Ordering};
use std::time::Duration;
use tracing::{debug, info, warn};

pub const STATE_CLOSED: u8 = 0;
pub const STATE_OPEN: u8 = 1;
pub const STATE_HALF_OPEN: u8 = 2;

/// Database error classification for circuit breaker and retry operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DbError {
    /// Circuit is open; requests fail-fast without attempting DB call.
    CircuitOpen,
    /// Database operation failed after all retry attempts.
    OperationFailed(String),
    /// Operation timed out.
    Timeout,
}

impl std::fmt::Display for DbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DbError::CircuitOpen => write!(f, "Database circuit breaker is open (fail-fast mode)"),
            DbError::OperationFailed(msg) => write!(f, "Database operation failed: {}", msg),
            DbError::Timeout => write!(f, "Database operation timed out"),
        }
    }
}

impl std::error::Error for DbError {}

/// Configuration settings for the database circuit breaker and retry policy.
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    pub enabled: bool,
    pub failure_threshold: u32,
    pub recovery_timeout_ms: u64,
    pub half_open_max_probes: u32,
    pub retry_attempts: u32,
    pub retry_base_delay_ms: u64,
    pub retry_max_delay_ms: u64,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            failure_threshold: 5,
            recovery_timeout_ms: 30000,
            half_open_max_probes: 2,
            retry_attempts: 3,
            retry_base_delay_ms: 100,
            retry_max_delay_ms: 2000,
        }
    }
}

impl CircuitBreakerConfig {
    /// Parses configuration from file and environment variables with precedence:
    /// Environment Variables -> config.yaml -> Built-in Defaults
    pub fn from_env_or_config() -> Self {
        let mut cfg = Self::default();

        let config_paths = [
            env::var("CONFIG_PATH").unwrap_or_default(),
            "config/config.yaml".to_string(),
            "config.yaml".to_string(),
            "../config/config.yaml".to_string(),
            "../../config/config.yaml".to_string(),
        ];

        for path in &config_paths {
            if path.is_empty() {
                continue;
            }
            if let Ok(contents) = std::fs::read_to_string(path) {
                Self::parse_yaml_into_config(&contents, &mut cfg);
                break;
            }
        }

        // Environment variable overrides
        if let Ok(val) = env::var("DB_BREAKER_ENABLED") {
            cfg.enabled = val == "1" || val.eq_ignore_ascii_case("true");
        }
        if let Ok(val) = env::var("DB_BREAKER_FAILURE_THRESHOLD") {
            if let Ok(n) = val.parse::<u32>() {
                cfg.failure_threshold = n;
            }
        }
        if let Ok(val) = env::var("DB_BREAKER_RECOVERY_TIMEOUT_MS") {
            if let Ok(n) = val.parse::<u64>() {
                cfg.recovery_timeout_ms = n;
            }
        }
        if let Ok(val) = env::var("DB_BREAKER_HALF_OPEN_MAX_PROBES") {
            if let Ok(n) = val.parse::<u32>() {
                cfg.half_open_max_probes = n;
            }
        }
        if let Ok(val) = env::var("DB_BREAKER_RETRY_ATTEMPTS") {
            if let Ok(n) = val.parse::<u32>() {
                cfg.retry_attempts = n;
            }
        }
        if let Ok(val) = env::var("DB_BREAKER_RETRY_BASE_DELAY_MS") {
            if let Ok(n) = val.parse::<u64>() {
                cfg.retry_base_delay_ms = n;
            }
        }
        if let Ok(val) = env::var("DB_BREAKER_RETRY_MAX_DELAY_MS") {
            if let Ok(n) = val.parse::<u64>() {
                cfg.retry_max_delay_ms = n;
            }
        }

        cfg
    }

    /// Internal YAML line parser for database.circuit_breaker section
    pub fn parse_yaml_into_config(contents: &str, cfg: &mut CircuitBreakerConfig) {
        let mut in_database = false;
        let mut in_circuit_breaker = false;

        for line in contents.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            if !line.starts_with(' ') && !line.starts_with('\t') {
                if trimmed.starts_with("database:") {
                    in_database = true;
                    in_circuit_breaker = false;
                    continue;
                } else {
                    in_database = false;
                    in_circuit_breaker = false;
                }
            }

            if in_database && (trimmed.starts_with("circuit_breaker:") || trimmed.starts_with("circuit-breaker:")) {
                in_circuit_breaker = true;
                continue;
            }

            if in_database && in_circuit_breaker {
                let leading_spaces = line.chars().take_while(|c| *c == ' ').count();
                if leading_spaces <= 2 && !trimmed.starts_with("circuit_breaker:") {
                    in_circuit_breaker = false;
                    continue;
                }

                if let Some((k, v)) = trimmed.split_once(':') {
                    let key = k.trim();
                    let val = v.split('#').next().unwrap_or("").trim().trim_matches('"');
                    match key {
                        "enabled" => {
                            if val == "true" || val == "1" {
                                cfg.enabled = true;
                            } else if val == "false" || val == "0" {
                                cfg.enabled = false;
                            }
                        }
                        "failure_threshold" => {
                            if let Ok(n) = val.parse::<u32>() {
                                cfg.failure_threshold = n;
                            }
                        }
                        "recovery_timeout_ms" => {
                            if let Ok(n) = val.parse::<u64>() {
                                cfg.recovery_timeout_ms = n;
                            }
                        }
                        "half_open_max_probes" => {
                            if let Ok(n) = val.parse::<u32>() {
                                cfg.half_open_max_probes = n;
                            }
                        }
                        "retry_attempts" => {
                            if let Ok(n) = val.parse::<u32>() {
                                cfg.retry_attempts = n;
                            }
                        }
                        "retry_base_delay_ms" => {
                            if let Ok(n) = val.parse::<u64>() {
                                cfg.retry_base_delay_ms = n;
                            }
                        }
                        "retry_max_delay_ms" => {
                            if let Ok(n) = val.parse::<u64>() {
                                cfg.retry_max_delay_ms = n;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

/// Thread-safe Database Circuit Breaker and Retry Engine.
pub struct DbCircuitBreaker {
    pub state: AtomicU8,
    pub consecutive_failures: AtomicU32,
    pub active_probes: AtomicU32,
    pub last_failure_time: tokio::sync::Mutex<Option<tokio::time::Instant>>,
    pub config: CircuitBreakerConfig,
}

impl DbCircuitBreaker {
    /// Initialize a new circuit breaker with given configuration.
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            state: AtomicU8::new(STATE_CLOSED),
            consecutive_failures: AtomicU32::new(0),
            active_probes: AtomicU32::new(0),
            last_failure_time: tokio::sync::Mutex::new(None),
            config,
        }
    }

    /// Returns human-readable state: "Closed", "Open", or "HalfOpen".
    pub fn current_state(&self) -> &'static str {
        match self.state.load(Ordering::Acquire) {
            STATE_CLOSED => "Closed",
            STATE_OPEN => "Open",
            STATE_HALF_OPEN => "HalfOpen",
            _ => "Unknown",
        }
    }

    /// Returns numerical state constant.
    pub fn state_u8(&self) -> u8 {
        self.state.load(Ordering::Acquire)
    }

    /// Snapshot of current metrics: (state, consecutive_failures, active_probes).
    pub fn metrics_snapshot(&self) -> (u8, u32, u32) {
        (
            self.state.load(Ordering::Acquire),
            self.consecutive_failures.load(Ordering::Acquire),
            self.active_probes.load(Ordering::Acquire),
        )
    }

    /// Returns true if circuit breaker is actively enabled.
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Reset breaker state to Closed with zero failures.
    pub fn reset(&self) {
        self.state.store(STATE_CLOSED, Ordering::Release);
        self.consecutive_failures.store(0, Ordering::Release);
        self.active_probes.store(0, Ordering::Release);
    }

    /// Forcefully trip the breaker to Open state (useful for tests).
    pub async fn trip_to_open(&self) {
        self.state.store(STATE_OPEN, Ordering::Release);
        let mut lock = self.last_failure_time.lock().await;
        *lock = Some(tokio::time::Instant::now());
    }

    /// Executes an asynchronous database operation wrapped with circuit breaking and retries.
    pub async fn execute<F, Fut, T>(&self, operation: F) -> Result<T, DbError>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = Result<T, sqlx::Error>>,
    {
        if !self.config.enabled {
            return operation()
                .await
                .map_err(|e| DbError::OperationFailed(e.to_string()));
        }

        let mut current_state = self.state.load(Ordering::Acquire);

        // Check if Open state has elapsed recovery timeout -> transition to HalfOpen
        if current_state == STATE_OPEN {
            let last_fail_guard = self.last_failure_time.lock().await;
            let elapsed_ok = if let Some(last_fail) = *last_fail_guard {
                last_fail.elapsed() >= Duration::from_millis(self.config.recovery_timeout_ms)
            } else {
                true
            };

            if elapsed_ok {
                self.state.store(STATE_HALF_OPEN, Ordering::Release);
                self.active_probes.store(0, Ordering::Release);
                current_state = STATE_HALF_OPEN;
                info!(
                    "[Database Circuit Breaker] Recovery timeout ({}ms) elapsed. State transitioned: Open -> HalfOpen",
                    self.config.recovery_timeout_ms
                );
            } else {
                return Err(DbError::CircuitOpen);
            }
        }

        // HalfOpen state: allow bounded number of probe operations without retry
        if current_state == STATE_HALF_OPEN {
            let probes = self.active_probes.fetch_add(1, Ordering::SeqCst);
            if probes >= self.config.half_open_max_probes {
                self.active_probes.fetch_sub(1, Ordering::SeqCst);
                return Err(DbError::CircuitOpen);
            }

            let res = operation().await;
            self.active_probes.fetch_sub(1, Ordering::SeqCst);

            return match res {
                Ok(val) => {
                    self.state.store(STATE_CLOSED, Ordering::Release);
                    self.consecutive_failures.store(0, Ordering::Release);
                    info!("[Database Circuit Breaker] Probe succeeded. State transitioned: HalfOpen -> Closed");
                    Ok(val)
                }
                Err(e) => {
                    self.state.store(STATE_OPEN, Ordering::Release);
                    let mut last_fail_guard = self.last_failure_time.lock().await;
                    *last_fail_guard = Some(tokio::time::Instant::now());
                    warn!(
                        "[Database Circuit Breaker] Probe failed: {}. State transitioned: HalfOpen -> Open",
                        e
                    );
                    Err(DbError::OperationFailed(e.to_string()))
                }
            };
        }

        // Closed state: execute with exponential backoff retry policy
        match self.run_with_retry(&operation).await {
            Ok(val) => {
                self.consecutive_failures.store(0, Ordering::Release);
                Ok(val)
            }
            Err(e) => {
                let failures = self.consecutive_failures.fetch_add(1, Ordering::SeqCst) + 1;
                if failures >= self.config.failure_threshold {
                    self.state.store(STATE_OPEN, Ordering::Release);
                    let mut last_fail_guard = self.last_failure_time.lock().await;
                    *last_fail_guard = Some(tokio::time::Instant::now());
                    warn!(
                        "[Database Circuit Breaker] Consecutive failures ({}) reached threshold ({}). State transitioned: Closed -> Open",
                        failures, self.config.failure_threshold
                    );
                }
                Err(DbError::OperationFailed(e.to_string()))
            }
        }
    }

    /// Internal retry logic with exponential backoff for transient errors.
    pub async fn run_with_retry<F, Fut, T>(&self, operation: &F) -> Result<T, sqlx::Error>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = Result<T, sqlx::Error>>,
    {
        let mut attempts = 0;
        loop {
            attempts += 1;
            match operation().await {
                Ok(val) => return Ok(val),
                Err(err) => {
                    if attempts >= self.config.retry_attempts || !Self::is_retryable_error(&err) {
                        return Err(err);
                    }
                    let factor = 1u64.checked_shl(attempts.saturating_sub(1)).unwrap_or(u64::MAX);
                    let delay_ms = (self.config.retry_base_delay_ms.saturating_mul(factor))
                        .min(self.config.retry_max_delay_ms);
                    debug!(
                        "[Database Circuit Breaker] Retryable error (attempt {}/{}): {}. Backing off {}ms",
                        attempts, self.config.retry_attempts, err, delay_ms
                    );
                    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                }
            }
        }
    }

    /// Classify whether an error represents a transient condition eligible for retry.
    pub fn is_retryable_error(err: &sqlx::Error) -> bool {
        match err {
            sqlx::Error::PoolTimedOut => true,
            sqlx::Error::PoolClosed => true,
            sqlx::Error::Io(_) => true,
            sqlx::Error::Database(db_err) => {
                if let Some(code) = db_err.code() {
                    matches!(
                        code.as_ref(),
                        "57P01" | "57P02" | "57P03" | "40001" | "40P01" | "08000" | "08003" | "08006"
                    )
                } else {
                    false
                }
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    use std::sync::Arc;

    #[test]
    fn test_circuit_breaker_config_defaults() {
        let cfg = CircuitBreakerConfig::default();
        assert!(cfg.enabled);
        assert_eq!(cfg.failure_threshold, 5);
        assert_eq!(cfg.recovery_timeout_ms, 30000);
        assert_eq!(cfg.half_open_max_probes, 2);
        assert_eq!(cfg.retry_attempts, 3);
        assert_eq!(cfg.retry_base_delay_ms, 100);
        assert_eq!(cfg.retry_max_delay_ms, 2000);
    }

    #[test]
    fn test_circuit_breaker_config_env_overrides() {
        std::env::set_var("DB_BREAKER_ENABLED", "false");
        std::env::set_var("DB_BREAKER_FAILURE_THRESHOLD", "10");
        std::env::set_var("DB_BREAKER_RECOVERY_TIMEOUT_MS", "15000");
        std::env::set_var("DB_BREAKER_HALF_OPEN_MAX_PROBES", "4");
        std::env::set_var("DB_BREAKER_RETRY_ATTEMPTS", "5");
        std::env::set_var("DB_BREAKER_RETRY_BASE_DELAY_MS", "50");
        std::env::set_var("DB_BREAKER_RETRY_MAX_DELAY_MS", "1000");

        let cfg = CircuitBreakerConfig::from_env_or_config();

        std::env::remove_var("DB_BREAKER_ENABLED");
        std::env::remove_var("DB_BREAKER_FAILURE_THRESHOLD");
        std::env::remove_var("DB_BREAKER_RECOVERY_TIMEOUT_MS");
        std::env::remove_var("DB_BREAKER_HALF_OPEN_MAX_PROBES");
        std::env::remove_var("DB_BREAKER_RETRY_ATTEMPTS");
        std::env::remove_var("DB_BREAKER_RETRY_BASE_DELAY_MS");
        std::env::remove_var("DB_BREAKER_RETRY_MAX_DELAY_MS");

        assert!(!cfg.enabled);
        assert_eq!(cfg.failure_threshold, 10);
        assert_eq!(cfg.recovery_timeout_ms, 15000);
        assert_eq!(cfg.half_open_max_probes, 4);
        assert_eq!(cfg.retry_attempts, 5);
        assert_eq!(cfg.retry_base_delay_ms, 50);
        assert_eq!(cfg.retry_max_delay_ms, 1000);
    }

    #[test]
    fn test_circuit_breaker_config_yaml_parsing() {
        let yaml_sample = r#"
database:
  circuit_breaker:
    enabled: true
    failure_threshold: 7
    recovery_timeout_ms: 45000
    half_open_max_probes: 3
    retry_attempts: 4
    retry_base_delay_ms: 200
    retry_max_delay_ms: 4000
"#;
        let mut cfg = CircuitBreakerConfig::default();
        CircuitBreakerConfig::parse_yaml_into_config(yaml_sample, &mut cfg);
        assert!(cfg.enabled);
        assert_eq!(cfg.failure_threshold, 7);
        assert_eq!(cfg.recovery_timeout_ms, 45000);
        assert_eq!(cfg.half_open_max_probes, 3);
        assert_eq!(cfg.retry_attempts, 4);
        assert_eq!(cfg.retry_base_delay_ms, 200);
        assert_eq!(cfg.retry_max_delay_ms, 4000);
    }

    #[tokio::test]
    async fn test_closed_state_success_resets_failures() {
        let breaker = DbCircuitBreaker::new(CircuitBreakerConfig {
            failure_threshold: 3,
            retry_attempts: 1,
            ..Default::default()
        });

        let _ = breaker
            .execute(|| async { Err::<(), _>(sqlx::Error::RowNotFound) })
            .await;
        let _ = breaker
            .execute(|| async { Err::<(), _>(sqlx::Error::RowNotFound) })
            .await;

        let (state, failures, _) = breaker.metrics_snapshot();
        assert_eq!(state, STATE_CLOSED);
        assert_eq!(failures, 2);

        let res = breaker.execute(|| async { Ok::<_, sqlx::Error>(42) }).await;
        assert_eq!(res.unwrap(), 42);

        let (state, failures, _) = breaker.metrics_snapshot();
        assert_eq!(state, STATE_CLOSED);
        assert_eq!(failures, 0);
    }

    #[tokio::test]
    async fn test_closed_state_opens_after_threshold() {
        let breaker = DbCircuitBreaker::new(CircuitBreakerConfig {
            failure_threshold: 3,
            retry_attempts: 1,
            ..Default::default()
        });

        for _ in 0..3 {
            let _ = breaker
                .execute(|| async { Err::<(), _>(sqlx::Error::RowNotFound) })
                .await;
        }

        assert_eq!(breaker.current_state(), "Open");
        let (state, failures, _) = breaker.metrics_snapshot();
        assert_eq!(state, STATE_OPEN);
        assert_eq!(failures, 3);
    }

    #[tokio::test]
    async fn test_open_state_returns_circuit_open_error() {
        let breaker = DbCircuitBreaker::new(CircuitBreakerConfig {
            failure_threshold: 1,
            recovery_timeout_ms: 60000,
            retry_attempts: 1,
            ..Default::default()
        });

        let _ = breaker
            .execute(|| async { Err::<(), _>(sqlx::Error::RowNotFound) })
            .await;
        assert_eq!(breaker.current_state(), "Open");

        let res = breaker.execute(|| async { Ok::<_, sqlx::Error>("never_called") }).await;
        assert!(matches!(res, Err(DbError::CircuitOpen)));
    }

    #[tokio::test]
    async fn test_open_to_half_open_after_recovery_timeout() {
        let breaker = DbCircuitBreaker::new(CircuitBreakerConfig {
            failure_threshold: 1,
            recovery_timeout_ms: 10,
            retry_attempts: 1,
            ..Default::default()
        });

        let _ = breaker
            .execute(|| async { Err::<(), _>(sqlx::Error::RowNotFound) })
            .await;
        assert_eq!(breaker.current_state(), "Open");

        tokio::time::sleep(Duration::from_millis(25)).await;

        let res = breaker.execute(|| async { Ok::<_, sqlx::Error>("recovered") }).await;
        assert_eq!(res.unwrap(), "recovered");
        assert_eq!(breaker.current_state(), "Closed");
    }

    #[tokio::test]
    async fn test_half_open_success_closes_circuit() {
        let breaker = DbCircuitBreaker::new(CircuitBreakerConfig {
            failure_threshold: 2,
            recovery_timeout_ms: 10,
            retry_attempts: 1,
            ..Default::default()
        });

        breaker.trip_to_open().await;
        tokio::time::sleep(Duration::from_millis(20)).await;

        let res = breaker.execute(|| async { Ok::<_, sqlx::Error>(100) }).await;
        assert_eq!(res.unwrap(), 100);
        assert_eq!(breaker.current_state(), "Closed");
        let (_, failures, _) = breaker.metrics_snapshot();
        assert_eq!(failures, 0);
    }

    #[tokio::test]
    async fn test_retry_backoff_transient_error() {
        let breaker = DbCircuitBreaker::new(CircuitBreakerConfig {
            failure_threshold: 5,
            retry_attempts: 3,
            retry_base_delay_ms: 10,
            retry_max_delay_ms: 50,
            ..Default::default()
        });

        let attempts = Arc::new(AtomicUsize::new(0));
        let attempts_clone = attempts.clone();

        let res = breaker
            .execute(move || {
                let attempts = attempts_clone.clone();
                async move {
                    let count = attempts.fetch_add(1, Ordering::SeqCst) + 1;
                    if count < 3 {
                        Err(sqlx::Error::PoolTimedOut)
                    } else {
                        Ok("success_after_retries")
                    }
                }
            })
            .await;

        assert_eq!(res.unwrap(), "success_after_retries");
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
        assert_eq!(breaker.current_state(), "Closed");
    }

    #[tokio::test]
    async fn test_half_open_failure_reopens() {
        let breaker = DbCircuitBreaker::new(CircuitBreakerConfig {
            failure_threshold: 2,
            recovery_timeout_ms: 10,
            retry_attempts: 1,
            ..Default::default()
        });

        breaker.trip_to_open().await;
        tokio::time::sleep(Duration::from_millis(20)).await;

        let res = breaker
            .execute(|| async { Err::<(), _>(sqlx::Error::RowNotFound) })
            .await;
        assert!(matches!(res, Err(DbError::OperationFailed(_))));
        assert_eq!(breaker.current_state(), "Open");
    }
}
