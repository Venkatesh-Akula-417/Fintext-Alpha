# FinText Alpha Vectorizer — API Error Code Reference

> Comprehensive troubleshooting manual for developers, quantitative integrators, and API consumers.

---

## 1. Overview & Standard Error Format

All FinText Alpha Vectorizer REST endpoints return standard HTTP status codes accompanied by a structured JSON payload conforming to the `application/json` Content-Type:

```json
{
  "error": "Bad Request",
  "message": "Invalid date range: start_date (2025-03-31) cannot be after end_date (2025-01-01)"
}
```

### Standard Error Fields

| Field Name | Type | Description |
| :--- | :---: | :--- |
| `error` | `string` | Canonical HTTP error title or category (e.g., `"Unauthorized"`, `"Forbidden"`, `"Too Many Requests"`). |
| `message` | `string` | Human-readable explanation with specific diagnostic details and corrective guidance. |

### Diagnostic Response Headers

When an error occurs, the server includes contextual response headers to assist automated retry and debugging logic:

```http
HTTP/1.1 429 Too Many Requests
Content-Type: application/json; charset=utf-8
x-ratelimit-limit: 100
x-ratelimit-remaining: 0
x-ratelimit-reset: 42
retry-after: 42
```

- `x-ratelimit-limit`: Maximum allowed requests within the sliding window.
- `x-ratelimit-remaining`: Number of remaining requests permitted in the current window.
- `x-ratelimit-reset`: Number of seconds until the current sliding window resets.
- `retry-after`: Recommended backoff wait time in seconds before re-attempting the request.
- `X-FinText-Sandbox`: Present with value `"true"` when executing in simulated sandbox mode.

---

## 2. Status Code Quick Reference Matrix

| Status Code | Canonical Name | Common Trigger | Typical Solution |
| :---: | :--- | :--- | :--- |
| **`400`** | **Bad Request** | Invalid query params, inverted date range, malformed syntax | Validate parameter ranges, ISO dates (`YYYY-MM-DD`), and JSON structure |
| **`401`** | **Unauthorized** | Missing token, expired JWT, invalid API key | Authenticate via `POST /auth/token` or provide valid `X-API-Key` |
| **`403`** | **Forbidden** | Client IP not whitelisted, insufficient role permissions | Add client IP to whitelist or request elevated RBAC credentials |
| **`404`** | **Not Found** | Unknown ticker, non-existent organization, missing ID | Verify entity identifier or check universe coverage |
| **`409`** | **Conflict** | Duplicate registration email, duplicate universe name | Use a distinct email/resource name or update the existing entity |
| **`410`** | **Gone** | Accessing retired endpoints (`/auth/register`, `/news/articles/:id`, `/audio/transcribe`) | Consult migration guide; endpoints removed in v1.0 |
| **`422`** | **Unprocessable Entity**| Missing required JSON field, mismatched data type | Review OpenAPI schema and ensure strict type conformance |
| **`429`** | **Too Many Requests** | Per-minute burst limit or monthly plan quota exceeded | Implement exponential backoff or upgrade institutional tier |
| **`500`** | **Internal Server Error**| Database socket error, unexpected execution failure | Check service status, verify mock fallback flags, or retry |
| **`503`** | **Service Unavailable**| Required data source unavailable when `PRODUCTION_MODE=true` | Restore real database/streaming connection; synthetic fallbacks are prohibited in production |

---

## 3. Detailed Error Scenarios & Remediation

---

### `400 Bad Request`

Returned when the server cannot process the request due to malformed query parameters, out-of-bounds numerical values, or logical validation conflicts.

#### Example Response: Inverted Date Range
```json
{
  "error": "Bad Request",
  "message": "Invalid date range: start_date (2025-03-31) cannot be after end_date (2025-01-01)"
}
```

#### Example Response: Missing Required Query Parameter
```json
{
  "error": "Bad Request",
  "message": "Query parameter 'ticker' is required and must not be empty"
}
```

