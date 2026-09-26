# FinText Alpha Vectorizer — Certified Metrics & Key Parameters Register
═══════════════════════════════════════════════════════════════════════════════
Document ID: REGISTER-METRICS-001  
Classification: AUTHORITATIVE SINGLE SOURCE OF TRUTH (P-A / P-B PRINCIPLES)  
Status: INSTITUTIONAL PRODUCTION CERTIFIED  
Effective Date: 2026-09-24  
Audience: Quantitative Clients, Institutional Procurement, SRE Leads, Compliance Auditors  
Repository: `FinText-Alpha-Vectorizer` (`git@github.com:Venkatesh-Akula-417/Fintext-Alpha.git`)  
═══════════════════════════════════════════════════════════════════════════════

## 1. Operating Principle: Single Source of Truth

In high-assurance quantitative systems, disparate documents frequently suffer from numeric drift when historical test snapshots are cited out of context. To prevent audit inconsistencies, this **Certified Metrics Register** establishes the definitive, binding values for all performance, latency, capacity, signal quality, and disaster recovery metrics.

**Mandatory Rule for All Documentation:**
> All documentation files (including guides, whitepapers, runbooks, and marketing materials) **MUST** cite the authoritative values published in this register or provide a direct link. Individual documentation files must not maintain hardcoded, conflicting estimates.

---

## 2. Authoritative Metrics Register Table

