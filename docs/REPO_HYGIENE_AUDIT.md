# FinText Alpha Vectorizer — Repository Hygiene & SSoT Audit Report

**Document Version**: `1.0.0`  
**Classification**: Institutional Repository Hygiene & Storage Classification Ledger  
**Author**: Principal Software Auditor & Site Reliability Engineer  
**Date of Audit**: September 25, 2026  
**Repository**: `Venkatesh-Akula-417/Fintext-Alpha` (`main`)  
**Audit Standard**: Single Source of Truth (SSoT) & Zero Waste Integrity

---

## 1. Executive Summary & Audit Methodology

This document establishes the definitive reconciliation between local filesystem artifacts and Git version control for the FinText Alpha Vectorizer platform. Institutional hedge fund auditors and quant clients require complete visibility into repository structure, ensuring that:
1. **Source Code vs. Generated Artifacts**: No compiled binaries, large machine learning weights, or volatile caches pollute the version control tree.
2. **Zero Waste & Duplication**: Obsolete scripts, orphaned audit drafts, and unversioned test harnesses are cleanly eliminated or archived.
3. **Cryptographic Release Channel**: Machine learning model weights (>100 MB) are systematically decoupled from Git and hosted via immutable, checksummed GitHub Releases.

### Audit Methodology Commands
```bash
# 1. Full tracked file inventory
git ls-files | sort

# 2. Ignored artifact classification
git status --ignored --short

# 3. Explicit rule attribution
git check-ignore -v <path>

# 4. Binary size audit
Get-ChildItem -Recurse | Where-Object { $_.Length -gt 10MB }
```

---

## 2. Master Item Classification & Decision Matrix

