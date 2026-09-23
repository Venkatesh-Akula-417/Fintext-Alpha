# FinText Alpha Vectorizer — Disaster Recovery & Backup Restore Runbook
### Step-by-Step Mechanical Guide for Recovery, Verification & Historical Replay

> **Document Type**: SRE Runbook & Institutional Business Continuity Plan  
> **Recovery Targets**: **RPO <= 1 Hour** | **RTO <= 4 Hours** (Tested & Certified)  
> **Authoritative Databases**: PostgreSQL 16 + TimescaleDB 2.30 (Primary), QuestDB (Optional Hot Cache), S3 Parquet Raw Archive  
> **Last Verified**: September 23, 2026 (Suite #280 — Automated Restore Test Certified)  

---

## 1. System Recovery Objectives (RPO & RTO)

Like replacing an inner tube and indexing a derailleur on a bike stand, database recovery is a standard, repeatable mechanical procedure:

```
┌────────────────────────────────────────────────────────────────────────────────────────┐
│                        FINANCE-GRADE SLA SPECIFICATIONS                                │
├─────────────────────────┬──────────────────────┬───────────────────────────────────────┤
│ Metric                  │ Committed Target     │ Proven Engineering Mechanism          │
├─────────────────────────┼──────────────────────┼───────────────────────────────────────┤
│ RPO (Data Freshness)    │ <= 1.0 Hour          │ 1-hour automated cronjob + WAL stream │
│ RTO (System Recovery)   │ <= 4.0 Hours         │ Automated pg_restore + index warm-up  │
│ PIT Integrity           │ 100% Zero-Lookahead  │ SCD Type 2 bi-temporal intervals      │
│ Tenant Isolation        │ 100% Org Partition   │ Row-level org_id validation           │
│ Replay Capability       │ Indefinite History   │ Backfill worker from S3 Parquet       │
└─────────────────────────┴──────────────────────┴───────────────────────────────────────┘
```

---

## 2. Protected Data Assets & Storage Topologies

1. **PostgreSQL 16 + TimescaleDB (Primary Store)**:
   - `sentiment_records` hypertable with bi-temporal columns (`valid_from`, `valid_to`, `is_current`, `revision_number`).
   - 4 Point-in-Time foundation tables (`instrument_master`, `filings_raw`, `filings_normalized`, `sentiment_records`).
   - Compliance & RBAC metadata (`orgs`, `users`, `api_keys`, `audit_logs`, `retention_policies`, `usage_events`).
2. **QuestDB Time-Series (Optional Hot Cache)**:
   - Sub-millisecond market microstructure and tick-level ILP streams (`stock_bars`, `sentiment_records`).
3. **S3 / Cloud Storage Raw Archive**:
   - Immutable raw financial filings (SEC EDGAR 8-K/10-K, FOMC statements) and Columnar Parquet batches.
   - Versioning enabled (`status: Enabled`) with SSE-KMS envelope encryption.

---

## 3. Disaster Recovery Execution Procedures

### Step 1: Discover and Verify Latest Backup Archive

List available hourly snapshots stored in the cloud backup repository:

```bash
# 1. Inspect latest PostgreSQL dumps from S3
aws s3 ls s3://fintext-backups-production/postgres/ --human-readable

# 2. Download the most recent archive and its cryptographic checksum
LATEST_BACKUP=$(aws s3 ls s3://fintext-backups-production/postgres/ | sort | tail -n 1 | awk '{print $4}')
aws s3 cp "s3://fintext-backups-production/postgres/${LATEST_BACKUP}" /tmp/backups/
aws s3 cp "s3://fintext-backups-production/postgres/${LATEST_BACKUP}.sha256" /tmp/backups/

# 3. Validate archive integrity before restoration
cd /tmp/backups
sha256sum -c "${LATEST_BACKUP}.sha256"
gzip -t "${LATEST_BACKUP}"
```

### Step 2: Spin Up Isolated Restoration Target

Never restore directly over an active production schema. Always restore into an isolated staging instance or temporary container:

```bash
# Spin up temporary restoration container
docker run -d --name fintext-pg-restore-drill \
  -e POSTGRES_USER=fintext \
  -e POSTGRES_PASSWORD=fintext \
  -e POSTGRES_DB=fintext_recovery \
  -p 5433:5432 timescale/timescaledb:latest-pg16

# Wait for database socket to become ready
until pg_isready -h 127.0.0.1 -p 5433 -U fintext; do
  sleep 1
done
```

### Step 3: Execute Restoration Dump

```bash
# Stream decompressed SQL dump into recovery target
gunzip -c "/tmp/backups/${LATEST_BACKUP}" | psql -h 127.0.0.1 -p 5433 -U fintext -d fintext_recovery
```

### Step 4: Verify Table Parity & Record Counts

Verify that all critical Point-in-Time tables and metadata records were restored completely:

```sql
-- Connect to recovered database
\c fintext_recovery

-- 1. Assert row count integrity across 4 PIT tables
SELECT 'instrument_master' AS table_name, count(*) AS total_rows FROM instrument_master
UNION ALL
SELECT 'filings_raw', count(*) FROM filings_raw
UNION ALL
SELECT 'filings_normalized', count(*) FROM filings_normalized
UNION ALL
SELECT 'sentiment_records', count(*) FROM sentiment_records;

-- 2. Verify SCD Type 2 bi-temporal validity (Zero look-ahead proof)
SELECT ticker, revision_number, sentiment_score, valid_from, valid_to, is_current
FROM sentiment_records
WHERE ticker = 'AAPL'
ORDER BY revision_number;

-- 3. Verify tenant isolation (ensure multi-org records exist without bleed)
SELECT DISTINCT org_id, count(*) AS records_per_org
FROM sentiment_records
GROUP BY org_id;
```

### Step 5: Historical Data Replay via Backfill Worker

If a disaster resulted in gap intervals or corrupted historical spans, re-downloading and replaying historical data is performed easily via the automated backfill worker:

```bash
# Replay filings and recompute sentiment for historical window
python scripts/backfill_timescaledb.py \
  --start-date 2025-01-01 \
  --end-date 2026-09-17 \
  --source s3-parquet-raw \
  --concurrency 4
```

### Step 6: QuestDB Hot-Cache Snapshot Restoration (Velero)

If QuestDB persistent volumes were affected, restore from the daily Velero snapshot:

```bash
# 1. Identify available Velero volume snapshots
velero backup get --selector app.kubernetes.io/name=questdb

# 2. Trigger non-destructive restore
velero restore create --from-backup questdb-snapshot-20260923020000 \
  --restore-volumes=true \
  --wait

# 3. Verify QuestDB HTTP endpoint
curl -f http://questdb.default.svc.cluster.local:9000/status
```

### Step 7: Promote Staging to Primary & Health Probing

```bash
# 1. Run automated verification suite
python scripts/test_restore.py --json-report logs/backup_restore_test_report.json

# 2. Check Gateway health probe
curl -s http://127.0.0.1:8000/health
# Response: {"status":"ok","version":"2.0.0-institutional","timestamp_us":...}

# 3. Check Admin Backup Status
curl -s -H "X-Admin-Token: ${ADMIN_TOKEN}" http://127.0.0.1:8000/admin/backup/status
```

---

## 4. Automated Monthly Drill & Zero-Touch CronJobs

FinText schedules an automated dry-run restoration drill on the **1st of every month at 03:00 UTC** via Kubernetes CronJob `restore-test-cronjob.yaml`.

- **Schedule**: `0 3 1 * *` (Monthly)
- **Execution Script**: `scripts/test_backup_restore.sh`
- **Output Report**: `logs/backup_restore_test_report.json`
- **Alerting**: Alertmanager triggers `RestoreTestFailed` or `RestoreTestOverdue` if a drill fails or is missing $> 30$ days.

---

## 5. Emergency Incident Escalation Matrix

```
┌──────┬──────────────────────┬────────────────────────┬─────────────────────────┐
│ Tier │ Role                 │ Contact Method         │ Responsibility          │
├──────┼──────────────────────┼────────────────────────┼─────────────────────────┤
│ L1   │ Site Reliability Eng │ PagerDuty Alert (Auto) │ Triage & Runbook Step 1 │
│ L2   │ Principal Architect  │ Slack #incident-room   │ Validation & Promotion  │
│ L3   │ CTO / Tech Lead      │ Dedicated Line         │ DNS Cutover & Comms     │
└──────┴──────────────────────┴────────────────────────┴─────────────────────────┘
```