| # | Metric Category & Name | Certified Value | Contractual SLA | Authoritative Source Artifact | Certifying Git Commit | Certified Date (UTC) | Re-Certification Cadence |
| :-: | :--- | :--- | :--- | :--- | :-: | :-: | :--- |
| **01** | **API P50 Latency** | `196.15 ms` | Informational | `logs/load_test_report.json` | `1887219` | 2026-09-24 | Bi-Weekly |
| **02** | **API P95 Latency** | `229.64 ms` | $\le 500\text{ ms}$ | `logs/load_test_report.json` | `1887219` | 2026-09-24 | Bi-Weekly |
| **03** | **API P99 Latency** | `240.35 ms` | $\le 1000\text{ ms}$ | `logs/load_test_report.json` | `1887219` | 2026-09-24 | Bi-Weekly |
| **04** | **Load Capacity & Concurrency** | `100 VUs @ 120.65 rps` | $\ge 10\text{ rps}, \text{Err} < 1\%$ | `logs/load_test_report.json` | `26e5e05` | 2026-09-24 | Monthly |
| **05** | **Load Test Error Rate** | `0.00% (0 errors)` | $< 1.0\%$ | `logs/load_test_report.json` | `1887219` | 2026-09-24 | Bi-Weekly |
| **06** | **Multi-Tenant RLS Confinement** | `0 Leak Rows (20 tables)` | Strict 0 Leaks | `logs/rls_isolation_report.json` | `1887219` | 2026-09-24 | Per Commit |
| **07** | **Disaster Recovery RTO** | `0.111 s` | $< 4\text{ hours}$ | `logs/backup_restore_test_report.json`| `79d4c50` | 2026-09-23 | Weekly Drill |
| **08** | **Disaster Recovery RPO** | $\le 1\text{ hour (continuous)}$ | $\le 1\text{ hour}$ | `logs/backup_restore_test_report.json`| `79d4c50` | 2026-09-23 | Continuous |
| **09** | **Signal Quality OOS Rank IC** | `+0.0518` (5-day) | $\ge +0.0500$ | `logs/signal_quality_report.json` | `4d001bf` | 2026-09-24 | Monthly |
| **10** | **Signal Quality OOS ICIR** | `1.60` | $\ge 1.50$ | `logs/signal_quality_report.json` | `4d001bf` | 2026-09-24 | Monthly |
| **11** | **Net Sharpe Ratio (OOS 24–25)**| `1.45` | $\ge 1.40$ | `logs/signal_quality_report.json` | `4d001bf` | 2026-09-24 | Monthly |
| **12** | **Slippage & Friction Standard**| `5 bps single / 10 bps round` | Realistic Execution | `logs/signal_quality_report.json` | `4d001bf` | 2026-09-24 | Static Standard |
| **13** | **Signal Half-Life** | `4.8 trading days` | $\ge 3.0\text{ days}$ | `logs/signal_quality_report.json` | `4d001bf` | 2026-09-24 | Monthly |
| **14** | **Signal Alpha Decay vs IS** | `4.07%` | $< 50.0\%$ | `logs/signal_quality_report.json` | `4d001bf` | 2026-09-24 | Monthly |
| **15** | **Time-To-First-Value (TTFV)** | `1.62 s` | $< 300\text{ s}$ | `logs/tenant_provisioning_report.json`| `a31d508` | 2026-09-24 | Per Onboarding |
| **16** | **Cloud Run-Rate Budget** | `$295.00–$301.44 / mo` | $\le \$310 / \text{mo}$ | `docs/CLOUD_COST_OPTIMIZATION.md` | `a31d508` | 2026-09-24 | Monthly Audit |
| **17** | **Readiness Audit Suite Checks** | `30 / 30 Checks Passed` | 100.0% Pass | `scripts/verify_private_beta_readiness.py` | current | 2026-09-26 | Per Commit |
| **18** | **Rust API Gateway Unit Tests** | `514 / 514 Passed` | 100.0% Pass | `rust/api_server/src/lib.rs` | current | 2026-09-26 | Per Commit |
| **19** | **Rust Ingestion Daemon Tests** | `121 / 121 Passed` | 100.0% Pass | `rust/ingestion_engine/` | current | 2026-09-26 | Per Commit |
| **20** | **Private Beta Launch Checks** | `52 / 52 Checks Passed` | 100.0% Pass | `docs/PRIVATE_BETA_LAUNCH_CHECKLIST.md` | current | 2026-09-26 | Per Release |
| **21** | **Billing Flow Lifecycle Certification** | `CERTIFIED (7/7 Scenarios)` | 100.0% Pass | `logs/billing_flow_report.json` | current | 2026-09-25 | Per Release |
| **22** | **Monthly Billing Reconciliation** | `RECONCILED (0 Discrepancies)`| Zero Drift | `logs/billing_reconciliation_report.json` | current | 2026-09-25 | Monthly Close |
| **23** | **Webhook Signature Scheme & Tolerance**| `HMAC-SHA256, 300s window` | Constant-Time | `rust/api_server/src/billing.rs` | current | 2026-09-25 | Continuous |
| **24** | **Model Assets Release Distribution** | `model-assets-v1.0.0 (3 assets)`| SSoT Tagged Release | `docs/MODEL_ASSETS.md` | current | 2026-09-25 | Per Model Version |
| **25** | **Repository Root Waste Count** | `0 Waste Files (Audited)` | Clean SSoT | `docs/REPO_HYGIENE_AUDIT.md` | current | 2026-09-25 | Continuous |
| **26** | **AWS Production IaC Deployment Certification** | `CERTIFIED (28/28 Resources)` | 0 Errors, 0 Warnings | `logs/infra_validate_report.json` | current | 2026-09-25 | Per Release |
| **27** | **AWS Production Monthly Cost Invariant** | `$281.49 / mo (c6i) / $195.64 / mo (Graviton)` | $\le \$301.44 / \text{mo}$ | `docs/PRODUCTION_DEPLOYMENT_RUNBOOK.md` | current | 2026-09-25 | Monthly Audit |
| **28** | **Tenant Usage & API Key Audit Endpoint** | `CERTIFIED (RLS-Scoped, 0 Leaks)` | 100.0% Pass | `rust/api_server/src/billing.rs` | current | 2026-09-26 | Per Release |
| **29** | **Ingestion Per-Source Lag P95 Telemetry** | `0.185 s (SEC EDGAR) / 0.042 s (Finnhub WS)` | $< 1.000\text{ s}$ | `rust/ingestion_engine/src/telemetry/metrics.rs` | current | 2026-09-26 | Continuous |
| **30** | **Assurance Pack Manifest** | `buildable, SHA256-pinned (v1)` | Informational | `logs/auditor_pack_manifest.json` | current | 2026-09-26 | Per Commit |
| **31** | **HA GA Cutover Plan** | `staged vars default-false, cap unchanged $301.44 pending founder decision` | Static Budget | `docs/HA_MULTIAZ_GA_CUTOVER_PLAN.md` | current | 2026-09-26 | Per Release |
| **32** | **Point-in-Time DR Restore Drill** | `CERTIFIED_HEALTHY (RTO=4.93s, 0 Leaks)` | $< 14,400\text{ s}$ (4h) | `logs/dr_report.json` | current | 2026-09-26 | Weekly Drill |
| **33** | **RLS Penetration & Boundary Defense** | `CERTIFIED (44/44 Vectors Pass, 0 Leaks)` | Strict 0 Leaks | `logs/rls_isolation_report.json` | current | 2026-09-26 | Per Commit |
| **34** | **Tenant API-Key Self-Service Lifecycle** | `CERTIFIED (Max 10 Keys, 0 Secret Leaks)` | 100.0% Pass | `rust/api_server/src/billing.rs` | current | 2026-09-26 | Per Release |
| **35** | **Quota Transparency Headers** | `RFC 6585 Compliant (Reset=Unix Epoch Sec)` | Informational | `rust/api_server/src/rate_limit.rs` | current | 2026-09-26 | Continuous |
| **36** | **Circuit Breaker Thresholds & Chaos Recovery** | `CERTIFIED (6/6 Scenarios Pass, 0 Gaps)` | Fast-Fail <= 300s Backoff | `logs/feed_chaos_report.json` | current | 2026-09-26 | Per Release |
| **37** | **Upstream Fallback Lag Budgets** | `Finnhub 2.0s REST / Polygon 5.0s Snapshot` | $P_{95} \le 5.000\text{ s}$ degraded | `rust/ingestion_engine/src/main.rs` | current | 2026-09-26 | Continuous |

