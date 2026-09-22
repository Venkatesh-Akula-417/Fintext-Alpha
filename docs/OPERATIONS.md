# ══════════════════════════════════════════════════════════════════════════════
# FinText-Alpha-Vectorizer — Operations & Site Reliability Engineering (SRE) Manual
# ══════════════════════════════════════════════════════════════════════════════
# Target SLA: 99.95% Availability | 95%+ Zero-Touch Automated Self-Healing Operations
# ══════════════════════════════════════════════════════════════════════════════

## 1. Zero-Touch Operations Architecture

The FinText Alpha Vectorizer platform is architected for institutional quantitative operations with minimal human intervention. Continuous telemetry, declarative Kubernetes manifests, and automated remediation loops ensure 95%+ zero-touch reliability:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                       FinText Zero-Touch SRE Topology                       │
└─────────────────────────────────────────────────────────────────────────────┘
                                  │
          ┌───────────────────────┴───────────────────────┐
          ▼                                               ▼
┌──────────────────┐                            ┌──────────────────┐
│ Prometheus TSDB  │                            │  Kubernetes      │
│  & Alert Rules   │                            │  CronJobs / S3   │
└─────────┬────────┘                            └─────────┬────────┘
          │ (Alert Trigger)                               │ (Scheduled Backup)
          ▼                                               ▼
┌──────────────────┐                            ┌──────────────────┐
│  Alertmanager    │                            │ PostgreSQL Dump  │
│  Router & Mute   │                            │ & QuestDB Velero │
└─────────┬────────┘                            └──────────────────┘
          │ (Rich Formatted Alert)
          ▼
┌───────────────────────────────────────────────┐
│ Slack (`#fintext-ops-alerts`) / PagerDuty P0  │
└───────────────────────────────────────────────┘
```

---

## 2. Prometheus Alerting Rules Catalog

All rules are defined in [`k8s/observability/prometheus-alerts.yaml`](../k8s/observability/prometheus-alerts.yaml) and automatically loaded into Prometheus.

| Alert Name | Group / Tier | Severity | Threshold Expression | Runbook Focus |
|---|---|---|---|---|
| `APIP95LatencyHigh` | `api / gateway` | **Critical** | `p95(latency) > 500ms` for 2m | HPA scaling, Pod memory pressure, QuestDB query queue |
| `APIServerErrorRateHigh` | `api / gateway` | **Critical** | `5xx_rate / total_rate > 1%` for 2m | DB connection pool exhaustion, unhandled exceptions |
| `QuestDBIngestionLagHigh` | `questdb / storage` | **Critical** | `delta(ingested_total[5m]) < 100` | Ingestion pipeline liveness, API key quota |
| `KafkaRealtimeConsumerLagHigh` | `kafka-realtime / messaging` | **Warning** | `pending_messages > 10,000` for 3m | Consumer worker scaling, partition rebalancing |
| `KafkaConsumerLagHigh` | `kafka / event-bus` | **Warning** | `consumer_lag > 100,000` for 3m | Redpanda partition rebalancing, IOPS limits |
| `PodRestartFrequent` | `k8s / infra` | **Warning** | `restarts_rate[15m] > 0` for 5m | OOMKilled cgroup limits, liveness probe timeouts |
| `DiskSpaceLow` | `storage / volumes` | **Critical** | `disk_used_percent > 80%` for 5m | QuestDB partition drop / historical retention pruning |
| `MemoryPressureHigh` | `k8s / infra` | **Critical** | `memory_used / limit > 90%` for 3m | Leak detection, sliding-window buffer sizing |
| `DatabaseBackupFailed` | `backups / governance` | **Critical** | `backup_status{status="failed"} == 1` | S3 bucket permissions, disk space in `/tmp` |

---

## 3. Provisioning Kubernetes Secrets

### 3.1. Slack & Alertmanager Notifications Secret
Provision the `fintext-alerting` secret with your institutional Slack Incoming Webhook URL:

```bash
kubectl create secret generic fintext-alerting \
  --from-literal=slack-webhook-url="${SLACK_WEBHOOK_URL}" \
  --namespace=default
