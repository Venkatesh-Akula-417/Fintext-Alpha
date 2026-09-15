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

### MiniLM-Seq32 Headline Model
- **Removed in:** Suite #258 / Consistency Matrix A13
- **Reason:** Replaced by 2-tier domain-specific FinBERT inference chain (Fine-tuned FinBERT INT8 v3.1.0 primary -> Base FinBERT v3.0.0 fallback).
- **Files Removed:** `models/minilm_seq32/`, exporter scripts moved to `_archive/scripts/`.

### Archived Handlers & Unused API Surfaces (P0-9 Surface Reduction)
- **Archived in:** P0-9 / Lean Production Optimization
- **Reason:** The original API had over 120 experimental, unmonetized endpoints across speculative asset classes (crypto, commodities, FX) and toy portfolio optimization endpoints that added maintenance overhead and attack surface. The platform is now standardized on 32 institutional core `/v1` endpoints tailored specifically to its Primary ICP (Mid-Frequency Quant Funds, Stat-Arb, Event-Driven Hedge Funds), with operational and audit endpoints isolated under `/internal/*`.
- **Files Archived (Preserved in `_archive/handlers/`):**
  - `alpha_report.rs`
  - `anomaly_scan.rs`
  - `backtest.rs`
  - `bankruptcy_risk.rs`
  - `commodity_sentiment.rs`
  - `credit_sentiment.rs`
  - `crypto_sentiment.rs`
  - `esg_scores.rs`
  - `event_study.rs`
  - `factor_exposure.rs`
  - `fix_orders.rs`
  - `fx_sentiment.rs`
  - `language_detect.rs`
  - `ma_rumors.rs`
  - `market_breadth.rs`
  - `market_regime.rs`
  - `portfolio_factor_exposure.rs`
  - `portfolio_optimize.rs`
  - `regulatory_filings.rs`
  - `return_correlation.rs`
  - `sector_rotation.rs`
  - `spillover_matrix.rs`
  - `spillovers.rs`

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
- `models/finbert/` (v3.0.0) — fallback for `models/finbert-finetuned/` (v3.1.0).

### VerticalPodAutoscaler (VPA) Manifest
- `k8s/vpa-api.yaml` — Retained in `k8s/` and registered in `k8s/kustomization.yaml` per Suite #253 test contract (`scripts/verify_single_region_k8s.py`).

---

## Active Technologies (Do Not Remove)

- Rust 1.80.1, Axum 0.7, Tokio 1.36
- PostgreSQL 16 + TimescaleDB (primary storage)
- QuestDB (optional hot-cache time-series, profile: `hot-cache`)
- Redpanda / Kafka (event streaming)
- AWS S3 / Local Parquet Archive (`data/archive`, cold storage)
- FinBERT fine-tuned INT8 ONNX (primary NLP v3.1.0)
- Whisper.cpp (audio ASR)
- Vault / AWS Secrets Manager (secrets)
