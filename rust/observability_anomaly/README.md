# FinText Observability & Anomaly Detector (`fintext_observability_anomaly`)

> **Last Verified**: 2026-09-10 (Suite #96) | **Audit Readiness**: Certified Clean | **Authoritative Deprecations**: [`docs/DEPRECATED.md`](../../docs/DEPRECATED.md)

## 1. Module Name & Purpose
`fintext_observability_anomaly` is a real-time statistical anomaly detection service monitoring the operational health, processing latency, and error rates of the FinText Alpha Vectorizer platform. It continuously scrapes Prometheus metrics endpoints, computes rolling Z-scores across sliding observation windows, and generates actionable alerts when latencies or error distributions deviate significantly from historical baselines.

## 2. Architecture
- **Prometheus Scraper (`prometheus.rs`)**: Regularly scrapes Prometheus `/metrics` endpoints across internal daemons (API server, ingestion engine, spillover engine).
- **Statistical Detector (`detector.rs`)**: Computes moving average ($\mu$) and standard deviation ($\sigma$) over a rolling time window, calculating exact Z-scores:
  $$Z = \frac{X_t - \mu}{\sigma}$$
- **Threshold Engine & Alerts (`config.rs`, `main.rs`)**: Evaluates whether $|Z| \ge \text{threshold}$ (default $3.0\sigma$) or error ratios exceed SLA budgets, dispatching alerts to monitoring systems.

```mermaid
flowchart LR
    Prom[/metrics Endpoint] --> Scraper[Prometheus Scraper]
    Scraper --> Window[Rolling Window Buffer]
    Window --> ZScore[Compute Dynamic Z-Score]
    ZScore --> Check{Z >= Multi-Sigma Threshold?}
    Check -- Yes --> Alert[Dispatch Anomaly Alert & Log Event]
    Check -- No --> Healthy[Update Baseline Stats]
```

## 3. Dependencies
| Dependency | Version | Purpose |
| :--- | :--- | :--- |
| `tokio` | `1.36` | Asynchronous timer loops and background polling |
| `reqwest` | `0.11` | HTTP client for Prometheus endpoint scraping |
| `serde` / `serde_json` | `1.0` | Serialization of anomaly alert payloads |
| `chrono` | `0.4` | Timestamps and sliding observation time calculations |
| `tracing` | `0.1` | Structured logging |
| `anyhow` | `1.0` | Ergonomic error handling |

## 4. Public API
- `pub struct AnomalyDetector`: Core statistical engine maintaining sliding metric buffers.
- `pub struct DetectorConfig`: Configuration model (`scrape_interval_secs`, `z_score_threshold`, `min_samples_window`, `metrics_url`).
- `pub struct PrometheusScraper`: Fetches and parses Prometheus text/OpenMetrics format.
- `pub struct AnomalyEvent`: Structured representation of a detected statistical anomaly.

## 5. Testing & Running
Run unit and integration tests:
```bash
cargo test -p fintext_observability_anomaly
```

Launch the anomaly detector daemon:
```bash
cargo run --release -p fintext_observability_anomaly
```

## 6. Usage Example
```rust
use fintext_observability_anomaly::detector::AnomalyDetector;

let mut detector = AnomalyDetector::new(3.0, 30);

// Populate baseline values around ~50ms latency
for _ in 0..30 {
    detector.record_metric("ingestion_latency_ms", 50.0);
}

// Check an anomalous spike (e.g. 500ms)
if let Some(anomaly) = detector.check_anomaly("ingestion_latency_ms", 500.0) {
    println!("Anomaly detected! Z-score: {:.2}", anomaly.z_score);
}
```

## 7. Related Modules
- [`fintext_ingestion_engine`](../ingestion_engine): Produces latency and throughput telemetry.
- [`fintext_api_server`](../api_server): Exposes `/metrics` and latency SLA monitoring endpoints.
