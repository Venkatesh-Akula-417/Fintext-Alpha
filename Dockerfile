# ═══════════════════════════════════════════════════════════════════════════════
# FinText-Alpha-Vectorizer — Production Multi-Stage Rust Container Build
# ═══════════════════════════════════════════════════════════════════════════════

# ── Stage 1: Native Rust Compiler ─────────────────────────────────────────────
# Updated to latest stable to support edition2024 and aws-sdk 1.94.1+ (block-buffer 0.12.0, aws-credential-types 1.3.0)
FROM rust:bookworm AS builder


WORKDIR /build

# Install system compilation dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    cmake \
    clang \
    && rm -rf /var/lib/apt/lists/*

# Copy Rust workspace
COPY rust /build/rust

# Build all release binaries
WORKDIR /build/rust
RUN cargo build --release --jobs 2

# ── Stage 2: Runtime Image (Ingestion Daemon) ─────────────────────────────────
FROM debian:bookworm-slim AS runtime-base

WORKDIR /app

# Install runtime shared libraries and CA certificates
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -u 1000 -m -s /bin/bash appuser

# Copy assets and models
COPY models /app/models
COPY config /app/config

# Create data directories with appropriate permissions
RUN mkdir -p /app/data/stream /app/data/quarantine /app/logs \
    && chown -R appuser:appuser /app

# ── Target: Ingestion Engine ──────────────────────────────────────────────────
FROM runtime-base AS ingestion

COPY --from=builder /build/rust/target/release/fintext_ingestion /app/bin/fintext_ingestion
RUN chown appuser:appuser /app/bin/fintext_ingestion && chmod +x /app/bin/fintext_ingestion

USER appuser
ENV QUESTDB_URL="http://questdb:9000" \
    KAFKA_BOOTSTRAP_SERVERS="kafka:9092" \
    KAFKA_TOPIC="sentiment-events" \
    RUST_LOG="info"

CMD ["/app/bin/fintext_ingestion"]

# ── Target: Axum API Gateway ──────────────────────────────────────────────────
FROM runtime-base AS api

COPY --from=builder /build/rust/target/release/fintext_api /app/bin/fintext_api
RUN chown appuser:appuser /app/bin/fintext_api && chmod +x /app/bin/fintext_api

USER appuser
EXPOSE 8000
ENV QUESTDB_URL="http://questdb:9000" \
    PORT="8000" \
    RUST_LOG="info"

HEALTHCHECK --interval=15s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://127.0.0.1:8000/health || exit 1

CMD ["/app/bin/fintext_api"]

# ── Target: Spillover Engine ──────────────────────────────────────────────────
FROM runtime-base AS spillover

COPY --from=builder /build/rust/target/release/fintext_spillover /app/bin/fintext_spillover
RUN chown appuser:appuser /app/bin/fintext_spillover && chmod +x /app/bin/fintext_spillover

USER appuser
ENV QUESTDB_URL="http://questdb:9000" \
    SPILLOVER_INTERVAL_SECS="3600" \
    SPILLOVER_LOOKBACK_DAYS="30" \
    SPILLOVER_MIN_HOURS="10" \
    SPILLOVER_CORR_THRESHOLD="0.3" \
    RUST_LOG="info"

CMD ["/app/bin/fintext_spillover"]

# ── Target: Dead Letter Queue Worker ──────────────────────────────────────────
FROM runtime-base AS dead_letter_worker

COPY --from=builder /build/rust/target/release/dead_letter_worker /app/bin/dead_letter_worker
RUN chown appuser:appuser /app/bin/dead_letter_worker && chmod +x /app/bin/dead_letter_worker

USER appuser
ENV KAFKA_BOOTSTRAP_SERVERS="kafka:9092" \
    DLQ_TOPIC="sentiment-dlq" \
    REPROCESS_TOPIC="sentiment-updates" \
    S3_QUARANTINE_BUCKET="fintext-dlq-quarantine" \
    LOCAL_QUARANTINE_DIR="/app/data/quarantine/" \
    DLQ_MAX_RETRIES="5" \
    DLQ_INITIAL_BACKOFF_MS="100" \
    DLQ_MAX_BACKOFF_MS="5000" \
    DLQ_MOCK_MODE="false" \
    RUST_LOG="info"

CMD ["/app/bin/dead_letter_worker"]

# ── Target: AI Latency Anomaly Detector ───────────────────────────────────────
FROM runtime-base AS anomaly_detector

COPY --from=builder /build/rust/target/release/fintext_anomaly_detector /app/bin/fintext_anomaly_detector
RUN chown appuser:appuser /app/bin/fintext_anomaly_detector && chmod +x /app/bin/fintext_anomaly_detector

USER appuser
ENV PROMETHEUS_URL="http://prometheus:9090" \
    PROMETHEUS_QUERY="sum(rate(http_request_duration_seconds_sum[5m])) / sum(rate(http_request_duration_seconds_count[5m]))" \
    ANOMALY_THRESHOLD_ZSCORE="3.0" \
    ANOMALY_WINDOW_SIZE="30" \
    CHECK_INTERVAL_SECS="60" \
    WEBHOOK_URL="" \
    RUST_LOG="info"

CMD ["/app/bin/fintext_anomaly_detector"]
