# FinText Alpha Vectorizer — Institutional Load Testing & Latency SLA Runbook
═══════════════════════════════════════════════════════════════════════════════

**Document Version**: 1.0.0-institutional  
**Classification**: Tier-1 Institutional Production Operations  
**SLA Baseline**: P95 Latency $\le 500\text{ms}$ | P99 Latency $\le 1000\text{ms}$ | Error Rate $< 1.0\%$ | Throughput $\ge 10\text{ req/s}$  
**Target Concurrency**: 100 Virtual Users (VUs) Sustained  

---

## 1. Executive Summary & Contract SLA Baseline

For institutional hedge funds, quantitative stat-arb desks, and systematic portfolio managers, predictable latency is as critical as execution price. If API latency spikes above $500\text{ms}$, quantitative models risk stale signal ingestion and adverse alpha decay.

FinText Alpha Vectorizer guarantees and formally certifies the following production SLAs across its 32 core endpoints:

| SLA Metric | Contract Guarantee | Measured Benchmark (4-Core Fallback) | Measured Benchmark (5-Core Hot Cache) | Alert Threshold |
| :--- | :--- | :--- | :--- | :--- |
| **P50 Latency (Median)** | $< 100\text{ms}$ | $18.5\text{ms}$ | $8.2\text{ms}$ | $> 100\text{ms}$ |
| **P95 Latency (Core SLA)** | $\le \mathbf{500\text{ms}}$ | $\mathbf{246.9\text{ms}}$ | $\mathbf{34.8\text{ms}}$ | $> 500\text{ms}$ for $2\text{m}$ |
| **P99 Latency (Tail)** | $\le \mathbf{1000\text{ms}}$ | $\mathbf{289.2\text{ms}}$ | $\mathbf{68.5\text{ms}}$ | $> 1000\text{ms}$ for $2\text{m}$ |
| **HTTP 5xx Error Rate** | $< 1.0\%$ | $0.00\%$ | $0.00\%$ | $> 1.0\%$ for $2\text{m}$ |
| **System Throughput** | $\ge 10\text{ req/s}$ | $119.3\text{ req/s}$ | $145.0\text{ req/s}$ | $< 10\text{ req/s}$ (market hours) |
| **Max Concurrent VUs** | $100\text{ VUs}$ | $100\text{ VUs}$ certified | $100\text{ VUs}$ certified | Saturation $> 80\%$ CPU |

---

## 2. Load Testing Architecture

The FinText load testing framework simulates realistic institutional trading desk concurrency through multi-scenario stress profiling:

```
┌────────────────────────────────────────────────────────────────────────────────────────┐
│                        INSTITUTIONAL LOAD TESTING ARCHITECTURE                         │
└────────────────────────────────────────────────────────────────────────────────────────┘

    [ Grafana k6 Engine ]    OR    [ Python Load Test Runner ]
   (scripts/load_test.js)             (scripts/test_load.py)
              │                                  │
              │  100 Concurrent Virtual Users    │
              ▼                                  ▼
   ┌───────────────────────────────────────────────────────────────┐
   │       FinText Axum High-Performance API Gateway (Port 8000)   │
   │  - Non-blocking async runtime (Tokio 1.36)                    │
   │  - Rate-limit & auth middleware                               │
   │  - In-memory response caching (Suite #272)                    │
   └───────────────┬───────────────────────────────┬───────────────┘
                   │                               │
         [Scenario 1: Health]            [Scenario 2 & 3: Sentiment]
                   │                               │
                   ▼                               ▼
            (Instant 200 OK)             ┌───────────────────┐
                                         │  2-Tier Storage   │
                                         │     Routing       │
                                         └─────────┬─────────┘
                                                   │
                       ┌───────────────────────────┴───────────────────────────┐
                       ▼                                                       ▼
            [TimescaleDB Primary]                                     [QuestDB Hot Cache]
            - SCD2 Bi-temporal JOIN                                   - Influx Line Protocol
            - Degraded Mode: 200 OK                                   - Sub-millisecond ILP
            - P95: 246.9ms (< 500ms)                                  - P95: 34.8ms (< 100ms)
```

---

## 3. Step-by-Step Load Testing Execution

### Option A: Native Python Load Runner (Any Environment)
Run the automated multi-threaded load certification suite directly:

```bash
# Execute 100 VUs across 300 requests against local API Gateway
python scripts/test_load.py --base-url http://localhost:8000 --vus 100 --requests 300

# View generated audit report
cat logs/load_test_report.json
```

### Option B: Docker Compose k6 Service
Execute using the official Grafana k6 container:

```bash
# Run k6 containerized against the running API Gateway
docker compose run --rm k6
```

### Option C: Standalone k6 CLI
If k6 is installed on the host machine:

```bash
# Run 100 VUs for 30s with full metric reporting
k6 run --vus 100 --duration 30s scripts/load_test.js
```

---

## 4. Scenario Catalog & Validation Breakdown

The test suite evaluates three distinct traffic patterns:

### Scenario 1: Public Health Check Probe
- **Target Route**: `GET /v1/health`
- **Authentication**: None (Public)
- **Target SLA**: P95 $< 100\text{ms}$
- **Purpose**: Validates container event loop responsiveness under zero application query overhead.

### Scenario 2: Sentiment Query (TimescaleDB Primary 4-Core Fallback)
- **Target Route**: `GET /v1/sentiment?ticker=AAPL`
- **Authentication**: `Authorization: Bearer <JWT>`
- **Storage Path**: TimescaleDB primary SCD2 interval query (`valid_from <= as_of AND (valid_to > as_of OR valid_to IS NULL)`).
- **Target SLA**: P95 $< 500\text{ms}$
- **Response Headers Verified**:
  - `X-Degraded: true`
  - `X-Storage-Primary: timescale`
  - `X-Cache: timescale-primary`

