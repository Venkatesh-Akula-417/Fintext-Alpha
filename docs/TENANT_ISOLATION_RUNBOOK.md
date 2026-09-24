# FinText Alpha Vectorizer — Tenant Isolation & PostgreSQL RLS Runbook
═══════════════════════════════════════════════════════════════════════════════
Document ID: RUNBOOK-SEC-RLS-001  
Classification: INSTITUTIONAL SECURITY & DATABASE INTEGRITY  
Status: PRODUCTION CERTIFIED (P1 PRIVATE-BETA BLOCKER RESOLVED)  
Audience: Platform Engineers, Security Architects, PostgreSQL DBAs, Quantitative SREs  
Repository: `FinText-Alpha-Vectorizer` (`git@github.com:Venkatesh-Akula-417/Fintext-Alpha.git`)  
═══════════════════════════════════════════════════════════════════════════════

## 1. Purpose & Threat Model

### 1.1 Purpose
FinText Alpha Vectorizer hosts multiple competing institutional quantitative hedge funds, statistical arbitrage desks, and proprietary trading groups on a shared high-performance infrastructure cluster. This runbook documents the cryptographic, database-enforced, and application-level controls that guarantee strict multi-tenant isolation. Under this architecture, no tenant can ever observe, modify, delete, or infer the existence of another tenant's data.

### 1.2 Threat Model & Confinement Boundary
In a shared database deployment, relying solely on application-level filtering (`WHERE org_id = $1`) presents catastrophic vulnerability modes:
- **T-Threat-1: Handler Omission:** An engineer adds a new endpoint or forgets `WHERE org_id = $1` in one of the 46 handler modules, resulting in a cross-tenant data leak.
- **T-Threat-2: SQL Injection / Dynamic Query Bypasses:** An attacker escapes input parameters and bypasses application filters to dump tables.
- **T-Threat-3: Connection Pool State Bleed:** A pooled connection (`sqlx::PgPool`) executes a query for Tenant A, is returned to the pool, and retains session state when serving Tenant B.
- **T-Threat-4: Administrative Role Over-Privilege:** The API gateway runs as database superuser, disabling built-in security features.

### 1.3 Confinement Guarantees
1. **Database-Enforced RLS:** PostgreSQL Row-Level Security is `ENABLED` and `FORCED` on every tenant-owned table (`relforcerowsecurity = true`). Even table owners are bound by security policies.
2. **Deny-by-Default:** If the session configuration variable `app.current_org_id` is unset or `NULL`, the RLS policy evaluates to `NULL` (falsy), returning **0 rows**. Queries fail closed rather than leaking data.
3. **Transaction-Local Isolation:** Tenant context is applied strictly via `SELECT set_config('app.current_org_id', $1, true)` inside an active SQL transaction. When the transaction commits or rolls back, the GUC is instantly destroyed, eliminating pool reuse bleed.
4. **Least-Privilege Role Separation:** The API Gateway connects as `fintext_app` (`NOBYPASSRLS`), making it impossible to circumvent policies even if arbitrary SQL were executed.

---

## 2. Architecture & Data Flow

```
[ Client Request ]
       │  Authorization: Bearer <JWT>
       ▼
┌────────────────────────────────────────────────────────────────────────┐
│ Rust Axum API Gateway (:8000)                                          │
│                                                                        │
│ 1. Auth Middleware: Validates RS256/HS256 JWT signature & expiry       │
│ 2. Claims Extractor: Extracts `TenantContext { org_id: Uuid }`         │
│ 3. Handler Layer: Calls `state.with_tenant(&org_id, |tx| async { ... })│
└───────────────────────────────────┬────────────────────────────────────┘
                                    │
                                    │ sqlx::PgPool Connection Acquired
                                    ▼