---

## 3. Metric Breakdown & Verification Methodology

### 3.1 Latency & Capacity (Metrics 01–05)
- **Mathematical Definition:** End-to-end HTTP request duration measured from initial TCP handshake completion to final response byte delivery across the Axum 0.7 REST gateway.
- **Verification Method:** Multi-phase k6 load testing suite executing across 100 virtual users (VUs) sustaining 120+ requests per second under Point-in-Time sentiment and batch query workloads.
- **SLA Commitment:** $P_{95} \le 500\text{ ms}$, $P_{99} \le 1000\text{ ms}$, Error Rate $< 1.0\%$.
- **Source Artifact:** `logs/load_test_report.json`

### 3.2 Tenant Isolation & Row-Level Security (Metric 06)
- **Mathematical Definition:** Cross-tenant leakage count $\sum \text{Rows}_{\text{unauthorized}}$ across all queries executed under authenticated tenant GUC context `app.current_org_id`.
- **Verification Method:** Exhaustive penetration test script (`scripts/test_rls_isolation.py`) cycling through authenticated Org A, Org B, unauthenticated sessions, and malformed JWT contexts across 20 CLASS-T tables with PostgreSQL RLS FORCED.
- **SLA Commitment:** Strictly $0\text{ rows}$ (0.00% leakage rate).
- **Source Artifact:** `logs/rls_isolation_report.json`

### 3.3 Disaster Recovery & Continuity (Metrics 07–08)
- **Mathematical Definition:**
  - Recovery Time Objective (RTO): Elapsed time from disaster declaration to cluster query restoration.
  - Recovery Point Objective (RPO): Maximum data age lost during catastrophic failure.
- **Verification Method:** Automated backup creation, volume drop, and PITR restoration drill executing `scripts/test_backup_restore.py`.
- **SLA Commitment:** $\text{RTO} < 4.0\text{ hours}$, $\text{RPO} \le 1.0\text{ hour}$.
- **Source Artifact:** `logs/backup_restore_test_report.json`

### 3.4 Signal Quality & Quantitative Integrity (Metrics 09–14)
- **Mathematical Definition:**
  - Rank Information Coefficient: $\text{Rank IC} = \text{Corr}_{\text{Spearman}}(S_{t}, R_{t+5d})$.
  - Net Sharpe Ratio: $\text{Sharpe}_{\text{Net}} = \frac{\mathbb{E}[R_{\text{portfolio}} - c_{\text{trans}}]}{\sigma(R_{\text{portfolio}})} \cdot \sqrt{252}$, where $c_{\text{trans}} = 5.0\text{ bps single-trip (10.0 bps round-trip)}$.
- **Verification Method:** Strict out-of-sample (OOS) evaluation script (`scripts/validate_signal_quality.py`) across 2024–2025 S&P 500 constituents with strict point-in-time publication timestamp filtering.
- **SLA Commitment:** $\text{Rank IC} \ge +0.0500$, $\text{ICIR} \ge 1.50$, $\text{Net Sharpe} \ge 1.40$.
- **Source Artifact:** `logs/signal_quality_report.json`

### 3.5 Onboarding Velocity & Platform Cost (Metrics 15–16)
- **Mathematical Definition:**
  - Time-To-First-Value (TTFV): Elapsed duration from tenant provisioning invocation to first successful authenticated sentiment query.
  - Cloud Run-Rate: Aggregated monthly hosting expense across Hetzner bare-metal, Cloudflare CDN, AWS S3 cold archive, and TimescaleDB compute.
- **Verification Method:** Automated tenant onboarding script (`scripts/provision_tenant.py`) and cost audit manifest (`docs/CLOUD_COST_OPTIMIZATION.md`).
- **SLA Commitment:** $\text{TTFV} < 300\text{ seconds}$, Budget $\le \$310.00/\text{month}$.
- **Source Artifact:** `logs/tenant_provisioning_report.json`, `docs/CLOUD_COST_OPTIMIZATION.md`

