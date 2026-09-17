# FinText Alpha Vectorizer — Postman Collection (32 Core Endpoints)

> **Specification**: Postman Collection Format v2.1.0  
> **Target Gateway**: `http://127.0.0.1:8000/v1`  
> **Collection File**: [`FinText_Alpha_Vectorizer_32_core.postman_collection.json`](./FinText_Alpha_Vectorizer_32_core.postman_collection.json)  
> **Last Verified**: 2026-09-17 (Suite #274)

---

## 1. Quick Import & Setup (2 Minutes)

1. Open **Postman** (Desktop or Web).
2. Click **Import** (top-left) and drag & drop `FinText_Alpha_Vectorizer_32_core.postman_collection.json`.
3. The collection imports with 8 organized folders containing all **32 Core Production Endpoints**.

---

## 2. Environment Variables

The collection includes pre-configured variables. You can edit them at the collection level:

| Variable | Default Value | Description |
| :--- | :--- | :--- |
| `base_url` | `http://127.0.0.1:8000` | Host and port of the FinText Axum API gateway |
| `admin_token` | `your_admin_token_here` | Administrative development secret used to mint JWTs |
| `jwt_token` | *(auto-populated)* | Bearer JWT acquired from `POST /v1/auth/token` |
| `ticker` | `AAPL` | Target equity ticker symbol |
| `as_of` | `2023-01-03T16:00:00Z` | Historical Point-in-Time RFC3339 evaluation timestamp |

---

## 3. Recommended Execution Flow

To achieve **First Signal in 5 Minutes**:

1. **Step 1 — Verify Health**:  
   Run `1. System & Diagnostics -> 1.1 System Health Probe`. Expect HTTP `200 OK`.
2. **Step 2 — Acquire Bearer Token**:  
   Run `2. Authentication & Identity -> 2.1 Acquire Bearer JWT Token`.  
   *Note: A Postman test script automatically captures `token` from the response and saves it to `{{jwt_token}}`.*
3. **Step 3 — Query Point-in-Time Sentiment**:  
   Run `3. Sentiment & NLP Signals -> 3.1 Point-in-Time Asset Sentiment`.  
   Returns the SCD2 point-in-time sentiment score for `AAPL`.
4. **Step 4 — Verify Zero-Lookahead Replay**:  
   Run `5. Point-in-Time (PIT) Intelligence -> 5.1 Historical State Replay`.  
   Reconstructs the exact database state visible on Jan 3, 2023.

---

## 4. Folder & Request Breakdown (32 Total)

- **1. System & Diagnostics (3 requests)**: `/v1/health`, `/v1/readyz`, `/v1/model-card`
- **2. Authentication & Identity (4 requests)**: `/v1/auth/token`, `/v1/users/me`, `/v1/users/api-keys` (POST/GET)
- **3. Sentiment & NLP Signals (7 requests)**: `/v1/sentiment`, `/v1/sentiment/batch`, `/v1/sentiment/history`, `/v1/sentiment/feed`, `/v1/sentiment/entities`, `/v1/sentiment/sector`, `/v1/sentiment/disagreement`
- **4. Options Microstructure (5 requests)**: `/v1/options/iv`, `/v1/options/microstructure`, `/v1/options/unusual`, `/v1/options/vol-surface`, `/v1/options/put-call-ratio`
- **5. Point-in-Time (PIT) Intelligence (3 requests)**: `/v1/pit/replay`, `/v1/pit/certificate`, `/v1/symbols/map`
- **6. Corporate Events & Graph Intelligence (4 requests)**: `/v1/events/8k`, `/v1/events/earnings-surprise`, `/v1/events/insider-trading`, `/v1/events/supply-chain-risk`
- **7. Universes & Call Transcripts (3 requests)**: `/v1/universes` (GET/POST), `/v1/transcripts`
- **8. Signal Quality & Research Export (3 requests)**: `/v1/signals/quality-report`, `/v1/export/parquet`, `/v1/webhooks`

---

## 5. Automated Testing via Newman (CLI)

You can run the full collection headlessly using `newman`:

```bash
npm install -g newman
newman run postman/FinText_Alpha_Vectorizer_32_core.postman_collection.json \
  --env-var "base_url=http://127.0.0.1:8000" \
  --env-var "admin_token=${ADMIN_TOKEN:-your_admin_token_here}"
```
