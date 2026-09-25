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
| **17** | **Readiness Audit Suite Checks** | `24 / 24 Checks Passed` | 100.0% Pass | `scripts/verify_private_beta_readiness.py` | current | 2026-09-25 | Per Commit |
| **18** | **Rust API Gateway Unit Tests** | `494 / 494 Passed` | 100.0% Pass | `rust/api_server/src/lib.rs` | current | 2026-09-25 | Per Commit |
| **19** | **Rust Ingestion Daemon Tests** | `108 / 108 Passed` | 100.0% Pass | `rust/ingestion_engine/` | `a31d508` | 2026-09-24 | Per Commit |
| **20** | **Private Beta Launch Checks** | `45 / 45 Checks Passed` | 100.0% Pass | `docs/PRIVATE_BETA_LAUNCH_CHECKLIST.md` | current | 2026-09-25 | Per Release |
| **21** | **Billing Flow Lifecycle Certification** | `CERTIFIED (7/7 Scenarios)` | 100.0% Pass | `logs/billing_flow_report.json` | current | 2026-09-25 | Per Release |
| **22** | **Monthly Billing Reconciliation** | `RECONCILED (0 Discrepancies)`| Zero Drift | `logs/billing_reconciliation_report.json` | current | 2026-09-25 | Monthly Close |
| **23** | **Webhook Signature Scheme & Tolerance**| `HMAC-SHA256, 300s window` | Constant-Time | `rust/api_server/src/billing.rs` | current | 2026-09-25 | Continuous |


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

---

## 4. Discrepancy Reconciliation Notes

1. **Transaction Cost Terminology:** Historical notes occasionally described the friction model as "15 bps two-way". The mathematical model coded in `scripts/validate_signal_quality.py` enforces a **5.0 bps single-trip execution fee (10.0 bps round-trip)**, producing the verified Net Sharpe Ratio of **1.45**. All documentation is now synchronized to this precise formulation.
2. **P95 Latency Measurement Variance:** Snapshot k6 load benchmarks recorded P95 latencies between `212.4 ms` and `252.41 ms` across various CPU governor states. The authoritative certified baseline recorded in `logs/load_test_report.json` under full 100 VU concurrent execution is **229.64 ms** (comfortably within the 500 ms SLA).
3. **Recovery Time Objective (RTO):** Full database restore drill certified at **0.111 seconds** against an institutional SLA contract commitment of `< 4.0 hours`.
4. **Soak Memory Leak Slope Tolerance:** Under standard 30-day soak monitoring, container RSS slope must remain $< 2.0\text{ MiB/hour}$ or maintain $R^2 < 0.50$ (indicating bounded non-monotonic oscillation rather than systemic leak).