#### Example Response: Malformed SCD2 Point-in-Time Timestamp
```json
{
  "error": "Invalid as_of_utc timestamp 'invalid-date', expected RFC3339/ISO-8601 format",
  "status": "bad_request"
}
```

#### Example Response: SCD2 Sentiment Score Out of Bounds
```json
{
  "error": "Bad Request",
  "message": "sentiment_score must be between -1.0 and 1.0, got 2.5"
}
```

#### Common Causes
1. **Invalid Date Formatting**: Passing `MM/DD/YYYY` instead of ISO-8601 standard `YYYY-MM-DD`.
2. **Malformed SCD2 `as_of_utc`**: Providing non-RFC3339 formatted timestamps (e.g. `2026/08/25` instead of `2026-08-25T14:30:00Z`).
3. **Inverted Date Boundaries**: `start_date` occurs chronologically after `end_date`.
4. **Out-of-Bounds Query Values**: Setting `limit=5000` when the maximum allowed bound is `1000` or `sentiment_score` outside `[-1.0, 1.0]`.
5. **Invalid Ticker Symbol**: Empty string or symbols exceeding 6 characters.

#### Client Fix
- Validate that all dates match `^\d{4}-\d{2}-\d{2}$` and `as_of_utc` satisfies standard RFC3339 (e.g. `2025-06-15T14:30:00Z`).
- Enforce `start_date <= end_date` before initiating the HTTP call.
- Clamp `limit` parameters between `1` and `1000`, and `sentiment_score` between `-1.0` and `1.0`.

---

### `401 Unauthorized`

Returned when authentication credentials (JWT Bearer token or API Key) are missing, malformed, expired, or have an invalid signature.

#### Example Response: Expired JWT Token
```json
{
  "error": "Unauthorized",
  "message": "JWT token has expired"
}
```

#### Example Response: Invalid API Key
```json
{
  "error": "Unauthorized",
  "message": "Invalid or revoked API Key"
}
```

#### Common Causes
1. **Missing Authorization Header**: Initiating a request without `Authorization: Bearer <token>` or `X-API-Key: <key>`.
2. **Expired Token Signature**: The token's `exp` timestamp has elapsed (default JWT lifetime is 1 hour).
3. **Secret Key Mismatch**: The token was signed with a different `JWT_SECRET` than configured on the server.
4. **Revoked API Key**: The API key was rotated or explicitly deactivated via `POST /users/keys/rotate`.

#### Client Fix
- Request a fresh JWT token via `POST /auth/token` with valid admin credentials.
- Cache the token locally with a proactive refresh timer (e.g. refresh at 50 minutes for a 60-minute token).
- Ensure headers are formatted correctly: `Authorization: Bearer eyJ...` or `X-API-Key: fintext_live_...`.

---

### `403 Forbidden`

Returned when the caller is authenticated, but does not have permission to access the requested resource or is connecting from an unauthorized network address.

#### Example Response: IP Whitelist Block
```json
{
  "error": "Forbidden",
  "message": "IP address not allowed"
}
```

#### Example Response: Insufficient RBAC Permissions
```json
{
  "error": "Forbidden",
  "message": "Admin role required to access DLQ management endpoints"
}
```

#### Common Causes
1. **IP Whitelist Restriction**: The user has one or more CIDR entries registered in `/security/ip-whitelist`, and the client's public IP does not match any entry.
2. **Role-Based Access Control (RBAC)**: A user with `role: "trader"` or `role: "viewer"` attempts to access an administrator route (e.g. `/admin/dlq` or `/admin/reprocess`).
3. **Organization Isolation**: Attempting to query or mutate an organization ID where the user is not a member.

#### Client Fix
- Check the client's egress IP address and add it via `POST /security/ip-whitelist`.
- If IP whitelisting is not required, delete all whitelist entries for the account to revert to unrestricted access.
- Obtain an administrative token with `"role": "admin"` for system governance operations.

