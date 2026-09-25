# FinText Alpha Vectorizer — Institutional Security Architecture & Tenant Confinement
═══════════════════════════════════════════════════════════════════════════════
Document ID: SEC-POLICY-001  
Classification: INSTITUTIONAL SECURITY & COMPLIANCE (SOC2 / SEC / SEBI)  
Status: PRODUCTION CERTIFIED  
═══════════════════════════════════════════════════════════════════════════════

## 1. Executive Summary

FinText Alpha Vectorizer is an institutional financial technology platform designed to deliver Point-in-Time alternative data and natural language sentiment alpha signals to quantitative hedge funds, statistical arbitrage desks, and proprietary trading teams. Because competing institutional market participants share a production cluster, multi-tenant data confinement is enforced at the database kernel level through PostgreSQL Row-Level Security (RLS), cryptographic JWT claims, and least-privilege role boundaries.

---

## 2. Multi-Tenant Database Row-Level Security (RLS)

### 2.1 Confinement Boundary & Policy Enforcement
Application-level isolation (`WHERE org_id = ...`) is treated as an optimization, not a security boundary. PostgreSQL Row-Level Security is **ENABLED** and **FORCED** across all 20 tenant-owned (`CLASS-T`) database tables:
- `audit_logs`
- `organizations`
- `organization_members`
- `api_keys`
- `usage_events`
- `universes`
- `webhooks`
- `data_retention_policies`
- `model_retraining_jobs`
- `kafka_credentials`
- `ip_whitelist`
- `chat_alert_subscriptions`
- `polling_webhooks`
- `subscriptions`
- `email_digest_subscriptions`
- `digest_send_history`
- `fix_orders`
- `instrument_master`
- `filings_raw`
- `filings_normalized`

Every table enforces the following database policy:
```sql
CREATE POLICY tenant_iso_<table_name> ON <table_name>
    AS PERMISSIVE FOR ALL
    USING (org_id::text = current_setting('app.current_org_id', TRUE))
    WITH CHECK (org_id::text = current_setting('app.current_org_id', TRUE));
```

### 2.2 Deny-by-Default Semantics
If a database query is initiated without setting the session variable `app.current_org_id`, PostgreSQL evaluates `current_setting('app.current_org_id', TRUE)` to `NULL`. The comparison `org_id::text = NULL` returns `NULL` (falsy), returning **0 rows** immediately. Under no circumstances can an unauthenticated query scan or leak cross-tenant records.

### 2.3 Transaction-Local Scope (Zero Connection Pool Bleed)
The Rust API Gateway uses `SELECT set_config('app.current_org_id', $1, true)` inside an active SQL transaction. The parameter `is_local = true` guarantees that the configuration variable is destroyed automatically upon transaction `COMMIT` or `ROLLBACK`. When pooled connections (`sqlx::PgPool`) are recycled, no residual tenant context persists.

---

## 3. Role Separation & Principle of Least Privilege

Database roles are decoupled to enforce least privilege:

1. **`fintext_app` (Runtime API Gateway Role)**:
   - Configured with `NOBYPASSRLS`.
   - Has DML permissions (`SELECT`, `INSERT`, `UPDATE`, `DELETE`) on tenant tables.
   - Strictly confined by RLS policies; cannot bypass tenant scoping even under SQL injection.
   - Has `REVOKE ALL` on operations tables (`dlq_events`).
2. **`fintext_ingest` (Data Pipeline Ingestion Role)**:
   - Configured with `BYPASSRLS` for public market reference data (`sentiment_records`, market feeds).
   - Dedicated service account for automated collectors.
3. **`fintext` / `fintext_admin` (Operations & Migrations Role)**:
   - Configured with `BYPASSRLS`.
   - Reserved exclusively for schema migrations (`03-rls-multi-tenant-isolation.sql`), disaster recovery backups (`pg_dump`), and administrative maintenance.
   - Never used for serving customer API requests.

---

## 4. Threat Mitigation Matrix

