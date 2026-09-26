# FinText Alpha Vectorizer — SOC 2 Auditor Evidence Pack & Control Mapping
═══════════════════════════════════════════════════════════════════════════════
Document ID: SOC2-AUDITOR-PACK-2026-V1  
Classification: INSTITUTIONAL COMPLIANCE & AUDIT WORKING PAPER  
Target Platform: FinText Alpha Vectorizer v1.0.0-rc1  
Reference Standards: AICPA Trust Services Criteria (Security CC6.x/CC7.x/CC8.x, Availability A1.x, Risk CC3.x)  
Effective Date: September 26, 2026  
Responsible Roles: Compliance & Regulatory Officer & Chief Information Security Officer (CISO)  
═══════════════════════════════════════════════════════════════════════════════

> [!IMPORTANT]
> **DISCLAIMER & SCOPE STATEMENT**:  
> **Evidence index for auditor convenience; not a SOC2 certification claim.**  
> This document and the associated automated tooling (`scripts/build_auditor_pack.py`) index existing, cryptographically verified technical artifacts within the FinText codebase to assist institutional compliance officers and third-party auditors during vendor risk assessments and SOC 2 Type II evaluations.

---

## 1. Overview & Verification Architecture

In high-assurance institutional financial technology, static compliance claims are insufficient. FinText Alpha Vectorizer implements **Evidence-as-Code**, where every control assertion is grounded in committed source files, immutable database policies, automated test suites, or execution ledgers.

