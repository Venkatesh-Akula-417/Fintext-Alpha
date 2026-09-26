//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Latency Telemetry, Histograms & Metrics Exporter
//! Problem #11: Ingestion Per-Source Fetch & Lag Telemetry (Sub-Second Buckets)
//! ═══════════════════════════════════════════════════════════════════════════════

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use tracing::info;

/// Standard Prometheus sub-second latency histogram buckets (0.005s to 2.0s).
pub const HISTOGRAM_BUCKETS: [f64; 9] = [
    0.005, 0.010, 0.025, 0.050, 0.100, 0.250, 0.500, 1.000, 2.000,
];

/// Known institutional data sources tracked by the telemetry pipeline.
pub const VALID_SOURCES: [&str; 5] = [
    "sec_edgar",
    "fomc",
    "finnhub_ws",
    "polygon_ws",
    "corporate_actions",
];

/// Per-source latency histogram and error counter telemetry container.
#[derive(Debug)]
pub struct PerSourceTelemetry {
    pub source: String,
    pub fetch_buckets: [AtomicU64; 9],
    pub fetch_overflow: AtomicU64,
    pub fetch_count: AtomicU64,
    pub fetch_sum_us: AtomicU64,

    pub lag_buckets: [AtomicU64; 9],
    pub lag_overflow: AtomicU64,
    pub lag_count: AtomicU64,
    pub lag_sum_us: AtomicU64,

    pub events_ingested_total: AtomicU64,
    pub errors_by_reason: Arc<RwLock<HashMap<String, u64>>>,
}

