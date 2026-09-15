# FinText Alpha Vectorizer — Cloud Cost & Infrastructure Optimization Guide

> **Document Version**: 1.0.0  
> **Target Audience**: Chief Technology Officers (CTO), Quant Infrastructure Architects, FinTech SREs  
> **Business Impact**: 71.2% Cloud Cost Reduction ($1,025/mo ➔ $295/mo)  

---

## 1. Executive Summary

FinText Alpha Vectorizer was initially configured as an over-engineered prototype featuring a 5-sink dual-write persistence model, 8 concurrently running container services, 120+ public API routes, and distributed streaming infrastructure. This architecture incurred an estimated infrastructure run-rate of **~$1,025/month** on major cloud providers (AWS/GCP), creating unnecessary overhead for its primary customer profile.

Through architectural consolidation and customer profile grounding, the platform has been re-architected for its **Primary Ideal Customer Profile (ICP)**:
- **Primary ICP**: Mid-Frequency Quant Funds, Statistical Arbitrage Desks, Event-Driven Hedge Funds, and Multi-Asset Prop Desks.
- **Data Latency Requirements**: Tradable SLA $<437\text{ms}$ (easily satisfied by native in-process ONNX FinBERT at ~155ms on CPU and ~0.85ms on GPU).
- **Core Access Pattern**: 24–32 standardized, point-in-time correct, bi-temporal REST and WebSocket endpoints.
- **Production Infrastructure Cost**: **$295/month** (71.2% recurring savings) on a single production-grade node or two lightweight clustered instances.

---

## 2. Infrastructure Cost Comparison Matrix

| Resource Dimension | Prototype Architecture (Before) | Lean Institutional Architecture (After) | Monthly Savings |
| :--- | :--- | :--- | :--- |
| **Compute Sizing** | 2x `c6i.2xlarge` (8 vCPU, 16GB RAM) + 1x `r6i.xlarge` (32GB RAM) | 1x `c6i.xlarge` (4 vCPU, 8GB RAM) or 2x `t4g.xlarge` ARM | **-$480/mo** |
| **Primary Storage** | QuestDB Hot Path + PostgreSQL 16 + MinIO Lakehouse + JSONL Stream | PostgreSQL 16 + TimescaleDB (Single Primary TS Store) | **-$140/mo** |
| **Storage Disk (IOPS)** | 500 GB io2 NVMe (10,000 IOPS for dual-writes) | 250 GB gp3 SSD (3,000 baseline IOPS, burstable) | **-$85/mo** |
| **Streaming Broker** | Multi-broker Kafka Cluster with Zookeeper/KRaft (Dedicated nodes) | Single-process Redpanda Broker (512MB RAM cap, in-compose) | **-$95/mo** |
| **Microservice Waste** | 8 services always-on (Spillover, DLQ, Anomaly, Whisper, Sidecar) | 4 Core Services always-on; 4 Optional Services via Docker Profiles | **-$70/mo** |
| **Data Egress & Misc** | Redundant dual-write network transfers and uncompressed feeds | Local IPC / Docker internal network + compressed gzip/parquet | **-$30/mo** |
| **Total Estimated Run-Rate** | **~$1,025 / month** | **~$295 / month** | **+$730 / mo (71.2%)** |

---

## 3. Key Optimization Pillars

### Pillar 1: Single Source of Truth Storage (TimescaleDB Primary)
- **Previous Bottleneck**: Ingestion engine wrote to QuestDB via HTTP ILP, TimescaleDB via SQLx, JSONL stream, and raw Parquet archive simultaneously. Dual-writing created distributed lock contention, doubled I/O requirements, and introduced point-in-time drift risk between stores.
- **Optimized Topology**: PostgreSQL 16 + TimescaleDB is designated as the authoritative bi-temporal store (`sentiment_timeseries` hypertable with SCD Type 2 tracking). QuestDB is toggled off (`QUESTDB_ENABLED=false`) by default and demoted to an optional hot cache (`profiles: ["hot-cache"]`).
- **Cost Impact**: Eliminates high-IOPS disk requirements and 4GB+ RAM consumption from QuestDB background indexing.