```

### 3.2. S3 / MinIO Database Backup Credentials Secret
Provision the `fintext-backup-s3-secret` containing AWS S3 or MinIO credentials:

```bash
kubectl create secret generic fintext-backup-s3-secret \
  --from-literal=backup-s3-endpoint="https://s3.us-east-1.amazonaws.com" \
  --from-literal=backup-s3-bucket="fintext-backups-production" \
  --from-literal=aws-access-key-id="AKIAIOSFODNN7EXAMPLE" \
  --from-literal=aws-secret-access-key="wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY" \
  --namespace=default
```

---

## 4. Automated Database Backup Procedures

### 4.1. PostgreSQL Metadata Store Backup (`postgres-backup-cronjob`)
- **Schedule**: Every 6 hours (`0 */6 * * *`)
- **Procedure**:
  1. Streams logical SQL dump via `pg_dump`
  2. Compresses with `gzip -9`
  3. Computes cryptographic `SHA-256` checksum
  4. Uploads both archive and checksum to `s3://${S3_BUCKET}/postgres/fintext_pg_meta_YYYYMMDD_HHMMSSZ.sql.gz`
  5. Automatically purges local and remote backups older than 7 days

#### Manual One-Off Trigger:
```bash
kubectl create job --from=cronjob/postgres-backup-cronjob postgres-manual-backup-$(date +%s)
```

#### Disaster Recovery Restoration:
```bash
# 1. Download backup and checksum from S3
aws s3 cp s3://fintext-backups-production/postgres/fintext_pg_meta_20260902_000000Z.sql.gz ./
aws s3 cp s3://fintext-backups-production/postgres/fintext_pg_meta_20260902_000000Z.sql.gz.sha256 ./

# 2. Verify SHA-256 integrity
sha256sum -c fintext_pg_meta_20260902_000000Z.sql.gz.sha256

# 3. Restore to PostgreSQL
gunzip -c fintext_pg_meta_20260902_000000Z.sql.gz | psql -h postgres-meta -U fintext_admin -d fintext_metadata
```

---

### 4.2. QuestDB Point-in-Time Snapshotting (`questdb-backup-cronjob`)
- **Schedule**: Daily at 02:00 UTC (`0 2 * * *`)
- **Procedure**:
  1. Requests Velero CSI snapshot for QuestDB persistent volume claim (`app.kubernetes.io/name=questdb`)
  2. Enforces 30-day (720h) immutable storage retention

#### Manual Snapshot Trigger:
```bash
velero backup create questdb-manual-$(date +%Y%m%d%H%M%S) \
  --include-namespaces default \
  --selector app.kubernetes.io/name=questdb \
  --snapshot-volumes=true
```

#### Restoring QuestDB from Snapshot:
```bash
velero restore create --from-backup questdb-backup-20260902
```

---

## 5. Simulating & Verifying Alerts

To verify end-to-end Alertmanager delivery to Slack without modifying production code, post a simulated firing alert to the Alertmanager API:

```bash
# Forward Alertmanager port
kubectl port-forward svc/alertmanager 9093:9093 &

# Dispatch test alert payload
curl -X POST "http://localhost:9093/api/v2/alerts" \
  -H "Content-Type: application/json" \
  -d '[{
    "labels": {
      "alertname": "TestZeroTouchAlert",
      "service": "fintext-api",
      "severity": "warning",
      "tier": "gateway"
    },
    "annotations": {
      "summary": "Simulated Alert for Zero-Touch Verification",
      "description": "Validating Slack notifications and Alertmanager routing tree.",
      "runbook_url": "https://docs.fintext.internal/ops/runbooks/test-alert"
    }
  }]'
```

---

## 6. SRE Incident Response Runbooks

### 6.1. `APIP95LatencyHigh` (P95 > 500ms)
1. Check horizontal pod autoscaler status: `kubectl get hpa hpa-api`
2. Inspect pod CPU throttling: `kubectl top pods -l app.kubernetes.io/name=fintext-api`
3. Inspect QuestDB query execution duration: `curl http://questdb:9000/exec?query=select+*+from+query_log`

### 6.2. `APIServerErrorRateHigh` (5xx > 1%)
1. Tail API server error logs: `kubectl logs -l app.kubernetes.io/name=fintext-api --tail=100 | grep ERROR`
2. Check database connection pool metrics: `pg_isready -h postgres-meta -p 5432`

