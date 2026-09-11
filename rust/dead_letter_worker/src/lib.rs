//! # FinText Dead Letter Queue (DLQ) Auto-Reprocessing & Quarantine Worker
//!
//! Provides automated, resilient dead-letter queue message ingestion, exponential backoff retries,
//! and AWS S3 quarantine routing for permanently failed quantitative sentiment events.

pub mod config;
pub mod quarantine;
pub mod retry;
pub mod worker;

pub use config::DlqConfig;
pub use quarantine::{
    generate_local_path, generate_s3_key, QuarantineManager, QuarantinedEnvelope,
};
pub use retry::{calculate_backoff_ms, execute_with_retry, RetryOutcome};
pub use worker::DeadLetterWorker;

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[test]
    fn test_exponential_backoff_calculation() {
        let initial_ms = 100;
        let max_ms = 5000;

        assert_eq!(calculate_backoff_ms(1, initial_ms, max_ms), 100);
        assert_eq!(calculate_backoff_ms(2, initial_ms, max_ms), 200);
        assert_eq!(calculate_backoff_ms(3, initial_ms, max_ms), 400);
        assert_eq!(calculate_backoff_ms(4, initial_ms, max_ms), 800);
        assert_eq!(calculate_backoff_ms(5, initial_ms, max_ms), 1600);
        assert_eq!(calculate_backoff_ms(6, initial_ms, max_ms), 3200);
        assert_eq!(calculate_backoff_ms(7, initial_ms, max_ms), 5000); // capped at max_ms
        assert_eq!(calculate_backoff_ms(10, initial_ms, max_ms), 5000);
    }

    #[test]
    fn test_s3_key_generation_format() {
        let fixed_dt = Utc.with_ymd_and_hms(2026, 8, 27, 10, 30, 0).unwrap();
        let key = generate_s3_key("dlq/", fixed_dt, "abc-123");
        assert!(key.starts_with("dlq/2026/08/27/message-"));
        assert!(key.ends_with("-abc-123.json"));

        // Without trailing slash in prefix
        let key2 = generate_s3_key("quarantine", fixed_dt, "xyz-789");
        assert!(key2.starts_with("quarantine/2026/08/27/message-"));
    }

    #[test]
    fn test_local_path_generation_format() {
        let fixed_dt = Utc.with_ymd_and_hms(2026, 8, 27, 10, 30, 0).unwrap();
        let path = generate_local_path("data/quarantine", fixed_dt, "test-id-1");
        let path_str = path.to_string_lossy().replace('\\', "/");
        assert!(path_str.starts_with("data/quarantine/message-"));
        assert!(path_str.ends_with("-test-id-1.json"));
    }

    #[tokio::test]
    async fn test_retry_transient_failure_eventual_success() {
        let attempts_called = Arc::new(AtomicUsize::new(0));
        let attempts_clone = Arc::clone(&attempts_called);

        let outcome = execute_with_retry(
            5,
            5, // fast 5ms backoff for unit tests
            50,
            r#"{"event_id": "test-1"}"#,
            move |_payload, attempt| {
                let count = attempts_clone.fetch_add(1, Ordering::SeqCst);
                async move {
                    if count < 2 {
                        Err(format!("Transient error on attempt {}", attempt))
                    } else {
                        Ok(())
                    }
                }
            },
        )
        .await;

        assert_eq!(outcome, RetryOutcome::Success { attempts: 3 });
        assert_eq!(attempts_called.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_retry_permanent_failure_exhaustion() {
        let attempts_called = Arc::new(AtomicUsize::new(0));
        let attempts_clone = Arc::clone(&attempts_called);

        let outcome = execute_with_retry(
            4,
            5,
            50,
            r#"{"event_id": "test-exhaust"}"#,
            move |_payload, attempt| {
                attempts_clone.fetch_add(1, Ordering::SeqCst);
                async move { Err(format!("Permanent database failure on attempt {}", attempt)) }
            },
        )
        .await;

        match outcome {
            RetryOutcome::Exhausted {
                attempts,
                last_error,
            } => {
                assert_eq!(attempts, 4);
                assert!(last_error.contains("Permanent database failure on attempt 4"));
            }
            _ => panic!("Expected RetryOutcome::Exhausted"),
        }

        assert_eq!(attempts_called.load(Ordering::SeqCst), 4);
    }

    #[tokio::test]
    async fn test_local_quarantine_fallback_writing() {
        let temp_dir = format!("data/quarantine_test_{}", uuid::Uuid::new_v4());
        let mgr = QuarantineManager::new(
            "mock-bucket".to_string(),
            "dlq/".to_string(),
            temp_dir.clone(),
            true, // mock mode (forces local file fallback)
        )
        .await;

        let payload = r#"{"ticker": "NVDA", "sentiment": -0.85, "reason": "timeout"}"#;
        let result = mgr
            .quarantine(payload, "Connection timeout to QuestDB", 5)
            .await;
        assert!(result.is_ok());

        let written_path = result.unwrap();
        assert!(std::path::Path::new(&written_path).exists());

        let file_content = tokio::fs::read_to_string(&written_path).await.unwrap();
        let envelope: QuarantinedEnvelope = serde_json::from_str(&file_content).unwrap();
        assert_eq!(envelope.attempts_exhausted, 5);
        assert_eq!(envelope.failure_reason, "Connection timeout to QuestDB");
        assert_eq!(envelope.original_payload["ticker"], "NVDA");

        // Clean up test directory
        let _ = tokio::fs::remove_file(&written_path).await;
        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
    }

    #[tokio::test]
    async fn test_worker_end_to_end_mock_cycle() {
        let mut config = DlqConfig::default();
        config.mock_mode = true;
        config.initial_backoff_ms = 5;
        config.max_backoff_ms = 20;
        config.local_quarantine_dir =
            format!("data/quarantine_worker_test_{}", uuid::Uuid::new_v4());

        let mut worker = DeadLetterWorker::new(config.clone()).await.unwrap();
        let run_result = worker.run().await;
        assert!(run_result.is_ok());

        // Clean up test directory if created
        let _ = tokio::fs::remove_dir_all(&config.local_quarantine_dir).await;
    }
}
