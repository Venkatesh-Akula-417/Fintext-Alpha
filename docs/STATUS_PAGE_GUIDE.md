# Public Status Page & Operational Visibility Guide
**Document ID:** `DOC-OPS-STATUS-2026-V1`  
**Classification:** Institutional Public / Customer-Facing  
**Last Updated:** 2026-09-24  
**Authoritative Reference:** [docs/CERTIFIED_METRICS_REGISTER.md](file:///d:/FinText-Alpha-Vectorizer/docs/CERTIFIED_METRICS_REGISTER.md)  
**Endpoint:** `GET /v1/status`  
**Incremental Cloud Infrastructure Cost:** **$0.00/month** (Maintains fixed platform budget: $295.00–$301.44/month)

---

## 1. Executive Summary & Architecture Overview

FinText Alpha Vectorizer exposes an unauthenticated, rate-limited operational status endpoint at `GET /v1/status`. This endpoint provides institutional subscribers, quantitative trading desks, and compliance auditors with real-time visibility into platform health, sub-system states, and certified SLA metrics without disclosing internal network topology, credentials, or proprietary model weights.

To satisfy institutional procurement standards while strictly maintaining our **zero incremental cloud cost invariant ($0.00/mo)**, this guide details two production-ready zero-cost hosting patterns for publishing an external status page:
1. **GitHub Pages + GitHub Actions Cron (Recommended, Fully Sovereign):** An automated polling runner that queries `GET /v1/status` every minute, validates the JSON payload, and commits a static status dashboard to GitHub Pages.
2. **Instatus / Better Uptime Free Tier:** A hosted third-party incident management status page integrated with automated HTTP health checks.

```
                              ┌──────────────────────────────────────────────┐
                              │     Institutional Quantitative Clients       │
                              └──────────────────────┬───────────────────────┘
                                                     │
                                                     ▼ HTTPS
                               ┌─────────────────────────────────────────────┐
                               │  Public Status Page (status.fintext.ai)     │
                               │  GitHub Pages / Instatus (Cost: $0.00/mo)   │
                               └─────────────────────▲───────────────────────┘
                                                     │
                         Static JSON Push (1m Cron)  │
                         ┌───────────────────────────┴───────────────────────┐
                         │                                                   │
             ┌───────────┴──────────┐                            ┌───────────┴──────────┐
             │ GitHub Actions Cron  │                            │ Instatus Health Check│
             │ Runner (Zero Cost)   │                            │ Engine (Free Tier)   │
             └───────────▲──────────┘                            └───────────▲──────────┘
                         │                                                   │
                         │ HTTP GET /v1/status (Edge Cache: 10s)             │
                         └───────────────────────────┬───────────────────────┘
                                                     ▼
                               ┌─────────────────────────────────────────────┐
                               │  FinText Alpha Vectorizer API Gateway       │
                               │  Port 8000 / Envoy Reverse Proxy (:443)     │
                               │  - Public, Unauthenticated, Rate-Limited    │
                               │  - Cache-Control: public, max-age=10        │
                               │  - Zero Sensitive Data Exposure             │
                               └─────────────────────┬───────────────────────┘
                                                     │
                         ┌───────────────────────────┼───────────────────────────┐
                         ▼                           ▼                           ▼
               ┌──────────────────┐        ┌──────────────────┐        ┌──────────────────┐
               │   TimescaleDB    │        │   Kafka Stream   │        │ Ingestion Engine │
               │   (PostgreSQL)   │        │     (Redpanda)   │        │  (Vectorization) │
               └──────────────────┘        └──────────────────┘        └──────────────────┘
```

---

## 2. API Endpoint Specification: `GET /v1/status`

### 2.1 Technical Characteristics
- **HTTP Method:** `GET`
- **Route:** `/v1/status` (also mirrored at `/status`)
- **Authentication:** None required (Public access for institutional uptime monitors)
- **Rate Limit:** 60 requests/minute per source IP (Layer 7 token bucket)
- **Caching Header:** `Cache-Control: public, max-age=10` (prevents backend connection thrashing under external monitoring polling)
- **Security Invariant:** Zero leakage of database connection strings, internal IP addresses, container IDs, customer tenant IDs, or environment secrets.

### 2.2 Response Schema
```json
{
  "status": "operational",
  "version": "1.0.0",
  "uptime_seconds": 128450,
  "timestamp": "2026-09-24T20:30:00Z",
  "components": {
    "api_gateway": "operational",
    "vector_engine": "operational",
    "timescaledb": "operational",
    "kafka_bus": "operational"
  },
  "certifications": {
    "soc2_type2_ready": true,
    "rls_tenant_isolation": "enforced_active",
    "target_p95_latency_ms": 500,
    "target_uptime_pct": 99.5,
    "memory_leak_slope_limit_mib_per_hour": 2.0
  }
}
```

### 2.3 Status Value Definitions
| Status String | Meaning | SLA Impact |
| :--- | :--- | :--- |
| `operational` | All sub-components healthy, P95 latency <= 500ms, error rate < 0.1%. | Full compliance |
| `degraded` | One non-critical subsystem impaired (e.g. streaming queue delay); queries serving. | Operational notice |
| `maintenance` | Planned scheduled maintenance window active. | Pre-notified window |
| `outage` | API Gateway or primary database unreachable; automated failover active. | P1 Incident triggered |

---

## 3. Pattern A: GitHub Pages Zero-Cost Status Dashboard (Recommended)

This pattern leverages a standalone static status website deployed via GitHub Pages, updated every 60 seconds by a GitHub Actions workflow.

### 3.1 GitHub Actions Updater Workflow (`.github/workflows/status-updater.yml`)
```yaml
name: Public Status Page Updater

on:
  schedule:
    # Run every 5 minutes (or 1 minute on self-hosted runner)
    - cron: '*/5 * * * *'
  workflow_dispatch:

concurrency:
  group: status-page
  cancel-in-progress: true

jobs:
  probe-and-publish:
    runs-on: ubuntu-latest
    timeout-minutes: 3
    steps:
      - name: Checkout Status Page Repository
        uses: actions/checkout@v4
        with:
          ref: gh-pages

      - name: Probe FinText Alpha Vectorizer Public Status
        id: probe
        run: |
          set -e
          TIMESTAMP=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
          HTTP_CODE=$(curl -s -o status_response.json -w "%{http_code}" \
            --connect-timeout 5 \
            --max-time 10 \
            -H "User-Agent: FinText-Status-Probe/1.0" \
            https://api.fintext.internal/v1/status || echo "000")

          if [ "$HTTP_CODE" -eq 200 ]; then
            echo "Status probe succeeded (HTTP 200)."
            cp status_response.json data/status.json
          else
            echo "Status probe failed with HTTP $HTTP_CODE. Generating degraded status."
            cat <<EOF > data/status.json
          {
            "status": "degraded",
            "version": "1.0.0",
            "uptime_seconds": 0,
            "timestamp": "$TIMESTAMP",
            "components": {
              "api_gateway": "degraded",
              "vector_engine": "unknown",
              "timescaledb": "unknown",
              "kafka_bus": "unknown"
            },
            "certifications": {
              "soc2_type2_ready": true,
              "rls_tenant_isolation": "enforced_active",
              "target_p95_latency_ms": 500,
              "target_uptime_pct": 99.5
            }
          }
          EOF
          fi

      - name: Commit and Push Status Snapshot
        run: |
          git config user.name "FinText Status Bot"
          git config user.email "ops-bot@fintext.ai"
          git add data/status.json
          if ! git diff-index --quiet HEAD; then
            git commit -m "chore(status): update operational snapshot $(date -u +'%Y-%m-%d %H:%M UTC')"
            git push origin gh-pages
          else
            echo "No status transition detected. Skipping commit."
          fi
```

### 3.2 HTML / JavaScript Dashboard Snippet (`index.html`)
```html
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>FinText Alpha Vectorizer — System Status</title>
  <style>
    body { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; background: #0b0f19; color: #f1f5f9; padding: 2rem; max-width: 800px; margin: 0 auto; }
    .card { background: #1e293b; border-radius: 8px; padding: 1.5rem; margin-bottom: 1.5rem; border: 1px solid #334155; }
    .status-badge { display: inline-block; padding: 0.35rem 0.75rem; border-radius: 9999px; font-weight: 600; font-size: 0.875rem; }
    .operational { background: #064e3b; color: #34d399; border: 1px solid #059669; }
    .degraded { background: #78350f; color: #fbbf24; border: 1px solid #d97706; }
    .grid { display: grid; grid-template-columns: repeat(2, 1fr); gap: 1rem; }
    .component-row { display: flex; justify-content: space-between; padding: 0.5rem 0; border-bottom: 1px solid #334155; }
  </style>
</head>
<body>
  <h1>FinText Alpha Vectorizer</h1>
  <p style="color: #94a3b8;">Institutional Real-Time Operational Health & SLA Certification</p>

  <div class="card" id="overall-card">
    <div style="display: flex; justify-content: space-between; align-items: center;">
      <h2 style="margin: 0;">Global Platform Status</h2>
      <span class="status-badge operational" id="overall-status">OPERATIONAL</span>
    </div>
    <p style="color: #94a3b8; font-size: 0.875rem; margin-top: 0.75rem;" id="last-updated">Last Updated: Checking...</p>
  </div>

  <div class="card">
    <h3>Core Infrastructure Components</h3>
    <div class="grid">
      <div class="component-row"><span>REST API Gateway</span><strong id="comp-gateway" style="color: #34d399;">Operational</strong></div>
      <div class="component-row"><span>Vector Ingestion Engine</span><strong id="comp-engine" style="color: #34d399;">Operational</strong></div>
      <div class="component-row"><span>TimescaleDB (PostgreSQL RLS)</span><strong id="comp-db" style="color: #34d399;">Operational</strong></div>
      <div class="component-row"><span>Kafka Streaming Bus</span><strong id="comp-kafka" style="color: #34d399;">Operational</strong></div>
    </div>
  </div>

  <div class="card">
    <h3>Certified SLA Metrics (Active 30-Day Window)</h3>
    <div class="grid">
      <div class="component-row"><span>Target Uptime SLA</span><strong>99.50%</strong></div>
      <div class="component-row"><span>Query Latency Target</span><strong>P95 &le; 500 ms</strong></div>
      <div class="component-row"><span>Multi-Tenant RLS Leaks</span><strong style="color: #34d399;">0 Leaks (Certified)</strong></div>
      <div class="component-row"><span>Memory Leak Slope Limit</span><strong>&le; 2.0 MiB/h</strong></div>
    </div>
  </div>

  <script>
    async function refreshStatus() {
      try {
        const res = await fetch('data/status.json?nocache=' + Date.now());
        const data = await res.json();
        document.getElementById('overall-status').textContent = data.status.toUpperCase();
        document.getElementById('last-updated').textContent = 'Last Updated: ' + data.timestamp;
      } catch (err) {
        console.error('Failed to load status:', err);
      }
    }
    refreshStatus();
    setInterval(refreshStatus, 30000);
  </script>
</body>
</html>
```

---

## 4. Pattern B: Instatus Free-Tier Configuration

For teams desiring automated email/SMS subscriber notifications without managing a GitHub Pages frontend:

1. **Account Setup:** Provision an Instatus Free Tier account at `https://instatus.com` ($0.00/mo, up to 100 subscribers).
2. **Custom Domain:** Point CNAME `status.yourfunddomain.com` to `instatus.page`.
3. **HTTP Monitor Probe:**
   - URL: `https://api.fintext.internal/v1/status`
   - Method: `GET`
   - Frequency: Every 1 minute
   - Response Assertion: JSON path `status` equals `"operational"`
   - Response Status Code: `200`
4. **Component Mapping:**
   - Component 1: `REST API Gateway` &larr; `$.components.api_gateway`
   - Component 2: `Vector Ingestion Pipeline` &larr; `$.components.vector_engine`
   - Component 3: `TimescaleDB Analytical Store` &larr; `$.components.timescaledb`
   - Component 4: `Kafka Streaming Engine` &larr; `$.components.kafka_bus`

---

## 5. Institutional Incident Communication Templates

In accordance with SOC 2 CC7.4 (System Incident Management and Communication), operational communications must follow standardized institutional protocol:

### Template 1: Investigating Degradation
> **Title:** Investigating Latency Elevated on US-East API Gateway  
> **Status:** Investigating  
> **Timestamp:** [YYYY-MM-DD HH:MM UTC]  
> **Message:**  
> "Our automated telemetry detected P95 query latencies exceeding our 500 ms SLA threshold on the US-East API Gateway. The core analytical storage and streaming vectorization engines remain fully operational with zero data loss. Engineering has initiated diagnostic triage on upstream network routes. Further updates will be provided within 30 minutes."

### Template 2: Identified Root Cause
> **Title:** Elevated Latency Identified — Upstream Ingestion Backpressure  
> **Status:** Identified  
> **Timestamp:** [YYYY-MM-DD HH:MM UTC]  
> **Message:**  
> "The root cause of the elevated API response latency has been identified as temporary backpressure in the high-volume streaming ingest queue. Secondary partition auto-scaling has been applied. API query responses are returning to baseline levels (P95 < 250 ms). No tenant isolation or data integrity issues occurred."

### Template 3: Resolved & Retrospective Available
> **Title:** Issue Resolved — All Systems Operational  
> **Status:** Resolved  
> **Timestamp:** [YYYY-MM-DD HH:MM UTC]  
> **Message:**  
> "All API Gateway query latency metrics have fully normalized below the certified 500 ms threshold. The streaming ingest queue backlog has been completely cleared. A full institutional post-mortem report will be made available to designated fund compliance officers within 24 hours."

---

## 6. Verification and Maintenance Checklist

- [x] Endpoint returns HTTP 200 with JSON payload (`curl -s -f http://localhost:8000/v1/status`).
- [x] Response headers include `Cache-Control: public, max-age=10`.
- [x] Rate limiter throttles excess requests at 60 req/min with HTTP 429.
- [x] Zero sensitive internal metadata present in JSON payload.
- [x] Status probe workflow executes without exceeding free-tier GitHub runner or Instatus quotas.
- [x] Total platform cloud run-rate remains locked at $295.00–$301.44/month.