┌────────────────────────────────────────────────────────────────────────┐
│ PostgreSQL 16 / TimescaleDB 2.30 Instance                              │
│                                                                        │
│ Step 1: BEGIN TRANSACTION;                                             │
│ Step 2: SELECT set_config('app.current_org_id', 'org_123', true);      │
│         ─── Scope is transaction-local (is_local = true) ───           │
│                                                                        │
│ Step 3: SELECT * FROM audit_logs;                                      │
│         PostgreSQL evaluates RLS Policy:                               │
│         USING (org_id::text = current_setting('app.current_org_id', t))│
│         ─── Only rows matching 'org_123' are visible ───               │
│                                                                        │
│ Step 4: INSERT INTO universes (..., org_id) VALUES (..., 'org_456');   │
│         WITH CHECK constraint rejects mismatched org_id -> ERROR       │
│                                                                        │
│ Step 5: COMMIT / ROLLBACK;                                             │
│         ─── GUC destroyed; connection returns clean to pool ───        │
└────────────────────────────────────────────────────────────────────────┘
```

---

## 3. Database Roles & Privilege Matrix

| Role Name | Can Login? | BYPASSRLS? | Usage Tier | Credentials Source |
| :--- | :---: | :---: | :--- | :--- |
| `fintext` / `fintext_admin` | Yes | **YES** | Superuser, Schema Migrations, Disaster Recovery Backups (`pg_dump`) | `POSTGRES_PASSWORD` in `.env` / Vault |
| `fintext_app` | Yes | **NO** | API Gateway runtime DML queries (`SELECT`, `INSERT`, `UPDATE`, `DELETE`) | `APP_DB_PASSWORD` / `DATABASE_APP_URL` |
| `fintext_ingest` | Yes | **YES** | Ingestion Engine market-data writer (`sentiment_records`, market feeds) | `INGEST_DB_PASSWORD` / `DATABASE_INGEST_URL` |

### Least-Privilege Role Rules:
1. `fintext_app` MUST NEVER be granted `BYPASSRLS` or `SUPERUSER`.
2. `fintext_app` has `REVOKE ALL` on operations tables (`dlq_events`).
3. Connection strings for API Gateway must use `DATABASE_APP_URL`.

---

## 4. Table Classification Matrix

Every table in the FinText metadata database is formally classified into one of three tiers based on schema ownership:

| Table Name | Class | Semantic Scope | RLS Enabled? | RLS Forced? | Schema Evidence |
| :--- | :---: | :--- | :---: | :---: | :--- |
| `audit_logs` | **CLASS-T** | Tenant audit trail | **YES** | **YES** | `audit_logs.org_id` (UUID) |
| `organizations` | **CLASS-T** | Organization entity | **YES** | **YES** | `organizations.id` (UUID) |
| `organization_members` | **CLASS-T** | Tenant user membership | **YES** | **YES** | `organization_members.org_id` (UUID) |
| `api_keys` | **CLASS-T** | Tenant API credentials | **YES** | **YES** | `api_keys.org_id` (VARCHAR) |
| `usage_events` | **CLASS-T** | Stripe billing usage records | **YES** | **YES** | `usage_events.org_id` (VARCHAR) |
| `universes` | **CLASS-T** | Custom quantitative portfolios | **YES** | **YES** | `universes.org_id` (VARCHAR) |
| `webhooks` | **CLASS-T** | Signal alert notification targets | **YES** | **YES** | `webhooks.org_id` (VARCHAR) |
| `data_retention_policies` | **CLASS-T** | Compliance data retention rules | **YES** | **YES** | `data_retention_policies.org_id` (VARCHAR) |
| `model_retraining_jobs` | **CLASS-T** | Custom model fine-tuning runs | **YES** | **YES** | `model_retraining_jobs.org_id` (VARCHAR) |
| `kafka_credentials` | **CLASS-T** | Dedicated Redpanda streaming SASL | **YES** | **YES** | `kafka_credentials.org_id` (VARCHAR) |
| `ip_whitelist` | **CLASS-T** | CIDR ingress access control | **YES** | **YES** | `ip_whitelist.org_id` (VARCHAR) |
| `chat_alert_subscriptions` | **CLASS-T** | Slack/Teams webhook alerts | **YES** | **YES** | `chat_alert_subscriptions.org_id` (VARCHAR) |
| `polling_webhooks` | **CLASS-T** | SQS/Queue polling configurations | **YES** | **YES** | `polling_webhooks.org_id` (VARCHAR) |
| `subscriptions` | **CLASS-T** | Commercial Stripe tier mapping | **YES** | **YES** | `subscriptions.org_id` (VARCHAR) |
| `email_digest_subscriptions`| **CLASS-T** | Periodic executive reports | **YES** | **YES** | `email_digest_subscriptions.org_id` (VARCHAR) |
| `digest_send_history` | **CLASS-T** | Dispatched email audit log | **YES** | **YES** | `digest_send_history.org_id` (VARCHAR) |
| `fix_orders` | **CLASS-T** | Institutional execution logs | **YES** | **YES** | `fix_orders.org_id` (VARCHAR) |
| `instrument_master` | **CLASS-T** | Tenant universe asset records | **YES** | **YES** | `instrument_master.org_id` (VARCHAR) |
| `filings_raw` | **CLASS-T** | Ingested proprietary filings | **YES** | **YES** | `filings_raw.org_id` (VARCHAR) |
| `filings_normalized` | **CLASS-T** | Cleaned tokenized filings | **YES** | **YES** | `filings_normalized.org_id` (VARCHAR) |
| `sentiment_records` | **CLASS-S** | Public market sentiment signals | NO | NO | TimescaleDB hypertable; shared market reference |
| `pit_fundamentals` | **CLASS-S** | Point-in-Time financial metrics | NO | NO | Point-in-time public SEC reference data |
| `pit_prices` | **CLASS-S** | Point-in-Time price histories | NO | NO | Market-wide historical pricing bars |
| `news_articles` | **CLASS-S** | Raw incoming public news feeds | NO | NO | Public market disclosures |
| `earnings_call_transcripts` | **CLASS-S** | Ingested audio transcripts | NO | NO | Public earnings disclosure transcripts |
| `data_provenance` | **CLASS-S** | SEC/Finnhub/Polygon source audit | NO | NO | Global provenance reference |
| `dlq_events` | **CLASS-O** | Ingestion Dead Letter Queue | NO | NO | Operations & auto-reprocessor worker only |

---

## 5. Row-Level Security DDL Specification

All policies are created with `PERMISSIVE FOR ALL`, enforcing isolation across `SELECT`, `INSERT`, `UPDATE`, and `DELETE`:

```sql
-- Standard policy template applied to all CLASS-T tables:
ALTER TABLE <table_name> ENABLE ROW LEVEL SECURITY;
ALTER TABLE <table_name> FORCE ROW LEVEL SECURITY;

