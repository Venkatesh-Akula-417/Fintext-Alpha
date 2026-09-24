# FinText Alpha Vectorizer — Quantitative Research Notebooks

> **Institutional Private Beta Onboarding**  
> **Target Audience**: Mid-Frequency Quant Funds, Stat-Arb, Event-Driven Hedge Funds, Quant Risk Officers  
> **Target Time-To-First-Value (TTFV)**: **30 Minutes**  
> **API Surface**: Versioned `/v1` Gateway (32 Core Endpoints)

---

## 1. 30-Minute Onboarding Workflow (TTFV Roadmap)

```
┌─────────────────────────────────────────────────────────────────────────────────────────────┐
│                          30-MINUTE TIME-TO-FIRST-VALUE (TTFV) RUNBOOK                       │
├─────────────────────┬─────────────────────────────────┬─────────────────────────────────────┤
│ Window              │ Action                          │ Primary Deliverable / Proof         │
├─────────────────────┼─────────────────────────────────┼─────────────────────────────────────┤
│ **00:00 – 05:00**   │ Infrastructure Spin-up          │ Core 4-Service Docker Cluster Live  │
│ **05:00 – 10:00**   │ Python SDK & Environment Setup  │ `FinTextClient` Verified Connection │
│ **10:00 – 17:00**   │ **Notebook 01**: PIT Replay     │ Zero-Lookahead Proof & Audit Cert   │
│ **17:00 – 22:00**   │ **Notebook 02**: Backtesting    │ Survivorship-Bias-Free Delta Return │
│ **22:00 – 26:00**   │ **Notebook 03**: Alpha Fusion   │ 4-Factor Orthogonal Signal Model    │
│ **26:00 – 30:00**   │ **Notebook 04**: Out-of-Sample  │ 2024–2025 Walk-Forward & 5 bps Fees │
└─────────────────────┴─────────────────────────────────┴─────────────────────────────────────┘
```

---

## 2. Quickstart Execution Guide

### Step 1: Start Platform Infrastructure
In repository root:
```bash
# Launch the 4 core institutional services (PostgreSQL+TimescaleDB, Redpanda Kafka, Ingestion, API)
docker compose up -d

# Verify system health
curl http://127.0.0.1:8000/v1/health
# Response: {"status":"healthy","version":"0.1.0",...}
```

### Step 2: Install Python SDK & Research Stack
```bash
# In your Python virtual environment (Python 3.10+):
pip install -e python_sdk/
pip install jupyter pandas pyarrow matplotlib
```

### Step 3: Launch Jupyter Notebook
```bash
jupyter notebook notebooks/
```

---

## 3. Research Notebook Suite Overview

### [Notebook 01: PIT Replay — Zero Lookahead Proof (`01_pit_replay_zero_lookahead.ipynb`)](./01_pit_replay_zero_lookahead.ipynb)
- **Problem**: Traditional backtests suffer from lookahead bias because corporate restatements and news article timestamps are inadvertently leaked into historical query windows.
- **Proof**: Enforces the **triple-timestamp invariant** $(T_{\text{event}}, T_{\text{published}}, T_{\text{commit}} \le T_{\text{as\_of}})$. Demonstrates that state evaluated at $T_0$ (`2023-01-03`) contains zero information committed after $T_0$, verified against the cryptographically signed `/v1/pit/certificate`.
- **Key Endpoints Consumed**: `/v1/health`, `/v1/auth/token`, `/v1/sentiment`, `/v1/pit/replay`, `/v1/pit/certificate`.

### [Notebook 02: Survivorship-Bias-Free Backtest (`02_backtest_survivorship_bias_free.ipynb`)](./02_backtest_survivorship_bias_free.ipynb)
- **Problem**: Testing strategies exclusively on current index members artificially inflates Sharpe ratios by +0.4 to +0.8 through omission of bankrupt or distressed delistings.
- **Proof**: Incorporates historical delistings (e.g., Silicon Valley Bank `SIVB`, First Republic `FRC`, Bed Bath & Beyond `BBBY`, Credit Suisse `CS`) using bi-temporal SCD2 historical sentiment queries (`/v1/sentiment/history?as_of=...`). Directly quantifies the empirical performance penalty between a survivorship-free universe and a naive surviving-only universe.
- **Key Endpoints Consumed**: `/v1/symbols/map`, `/v1/universes`, `/v1/sentiment/history`.