### 3.6 Automated Test Coverage & Verification (Metrics 17–20)
- **Mathematical Definition:** Aggregate pass rate across all Rust cargo unit/integration tests and python platform readiness verification suites.
- **Verification Method:** Execution of `cargo test --all-targets` and `python scripts/verify_private_beta_readiness.py`.
- **SLA Commitment:** 100.0% clean pass rate with zero test failures or warnings.
- **Source Artifact:** Cargo test runner output and `docs/PRIVATE_BETA_LAUNCH_CHECKLIST.md`.

### 3.7 Revenue Assurance & Billing Reconciliation (Metrics 21–23)
- **Mathematical Definition:**
  - Billing Flow Lifecycle: Complete verification across 7 state machine scenarios (checkout, dunning failure, idempotency, tampered signature, replayed timestamp, grace expiration suspension, and reactivation).
  - Billing Reconciliation: Discrepancy count $\sum (\text{OverLimitUnbilled} + \text{DriftStripeDB})$ comparing `usage_events` metering against subscription plan quotas and `billing_events` invoice payment records.
  - Signature Verification: HMAC-SHA256 evaluation with $|t_{\text{now}} - t_{\text{event}}| \le 300\text{ seconds}$ and constant-time hex comparison.
- **Verification Method:** Automated test execution via `scripts/test_billing_flow.py` and `scripts/reconcile_billing.py`.
- **SLA Commitment:** Verdict `CERTIFIED` and `RECONCILED` with 0 discrepancies and zero customer data loss under Phase-1 suspension.
- **Source Artifact:** `logs/billing_flow_report.json`, `logs/billing_reconciliation_report.json`, and `docs/BILLING_RUNBOOK.md`.

### 3.8 Tenant Operations Visibility & Ingestion Telemetry (Metrics 28–29)
- **Mathematical Definition:**
  - Tenant Usage & Audit Scoping: Absolute containment of tenant queries to `app.current_org_id` with 0 cross-tenant rows returned and zero API key hash or raw secret leakage.
  - Ingestion Event Lag P95: $\text{Lag} = t_{\text{db\_commit}} - t_{\text{source\_published}}$, sampled across sub-second histogram buckets $[0.005, 0.010, 0.025, 0.050, 0.100, 0.250, 0.500, 1.000, 2.000]$ seconds.
- **Verification Method:** Execution of `cargo test -p fintext_api_server --lib` (usage scoping tests), `cargo test -p fintext_ingestion_engine`, and Prometheus histogram scrape from internal port 9102.
- **SLA Commitment:** $P_{95}\text{ Lag} < 1.000\text{ s}$ across streaming feeds and 100.0% clean RLS containment.
- **Source Artifact:** `rust/api_server/src/billing.rs`, `rust/ingestion_engine/src/telemetry/metrics.rs`, and `dashboards/tenant_usage.json`.

### 3.9 Assurance & GA-Path Governance (Metrics 30–31)
- **Mathematical Definition:**
  - SOC 2 Evidence Integrity: Deterministic SHA-256 fingerprinting across all 28 declared Trust Services Criteria artifacts with 0 missing items.
  - High Availability Cutover Readiness: Zero-drift Terraform variable defaults (`rds_multi_az=false`, `ha_compute_enabled=false`, `alb_enabled=false`) preserving the baseline $281.49/mo cost invariant and $301.44/mo cap until formal founder authorization.
- **Verification Method:** Execution of `python scripts/build_auditor_pack.py` and `terraform -chdir=infra/terraform validate`.
- **SLA Commitment:** 100.0% evidence artifact presence and zero unbudgeted infrastructure cost increases.
- **Source Artifact:** `docs/SOC2_AUDITOR_PACK.md`, `logs/auditor_pack_manifest.json`, and `docs/HA_MULTIAZ_GA_CUTOVER_PLAN.md`.

### 3.10 Disaster Recovery Drill & RLS Boundary Defense (Metrics 32–33)
- **Mathematical Definition:**
  - Ephemeral Container Restore RTO: Elapsed wall-clock seconds from drill invocation to cluster health confirmation across 28 restored tables: $\text{RTO} \le 14,400\text{ s}$ (4 hours contractual SLA).
  - Cross-Tenant Boundary Defense: Penetration matrix across 44 automated test vectors evaluating `fintext_app` (`NOBYPASSRLS`) containment, default-deny empty sets, and `WITH CHECK OPTION` write isolation.
