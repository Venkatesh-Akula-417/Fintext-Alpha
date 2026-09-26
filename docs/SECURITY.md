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

---

## 8. Model Supply-Chain Integrity & Cryptographic Release Verification

FinText enforces strict machine learning supply-chain governance to prevent model poisoning, weight tampering, and supply-chain disruptions:
- **Zero Binary Weights in Git Tree:** To adhere to repository hygiene and avoid Git LFS bandwidth overheads, zero raw model weight binaries (`.bin`, `.pt`, `.onnx`) are tracked in the version control tree.
- **Cryptographic Release Artifacts:** Production model weights (`finbert-finetuned-v1.0.0.zip`) and research entity models (`ner-v1.0.0.zip`) are published strictly as immutable GitHub Release assets under annotated tag `model-assets-v1.0.0`.
- **Cryptographic Checksum Ledger:** Every released asset is bound to an immutable SHA256 checksum recorded in [`models_release_v1/SHA256SUMS.txt`](../models_release_v1/SHA256SUMS.txt) and [`docs/MODEL_ASSETS.md`](./MODEL_ASSETS.md):
  - `finbert-finetuned-v1.0.0.zip`: `182788c0e554ac3891e88b09f98c6b92ab6cd85f5b1c7c039b1d561049b86d33`
  - `ner-v1.0.0.zip`: `d6ff862b8b2ae293bea50b3dd0058d0facf7d9e154f94ad7619a92e940f9bbdc`
- **Automated Verification:** Production deployment pipelines and CI test harnesses verify the SHA256 checksum both pre-download and post-extraction prior to mounting weights into runtime inference memory. Any hash divergence triggers immediate container launch termination.
- **Permissive Open-Source Licensing:** Base architectures are verified as Apache-2.0 (`ProsusAI/finbert`) and MIT (`dslim/bert-base-NER`), ensuring unencumbered institutional commercial operation.

---

## 9. Network Ingress Confinement & AWS Infrastructure Defense-in-Depth

FinText enforces defense-in-depth network confinement across both cloud infrastructure (AWS) and application layers (Rust Axum middleware):

### 9.1 Application Layer: Per-Tenant IP & CIDR Whitelisting
- **Middleware Boundary:** Requests passing Bearer JWT or API Key authentication enter `ip_whitelist_middleware` before reaching route handlers.
- **Dynamic CIDR Evaluation:** Evaluates client source IP against active CIDR subnets registered for the user/organization in sub-microsecond in-memory `DashMap` cache backed by PostgreSQL.
- **Fail-Secure 403 Response:** If $\ge 1$ entry exists and the source IP does not match, request terminates immediately with `403 Forbidden`: `{"error": "Forbidden", "message": "IP address not allowed"}`.
- **Operational Exemption:** Public health probes (`/v1/health`, `/v1/status`) and billing webhooks (`/v1/billing/webhook`) are decoupled from tenant whitelists.

### 9.2 Infrastructure Layer: AWS VPC & Security Groups
- **Isolated VPC Topology:** Compute host resides in public subnet with Elastic IP; managed RDS TimescaleDB resides strictly in private subnets across dual Availability Zones.
- **Security Group Chaining:** RDS security group allows TCP port 5432 ingress **strictly** from the EC2 security group ID (`aws_security_group.ec2_sg.id`). Direct public Internet access to the database is physically impossible at the hypervisor level.
- **S3 & KMS Envelope Encryption:** All backup and Parquet archive buckets enforce `block_public_acls = true`, `block_public_policy = true`, versioning, and SSE-KMS customer master key envelope encryption.

---

## 10. Usage & Audit Surfaces Security, Admin Token Gating, and Key Redaction

### 10.1 Tenant Self-Service Scoping (`GET /v1/account/usage`)
The customer-facing usage endpoint enforces multi-tenant confinement at the database kernel level:
- **Transaction-Local RLS Scoping:** Every query executes within a PostgreSQL transaction where `set_config('app.current_org_id', clean_org, true)` is applied. Tenant A’s credentials physically cannot return records belonging to Tenant B.
- **Credential Redaction (Safety Constraint S-1):** The endpoint lists tenant API keys strictly by their safe public prefix (e.g. `fta_live_...` or `ak_live_...`), human-readable label, creation timestamp, and active status. Raw key tokens and Argon2/HMAC hashes are excluded from DTO serialization by design.

### 10.2 Administrative Support Surface & Accountability (`GET /v1/admin/tenants/{org_id}/usage`)
- **Header Token-Gated:** Protected by `X-Admin-Token` using constant-time comparison (`subtle::ConstantTimeEq`) against the master operations credential (`ADMIN_TOKEN`).
- **Accountability Audit Trail (Safety Constraint S-2):** Every administrative diagnostic call creates an immutable record in `audit_logs` (`event_type: "admin.tenant_usage_view"`), recording the administrator identity, target `org_id`, and UTC timestamp to guarantee internal support accountability.
- **Internal-Only Telemetry:** Ingestion telemetry endpoints (`fintext-ingestion:9102/metrics`) are bound to internal container networking and strictly blocked from public security groups.




