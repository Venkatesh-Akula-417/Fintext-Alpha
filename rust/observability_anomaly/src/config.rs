//! Configuration for the Anomaly Detection Engine.

use std::env;

#[derive(Debug, Clone)]
pub struct AnomalyConfig {
    /// Prometheus server base URL (e.g. "http://prometheus:9090")
    pub prometheus_url: String,
    /// PromQL query to fetch the current latency metric
    pub query: String,
    /// Statistical Z-score threshold beyond which a sample is flagged as an anomaly (default: 3.0)
    pub threshold_zscore: f64,
    /// Rolling window sample size for calculating mean and standard deviation (default: 30)
    pub window_size: usize,
    /// Metric sampling check interval in seconds (default: 60)
    pub check_interval_secs: u64,
    /// Webhook URL to trigger upon detecting an anomaly (e.g. Alertmanager / KEDA / ArgoCD)
    pub webhook_url: Option<String>,
    /// Minimum standard deviation floor to avoid division by zero on flat baselines
    pub min_std_dev: f64,
    /// Mock mode flag for running self-tests without an active Prometheus instance
    pub mock_mode: bool,
}

impl Default for AnomalyConfig {
    fn default() -> Self {
        Self {
            prometheus_url: "http://prometheus:9090".to_string(),
            query: "sum(rate(http_request_duration_seconds_sum[5m])) / sum(rate(http_request_duration_seconds_count[5m]))".to_string(),
            threshold_zscore: 3.0,
            window_size: 30,
            check_interval_secs: 60,
            webhook_url: None,
            min_std_dev: 0.001,
            mock_mode: false,
        }
    }
}

impl AnomalyConfig {
    pub fn from_env() -> Self {
        let defaults = Self::default();

        let prometheus_url = env::var("PROMETHEUS_URL").unwrap_or(defaults.prometheus_url);
        let query = env::var("PROMETHEUS_QUERY").unwrap_or(defaults.query);

        let threshold_zscore = env::var("ANOMALY_THRESHOLD_ZSCORE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(defaults.threshold_zscore);

        let window_size = env::var("ANOMALY_WINDOW_SIZE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(defaults.window_size);

        let check_interval_secs = env::var("CHECK_INTERVAL_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(defaults.check_interval_secs);

        let webhook_url = env::var("WEBHOOK_URL").ok().filter(|s| !s.is_empty());

        let mock_mode = env::var("ANOMALY_MOCK_MODE")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        Self {
            prometheus_url,
            query,
            threshold_zscore,
            window_size,
            check_interval_secs,
            webhook_url,
            min_std_dev: defaults.min_std_dev,
            mock_mode,
        }
    }
}
