//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Latency Telemetry & Performance Counter
//! ═══════════════════════════════════════════════════════════════════════════════

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

#[derive(Debug, Default, Clone)]
pub struct PipelineMetrics {
    pub total_ingested: Arc<AtomicU64>,
    pub total_preprocessed: Arc<AtomicU64>,
    pub total_spams_rejected: Arc<AtomicU64>,
    pub total_sent_to_python: Arc<AtomicU64>,
    pub total_stored: Arc<AtomicU64>,
    pub total_preprocessing_latency_us: Arc<AtomicU64>,
}

impl PipelineMetrics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_preprocessing(&self, latency_us: u64, is_spam: bool) {
        self.total_preprocessed.fetch_add(1, Ordering::Relaxed);
        self.total_preprocessing_latency_us
            .fetch_add(latency_us, Ordering::Relaxed);
        if is_spam {
            self.total_spams_rejected.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn record_stored(&self) {
        self.total_stored.fetch_add(1, Ordering::Relaxed);
    }

    pub fn avg_preprocessing_latency_us(&self) -> f64 {
        let total = self.total_preprocessed.load(Ordering::Relaxed);
        if total == 0 {
            0.0
        } else {
            let lat = self.total_preprocessing_latency_us.load(Ordering::Relaxed) as f64;
            lat / (total as f64)
        }
    }
}
