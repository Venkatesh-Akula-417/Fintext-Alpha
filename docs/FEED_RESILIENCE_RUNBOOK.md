# FinText Alpha Vectorizer — Feed Resilience & Circuit Breaker Ops Runbook
**Document ID:** `DOC-OPS-RESILIENCE-2026-V1`  
**Classification:** Institutional SRE & Trading Operations Manual  
**Last Updated:** 2026-09-26  
**Problem Reference:** Problem #15 — Feed Outages Bound into Labeled Degradation & Stale Fallbacks  
**Authoritative References:**  
[`rust/ingestion_engine/src/resilience/breaker.rs`](file:///d:/FinText-Alpha-Vectorizer/rust/ingestion_engine/src/resilience/breaker.rs) |  
[`rust/ingestion_engine/src/telemetry/metrics.rs`](file:///d:/FinText-Alpha-Vectorizer/rust/ingestion_engine/src/telemetry/metrics.rs) |  
[`dashboards/feed_resilience.json`](file:///d:/FinText-Alpha-Vectorizer/dashboards/feed_resilience.json) |  
[`scripts/run_feed_chaos.py`](file:///d:/FinText-Alpha-Vectorizer/scripts/run_feed_chaos.py) |  
[`docs/PRODUCTION_OPS_RUNBOOK.md`](file:///d:/FinText-Alpha-Vectorizer/docs/PRODUCTION_OPS_RUNBOOK.md) |  
[`docs/API_CUSTOMER_GUIDE.md`](file:///d:/FinText-Alpha-Vectorizer/docs/API_CUSTOMER_GUIDE.md)

---

## 1. Executive Summary & Core Invariant

For an institutional platform selling event-driven alpha to mid-frequency quantitative funds:
> **A feed vendor outage must degrade gracefully into labeled stale/slower data — NEVER into silent gaps or pipeline crashes.**

When an upstream vendor (Finnhub, Polygon.io, SEC EDGAR, Federal Reserve, Corporate Actions) disconnects, rate-limits, or stalls, the Ingestion Engine's per-source pure-logic circuit breaker trips. The pipeline immediately transitions into bounded REST degradation or rate-limited stale serving, exposing clear operational status to ops (`GET /providers` on port 9102) and labeling all emitted events (`primary`, `degraded`, `stale`).

```
                    ┌─────────────────────────┐
                    │      CLOSED (0)         │◄────────────────┐
                    │  Mode: PRIMARY          │                 │
                    │  All requests allowed   │                 │ Probe
                    └───────────┬─────────────┘                 │ Success
                                │ 5 consecutive failures        │
                                │ OR >=50% errs in 60s window   │
                                ▼                               │
                    ┌─────────────────────────┐                 │
         Failure    │       OPEN (2)          │                 │
      ┌────────────►│  Mode: DEGRADED / STALE │                 │
      │ (Doubles    │  Fast-reject with       │                 │
      │  Backoff)   │  exponential backoff    │                 │
      │             └───────────┬─────────────┘                 │
      │                         │ Backoff timer elapses         │
      │                         │ (30s doubling to 300s cap)    │
      │                         ▼                               │
      │             ┌─────────────────────────┐                 │
      └─────────────┤     HALF-OPEN (1)       │─────────────────┘
                    │  Admits exactly 1 probe │
                    └─────────────────────────┘
```

---

## 2. Upstream Degradation Matrix

| Source | Primary Ingestion Path | Fallback Degradation Path | Fallback Cadence | Lag Budget Increase | Customer-Visible Impact |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **`finnhub_ws`** | Real-time WebSocket streaming (`wss://ws.finnhub.io`) | Finnhub REST Poller (`/news?category=general`) | **2.0 seconds** | $+ \le 3.0\text{ s}$ | P95 lag increases from sub-second to ~2.5s. All articles labeled `mode=degraded`. Zero gap in news coverage. |
| **`polygon_ws`** | Options Microstructure WebSocket (`wss://delayed.polygon.io`) | Polygon REST Snapshot (`/v2/aggs/ticker/SPY/prev`) | **5.0 seconds** | $+ \le 6.0\text{ s}$ | VPIN/GEX updates transition from trade ticks to 5s bar snapshots. Labeled `mode=degraded`. |
| **`sec_edgar`** | Direct CIK filings poll with conditional ETag caching | Exponential backoff retry (30s..300s); transitions to Stale after 10m outage | **Indexed backoff** | Outage duration | Filings delivered upon vendor recovery; if outage $>10$m, emitted events marked `mode=stale`. |
| **`fomc`** | HTTP/2 micro-burst statement poller (10ms) | Serve cached FOMC schedule & policy calendar with `stale=true` | **$\le$ once / 5 min** | Stale cache | Quant consumers receive last verified policy stance with `stale=true`. Zero pipeline panic. |
| **`corporate_actions`** | Scheduled SEC bulk ticker change poller (`sec_updater`) | Replay previous-day parquet snapshot with `stale=true` | **$\le$ once / 5 min** | 24 hours | Symbology mapping falls back to yesterday's certified snapshot. |

---

## 3. Provider Telemetry & Observability (Port 9102)

The Ingestion Engine hosts an internal, SG-confined zero-dependency telemetry server on port `9102`:

### 3.1 `GET /providers` (JSON Diagnostic Route)
Returns sorted JSON keys with current breaker state, backoff, and mode for each of the 5 institutional sources:

```bash
curl -s http://127.0.0.1:9102/providers | jq .
```

```json
{
  "corporate_actions": {
    "backoff_s": 0,
    "consecutive_failures": 0,
    "last_success_ts": 1727351234,
    "mode": "primary",
    "state": "closed"
  },
  "finnhub_ws": {
    "backoff_s": 0,
    "consecutive_failures": 0,
    "last_success_ts": 1727351234,
    "mode": "primary",
    "state": "closed"
  },
  "fomc": {
    "backoff_s": 0,
    "consecutive_failures": 0,
    "last_success_ts": 1727351234,
    "mode": "primary",
    "state": "closed"
  },
  "polygon_ws": {
    "backoff_s": 0,
    "consecutive_failures": 0,
    "last_success_ts": 1727351234,
    "mode": "primary",
    "state": "closed"
  },
  "sec_edgar": {
    "backoff_s": 0,
    "consecutive_failures": 0,
    "last_success_ts": 1727351234,
    "mode": "primary",
    "state": "closed"
  }
}
```

### 3.2 Prometheus Metrics Exposition (`GET /metrics`)
Exposes 3 fixed-cardinality metric families:
- `fintext_provider_state{source}`: Gauge (`0 = Closed`, `1 = Half-Open`, `2 = Open`)
- `fintext_fallback_active{source}`: Gauge (`0 = Primary`, `1 = Degraded/Stale`)
- `fintext_fallback_activations_total{source}`: Counter (Trips to Open)
- `fetch_duration_seconds{source, mode, le}`: Histogram per source and mode
- `event_lag_seconds{source, mode, le}`: Histogram per source and mode

Grafana Dashboard: [`dashboards/feed_resilience.json`](file:///d:/FinText-Alpha-Vectorizer/dashboards/feed_resilience.json) (7 panels, Prometheus-only).

---

## 4. Operational Incident Response Cards

### Response Card 1: Upstream Circuit Breaker Open > 5 Minutes
- **Trigger**: `fintext_provider_state{source} == 2` for $> 5$ minutes, or CloudWatch Alarm `ProviderCircuitBreakerOpen` fires SNS alert.
- **Severity**: SEV-2 (Degraded Ingestion)
- **Automated State**: Fallback active (REST polling at 2s/5s cadence or stale serving). Customers receiving continuous signals with `mode=degraded`.
- **Immediate Diagnostic Steps**:
  1. Inspect provider health: `curl -s http://127.0.0.1:9102/providers | jq .`
  2. Inspect vendor status pages:
     - Finnhub: `https://status.finnhub.io/`
     - Polygon.io: `https://status.polygon.io/`
     - SEC EDGAR: `https://www.sec.gov/edgar/searchedgar/edgarsubmissionstatus`
  3. Verify fallback data flow in Grafana: Dashboard `FinText Alpha — Upstream Feed Resilience & Circuit Breakers`.
  4. If vendor has confirmed outage, no manual restart of ingestion engine is required — auto-recovery will probe and close breaker automatically once vendor restores service.

### Response Card 2: Upstream Source Stale > 60 Minutes
- **Trigger**: Any provider in `mode=stale` for $> 60$ minutes.
- **Severity**: SEV-1 (Extended Upstream Outage)
- **Action**: Dispatch Customer Advisory via Cohort-1 Communication Template.

#### Cohort-1 Communication Template
```
To: institutional-ops@cohort1-funds.internal
From: noc@fintext-alpha.com
Subject: [ADVISORY] Upstream Feed Degradation Notice — {SOURCE} ({VENDOR})

Dear Institutional Partner,

At {TIMESTAMP_UTC}, our automated resilience monitors detected an upstream feed 
interruption from {VENDOR} affecting {SOURCE}.

Current Operational Status:
- Ingestion Pipeline: ACTIVE (Graceful Degradation Mode)
- Current Mode: DEGRADED / STALE (Emitted signals tagged accordingly)
- Data Correctness: Point-in-time database invariants strictly maintained; zero look-ahead bias
- Latency Impact: Real-time socket stream temporarily replaced with bounded REST snapshot poll

Our engineering team is actively tracking vendor status. Once {VENDOR} resolves the 
upstream outage, the platform will automatically transition back to Primary WebSocket 
streaming with zero downtime.

Real-time platform status is available via GET /v1/health/providers or our internal portal.

FinText Alpha Operations Team
```

---

## 5. Cross-Links to Production Ops Runbook

This runbook integrates directly with the main Day-2 Operations Runbook:
- Staged SNS Alerts: See [`infra/terraform/sns.tf`](file:///d:/FinText-Alpha-Vectorizer/infra/terraform/sns.tf) and [`docs/PRODUCTION_OPS_RUNBOOK.md`](file:///d:/FinText-Alpha-Vectorizer/docs/PRODUCTION_OPS_RUNBOOK.md) §4.
- Cron Monitoring: See `PRODUCTION_OPS_RUNBOOK.md` §2.4.
- Chaos Testing: Execute `python scripts/run_feed_chaos.py` weekly during DR drills.
