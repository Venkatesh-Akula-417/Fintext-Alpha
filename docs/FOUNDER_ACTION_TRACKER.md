# FinText Alpha Vectorizer — Founder Action Tracker & Hardening Playbook
═══════════════════════════════════════════════════════════════════════════════
Document ID: TRACKER-FOUNDER-ACTIONS-2026-V1  
Classification: INTERNAL AUDIT & FOUNDER GOVERNANCE RECORD  
Target System: FinText Alpha Vectorizer v1.0.0-rc1  
Effective Date: September 26, 2026  
Responsible Board Lead: Chief Technology Officer & Platform Founder  
═══════════════════════════════════════════════════════════════════════════════

## 1. Executive Summary & Tracking Objective

While all core platform code, automated test suites, database migrations, and Infrastructure-as-Code definitions are 100% committed, validated, and certified in the repository, several critical security and operational tasks require direct founder credentials or interactive environment access.

This document serves as the authoritative, auditable tracker for these founder-side items. Every action is mapped to an immutable evidence file at [`logs/founder_actions_evidence.md`](file:///d:/FinText-Alpha-Vectorizer/logs/founder_actions_evidence.md).

---

## 2. Master Founder Action Matrix

```
┌────────────────────────────────────────────────────────────────────────────────────────┐
│                        FOUNDER ACTION TRACKER & AUDIT MATRIX                           │
├────────┬─────────────────────────────────────┬─────────┬───────────────────────────────┤
│ ID     │ Action Item Description             │ Status  │ Evidence Pointer & Log Target │
├────────┼─────────────────────────────────────┼─────────┼───────────────────────────────┤
│ E-7    │ Nightly Soak Scheduler Setup        │ PENDING │ logs/founder_actions_evidence.│
│        │ (Windows Task Scheduler / Cron)     │         │ md (Section E-7)              │
├────────┼─────────────────────────────────────┼─────────┼───────────────────────────────┤
│ E-7b   │ 24-Hour Continuous Soak Baseline    │ PENDING │ logs/soak_ledger.md &         │
│        │ (Uninterrupted Stability Proof)     │         │ logs/soak_report.json         │
├────────┼─────────────────────────────────────┼─────────┼───────────────────────────────┤
│ E-8a   │ Enforce Hardware/TOTP 2FA           │ PENDING │ logs/founder_actions_evidence.│
│        │ (GitHub Org & AWS Root Account)     │         │ md (Section E-8a)             │
├────────┼─────────────────────────────────────┼─────────┼───────────────────────────────┤
│ E-8b   │ Revoke Legacy Developer PATs        │ PENDING │ logs/founder_actions_evidence.│
│        │ (Confirm GCM Browser Authentication)│         │ md (Section E-8b)             │
├────────┼─────────────────────────────────────┼─────────┼───────────────────────────────┤
│ E-8c   │ Rotate RDS Database Master Password │ PENDING │ logs/founder_actions_evidence.│
│        │ (Update AWS Secrets Manager)        │         │ md (Section E-8c)             │
├────────┼─────────────────────────────────────┼─────────┼───────────────────────────────┤
│ AWS-0  │ AWS Production Account Bootstrap    │ PENDING │ logs/founder_actions_evidence.│
│        │ (Root Lock, Budget Alarm $301.44)   │         │ md (Section AWS-0)            │
├────────┼─────────────────────────────────────┼─────────┼───────────────────────────────┤
│ AWS-1  │ AWS Phase-0 Infrastructure Apply    │ PENDING │ logs/infra_validate_report.   │
│        │ (Initial terraform apply execution) │         │ json & Section AWS-1          │
└────────┴─────────────────────────────────────┴─────────┴───────────────────────────────┘
```

---

## 3. Copy-Paste Deployment Instructions: Nightly Soak Scheduler (E-7)

The nightly soak script (`scripts/soak_test.py --mode standard`) exercises the platform across 20 concurrent connections, tracks container RSS memory growth slope, monitors P95 latency drift (< 500ms SLA), and appends cryptographic verification records to [`logs/soak_ledger.md`](file:///d:/FinText-Alpha-Vectorizer/logs/soak_ledger.md).

### Option A: Windows Environment (PowerShell One-Liner)
Run the following command in an elevated PowerShell terminal to register the daily 02:00 AM scheduled task:

```powershell
$Action = New-ScheduledTaskAction `
    -Execute "python" `
    -Argument "scripts/soak_test.py --mode standard" `
    -WorkingDirectory "D:\FinText-Alpha-Vectorizer"

$Trigger = New-ScheduledTaskTrigger -Daily -At 2:00AM

$Settings = New-ScheduledTaskSettingsSet `
    -AllowStartIfOnBatteries `
    -DontStopIfGoingOnBatteries `
    -StartWhenAvailable `
    -ExecutionTimeLimit (New-TimeSpan -Hours 2)

Register-ScheduledTask `
    -TaskName "FinText_Nightly_Soak_Surveillance" `
    -Action $Action `
    -Trigger $Trigger `
    -Settings $Settings `
    -Description "Automated Nightly Soak Stability & RSS Memory Leak Audit for FinText Private Beta"
```

#### Windows Task Scheduler XML Definition (`FinText_Soak_Task.xml`):
```xml
<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.4" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Description>FinText Alpha Vectorizer Nightly Soak Stability Surveillance</Description>
  </RegistrationInfo>
  <Triggers>
    <CalendarTrigger>
      <StartBoundary>2026-09-26T02:00:00</StartBoundary>
      <Enabled>true</Enabled>
      <ScheduleByDay>
        <DaysInterval>1</DaysInterval>
      </ScheduleByDay>
    </CalendarTrigger>
  </Triggers>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <ExecutionTimeLimit>PT2H</ExecutionTimeLimit>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>python</Command>
      <Arguments>scripts/soak_test.py --mode standard</Arguments>
      <WorkingDirectory>D:\FinText-Alpha-Vectorizer</WorkingDirectory>
    </Exec>
  </Actions>
</Task>
```

### Option B: Linux Host Deployment (crontab entry)
Add the following line to the `ubuntu` crontab on the AWS EC2 production host:

```bash
# FinText Nightly Soak Surveillance: Runs daily at 02:00 UTC and appends to soak ledger
0 2 * * * cd /opt/fintext && /usr/bin/python3 scripts/soak_test.py --mode standard >> /opt/fintext/logs/soak_cron.log 2>&1
```

---

## 4. Operational Playbook for Remaining Founder Actions

### E-7b: 24-Hour Continuous Soak Baseline
- **Execution**: Run `python scripts/soak_test.py --mode extended --duration-hours 24`.
- **Target Invariant**: OLS memory slope $< 2.0\text{ MiB/hour}$, $R^2 < 0.50$, Error Rate $= 0.00\%$.
- **Verification**: Ensure results automatically commit to `logs/soak_report.json` and append to `logs/soak_ledger.md`.

### E-8a: Two-Factor Authentication (2FA) Enforcement
- **Execution**: Navigate to GitHub Settings -> Password and authentication -> Two-factor authentication. Enable Security Keys (FIDO2) or Authenticator App.
- **Scope**: Must be enforced for GitHub account `@Venkatesh-Akula-417` and AWS Root account.

### E-8b: Legacy Personal Access Token (PAT) Revocation
- **Pre-Condition**: Verify Git Credential Manager (GCM) browser authentication works cleanly for `git push origin main`.
- **Execution**: Navigate to GitHub Developer Settings -> Personal access tokens. Identify any legacy tokens issued prior to September 2026 and click **Revoke**.

### E-8c: Master Database Password Rotation (`fintext_admin`)
- **Execution**: Generate a 32-character high-entropy secret (`openssl rand -base64 24`). Update in AWS Secrets Manager:
  ```bash
  aws secretsmanager put-secret-value \
    --secret-id "fintext/production/rds/credentials" \
    --secret-string '{"username":"fintext_admin","password":"<NEW_HIGH_ENTROPY_PASSWORD>"}'
  ```

### AWS-0 & AWS-1: AWS Account Bootstrap & Phase-0 Apply
- **AWS-0**: Lock root user credentials with hardware MFA. Create IAM administrative user `admin-devops`. Configure AWS Budgets with hard ceiling alert at `$301.44 / month`.
- **AWS-1**: Follow step-by-step instructions in [`docs/PRODUCTION_DEPLOYMENT_RUNBOOK.md`](file:///d:/FinText-Alpha-Vectorizer/docs/PRODUCTION_DEPLOYMENT_RUNBOOK.md) Section 4 to apply Phase-0 Terraform resources.