The platform provides a deterministic evidence generator:
```bash
python scripts/build_auditor_pack.py
```
This utility crawls all declared control artifacts, computes SHA-256 cryptographic hashes, links Git HEAD commit lineage and CI workflow runs, and produces a tamper-evident audit manifest at [`logs/auditor_pack_manifest.json`](file:///d:/FinText-Alpha-Vectorizer/logs/auditor_pack_manifest.json).

---

## 2. Trust Services Criteria Control-to-Evidence Matrix

The table below maps AICPA SOC 2 Trust Services Criteria to exact repository files and verification artifacts:

```
┌────────────────────────────────────────────────────────────────────────────────────────┐
│                        SOC 2 TRUST SERVICES CRITERIA EVIDENCE MAP                     │
├─────────┬──────────────────────────────┬───────────────────────────────────────────────┤
│ TSC Ref │ Control Objective            │ Primary Implementation & Evidence Artifact    │
├─────────┼──────────────────────────────┼───────────────────────────────────────────────┤
│ CC6.1   │ Logical Access & Auth        │ rust/api_server/src/auth.rs (JWT validation)  │
│         │                              │ config/timescale/03-row-level-security.sql    │
│         │                              │ logs/rls_isolation_report.json (0 leak rows)  │
├─────────┼──────────────────────────────┼───────────────────────────────────────────────┤
│ CC6.3   │ Role-Based Boundary & Scoping│ rust/api_server/src/billing.rs (Usage RLS)    │
│         │                              │ rust/api_server/src/ip_whitelist.rs (403 CIDR)│
│         │                              │ logs/billing_flow_report.json (7/7 pass)      │
├─────────┼──────────────────────────────┼───────────────────────────────────────────────┤
│ CC6.6   │ Infrastructure & Network     │ infra/terraform/security_groups.tf (SG chains)│
│         │ Defense-in-Depth             │ infra/terraform/iam.tf (least privilege)      │
│         │                              │ docs/SECURITY.md (system threat model)        │
├─────────┼──────────────────────────────┼───────────────────────────────────────────────┤
│ CC7.1   │ Telemetry, Health & Alarms   │ infra/terraform/cloudwatch.tf (Alarms/Logs)   │
│         │                              │ rust/ingestion_engine/telemetry/metrics.rs    │
│         │                              │ dashboards/tenant_usage.json (Grafana panels) │
├─────────┼──────────────────────────────┼───────────────────────────────────────────────┤
│ CC7.2   │ Continuous Operational Audit │ .github/workflows/status-page.yml (Telemetry) │
│         │                              │ k8s/observability/prometheus-alerts.yaml      │
├─────────┼──────────────────────────────┼───────────────────────────────────────────────┤
│ CC8.1   │ Change Management & Quality  │ docs/PRIVATE_BETA_LAUNCH_CHECKLIST.md (49 chk)│
│         │ Gate Enforcement             │ scripts/verify_private_beta_readiness.py      │
│         │                              │ .github/workflows/ci.yml (6 green workflows)  │
│         │                              │ docs/CERTIFIED_METRICS_REGISTER.md (SSoT)     │
├─────────┼──────────────────────────────┼───────────────────────────────────────────────┤
│ A1.2    │ System Availability & DR     │ logs/backup_restore_test_report.json (RTO .11)│
│         │                              │ logs/soak_ledger.md (memory leak proof)       │
│         │                              │ infra/terraform/s3.tf (lifecycle & Glacier)   │
│         │                              │ docs/HA_MULTIAZ_GA_CUTOVER_PLAN.md (HA plan)  │
├─────────┼──────────────────────────────┼───────────────────────────────────────────────┤
│ CC3.1   │ Risk Assessment & Planning   │ docs/LATENCY_AND_COLOCATION_DECISION.md       │
│ CC3.2   │                              │ docs/PRODUCTION_DEPLOYMENT_RUNBOOK.md         │
│         │                              │ docs/PENTEST_PLAN.md (Pen-test methodology)   │
│         │                              │ docs/FOUNDER_ACTION_TRACKER.md (Hardening)    │
└─────────┴──────────────────────────────┴───────────────────────────────────────────────┘
```

---

## 3. Deep-Dive Control Verification Summaries

### 3.1 CC6.1 & CC6.3 — Multi-Tenant Logical Separation & Access Enforcement
- **PostgreSQL Row-Level Security**: All tenant tables (`CLASS-T`) enforce kernel-level RLS policies requiring `app.current_org_id` context binding.
- **Verification Proof**: [`logs/rls_isolation_report.json`](file:///d:/FinText-Alpha-Vectorizer/logs/rls_isolation_report.json) certifies 0 leak rows across 20 tables during adversarial cross-tenant drills.
- **API Key Redaction**: `GET /v1/account/usage` exposes only safe prefixes (`ft_live_xxxx`); raw hashes and plaintext secrets are stripped at the serialization boundary (`#[serde(skip_serializing)]`).
- **Administrative Token Gating**: Support endpoints (`/v1/admin/tenants/{org_id}/usage`) require `X-Admin-Token` and write an immutable audit log entry before responding.

### 3.2 CC6.6 — Infrastructure Defense & Network Isolation
- **Security Group Chaining**: EC2 gateway ingress permits ports 80/443 (HTTP/HTTPS) and port 22 (admin SSH). RDS PostgreSQL (port 5432) strictly permits ingress from the EC2 security group, isolated in private subnets with no public IP allocation ([`infra/terraform/security_groups.tf`](file:///d:/FinText-Alpha-Vectorizer/infra/terraform/security_groups.tf)).
- **Ingestion Telemetry Isolation**: Internal metrics port `9102` is bound strictly to `127.0.0.1` and container internal networks, blocked from all external ingress.
- **KMS & Storage Encryption**: S3 backup buckets and RDS PostgreSQL volumes enforce AES-256 server-side encryption with automated lifecycle policies.

### 3.3 CC7.1 & CC7.2 — Telemetry, Monitoring & Alerting
- **CloudWatch Infrastructure Alarms**: Automated alarms trigger on EC2 CPU $> 80\%$, RDS CPU $> 80\%$, storage remaining $< 20\text{ GB}$, and status check failures ([`infra/terraform/cloudwatch.tf`](file:///d:/FinText-Alpha-Vectorizer/infra/terraform/cloudwatch.tf)).
- **Sub-Second Ingestion Histograms**: Upstream collector latencies (`fetch_duration_seconds` and `event_lag_seconds`) are tracked via Prometheus buckets from `0.005s` to `2.0s`.
- **Public Status Telemetry**: Automated cron workflow ([`.github/workflows/status-page.yml`](file:///d:/FinText-Alpha-Vectorizer/.github/workflows/status-page.yml)) audits platform health without exposing internal topology.

### 3.4 CC8.1 — Change Management & Automated Gates
- **Branch Protection & CI Gates**: 6 GitHub Actions workflows enforce Rust formatting (`cargo fmt --check`), workspace compilation (`cargo check`), comprehensive test suites (620+ tests passing), security vulnerability scans (`cargo audit`, `gitleaks`), point-in-time backtesting, and data drift detection.
- **Master Launch Checklist**: Pre-flight verification requires 100% pass across 49 checks in [`docs/PRIVATE_BETA_LAUNCH_CHECKLIST.md`](file:///d:/FinText-Alpha-Vectorizer/docs/PRIVATE_BETA_LAUNCH_CHECKLIST.md).

### 3.5 A1.2 — Availability, Disaster Recovery & Continuous Soak
- **Disaster Recovery RTO/RPO**: Backup restoration drill ([`logs/backup_restore_test_report.json`](file:///d:/FinText-Alpha-Vectorizer/logs/backup_restore_test_report.json)) certified Recovery Time Objective of **0.111 seconds** against the 4.0-hour SLA.
- **Soak Stability & Leak Surveillance**: Continuous OLS linear regression monitoring of container memory RSS ([`logs/soak_ledger.md`](file:///d:/FinText-Alpha-Vectorizer/logs/soak_ledger.md)) certifies bounded memory slope $< 2.0\text{ MiB/hour}$ or $R^2 < 0.50$.
- **Cold Storage Transition**: S3 backups expire after 7 days; raw market parquet files transition to Glacier Flexible Retrieval after 90 days.

---

## 4. Auditor Step-by-Step Manifest Verification Procedure

External compliance auditors and procurement technical leads can verify the integrity of the evidence pack using standard command-line tools:

1. **Generate or Refresh Evidence Manifest**:
   ```bash
   python scripts/build_auditor_pack.py
   ```
2. **Inspect Evidence Summary & Status**:
   ```bash
   python -c "import json; d=json.load(open('logs/auditor_pack_manifest.json')); print(f'Total: {d[\"summary\"][\"total_items\"]}, Present: {d[\"summary\"][\"present_items\"]}, Missing: {d[\"summary\"][\"missing_items\"]}')"
   ```
3. **Verify SHA-256 Digest of Any Specific Artifact**:
   ```bash
   # Example: Verify RLS isolation report hash
   python -c "import hashlib, json; manifest=json.load(open('logs/auditor_pack_manifest.json')); target=[e for e in manifest['evidence_items'] if e['id']=='EVID-CC61-01'][0]; computed=hashlib.sha256(open(target['path'], 'rb').read()).hexdigest(); assert computed==target['sha256']; print('EVID-CC61-01 SHA256 MATCH:', computed)"
   ```
