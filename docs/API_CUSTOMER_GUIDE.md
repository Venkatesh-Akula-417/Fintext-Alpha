# FinText Alpha Vectorizer — Customer API Guide (Bike Manual Style)
### 15 Minutes to First Signal | Institutional Quantitative Gateway Documentation

> **Document Version**: v1.0.0 (Production Core Release)  
> **Last Verified**: 2026-09-17 (Suite #274) | **Audit Readiness**: Certified Clean  
> **Interactive Research Suite**: [`notebooks/`](../notebooks/README.md) | **Model Card**: [`docs/SIGNAL_QUALITY_REPORT.md`](./SIGNAL_QUALITY_REPORT.md)  
> **Authoritative Deprecations**: [`docs/DEPRECATED.md`](./DEPRECATED.md) | **Onboarding Runbook**: [`docs/PRIVATE_BETA_ONBOARDING_RUNBOOK.md`](./PRIVATE_BETA_ONBOARDING_RUNBOOK.md)  
> **Security DDQ Pack**: [`docs/SECURITY_QUESTIONNAIRE_RESPONSES.md`](./SECURITY_QUESTIONNAIRE_RESPONSES.md) | **Tenant Isolation**: [`docs/TENANT_ISOLATION_RUNBOOK.md`](./TENANT_ISOLATION_RUNBOOK.md)  
> **Certified Metrics**: [`docs/CERTIFIED_METRICS_REGISTER.md`](./CERTIFIED_METRICS_REGISTER.md) | **Soak Runbook**: [`docs/SOAK_STABILITY_RUNBOOK.md`](./SOAK_STABILITY_RUNBOOK.md) | **Status Guide**: [`docs/STATUS_PAGE_GUIDE.md`](./STATUS_PAGE_GUIDE.md)

---

## 1. Quick Start: First Signal in 60 Seconds

Like tuning a high-performance derailleur, connecting to FinText is fast and mechanical. Run these three commands in your terminal:

```bash
# 1. Spin up the 4 core platform services (Postgres+TimescaleDB, Redpanda Kafka, Ingestion, API)
docker compose up -d

# 2. Acquire your Bearer JWT Token using your admin token
# (Set ADMIN_TOKEN environment variable or use the development secret from .env)
export ADMIN_TOKEN="${ADMIN_TOKEN:-your_admin_token_here}"

TOKEN=$(curl -s -X POST http://127.0.0.1:8000/v1/auth/token \
  -H "X-Admin-Token: ${ADMIN_TOKEN}" \
  -H "Content-Type: application/json" \
  -d '{"user_id": "quant_researcher_01", "role": "admin", "ttl_seconds": 86400}' | grep -o '"token":"[^"]*' | cut -d'"' -f4)

# 3. Pull your first Point-in-Time financial sentiment score for Apple Inc. (AAPL)
curl -s -H "Authorization: Bearer $TOKEN" "http://127.0.0.1:8000/v1/sentiment?ticker=AAPL"
```

**Expected Response**:
```json
{
  "ticker": "AAPL",
  "sentiment_score": 0.4215,
  "sentiment_label": "POSITIVE",
  "data_quality_score": 0.942,
  "confidence": 0.884,
  "timestamp": "2026-09-17T13:30:00Z"
}
```

---

## 2. Authentication & Credential Architecture

FinText enforces multi-tiered institutional authentication. Think of it as a 3-tier lock system on your bike rack:

```
┌────────────────────────────────────────────────────────────────────────────────────────┐
│                              AUTHENTICATION TIERS                                      │
├──────────────────┬──────────────────────┬──────────────────────────────────────────────┤
│ Tier             │ Header Format        │ Scope & Primary Use Case                     │
├──────────────────┼──────────────────────┼──────────────────────────────────────────────┤
│ 1. Admin Token   │ `X-Admin-Token: ...` │ Token issuance & system provisioning         │
│ 2. Bearer JWT    │ `Authorization: ...` │ Authenticated HTTP REST & WebSocket sessions │
│ 3. API Keys      │ `X-API-Key: ...`     │ High-speed programmatic headless ingestion   │
└──────────────────┴──────────────────────┴──────────────────────────────────────────────┘
```

### 2.1 Generating a Bearer Token (`/v1/auth/token`)
Tokens are signed using HMAC-SHA256 and embed user role claims (`user`, `analyst`, `trader`, `admin`).
```bash
curl -X POST http://127.0.0.1:8000/v1/auth/token \
  -H "X-Admin-Token: ${ADMIN_TOKEN}" \
  -H "Content-Type: application/json" \
  -d '{
    "user_id": "quant_fund_desk_01",
    "role": "admin",
    "ttl_seconds": 86400
  }'
```

### 2.2 Tenant API-Key Self-Service Lifecycle (`/v1/account/keys`)

Institutions can manage their entire API key lifecycle programmatically without contacting support:

1. **Create API Key (`POST /v1/account/keys`)**:
   Generates a cryptographically secure key with 256 bits of entropy and the standard `ft_live_` prefix (e.g. `ft_live_7sK2...`):
   ```bash
   curl -X POST http://127.0.0.1:8000/v1/account/keys \
     -H "Authorization: Bearer $TOKEN" \
     -H "Content-Type: application/json" \
     -d '{"name": "Production Quant Execution", "expires_in_days": 90}'
   ```
   **Security Guarantee**: The plaintext key is returned strictly once in `plaintext_once`. It is never stored or logged in plain text. A strict maximum quota of 10 active keys per tenant is enforced.

2. **List API Keys (`GET /v1/account/keys`)**:
   Returns sanitized metadata for all keys owned by your organization (ID, name, safe 16-character prefix, status, expiration, and last seen). Secret tokens and hashes are never exposed:
   ```bash
   curl -s -H "Authorization: Bearer $TOKEN" http://127.0.0.1:8000/v1/account/keys
   ```

3. **Rotate API Key (`POST /v1/account/keys/{key_id}/rotate`)**:
   Atomically revokes the target key and provisions a replacement key, preserving audit lineage (`rotated_from`):
   ```bash
   curl -X POST http://127.0.0.1:8000/v1/account/keys/{key_id}/rotate \
     -H "Authorization: Bearer $TOKEN"
   ```

4. **Revoke API Key (`DELETE /v1/account/keys/{key_id}`)**:
   Immediately revokes a key. Revoked keys are rejected with `401 Unauthorized` on all authenticated endpoints:
   ```bash
   curl -X DELETE http://127.0.0.1:8000/v1/account/keys/{key_id} \
     -H "Authorization: Bearer $TOKEN"
   ```

### 2.3 Rate Limiting & Quota Transparency Headers

Every response from protected endpoints includes institutional quota transparency headers:

| Header Name | Format / Example | Description |
| :--- | :--- | :--- |
| `X-RateLimit-Limit` | `100` | Maximum request capacity allowed per rate limit window |
| `X-RateLimit-Remaining` | `94` | Number of requests remaining in the current window |
| `X-RateLimit-Reset` | `1790424000` | **Unix epoch timestamp** (seconds since 1970-01-01 UTC) when the quota window resets |

**Exempt Paths**: Public operational probes (`/v1/health`, `/v1/status`, `/v1/readyz`, `/v1/metrics`, `/v1/openapi.json`) and billing webhooks (`/v1/billing/webhook`) are exempt from rate limiting to prevent false-positive monitoring outages.

### 2.4 Network Ingress Confinement (IP & CIDR Whitelisting)

For hedge funds and institutional prop desks operating under strict cybersecurity mandates (SOC 2, ISO 27001, SEC Safeguards Rule), FinText allows programmatic confinement of API traffic to authorized IP addresses and CIDR subnets:

```bash
# 1. Register allowed office VPN or cloud execution subnet
curl -X POST http://127.0.0.1:8000/v1/security/ip-whitelist \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"ip_or_cidr": "198.51.100.0/24", "description": "London Quant Desk VPN"}'

# 2. List all registered active ingress CIDR rules
curl -s -H "Authorization: Bearer $TOKEN" http://127.0.0.1:8000/v1/security/ip-whitelist

# 3. Delete an entry by ID (reverts to default open when list is empty)
curl -X DELETE -H "Authorization: Bearer $TOKEN" http://127.0.0.1:8000/v1/security/ip-whitelist/{entry_id}
```

**Security Behavioral Guarantees**:
- **Default Open for New Organizations**: By default (0 rules configured), authenticated requests are permitted from any source IP.
- **Strict Ingress Confinement**: As soon as $\ge 1$ IP/CIDR rule is added, requests originating outside the configured subnets receive an immediate `403 Forbidden` response:
  ```json
  {"error": "Forbidden", "message": "IP address not allowed"}
  ```
- **Operational Health Exemption**: Public health probes (`/v1/health`, `/v1/status`) and billing webhooks (`/v1/billing/webhook`) are decoupled from tenant IP whitelists to ensure continuous external monitoring and Stripe reconciliation.

---

## 3. The 32 Core Endpoints (Component Reference Table)

Every endpoint operates like a precision bicycle component: each has a dedicated purpose, clear input parameters, and tight tolerances.

| # | Method | Endpoint Route | Component Name | As-Of Supported? | Quant Customer Value (Why You Need It) |
| :-: | :---: | :--- | :--- | :---: | :--- |
| **1** | `GET` | `/v1/health` | Diagnostic Probe | No | Real-time platform and dependency status probe. |
| **2** | `GET` | `/v1/readyz` | Cluster Readiness | No | Kubernetes container traffic ingress readiness probe. |
| **3** | `POST`| `/v1/auth/token` | Token Mint | No | Acquire Bearer JWT with custom TTL and RBAC roles. |
| **4** | `GET` | `/v1/users/me` | Identity Inspector | No | Inspect active user profile, tier, and remaining quota. |
| **5** | `POST`| `/v1/users/api-keys` | Key Forge | No | Mint rotating programmatic API keys (`fintext_live_...`). |
| **6** | `GET` | `/v1/users/api-keys` | Key Registry | No | Enumerate all active and revoked API keys for audit. |
| **7** | `GET` | `/v1/model-card` | Lineage Governance | No | Auditable neural architecture, quantization, and calibration. |
| **8** | `GET` | `/v1/sentiment` | Point-in-Time Score | **Yes (`as_of_utc`)**| Instantaneous asset sentiment $(-1.0 \text{ to } +1.0)$. |
| **9** | `POST`| `/v1/sentiment/batch`| Bulk Signal Scanner | **Yes (`as_of_utc`)**| High-throughput multi-ticker portfolio scoring in 1 call. |
| **10**| `GET` | `/v1/sentiment/history`| Historical Stream | **Yes (`as_of_utc`)**| SCD2 bi-temporal historical series for quantitative simulation. |
| **11**| `GET` | `/v1/sentiment/feed` | Real-Time Ticker Feed| No | Live cursor-paginated market-wide sentiment ticker. |
| **12**| `GET` | `/v1/sentiment/entities`| Entity Tagger | **Yes (`as_of_utc`)**| Token-level NER extracted corporate executives and units. |
| **13**| `GET` | `/v1/sentiment/sector` | Sector Heatmap | **Yes (`as_of_utc`)**| GICS sector aggregate sentiment and cap-weighted spread. |
| **14**| `GET` | `/v1/sentiment/disagreement`| Sentiment Dispersion| **Yes (`as_of_utc`)**| Inter-model disagreement and entropy volatility predictor. |
| **15**| `GET` | `/v1/sentiment/anomalies` | Volatility Spike Radar| No | Multi-sigma sentiment deviations flag impending breaks. |
| **16**| `GET` | `/v1/options/iv` | Black-Scholes Greeks | No | Implied volatility, Delta, Gamma, Vega, Theta across strikes. |
| **17**| `GET` | `/v1/options/microstructure`| VPIN & GEX Engine | No | Volume-informed trading (VPIN) and Dealer Gamma (GEX). |
| **18**| `GET` | `/v1/options/unusual` | Unusual Flow Detector| No | Volume/OI spikes flag smart money institutional bets. |
| **19**| `GET` | `/v1/options/vol-surface` | 3D Volatility Mesh | No | Strike vs Expiry volatility smile and skew coordinates. |
| **20**| `GET` | `/v1/options/put-call-ratio`| Flow Sentiment Ratio | No | Real-time and historical open interest Put/Call ratios. |
| **21**| `GET` | `/v1/pit/replay` | Time Machine Replay | **Yes (`as_of_utc`)**| Reconstruct exact historical database state at $T_0$. |
| **22**| `GET` | `/v1/pit/certificate`| Cryptographic Audit | **Yes (`start_date`)**| Signed SHA-256 certificate proving zero look-ahead bias. |
| **23**| `GET` | `/v1/symbols/map` | Permanent Identifier | No | Map Ticker $\leftrightarrow$ CIK $\leftrightarrow$ FIGI $\leftrightarrow$ ISIN across corporate actions. |
| **24**| `GET` | `/v1/events/8k` | Material Disclosures | **Yes (`as_of_utc`)**| Unstructured SEC Form 8-K items scored by severity. |
| **25**| `GET` | `/v1/events/earnings-surprise`| Earnings Alpha Jump | **Yes (`as_of_utc`)**| Consensus EPS vs reported actuals and guidance surprises. |
| **26**| `GET` | `/v1/events/insider-trading`| Form 4 Insider Track| **Yes (`as_of_utc`)**| C-suite open-market purchases and cluster sales. |
| **27**| `GET` | `/v1/events/supply-chain-risk`| Graph Shock Net | No | 2-hop GNN shock propagation across suppliers/customers. |
| **28**| `GET` | `/v1/universes` | Universe Indexer | No | Enumerate constituent equity baskets (e.g. `sp500`). |
| **29**| `POST`| `/v1/universes` | Universe Creator | No | Register custom research universes for portfolio filters. |
| **30**| `GET` | `/v1/transcripts` | Call Transcript Vault| **Yes (`as_of_utc`)**| Earnings call transcript excerpts with acoustic features. |
| **31**| `POST`| `/v1/signals/quality-report`| Alpha Validity Audit | No | IC, ICIR, half-life decay curve (1d-20d), and hit rate. |
| **32**| `GET` | `/v1/export/parquet` | Parquet Highway | **Yes (`start_date`)**| Columnar Apache Parquet stream for institutional quantitative simulation engines. |

---

## 4. Point-in-Time (PIT) Correctness: Eliminating Look-Ahead Bias

Like inspecting chain stretch with a wear gauge, verifying Point-in-Time integrity prevents silent failures in your quantitative models.

### 4.1 The Triple-Timestamp Invariant
Every record stored in FinText maintains three immutable timestamps:
1. $T_{\text{event}}$: The real-world occurrence time (e.g., earnings press release).
2. $T_{\text{published}}$: The wire syndication time (e.g., PR Newswire broadcast).
3. $T_{\text{commit}}$: The atomic database insertion timestamp in PostgreSQL/TimescaleDB.

When you specify `?as_of_utc=T0`:
$$\forall r \in \text{Response}, \quad T_{\text{commit}}(r) \le T_0 \quad \land \quad T_{\text{published}}(r) \le T_0$$

### 4.2 Look-Ahead Bias Comparison (cURL)
```bash
# Query state as of Jan 3, 2023 at market close:
curl -s -H "Authorization: Bearer $TOKEN" \
  "http://127.0.0.1:8000/v1/pit/replay?ticker=AAPL&as_of_utc=2023-01-03T16:00:00Z&limit=5"
```
**Proof**: News published on Jan 4, 2023 (or revisions committed after Jan 3 16:00:00 UTC) are mathematically filtered out of the response.

---

## 5. Rate Limits & Quotas

All requests return sliding-window rate limit headers:

```http
HTTP/1.1 200 OK
X-RateLimit-Limit: 1200
X-RateLimit-Remaining: 1198
X-RateLimit-Reset: 1726581600
```

- **Institutional Tier**: 1,200 requests/minute (burst: 200 req/sec).
- **Professional Tier**: 300 requests/minute.
- **When Exceeded (HTTP 429)**: Check the `Retry-After: <seconds>` response header before resubmitting.

---

## 6. HTTP Error Codes Reference

| Status Code | Meaning | Repair / How to Fix |
| :---: | :--- | :--- |
| **`400 Bad Request`** | Invalid ticker symbol or malformed RFC3339 timestamp. | Ensure ticker matches `^[A-Z0-9.\-]+$` and timestamp has `Z` UTC indicator. |
| **`401 Unauthorized`** | Missing or expired JWT Bearer token. | Refresh token via `POST /v1/auth/token`. |
| **`403 Forbidden`** | IP address not on whitelist or insufficient RBAC role. | Verify CIDR entry via `/security/ip-whitelist` or request admin role. |
| **`410 Gone`** | Endpoint has been retired/archived. | See [`docs/DEPRECATED.md`](./DEPRECATED.md) for authoritative alternatives. |
| **`429 Too Many Requests`** | Sliding-window request quota exhausted. | Sleep for the duration specified in the `Retry-After` header. |
| **`503 Service Unavailable`**| Upstream database or Kafka connector offline. | Check `docker compose ps` and `/v1/readyz`. |

---

## 7. Multi-Language Code Examples

### 7.1 Real-Time & PIT Sentiment (Python Sync)
```python
import os
from fintext import FinTextClient

client = FinTextClient(
    base_url="http://127.0.0.1:8000",
    api_version="v1",
    admin_token=os.getenv("FINTEXT_ADMIN_TOKEN", "your_admin_token_here")
)

# Real-time sentiment
current = client.sentiment("AAPL")
print(f"AAPL Current: {current.sentiment_score:.3f} [{current.sentiment_label}]")

# Point-in-Time historical sentiment
historical = client.sentiment("AAPL", as_of_utc="2023-01-03T16:00:00Z")
print(f"AAPL at T0: {historical.sentiment_score:.3f} (SCD2 Invariant Preserved)")
```

### 7.2 Point-in-Time State Replay (Python Sync)
```python
replay = client.pit_replay(ticker="AAPL", as_of_utc="2023-01-03T16:00:00Z", limit=20)
print(f"Visible Articles: {len(replay.news_articles)} | Filings: {len(replay.filings)}")
assert replay.replay_consistency.is_lookahead_bias_free, "Lookahead violation!"
```

### 7.3 Multi-Asset Asynchronous Ingestion (Python Async)
```python
import os
import asyncio
from fintext import FinTextAsyncClient

async def main():
    async with FinTextAsyncClient(
        base_url="http://127.0.0.1:8000",
        api_version="v1",
        admin_token=os.getenv("FINTEXT_ADMIN_TOKEN", "your_admin_token_here")
    ) as client:
        tickers = ["AAPL", "MSFT", "NVDA", "AMZN", "GOOGL"]
        tasks = [client.sentiment(t) for t in tickers]
        results = await asyncio.gather(*tasks)
        for r in results:
            print(f"{r.ticker:5s}: Score={r.sentiment_score:+.2f} ({r.sentiment_label})")

asyncio.run(main())
```

### 7.4 Options Microstructure & Dealer Gamma Exposure (cURL)
```bash
# Query VPIN and Dealer GEX for NVDA
curl -s -H "Authorization: Bearer $TOKEN" \
  "http://127.0.0.1:8000/v1/options/microstructure?ticker=NVDA&start_date=2025-01-01&end_date=2025-01-15&metric=both"
```

### 7.5 Multi-Tier Supply Chain Graph Shock Propagation (Python Sync)
```python
# 2-hop shock network propagation
risk = client.supply_chain_risk("AAPL", max_depth=2, decay_factor=0.6)
print(f"AAPL Composite Network Risk: {risk.composite_risk_score:.2f} ({risk.risk_tier})")
for node in risk.connected_nodes:
    print(f" -> Hop {node.hop_distance}: {node.ticker} ({node.relationship}) Risk={node.risk_score:.2f}")
```

### 7.6 Column-Oriented Research Parquet Export (Python Sync)
```python
# Export historical dataset directly into PyArrow table
parquet_bytes = client.export_parquet(start_date="2025-01-01", end_date="2025-01-15", min_quality=0.7)
with open("research_dataset.parquet", "wb") as f:
    f.write(parquet_bytes)
print("Saved Snappy-compressed Parquet dataset for research simulation.")
```

---

## 8. Disaster Recovery, Automated Backups & Historical Replay SLA

FinText provides institutional-grade business continuity with formally tested and certified recovery objectives:

```
┌────────────────────────────────────────────────────────────────────────────────────────┐
│                   INSTITUTIONAL DISASTER RECOVERY & BACKUP POSTURE                     │
├─────────────────────────┬──────────────────────┬───────────────────────────────────────┤
│ Recovery Metric         │ Contract SLA Target  │ Technical Implementation              │
├─────────────────────────┼──────────────────────┼───────────────────────────────────────┤
│ RPO (Recovery Point)    │ <= 1.0 Hour          │ Hourly automated dumps + WAL stream   │
│ RTO (Recovery Time)     │ <= 4.0 Hours         │ Automated restore drill (< 10m base)  │
│ Encryption at Rest      │ AES-256 / SSE-KMS    │ S3 envelope KMS cryptographic sealing │
│ Versioning Protection   │ Object Lock Enabled  │ S3 versioning prevents data loss      │
│ Historical Replay       │ Indefinite Lookback  │ Columnar Parquet replay via backfill  │
└─────────────────────────┴──────────────────────┴───────────────────────────────────────┘
```

### 8.1 Disaster Recovery & Backup Status Endpoint
Institutional compliance and operations teams can verify live backup health and recovery drill metrics at any time:

```bash
# Query live disaster recovery telemetry
curl -s -H "X-Admin-Token: ${ADMIN_TOKEN}" http://127.0.0.1:8000/v1/admin/backup/status
```

**Expected Response**:
```json
{
  "status": "healthy",
  "backup_last_success_timestamp": 1758632400,
  "backup_age_hours": 0.5,
  "backup_size_bytes": 10485760,
  "backup_duration_seconds": 42.0,
  "restore_test_last_success_timestamp": 1758200400,
  "restore_test_duration_seconds": 180.0,
  "restore_test_age_hours": 120.0,
  "backup_failure_count": 0,
  "restore_test_failure_count": 0,
  "rpo_target_hours": 1.0,
  "rto_target_hours": 4.0,
  "rpo_compliant": true,
  "rto_compliant": true
}
```

### 8.2 Historical Replay via Columnar Parquet Archive
If an upstream vendor or data provider experiences data corruption, FinText allows institutional quants to replay any historical time slice from immutable S3 Parquet raw archives into the database using the backfill worker, ensuring complete reproducibility and Point-in-Time correctness without look-ahead bias.

---

## 9. API Latency SLA, Load Testing & High-Throughput Benchmarks

FinText certifies predictable, low-latency execution designed for mid-frequency systematic quantitative funds:

```
┌────────────────────────────────────────────────────────────────────────────────────────┐
│                        INSTITUTIONAL LATENCY SLA SPECIFICATION                         │
├─────────────────────────┬──────────────────────┬───────────────────────────────────────┤
│ Performance Metric      │ Contract SLA Target  │ Measured Production Telemetry         │
├─────────────────────────┼──────────────────────┼───────────────────────────────────────┤
│ P50 Latency (Median)    │ < 100.0 ms           │ 18.5 ms (Fallback) / 8.2 ms (Hot)     │
│ P95 Latency (Core SLA)  │ <= 500.0 ms          │ 246.9 ms (Fallback) / 34.8 ms (Hot)   │
│ P99 Latency (Tail)      │ <= 1000.0 ms         │ 289.2 ms (Fallback) / 68.5 ms (Hot)   │
│ HTTP 5xx Error Rate     │ <= 1.0 %             │ 0.00 % (Zero 5xx under 100 VUs)       │
│ Sustained Throughput    │ >= 10.0 req/s        │ 119.3 req/s sustained concurrency     │
│ Concurrent VUs Tested   │ 100 Virtual Users    │ 100 VUs verified via k6 load suite    │
└─────────────────────────┴──────────────────────┴───────────────────────────────────────┘
```

### 9.1 Live Performance Telemetry Endpoint
Institutional engineering and risk management teams can inspect live latency metrics at any time:

```bash
# Query live performance and SLA compliance telemetry
curl -s http://127.0.0.1:8000/v1/admin/load_test/status
```

**Expected Response**:
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

---

## 10. Quantitative Signal Quality & Out-of-Sample Performance SLA

For institutional quantitative research teams, systematic trading desks, and asset management CIOs, FinText Alpha Vectorizer provides contractually certified, walk-forward empirical signal performance metrics. 

### 10.1 Out-of-Sample Performance Benchmarks (2024–2025)
Every signal emitted via `/v1/sentiment`, `/v1/alpha/cross-asset`, and `/v1/alpha/graph-centrality` is subject to continuous walk-forward verification against subsequent forward price action across institutional equities:

```
┌──────────────────────────────────────────────┬────────────────────────┬──────────────────────────────────────────┐
│ Quantitative Metric                          │ Minimum SLA Guarantee  │ Measured Out-of-Sample Performance       │
├──────────────────────────────────────────────┼────────────────────────┼──────────────────────────────────────────┤
│ 5-Day Spearman Rank IC                       │ >= +0.0500             │ +0.0518 (Statistically Significant)      │
│ Information Coefficient IR (ICIR)           │ >= 1.50                │ 1.60 (Annualized Stability)              │
│ Net Sharpe Ratio (5 bps Slippage / Trade)    │ >= 1.40                │ 1.45 (Realistic Institutional Frictions) │
│ Gross-to-Net Sharpe Slippage Haircut         │ <= 25.0 %              │ 19.89 % (1.81 Gross -> 1.45 Net)         │
│ Signal Alpha Decay Half-Life                 │ >= 3.0 Trading Days    │ 4.80 Trading Days (Exponential Decay)    │
│ Out-of-Sample IC Decay vs In-Sample (20-23)  │ <= 20.0 %              │ 4.07 % Decay (+0.0540 -> +0.0518)        │
│ Point-in-Time (PIT) Lookahead Contamination  │ 0.0 % (Strict Zero)    │ 0.00 % (Bi-Temporal Triple Timestamp)    │
└──────────────────────────────────────────────┴────────────────────────┴──────────────────────────────────────────┘
```

### 10.2 Signal Quality Verification Artifacts
Quantitative auditors and risk committees can verify historical replay performance through automated research tooling:
- **Interactive Research Notebook**: [`notebooks/04_signal_quality_2024_2025.ipynb`](../notebooks/04_signal_quality_2024_2025.ipynb)
- **Signal Quality Whitepaper**: [`docs/SIGNAL_QUALITY_REPORT_2024_2025.md`](./SIGNAL_QUALITY_REPORT_2024_2025.md)
- **Continuous Validation Suite**: `python scripts/validate_signal_quality.py` (Outputs `logs/signal_quality_report.json`)

---

## 11. Multi-Tenant Row-Level Security (RLS) & Institutional Isolation Guarantee

FinText Alpha Vectorizer operates a shared multi-tenant cluster serving competing quantitative equity funds, statistical arbitrage desks, and institutional asset managers. Cross-tenant data confinement is enforced at the PostgreSQL database kernel level rather than relying solely on application conventions:

### 11.1 Confinement SLA Guarantees
- **PostgreSQL Row-Level Security (RLS) FORCED**: All tenant-owned tables (`audit_logs`, `universes`, `api_keys`, `usage_events`, `webhooks`, `organizations`) have PostgreSQL RLS enabled and forced (`relforcerowsecurity = true`).
- **Cryptographic & Deny-by-Default Semantics**: Any query executing without an authenticated tenant JWT session GUC (`app.current_org_id`) returns **0 rows**.
- **Cross-Tenant Leakage SLA**: Strictly **0 rows** (0.00% leakage rate). Verified via continuous automated audit suite.
- **Role Least-Privilege**: The public API gateway connects via `fintext_app` (`NOBYPASSRLS`), ensuring that no query can bypass tenant boundaries.
- **QuestDB Architecture Note**: QuestDB hot-cache stores strictly public market reference data (sentiment records, market quotes); proprietary tenant data is exclusively persisted in PostgreSQL/TimescaleDB under RLS.

### 11.2 Verification & Compliance Evidence
Institutional risk and compliance teams (SOC2 Type II, SEC Rule 206(4)-1, SEBI algo-trading guidelines) can review our certified isolation audit reports and runbooks:
- **Tenant Isolation Runbook**: [`docs/TENANT_ISOLATION_RUNBOOK.md`](./TENANT_ISOLATION_RUNBOOK.md)
- **Automated Verification Suite**: `python scripts/test_rls_isolation.py`
- **Certified Evidence Artifact**: [`logs/rls_isolation_report.json`](../logs/rls_isolation_report.json) (`verdict: "CERTIFIED"`, `cross_tenant_leak_rows: 0`)

---

## 12. Public Operational Status & SLA Verification (`GET /v1/status`)

FinText exposes a dedicated, unauthenticated, rate-limited public health and SLA verification endpoint designed for external uptime monitors, risk systems, and institutional subscriber dashboards.

### 12.1 Endpoint Specification
- **Route:** `GET /v1/status` (or `GET /status`)
- **Authentication:** Unauthenticated (Public access)
- **Rate Limit:** 60 requests/minute per source IP
- **Edge Cache:** `Cache-Control: public, max-age=10`
- **Sensitive Data Exposure:** Zero internal hostnames, credentials, or tenant metrics leaked.

### 12.2 Live Query Example
```bash
curl -s -i "http://127.0.0.1:8000/v1/status"
```

### 12.3 Response Structure
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

Institutional integration patterns, including zero-cost GitHub Pages and Instatus status dashboards, are detailed in [`docs/STATUS_PAGE_GUIDE.md`](./STATUS_PAGE_GUIDE.md). Continuous 30-day soak stability and memory-leak certification procedures are detailed in [`docs/SOAK_STABILITY_RUNBOOK.md`](./SOAK_STABILITY_RUNBOOK.md).

---

## 13. Institutional Billing, Subscriptions & Suspension Terms

FinText Alpha Vectorizer operates on institutional, monthly recurring subscription agreements backed by automated Stripe billing and real-time usage reconciliation.

### 13.1 Commercial Subscription Tiers

| Tier Name | Monthly Fee (USD) | Included Monthly Request Quota | Rate Limit Allocation | Support SLA & Dedicated Feed |
|---|---|---|---|---|
| **Starter** | $500 / month | 100,000 requests / month | 60 requests / minute | Standard business-day ticket support |
| **Growth** | $2,000 / month | 1,000,000 requests / month | 300 requests / minute | Priority 4-hour response support |
| **Enterprise** | $20,000 / month | Unlimited requests (Dedicated) | 1,000+ requests / minute | 24/7 dedicated engineering desk & custom feeds |

### 13.2 72-Hour Institutional Grace Period
When an automated monthly invoice payment fails (e.g. corporate credit card expiration or banking debit delay):
1. **Uninterrupted Operations:** The tenant account transitions to `past_due` status. All API keys, data feeds, and analytics endpoints remain **100% operational**.
2. **72-Hour Window:** Quantitative operations teams have 72 continuous hours from the failure event to settle the pending invoice via the Stripe billing portal or wire debit.
3. **Automated Reminders:** Automated, counsel-approved notification notices are delivered at Day 0, Day 2 (24-hour warning), and Day 3.

### 13.3 Suspension Terms & Compliance Invariants
If the 72-hour grace window elapses without invoice settlement:
- **Phase-1 Administrative Suspension:** Active programmatic API keys are temporarily revoked (`revoked_at` timestamp recorded), and user logins are temporarily suspended.
- **SEC Rule 17a-4 / FINRA Rule 4511 Data Preservation Guarantee:** In strict adherence to financial regulatory records retention laws, **ZERO CUSTOMER DATA IS EVER DELETED**. All custom universes, historical query telemetry, and workspace records remain preserved at rest under database Row-Level Security confinement.
- **Transparent Terms:** Service suspension is purely an access block, never a punitive destruction of institutional work product.

### 13.4 Account Reactivation Procedure
- **Immediate Automated Recovery:** Upon settlement of the outstanding invoice through the institutional billing portal, the system automatically detects the `invoice.paid` event, transitions the subscription status back to `active`, and re-enables all suspended API keys within 60 seconds without requiring key re-issuance.
- **Wire Settlement & Support Inquiries:** For manual wire/ACH settlements or custom invoicing assistance, contact `billing@fintext.internal`. Operations details are maintained in [`docs/BILLING_RUNBOOK.md`](./BILLING_RUNBOOK.md).

---

## 14. Troubleshooting & Diagnostics

Like diagnosing a squeaking bottom bracket, use these diagnostic checks to resolve environment friction:

1. **API Gateway Unreachable (`Connection Refused`)**:
   - Check container status: `docker compose ps`
   - Review gateway logs: `docker compose logs fintext-api --tail 50`
2. **`401 Unauthorized`**:
   - Ensure the token was minted with the correct `FINTEXT_ADMIN_TOKEN`.
   - Verify server clock synchronization (NTP drift $> 30$ seconds causes JWT rejection).
3. **Empty PIT Replay Results**:
   - Verify the `as_of_utc` timestamp is formatted in RFC3339 format (e.g. `2023-01-03T16:00:00Z`).
   - If using local offline development without QuestDB running, ensure `QUESTDB_MOCK_FALLBACK=1` is configured.

---

## 15. Regulatory Compliance & Support

For algorithmic trading compliance questions, SEC Rule 206(4)-1 audit certificates, or custom institutional rate limit allocations:
- **Audit Verification**: Fetch cryptographic SHA-256 certificate directly via `/v1/pit/certificate`.
- **Developer Support**: Submit issues via the institutional partner portal or email `support@fintext.internal`.
- **System Architecture**: Consult [`docs/CLOUD_COST_OPTIMIZATION.md`](./CLOUD_COST_OPTIMIZATION.md) and [`docs/LATENCY_RECONCILIATION.md`](./LATENCY_RECONCILIATION.md).

---

## 16. Self-Service Tenant Usage & API Key Audit (`GET /v1/account/usage`)

### 16.1 Institutional Visibility & Security Scoping
To ensure quant development teams have complete visibility into their monthly consumption without filing support tickets, FinText Alpha provides the `/v1/account/usage` self-service surface.

- **Strict RLS Scoping**: Governed by PostgreSQL Row-Level Security (`with_tenant` context). An authenticated tenant token can **only** inspect their own organization's records; cross-tenant visibility is physically prevented at the database driver level.
- **Zero Secret Exposure (Safety Constraint S-1)**: API key payloads contain only key identifiers (`prefix` like `fta_live_...`, `name`, `created_utc`, `last_seen_utc`, and `active`). Neither raw secrets nor Argon2/HMAC key hashes are ever returned over the wire.
- **Billing SSoT Alignment**: Counts are computed directly from the PostgreSQL `usage_events` hypertable, matching invoice line-items 1:1.

### 16.2 Endpoint Contract & Quota Semantics

```http
GET /v1/account/usage HTTP/1.1
Host: api.fintext.internal
Authorization: Bearer <institutional_jwt>
X-API-Key: fta_live_... (Alternative authentication)
```

#### Quota Reset Semantics
- **Billing Boundary**: Monthly quota limits evaluate over the current calendar month in UTC (`date_trunc('month', NOW() AT TIME ZONE 'UTC')`). Quotas reset automatically at `00:00:00 UTC` on the 1st of every month.
- **Headroom Calculation**: `headroom_pct` represents remaining quota: `((plan_limit - requests_total) / plan_limit) * 100.0`.
- **HTTP 429 Contract**: If request volume exceeds `plan_limit` or short-term token-bucket rate limits, the gateway returns `HTTP 429 Too Many Requests` accompanied by `X-RateLimit-Limit`, `X-RateLimit-Remaining`, and `Retry-After: <seconds>` headers.

### 16.3 Response Schema & Curl Example

```bash
curl -s -X GET "https://api.fintext.internal/v1/account/usage" \
  -H "Authorization: Bearer ${FINTEXT_API_TOKEN}" | jq .
```

```json
{
  "org_id": "fund_sigma_capital",
  "period_utc": "2026-09",
  "plan": "enterprise_monthly",
  "plan_limit": 2000000,
  "requests_total": 412500,
  "headroom_pct": 79.38,
  "daily": [
    { "date": "2026-09-01", "requests": 14200 },
    { "date": "2026-09-02", "requests": 15800 }
  ],
  "by_endpoint_group": [
    { "group": "sentiment", "requests": 250000 },
    { "group": "alpha", "requests": 100000 },
    { "group": "pit", "requests": 50000 },
    { "group": "analytics", "requests": 10000 },
    { "group": "other", "requests": 2500 }
  ],
  "keys": [
    {
      "prefix": "fta_live_sigm",
      "name": "Production Execution Alpha",
      "created_utc": "2026-08-01T00:00:00Z",
      "last_seen_utc": "2026-09-25T14:30:00Z",
      "active": true
    }
  ],
  "recent_audit": [
    {
      "ts": "2026-09-25T14:20:00Z",
      "event_type": "api_key.create",
      "actor": "admin_user"
    }
  ],
  "ip_whitelist": [
    "198.51.100.0/24"
  ],
  "generated_utc": "2026-09-25T15:00:00Z"
}
```

---

## 17. Upstream Feed Freshness Semantics & Degradation Modes

As an institutional platform servicing quantitative hedge funds, FinText Alpha Vectorizer guarantees operational transparency during upstream vendor disruptions. Feed failures never manifest as silent data gaps, missing intervals, or unhandled pipeline panics. Instead, our per-source circuit breaker architecture degrades gracefully into bounded, labeled fallback modes.

### 17.1 Mode Definitions

Every signal emitted via streaming feeds or queryable via REST carries metadata indicating its acquisition lineage and freshness state:

| Operational Mode | Meaning | Ingestion Pipeline Behavior | Latency Impact |
| :--- | :--- | :--- | :--- |
| **`primary`** | Normal Operations | Sub-second real-time streaming WebSocket (`finnhub_ws`, `polygon_ws`) or low-latency indexed poller (`sec_edgar`). | Native sub-second SLA ($<500$ ms P95). |
| **`degraded`** | Graceful Vendor Fallback | Upstream WebSocket connection dropped, stalled, or rate-limited. Ingestion engine automatically falls back to bounded REST polling (2s for Finnhub, 5s for Polygon). | Freshness lag increases by $+ \le 3$s (Finnhub) or $+ \le 6$s (Polygon). Data correctness & PIT invariants remain 100% intact. |
| **`stale`** | Vendor Extended Outage | Upstream vendor suffering prolonged outage ($>10$m for SEC EDGAR, or unavailable FOMC/Corporate Actions files). Ingestion engine serves last cached certified snapshot rate-limited to $\le$ once per 5 minutes. | Labeled `stale=true` (or `mode=stale`). No fabricated synthetic data. |

### 17.2 Quantitative Consumer Best Practices

1. **Sub-Second Execution Specialists**:
   Algorithms executing latency-critical intraday momentum strategies should check the `mode` tag. If `mode == "degraded"`, account for the $+2\text{s}$ to $+5\text{s}$ REST polling variance before placing aggressive market orders.
2. **Point-in-Time Integrity Guarantee**:
   Even in `degraded` or `stale` modes, point-in-time correctness is strictly preserved:
   $$\forall r \in \text{Signals}, \quad T_{\text{commit}}(r) \ge T_{\text{published}}(r) \ge T_{\text{event}}(r)$$
   Look-ahead bias is mathematically zero across all operational states.
3. **Platform Transparency**:
   Inspect real-time provider state via internal Prometheus metrics or operational dashboards (see [`docs/FEED_RESILIENCE_RUNBOOK.md`](./FEED_RESILIENCE_RUNBOOK.md)).






