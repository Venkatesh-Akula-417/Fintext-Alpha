//! Statistical Z-Score and Autoencoder-proxy Anomaly Detection Engine.

use chrono::Utc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AnomalyReport {
    pub is_anomaly: bool,
    pub current_value: f64,
    pub rolling_mean: f64,
    pub rolling_std_dev: f64,
    pub z_score: f64,
    pub threshold: f64,
    pub sample_count: usize,
    pub timestamp_utc: String,
}

pub struct RollingWindowAnomalyDetector {
    window_size: usize,
    threshold: f64,
    min_std_dev: f64,
    history: Vec<f64>,
}

impl RollingWindowAnomalyDetector {
    pub fn new(window_size: usize, threshold: f64, min_std_dev: f64) -> Self {
        Self {
            window_size,
            threshold,
            min_std_dev,
            history: Vec::with_capacity(window_size),
        }
    }

    /// Computes the sample mean and sample standard deviation of historical observations.
    pub fn compute_statistics(&self) -> (f64, f64) {
        if self.history.is_empty() {
            return (0.0, self.min_std_dev);
        }

        let n = self.history.len() as f64;
        let mean = self.history.iter().sum::<f64>() / n;

        if self.history.len() < 2 {
            return (mean, self.min_std_dev);
        }

        let variance = self
            .history
            .iter()
            .map(|&x| (x - mean).powi(2))
            .sum::<f64>()
            / (n - 1.0);

        let std_dev = variance.sqrt().max(self.min_std_dev);
        (mean, std_dev)
    }

    /// Evaluates a new observation against the statistical baseline.
    pub fn evaluate_sample(&mut self, sample: f64) -> AnomalyReport {
        let (mean, std_dev) = self.compute_statistics();

        // Calculate Z-Score (reconstruction divergence measure)
        let z_score = if self.history.len() >= 3 {
            (sample - mean) / std_dev
        } else {
            0.0 // Insufficient warmup samples to trigger false positive
        };

        let is_anomaly = z_score.abs() >= self.threshold;

        // Update rolling window
        if self.history.len() >= self.window_size {
            self.history.remove(0);
        }
        self.history.push(sample);

        AnomalyReport {
            is_anomaly,
            current_value: sample,
            rolling_mean: mean,
            rolling_std_dev: std_dev,
            z_score,
            threshold: self.threshold,
            sample_count: self.history.len(),
            timestamp_utc: Utc::now().to_rfc3339(),
        }
    }

    /// Warm up detector with known baseline values.
    pub fn seed_history(&mut self, values: &[f64]) {
        self.history.clear();
        for &v in values.iter().take(self.window_size) {
            self.history.push(v);
        }
    }
}
