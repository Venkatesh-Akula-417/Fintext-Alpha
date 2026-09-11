# Unused Feature Flags Audit Report

**Generated Date**: 2026-09-10  
**Target File**: `config/feature_flags.yaml`  
**Purpose**: Audit of institutional feature flags defined in `config/feature_flags.yaml` against native Rust runtime source code (`rust/`).

---

## Executive Summary

- **Total Feature Flags Defined**: 20
- **Flags with Direct Code Name Matches**: 3 (`sources.sec_edgar`, `sources.finnhub`, `sources.polygon`)
- **Unused / Unreferenced Flags in Rust**: 17
- **Runtime Configuration Reference Status**: `config/feature_flags.yaml` is currently **not loaded at runtime** by `rust/api_server` or `rust/ingestion_engine`. Features are statically compiled and managed via `config/config.yaml` or environment variables.

---

## Unreferenced Feature Flags

### Core Production Engines (`core.*`)
The following flags are intended to gate core computational pipelines, but the pipeline execution paths in `rust/ingestion_engine` and `rust/api_server` are compiled directly into the binary without checking these flag names:

1. `core.sec_edgar_fetcher`
   - *Description*: Real-time SEC EDGAR 8-K, 10-Q, 10-K regulatory filing ingestion.
   - *Runtime Status*: Active via `SecEdgarClient` directly; flag is not queried.

2. `core.onnx_finbert_sentiment`
   - *Description*: Native in-process ONNX Runtime MiniLM-FinBERT static shape sentiment classification.
   - *Runtime Status*: Statically linked in `onnx_sentiment.rs`.

3. `core.onnx_ner_pipeline`
   - *Description*: Native in-process ONNX Runtime Named Entity Recognition (ORG, LOC, MISC).
   - *Runtime Status*: Executed in `ner.rs`.

4. `core.whisper_asr_transcription`
   - *Description*: Native Whisper.cpp Automatic Speech Recognition for earnings call audio.
   - *Runtime Status*: Controlled via `WHISPER_MOCK_FALLBACK` / ONNX binding, not this flag.

5. `core.acoustic_dsp_features`
   - *Description*: Normalized autocorrelation F0 pitch, RMS energy, pause ratio, and speech rate extraction.
   - *Runtime Status*: Statically invoked in audio pipeline.

6. `core.supply_chain_gnn`
   - *Description*: 2-layer symmetric normalized Laplacian Graph Convolutional Network shock propagation.
   - *Runtime Status*: Handled directly in `gnn.rs`.

7. `core.polygon_options_microstructure`
   - *Description*: Real-time options trade ingestion via Polygon.io REST API.
   - *Runtime Status*: Handled via `PolygonClient`.

8. `core.vpin_order_toxicity`
   - *Description*: Volume-Synchronized Probability of Informed Trading (VPIN) bucketing engine.
   - *Runtime Status*: Statically computed in quantitative pipelines.

9. `core.dealer_gamma_exposure_gex`
   - *Description*: Black-Scholes analytical dollar Gamma Exposure (GEX) calculation.
   - *Runtime Status*: Statically computed in quantitative pipelines.

10. `core.cross_asset_spillover_engine`
    - *Description*: Multi-lag Pearson cross-correlation scan for lead-lag risk propagation.
    - *Runtime Status*: Dedicated standalone microservice (`fintext_spillover_engine`).

11. `core.axum_api_gateway`
    - *Description*: Sub-millisecond Axum HTTP REST and WebSocket streaming gateway.
    - *Runtime Status*: Core entry point of `fintext_api_server`.

12. `core.point_in_time_backtest`
    - *Description*: Bi-temporal ASOF point-in-time quantitative backtesting engine.
    - *Runtime Status*: Active endpoint route `/v1/backtest/point-in-time`.

### Streaming & Storage Sinks (`sinks.*`)
13. `sinks.questdb_ilp_sink`
    - *Description*: High-throughput Influx Line Protocol (ILP) time-series hot storage.
    - *Runtime Status*: Configured via `config.yaml` `questdb.url`.

14. `sinks.kafka_realtime_sink`
    - *Description*: Sub-millisecond real-time event distribution for trading desks via Kafka/Redpanda.
    - *Runtime Status*: Configured via `KAFKA_BROKERS` env var and `streaming.kafka_sink`.

15. `sinks.kafka_producer_sink`
    - *Description*: Persistent event streaming to Redpanda/Kafka topics.
    - *Runtime Status*: Configured via `kafka_sink.rs`.

16. `sinks.jsonl_stream_sink`
    - *Description*: Local file stream buffer fallback (`data/stream/processed_signals.jsonl`).
    - *Runtime Status*: Fallback write handler.

### Enterprise Features (`enterprise.*`)
17. `enterprise.fix_protocol_bridge`
    - *Description*: Enterprise FIX 4.4 protocol bridge and order execution engine.
    - *Runtime Status*: Managed in `handlers/fix_orders.rs` and `fix.rs` through route registration; not dynamic flag-gated.

---

## Architectural Recommendation
In Phase 3, decide between:
1. **Dynamic Gating Integration**: Introduce a `FeatureFlagService` in `rust/api_server/src/flags.rs` that loads `config/feature_flags.yaml` and dynamically activates or returns HTTP 403 / 503 for toggled features.
2. **Deprecation**: If compile-time features and environment variables remain standard, deprecate `feature_flags.yaml` and record in `docs/DEPRECATED.md`.