CREATE POLICY tenant_iso_<table_name> ON <table_name>
    AS PERMISSIVE FOR ALL
    USING (org_id::text = current_setting('app.current_org_id', TRUE))
    WITH CHECK (org_id::text = current_setting('app.current_org_id', TRUE));
```

### Key DDL Properties:
1. `current_setting('app.current_org_id', TRUE)`: The second argument `TRUE` tells PostgreSQL to return `NULL` instead of throwing a runtime error when the variable is unset.
2. `org_id::text = NULL`: Evaluates to `NULL`, which PostgreSQL treats as `FALSE`. Thus, unauthenticated queries return 0 records immediately.
3. `WITH CHECK`: Enforces that any `INSERT` or `UPDATE` cannot write a row with an `org_id` differing from the caller's active GUC.

---

## 6. How to Verify Locally (Step-by-Step)

Follow this exact procedure using `psql` to verify RLS enforcement:

### Step 6.1: Verify RLS is Enabled & Forced
```bash
docker exec -i fintext-postgres psql -U fintext -d fintext_metadata -c "
SELECT relname, relrowsecurity, relforcerowsecurity 
FROM pg_class 
WHERE relname IN ('audit_logs', 'universes', 'api_keys', 'usage_events');"
```
**Expected Output:**
```
   relname    | relrowsecurity | relforcerowsecurity 