### 6.3. `KafkaRealtimeConsumerLagHigh` (Pending > 10,000)
1. Inspect Kafka real-time consumer lag: `rpk group describe fintext-api-websocket`
2. Scale up ingestion or websocket broker replicas: `kubectl scale deployment/fintext-ingestion --replicas=5`

---

## 7. Production Mode Guard & Anti-Synthetic Data Policy

### 7.1. Architecture & Operational Invariants
In institutional production deployments, **synthetic/mock data generation is strictly prohibited**. All production deployments MUST have `PRODUCTION_MODE=true` (or `production_mode: true` in `config/config.yaml`).

When `PRODUCTION_MODE` is enabled:
1. **Mock Fallback Suppression**: All environment fallback toggles (`QUESTDB_MOCK_FALLBACK`, `KAFKA_MOCK_FALLBACK`, `POLYGON_MOCK_FALLBACK`, `WHISPER_MOCK_FALLBACK`, `NATS_MOCK_MODE`, `FINNHUB_MOCK_MODE`) are unconditionally overridden and forced to `false`.
2. **Explicit 503 Failures**: If any required service (QuestDB, Kafka/Redpanda, PostgreSQL, Polygon.io, Finnhub, Whisper) is unreachable, the API returns HTTP `503 Service Unavailable` with `{"error": "Service Unavailable", "message": "Required data source unavailable in production mode."}` instead of serving synthetic mock records.
3. **Ingestion Engine Refusal**: The ingestion engine halts startup or refuses stream initialization if credentials or live connections are missing, logging a critical error.

### 7.2. Production Deployment Verification
When launching the stack in production:
```bash
# Verify PRODUCTION_MODE is set in deployment environment
kubectl exec -it deployment/fintext-api -n default -- env | grep PRODUCTION_MODE
# Output: PRODUCTION_MODE=true

# Check logs for the Production Guard banner
kubectl logs -l app.kubernetes.io/name=fintext-api --tail=50 | grep -i "production mode guard"
```

---

## 8. Data Source Quality Scoring & Quarantine Operations Runbook

### 8.1. Data Quality Governance Architecture
The FinText platform continuously computes an automated Source Quality Score $Q_s \in [0.0, 1.0]$ for each upstream data provider (SEC EDGAR, Polygon, Finnhub, synthetic feeds) using historical telemetry:

$$Q_s = 0.4 \times \left(\frac{\text{Accepted}}{\text{Total}}\right) + 0.2 \times \text{Freshness} + 0.2 \times \text{Completeness} + 0.2 \times \text{Reliability}$$

- **Freshness Factor**: $1.0$ for latency $\le$ 5 minutes ($300{,}000\text{ ms}$), linearly decaying to $0.5$ at 30 minutes ($1{,}800{,}000\text{ ms}$).
- **Completeness Factor**: Average fraction of required fields present across ingested documents (`id`, `title`, `source`, `url`, `published_utc`, `raw_content`).
- **Reliability Factor**: Baseline provider reputation: SEC EDGAR = $1.0$, Polygon = $0.9$, Finnhub = $0.8$, Mock = $0.5$.

### 8.2. Ingestion Gate Enforcement
Incoming documents pass sequentially through four automated gates:
1. `SCHEMA_VALIDATION`: Rejects malformed records (empty required fields, invalid RFC-3339 timestamps).
2. `BUSINESS_RULES`: Quarantines documents with invalid ticker symbols or publication timestamps in the future ($> \text{now} + 300\text{s}$).
3. `DUPLICATE_DETECTION`: Quarantines documents matching a 64-bit natural key hash `(source_id, ticker, published_utc)` observed within 24 hours.
4. `SOURCE_QUALITY_THRESHOLD`: Quarantines documents if the source composite score falls below the configured threshold (default `0.60`).

### 8.3. Quarantine Directory Structure & Inspection
Quarantined documents are committed atomically to:
```text
data/quarantine/<source>/<YYYY-MM-DD>/<uuid>.json
```

To list and inspect quarantined records:
```bash
# Count quarantined documents by source
ls -lh data/quarantine/*/*/*.json | wc -l

# Inspect quarantine details for a specific record
cat data/quarantine/finnhub/2026-09-06/<uuid>.json | jq .
```

### 8.4. Reprocessing & Recovery Runbook
1. **Root Cause Analysis**: Inspect `rule` and `reason` fields in the quarantine envelope.
2. **Clock Drift Correction**: If quarantined for future timestamps due to server NTP drift, resync host clock:
   ```bash
   sudo chronyd -q 'server pool.ntp.org iburst'
   ```