| Threat Vector | Potential Impact | Enforced Mitigation |
| :--- | :--- | :--- |
| **Handler Developer Error** | Developer forgets `WHERE org_id = $1` in Axum handler | PostgreSQL RLS filters rows at query execution; zero leak |
| **Connection Pool Reuse** | Tenant A's connection re-used for Tenant B retains tenant state | `SELECT set_config(..., is_local=true)` resets GUC on commit/rollback |
| **SQL Injection Vulnerability** | Malicious payload appends `OR 1=1` to query | Database RLS policy evaluates independently; returns only caller's tenant rows |
| **Cross-Tenant Write Tampering** | Tenant A attempts to `INSERT`/`UPDATE` Tenant B's rows | PostgreSQL `WITH CHECK` constraint aborts transaction with error |
| **Missing Tenant Context** | Handler executes query without JWT claims | Deny-by-default returns 0 rows; increments `fintext_rls_context_missing_total` metric |

---

## 5. Security Observability & Alerting

- **Prometheus Metric**: `fintext_rls_context_missing_total` (counter of database operations attempted without tenant context; normal baseline: `0`).
- **Prometheus Alert**: `RLSContextMissing` (Severity: `critical`, fires if `fintext_rls_context_missing_total > 0` for 5 minutes).
- **Incident Playbook**: Detailed in [`docs/TENANT_ISOLATION_RUNBOOK.md#incident`](./TENANT_ISOLATION_RUNBOOK.md#incident).

---

## 7. Webhook Cryptographic Verification & Secret Lifecycle

FinText Alpha Vectorizer exposes an unauthenticated webhook ingestion endpoint (`POST /v1/billing/webhook`) designed specifically for receiving asynchronous lifecycle events from the Stripe payment network. Because the endpoint cannot use Bearer JWT or API key authentication, security is established strictly via cryptographic authenticity checks.

### 7.1 Cryptographic Verification Boundary
1. **Raw Body Byte Extraction:** The HTTP request body is extracted as immutable raw bytes before any JSON deserialization occurs. Canonical whitespace and byte ordering are preserved exactly as signed by Stripe.
2. **Signature Header Parsing:** The incoming `Stripe-Signature` header is parsed into its component timestamp `t` and signature vector `v1`.
3. **Replay Window Enforcement:** To defend against replay attacks, the server rejects any event where `|t_now - t_event| > 300` seconds with `HTTP 400 Bad Request` and increments `fintext_billing_webhook_errors_total{reason="timestamp"}`.
4. **HMAC-SHA256 Signature Computation:** The expected signature is computed as:
   $$\text{ExpectedSig} = \text{HMAC-SHA256}\Big(\text{Secret},\; t \;\|\; \text{"."} \;\|\; \text{RawBodyBytes}\Big)$$
5. **Constant-Time Comparison:** The computed hex digest and incoming vector are compared byte-by-byte using `constant_time_hex_compare`, which evaluates in strict constant time $O(N)$ with zero short-circuiting. This eliminates timing oracle side-channel vulnerabilities.

### 7.2 Secret Storage & Zero-Downtime Multi-Secret Rotation
- **Secret Storage:** Signing secrets (`whsec_...`) are provisioned via Kubernetes sealed secrets (`stripe-secret`) mounted as container environment variables. In development, placeholders are maintained in `.env.example`. Live secrets are NEVER committed to version control or printed to logs.
- **Overlapping Rotation Window:** `STRIPE_WEBHOOK_SECRET` accepts comma- or semicolon-delimited secrets. During quarterly key rotation:
  1. The new secret is appended alongside the active secret (`whsec_NEW,whsec_OLD`).
  2. The gateway verifies each candidate secret sequentially using constant-time comparisons.
  3. Once the 24-hour Stripe transition window concludes, `whsec_OLD` is removed with zero webhook drops or downtime.

### 7.3 Idempotency & Threat Observability
- **Idempotency Guarantee:** Events are indexed in `billing_events` by `stripe_event_id UNIQUE`. Duplicate deliveries respond immediately with `HTTP 200 OK` and execute zero state mutations.
- **Telemetry & Alerting:** Webhook failures increment Prometheus counter `fintext_billing_webhook_errors_total{reason="signature"|"timestamp"|"parse"}`. Any increase in signature failures triggers the critical alert `BillingWebhookSignatureFailures`. Operational procedures are maintained in [`docs/BILLING_RUNBOOK.md`](./BILLING_RUNBOOK.md).

