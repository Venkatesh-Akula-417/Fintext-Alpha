//! Exponential backoff retry engine for DLQ auto-reprocessing.

use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, warn};

#[derive(Debug, PartialEq, Eq)]
pub enum RetryOutcome {
    /// Processing succeeded on the given attempt number (1-indexed)
    Success { attempts: usize },
    /// Processing exhausted all allowed attempts
    Exhausted { attempts: usize, last_error: String },
}

/// Computes exponential backoff with a hard ceiling.
/// attempt: 1 -> initial_ms
/// attempt: 2 -> initial_ms * 2
/// attempt: 3 -> initial_ms * 4
/// ... capped at max_backoff_ms
pub fn calculate_backoff_ms(attempt: usize, initial_backoff_ms: u64, max_backoff_ms: u64) -> u64 {
    if attempt <= 1 {
        return initial_backoff_ms;
    }
    let multiplier = 1u64
        .checked_shl((attempt - 1).min(30) as u32)
        .unwrap_or(u64::MAX);
    initial_backoff_ms
        .saturating_mul(multiplier)
        .min(max_backoff_ms)
}

/// Executes a processing closure with exponential backoff up to `max_retries` attempts.
pub async fn execute_with_retry<F, Fut>(
    max_retries: usize,
    initial_backoff_ms: u64,
    max_backoff_ms: u64,
    payload: &str,
    process_fn: F,
) -> RetryOutcome
where
    F: Fn(&str, usize) -> Fut,
    Fut: std::future::Future<Output = Result<(), String>>,
{
    let mut last_error = String::new();

    for attempt in 1..=max_retries {
        match process_fn(payload, attempt).await {
            Ok(()) => {
                info!(
                    attempt = attempt,
                    "Message successfully re-processed and acknowledged."
                );
                return RetryOutcome::Success { attempts: attempt };
            }
            Err(e) => {
                last_error = e;
                if attempt < max_retries {
                    let backoff = calculate_backoff_ms(attempt, initial_backoff_ms, max_backoff_ms);
                    warn!(
                        attempt = attempt,
                        max_retries = max_retries,
                        backoff_ms = backoff,
                        error = %last_error,
                        "DLQ processing failed. Backing off before retry..."
                    );
                    sleep(Duration::from_millis(backoff)).await;
                }
            }
        }
    }

    RetryOutcome::Exhausted {
        attempts: max_retries,
        last_error,
    }
}