| Item Path | Type | Git Status | Classification | Audit Decision | Technical Rationale & Action |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **`load_test.js` (root)** | File (6.8 KB) | Tracked $\rightarrow$ **Deleted** | **WASTE** | **DELETE** | Obsolete pre-v1 script testing deprecated unversioned routes and `/backtest`. Modern k6 suite lives at `scripts/load_test.js`. Executed `git rm load_test.js`. |
| **`FULL_PROJECT_AUDIT.md`** | File (124 KB) | Tracked $\rightarrow$ **Moved** | Historical Doc | **MOVE** | Comprehensive 1,610-line baseline architecture audit from project inception. Moved to `docs/archive/FULL_PROJECT_AUDIT.md` via `git mv`. |
| **`models_release_v1/*.zip`** | Files (470 MB) | Ignored | Binary Assets | **RELEASE-ASSET** | `finbert-finetuned-v1.0.0.zip` (88.7 MB) and `ner-v1.0.0.zip` (380.4 MB). Exceeds GitHub 100 MB limit. Distributed via GitHub Releases tag `model-assets-v1.0.0`. Ignored via `.gitignore:34:**/*.zip`. |
| **`models_release_v1/SHA256SUMS.txt`** | File (184 B) | Untracked $\rightarrow$ **Tracked** | SSoT Checksum | **KEEP-TRACKED** | Cryptographic hash ledger for all released models. Whitelisted in `.gitignore` via `!models_release_v1/SHA256SUMS.txt`. |
| **`onnxruntime.dll`** | File (18 MB) | Ignored | Binary Library | **KEEP-IGNORED** | Windows native dynamic library for local debugging and offline development. Excluded from Git via `**/*.dll`. Linux containers resolve ORT via standard wheel packages. |
| **`requirements.txt`** | File (905 B) | Tracked | SSoT Config | **KEEP-TRACKED** | Python dependencies for primary integration test execution (`ci.yml`, `security-scan.yml`). |
| **`requirements-finetune.txt`** | File (749 B) | Tracked | ML Config | **KEEP-TRACKED** | Training, domain adaptation, and ONNX quantization dependencies (`security-scan.yml`). |
| **`requirements-model-validation.txt`** | File (79 B) | Tracked | CI Pipeline | **KEEP-TRACKED** | Lightweight dependencies for automated CI model drift and validation workflows (`model-validation.yml`, `model-drift.yml`). |
| **`requirements-validation.txt`** | File (37 B) | Tracked | CI Pipeline | **KEEP-TRACKED** | Lightweight database drivers (`psycopg`) and test runner for point-in-time certification (`pit-validation.yml`). |
| **`models/`** | Directory | Tracked (Partial) | ML Configs | **KEEP-TRACKED** | Contains tokenizers (`tokenizer.json`), architecture specs (`config.json`), and metrics. All binary `.onnx` weights are ignored via `**/*.onnx`. |
| **`data/`** | Directory | Ignored (Partial) | Runtime Storage | **KEEP-IGNORED** | Ingestion queues, quarantine folders, and temporary files. Excluded via `**/data/`. Only static test baseline `data/model-drift/baseline.json` is tracked. |
| **`logs/`** | Directory | Ignored (Whitelisted) | Compliance Ledgers | **WHITELISTED** | Volatile runtime logs are ignored (`logs/*`). Cryptographically certified audit reports (`load_test_report.json`, `billing_flow_report.json`, `soak_report.json`) are whitelisted. |
| **`tests/`** | Directory | Tracked | Test Suite | **KEEP-TRACKED** | Houses automated backend model drift alerting tests (`tests/test_model_drift_alerts.py`). |
| **`python_sdk/tests/`** | Directory | Tracked | SDK Tests | **KEEP-TRACKED** | Houses 11 comprehensive client SDK unit and integration test modules. |
| **`.pytest_cache/`** | Directory | Ignored | Python Cache | **KEEP-IGNORED** | Local test executor cache. Excluded via `**/.pytest_cache/`. |
| **`scratch/`** | Directory | Ignored | Local Scratch | **KEEP-IGNORED** | Ephemeral agent scripts and scratch files. Excluded via `scratch/`. |
| **`tools/`** | Directory | Ignored | Local Tools | **KEEP-IGNORED** | Local developer toolchain scripts. Excluded via `tools/`. |
| **`_archive/`** | Directory | Tracked | Historical Archive | **KEEP-TRACKED** | Preserved historical codebase components. Explicitly allowlisted in `.gitleaks.toml`. |
| **`infra/`** | Directory | Tracked | Infrastructure | **KEEP-TRACKED** | Terraform and cloud provisioning scripts. |
| **`k8s/`** | Directory | Tracked | Orchestration | **KEEP-TRACKED** | Production Kubernetes manifests (CronJobs, alerts, deployments). |
| **`dashboards/`** | Directory | Tracked | Observability | **KEEP-TRACKED** | Grafana dashboard JSON specifications (API performance, Billing, Status). |
| **`postman/`** | Directory | Tracked | API Specification | **KEEP-TRACKED** | 32-core Postman Collection v2.1.0 and Newman CI runner guide. |
| **`notebooks/`** | Directory | Tracked | Research Artifacts | **KEEP-TRACKED** | 4 production research notebooks (Signal quality, PIT backtesting, Model training). |
| **`.env`** | File (105 B) | Ignored | Local Config | **KEEP-IGNORED** | Local environment overrides. Excluded via `.env`. |
| **`.env.example`** | File (12.3 KB) | Tracked | SSoT Template | **KEEP-TRACKED** | Fully commented environment configuration template with sanitized placeholders. |
| **`Dockerfile`** | File (5.4 KB) | Tracked | Container Engine | **KEEP-TRACKED** | Multi-stage production container build definition for Rust binaries. |
| **`docker-compose.yml`** | File (8.7 KB) | Tracked | Local Stack | **KEEP-TRACKED** | 4-core microservices configuration (TimescaleDB, QuestDB, Kafka, Ingestion, API Gateway). |
| **`rust-toolchain.toml`** | File (86 B) | Tracked | Compiler Config | **KEEP-TRACKED** | Pins Rust compiler toolchain to stable release. |
| **`.dockerignore`** | File (954 B) | Tracked | Build Config | **KEEP-TRACKED** | Excludes target directories and local assets from container build contexts. |
| **`.pre-commit-config.yaml`** | File (2.0 KB) | Tracked | Linter Config | **KEEP-TRACKED** | Pre-commit hook definitions for formatting and linting. |
| **`.gitleaks.toml`** | File (2.3 KB) | Tracked | Security Policy | **KEEP-TRACKED** | Global Gitleaks secret detection rules and strict placeholder allowlists. |
| **`README.md`** | File (35 KB) | Tracked | Public Face | **KEEP-TRACKED** | Core repository entry point, architecture overview, and documentation index. |

---

## 3. Waste Elimination & Consolidation Proofs