3. **Automated Reprocessing**: Move validated/remediated JSON files back into the ingestion queue or trigger replay:
   ```bash
   python scripts/reprocess_quarantine.py --source finnhub --date 2026-09-06
   ```

---

## 9. TimescaleDB Migration: Backfill & Production Switchover Runbook (Phase 2)

### 9.1. Architectural Overview
Phase 2 transitions the platform from dual-write testing into active primary query operation with zero downtime.

| Mode | `timescaledb.enabled` | `timescaledb.primary` | Ingestion Write Path | API Server Read Path |
|---|---|---|---|---|
| **Phase 1 (Dual-Write)** | `true` | `false` | QuestDB ILP (Primary) + TimescaleDB (Async secondary) | QuestDB (Primary) with TimescaleDB fallback |
| **Phase 2 (Switchover)** | `true` | `true` | TimescaleDB (Sync Primary) + QuestDB (Async secondary) | TimescaleDB (Primary) with QuestDB fallback |
| **Legacy (Default)** | `false` | `false` | QuestDB ILP only | QuestDB only |

### 9.2. Executing Historical Backfill
Before enabling TimescaleDB as primary, all historical sentiment records from QuestDB must be populated into TimescaleDB.

#### Step 1: Dry-Run Simulation (Zero Risk)
Simulate backfill to verify row counts, network access, and pagination without writing to the database:
```bash
# Run via Rust CLI binary
./rust/target/release/backfill_timescaledb --dry-run --batch-size 5000

# Or via Python companion utility
python scripts/backfill_timescaledb.py --dry-run --batch-size 5000
```

#### Step 2: Date-Bounded Migration
Migrate historical partitions in manageable date windows (e.g., quarterly or yearly):
```bash
./rust/target/release/backfill_timescaledb \
    --start-date 2024-01-01 \
    --end-date 2024-12-31 \
    --batch-size 1000 \
    --questdb-url http://questdb.internal:9000 \
    --timescale-url postgres://fintext:fintext@timescaledb.internal:5432/fintext_timeseries
```

#### Step 3: Verify Idempotency & Duplicate Skipping
The backfill utility verifies natural keys `(ticker, published_utc, source)` before insertion. Re-running the utility over existing data skips 100% of existing records:
```bash
./rust/target/release/backfill_timescaledb --start-date 2024-01-01 --end-date 2024-12-31
# Output:
# Total Records Processed: 154200
# Total Records Inserted:  0
# Total Records Skipped:   154200
```

### 9.3. Production Switchover Procedure
1. **Enable Primary Switch in Config**:
   In `config/config.yaml`:
   ```yaml
   database:
     timescaledb:
       enabled: true
       primary: true
   ```
   Or via environment variables:
   ```bash
   export ENABLE_TIMESCALEDB=1
   export TIMESCALE_PRIMARY=1
   ```
2. **Deploy Rolling Restart**:
   Deploy updated configuration to API server and Ingestion Engine pods using rolling restart:
   ```bash
   kubectl rollout restart deployment/fintext-api-server -n fintext
   kubectl rollout restart deployment/fintext-ingestion-engine -n fintext
   ```
3. **Monitor Telemetry**:
   - Check API server logs for `[TimescaleDB Primary]` query routing messages.
   - Verify Prometheus metric `api_request_duration_seconds` to confirm P95 latency $<50\text{ms}$.
   - Confirm fallback rate to QuestDB is zero under normal conditions.

### 9.4. Instant Rollback Procedure
If any unexpected latency or database issues arise, instantly revert query and write priority back to QuestDB:
```bash
export TIMESCALE_PRIMARY=0
# Or update config.yaml: database.timescaledb.primary: false
kubectl rollout restart deployment/fintext-api-server -n fintext
```
Because QuestDB dual-write remained active throughout, no sentiment data is lost during either switchover or rollback.

---

## 10. Quantitative Signal Quality & ICIR Governance

### 10.1. Overview
The FinText Signal Quality engine (`POST /signals/quality-report`) delivers comprehensive statistical diagnostics on generated alpha signals (sentiment, spillover, GEX, insider, event). In addition to coverage, freshness latency, decay curves, and quantile spreads, the engine evaluates cross-sectional predictive consistency via the **Information Coefficient Information Ratio (ICIR)**.

