# FinText Alpha Vectorizer — AI Audit-Ready System Summary

> **Last Verified**: 2026-09-10 (Suite #96)  
> **Documentation Lineage & Deprecations**: See [DEPRECATED.md](file:///d:/FinText-Alpha-Vectorizer/docs/DEPRECATED.md)

## 1. System Purpose
- **Mission**: High-throughput quantitative signal engine vectorizing financial text, earnings call audio, and market microstructure into real-time tradable alpha.
- **Target Users**: Quantitative hedge funds, StatArb desks, and institutional risk engines.
- **Key Capability**: Sub-millisecond NLP sentiment scoring, vocal stress DSP, GNN shock propagation, and point-in-time backtesting with zero look-ahead bias.

## 2. Technology Stack
- **Core Language**: 100% Native Rust (Cargo workspace with 10 crates, Rust 1.80.1 / MSRV 1.80).
- **ML / Inference**: Native ONNX Runtime (`ort` 2.0) with TensorRT, CUDA, and CPU execution tiers. Primary model: FinBERT v3.1.0 INT8 fine-tuned (with ProsusAI FinBERT v3.0.0 base and MiniLM v2.1.0 fallback).
- **Audio & DSP**: `whisper-rs` (Whisper.cpp ASR) + `rustfft` normalized autocorrelation DSP.
- **Graph & Microstructure**: `nalgebra` for GCN matrix algebra + Black-Scholes analytical gamma math.
- **Multi-Tiered Storage**:
  - *Primary Analytical Store*: PostgreSQL 16 + TimescaleDB hypertables.
  - *Hot Time-Series Path*: QuestDB via Influx Line Protocol (ILP) with nanosecond designated timestamps.
  - *Cold Historical Archive*: S3 / MinIO Parquet with lifecycle compaction.
- **Real-Time Distribution**: Kafka / Redpanda pub/sub + Axum WebSocket broadcast.
- **API Gateway**: Axum 0.7 multi-threaded HTTP REST server (/v1 public surface, 32 core endpoints).
- **Reliability & Anomaly Sentinel**: Kafka DLQ auto-reprocessor + statistical latency autoencoder proxy.

## 3. Key Quantitative & Reliability Modules
- **FinBERT Sentiment**: Fine-tuned ProsusAI FinBERT INT8 dynamic/static shape transformer classification ($<1.8\text{ ms}$).
- **Named Entity Recognition (NER)**: Static shape `[1, 128]` token classification extracting `ORG`, `LOC`, `MISC`.
- **Acoustic Vocal Stress**: Autocorrelation F0 pitch tracking, RMS energy variance, silence pause ratio, and speech burst rate.
- **Supply Chain GNN**: 2-layer Graph Convolutional Network with symmetric normalized Laplacian $\hat{A} = \tilde{D}^{-1/2} \tilde{A} \tilde{D}^{-1/2}$ and spatial distance decay.
- **VPIN & Dealer GEX**: Volume-Synchronized Probability of Informed Trading (VPIN) tick rule bucketing and 1% delta-neutral dollar Gamma Exposure.
- **Cross-Asset Spillover Engine**: Multi-lag Pearson cross-correlation scan ($\tau \in [-24\text{h}, +24\text{h}]$) over 30-day rolling windows.
- **DLQ Reprocessing & Quarantine**: Exponential backoff ($100\text{ms} \rightarrow 5000\text{ms}$) with automated S3 / local disk quarantine isolation.
- **AI Latency Anomaly Detection**: Prometheus-scraped rolling window Z-score ($Z \ge 3.0\sigma$) latency anomaly detector.

## 4. Performance & SLA Benchmarks
- **Total Pipeline Latency**: $<4.5\text{ ms}$ (Tradable SLA threshold: $437\text{ ms}$).
- **Point Lookup API**: $<1.5\text{ ms}$ via analytical storage lookup.
- **Throughput**: $>10,000$ documents/sec across distributed worker instances.
- **Memory Footprint**: Strict hardware bounds ($<2.0\text{ GB}$ total RAM).

## 5. How to Query the System
- **Health Check**: `GET /health` -> `{"status":"ok","version":"2.0.0-institutional"}`
- **Latest Sentiment & Microstructure**: `GET /v1/sentiment?ticker=NVDA`
- **Cross-Asset Spillovers**: `GET /v1/spillovers?ticker=AAPL&limit=5`
- **Point-in-Time Backtesting**: `POST /v1/backtest` with JSON payload `{"ticker":"AAPL","start_date":"2025-01-01","end_date":"2025-03-31","long_threshold":0.2,"short_threshold":-0.2,"holding_days":5,"initial_capital":1000000.0}`
- **Live WebSocket Stream**: Connect to `ws://localhost:8000/ws` for real-time JSON event broadcasts.

## 6. Verification & Test Certification
- **Rust Unit Tests**: `cargo test --workspace --manifest-path rust/Cargo.toml` (**734 tests passing**).
- **Python SDK Tests**: `pytest python_sdk/tests/ -v` (**308 tests passing**).
- **Master Test Certification Runner**: `python scripts/run_all_tests.py` (**96 Master Certification Suites passing**).
