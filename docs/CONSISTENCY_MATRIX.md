# FinText Documentation Consistency Matrix

> Generated: 2026-09-13 14:30:00 UTC  
> Scope: code vs docs vs config vs env  
> Status: RESOLVED / RECONCILED (P0-P1 Architecture, Storage Consolidation, and Config Fixes Applied 2026-09-15)  

---

## Executive Summary

- **Total drift findings**: 49
- **Critical (P0)**: 10
- **High (P1)**: 23
- **Medium (P2)**: 13
- **Low (P3)**: 3

This document is an exhaustive, strictly read-only consistency audit cross-referencing repository implementation code (Rust crates, Python SDK, automation scripts), active configurations (`config/`, `.env.example`, `docker-compose.yml`, `k8s/`), continuous integration workflows (`.github/workflows/`), and platform documentation (`README.md`, `docs/*.md`, crate READMEs).

---

## Category A: Reference Drift (Removed Features)

This category audits lingering mentions of features, models, or components that have been removed or superseded (specifically MiniLM / MiniLM-Seq32, ClickHouse, GraphQL, NATS JetStream, and legacy models).

| # | Reference | File:Line | Content Snippet | Severity | Suggested Action |
| :--- | :--- | :--- | :--- | :---: | :--- |
| A1 | `"MiniLM"` | [`README.md:15`](file:///d:/FinText-Alpha-Vectorizer/README.md#L15) | `"with base FinBERT v3.0.0 and MiniLM v2.1.0 fallbacks"` | **P0** | Update to reflect 2-tier FinBERT fallback chain (FinBERT fine-tuned -> FinBERT base). |
| A2 | `"MiniLM Seq32"` | [`README.md:39`](file:///d:/FinText-Alpha-Vectorizer/README.md#L39) | `SENTIMENT["FinBERT INT8 ONNX (Primary v3.1.0)<br/>Fallback: Base FinBERT + MiniLM Seq32"]` | **P0** | Remove `+ MiniLM Seq32` from Mermaid architecture diagram. |
| A3 | `"models/minilm_seq32/"` | [`README.md:380`](file:///d:/FinText-Alpha-Vectorizer/README.md#L380) | `models/minilm_seq32/ # 32-token headline sentiment v2.1.0 (Fallback)` | **P1** | Delete `minilm_seq32/` from repository directory tree map (directory deleted). |
| A4 | `"MiniLM v2.1.0"` | [`rust/ingestion_engine/README.md:6`](file:///d:/FinText-Alpha-Vectorizer/rust/ingestion_engine/README.md#L6) | `"(fine-tuned INT8 v3.1.0 primary with base FinBERT v3.0.0 and MiniLM v2.1.0 headline fallbacks)"` | **P1** | Align crate documentation with 2-tier FinBERT fallback policy. |
| A5 | `"minilm_seq32/"` | [`models/README.md:12-14`](file:///d:/FinText-Alpha-Vectorizer/models/README.md#L12-L14) | `"- minilm_seq32/ — Ultra-fast 32-token headline fallback (v2.1.0)"` | **P1** | Remove from active fallback section; classify as archived/removed. |
| A6 | `"minilm_seq32/"` | [`models/README.md:20`](file:///d:/FinText-Alpha-Vectorizer/models/README.md#L20) | `"- minilm_seq32/ (if removed) — Legacy model, see docs/DEPRECATED.md"` | **P2** | Resolve in-file contradiction (file lists minilm as both active fallback on line 12 and archived on line 20). |
| A7 | `"minilm_seq32/"` | [`docs/current_architecture.md:101-102`](file:///d:/FinText-Alpha-Vectorizer/docs/current_architecture.md#L101-L102) | `minilm_seq32/ # Fast headline sentiment fallback` | **P1** | Remove non-existent model directory from architectural directory catalog. |
| A8 | `"export_minilm_seq32_onnx.py"` | [`docs/current_architecture.md:130`](file:///d:/FinText-Alpha-Vectorizer/docs/current_architecture.md#L130) | `export_minilm_seq32_onnx.py # Sentiment ONNX export utility` | **P2** | Remove active exporter script reference from architecture manual. |
| A9 | `"MiniLM-FinBERT"` | [`docs/current_architecture.md:156`](file:///d:/FinText-Alpha-Vectorizer/docs/current_architecture.md#L156) | `### 3.1 MiniLM-FinBERT Sentiment Scoring` | **P1** | Replace MiniLM scoring section with FinBERT INT8 inference specifications. |
| A10 | `"MiniLM embedding weights"` | [`docs/MODEL_ASSET_DISTRIBUTION.md:9`](file:///d:/FinText-Alpha-Vectorizer/docs/MODEL_ASSET_DISTRIBUTION.md#L9) | `"NER sequence taggers, and MiniLM embedding weights) are binary files totaling ~155 MB"` | **P2** | Remove MiniLM from list of ML model weights. |
| A11 | `"MiniLM v2.1.0 fallback"` | [`docs/ai_audit_ready_summary.md:13`](file:///d:/FinText-Alpha-Vectorizer/docs/ai_audit_ready_summary.md#L13) | `"with ProsusAI FinBERT v3.0.0 base and MiniLM v2.1.0 fallback"` | **P1** | Remove MiniLM fallback claim from executive AI audit summary. |
| A12 | `"models/minilm_seq32/"` | [`docs/BROKEN_LINKS.md:26`](file:///d:/FinText-Alpha-Vectorizer/docs/BROKEN_LINKS.md#L26) | `| docs/DEPRECATED.md | models/minilm_seq32/ | models/minilm_seq32/ | VALID |` | **P1** | Re-evaluate link status; path does not exist on disk, link is broken. |
| A13 | `"models/minilm_seq32/"` | [`docs/DEPRECATED.md:71`](file:///d:/FinText-Alpha-Vectorizer/docs/DEPRECATED.md#L71) | `"- models/minilm_seq32/ — ultra-fast 32-token headline fallback when FinBERT is unavailable"` | **P1** | Remove from "Kept Fallback Code (Intentional)"; move to "Removed Technologies". |
| A14 | `"MiniLM fallback"` | [`config/feature_flags.yaml:28`](file:///d:/FinText-Alpha-Vectorizer/config/feature_flags.yaml#L28) | `description: "Native in-process ONNX Runtime FinBERT INT8 sentiment classification with MiniLM fallback."` | **P1** | Update feature flag description to remove MiniLM fallback mention. |
| A15 | `"Base MiniLM-L6-v2"` | [`config/config.yaml:164`](file:///d:/FinText-Alpha-Vectorizer/config/config.yaml#L164) | `changes: "Base MiniLM-L6-v2 fine-tuned for sentiment"` in `version_history` | **P3** | Historical release note entry; verify active base model is FinBERT. |
| A16 | `export_minilm_seq32_onnx.py` | [`scripts/export_minilm_seq32_onnx.py:1-144`](file:///d:/FinText-Alpha-Vectorizer/scripts/export_minilm_seq32_onnx.py) | Full active exporter utility generating `models/minilm_seq32/model_static.onnx` | **P2** | Move script to `_archive/scripts/` alongside `export_minilm_finbert_onnx.py`. |
| A17 | `"MiniLM-FinBERT Seq32"` | [`scripts/run_performance_benchmarks.py:182`](file:///d:/FinText-Alpha-Vectorizer/scripts/run_performance_benchmarks.py#L182) | `"Metric": "ONNX Sentiment Inference (MiniLM-FinBERT Seq32)"` | **P2** | Update benchmark metric reporting name to FinBERT. |

---

## Category B: Missing Documentation (New Features)

This category audits newly implemented production features, validation harnesses, monitors, dispatchers, and CI workflows that lack documentation in `README.md` or dedicated pages in `docs/`.

| # | Feature | Code File | Docs Reference | Severity | Suggested Action |
| :--- | :--- | :--- | :--- | :---: | :--- |
| B1 | Point-in-Time (PIT) Correctness Validation Harness | [`scripts/validate_pit_correctness.py`](file:///d:/FinText-Alpha-Vectorizer/scripts/validate_pit_correctness.py), [`scripts/pit_validation/`](file:///d:/FinText-Alpha-Vectorizer/scripts/pit_validation/) | NOT IN README; [`docs/PIT_VALIDATION_REPORT.md`](file:///d:/FinText-Alpha-Vectorizer/docs/PIT_VALIDATION_REPORT.md) unlinked | **P0** | Add PIT correctness validation section to `README.md` and link report in docs directory. |
| B2 | Model Quality & Calibration Validation Harness | [`scripts/validate_model_quality.py`](file:///d:/FinText-Alpha-Vectorizer/scripts/validate_model_quality.py), [`scripts/model_validation/`](file:///d:/FinText-Alpha-Vectorizer/scripts/model_validation/) | NOT IN README testing guide; [`docs/MODEL_VALIDATION_REPORT.md`](file:///d:/FinText-Alpha-Vectorizer/docs/MODEL_VALIDATION_REPORT.md) unlinked | **P1** | Document CLI options (`--tolerance`, `--strict`, exit codes 0-3) in `README.md`. |
| B3 | Model Drift Monitoring Watchdog | [`scripts/model_drift_monitor.py`](file:///d:/FinText-Alpha-Vectorizer/scripts/model_drift_monitor.py), [`scripts/model_drift/`](file:///d:/FinText-Alpha-Vectorizer/scripts/model_drift/) | NOT IN README; [`docs/MODEL_DRIFT_REPORT.md`](file:///d:/FinText-Alpha-Vectorizer/docs/MODEL_DRIFT_REPORT.md) unlinked | **P1** | Add Model Drift Monitoring section in `README.md` and `docs/OPERATIONS.md`. |
| B4 | Model Drift Alert Dispatcher (JSONL + Webhooks) | [`scripts/model_drift/alerts.py`](file:///d:/FinText-Alpha-Vectorizer/scripts/model_drift/alerts.py) | COMPLETELY MISSING from all docs | **P1** | Document `reports/drift_alerts.jsonl` audit trail, webhook payload format, and dispatch triggers in `docs/OPERATIONS.md`. |
| B5 | Model Asset Distribution & Fetch Mechanism | [`scripts/fetch_models.py`](file:///d:/FinText-Alpha-Vectorizer/scripts/fetch_models.py), [`config/models_manifest.json`](file:///d:/FinText-Alpha-Vectorizer/config/models_manifest.json) | NOT IN README; [`docs/MODEL_ASSET_DISTRIBUTION.md`](file:///d:/FinText-Alpha-Vectorizer/docs/MODEL_ASSET_DISTRIBUTION.md) unlinked | **P1** | Document `fetch_models.py` workflow, SKIP mode fallback, and manifest configuration in `README.md`. |
| B6 | Specialized GitHub Actions CI Workflows (4 new pipelines) | [`.github/workflows/model-drift.yml`](file:///d:/FinText-Alpha-Vectorizer/.github/workflows/model-drift.yml), [`model-validation.yml`](file:///d:/FinText-Alpha-Vectorizer/.github/workflows/model-validation.yml), [`pit-validation.yml`](file:///d:/FinText-Alpha-Vectorizer/.github/workflows/pit-validation.yml), [`security-scan.yml`](file:///d:/FinText-Alpha-Vectorizer/.github/workflows/security-scan.yml) | NOT IN README (`README.md:362` only lists `ci.yml`) | **P0** | Add multi-workflow CI table to `README.md` documenting all 5 automated pipelines and triggers. |
| B7 | Docker Compose PostgreSQL (TimescaleDB) Service | [`docker-compose.yml:159-180`](file:///d:/FinText-Alpha-Vectorizer/docker-compose.yml#L159-L180) | NOT IN README deployment section (`README.md:351` lists only QuestDB & API) | **P1** | Document PostgreSQL port `5432` container mapping and healthcheck probe in `README.md`. |

---

## Category C: Config Drift (Env Vars & Config Keys)

This category audits environment variables read by Rust crates or Python scripts against `.env.example` and `config/config.yaml`, identifying contract discrepancies and naming mismatches.

| # | Env Var | Read In Code? | In .env.example? | In config.yaml? | Severity | Action |
| :--- | :--- | :---: | :---: | :---: | :---: | :--- |
| C1 | `CACHE_DEFAULT_TTL_SECS`<br/>`CACHE_MAX_CAPACITY`<br/>`CACHE_CLEANUP_INTERVAL_SECS` | Yes ([`cache.rs:88-100`](file:///d:/FinText-Alpha-Vectorizer/rust/api_server/src/cache.rs#L88-L100)) | Yes (`CACHE_DEFAULT_TTL_SECONDS`, `CACHE_DEFAULT_MAX_CAPACITY`, `CACHE_CLEANUP_INTERVAL_SECONDS`) | Yes (`default_ttl_seconds`, `default_max_capacity`, `cleanup_interval_seconds`) | **P0** | **FIXED**: `cache.rs` aligned to parse `_SECONDS` consistently across env vars and YAML config. |
| C2 | `ENABLE_TIMESCALEDB`<br/>`TIMESCALE_PRIMARY`<br/>`TIMESCALE_DB_URL`<br/>`TIMESCALE_MAX_CONNECTIONS`<br/>`TIMESCALE_MOCK_FALLBACK` | Yes ([`timescaledb.rs:36-68`](file:///d:/FinText-Alpha-Vectorizer/rust/ingestion_engine/src/storage/timescaledb.rs#L36-L68)) | Yes (Added to `.env.example`) | Yes (`database.timescaledb.*`) | **P1** | **FIXED**: TimescaleDB primary storage environment variables added to `.env.example`. |
| C3 | `MODEL_DRIFT_WEBHOOK_URL`<br/>`DRIFT_ALERT_LOG_PATH` | Yes ([`alerts.py:102,130`](file:///d:/FinText-Alpha-Vectorizer/scripts/model_drift/alerts.py#L102)) | Yes (Added to `.env.example`) | Yes | **P1** | **FIXED**: Documented webhook URL and alert log destination in `.env.example`. |
| C4 | `MODEL_DIR`<br/>`FINBERT_MODEL_DIR`<br/>`FINBERT_MODEL_PATH`<br/>`FINBERT_TOKENIZER_PATH` | Yes ([`onnx_sentiment.rs:783-798`](file:///d:/FinText-Alpha-Vectorizer/rust/ingestion_engine/src/nlp/onnx_sentiment.rs#L783-L798)) | Yes (Added to `.env.example`) | Yes (`sentiment_config.model_path`) | **P1** | **FIXED**: Model path and directory override environment variables added to `.env.example`. |
| C5 | `SPILLOVER_INTERVAL_SECS`<br/>`SPILLOVER_LOOKBACK_DAYS`<br/>`SPILLOVER_CORR_THRESHOLD`<br/>`SPILLOVER_MIN_HOURS`<br/>`SPILLOVER_MAX_LAG_HOURS`<br/>`SPILLOVER_MAX_TICKERS`<br/>`SPILLOVER_MOCK_MODE` | Yes ([`spillover_engine/src/config.rs:44-85`](file:///d:/FinText-Alpha-Vectorizer/rust/spillover_engine/src/config.rs#L44-L85)) | Yes (Added to `.env.example`) | Yes | **P2** | **FIXED**: Dedicated Cross-Asset Spillover section added to `.env.example`. |
| C6 | `PROMETHEUS_URL`<br/>`PROMETHEUS_QUERY`<br/>`ANOMALY_THRESHOLD_ZSCORE`<br/>`ANOMALY_WINDOW_SIZE`<br/>`CHECK_INTERVAL_SECS`<br/>`WEBHOOK_URL` | Yes ([`observability_anomaly/src/config.rs:44-64`](file:///d:/FinText-Alpha-Vectorizer/rust/observability_anomaly/src/config.rs#L44-L64)) | Yes (Added to `.env.example`) | Yes | **P2** | **FIXED**: Anomaly Detector & Observability Sentinel section added to `.env.example`. |
| C7 | `S3_QUARANTINE_BUCKET`<br/>`LOCAL_QUARANTINE_DIR`<br/>`DLQ_MAX_RETRIES`<br/>`DLQ_INITIAL_BACKOFF_MS`<br/>`DLQ_MAX_BACKOFF_MS`<br/>`DLQ_MOCK_MODE` | Yes ([`dead_letter_worker/src/config.rs`](file:///d:/FinText-Alpha-Vectorizer/rust/dead_letter_worker/src/config.rs)) | Yes (Added to `.env.example`) | Yes | **P2** | **FIXED**: Dead Letter Queue (DLQ) Auto-Reprocessor section added to `.env.example`. |
| C8 | `MODEL_MANIFEST_PATH` | Yes ([`fetch_models.py:220`](file:///d:/FinText-Alpha-Vectorizer/scripts/fetch_models.py#L220)) | Yes (Added to `.env.example`) | Yes | **P2** | **FIXED**: `MODEL_MANIFEST_PATH=config/models_manifest.json` added to `.env.example`. |
| C9 | `ORT_CPU_THREADS`<br/>`ORT_EXECUTION_PROVIDER`<br/>`ORT_INT8_ENABLE`<br/>`NER_CPU_THREADS`<br/>`NER_EXECUTION_PROVIDER` | Yes ([`onnx_sentiment.rs:520-560`](file:///d:/FinText-Alpha-Vectorizer/rust/ingestion_engine/src/nlp/onnx_sentiment.rs#L520), [`ner.rs:250-290`](file:///d:/FinText-Alpha-Vectorizer/rust/ingestion_engine/src/nlp/ner.rs#L250)) | Yes (Added to `.env.example`) | Yes | **P2** | **FIXED**: ONNX Runtime hardware execution provider tuning options added to `.env.example`. |
| C10 | `BACKUP_S3_BUCKET`<br/>`SLACK_WEBHOOK_URL`<br/>`STRIPE_PRICE_ENTERPRISE_MONTHLY`<br/>`STRIPE_PRICE_PRO_MONTHLY` | No (Unused in Rust/Python active runtime code) | Yes (Annotated under UNUSED section) | Yes | **P3** | **FIXED**: Unused template variables annotated under dedicated UNUSED section in `.env.example`. |

---

## Category D: Version Drift

This category audits model version identifiers, schema versions, and verification counts between codebase claims and runtime reality.

| # | Claim | Claim Location | Actual Value | Severity | Action |
| :--- | :--- | :--- | :--- | :---: | :--- |
| D1 | Model Version: `v3.1.0` | [`README.md:15,39,251`](file:///d:/FinText-Alpha-Vectorizer/README.md#L15) (`"FinBERT INT8 v3.1.0"`) | `rust/api_server/src/models/sentiment.rs:24` sets `DEFAULT_MODEL_VERSION = "finbert-minilm-v2.1"`; OpenAPI examples cite `"finbert-minilm-v2.1"`; `language.rs:8` sets `DEFAULT_ENGLISH_MODEL = "finbert-minilm-v2.1"` | **P0** | **Critical Semantic Invariant**: Align `DEFAULT_MODEL_VERSION` in API server models to `"finbert-v3.1.0"` (or `"finbert-finetuned"`). |
| D2 | Python SDK Model Version | [`python_sdk/src/fintext/models.py:24,69,2261`](file:///d:/FinText-Alpha-Vectorizer/python_sdk/src/fintext/models.py#L24) | `model_version: str = Field(default="finbert-minilm-v2.1")` | **P0** | Update Pydantic default model version to `"finbert-v3.1.0"`. |
| D3 | Master Test Certification Suite Count | [`README.md:13,208,398`](file:///d:/FinText-Alpha-Vectorizer/README.md#L13) (`"96/96 Suites Certified"`) vs [`README.md:397`](file:///d:/FinText-Alpha-Vectorizer/README.md#L397) (`"112 verification & test suite orchestrators"`) | `scripts/run_all_tests.py` registers 96 test suites (`Suite #179` through `Suite #273` + cargo workspace); directory contains 112 Python test scripts | **P1** | Clarify distinction in `README.md` between master certification orchestrator suites (96) and total verification scripts (112). |
| D4 | Feature Flag Schema Version | [`config/feature_flags.yaml:6`](file:///d:/FinText-Alpha-Vectorizer/config/feature_flags.yaml#L6) (`"2.0.0-institutional"`) vs [`config/models_manifest.json:2`](file:///d:/FinText-Alpha-Vectorizer/config/models_manifest.json#L2) (`"1.0.0"`) | Discrepancy between manifest schema versioning conventions | **P3** | Document independent versioning scheme for model asset distribution manifests. |
| D5 | Last Verified Timestamp Drift | [`README.md:3`](file:///d:/FinText-Alpha-Vectorizer/README.md#L3), [`docs/CONSISTENCY_MATRIX.md:4`](file:///d:/FinText-Alpha-Vectorizer/docs/CONSISTENCY_MATRIX.md#L4), [`python_sdk/README.md:9`](file:///d:/FinText-Alpha-Vectorizer/python_sdk/README.md#L9), [`models/README.md:3`](file:///d:/FinText-Alpha-Vectorizer/models/README.md#L3) | `"Last Verified: 2026-09-10 (Suite #96)"` across all headers | **P1** | Bump verification timestamp to reflect commits through 2026-09-13 (MiniLM removal, PIT validation, model validation, drift monitor, alert dispatcher, CI assertion alignment). |

---

## Category E: Workflow Drift

This category audits Continuous Integration / Continuous Deployment (CI/CD) claims against actual GitHub Actions workflow definitions.

| # | Claim | Location | Actual | Severity |
| :--- | :--- | :--- | :--- | :---: |
| E1 | "Single CI workflow" / "8/8 workflows passing" | [`README.md:362`](file:///d:/FinText-Alpha-Vectorizer/README.md#L362), [`docs/MODEL_ASSET_DISTRIBUTION.md:25`](file:///d:/FinText-Alpha-Vectorizer/docs/MODEL_ASSET_DISTRIBUTION.md#L25) | Exactly **5 active workflows** exist in `.github/workflows/` (`ci.yml`, `model-drift.yml`, `model-validation.yml`, `pit-validation.yml`, `security-scan.yml`). | **P0** |
| E2 | "Master Suite executed in CI" | [`README.md:362`](file:///d:/FinText-Alpha-Vectorizer/README.md#L362) (`"Windows CI/CD pipeline (Rust, Python SDK, Master Suite)"`) | Master Suite (`scripts/run_all_tests.py`) was **explicitly removed from `ci.yml:115-120`** due to cold runner timeouts and missing external infrastructure dependencies; CI runs unit tests, clippy, fmt, release build, and pytest. | **P0** |
| E3 | CI Documentation Links | [`README.md:426`](file:///d:/FinText-Alpha-Vectorizer/README.md#L426) | Links exclusively to `.github/workflows/ci.yml`; the other 4 workflows are unreferenced and unlinked. | **P1** |
| E4 | Automated Triggers & Schedules | [`docs/OPERATIONS.md`](file:///d:/FinText-Alpha-Vectorizer/docs/OPERATIONS.md) | Cron schedules (`0 6 * * 1` on drift/validation/security) and workflow path dispatch triggers are completely undocumented in the Operations manual. | **P2** |

---

## Category F: Architecture Drift

This category audits architectural claims regarding inference fallback chains, persistence layers, and storage tiers against code behavior.

| # | Aspect | Code Behavior | Docs Claim | Severity |
| :--- | :--- | :--- | :--- | :---: |
| F1 | Sentiment Inference Fallback Chain | **2-Tier Domain Fallback**: [`onnx_sentiment.rs:846-904`](file:///d:/FinText-Alpha-Vectorizer/rust/ingestion_engine/src/nlp/onnx_sentiment.rs#L846-L904) resolves `models/finbert-finetuned` primary, then falls back to `models/finbert` base. | **3-Tier Fallback**: [`README.md:15,39`](file:///d:/FinText-Alpha-Vectorizer/README.md#L15), [`ingestion_engine/README.md:6`](file:///d:/FinText-Alpha-Vectorizer/rust/ingestion_engine/README.md#L6), and [`current_architecture.md:101,156`](file:///d:/FinText-Alpha-Vectorizer/docs/current_architecture.md#L101) claim a 3-tier chain including `MiniLM Seq32` for fast headlines. | **P0** |
| F2 | Primary Analytical Storage Designation | Dual-storage architecture: PostgreSQL 16 + TimescaleDB designated as primary persistence with QuestDB as hot path (`timescaledb.rs`, `README.md:7,49`). | [`docs/current_architecture.md:33`](file:///d:/FinText-Alpha-Vectorizer/docs/current_architecture.md#L33) summary box lists QuestDB as single time-series storage and omits TimescaleDB from platform core capabilities box. | **P1** |
| F3 | Cold Storage Object Lakehouse | Local filesystem archive `data/archive` used by default (`raw_archive.rs`); S3/MinIO requires explicit cloud configuration. | [`README.md:51`](file:///d:/FinText-Alpha-Vectorizer/README.md#L51) and previous matrix imply in-cluster MinIO service exists, but `docker-compose.yml` has no MinIO container. | **P2** |

---

## Category G: Deployment Drift

This category audits container orchestration (`docker-compose.yml`) and declarative Kubernetes manifests (`k8s/`).

| # | Aspect | Actual | Docs | Severity |
| :--- | :--- | :--- | :--- | :---: |
| G1 | Docker Compose Services Count | **8 services** defined in `docker-compose.yml` (`questdb`, `kafka`, `fintext-ingestion`, `fintext-api`, `fintext-spillover`, `fintext-dead-letter`, `fintext-anomaly-detector`, `postgres`). | [`README.md:405`](file:///d:/FinText-Alpha-Vectorizer/README.md#L405) and previous matrix claim "7-service multi-container local orchestrator" / "7 services". | **P1** |
| G2 | Kubernetes Manifest Directory Paths | Manifests reside at `k8s/postgres-statefulset.yaml`, `k8s/dead-letter-worker.yaml`, `k8s/observability/anomaly-detector.yaml`. | Previous matrix listed non-existent paths under `k8s/deployments/` (e.g. `k8s/deployments/postgres-statefulset.yaml`, `questdb-deployment.yaml`, `kafka-deployment.yaml`, `dlq-worker-deployment.yaml`, `anomaly-detector-deployment.yaml`). | **P1** |
| G3 | Public Repository Licensing Notice | Repository was made public on GitHub. | [`README.md:435`](file:///d:/FinText-Alpha-Vectorizer/README.md#L435) states "Institutional Proprietary Commercial License" and [`python_sdk/README.md:7`](file:///d:/FinText-Alpha-Vectorizer/python_sdk/README.md#L7) displays a `license-Proprietary-red.svg` badge. | **P2** |

---

## Priority Summary

### P0 (Fix Immediately — Auditor First Impression & Runtime Correctness)
1. **C1** — Cache configuration naming bug: code expects `CACHE_DEFAULT_TTL_SECS` / `_SECS`, but `.env.example` and `config.yaml` specify `_SECONDS`, causing cache TTL and cleanup settings to be silently ignored.
2. **D1** — API server default model version in [`rust/api_server/src/models/sentiment.rs:24`](file:///d:/FinText-Alpha-Vectorizer/rust/api_server/src/models/sentiment.rs#L24) and [`language.rs:8`](file:///d:/FinText-Alpha-Vectorizer/rust/api_server/src/language.rs#L8) remains `"finbert-minilm-v2.1"`, contradicting the `v3.1.0` institutional FinBERT standard.
3. **D2** — Python Client SDK default model version in [`python_sdk/src/fintext/models.py:24`](file:///d:/FinText-Alpha-Vectorizer/python_sdk/src/fintext/models.py#L24) defaults to `"finbert-minilm-v2.1"`.
4. **F1** — Architecture fallback chain mismatch: code implements a 2-tier FinBERT chain, while `README.md` and architecture docs claim a 3-tier chain with MiniLM.
5. **A1** — [`README.md:15`](file:///d:/FinText-Alpha-Vectorizer/README.md#L15) prominent opening statement claims MiniLM v2.1.0 fallback.
6. **A2** — [`README.md:39`](file:///d:/FinText-Alpha-Vectorizer/README.md#L39) Mermaid architecture diagram displays MiniLM Seq32 fallback.
7. **E1** — CI workflow count mismatch: docs claim single CI workflow or 8 workflows; exactly 5 exist.
8. **E2** — [`README.md:362`](file:///d:/FinText-Alpha-Vectorizer/README.md#L362) claims CI executes Master Test Suite; step was removed in CI reliability remediation.
9. **B1** — Bi-temporal Point-in-Time (PIT) correctness validation harness is completely missing from `README.md`.
10. **B6** — Four specialized GitHub Actions CI workflows (`model-drift`, `model-validation`, `pit-validation`, `security-scan`) are undocumented in `README.md`.

### P1 (Fix Before Private Beta / Institutional Due Diligence)
1. **A3** — Remove `models/minilm_seq32/` from `README.md` repository directory tree map.
2. **A4** — Update `rust/ingestion_engine/README.md:6` to remove MiniLM headline fallback.
3. **A5** — Update `models/README.md:12-14` to remove MiniLM from active fallbacks.
4. **A7** — Remove `minilm_seq32/` from `docs/current_architecture.md:101`.
5. **A9** — Replace Section 3.1 in `docs/current_architecture.md` with FinBERT INT8 specs.
6. **A11** — Remove MiniLM fallback from `docs/ai_audit_ready_summary.md:13`.
7. **A12** — Mark `models/minilm_seq32/` link in `docs/BROKEN_LINKS.md` as broken/deleted.
8. **A13** — Reclassify `models/minilm_seq32/` in `docs/DEPRECATED.md:71` from kept fallback to fully removed.
9. **A14** — Update feature flag description in `config/feature_flags.yaml:28`.
10. **B2** — Document Model Validation & Calibration harness CLI options in `README.md`.
11. **B3** — Add Model Drift Monitoring section to `README.md` and `docs/OPERATIONS.md`.
12. **B4** — Document Model Drift Alert Dispatcher (`alerts.py`, JSONL audit log, webhooks) in `docs/OPERATIONS.md`.
13. **B5** — Document `fetch_models.py` workflow, SKIP mode, and `models_manifest.json` in `README.md`.
14. **B7** — Document PostgreSQL (TimescaleDB) service in `README.md` Docker Compose quick deployment section.
15. **C2** — Add TimescaleDB primary storage environment variables to `.env.example`.
16. **C3** — Add `MODEL_DRIFT_WEBHOOK_URL` and `DRIFT_ALERT_LOG_PATH` to `.env.example`.
17. **C4** — Add `MODEL_DIR` and `FINBERT_MODEL_DIR` path overrides to `.env.example`.
18. **D3** — Clarify distinction between 96 master orchestrator test suites and 112 verification scripts in `README.md`.
19. **D5** — Update stale "Last Verified: 2026-09-10" headers across all markdown documents.
20. **E3** — Add documentation links for all 5 CI workflows in `README.md:426`.
21. **F2** — Reconcile TimescaleDB primary storage in `docs/current_architecture.md` capability summary.
22. **G1** — Update Docker Compose service count from 7 to 8 in `README.md:405`.
23. **G2** — Fix fictitious `k8s/deployments/` paths in Kubernetes architectural inventory.

### P2 (Fix When Convenient — Hygiene & Polish)
1. **A6** — Resolve in-file contradiction in `models/README.md` (lines 12 vs 20).
2. **A8** — Deprecate `export_minilm_seq32_onnx.py` in `docs/current_architecture.md:130`.
3. **A10** — Remove MiniLM mention from `docs/MODEL_ASSET_DISTRIBUTION.md:9`.
4. **A16** — Archive `scripts/export_minilm_seq32_onnx.py` to `_archive/scripts/`.
5. **A17** — Update metric name in `scripts/run_performance_benchmarks.py:182`.
6. **C5** — Add `SPILLOVER_*` configuration variables to `.env.example`.
7. **C6** — Add `PROMETHEUS_*` and `ANOMALY_*` configuration variables to `.env.example`.
8. **C7** — Add `DLQ_*` and `S3_QUARANTINE_*` configuration variables to `.env.example`.
9. **C8** — Add `MODEL_MANIFEST_PATH` to `.env.example`.
10. **C9** — Add `ORT_*` and `NER_*` inference provider tuning variables to `.env.example`.
11. **E4** — Document automated workflow cron triggers in `docs/OPERATIONS.md`.
12. **F3** — Clarify local filesystem archive fallback vs external S3/MinIO cloud lakehouse.
13. **G3** — Reconcile public repository status with proprietary license notices.

### P3 (Optional / Historical Notes)
1. **A15** — Retain `config/config.yaml:164` MiniLM line as historical version history note.
2. **C10** — Annotate unused template variables in `.env.example`.
3. **D4** — Document independent schema versioning conventions between feature flags and models manifest.