--------------+----------------+---------------------
 api_keys     | t              | t
 audit_logs   | t              | t
 universes    | t              | t
 usage_events | t              | t
(4 rows)
```

### Step 6.2: Verify Deny-by-Default (Unset GUC)
```bash
docker exec -i fintext-postgres psql -U fintext_app -d fintext_metadata -c "SELECT count(*) FROM audit_logs;"
```
**Expected Output:**
```
 count 
-------
     0
(1 row)
```

### Step 6.3: Verify Cross-Tenant Isolation
```bash
docker exec -i fintext-postgres psql -U fintext_app -d fintext_metadata -c "
SET app.current_org_id = 'a0000000-0000-0000-0000-000000000001';
SELECT count(*) FROM audit_logs WHERE org_id::text = 'b0000000-0000-0000-0000-000000000002';
"
```
**Expected Output:**
```
SET
 count 
-------
     0
(1 row)
```

### Step 6.4: Verify WITH CHECK Rejection on Spoofed Insert
```bash
docker exec -i fintext-postgres psql -U fintext_app -d fintext_metadata -c "
SET app.current_org_id = 'a0000000-0000-0000-0000-000000000001';
INSERT INTO audit_logs (id, org_id, user_id, action, entity_type, details, created_at)
VALUES (gen_random_uuid(), 'b0000000-0000-0000-0000-000000000002', 'hacker', 'LEAK', 'ALERT', '{}', NOW());
"
```
**Expected Output:**
```
ERROR:  new row violates row-level security policy for table "audit_logs"
```

---

## 7. How to Add a New Tenant-Owned Table

When expanding FinText Alpha Vectorizer with new tenant-specific entities, follow this 7-step checklist:

1. **Schema DDL:**
   - Include an `org_id` column (`uuid` or `varchar`).
   - Create an index on `(org_id)` for sub-millisecond query performance:
     `CREATE INDEX idx_<table_name>_org_id ON <table_name>(org_id);`
2. **Enable & Force RLS:**
   ```sql
   ALTER TABLE <table_name> ENABLE ROW LEVEL SECURITY;
   ALTER TABLE <table_name> FORCE ROW LEVEL SECURITY;
   ```
3. **Define Policy:**
   ```sql
   CREATE POLICY tenant_iso_<table_name> ON <table_name>
       AS PERMISSIVE FOR ALL
       USING (org_id::text = current_setting('app.current_org_id', TRUE))
       WITH CHECK (org_id::text = current_setting('app.current_org_id', TRUE));
   ```
4. **Grant Permissions:**
   ```sql
   GRANT SELECT, INSERT, UPDATE, DELETE ON <table_name> TO fintext_app;
   ```
5. **Rust Handler Integration:**
   - Always access the table inside `state.with_tenant(&org_id, |tx| async { ... })`.
   - Never use direct pool queries without tenant context.
6. **Integration Tests:**
   - Add the table name to `CLASS_T_TABLES` in `scripts/test_rls_isolation.py`.
   - Run `python scripts/test_rls_isolation.py` and verify 0 leak rows.
7. **Documentation:**
   - Update Table Classification Matrix in this Runbook.

---

## 8. QuestDB Hot-Cache Compensating Controls

### Architectural Asymmetry:
QuestDB is a high-performance columnar time-series database designed for sub-millisecond market microstructure queries. **QuestDB does not support native PostgreSQL Row-Level Security.**

### Compensating Controls:
1. **Public Market Data Only:** QuestDB stores only **CLASS-S** data (`sentiment_records`, market quotes). No tenant-owned data (api_keys, audit logs, custom universes) is ever persisted in QuestDB.
2. **Server-Side Filter Injection:** When querying QuestDB fallback paths, the Rust gateway query builder strictly enforces tenant filtering by appending `AND org_id = '$1'` using verified server-side claims from `TenantContext`.
3. **TimescaleDB Fallback Parity:** If degraded mode triggers, queries route to TimescaleDB primary, where TimescaleDB hypertables inherit PostgreSQL RLS.

---

## 9. Operations & Maintenance Procedures

### 9.1 Database Migrations
Database migrations must be applied using the administrative role `fintext`:
```bash
docker exec -i fintext-postgres psql -U fintext -d fintext_metadata -f /path/to/migration.sql
```
*Note: If attempted as `fintext_app`, PostgreSQL will throw `ERROR: permission denied for table ...` by design.*

### 9.2 Disaster Recovery & Backups
Scheduled backups (`pg_dump`) must connect as `fintext` with `BYPASSRLS`:
```bash
docker exec -i fintext-postgres pg_dump -U fintext -d fintext_metadata --clean | gzip > backup.sql.gz
```
Because `fintext` has `BYPASSRLS = true`, backups capture the entire database across all tenants without needing mock GUC configuration.

---

## 10. Incident Playbook: Investigating Suspected Leaks (#incident)

If a tenant reports observing unfamiliar records, or if Prometheus fires the `RLSContextMissing` alert, immediately follow this containment procedure:

### Phase 1: Triage & Metric Verification
1. Inspect the Prometheus counter:
   ```bash
   curl -s http://localhost:8000/metrics | grep fintext_rls_context_missing_total
   ```
2. If `fintext_rls_context_missing_total > 0`, an endpoint queried a CLASS-T table without establishing a tenant context.

### Phase 2: Log Inspection
Search the API Gateway structured JSON logs for tenant context failures:
```bash
grep -i "rls_context_missing" logs/api_server.log
```

### Phase 3: Immediate Containment
1. If an active breach is suspected on an organization's API keys:
   ```bash
   docker exec -i fintext-postgres psql -U fintext -d fintext_metadata -c "
   UPDATE api_keys SET rotation_status = 'REVOKED', revoked_at = NOW() WHERE org_id = '<COMPROMISED_ORG_ID>';"
   ```
2. Rotate JWT signing secret in Kubernetes Secrets / Vault:
   ```bash
   kubectl create secret generic api-secrets --from-literal=JWT_SECRET=$(openssl rand -hex 32) --dry-run=client -o yaml | kubectl apply -f -
   kubectl rollout restart deployment/fintext-api-gateway
   ```

### Phase 4: Root Cause Audit
Run the automated isolation test suite:
```bash
python scripts/test_rls_isolation.py
```
Inspect `logs/rls_isolation_report.json` to verify that `cross_tenant_leak_rows == 0`.

---

## 11. Troubleshooting & Failure Modes

| Symptom | Probable Cause | Remediating Action |
| :--- | :--- | :--- |
| Handlers return `0` rows after successful login | `app.current_org_id` was not set in transaction | Ensure the handler uses `state.with_tenant(&org_id, ...)` |
| `permission denied for table ...` during migration | Migration script executed as `fintext_app` | Re-run migration connecting as administrative user `fintext` |
| INSERT fails with `violates row-level security policy` | Insert payload `org_id` does not match active GUC | Verify JWT claims match the payload `org_id` being persisted |
| Table owner sees all rows despite RLS | Table missing `FORCE ROW LEVEL SECURITY` | Run `ALTER TABLE <tbl> FORCE ROW LEVEL SECURITY;` |
| Timescale hypertable chunk doubt | Concern that chunks bypass policy | TimescaleDB automatically propagates RLS policies from root hypertable to all temporal chunks |

---

## 12. Institutional Evidence Index

- **Active Migration:** `config/timescale/03-rls-multi-tenant-isolation.sql`
- **Rust Tenant Context Module:** `rust/api_server/src/tenant.rs`
- **Application State Integration:** `rust/api_server/src/state.rs`
- **Automated Audit Suite:** `scripts/test_rls_isolation.py`
- **Certified Evidence Artifact:** `logs/rls_isolation_report.json`
- **Readiness Verification Suite:** `scripts/verify_private_beta_readiness.py` (Check 21)
- **Prometheus Security Alerting:** `k8s/observability/prometheus-alerts.yaml` (`RLSContextMissing`)