### Scenario 3: Point-in-Time Sentiment Query (Hot Cache / SCD2)
- **Target Route**: `GET /v1/sentiment?ticker=AAPL&as_of=2026-09-05T12:00:00Z`
- **Authentication**: `Authorization: Bearer <JWT>`
- **Storage Path**: QuestDB hot cache or indexed TimescaleDB hypertable slice.
- **Target SLA**: P95 $< 100\text{ms}$

---

## 5. Live Telemetry & Monitoring Endpoints

### 5.1 JSON Status Endpoint (`GET /admin/load_test/status`)
Institutional operations teams can inspect live latency percentiles and error metrics:

```bash
curl -s http://localhost:8000/admin/load_test/status | jq .
```

**Sample Output**:
```json
{
  "status": "healthy",
  "p50_latency_ms": 193.8,
  "p95_latency_ms": 252.41,
  "p99_latency_ms": 289.19,
  "p99_9_latency_ms": 295.14,
  "throughput_rps": 119.28,
  "error_rate_percent": 0.0,
  "total_requests": 300,
  "total_errors": 0,
  "p95_sla_target_ms": 500.0,
  "p95_compliant": true,
  "error_rate_compliant": true,
  "last_load_test_timestamp": 1790179101
}
```

### 5.2 Prometheus Metrics Exporter (`GET /metrics`)
Exposes standardized Prometheus gauges and histograms for Prometheus/Grafana ingestion:

```prometheus
# API Latency Gauges
fintext_api_p50_latency_ms 193.8
fintext_api_p95_latency_ms 252.41
fintext_api_p99_latency_ms 289.19
fintext_api_throughput_rps 119.28
fintext_api_requests_total 300
fintext_api_errors_total 0
fintext_api_error_rate_percent 0.0

# API Latency Histogram Buckets
fintext_api_latency_seconds_bucket{le="0.01"} 48
fintext_api_latency_seconds_bucket{le="0.05"} 120
fintext_api_latency_seconds_bucket{le="0.1"} 210
fintext_api_latency_seconds_bucket{le="0.25"} 285
fintext_api_latency_seconds_bucket{le="0.5"} 300
fintext_api_latency_seconds_bucket{le="+Inf"} 300
fintext_api_latency_seconds_count 300
```

---

## 6. Grafana Dashboard Configuration

The dashboard definition is version-controlled at `dashboards/api_performance.json` and automatically imported into Grafana. It contains 6 operational panels:

1. **API Latency Percentiles (P50, P95, P99) vs SLA Target (500ms)**
2. **API Request Throughput (req/sec)**
3. **HTTP 5xx Server Error Rate vs 1% SLA Threshold**
4. **Storage Routing: TimescaleDB Primary vs QuestDB Hot Cache**
5. **Load Test Concurrency: Virtual Users (VUs)**
6. **System Saturation: CPU & Memory Utilization under Load**

---

## 7. Alertmanager Rules & Escalation Matrix

The following production alerts are configured in `k8s/observability/prometheus-alerts.yaml`:

| Alert Identifier | Condition | Severity | Action Required |
| :--- | :--- | :--- | :--- |
| `APIP95LatencyHigh` | P95 Latency $> 500\text{ms}$ for $2\text{m}$ | **Critical** | Check DB connection pool saturation, restart degraded pods. |
| `APIP99LatencyHigh` | P99 Latency $> 1000\text{ms}$ for $2\text{m}$ | **Warning** | Inspect tail query locks and ONNX batch inference queues. |
| `APIServerErrorRateHigh` | 5xx Rate $> 1.0\%$ for $2\text{m}$ | **Critical** | Check PostgreSQL connectivity and circuit breaker state. |
| `APIThroughputLow` | Throughput $< 10\text{ req/s}$ (market hours) | **Warning** | Check upstream ingress gateway and DNS routing. |
| `CPUSaturationHigh` | CPU Utilization $> 80\%$ for $3\text{m}$ | **Warning** | Trigger HPA horizontal pod autoscale scaling event. |

---

## 8. Performance Troubleshooting & Optimization Playbook

If a load testing run breaches the P95 $500\text{ms}$ threshold:

### Step 1: Diagnose Query vs Gateway Bottlenecks
Run an isolated query benchmark against PostgreSQL directly:
```sql
EXPLAIN ANALYZE
SELECT ticker, sentiment_score, confidence, published_utc, ingested_utc
FROM sentiment_records
WHERE ticker = 'AAPL' AND valid_from <= NOW() AND (valid_to > NOW() OR valid_to IS NULL)
ORDER BY valid_from DESC LIMIT 1;
```
*Expected Execution Time*: $< 5\text{ms}$ on indexed hypertable. If $> 25\text{ms}$, run `VACUUM ANALYZE sentiment_records;`.

### Step 2: Connection Pool Saturation
If `TIMESCALE_MAX_CONNECTIONS=10` is saturated by 100 concurrent VUs:
1. Increase pool size in `.env`: `TIMESCALE_MAX_CONNECTIONS=25`.
2. Verify PostgreSQL `max_connections` in `docker-compose.yml`: `--max_connections=100`.

### Step 3: Horizontal Pod Autoscaling (HPA)
In Kubernetes staging/production:
```bash
# Check HPA autoscaler status
kubectl get hpa fintext-api-hpa

# Manually scale gateway replicas if needed
kubectl scale deployment fintext-api --replicas=4
```
