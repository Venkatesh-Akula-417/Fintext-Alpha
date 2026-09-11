# Deprecated & Removed Features

This file is the AUTHORITATIVE log of features, technologies, and files removed from the FinText Alpha Vectorizer project. Auditors should reference this file to understand what is no longer present and why.

**Last Updated:** 2026-09-10

---

## Removed Technologies

### GraphQL API
- **Removed in:** Suite #231
- **Reason:** Superseded by REST + WebSocket; unnecessary complexity
- **Replacement:** `/v1/*` REST endpoints, `/ws` WebSocket streams
- **Files Removed:** `rust/api_server/src/graphql_api.rs`, `async-graphql` dependency
- **Verification:** `scripts/verify_graphql_elimination.py` (Suite #231)

### ClickHouse Cold Storage
- **Removed in:** Suite #252
- **Reason:** Consolidated to QuestDB (hot) + TimescaleDB (warm) + S3 Parquet (cold)
- **Files Removed:** `config/clickhouse/init.sql`, all ClickHouse service definitions
- **Verification:** `scripts/verify_clickhouse_elimination.py` (Suite #252)

### NATS JetStream Messaging
- **Removed in:** Suite #251
- **Reason:** Consolidated to Kafka/Redpanda only
- **Replacement:** Kafka topics `sentiment-updates`, `sentiment-events`, `sentiment-dlq`
- **Files Removed:** `rust/ingestion_engine/src/streaming/nats_sink.rs`, `rust/api_server/src/streaming/nats_subscriber.rs`, `async-nats` dependency
- **Verification:** `scripts/verify_kafka_messaging_simplification.py` (Suite #251)

### Multi-Region Kubernetes Manifests
- **Removed in:** Suite #253
- **Reason:** Single-region bootstrap phase
- **Replacement:** Single-region Kustomize topology in `k8s/`
- **Verification:** `scripts/verify_single_region_k8s.py` (Suite #253)

### Debian 11 (Legacy) Base Images
- **Removed in:** Suite #254
- **Replacement:** Debian Bookworm + Rust 1.80.1
- **Verification:** `scripts/verify_toolchain_and_docker.py` (Suite #254)

### MiniLM-L6-v2 Model
- **Removed in:** Suite #258
- **Reason:** Superseded by fine-tuned FinBERT INT8
- **Fallback:** `models/finbert/` (v3.0.0) is retained as fallback for `models/finbert-finetuned/` (v3.1.0)
- **Verification:** `scripts/verify_finetuned_model.py` (Suite #258)

### Retired Data Sources
- **Alpha Vantage** — Removed due to licensing restrictions. Benchmark reliability: 0.75
- **NewsAPI** — Removed due to commercial use restrictions. Benchmark reliability: 0.60
- **Tiingo** — Removed due to entitlement issues. Benchmark reliability: 0.80
- **RSS Feeds** — Removed due to spam noise. Parser utility preserved in `html_sanitizer`.

**Note:** Benchmark reliability values retained in `SOURCE_RELIABILITY` for historical data quality comparison only.

---

## Kept Fallback Code (Intentional)

### Static JSON Config Files (Fallback Only)
The following files are FALLBACKS used only when `pit_database.enabled=false`:
- `config/ticker_history.json`
- `config/delisted_securities.json`
- `config/corporate_actions.json`
- `config/index_membership.json`

**Canonical source:** PostgreSQL bi-temporal tables in `config/pit_reference_schema.sql`.

### Base FinBERT Model (Fallback)
- `models/finbert/` (v3.0.0) — fallback for `models/finbert-finetuned/` (v3.1.0)
- `models/minilm_seq32/` — ultra-fast 32-token headline fallback when FinBERT is unavailable

### VerticalPodAutoscaler (VPA) Manifest
- `k8s/vpa-api.yaml` — Retained in `k8s/` and registered in `k8s/kustomization.yaml` per Suite #253 test contract (`scripts/verify_single_region_k8s.py`).

---

## Active Technologies (Do Not Remove)

- Rust 1.80.1, Axum 0.7, Tokio 1.36
- PostgreSQL 16 + TimescaleDB (primary storage)
- QuestDB (hot-path time-series, dual-storage)
- Redpanda / Kafka (event streaming)
- AWS S3 / MinIO (cold archival, Parquet)
- FinBERT fine-tuned INT8 ONNX (primary NLP)
- Whisper.cpp (audio ASR)
- Vault / AWS Secrets Manager (secrets)