### Proof A: Root `load_test.js` Removal
```bash
# Verified obsolete content before removal:
# Contains calls to deprecated unversioned /backtest route:
#   const res = http.post(`${BASE_URL}/backtest`, payload, { headers: authHeaders });
# Replacement: scripts/load_test.js (certified k6 runner testing /v1/health, /v1/sentiment)
git rm load_test.js
# Output: rm 'load_test.js'
```

### Proof B: `FULL_PROJECT_AUDIT.md` Archive Relocation
```bash
# Relocated root audit to historical documentation archive:
git mv FULL_PROJECT_AUDIT.md docs/archive/FULL_PROJECT_AUDIT.md
# Output: renamed: FULL_PROJECT_AUDIT.md -> docs/archive/FULL_PROJECT_AUDIT.md
```

### Proof C: Requirements Files Differentiation
- `requirements.txt`: 22 runtime and test packages used by GitHub Actions `ci.yml`.
- `requirements-finetune.txt`: PyTorch, Transformers, and ONNX packages for training.
- `requirements-model-validation.txt`: Standalone ONNX runtime, Tokenizers, and NumPy for fast CI drift checks (`model-validation.yml`).
- `requirements-validation.txt`: Standalone Psycopg and PyTest for database validation (`pit-validation.yml`).

---

## 4. Reorganized `.gitignore` Architecture

The root `.gitignore` has been regrouped into 9 logical sections with comprehensive explanatory rationale:
1. `Executables, Compiled Libraries & Model Assets` (excl. `.exe`, `.dll`, `.onnx`, `.bin`, `.pt`, `models_release_v1/*`, whitelisting `!models_release_v1/SHA256SUMS.txt`).
2. `Build Targets & Toolchain Artifacts` (`**/target/`, `build/`, `dist/`).
3. `Heavy Data Formats & Compressed Archives` (`**/*.parquet`, `**/*.csv`, `**/*.zip`, `**/*.jsonl`, whitelisting `!config/sector_mapping.csv`).
4. `Generated Runtime Data & Storage Directories` (`**/data/`, preserving `!rust/ingestion_engine/src/data/`).
5. `Certified Audit Reports Allowlist` (`**/logs/*`, whitelisting certified compliance reports).
6. `Local Scratch, Tools & Historical Archive` (`scratch/`, `tools/`, `_archive/`).
7. `Secrets, Environment Variables & Virtual Environments` (`.env`, `venv/`).
8. `Python & IDE Caches` (`.pytest_cache/`, `.ipynb_checkpoints/`, `__pycache__/`).
9. `Ephemeral Local Server Logs & Development Task Artifacts`.

---

## 5. Clean Repository Root Structure

```text
FinText-Alpha-Vectorizer/
├── .dockerignore
├── .env.example
├── .gitignore
├── .gitleaks.toml
├── .pre-commit-config.yaml
├── Dockerfile
├── README.md
├── docker-compose.yml
├── rust-toolchain.toml
├── requirements.txt
├── requirements-finetune.txt
├── requirements-model-validation.txt
├── requirements-validation.txt
├── .github/              # CI/CD Workflows (5 automated pipelines)
├── config/               # Database migrations, seed mappings, models manifest
├── dashboards/           # Grafana production dashboards (Performance, Billing, Status)
├── docs/                 # Institutional documentation, runbooks, specifications
│   ├── archive/          # Historical architecture reports (FULL_PROJECT_AUDIT.md)
│   ├── BILLING_RUNBOOK.md
│   ├── MODEL_ASSETS.md
│   ├── MODEL_ASSETS_RELEASE_NOTES.md
│   └── REPO_HYGIENE_AUDIT.md
├── infra/                # Terraform cloud provisioning
├── k8s/                  # Kubernetes manifests (Observability, Backup, Billing)
├── logs/                 # Certified compliance ledgers & test reports
├── models/               # Tokenizer JSONs, config specs (weights on Releases)
├── models_release_v1/    # Checksum ledger (SHA256SUMS.txt)
├── notebooks/            # 4 institutional research & validation notebooks
├── postman/              # 32-request Postman Collection v2.1 & Newman guide
├── python_sdk/           # Official Python client SDK & tests
├── rust/                 # 100% Native Rust Workspace (api_server, ingestion_engine, ticker_extractor)
├── scripts/              # Operational test harnesses, reconciliation, provisioning
└── tests/                # Automated backend model drift alerting tests
```
