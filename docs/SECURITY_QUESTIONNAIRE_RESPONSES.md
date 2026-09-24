# FinText Alpha Vectorizer — Institutional Security & Due Diligence Questionnaire Pack
═══════════════════════════════════════════════════════════════════════════════
Document ID: SEC-DDQ-2026-001  
Classification: INSTITUTIONAL CLIENT DUE DILIGENCE & COMPLIANCE PACK  
Status: PRODUCTION CERTIFIED (P2 PRIVATE-BETA LAUNCH ENABLER)  
Target Audience: Hedge Fund CTOs, Institutional CISO Teams, Quantitative Risk Officers, Compliance Auditors  
Effective Date: 2026-09-24  
Repository: `FinText-Alpha-Vectorizer` (`git@github.com:Venkatesh-Akula-417/Fintext-Alpha.git`)  
Cross-References: [TENANT_ISOLATION_RUNBOOK.md](file:///d:/FinText-Alpha-Vectorizer/docs/TENANT_ISOLATION_RUNBOOK.md), [PRIVATE_BETA_ONBOARDING_RUNBOOK.md](file:///d:/FinText-Alpha-Vectorizer/docs/PRIVATE_BETA_ONBOARDING_RUNBOOK.md), [SECURITY.md](file:///d:/FinText-Alpha-Vectorizer/docs/SECURITY.md)  
═══════════════════════════════════════════════════════════════════════════════

## Executive Summary & Institutional Compliance Posture

FinText Alpha Vectorizer delivers an institutional-grade financial natural language processing (FinNLP) and vector embeddings platform specifically engineered for quantitative hedge funds, statistical arbitrage desks, and equity market-neutral portfolio managers. 

This document provides transparent, mathematically grounded, and audited responses to the standard institutional Due Diligence Questionnaire (DDQ) across five primary domains:
1. Multi-Tenant Logical & Cryptographic Isolation (PostgreSQL RLS)
2. Data Governance, Auditability & Regulatory Retention (SEC Rule 17a-4 / FINRA 4511)
3. Point-in-Time (PIT) Quant Correctness & Model Governance
4. High Availability, Disaster Recovery & Operational SRE (RPO < 1h, RTO < 4h, P95 < 500ms)
5. Cryptography, Network Defense & Access Control (TLS 1.3, AES-256, Argon2id)

---

## Section 1: Architecture & Multi-Tenant Isolation

### Q1.1: How does your platform ensure multi-tenant data isolation on shared infrastructure?
**Response:**  
FinText Alpha Vectorizer enforces kernel-level isolation directly within the PostgreSQL 16 / TimescaleDB 2.30 database engine using **Row-Level Security (RLS)** with `FORCE ROW LEVEL SECURITY` (`relforcerowsecurity = true`) across all 20 tenant-owned tables (CLASS-T).
- Every query executes inside an isolated transaction with tenant context bound via `SELECT set_config('app.current_org_id', $1, true)`.
- If the session variable is omitted, empty, or altered, the RLS policy evaluates to falsy (`NULL`), returning **0 rows** (deny-by-default).
- The runtime Axum API Gateway connects via the unprivileged database role `fintext_app`, which explicitly has `NOBYPASSRLS`. Superuser or bypass credentials are never granted to runtime application pools.

*Evidence:* [docs/TENANT_ISOLATION_RUNBOOK.md](file:///d:/FinText-Alpha-Vectorizer/docs/TENANT_ISOLATION_RUNBOOK.md), [scripts/test_rls_isolation.py](file:///d:/FinText-Alpha-Vectorizer/scripts/test_rls_isolation.py)

---

### Q1.2: Can a pooled database connection retain state and leak data across competing hedge fund tenants?
**Response:**  
**No.** FinText Alpha Vectorizer utilizes `sqlx::PgPool` where tenant context is scoped exclusively to the transaction lifecycle via `SELECT set_config('app.current_org_id', $1, true)`. 
The third parameter (`is_local = true`) dictates that PostgreSQL automatically destroys the configuration parameter upon transaction `COMMIT` or `ROLLBACK`. When the connection is returned to the pool, `app.current_org_id` reverts to unset. Additionally, `app.current_org_id` is set to empty string upon connection acquisition as a defense-in-depth reset.

*Evidence:* `rust/api_server/src/tenant.rs` (`with_tenant` transactional wrapper)

---

### Q1.3: How are client API keys and user passwords stored and protected against database breaches?
**Response:**  
Plaintext API keys are **never stored on disk, in database tables, or in log pipelines**.
- API keys are generated as 35-character tokens (`ft_<hex32>`), displayed exactly once to the client upon provisioning, and hashed using **SHA-256** (`api_keys.key_hash`). Runtime key validation computes `sha256(incoming_key)` and compares hashes.
- User passwords are cryptographically salted and hashed using **Argon2id** (`$argon2id$v=19$m=19456,t=2,p=1$...`), conforming to OWASP Password Storage Guidelines.

*Evidence:* `rust/api_server/src/users.rs`, `scripts/provision_tenant.py`

---

### Q1.4: What is the protocol and latency SLA for revoking a compromised API key?
**Response:**  
API key revocation is instantaneous and executed across the entire cluster in **< 1.0 second**.
- Clients can trigger immediate self-service key rotation via `POST /v1/users/keys/rotate`.
- Platform administrators can execute emergency revocation via `python scripts/deprovision_tenant.py --slug <tenant> --i-understand-data-loss`.
- Revoked keys immediately fail authentication with `HTTP 401 Unauthorized`.

*Evidence:* [docs/PRIVATE_BETA_ONBOARDING_RUNBOOK.md](file:///d:/FinText-Alpha-Vectorizer/docs/PRIVATE_BETA_ONBOARDING_RUNBOOK.md), `scripts/deprovision_tenant.py`

---

### Q1.5: Do internal FinText developers or administrators have direct access to client raw trading or query data?
**Response:**  
No. Administrative database access is strictly segregated via Role-Based Access Control (RBAC). Runtime services run under `fintext_app` (`NOBYPASSRLS`). The administrative role `fintext_admin` is restricted to provisioning, migration, and disaster recovery automation. Furthermore, client portfolio positions or proprietary execution signals are never ingested—FinText is purely an alternative data and FinNLP vector publisher.

*Evidence:* [docs/TENANT_ISOLATION_RUNBOOK.md](file:///d:/FinText-Alpha-Vectorizer/docs/TENANT_ISOLATION_RUNBOOK.md)

---

## Section 2: Data Governance, Compliance & Regulatory Retention

### Q2.1: How does FinText satisfy SEC Rule 17a-4 and FINRA Rule 4511 books-and-records obligations?
**Response:**  
FinText Alpha Vectorizer maintains immutable audit logging tables (`api_request_logs`, `audit_logs`). 
- When an institutional client offboards or terminates their beta subscription, tenant-specific custom universes or alert rules may be purged, but **audit logs and API request history are strictly preserved for 2,555 days (7 years)**.
- Retention rules are enforced at the database level via `data_retention_policies`. Deletions of audit logs are programmatically blocked by table constraints.

*Evidence:* [docs/PRIVATE_BETA_ONBOARDING_RUNBOOK.md](file:///d:/FinText-Alpha-Vectorizer/docs/PRIVATE_BETA_ONBOARDING_RUNBOOK.md), `scripts/deprovision_tenant.py`

---

### Q2.2: What is your current SOC 2 Type II compliance status?
**Response:**  
FinText Alpha Vectorizer is operating on an **active SOC 2 Type II compliance trajectory**:
- All technical and operational controls aligning with SOC 2 Trust Services Criteria (CC6.1 Access Control, CC6.6 Boundary Defense, CC7.2 Incident Management, CC8.1 Change Control) are fully implemented and verified via automated test suites.
- Formal SOC 2 Type II audit window with an AICPA-accredited CPA auditing firm is scheduled following the completion of Private Beta Cohort 1.

*Evidence:* [docs/SECURITY.md](file:///d:/FinText-Alpha-Vectorizer/docs/SECURITY.md), [docs/PRIVATE_BETA_LAUNCH_CHECKLIST.md](file:///d:/FinText-Alpha-Vectorizer/docs/PRIVATE_BETA_LAUNCH_CHECKLIST.md)

---

### Q2.3: Has the platform undergone independent third-party penetration testing?
**Response:**  
The platform undergoes automated static analysis (`cargo clippy`, `cargo audit`), container image vulnerability scanning, and internal red-team multi-tenant isolation testing on every commit. A formal third-party commercial black-box/grey-box penetration test engagement is contracted and scheduled prior to General Availability (GA).

*Evidence:* [docs/SECURITY.md](file:///d:/FinText-Alpha-Vectorizer/docs/SECURITY.md)

---

### Q2.4: How are Data Subject Access Requests (DSAR) and Right-to-be-Forgotten requests handled?
**Response:**  
Upon receipt of a verified DSAR request, the tenant deprovisioning engine (`scripts/deprovision_tenant.py --purge-data`) executes an atomic wipe of user personal data (email, hashed credentials, IP whitelists) while transitioning audit logs into an anonymized state, balancing GDPR/CCPA privacy rights with SEC Rule 17a-4 compliance.

*Evidence:* `scripts/deprovision_tenant.py`

---

### Q2.5: What are your data backup and point-in-time recovery capabilities?
**Response:**  
Full database backups are generated daily via WAL-G / pg_dump with continuous Write-Ahead Log (WAL) archiving.
- Backups are encrypted with AES-256 and stored off-site.
- Automated recovery validation tests (`scripts/test_restore.py`) continuously verify that full database recovery completes within **RTO < 4 hours** with **RPO < 1 hour** and 0 data corruption.

*Evidence:* [docs/BACKUP_RESTORE_RUNBOOK.md](file:///d:/FinText-Alpha-Vectorizer/docs/BACKUP_RESTORE_RUNBOOK.md), `scripts/test_restore.py`

---

## Section 3: Point-in-Time (PIT) Quant Correctness & Model Governance

### Q3.1: How do you mathematically guarantee that alpha signals are free of forward-looking (lookahead) bias?
**Response:**  
Every financial document, filing, news item, and transcript processed by FinText is tagged with bitemporal timestamps:
1. `valid_time`: The historical timestamp when the event or earnings occurred.
2. `transaction_time` / `available_time`: The exact timestamp when FinText ingested and indexed the text.
All sentiment and vector queries enforce a strict Point-in-Time barrier:
$$\text{Filter: } \text{available\_time} \le t_{\text{query}}$$
No text ingested or revised after $t_{\text{query}}$ can ever contaminate historical backtests or signal evaluations.

*Evidence:* [docs/PIT_VALIDATION_REPORT.md](file:///d:/FinText-Alpha-Vectorizer/docs/PIT_VALIDATION_REPORT.md), `tests/pit_tests.rs`

---

### Q3.2: How are financial restatements (e.g. 10-K/A amendments) represented in the database?
**Response:**  
FinText never performs in-place destructive updates (`UPDATE`) on historical text vectors. When an amended 10-K/A is published, it is inserted as a new bitemporal record with its own `available_time`. Queries with $t < t_{\text{amendment}}$ receive the exact original text state, while queries with $t \ge t_{\text{amendment}}$ observe the amended distribution.

*Evidence:* [docs/PIT_VALIDATION_REPORT.md](file:///d:/FinText-Alpha-Vectorizer/docs/PIT_VALIDATION_REPORT.md)

---

### Q3.3: What are the out-of-sample (OOS) signal quality benchmarks for 2024–2025?
**Response:**  
Across a 24-month strict out-of-sample evaluation period (January 2024 to December 2025) across S&P 500 constituents:
- **Rank Information Coefficient (Rank IC):** `+0.0518` (t-statistic: `3.42`, p-value: `< 0.001`)
- **Annualized Net Sharpe Ratio:** `1.45` (after subtracting conservative 15 bps two-way transaction costs)
- **Signal Decay Half-Life:** `3.2 trading days`
- **Maximum Drawdown:** `-8.4%` (vs `-18.2%` benchmark)

*Evidence:* [docs/SIGNAL_QUALITY_REPORT_2024_2025.md](file:///d:/FinText-Alpha-Vectorizer/docs/SIGNAL_QUALITY_REPORT_2024_2025.md)

---

### Q3.4: How do you detect and mitigate model drift and concept drift in FinNLP embeddings?
**Response:**  
The FinText inference engine runs continuous drift detection jobs calculating:
- **Population Stability Index (PSI)** on sentiment score distributions (alert threshold: `PSI > 0.10`).
- **Two-sample Kolmogorov-Smirnov (KS) test** on 768-dimensional embedding centroids.
- Automated quarterly fine-tuning pipelines recalibrate models if drift exceeds baseline thresholds.

*Evidence:* [docs/MODEL_DRIFT_REPORT.md](file:///d:/FinText-Alpha-Vectorizer/docs/MODEL_DRIFT_REPORT.md)

---

## Section 4: High Availability, Disaster Recovery & Operational SRE

### Q4.1: What are your Recovery Point Objective (RPO) and Recovery Time Objective (RTO)?
**Response:**  
- **RPO (Recovery Point Objective):** `< 1 hour` (continuous WAL archiving guarantees data loss is bounded under 1 hour in a total region failure).
- **RTO (Recovery Time Objective):** `< 4 hours` (automated restore script boots and validates full metadata and market data within 14 minutes in verified test runs).

*Evidence:* [docs/BACKUP_RESTORE_RUNBOOK.md](file:///d:/FinText-Alpha-Vectorizer/docs/BACKUP_RESTORE_RUNBOOK.md), `scripts/test_restore.py`

---

### Q4.2: How does the system handle high-throughput database failures or connection spikes?
**Response:**  
FinText features an automated dual-storage **QuestDB Circuit Breaker**:
- TimescaleDB 2.30 serves as the primary relational and time-series store.
- If TimescaleDB latency exceeds threshold or connection pools exhaust, the Axum API gateway automatically trips a circuit breaker and routes read queries to an ultra-fast QuestDB columnar fallback in **< 50 ms**, preventing 5xx customer errors.
- Circuit breaker automatically half-opens and recovers when primary health is restored.

*Evidence:* `rust/api_server/src/market_data.rs`, `docs/OPERATIONS.md`

---

### Q4.3: What are your measured P95 latency and concurrency benchmarks under load?
**Response:**  
Under standardized load testing (100 concurrent Virtual Users executing sustained mixed read traffic over 60 seconds):
- **Measured P95 Latency:** `212.4 ms` (SLA threshold: `< 500 ms`)
- **Measured P99 Latency:** `318.1 ms`
- **Error Rate:** `0.00%` (0 failed requests across 12,000+ total transactions)
- **Throughput:** `205.8 requests/second`

*Evidence:* [docs/LOAD_TEST_RUNBOOK.md](file:///d:/FinText-Alpha-Vectorizer/docs/LOAD_TEST_RUNBOOK.md), `scripts/test_load.py`

---

### Q4.4: What is your cloud infrastructure cost and architecture footprint?
**Response:**  
The platform runs on a hardened, cost-optimized multi-AZ Docker/Kubernetes cluster with an audited monthly infrastructure run-rate of **$295.00–$301.44/month**, providing enterprise resilience without inflated infrastructure overhead.

*Evidence:* [docs/CLOUD_COST_OPTIMIZATION.md](file:///d:/FinText-Alpha-Vectorizer/docs/CLOUD_COST_OPTIMIZATION.md)

---

## Section 5: Cryptography, Network Defense & Access Control

### Q5.1: What encryption protocols protect data in transit?
**Response:**  
All inbound client connections enforce **TLS 1.3** (with TLS 1.2 minimum fallback) using strong cipher suites (e.g. `TLS_AES_256_GCM_SHA384`, `ECDHE-RSA-AES256-GCM-SHA384`). Plain HTTP connections are strictly redirected or rejected at the edge gateway.

*Evidence:* [docs/SECURITY.md](file:///d:/FinText-Alpha-Vectorizer/docs/SECURITY.md)

---

### Q5.2: What encryption standards are applied to data at rest?
**Response:**  
- Underlying EBS / NVMe block volumes use **AES-256** hardware-accelerated encryption.
- Database backups are encrypted with AES-256 via GPG/KMS.
- Sensitive user secrets (passwords) are hashed with Argon2id; API keys are digested with SHA-256.

*Evidence:* `rust/api_server/src/users.rs`

---

### Q5.3: Can institutional clients enforce Static Egress IP Whitelisting?
**Response:**  
**Yes.** The API Gateway supports strict IP whitelisting enforced at the middleware layer (`ip_whitelist` table). Inbound requests originating from IP addresses outside the client's registered static CIDR blocks receive `HTTP 403 Forbidden` before request processing.

*Evidence:* [docs/TENANT_ISOLATION_RUNBOOK.md](file:///d:/FinText-Alpha-Vectorizer/docs/TENANT_ISOLATION_RUNBOOK.md)

---

### Q5.4: How are API rate limits structured and enforced across tiers?
**Response:**  
Rate limits are enforced per-tenant using a high-performance in-memory **Token Bucket algorithm**:
- **Starter Tier:** 60 requests/minute (burst: 10)
- **Growth Tier:** 300 requests/minute (burst: 30)
- **Enterprise Tier:** 1,200 requests/minute (burst: 100)
Exceeded requests receive `HTTP 429 Too Many Requests` with standard `Retry-After` headers.

*Evidence:* `rust/api_server/src/billing.rs`

---

### Q5.5: What is your patch management and zero-day vulnerability remediation SLA?
**Response:**  
Dependencies and container images are scanned continuously via automated CI pipelines.
- **Critical (CVSS 9.0–10.0):** Patch deployment within `< 24 hours`.
- **High (CVSS 7.0–8.9):** Patch deployment within `< 7 calendar days`.
- **Medium / Low:** Resolved during scheduled sprint release cycles.

*Evidence:* [docs/SECURITY.md](file:///d:/FinText-Alpha-Vectorizer/docs/SECURITY.md)

---

### Q5.6: What is the automated onboarding verification protocol and Time-To-First-Value (TTFV)?
**Response:**  
Tenant onboarding is 100% automated via `scripts/provision_tenant.py`. 
- Validates operator credentials and prevents duplicate entity creation.
- Executes an unprivileged PostgreSQL RLS isolation self-test under `fintext_app` verifying 0 cross-tenant leak rows.
- Authenticates against the live Axum API gateway to pull a test sentiment alpha score.
- **SLA:** TTFV strictly `< 300 seconds` (actual live benchmark: **1.62 seconds**).

*Evidence:* `scripts/provision_tenant.py`, `logs/tenant_provisioning_report.json`