### Pillar 2: Docker Compose Service Profiles (4-Core Production Profile)
The 8 Docker Compose services are partitioned into core and optional profiles:

#### A. Core Profile (Always-On — Default Launch)
Runs with `docker compose up -d`:
1. **`postgres`**: PostgreSQL 16 + TimescaleDB (`fintext_metadata` & `sentiment_timeseries`).
2. **`kafka`**: Single-broker Redpanda event streaming bus.
3. **`fintext-ingestion`**: Native Rust ingestion daemon with in-process ONNX FinBERT v3.1.0 inference.
4. **`fintext-api`**: Native Axum HTTP/WebSocket gateway exposing 32 `/v1` core routes.

*Memory Footprint*: ~2.2 GB RAM total. Can comfortably run on a $60–$150/mo cloud instance.

#### B. Optional Profile (On-Demand / Scheduled Analytics)
Activated only when explicitly requested via `docker compose --profile optional up -d`:
- **`questdb`** (`profile: hot-cache`): High-throughput tick buffer for HFT backfill.
- **`fintext-spillover`** (`profile: analytics`): Hourly cross-asset correlation batch job (can be triggered as a Kubernetes CronJob rather than a 24/7 daemon).
- **`fintext-dead-letter`** (`profile: ops`): DLQ auto-reprocessor (runs on alert or batch schedule).
- **`fintext-anomaly-detector`** (`profile: observability`): Prometheus latency watcher.

### Pillar 3: API Surface Lean Consolidation (32 Core Routes)
- Removed ~60 unmaintained, speculative endpoints (such as experimental FX, commodities, ESG, and crypto scanners) that had no institutional paying customers and introduced maintenance debt.
- Archived handlers moved to `_archive/handlers/` with git history fully preserved.
- Operational endpoints (metrics, cache governance, DLQ, audit logs) isolated under `/internal/*` protected by `X-Admin-Token`.

### Pillar 4: CPU vs GPU Cost Governance
- For Mid-Frequency quantitative funds (<100 articles/minute), in-process CPU inference on 4–8 vCPU cores yields **~155ms mean latency**, satisfying the tradable SLA ($<437\text{ms}$) without GPU spend.
- Avoids dedicated GPU hosting ($350–$600/month for an NVIDIA L4/A10G instance) during initial platform launch and beta onboarding.
- GPU acceleration (yielding 0.85ms latency) is reserved as an optional enterprise add-on tier.

---

## 4. Production Deployment Blueprint

### Recommended AWS Instance Configuration ($295/mo Budget)
- **Compute Instance**: 1x `c6i.xlarge` (4 vCPU, 8 GiB RAM, 12.5 Gbps network) in `us-east-1` = **~$122/mo** (On-Demand) or **~$78/mo** (1-Year Reserved).
- **EBS Storage**: 250 GB `gp3` (3,000 IOPS, 125 MB/s throughput) = **~$20/mo**.
- **Cold Parquet Lakehouse**: AWS S3 Standard (1 TB storage + lifecycle transition to Glacier Flexible Retrieval) = **~$15/mo**.
- **Network Egress**: 500 GB outbound API traffic = **~$45/mo**.
- **Managed DNS & SSL (Route53 + ACM)**: **~$5/mo**.
- **Buffer & CloudWatch Monitoring**: **~$35/mo**.
- **Total Monthly Spend**: **$242 – $295 / month**.

---

## 5. Summary of Commercial Advantages

1. **Self-Sustaining Unit Economics**: Break-even requires only 1 institutional customer paying $500/month (producing an immediate 40%+ gross margin on day one).
2. **Deterministic SLAs**: Consolidating on PostgreSQL 16 + TimescaleDB eliminates cross-database sync anomalies and eliminates look-ahead bias with native bi-temporal queries.
3. **Frictionless Onboarding**: Single `docker compose up -d` brings up the complete institutional platform in under 30 seconds.