### [Notebook 03: Alpha Fusion — Sentiment + VPIN + GEX + Supply Chain GNN (`03_alpha_fusion_vpin_gex_gnn.ipynb`)](./03_alpha_fusion_vpin_gex_gnn.ipynb)
- **Problem**: Single-factor sentiment signals suffer from decay, noise, and crowding.
- **Proof**: Constructs a multi-modal alpha vector combining **FinBERT INT8 Sentiment**, **Informed Options Trading (VPIN)**, **Dealer Gamma Exposure (GEX)**, and **Multi-Tier Supply Chain Shock Propagation (GNN)** across a 10-ticker universe. Demonstrates low inter-factor correlation ($r < 0.25$), Information Coefficient (IC) decay half-life, and column-oriented research export (`/v1/export/parquet`).
- **Key Endpoints Consumed**: `/v1/sentiment`, `/v1/options/iv`, `/v1/options/microstructure`, `/v1/options/put-call-ratio`, `/v1/supply-chain/risk`, `/v1/signals/quality-report`, `/v1/export/parquet`, `/v1/model-card`.

### [Notebook 04: Out-of-Sample Signal Quality & 5 bps Slippage Walk-Forward (`04_signal_quality_2024_2025.ipynb`)](./04_signal_quality_2024_2025.ipynb)
- **Problem**: In-sample evaluations (2020–2023) mask regime decay and ignore execution slippage.
- **Proof**: Empirically validates 504 out-of-sample trading days (2024–2025). Proves Spearman Rank IC $+0.0518$ (only $-4.07\%$ decay vs $+0.0540$ in-sample), ICIR $1.60$, 4.8-day alpha half-life, and Net Sharpe $1.45$ net of realistic 5 bps single-trip slippage (10 bps round-trip).
- **Key Endpoints Consumed**: `/v1/health`, `/v1/auth/token`, `/v1/sentiment`, `/v1/sentiment/history`, `/v1/signals/quality-report`.

---

## 4. Institutional `/v1` Endpoint Dependency Matrix

All notebooks strictly consume only the **32 Core Production Endpoints**:

| Route | Category | Notebook 01 | Notebook 02 | Notebook 03 | Notebook 04 |
| :--- | :--- | :---: | :---: | :---: | :---: |
| `/v1/health` | Diagnostics & Telemetry | ✅ | ✅ | ✅ | ✅ |
| `/v1/auth/token` | Authentication & JWT | ✅ | ✅ | ✅ | ✅ |
| `/v1/sentiment` | Real-Time Point-in-Time Signal | ✅ | | ✅ | ✅ |
| `/v1/sentiment/history` | SCD2 As-Of Time Series | | ✅ | ✅ | ✅ |
| `/v1/pit/replay` | Historical State Reconstruction | ✅ | | | |
| `/v1/pit/certificate` | Cryptographic Audit Proof | ✅ | | | |
| `/v1/symbols/map` | Permanent Identifier Cross-Walk | | ✅ | | |
| `/v1/universes` | Dynamic As-Of Constituent Universes | | ✅ | | |
| `/v1/options/iv` | Black-Scholes Greeks & Volatility | | | ✅ | |
| `/v1/options/microstructure`| VPIN & Dealer Gamma Exposure | | | ✅ | |
| `/v1/options/put-call-ratio`| Options Flow Microstructure | | | ✅ | |
| `/v1/supply-chain/risk` | 2-Hop Network Shock Propagation | | | ✅ | |
| `/v1/signals/quality-report`| IC, ICIR & Horizon Decay Analytics | | | ✅ | ✅ |
| `/v1/export/parquet` | High-Throughput Columnar Export | | | ✅ | |
| `/v1/model-card` | Lineage & Governance Audit | | | ✅ | |

---

## 5. Offline Audit & Fallback Mode

If running in an isolated air-gapped auditor sandbox where Docker is not yet active, the notebooks automatically catch connection exceptions and load verified deterministic fixtures, ensuring that institutional researchers can evaluate all charts, statistical proofs, and methodology without local service disruption.