impl PerSourceTelemetry {
    pub fn new(source: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            fetch_buckets: Default::default(),
            fetch_overflow: AtomicU64::new(0),
            fetch_count: AtomicU64::new(0),
            fetch_sum_us: AtomicU64::new(0),

            lag_buckets: Default::default(),
            lag_overflow: AtomicU64::new(0),
            lag_count: AtomicU64::new(0),
            lag_sum_us: AtomicU64::new(0),

            events_ingested_total: AtomicU64::new(0),
            errors_by_reason: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Records HTTP/WS batch fetch duration in seconds.
    pub fn record_fetch_duration(&self, duration_secs: f64) {
        let dur = duration_secs.max(0.0);
        let us = (dur * 1_000_000.0) as u64;
        self.fetch_sum_us.fetch_add(us, Ordering::Relaxed);
        self.fetch_count.fetch_add(1, Ordering::Relaxed);

        let mut matched = false;
        for (i, &bucket) in HISTOGRAM_BUCKETS.iter().enumerate() {
            if dur <= bucket {
                self.fetch_buckets[i].fetch_add(1, Ordering::Relaxed);
                matched = true;
            }
        }
        if !matched {
            self.fetch_overflow.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Records event latency lag (source event timestamp to DB commit timestamp) in seconds.
    pub fn record_event_lag(&self, lag_secs: f64) {
        let lag = lag_secs.max(0.0);
        let us = (lag * 1_000_000.0) as u64;
        self.lag_sum_us.fetch_add(us, Ordering::Relaxed);
        self.lag_count.fetch_add(1, Ordering::Relaxed);

        let mut matched = false;
        for (i, &bucket) in HISTOGRAM_BUCKETS.iter().enumerate() {
            if lag <= bucket {
                self.lag_buckets[i].fetch_add(1, Ordering::Relaxed);
                matched = true;
            }
        }
        if !matched {
            self.lag_overflow.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Increments the count of events successfully ingested from this source.
    pub fn record_event_ingested(&self) {
        self.events_ingested_total.fetch_add(1, Ordering::Relaxed);
    }

    /// Increments the error counter for a specific failure reason.
    pub fn record_error(&self, reason: &str) {
        let clean = reason.trim().to_lowercase();
        if let Ok(mut map) = self.errors_by_reason.write() {
            *map.entry(clean).or_insert(0) += 1;
        }
    }

    /// Computes the estimated P95 event lag in seconds using histogram cumulative counts.
    pub fn p95_lag_seconds(&self) -> f64 {
        let total = self.lag_count.load(Ordering::Relaxed);
        if total == 0 {
            return 0.0;
        }

        let p95_target = (total as f64 * 0.95).ceil() as u64;
        for (i, &bucket) in HISTOGRAM_BUCKETS.iter().enumerate() {
            let count = self.lag_buckets[i].load(Ordering::Relaxed);
            if count >= p95_target {
                return bucket;
            }
        }
        // If not contained in buckets, return upper bound
        2.500
    }

    /// Computes the estimated P95 fetch duration in seconds.
    pub fn p95_fetch_duration_seconds(&self) -> f64 {
        let total = self.fetch_count.load(Ordering::Relaxed);
        if total == 0 {
            return 0.0;
        }

        let p95_target = (total as f64 * 0.95).ceil() as u64;
        for (i, &bucket) in HISTOGRAM_BUCKETS.iter().enumerate() {
            let count = self.fetch_buckets[i].load(Ordering::Relaxed);
            if count >= p95_target {
                return bucket;
            }
        }
        2.500
    }
}

/// Central Pipeline Telemetry Engine holding legacy and per-source Prometheus counters.
#[derive(Debug, Clone)]
pub struct PipelineMetrics {
    // Legacy metrics
    pub total_ingested: Arc<AtomicU64>,
    pub total_preprocessed: Arc<AtomicU64>,
    pub total_spams_rejected: Arc<AtomicU64>,
    pub total_sent_to_python: Arc<AtomicU64>,
    pub total_stored: Arc<AtomicU64>,
    pub total_preprocessing_latency_us: Arc<AtomicU64>,

    // Problem #11: Per-source telemetry
    sources: Arc<HashMap<String, Arc<PerSourceTelemetry>>>,
}

impl Default for PipelineMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl PipelineMetrics {
    pub fn new() -> Self {
        let mut map = HashMap::new();
        for &s in &VALID_SOURCES {
            map.insert(s.to_string(), Arc::new(PerSourceTelemetry::new(s)));
        }

        Self {
            total_ingested: Arc::new(AtomicU64::new(0)),
            total_preprocessed: Arc::new(AtomicU64::new(0)),
            total_spams_rejected: Arc::new(AtomicU64::new(0)),
            total_sent_to_python: Arc::new(AtomicU64::new(0)),
            total_stored: Arc::new(AtomicU64::new(0)),
            total_preprocessing_latency_us: Arc::new(AtomicU64::new(0)),
            sources: Arc::new(map),
        }
    }

    pub fn get_source(&self, source: &str) -> Option<Arc<PerSourceTelemetry>> {
        self.sources.get(source).cloned()
    }

    pub fn record_fetch_duration(&self, source: &str, duration_secs: f64) {
        if let Some(s) = self.sources.get(source) {
            s.record_fetch_duration(duration_secs);
        }
    }

    pub fn record_event_lag(&self, source: &str, lag_secs: f64) {
        if let Some(s) = self.sources.get(source) {
            s.record_event_lag(lag_secs);
        }
    }

    pub fn record_event_ingested(&self, source: &str) {
        self.total_ingested.fetch_add(1, Ordering::Relaxed);
        if let Some(s) = self.sources.get(source) {
            s.record_event_ingested();
        }
    }

    pub fn record_error(&self, source: &str, reason: &str) {
        if let Some(s) = self.sources.get(source) {
            s.record_error(reason);
        }
    }

    pub fn get_p95_lag(&self, source: &str) -> f64 {
        self.sources
            .get(source)
            .map(|s| s.p95_lag_seconds())
            .unwrap_or(0.0)
    }

    pub fn get_p95_fetch(&self, source: &str) -> f64 {
        self.sources
            .get(source)
            .map(|s| s.p95_fetch_duration_seconds())
            .unwrap_or(0.0)
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

    /// Renders all per-source and pipeline metrics into standard Prometheus exposition format.
    pub fn render_prometheus(&self) -> String {
        let mut out = String::with_capacity(4096);

        // Header comments
        out.push_str("# HELP fetch_duration_seconds HTTP/WS batch fetch wall time per source\n");
        out.push_str("# TYPE fetch_duration_seconds histogram\n");

        for &src in &VALID_SOURCES {
            if let Some(s) = self.sources.get(src) {
                let count = s.fetch_count.load(Ordering::Relaxed);
                let sum_sec = s.fetch_sum_us.load(Ordering::Relaxed) as f64 / 1_000_000.0;

                for (i, &bucket) in HISTOGRAM_BUCKETS.iter().enumerate() {
                    let b_count = s.fetch_buckets[i].load(Ordering::Relaxed);
                    out.push_str(&format!(
                        "fetch_duration_seconds_bucket{{source=\"{}\",le=\"{:.3}\"}} {}\n",
                        src, bucket, b_count
                    ));
                }
                out.push_str(&format!(
                    "fetch_duration_seconds_bucket{{source=\"{}\",le=\"+Inf\"}} {}\n",
                    src, count
                ));
                out.push_str(&format!(
                    "fetch_duration_seconds_sum{{source=\"{}\"}} {:.6}\n",
                    src, sum_sec
                ));
                out.push_str(&format!(
                    "fetch_duration_seconds_count{{source=\"{}\"}} {}\n",
                    src, count
                ));
            }
        }

        out.push_str("\n# HELP event_lag_seconds Freshness lag from source event timestamp to database commit timestamp\n");
        out.push_str("# TYPE event_lag_seconds histogram\n");

        for &src in &VALID_SOURCES {
            if let Some(s) = self.sources.get(src) {
                let count = s.lag_count.load(Ordering::Relaxed);
                let sum_sec = s.lag_sum_us.load(Ordering::Relaxed) as f64 / 1_000_000.0;

                for (i, &bucket) in HISTOGRAM_BUCKETS.iter().enumerate() {
                    let b_count = s.lag_buckets[i].load(Ordering::Relaxed);
                    out.push_str(&format!(
                        "event_lag_seconds_bucket{{source=\"{}\",le=\"{:.3}\"}} {}\n",
                        src, bucket, b_count
                    ));
                }
                out.push_str(&format!(
                    "event_lag_seconds_bucket{{source=\"{}\",le=\"+Inf\"}} {}\n",
                    src, count
                ));
                out.push_str(&format!(
                    "event_lag_seconds_sum{{source=\"{}\"}} {:.6}\n",
                    src, sum_sec
                ));
                out.push_str(&format!(
                    "event_lag_seconds_count{{source=\"{}\"}} {}\n",
                    src, count
                ));
            }
        }

        out.push_str(
            "\n# HELP events_ingested_total Total raw events ingested per collector source\n",
        );
        out.push_str("# TYPE events_ingested_total counter\n");

        for &src in &VALID_SOURCES {
            if let Some(s) = self.sources.get(src) {
                let count = s.events_ingested_total.load(Ordering::Relaxed);
                out.push_str(&format!(
                    "events_ingested_total{{source=\"{}\"}} {}\n",
                    src, count
                ));
            }
        }

        out.push_str(
            "\n# HELP errors_total Total ingestion errors per collector source and error reason\n",
        );
        out.push_str("# TYPE errors_total counter\n");

        for &src in &VALID_SOURCES {
            if let Some(s) = self.sources.get(src) {
                if let Ok(map) = s.errors_by_reason.read() {
                    for (reason, count) in map.iter() {
                        out.push_str(&format!(
                            "errors_total{{source=\"{}\",reason=\"{}\"}} {}\n",
                            src, reason, count
                        ));
                    }
                    if map.is_empty() {
                        out.push_str(&format!(
                            "errors_total{{source=\"{}\",reason=\"none\"}} 0\n",
                            src
                        ));
                    }
                }
            }
        }

        // Summary gauges for P95 metrics (convenient for single-stat Grafana gauges)
        out.push_str("\n# HELP fintext_ingestion_p95_lag_seconds Current estimated P95 event lag in seconds\n");
        out.push_str("# TYPE fintext_ingestion_p95_lag_seconds gauge\n");
        for &src in &VALID_SOURCES {
            out.push_str(&format!(
                "fintext_ingestion_p95_lag_seconds{{source=\"{}\"}} {:.3}\n",
                src,
                self.get_p95_lag(src)
            ));
        }

        out.push_str("\n# HELP fintext_ingestion_p95_fetch_seconds Current estimated P95 batch fetch duration in seconds\n");
        out.push_str("# TYPE fintext_ingestion_p95_fetch_seconds gauge\n");
        for &src in &VALID_SOURCES {
            out.push_str(&format!(
                "fintext_ingestion_p95_fetch_seconds{{source=\"{}\"}} {:.3}\n",
                src,
                self.get_p95_fetch(src)
            ));
        }

        out
    }
}

/// Starts an internal HTTP Prometheus metrics exposition server on the given address.
/// Only binds to internal network interfaces (e.g. 127.0.0.1 or VPC private IP).
pub async fn start_metrics_server(
    metrics: Arc<PipelineMetrics>,
    bind_addr: &str,
) -> Result<tokio::task::JoinHandle<()>, std::io::Error> {
    let listener = tokio::net::TcpListener::bind(bind_addr).await?;
    info!(
        "[Telemetry Server] Internal Ingestion /metrics server bound to http://{}",
        bind_addr
    );

    let handle = tokio::spawn(async move {
        loop {
            if let Ok((mut stream, _)) = listener.accept().await {
                let m = metrics.clone();
                tokio::spawn(async move {
                    use tokio::io::{AsyncReadExt, AsyncWriteExt};
                    let mut buf = [0u8; 1024];
                    if let Ok(n) = stream.read(&mut buf).await {
                        if n > 0 {
                            let req = String::from_utf8_lossy(&buf[..n]);
                            let first_line = req.lines().next().unwrap_or("");
                            let (status, content_type, body) = if first_line
                                .starts_with("GET /metrics")
                            {
                                (
                                    "HTTP/1.1 200 OK",
                                    "text/plain; version=0.0.4; charset=utf-8",
                                    m.render_prometheus(),
                                )
                            } else if first_line.starts_with("GET /health") {
                                (
                                    "HTTP/1.1 200 OK",
                                    "application/json",
                                    r#"{"status":"healthy","service":"fintext_ingestion_telemetry"}"#
                                        .to_string(),
                                )
                            } else {
                                (
                                    "HTTP/1.1 404 Not Found",
                                    "text/plain",
                                    "Not Found\n".to_string(),
                                )
                            };

                            let resp = format!(
                                "{}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                                status,
                                content_type,
                                body.len(),
                                body
                            );
                            let _ = stream.write_all(resp.as_bytes()).await;
                        }
                    }
                });
            }
        }
    });

    Ok(handle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_sources_list_integrity() {
        assert_eq!(VALID_SOURCES.len(), 5);
        assert!(VALID_SOURCES.contains(&"sec_edgar"));
        assert!(VALID_SOURCES.contains(&"fomc"));
        assert!(VALID_SOURCES.contains(&"finnhub_ws"));
        assert!(VALID_SOURCES.contains(&"polygon_ws"));
        assert!(VALID_SOURCES.contains(&"corporate_actions"));
    }

    #[test]
    fn test_fetch_duration_histogram_bucketing() {
        let metrics = PipelineMetrics::new();

        // Record fetch durations for sec_edgar: 3ms (0.003s), 15ms (0.015s), 300ms (0.300s)
        metrics.record_fetch_duration("sec_edgar", 0.003);
        metrics.record_fetch_duration("sec_edgar", 0.015);
        metrics.record_fetch_duration("sec_edgar", 0.300);

        let source = metrics
            .get_source("sec_edgar")
            .expect("sec_edgar must exist");
        assert_eq!(source.fetch_count.load(Ordering::Relaxed), 3);

        // Bucket 0.005 should have 1 item (0.003 <= 0.005)
        assert_eq!(source.fetch_buckets[0].load(Ordering::Relaxed), 1);
        // Bucket 0.025 should include 0.003 and 0.015 -> 2
        assert_eq!(source.fetch_buckets[2].load(Ordering::Relaxed), 2);
        // Bucket 0.500 should include all 3 -> 3
        assert_eq!(source.fetch_buckets[6].load(Ordering::Relaxed), 3);
    }

    #[test]
    fn test_event_lag_p95_computation() {
        let metrics = PipelineMetrics::new();

        // Seed 100 samples for polygon_ws
        // 94 samples at 0.004s (<= 0.005)
        for _ in 0..94 {
            metrics.record_event_lag("polygon_ws", 0.004);
        }
        // 6 samples at 0.045s (<= 0.050)
        for _ in 0..6 {
            metrics.record_event_lag("polygon_ws", 0.045);
        }

        let p95 = metrics.get_p95_lag("polygon_ws");
        assert_eq!(p95, 0.050);
    }

    #[test]
    fn test_error_recording_and_prometheus_rendering() {
        let metrics = PipelineMetrics::new();
        metrics.record_error("finnhub_ws", "websocket_disconnect");
        metrics.record_error("finnhub_ws", "rate_limited");
        metrics.record_event_ingested("finnhub_ws");

        let prom = metrics.render_prometheus();
        assert!(prom.contains("events_ingested_total{source=\"finnhub_ws\"} 1"));
        assert!(
            prom.contains("errors_total{source=\"finnhub_ws\",reason=\"websocket_disconnect\"} 1")
        );
        assert!(prom.contains("errors_total{source=\"finnhub_ws\",reason=\"rate_limited\"} 1"));
        assert!(prom.contains("# TYPE event_lag_seconds histogram"));
        assert!(prom.contains("# TYPE fetch_duration_seconds histogram"));
    }

    #[tokio::test]
    async fn test_internal_metrics_server_http_endpoint() {
        let metrics = Arc::new(PipelineMetrics::new());
        metrics.record_event_ingested("sec_edgar");
        metrics.record_event_lag("sec_edgar", 0.012);

        // Bind on localhost dynamic port (port 0)
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);

        let bind_addr = format!("127.0.0.1:{}", port);
        let handle = start_metrics_server(metrics.clone(), &bind_addr)
            .await
            .unwrap();

        // Fetch /metrics via reqwest
        let url = format!("http://127.0.0.1:{}/metrics", port);
        let client = reqwest::Client::new();
        let resp = client
            .get(&url)
            .send()
            .await
            .expect("Must connect to metrics server");
        assert_eq!(resp.status(), reqwest::StatusCode::OK);

        let body = resp.text().await.unwrap();
        assert!(body.contains("events_ingested_total{source=\"sec_edgar\"} 1"));
        assert!(body.contains("event_lag_seconds_bucket{source=\"sec_edgar\",le=\"0.025\"} 1"));

        // Test /health
        let health_url = format!("http://127.0.0.1:{}/health", port);
        let health_resp = client.get(&health_url).send().await.unwrap();
        assert_eq!(health_resp.status(), reqwest::StatusCode::OK);

        handle.abort();
    }
}
