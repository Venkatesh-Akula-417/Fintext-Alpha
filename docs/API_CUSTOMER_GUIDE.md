# FinText Alpha Vectorizer — Customer API Guide (Bike Manual Style)
### 15 Minutes to First Signal | Institutional Quantitative Gateway Documentation

> **Document Version**: v1.0.0 (Production Core Release)  
> **Last Verified**: 2026-09-17 (Suite #274) | **Audit Readiness**: Certified Clean  
> **Interactive Research Suite**: [`notebooks/`](../notebooks/README.md) | **Model Card**: [`docs/SIGNAL_QUALITY_REPORT.md`](./SIGNAL_QUALITY_REPORT.md)  
> **Authoritative Deprecations**: [`docs/DEPRECATED.md`](./DEPRECATED.md)

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

### 2.2 Rotating API Keys (`/v1/users/api-keys`)
Generate rotatable programmatic keys prefixed with `fintext_live_...`:
```bash
curl -X POST http://127.0.0.1:8000/v1/users/api-keys \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"name": "production_hft_feed", "expires_in_days": 90}'
```

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

## 8. Troubleshooting & Diagnostics

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

## 9. Regulatory Compliance & Support

For algorithmic trading compliance questions, SEC Rule 206(4)-1 audit certificates, or custom institutional rate limit allocations:
- **Audit Verification**: Fetch cryptographic SHA-256 certificate directly via `/v1/pit/certificate`.
- **Developer Support**: Submit issues via the institutional partner portal or email `support@fintext.internal`.
- **System Architecture**: Consult [`docs/CLOUD_COST_OPTIMIZATION.md`](./CLOUD_COST_OPTIMIZATION.md) and [`docs/LATENCY_RECONCILIATION.md`](./LATENCY_RECONCILIATION.md).
