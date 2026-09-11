//! # FinText Observability Anomaly Detector
//!
//! Autonomous ML/Statistical Anomaly Detection Engine monitoring request latency
//! and error rates to trigger predictive auto-scaling and automatic rollbacks.

pub mod config;
pub mod detector;
pub mod prometheus;

pub use config::AnomalyConfig;
pub use detector::{AnomalyReport, RollingWindowAnomalyDetector};
pub use prometheus::ObservabilityClient;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_statistical_mean_and_std_calculation() {
        let mut detector = RollingWindowAnomalyDetector::new(10, 3.0, 0.001);
        let samples = vec![10.0, 12.0, 10.0, 11.0, 9.0];
        detector.seed_history(&samples);

        let (mean, std_dev) = detector.compute_statistics();
        assert!((mean - 10.4).abs() < 1e-5);
        assert!((std_dev - 1.140175).abs() < 1e-4);
    }

    #[test]
    fn test_nominal_distribution_no_anomaly() {
        let mut detector = RollingWindowAnomalyDetector::new(20, 3.0, 0.001);
        // Feed 10 steady latency samples (around 20ms)
        for &s in &[20.0, 21.0, 19.5, 20.5, 19.8, 20.2, 20.0, 20.1] {
            let report = detector.evaluate_sample(s);
            assert!(!report.is_anomaly, "Nominal sample flagged as anomaly!");
        }
    }

    #[test]
    fn test_severe_latency_spike_triggers_anomaly() {
        let mut detector = RollingWindowAnomalyDetector::new(20, 3.0, 0.001);
        // Baseline: ~10ms latency
        for &s in &[10.0, 10.2, 9.8, 10.1, 10.0, 9.9, 10.3, 10.0, 9.7, 10.1] {
            detector.evaluate_sample(s);
        }

        // Severe spike to 150ms (>10x baseline)
        let spike_report = detector.evaluate_sample(150.0);
        assert!(
            spike_report.is_anomaly,
            "Latency spike to 150ms was not flagged as anomaly!"
        );
        assert!(spike_report.z_score > 3.0);
    }

    #[test]
    fn test_rolling_window_bounded_capacity() {
        let mut detector = RollingWindowAnomalyDetector::new(5, 3.0, 0.001);
        for i in 1..=20 {
            detector.evaluate_sample(i as f64);
        }
        let (mean, _) = detector.compute_statistics();
        // The last 5 samples are 16, 17, 18, 19, 20 -> mean = 18.0
        assert!((mean - 18.0).abs() < 1e-5);
    }
}
