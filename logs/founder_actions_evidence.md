# FinText Alpha Vectorizer — Founder Action Evidence Ledger
═══════════════════════════════════════════════════════════════════════════════
Document ID: EV-FOUNDER-ACTIONS-2026-V1  
Classification: AUDIT & FOUNDER EVIDENCE RECORD  
Target System: FinText Alpha Vectorizer v1.0.0-rc1  
═══════════════════════════════════════════════════════════════════════════════

This ledger records physical and cryptographic evidence for founder-side security and infrastructure actions tracked in `docs/FOUNDER_ACTION_TRACKER.md`.

---

## Action Evidence Entries

### E-7: Nightly Soak Surveillance Scheduler Deployment
- **Status**: PENDING
- **Action Type**: Automated Nightly Cron / Windows Task Scheduler Configuration
- **Scheduled Time**: 02:00 Local / UTC Daily (`scripts/soak_test.py --mode standard`)
- **Evidence Output Target**: `logs/soak_ledger.md` and `logs/soak_report.json`
- **Verification Hash / Command Output**:
  ```
  [Awaiting founder execution of scheduler registration]
  ```

### E-7b: 24-Hour Continuous Soak Baseline Certification
- **Status**: PENDING
- **Action Type**: Uninterrupted 24-Hour Container RSS Memory & Latency Drift Run
- **Target Invariants**: Container RSS slope < 2.0 MiB/h or R^2 < 0.50; P95 Latency < 500ms; Error Rate 0.00%
- **Verification Hash / Command Output**:
  ```
  [Awaiting 24-hour continuous baseline run execution]
  ```

### E-8a: GitHub & AWS Root Account Two-Factor Authentication (2FA)
- **Status**: PENDING
- **Action Type**: Hardware Security Key (FIDO2/WebAuthn) or TOTP Enrollment
- **Target Accounts**: GitHub `@Venkatesh-Akula-417` & AWS Production Account Root
- **Verification Confirmation**:
  ```
  [Awaiting founder confirmation of 2FA enforcement]
  ```

### E-8b: Revocation of Legacy GitHub Personal Access Token (PAT)
- **Status**: PENDING
- **Action Type**: Audit and Revocation of Expired / Broad-Scope Developer PATs
- **Pre-Condition**: Git Credential Manager (GCM) browser authentication verified
- **Verification Confirmation**:
  ```
  [Awaiting founder revocation in GitHub Developer Settings]
  ```

### E-8c: Master Database Password Rotation (`fintext_admin`)
- **Status**: PENDING
- **Action Type**: Cryptographic entropy re-generation for RDS PostgreSQL / TimescaleDB
- **Secret Target**: AWS Secrets Manager `fintext/production/rds/credentials`
- **Verification Confirmation**:
  ```
  [Awaiting rotation in AWS Secrets Manager and Terraform state]
  ```

### AWS-0: AWS Production Account Bootstrap
- **Status**: PENDING
- **Action Type**: AWS Organization creation, Root account lockdown, Billing budget alerts ($301.44)
- **Target Region**: `us-east-1` (N. Virginia)
- **Verification Confirmation**:
  ```
  [Awaiting AWS Account Bootstrap completion]
  ```

### AWS-1: AWS Phase-0 Infrastructure-as-Code Apply
- **Status**: PENDING
- **Action Type**: Initial `terraform apply` across VPC, Subnets, S3 Buckets, Security Groups, and RDS
- **Target Manifest**: `infra/terraform/` (28 resources, $281.49/mo baseline)
- **Verification Confirmation**:
  ```
  [Awaiting founder Phase-0 deployment]
  ```