---

### `404 Not Found`

Returned when the requested URL path or underlying entity (user, organization, custom universe, filing, or transcript) does not exist.

#### Example Response: Resource Not Found
```json
{
  "error": "Not Found",
  "message": "Custom universe 'tech_alpha_50' not found"
}
```

#### Example Response: Point-in-Time Record Not Found Prior to Valid Window
```json
{
  "error": "No sentiment events found as of '2025-06-14T00:00:00Z' for ticker 'AAPL'",
  "ticker": "AAPL",
  "date": "LATEST",
  "status": "not_found"
}
```

#### Common Causes
1. **Non-Existent ID / Slug**: Requesting an invalid organization UUID or deleted webhook subscriber.
2. **Uncovered Ticker**: Querying historical data for a ticker symbol not present in QuestDB or the active ingestion universe.
3. **Querying Prior to Historical Genesis**: Point-in-time `as_of_utc` query targeted a timestamp before any valid sentiment record was committed for the ticker.
4. **Route Typo**: Calling `/sentiment/historic` instead of `/sentiment/history`.

#### Client Fix
- Query `GET /universes` or `GET /orgs` to list existing identifiers before performing lookups.
- Use `GET /sentiment/revisions?ticker={TICKER}` to inspect the first available `valid_from` timestamp in the lineage before running historical as-of queries.
- Check the official route mappings in [Swagger UI](http://127.0.0.1:8000/swagger-ui).

---

### `409 Conflict`

Returned when attempting to create a resource that violates unique constraints.

#### Example Response: Duplicate Registration
```json
{
  "error": "Conflict",
  "message": "Email 'trader@hedgefund.com' is already registered"
}
```

#### Common Causes
1. **User Registration Conflict**: An account already exists with the supplied email.
2. **Custom Universe Name Conflict**: Attempting to create a custom universe with a name that already exists for that user/organization.

#### Client Fix
- If the account already exists, authenticate via `POST /auth/token` instead of registering again.
- Use `PUT /universes/{id}` to update existing universes rather than creating duplicates.

---

### `410 Gone`

Returned when attempting to access an API endpoint that has been permanently retired in version 1.0.

#### Example Response: Retired Endpoint
```json
{
  "error": "Gone",
  "message": "This endpoint has been permanently removed in v1.0. Consult the documentation for migration guidance."
}
```

Includes standard RFC 8594 `Sunset` response header:
```http
Sunset: Wed, 11 Nov 2026 00:00:00 GMT
```

#### Affected Endpoints
- `POST /auth/register` (and `/v1/auth/register`)
- `GET /news/articles/:id` (and `/v1/news/articles/:id`)
- `POST /audio/transcribe` (and `/v1/audio/transcribe`)

---

### `422 Unprocessable Entity`

Returned by the Axum web framework when the incoming request body is syntactically valid JSON, but cannot be deserialized into the target Rust data structure.

#### Example Response: Deserialization Failure
```json
{
  "error": "Unprocessable Entity",
  "message": "Failed to deserialize the JSON body into the target type: invalid type: string \"five\", expected u64 at line 4 column 23"
}
```

#### Common Causes
1. **Mismatched Field Types**: Sending `"holding_days": "5"` (string) instead of `5` (integer).
2. **Missing Non-Nullable Fields**: Omitting mandatory parameters in the request payload (e.g. missing `ticker` in `/signals/quality-report`).
3. **Malformed Enums**: Passing an unsupported enum string (e.g. `"aggregation": "MINUTES"` instead of `"1m"`).

#### Client Fix
- Inspect the endpoint's request model in `python_sdk/src/fintext/models.py` or Swagger UI.
- Use strict typing in your client library (e.g. Pydantic models in Python).

---

### `429 Too Many Requests`

Returned when the client exceeds their configured rate limit or monthly institutional quota.

#### Example Response: Per-Minute Burst Limit Exceeded
```json
{
  "error": "Too Many Requests",
  "message": "Rate limit exceeded. Quota: 100 requests per 60s. Retry after 28s."
}
```

#### Example Response: Monthly Plan Quota Exceeded
```json
{
  "error": "Too Many Requests",
  "message": "Monthly quota exceeded for your plan tier. Please upgrade at /billing/checkout."
}
```

#### Common Causes
1. **Rapid Bursting**: Firing unthrottled concurrent requests without client-side rate limiting.
2. **Monthly Plan Ceiling**: Exhausting total allowed monthly calls on Starter or Pro tiers.

#### Client Fix
- Read the `retry-after` header and sleep for the specified duration before re-transmitting.
- Implement token-bucket or leaky-bucket rate limiting on the client side.
- For high-frequency trading workloads, upgrade to an Enterprise Tier or configure higher limits in `.env` via `RATE_LIMIT_REQUESTS`.

---

### `500 Internal Server Error`

Returned when an unhandled server-side fault occurs, such as a database connection loss or internal pipeline timeout.

#### Example Response
```json
{
  "error": "Internal Server Error",
  "message": "Failed to execute database query"
}
```

#### Remediation
- **Client Side**: Treat 500 errors as transient; retry with exponential backoff and jitter up to 3 times.
- **Server Side**: Inspect server logs (`RUST_LOG=info` or `debug`). Ensure PostgreSQL, QuestDB, and Kafka/Redpanda daemons are healthy. If operating in a standalone development environment without external services, ensure `QUESTDB_MOCK_FALLBACK=1` and `KAFKA_MOCK_FALLBACK=1` are enabled in `.env`.

---

### `503 Service Unavailable` (Production Mode Guard)

Returned when the server operates in **Production Mode** (`PRODUCTION_MODE=true` or `production_mode: true` in `config/config.yaml`) and a required live upstream data source (QuestDB, Kafka/Redpanda, Polygon.io, Finnhub, Whisper transcription, or PostgreSQL) is disconnected or unreachable.

In production mode, **all synthetic mock fallbacks are strictly prohibited and disabled**. The system guarantees zero synthetic data leakage into quantitative customer trading pipelines by immediately returning HTTP 503 rather than serving fallback mock payloads.

#### Example Response
```json
{
  "error": "Service Unavailable",
  "message": "Required data source unavailable in production mode.",
  "status": "service_unavailable"
}
```

#### Common Causes
1. **QuestDB Unreachable**: QuestDB ILP / HTTP query server is down, misconfigured, or unreachable from the API server container.
2. **Kafka Broker Offline**: Redpanda / Kafka cluster is down and live market stream cannot be consumed.
3. **Missing API Keys**: Upstream market data providers (Polygon, Finnhub) have missing, invalid, or expired credentials in production mode.
4. **Transcription Service Offline**: Whisper model/API endpoint is unavailable.

#### Remediation
- **Infrastructure Team**: Verify network connectivity and health of upstream time-series and streaming clusters (`docker compose ps`, `kubectl get pods -n fintext`).
- **Configuration**: Ensure production secrets (`POLYGON_API_KEY`, `FINNHUB_API_KEY`, `QUESTDB_URL`, `KAFKA_BROKERS`) are properly mounted via Kubernetes Secrets or production `.env`.
- **Local Dev / Testing**: For local development or CI environments where mock fallbacks are desired, set `PRODUCTION_MODE=false` (or omit the variable).

---

## 4. Rate Limiting & Quota Management

The FinText API utilizes an in-memory sliding-window token bucket algorithm combined with PostgreSQL monthly usage tracking.

### Headers Sent on Every Request
```http
x-ratelimit-limit: 100
x-ratelimit-remaining: 74
x-ratelimit-reset: 48
```

### Rate Limit Tiers

| Tier Plan | Burst Rate (req / min) | Monthly Quota | WebSocket Stream Access |
| :--- | :---: | :---: | :---: |
| **Starter** | 60 | 50,000 | ❌ No |
| **Professional** | 300 | 500,000 | 🟢 5 Concurrent Streams |
| **Enterprise / Institutional** | 5,000+ | Unlimited | 🟢 Dedicated JetStream & Kafka |

---

## 5. IP & CIDR Whitelisting Guide

The API features zero-trust IP access control.

```mermaid
flowchart TD
    Req[Incoming HTTP Request] --> Auth[Validate JWT / API Key]
    Auth --> CheckRules{Does User Have Active Whitelist Rules?}
    CheckRules -- No Rules Configured --> Allow[Allow Access (Default Open)]
    CheckRules -- 1+ Rules Active --> Match{Does Client IP Match Any Whitelisted CIDR?}
    Match -- Yes --> Allow
    Match -- No --> Deny[Return HTTP 403 Forbidden]
```

### Whitelist Management Endpoints

| Method | Endpoint | Description |
| :--- | :--- | :--- |
| `GET` | `/security/ip-whitelist` | List all active IP rules for the authenticated user |
| `POST` | `/security/ip-whitelist` | Register a new IP address (`203.0.113.14`) or CIDR block (`203.0.113.0/24`) |
| `DELETE`| `/security/ip-whitelist/{id}` | Delete a whitelist rule by its UUID |

---

## 6. Model Governance & Validation Endpoints

FinText Alpha Vectorizer exposes automated model governance, lineage, and quantitative validation endpoints:

| Method | Endpoint | Auth | Description |
| :--- | :--- | :---: | :--- |
| `GET` | `/model-card` | Public | Comprehensive model architecture, ONNX quantization (`INT8_dynamic`), latency benchmarks, and lineage |
| `GET` | `/model-validation` | Bearer JWT | Quantitative benchmark evaluation metrics (accuracy $\ge 0.80$, macro F1 $\ge 0.80$, 3x3 confusion matrix, 10-decile probability calibration curve, ECE $\le 0.15$, Brier score $\le 0.25$) |

### Common Error Responses
- **HTTP 401 Unauthorized**: Missing or expired `Authorization: Bearer <jwt>` header when accessing protected endpoints such as `/model-validation`.
- **HTTP 400 Bad Request**: Invalid query parameters provided to `/model-validation?dataset_version=...`.

---

## 7. WebSocket & Streaming Errors

When connecting to the live WebSocket broadcast feed at `ws://127.0.0.1:8000/ws/sentiment`:

### Connection Handshake Failures
- **HTTP 401 Unauthorized**: Missing `?token=<jwt>` query parameter or invalid token during upgrade.
- **HTTP 403 Forbidden**: Client IP rejected by whitelist middleware before WebSocket upgrade.
- **WebSocket Close Code 1011 (Service Unavailable)**: In Production Mode (`PRODUCTION_MODE=true`), if the upstream message broker (Kafka/Redpanda) is disconnected, the server sends an error message `{"error": "Service Unavailable", "message": "Required streaming data source unavailable in production mode.", "status": "service_unavailable"}` and terminates the connection with a `1011 Internal Error / Service Unavailable` close frame. Synthetic ticker simulation is strictly disabled.

### In-Session Protocol Errors
If the client transmits an unrecognized or malformed command frame over an active WebSocket connection:
```json
{
  "event": "error",
  "message": "Malformed message format: expected valid JSON subscription object"
}
```

---

## 7. Client-Side Integration Best Practices

### 1. Robust Exponential Backoff with Jitter
When encountering `429 Too Many Requests` or `500 Internal Server Error`, use randomized exponential backoff:

$$\text{Delay} = \min\left(\text{MaxDelay},\, \text{InitialDelay} \times 2^{\text{attempt}}\right) \pm \text{RandomJitter}$$

### 2. Python Client Example (using `httpx` and `fintext`)
```python
import httpx
import time
import random

def fetch_sentiment_resilient(client: httpx.Client, ticker: str, max_retries: int = 3) -> dict:
    url = f"http://127.0.0.1:8000/sentiment?ticker={ticker}"
    
    for attempt in range(max_retries):
        response = client.get(url)
        
        if response.status_code == 200:
            return response.json()
        
        if response.status_code == 429:
            retry_after = int(response.headers.get("retry-after", 2 ** attempt))
            jitter = random.uniform(0.1, 0.5)
            time.sleep(retry_after + jitter)
            continue
            
        if response.status_code >= 500:
            time.sleep((2 ** attempt) + random.uniform(0.1, 0.5))
            continue
            
        # Unrecoverable client error (400, 401, 403, 404, 422)
        response.raise_for_status()
        
    raise RuntimeError(f"Failed to fetch sentiment for {ticker} after {max_retries} retries")
```

### 3. Pre-Flight Input Validation
- **Ticker Validation**: Ensure ticker symbols match `^[A-Za-z0-9.]{1,6}$`.
- **Date Validation**: Validate dates against `YYYY-MM-DD` and verify `start_date <= end_date`.
- **Bounds Checking**: Restrict pagination parameters (`limit`) to `1000` or lower.

---

## 8. Data Quality Ingestion Gate Error & Quarantine Catalog

The FinText ingestion engine routes all incoming documents through four validation gates before ML inference and persistent storage. Records failing validation are either **REJECTED** (schema failures) or **QUARANTINED** (business rule violations, duplicate data, or low source quality).

### Validation Gate Sequence

| Stage | Gate Name | Evaluation Criteria | On Violation | Diagnostic Reason Code |
| :---: | :--- | :--- | :---: | :--- |
| **1** | `SCHEMA_VALIDATION` | Required fields (`id`, `title`, `source`, `published_utc`) non-empty, RFC-3339 timestamp valid | **REJECT** | `SCHEMA_VALIDATION_FAILED`: Document ID, title, source missing or malformed published timestamp |
| **2** | `BUSINESS_RULES` | Ticker format (1–10 alphanumeric chars), publication timestamp not in future (tolerance $\le$ 300s) | **QUARANTINE** | `BUSINESS_RULE_VIOLATION`: `Invalid ticker format` or `Published timestamp is in the future` |
| **3** | `DUPLICATE_DETECTION` | Natural key hash of `(source_id, ticker, published_utc)` checked against 24h cache | **QUARANTINE** | `DUPLICATE_DOCUMENT`: `duplicate` |
| **4** | `SOURCE_QUALITY_THRESHOLD` | Source composite quality score $\ge$ threshold (default `0.60`) | **QUARANTINE** | `LOW_SOURCE_QUALITY`: `low_source_quality: score X below threshold Y` |
| **5** | **ACCEPT** | Passes all 4 gates | **ACCEPT** | Processed through ONNX sentiment pipeline and QuestDB/Kafka sinks |

### Quarantine Storage & Persistence

Quarantined documents are stored in structured JSON files with atomic rename semantics:
```text
data/quarantine/<source>/<YYYY-MM-DD>/<uuid>.json
```

#### Example Quarantine File Envelope
```json
{
  "quarantine_id": "8f3e5b41-3829-4d8b-9e20-5712f5a6b7c8",
  "quarantined_at": "2026-09-06T12:00:00Z",
  "rule": "BUSINESS_RULES",
  "reason": "Published timestamp '2026-09-06T14:30:00Z' is in the future (current: '2026-09-06T12:00:00Z', tolerance: 300s)",
  "source": "finnhub",
  "document": {
    "id": "fh-article-9941",
    "title": "Future Market Forecast",
    "source": "finnhub",
    "url": "https://finnhub.io/news/9941",
    "published_utc": "2026-09-06T14:30:00Z",
    "raw_content": "Upcoming earnings expectations..."
  }
}
```

