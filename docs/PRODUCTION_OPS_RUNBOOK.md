# FinText Alpha Vectorizer — Production Day-2 Operations Runbook
**Document ID:** `DOC-OPS-DAY2-2026-V1`
**Classification:** Institutional Operations / Site Reliability Engineering Manual
**Last Updated:** 2026-09-26
**Execution Target:** AWS Region `us-east-1` (N. Virginia)
**Cost Impact:** $0.00 incremental (all tooling runs on existing c6i.xlarge host)
**Target Audience:** Founder / CTO, SRE On-Call, DevOps Lead
**Authoritative References:**
[`scripts/backup_pg.sh`](file:///d:/FinText-Alpha-Vectorizer/scripts/backup_pg.sh) |
[`scripts/dr_weekly_drill.py`](file:///d:/FinText-Alpha-Vectorizer/scripts/dr_weekly_drill.py) |
[`infra/terraform/sns.tf`](file:///d:/FinText-Alpha-Vectorizer/infra/terraform/sns.tf) |
[`infra/terraform/cloudwatch.tf`](file:///d:/FinText-Alpha-Vectorizer/infra/terraform/cloudwatch.tf) |
[`docs/BACKUP_RESTORE_RUNBOOK.md`](file:///d:/FinText-Alpha-Vectorizer/docs/BACKUP_RESTORE_RUNBOOK.md)

---

## 1. Document Purpose & Scope

This runbook is the definitive, step-by-step **repair manual** for Day-2 production
operations — the recurring tasks that keep FinText Alpha Vectorizer healthy **after**
initial deployment. It covers five operational domains:

| Domain | Tooling | Cadence |
| :--- | :--- | :--- |
| Scheduled Database Backups | `scripts/backup_pg.sh` | Hourly (cron) |
| Disaster Recovery Drills | `scripts/dr_weekly_drill.py` | Weekly (cron) |
| SNS Operational Alerting | `infra/terraform/sns.tf` + `cloudwatch.tf` | Event-driven |
| Container Log Shipping | `docker-compose.yml` (prod profile) | Continuous |
| Incident Response Playbook | This document §7 | As-needed |

**Standing Invariants** (these are business law, not suggestions):
- P95 API latency ≤ 500 ms SLA (actual: 12–40 ms)
- Monthly cloud cap: $301.44 (current run-rate: $281.49)
- Zero cross-tenant data leakage (RLS enforced on all 20 base tables)
- RPO ≤ 1 hour, RTO ≤ 4 hours (automated DR drill target: < 5 minutes)

---

## 2. Scheduled Database Backups (`backup_pg.sh`)

### 2.1 What It Does

The backup script executes a PostgreSQL `pg_dump` (custom archive format, level-6
compression) against the `fintext-postgres` container, produces a SHA-256 cryptographic
sidecar for chain-of-custody verification, maintains a markdown audit ledger, optionally
ships to S3 with CloudWatch success/failure metric publication, and prunes local staging
to retain the last 3 dumps.

### 2.2 Architecture Constants (Source-of-Truth Citations)

```
DB_CONTAINER = fintext-postgres        # docker-compose.yml line 176
DB_USER      = fintext                 # docker-compose.yml line 179
DB_NAME      = fintext_metadata        # docker-compose.yml line 181
S3_BUCKET    = fintext-backups-production  # infra/terraform/s3.tf line 25
S3_PREFIX    = postgres/               # infra/terraform/s3.tf line 77
AWS_REGION   = us-east-1               # infra/terraform/variables.tf line 8
```

### 2.3 Usage

```bash
# Local-only backup (development / staging — no S3, no CloudWatch)
bash scripts/backup_pg.sh local hourly

# Production backup with S3 shipping + CloudWatch metric
bash scripts/backup_pg.sh s3 hourly

# Nightly production backup (larger window, same mechanics)
bash scripts/backup_pg.sh s3 nightly
```

### 2.4 Crontab Installation (Production EC2 Host)

Add to the `fintext_admin` user crontab via `crontab -e`:

```crontab
# ── FinText Hourly PostgreSQL Backup (S3 mode) ──────────────────────────────
0 * * * * cd /opt/fintext && bash scripts/backup_pg.sh s3 hourly >> /opt/fintext/logs/backup_cron.log 2>&1

# ── FinText Nightly Full Backup (S3 mode) ────────────────────────────────────
30 2 * * * cd /opt/fintext && bash scripts/backup_pg.sh s3 nightly >> /opt/fintext/logs/backup_cron.log 2>&1
```

### 2.5 Output Artifacts

| Artifact | Location | Purpose |
| :--- | :--- | :--- |
| Compressed dump | `logs/backups_local/fintext_{cadence}_{timestamp}.dump.gz` | Local staging archive |
| SHA-256 sidecar | `logs/backups_local/fintext_{cadence}_{timestamp}.dump.gz.sha256` | Integrity verification |
| Backup ledger | `logs/backup_ledger.md` | Immutable audit trail (SEC 17a-4) |
| S3 copy (s3 mode) | `s3://fintext-backups-production/postgres/{cadence}/` | Cloud durable archive |
| CloudWatch metric | `fintext/BackupJobFailed` (value 0=success, 1=failure) | Alarm integration |

### 2.6 Failure Handling

On any failure (`set -euo pipefail` + `trap on_failure ERR`):
1. A `FAIL` row is appended to the ledger with zero-hash sentinel digest
2. CloudWatch `BackupJobFailed=1` metric is emitted (s3 mode only)
3. The `BackupJobFailed` alarm triggers SNS notification to ops email (if configured)
4. Exit code is preserved for cron alerting

### 2.7 Local Retention Policy

The script retains the last 3 local dumps (`MAX_LOCAL_KEEP=3`). Older dumps and their
SHA-256 sidecars are pruned automatically after each successful backup. S3 retention
is governed by the bucket lifecycle policy defined in `infra/terraform/s3.tf`.

---

## 3. Disaster Recovery Weekly Drill (`dr_weekly_drill.py`)

### 3.1 What It Does

The DR drill script automates the full disaster recovery verification loop:
1. Discovers the latest backup archive (local staging or S3)
2. Spawns an **ephemeral** PostgreSQL 16 container (never touches production containers)
3. Restores the dump into a clean, isolated database instance
4. Executes verification battery: schema table count, PIT table record counts, RLS isolation
5. Benchmarks wall-clock RTO against the 4-hour SLA
6. Records results in `logs/dr_ledger.md` and `logs/dr_report.json`
7. Guarantees container cleanup via `try/finally` (no orphaned containers)

### 3.2 Usage

```bash
# Standard local-mode weekly drill
python scripts/dr_weekly_drill.py --mode local

# Drill from specific backup file
python scripts/dr_weekly_drill.py --mode local --dump-path logs/backups_local/fintext_hourly_20260926_080000Z.dump.gz

# S3-sourced drill (downloads latest from cloud backup bucket)
python scripts/dr_weekly_drill.py --mode s3

# Custom report output path
python scripts/dr_weekly_drill.py --mode local --json-report logs/dr_report_custom.json
```

### 3.3 Crontab Installation (Production EC2 Host)

```crontab
# ── FinText Weekly DR Drill (Sunday 03:00 UTC, S3 mode) ─────────────────────
0 3 * * 0 cd /opt/fintext && python3 scripts/dr_weekly_drill.py --mode s3 >> /opt/fintext/logs/dr_cron.log 2>&1
```

### 3.4 Verification Battery

| Check | Threshold | Pass Criteria |
| :--- | :--- | :--- |
| Schema base table count | ≥ 20 CLASS-T tables | `information_schema.tables` count in public schema |
| `instrument_master` records | > 0 | PIT table must have at least 1 restored row |
| `filings_raw` records | > 0 | PIT table must have at least 1 restored row |
| `filings_normalized` records | > 0 | PIT table must have at least 1 restored row |
| `sentiment_records` records | > 0 | PIT table must have at least 1 restored row |
| RLS cross-tenant isolation | 0 leaks | `SET ROLE fintext_app; SET app.current_org_id = 'org_A'; SELECT FROM ... WHERE org_id = 'org_B'` returns 0 |
| Restore RTO wall-clock | < 14,400s (4h SLA) | Measured from `docker run` to verification complete |

### 3.5 Output Artifacts

| Artifact | Location | Purpose |
| :--- | :--- | :--- |
| JSON drill report | `logs/dr_report.json` | Machine-readable drill results |
| DR ledger | `logs/dr_ledger.md` | Immutable institutional audit trail |

### 3.6 Critical Note: TimescaleDB Schema Pre-Creation

The DR drill creates `_timescaledb_internal` and `_timescaledb_catalog` schemas in the
ephemeral container **before** running `pg_restore`. This prevents chunk inheritance
failures when restoring hypertable data backed up from a TimescaleDB-enabled source into
a vanilla PostgreSQL 16 target. This is a known requirement — see lines 250–254 of
[`dr_weekly_drill.py`](file:///d:/FinText-Alpha-Vectorizer/scripts/dr_weekly_drill.py#L246-L254).

---

## 4. SNS Operational Alerting (`sns.tf`)

### 4.1 Architecture

SNS alerting is **staged** — gated by `var.ops_alert_email` (default `""`). When the
variable is empty, zero SNS resources are provisioned (zero cost). When a valid email is
provided, the following resources are created:

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                         SNS ALERT ROUTING TOPOLOGY                          │
├──────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  CloudWatch Metric Alarms                  SNS Topic                        │
│  ├─ ec2_high_cpu (CPU > 80%)    ──────►  fintext-ops-alerts-{env}           │
│  ├─ ec2_status_check (Failed)   ──────►       │                             │
│  ├─ rds_high_cpu (CPU > 80%)    ──────►       │                             │
│  ├─ rds_low_storage (< 15 GB)   ──────►       │                             │
│  └─ backup_failed (≥ 1)        ──────►       │                             │
│                                               ▼                             │
│                                     Email Subscription                      │
│                                     (var.ops_alert_email)                   │
│                                                                              │
└──────────────────────────────────────────────────────────────────────────────┘
```

### 4.2 Enabling Alerting

```bash
# Set the email address in terraform.tfvars or via CLI
terraform apply -var="ops_alert_email=sre-oncall@fintext.ai"
```

**Important:** After initial `terraform apply`, the email subscriber must confirm the
SNS subscription by clicking the confirmation link sent to the provided email address.

### 4.3 Disabling Alerting

```bash
# Reset to default (empty string) — destroys SNS topic and subscription
terraform apply -var="ops_alert_email="
```

### 4.4 Security

- SNS topic uses server-side encryption via `alias/aws/sns` KMS key
- Tags include `Project`, `Environment`, and `Component` for cost attribution

---

## 5. Container Log Shipping (Production Profile)

### 5.1 Architecture

Production container log shipping uses Docker's `awslogs` logging driver to stream
container stdout/stderr directly to CloudWatch Logs. This is configured in the
`docker-compose.yml` production profile services (`fintext-api-prod` and
`fintext-ingestion-prod`).

### 5.2 Configuration

```yaml
logging:
  driver: "awslogs"
  options:
    awslogs-region: "us-east-1"
    awslogs-group: "${FINTEXT_LOG_GROUP:-fintext-prod}"
    awslogs-stream: "${HOSTNAME:-node1}"
    max-size: "50m"
    max-file: "3"
```

### 5.3 Starting Production Services

```bash
# Production profile (with awslogs log shipping)
docker compose --profile prod up -d

# Default profile (development — local logging, no awslogs)
docker compose up -d
```

**Important:** The production profile services are completely isolated from the default
profile services. Running `docker compose up -d` (no `--profile`) starts only the
development services with local logging — zero impact on the default developer workflow.

### 5.4 CloudWatch Log Groups

| Log Group | Source Container | CloudWatch Name |
| :--- | :--- | :--- |
| API Gateway | `fintext-api-gateway-prod` | `/fintext/api-server-production` |
| Ingestion Engine | `fintext-ingestion-engine-prod` | `/fintext/ingestion-engine-production` |
| Custom (env var) | Both | `${FINTEXT_LOG_GROUP}` override |

### 5.5 Prerequisites

1. IAM instance role must have `logs:CreateLogStream` and `logs:PutLogEvents` permissions
2. CloudWatch log groups must exist (created by `infra/terraform/cloudwatch.tf`)
3. EC2 host must have Docker configured with `awslogs` driver available

---

## 6. Monitoring & Alarm Reference

### 6.1 CloudWatch Metric Alarms

| Alarm Name | Metric | Threshold | Evaluation | Severity |
| :--- | :--- | :--- | :--- | :--- |
| `fintext-ec2-high-cpu-{env}` | `AWS/EC2/CPUUtilization` | > 80% avg | 2 × 5m periods | Warning |
| `fintext-ec2-status-check-{env}` | `AWS/EC2/StatusCheckFailed` | > 0 max | 2 × 1m periods | Critical |
| `fintext-rds-high-cpu-{env}` | `AWS/RDS/CPUUtilization` | > 80% avg | 2 × 5m periods | Warning |
| `fintext-rds-low-storage-{env}` | `AWS/RDS/FreeStorageSpace` | < 15 GB avg | 1 × 5m period | Critical |
| `fintext-backup-failed-{env}` | `fintext/BackupJobFailed` | ≥ 1 max | 1 × 1m period | Critical |
| `fintext-feed-breaker-open-{env}` | `fintext_provider_state` | == 2 (Open) | 1 × 5m period | Warning (Degraded) |

> **Note on Feed Resilience:** Upstream feed vendor disruptions trigger per-source circuit breakers and automatic REST degradation. For operational runbook and customer advisory templates, see [`docs/FEED_RESILIENCE_RUNBOOK.md`](file:///d:/FinText-Alpha-Vectorizer/docs/FEED_RESILIENCE_RUNBOOK.md).

### 6.2 Custom Application Metrics

| Metric | Namespace | Published By | Values |
| :--- | :--- | :--- | :--- |
| `BackupJobFailed` | `fintext` | `scripts/backup_pg.sh` (s3 mode) | `0` = success, `1` = failure |
| `fintext_provider_state` | `fintext/ingestion` | Ingestion Telemetry (:9102) | `0` = Closed, `1` = Half-Open, `2` = Open |
| `fintext_fallback_active` | `fintext/ingestion` | Ingestion Telemetry (:9102) | `0` = Primary, `1` = Degraded/Stale |

---

## 7. Incident Response Playbook

### 7.1 Backup Job Failure

**Trigger:** `fintext-backup-failed` alarm fires, or `backup_ledger.md` shows `FAIL` row.

**Immediate Actions (< 15 minutes):**
1. SSH to EC2 host: `ssh -i ~/.ssh/fintext-prod.pem ec2-user@<elastic-ip>`
2. Check container health: `docker inspect --format='{{.State.Health.Status}}' fintext-postgres`
3. Review cron log: `tail -100 /opt/fintext/logs/backup_cron.log`
4. Verify disk space: `df -h /opt/fintext/logs/backups_local/`
5. Attempt manual backup: `cd /opt/fintext && bash scripts/backup_pg.sh local hourly`

**Escalation:**
- If manual backup succeeds → transient issue, monitor next scheduled run
- If manual backup fails → check PostgreSQL container logs: `docker logs fintext-postgres --tail 200`
- If container is unhealthy → restart: `docker compose restart postgres`
- If disk full → prune old backups and investigate growth rate

### 7.2 EC2 Status Check Failure

**Trigger:** `fintext-ec2-status-check` alarm fires.

**Immediate Actions:**
1. Check AWS console for instance status (system/instance check details)
2. If system check failed → AWS host hardware issue; stop and start instance (not reboot)
3. Verify Elastic IP re-attachment: `aws ec2 describe-addresses --allocation-ids <eip-alloc-id>`
4. Verify services recovered: `docker compose ps`
5. Run smoke test: `curl -s http://localhost:8000/health | jq .`

### 7.3 RDS Storage Exhaustion

**Trigger:** `fintext-rds-low-storage` alarm fires (< 15 GB free).

**Immediate Actions:**
1. Check current usage: `aws rds describe-db-instances --db-instance-identifier fintext-metadata --query 'DBInstances[0].AllocatedStorage'`
2. Identify growth driver: large tables, bloated indexes, or WAL accumulation
3. Run `VACUUM FULL` on largest tables (schedule during low-traffic window)
4. If persistent → increase `db_allocated_storage_gb` in `variables.tf` and `terraform apply`

### 7.4 DR Drill Failure

**Trigger:** `dr_report.json` shows status other than `CERTIFIED_HEALTHY`.

**Immediate Actions by Status:**

| Status | Root Cause | Action |
| :--- | :--- | :--- |
| `DOCKER_SPAWN_FAILED` | Docker daemon issue | Restart Docker daemon on host |
| `INSUFFICIENT_TABLES` | Corrupt or partial backup | Verify latest backup manually with `pg_restore --list` |
| `PIT_DATA_EMPTY` | Missing PIT seed data | Check if `config/pit_reference_schema.sql` was applied |
| `RLS_ISOLATION_LEAK` | Policy regression | Audit `config/timescale/03-rls-multi-tenant-isolation.sql` |
| `CHECKSUM_MISMATCH` | Corrupted transfer | Re-download from S3 and re-verify SHA-256 |

---

## 8. Terraform Plan Diff Verification

Before any infrastructure change, verify zero unintended drift:

```bash
cd infra/terraform

# Format check (must exit 0 with no output)
terraform fmt -check -diff

# Validation (must report "Success!")
terraform validate

# Plan with default values (should show "No changes" for ops resources when email is empty)
terraform plan -var="ops_alert_email="

# Plan with alerting enabled
terraform plan -var="ops_alert_email=sre@fintext.ai"
```

**Invariant:** With default `ops_alert_email=""`, the plan diff for SNS and backup alarm
resources must be empty (zero resources created, zero cost).

---

## 9. File Inventory & Cross-References

| File | Purpose | Lines | Dependencies |
| :--- | :--- | :--- | :--- |
| `scripts/backup_pg.sh` | Scheduled PostgreSQL backup automation | 194 | `docker`, `pg_dump`, optionally `aws` CLI |
| `scripts/dr_weekly_drill.py` | Automated weekly DR drill | 423 | Python 3.8+ stdlib only, `docker` CLI |
| `infra/terraform/sns.tf` | SNS topic + email subscription | 34 | `var.ops_alert_email`, `var.environment` |
| `infra/terraform/cloudwatch.tf` | Log groups + metric alarms | 141 | `sns.tf` (for `local.alerting_enabled`), EC2/RDS refs |
| `infra/terraform/variables.tf` | Variable definitions (incl. `ops_alert_email`) | 148 | None |
| `infra/terraform/outputs.tf` | Output definitions (incl. `ops_topic_arn`) | 64 | `sns.tf` |
| `docker-compose.yml` | Prod profile services with `awslogs` | +76 lines | AWS IAM role, CloudWatch log groups |
| `logs/backup_ledger.md` | Immutable backup audit trail | Generated | `backup_pg.sh` |
| `logs/dr_ledger.md` | Immutable DR drill audit trail | Generated | `dr_weekly_drill.py` |
| `logs/dr_report.json` | Machine-readable DR drill report | Generated | `dr_weekly_drill.py` |

---

## 10. Compliance & Regulatory Mapping

| Requirement | Standard | Implementation |
| :--- | :--- | :--- |
| Automated backup with audit trail | SOC 2 CC8.1 / A1.2 | `backup_pg.sh` + `backup_ledger.md` |
| Cryptographic integrity verification | SEC Rule 17a-4 | SHA-256 sidecar files |
| Periodic recovery testing | SOC 2 A1.3 | `dr_weekly_drill.py` weekly cron |
| Multi-tenant data isolation | SOC 2 CC6.1 | RLS verification in DR drill |
| Infrastructure change alerting | SOC 2 CC7.2 | SNS + CloudWatch alarms |
| Log retention & shipping | SOC 2 CC7.3 | `awslogs` driver → CloudWatch (30-day retention) |

---

## Appendix A: Quick Command Reference

```bash
# ── Backup Operations ────────────────────────────────────────────────────────
bash scripts/backup_pg.sh local hourly    # Local dev backup
bash scripts/backup_pg.sh s3 hourly       # Production hourly backup
bash scripts/backup_pg.sh s3 nightly      # Production nightly backup

# ── DR Drill Operations ──────────────────────────────────────────────────────
python scripts/dr_weekly_drill.py --mode local     # Local DR drill
python scripts/dr_weekly_drill.py --mode s3        # S3-sourced DR drill

# ── Production Container Operations ──────────────────────────────────────────
docker compose --profile prod up -d                # Start prod services
docker compose --profile prod logs -f              # Follow prod logs
docker compose --profile prod down                 # Stop prod services

# ── Monitoring ───────────────────────────────────────────────────────────────
cat logs/backup_ledger.md                          # View backup history
cat logs/dr_ledger.md                              # View DR drill history
cat logs/dr_report.json | python -m json.tool      # View latest DR report

# ── Terraform ────────────────────────────────────────────────────────────────
terraform fmt -check -diff                         # Format check
terraform validate                                 # Syntax validation
terraform plan -var="ops_alert_email="              # Verify zero-cost default
```
