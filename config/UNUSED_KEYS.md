# Unused & Orphaned Configuration Keys Review Report

**Generated Date**: 2026-09-10  
**Target File**: `config/config.yaml`  
**Purpose**: Comprehensive audit of configuration keys defined in `config/config.yaml` that currently have zero direct symbol references in active native Rust (`rust/`) source code.

> [!NOTE]
> In accordance with Phase 2 safety rules, these keys have **not** been removed from `config/config.yaml` to prevent runtime configuration failures in environments relying on fallback defaults. They are documented below for architectural review prior to eventual removal in Phase 3.

---

## Executive Summary

- **Total Key Paths Analyzed**: 272
- **Active Referenced Keys**: 204
- **Orphaned / Unreferenced Key Candidates**: 68
- **Referenced via Environment Overrides**: 1 (`backups`)

---

## Categorized Orphaned Configuration Keys

### 1. QuestDB & Buffer Ingestion Settings
These parameters represent legacy or prospective QuestDB engine configuration parameters not currently queried by `rust/api_server` or `rust/ingestion_engine`:
- `questdb.ilp_port`: Port for Influx Line Protocol (code derives connection endpoint via `questdb.url` or `QUESTDB_URL`).
- `questdb.pgwire_port`: PostgreSQL wire protocol port (not directly referenced; queries use REST/ILP clients).
- `questdb.triple_timestamp_tracking`: Proposed triple-timestamp audit telemetry flag.
- `questdb_buffer.retry_max_attempts`: Buffer flush retry limit (code uses exponential backoff duration limits).

### 2. Streaming WebSocket Flags
- `streaming.enable_finnhub_websocket`: WebSocket toggle (source configuration directly controls connection lifecycle).
- `streaming.enable_polygon_websocket`: WebSocket toggle for Polygon trade stream.
- `streaming.finnhub_ws_url`: Endpoint override (code defaults to standard Finnhub WS URL).
- `streaming.polygon_ws_url`: Endpoint override for Polygon WS.

### 3. Quantitative Alpha Weightings & Decay
- `source_weights`: Top-level dictionary for source credibility weights.
  - `source_weights.FOMC`: 1.5
  - `source_weights.Financial_Times`: 1.2
  - `source_weights.Google_News_Finance`: 1.0
  - `source_weights.investing`: 0.9
  - `source_weights.Google_Trends`: 0.7
- `sentiment_velocity`: Velocity parameter block for time-weighted alpha score acceleration.

### 4. Service Timeouts & Token Expiry
- `api_server.timeout_seconds`: Server-wide timeout (Axum router uses per-route middleware timeouts).
- `api_server.auth.token_expiry_seconds`: JWT expiration default (JWT generator uses hardcoded 24h/86400s standard).

### 5. Ingestion Engine Pipeline Limits
- `ingestion_engine.poll_interval_sec`: Polling loop interval.
- `ingestion_engine.tradable_sla_ms`: Millisecond SLA threshold for tradable signals.
- `ingestion_engine.sentiment_chunking_enabled`: Chunking toggle (FinBERT static-shape pipeline handles chunking automatically).
- `ingestion_engine.sentiment_max_tokens`: Token window limit.
- `ingestion_engine.sentiment_chunk_overlap_tokens`: Overlap token count.

### 6. Legacy Sentiment Fallback Configuration
- `sentiment_config.fallback_model_path`: Model path under `sentiment_config` block (code queries `models.finbert` or `models.minilm_seq32`).
- `sentiment_config.fallback_tokenizer_path`: Tokenizer fallback path.
- `sentiment_config.max_seq_len`: Max sequence length.
- `sentiment_config.overlap_tokens`: Chunk overlap token count.
- `sentiment_model.model_dir`: Directory path under legacy `sentiment_model` key.
- `sentiment_model.fallback_model_dir`: Fallback directory under legacy key.
- `sentiment_model.max_sequence_length`: Sequence length under legacy key.
- `sentiment_model.target_f1_score`: Target F1 score metric assertion (0.85).

### 7. Data Source Compliance Notes & Sub-Sources
- `sources.sec_edgar.redistribution_note`: Informational licensing string.
- `sources.finnhub.redistribution_note`: Informational licensing string.
- `sources.polygon.redistribution_note`: Informational licensing string.
- `sources.stock_prices`: Sub-source dictionary for equity price polling.
  - `sources.stock_prices.poll_interval_seconds`: 60
  - `sources.stock_prices.redistribution_note`: Licensing text.

### 8. SLA & Compliance Metric Settings
- `sla.default_target_ms`: Global latency SLA target.
- `sla.compliance_threshold_pct`: Compliance percentage goal (99.9%).
- `sla.latency_metrics_enabled`: Toggle for SLA metric emission.

### 9. Alerting Thresholds
- `alerting.p95_latency_threshold_sec`: Latency alert trigger.
- `alerting.server_error_rate_threshold_pct`: 5xx error percentage threshold.
- `alerting.questdb_min_throughput_5m`: Min throughput threshold.
- `alerting.kafka_max_consumer_lag`: Max consumer lag threshold.
- `alerting.disk_utilization_threshold_pct`: Disk threshold (85%).
- `alerting.memory_utilization_threshold_pct`: RAM threshold (85%).
- `alerting.slack_channel`: Target webhook notification channel.

### 10. Enterprise Feature Toggles & Model Validation
- `enterprise_features.enable_provider_health`: Health monitoring toggle.
- `enterprise_features.enable_model_validation`: Validation pipeline toggle.
- `model_validation.calibration_bins`: Expected calibration error bin count (10).
- `model_validation.min_accuracy_threshold`: Accuracy gate (0.80).
- `model_validation.min_macro_f1_threshold`: F1 gate (0.75).
- `model_validation.max_ece_threshold`: Max ECE threshold (0.15).
- `billing.webhook.secret_env`: Webhook env var name pointer.

### 11. Cache TTL & Capacity Configurations
- `cache.default_ttl_seconds`: General cache TTL.
- `cache.cleanup_interval_seconds`: Background cache eviction interval.
- `cache.quota_cache_ttl_seconds`: Billing quota cache TTL.
- `cache.quota_cache_max_capacity`: Max quota cache items.
- `cache.rate_limit_bucket_ttl_seconds`: Token bucket TTL.
- `cache.rate_limit_max_buckets`: Max rate-limit buckets.
- `cache.provider_health_ttl_seconds`: Provider health status cache TTL.
- `cache.provider_health_max_capacity`: Provider health cache item limit.
- `cache.scd2_cache_ttl_seconds`: Point-in-time SCD2 cache TTL.
- `cache.scd2_cache_max_capacity`: SCD2 cache item limit.

---

## Recommendations for Phase 3
1. **Prune Legacy Blocks**: Remove `source_weights`, `sentiment_velocity`, and `sentiment_model` blocks as the native Rust pipeline uses hardcoded optimized weights.
2. **Consolidate Cache Settings**: Unify the 10 separate `cache.*` entries into a standard `cache.ttl` and `cache.capacity` specification.
3. **Keep Licensing Notes**: Keep `redistribution_note` fields as non-functional compliance documentation within the YAML file.