- **Verification Method:** Execution of `python scripts/run_dr_drill.py` (measured 4.928s) and `python scripts/test_rls_isolation.py` (44/44 pass, 0 leak rows).
- **SLA Commitment:** $\text{RTO} < 14,400\text{ s}$, 0 cross-tenant leak rows across all tables.
- **Source Artifact:** `logs/dr_report.json`, `logs/backup_ledger.md`, and `logs/rls_isolation_report.json`.

### 3.11 Tenant Key Lifecycle & Quota Transparency (Metrics 34–35)
- **Mathematical Definition:**
  - Key Entropy & Isolation: 256 bits of CSPRNG entropy (`fintext_live_` + 32-byte hexadecimal token), SHA-256 salted digest storage, maximum 10 active keys per tenant, strictly scoped via `app.current_org_id`.
  - Quota Transparency Emission: RFC 6585 header tuple `X-RateLimit-Limit`, `X-RateLimit-Remaining`, and `X-RateLimit-Reset` where Reset emits Unix epoch timestamp in UTC seconds: $t_{\text{reset}} = t_{\text{epoch}} + \Delta t_{\text{window}}$.
- **Verification Method:** Unit and integration tests in `rust/api_server/src/billing.rs`, `rust/api_server/src/rate_limit.rs`, and Python SDK `python_sdk/tests/test_account_keys.py`.
- **SLA Commitment:** 100.0% clean pass rate, zero plaintext credential leakage in logs or database, exact Unix epoch integer headers.
- **Source Artifact:** `rust/api_server/src/billing.rs`, `rust/api_server/src/rate_limit.rs`, and `docs/API_CUSTOMER_GUIDE.md`.

### 3.12 Ingestion Circuit Breakers & Upstream Fallbacks (Metrics 36–37)
- **Mathematical Definition:**
  - Circuit Breaker State Transition: Pure-logic state machine per feed source (`Closed` -> `Open` on 5 consecutive failures or $\ge 50\%$ errors over 60s sliding window; `Open` fast-rejects with exponential backoff $30\text{s} \times 2^k \le 300\text{s}$; `HalfOpen` admits 1 canary probe; success -> `Closed`, failure -> `Open`).
  - Upstream Degradation Fallback Budgets: Streaming feed websocket outages degrade cleanly into labeled REST pollers (`finnhub_ws` $\to$ REST 2s polling; `polygon_ws` $\to$ REST 5s snapshots; `sec_edgar` $\to$ indexed backoff + stale marker at 10m; `fomc` $\to$ cached calendar stale=true $\le$ once/5m; `corporate_actions` $\to$ previous-day parquet replay stale=true $\le$ once/5m). Every event carries explicit mode label `primary`, `degraded`, or `stale`.
- **Verification Method:** Unit tests in `rust/ingestion_engine/src/resilience/breaker.rs` (121/121 lib tests pass), automated chaos harness `scripts/run_feed_chaos.py` (6/6 scenarios certified), and internal provider telemetry endpoint `GET http://127.0.0.1:9102/providers`.
- **SLA Commitment:** Zero unhandled crashes, zero silent data gaps, fallback lag bounded by documented SLA, and 100% mode labeling on `fetch_duration_seconds` and `event_lag_seconds`.
- **Source Artifact:** `logs/feed_chaos_report.json`, `logs/feed_chaos_ledger.md`, `dashboards/feed_resilience.json`, and `docs/FEED_RESILIENCE_RUNBOOK.md`.

---

## 4. Discrepancy Reconciliation Notes

1. **Transaction Cost Terminology:** Historical notes occasionally described the friction model as "15 bps two-way". The mathematical model coded in `scripts/validate_signal_quality.py` enforces a **5.0 bps single-trip execution fee (10.0 bps round-trip)**, producing the verified Net Sharpe Ratio of **1.45**. All documentation is now synchronized to this precise formulation.
2. **P95 Latency Measurement Variance:** Snapshot k6 load benchmarks recorded P95 latencies between `212.4 ms` and `252.41 ms` across various CPU governor states. The authoritative certified baseline recorded in `logs/load_test_report.json` under full 100 VU concurrent execution is **229.64 ms** (comfortably within the 500 ms SLA).
3. **Recovery Time Objective (RTO):** Full database restore drill certified at **0.111 seconds** against an institutional SLA contract commitment of `< 4.0 hours`.
4. **Soak Memory Leak Slope Tolerance:** Under standard 30-day soak monitoring, container RSS slope must remain $< 2.0\text{ MiB/hour}$ or maintain $R^2 < 0.50$ (indicating bounded non-monotonic oscillation rather than systemic leak).
