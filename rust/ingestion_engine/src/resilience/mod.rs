//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Ingestion Engine Resilience & Circuit Breakers
//! Problem #15: Feed Outage Fault Isolation, Degraded State Machine & Chaos Harness
//! ═══════════════════════════════════════════════════════════════════════════════

pub mod breaker;

pub use breaker::{
    AllowDecision, BreakerRegistry, BreakerState, CircuitBreaker, IngestionMode, BASE_BACKOFF_MS,
    ERROR_RATIO, FAILURE_STREAK, MAX_BACKOFF_MS, VALID_RESILIENCE_SOURCES, WINDOW_MS,
};

use std::sync::atomic::{AtomicBool, Ordering};

/// Global flag indicating whether feed chaos injection is actively simulated.
static CHAOS_INJECTION_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Evaluates if chaos failure should be simulated for the given `source`
/// based on `FINTEXT_CHAOS_INJECT` environment variable (e.g. `finnhub_ws:1.0`).
///
/// In production mode, chaos injection is strictly prohibited and returns false.
pub fn should_inject_chaos(source: &str) -> bool {
    if crate::is_production_mode() {
        return false;
    }

    if let Ok(val) = std::env::var("FINTEXT_CHAOS_INJECT") {
        let val = val.trim();
        if val.is_empty() {
            return false;
        }

        // Parse format "<source>:<fail_rate>" or comma-separated pairs
        for part in val.split(',') {
            let part = part.trim();
            let mut pieces = part.split(':');
            if let (Some(src), Some(rate_str)) = (pieces.next(), pieces.next()) {
                if src.eq_ignore_ascii_case(source) {
                    if let Ok(rate) = rate_str.parse::<f64>() {
                        if rate >= 1.0 {
                            if !CHAOS_INJECTION_ACTIVE.swap(true, Ordering::Relaxed) {
                                eprintln!(
                                    "[FEED CHAOS] Activated failure simulation for '{}' (rate={:.2})",
                                    source, rate
                                );
                            }
                            return true;
                        } else if rate > 0.0 {
                            // Deterministic check or pseudorandom threshold
                            let pseudo_rand = (std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_micros()
                                % 100) as f64
                                / 100.0;
                            return pseudo_rand < rate;
                        }
                    }
                }
            } else if part.eq_ignore_ascii_case(source) {
                return true;
            }
        }
    }

    false
}