### 10.2. ICIR Statistical Definition
The Information Coefficient Information Ratio measures signal consistency and risk-adjusted predictive skill:
$$\text{ICIR} = \frac{\overline{\text{IC}}}{\sigma_{\text{IC}}}$$
where:
- $\overline{\text{IC}} = \frac{1}{N} \sum_{t=1}^N \text{IC}_t$ is the mean Spearman rank correlation across $N$ cross-sectional periods.
- $\sigma_{\text{IC}} = \sqrt{\frac{1}{N-1} \sum_{t=1}^N (\text{IC}_t - \overline{\text{IC}})^2}$ is the sample standard deviation of period ICs.

### 10.3. Operational Thresholds
- **Minimum Period Requirement**: A minimum of $N \ge 10$ evaluation periods is required. If $N < 10$, `icir` returns `null` with a warning log to prevent spurious estimates.
- **Period Sampling**:
  - For universe sizes $\ge 3$ tickers: Daily cross-sectional ICs across tickers.
  - For small universes ($< 3$ tickers): Weekly aggregated cross-sectional periods.
- **Quality Benchmarks**:
  - $\text{ICIR} \ge 0.50$: Good systematic alpha signal.
  - $\text{ICIR} \ge 1.00$: Exceptional signal stability across market cycles.

---

## 11. Raw Data Archive Lifecycle Governance & Upload Verification (Suite #265 & #273)

### 11.1. Overview
The Raw Data Archive subsystem automatically captures, batches, and persists raw financial news articles, SEC filings, social sentiment feeds, and options flow documents into immutable, Snappy-compressed Apache Parquet files. Files are organized into Hive-style date partitions (`year=YYYY/month=MM/day=DD`) and uploaded to S3 or MinIO object storage.

### 11.2. Cloud Storage Lifecycle Governance
To comply with SEC/FINRA 10-year regulatory retention mandates while minimizing cloud infrastructure expenditure, the archive uploader automatically applies AWS S3 / MinIO `BucketLifecycleConfiguration`:

1. **Storage Tiering to Glacier (`lifecycle_transition_days`, default: 30)**:
   - Parquet objects under the archive prefix (`raw/`) are automatically transitioned from `STANDARD` storage class to `GLACIER` after 30 days.
   - Reduces ongoing storage costs by **~81.2%** ($0.023/GB-mo down to $0.004/GB-mo).
   - Setting `lifecycle_transition_days: 0` or negative disables the transition rule.

2. **Automatic Expiration (`lifecycle_expiration_days`, default: 3650 / 10 years)**:
   - Parquet files older than 10 years are automatically pruned by S3/MinIO bucket lifecycle policies.
   - Setting `lifecycle_expiration_days: 0` or negative disables the expiration rule.

### 11.3. HeadObject Upload Verification Protocol
When `verify_upload` is enabled (`verify_upload: true` or `RAW_ARCHIVE_VERIFY_UPLOAD=1`):
1. Immediately following a successful `PutObject` call, the uploader executes an AWS S3 `HeadObject` request.
2. The remote `ContentLength` is verified against local on-disk Parquet file metadata.
3. If `HeadObject` fails (e.g., HTTP 404/network error) or if remote and local byte sizes mismatch, a warning is logged and the operation returns `Err(...)`.
4. **Data Safety Guarantee**: Whenever verification fails or S3 is unreachable, the local Parquet file is **retained on disk** in `data/archive/...` to prevent data loss.

### 11.4. Operational Runbook
```bash
# Enable Raw Archiving with S3 provider and upload verification
export RAW_ARCHIVE_ENABLED=1
export RAW_ARCHIVE_PROVIDER=s3
export RAW_ARCHIVE_BUCKET=fintext-raw-archive
export RAW_ARCHIVE_LIFECYCLE_TRANSITION_DAYS=30
export RAW_ARCHIVE_LIFECYCLE_EXPIRATION_DAYS=3650
export RAW_ARCHIVE_VERIFY_UPLOAD=1

# Verify Suite #265 (Base Archiver) and Suite #273 (Lifecycle & Verification)
python scripts/verify_raw_archive.py
python scripts/verify_raw_archive_lifecycle.py
```




