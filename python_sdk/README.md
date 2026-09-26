# FinText Alpha Vectorizer — Python Client SDK

Official synchronous and asynchronous Python Client SDK for the **FinText Alpha Vectorizer** High-Frequency Financial Sentiment Analysis & Quantitative Alpha Engine.

[![Python Version](https://img.shields.io/badge/python-3.10%2B-blue.svg)](https://python.org)
[![Fast & Async](https://img.shields.io/badge/runtime-asyncio%20%7C%20httpx-green.svg)](https://www.python-httpx.org/)
[![License](https://img.shields.io/badge/license-Proprietary-red.svg)]()

> **Last Verified**: 2026-09-10 (Suite #96) | **Audit Readiness**: Certified Clean | **Authoritative Deprecations**: [`docs/DEPRECATED.md`](../docs/DEPRECATED.md)

---

## Features

- ⚡ **Synchronous & Asynchronous Clients**: First-class support for both `FinTextClient` (sync) and `FinTextAsyncClient` (asyncio).
- 🛡️ **Automated JWT Lifecycle**: Automatically acquires and injects signed JWT bearer tokens using `admin_token` credentials.
- ⏱️ **Rate Limit Awareness**: Tracks and exposes `X-RateLimit-*` and `Retry-After` headers on every call.
- 📈 **Options Implied Volatility & Greeks**: Query Black-Scholes IV and analytical Greeks (Delta, Gamma, Theta, Vega, Rho) across contracts.
- ⏳ **Point-in-Time Historical State Replay**: Reconstruct historical state with strict survivorship and look-ahead bias prevention.
- 🕸️ **Multi-Tier Supply Chain Risk**: Evaluate upstream and downstream corporate network shocks and graph dependencies.
- 📡 **WebSocket Stream URL Generator**: Effortlessly construct authenticated streaming URLs (`ws://` and `wss://`).
- 🔒 **Type-Safe Pydantic v2 Models**: Strict typed dataclasses for all request parameters and response bodies.

---

## Installation

```bash
# Install from source in editable development mode:
pip install -e python_sdk/

# Or via wheel:
pip install fintext
```

---

## Quickstart

### 1. Synchronous Client (`FinTextClient`)

```python
from fintext import FinTextClient

# Initialize client with versioned /v1 API gateway
client = FinTextClient(
    base_url="http://127.0.0.1:8000",
    api_version="v1",
    admin_token="suite180-admin-token"
)

# 1. System Health Probe
health = client.health()
print(f"Server Status: {health.status} (v{health.version})")

# 2. Point-in-Time Asset Sentiment (Auto-authenticates on first request)
sentiment = client.sentiment("AAPL")
print(f"AAPL Sentiment: {sentiment.sentiment_score:.3f} [{sentiment.sentiment_label}]")

# 3. Options Implied Volatility & Greeks
options = client.options_iv("AAPL", expiration_date="2026-03-20")
print(f"AAPL IV: {options.implied_volatility:.2%} | Delta: {options.delta:.3f}")

# 4. Point-in-Time Historical State Replay
replay = client.pit_replay("AAPL", as_of_utc="2025-01-15T09:30:00Z")
print(f"PIT Replay Articles: {len(replay.news)} | Filings: {len(replay.filings)}")

# 5. Multi-Tier Supply Chain Risk Propagation
supply_chain = client.supply_chain_risk("AAPL", max_depth=2)
print(f"Supply Chain Risk: {supply_chain.composite_risk_score:.2f} ({supply_chain.risk_tier})")

# 6. Rate Limit Status
print(f"Remaining Requests: {client.last_rate_limit.remaining}/{client.last_rate_limit.limit}")

# 7. WebSocket Stream URL
print(f"Real-time Stream URL: {client.ws_url(ticker='AAPL')}")

# 8. Tenant Usage & Quota Audit (Self-Service)
usage = client.usage()
print(f"Quota Used: {usage['requests_total']}/{usage['plan_limit']} ({usage['headroom_pct']}% headroom)")
```

---

### 2. Asynchronous Client (`FinTextAsyncClient`)

```python
import asyncio
from fintext import FinTextAsyncClient

async def main():
    async with FinTextAsyncClient(
        base_url="http://127.0.0.1:8000",
        api_version="v1",
        admin_token="suite180-admin-token"
    ) as client:
        # Fetch sentiment for multiple assets concurrently
        tickers = ["AAPL", "NVDA", "MSFT", "AMZN"]
        tasks = [client.sentiment(t) for t in tickers]
        results = await asyncio.gather(*tasks)

        for res in results:
            print(f"{res.ticker}: Score={res.sentiment_score:.2f} ({res.sentiment_label})")

asyncio.run(main())
```

---

## Environment Variables

| Variable | Description | Default |
| :--- | :--- | :--- |
| `FINTEXT_BASE_URL` | Base URL of the FinText Axum Gateway | `http://127.0.0.1:8000` |
| `FINTEXT_API_TOKEN` | Bearer JWT Token string | `None` |
| `FINTEXT_ADMIN_TOKEN` | Administrative token for JWT issuance | `None` |

---

## Error Handling

The SDK provides specialized exception classes inheriting from `FinTextError`:

```python
from fintext import FinTextClient, FinTextAuthError, FinTextRateLimitError, FinTextAPIError

client = FinTextClient(base_url="http://127.0.0.1:8000")

try:
    sentiment = client.sentiment("AAPL")
except FinTextRateLimitError as e:
    print(f"Quota exceeded! Wait {e.retry_after} seconds before retrying.")
except FinTextAuthError as e:
    print(f"Authentication failed: {e.message}")
except FinTextAPIError as e:
    print(f"API Error [{e.status_code}]: {e.message}")
```

---

## Running Tests

```bash
pytest python_sdk/tests/ -v
```
