# FinText Alpha Vectorizer — Private-Beta Tenant Onboarding & Offboarding Runbook
═══════════════════════════════════════════════════════════════════════════════
Document ID: RUNBOOK-OPS-ONB-001  
Classification: INSTITUTIONAL OPERATIONS & CLIENT ONBOARDING  
Status: PRODUCTION OPERATIONAL (P2 PRIVATE-BETA LAUNCH ENABLER)  
Audience: Platform Engineers, Client Integration Engineers, Quantitative Support, Compliance Officers  
Repository: `FinText-Alpha-Vectorizer` (`git@github.com:Venkatesh-Akula-417/Fintext-Alpha.git`)  
Cross-Reference: [TENANT_ISOLATION_RUNBOOK.md](file:///d:/FinText-Alpha-Vectorizer/docs/TENANT_ISOLATION_RUNBOOK.md), [API_CUSTOMER_GUIDE.md](file:///d:/FinText-Alpha-Vectorizer/docs/API_CUSTOMER_GUIDE.md), [SECURITY.md](file:///d:/FinText-Alpha-Vectorizer/docs/SECURITY.md)  
═══════════════════════════════════════════════════════════════════════════════

## 1. Overview & Operating Philosophy

### 1.1 Objective
This runbook provides the definitive, end-to-end procedural manual for provisioning, onboarding, credential handoff, operational monitoring, and deprovisioning institutional clients for FinText Alpha Vectorizer. It guarantees that every client—from multi-billion quantitative hedge funds to proprietary statistical arbitrage desks—is onboarded deterministically, with zero-leak multi-tenant Row-Level Security (RLS) enforcement, out-of-band secret transmission, and measured Time-To-First-Value (TTFV) strictly under 300 seconds.

### 1.2 Core Security Invariants
1. **Zero Secret Persistence on Disk or Logs:** Plaintext API keys exist exclusively in ephemeral process memory during script execution and are printed exactly once to `STDOUT`. Neither `logs/tenant_provisioning_report.json` nor database audit tables store raw API keys. Key hashes are stored using SHA-256 (`api_keys.key_hash`), and passwords are hashed using Argon2id (`users.password_hash`).
2. **Database-Enforced Row-Level Security (RLS):** All tenant data is committed with strict foreign-key linkage to an `org_id` and isolated via PostgreSQL 16 RLS policies with `FORCE ROW LEVEL SECURITY`. Every provisioning operation runs an immediate automated RLS confinement self-test under the unprivileged role `fintext_app` (`NOBYPASSRLS`).
3. **Idempotency & Pre-Flight Validation:** All scripts require explicit parameters, validate email/slug formats, check for pre-existing records, and default to safe, non-destructive DRY-RUN modes (exit code 2) before committing live transactions (exit code 0).
4. **SEC Rule 17a-4 & FINRA Rule 4511 Compliance:** Client offboarding deactivates access keys and suspends user accounts instantly, but strictly preserves immutable audit logs (`api_request_logs`, `audit_logs`) for 7 years (2,555 days) to meet institutional regulatory books-and-records obligations.

---

## 2. Institutional Client Lifecycle Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│ 1. Legal & Vetting Phase                                                    │
│    • Mutual NDA Execution                                                   │
│    • Commercial Beta Agreement & Order Form Signed                          │
│    • IP Egress Whitelist CIDR Collection & Primary Operator Designation    │
└──────────────────────────────────────┬──────────────────────────────────────┘
                                       │
                                       ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ 2. Automated Provisioning (`scripts/provision_tenant.py`)                   │
│    • Step 1: Input Validation & Idempotency Check                           │
│    • Step 2: Atomic Transaction: Users, Orgs, Keys, Subs, Universes, DR     │
│    • Step 3: RLS Isolation Self-Test (`fintext_app` role, 0 leak rows)     │
│    • Step 4: TTFV Benchmark (< 300s SLA, typically ~1.6s)                   │
│    • Step 5: Single STDOUT Handoff Block + Zero-Secret JSON Report          │
└──────────────────────────────────────┬──────────────────────────────────────┘
                                       │
                                       ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ 3. Secure Credential Handoff                                                │
│    • Out-of-band transmission (PGP / 1Password / Bitwarden / Keybase)       │
│    • Client Operator confirms receipt & test token acquisition              │
└──────────────────────────────────────┬──────────────────────────────────────┘
                                       │
                                       ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ 4. Client Integration & First-Signal Verification                           │
│    • Health check ping (`/v1/health`)                                       │
│    • JWT token issuance via `/v1/auth/token`                                │
│    • First point-in-time sentiment query (`/v1/sentiment?ticker=AAPL`)     │
└──────────────────────────────────────┬──────────────────────────────────────┘
                                       │
                                       ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ 5. Ongoing Proactive Monitoring (Week-1 SLA Vigilance)                      │
│    • P95 latency tracking (< 500ms target)                                  │
│    • Token bucket rate limit headroom monitoring                            │
│    • Daily ingestion freshness & QuestDB fallback circuit-breaker status    │
└──────────────────────────────────────┬──────────────────────────────────────┘
                                       │
                                       ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ 6. Controlled Offboarding (`scripts/deprovision_tenant.py`)                 │
│    • Immediate key revocation & user suspension                             │
│    • Optional domain universe purge (`--purge-data`)                        │
│    • Audit logs preserved for 7-year regulatory retention compliance        │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 3. Pre-Requisites & Technical Onboarding Intake

Before executing provisioning automation, the Quantitative Integration Engineer must obtain and verify the following institutional intake items:

| Intake Item | Technical Requirement | Example / Value |
| :--- | :--- | :--- |
| **Organization Slug** | Lowercase alphanumeric, hyphens/underscores allowed, 3–32 chars | `org_millennium_stat` |
| **Legal Entity Name** | Full legal entity name for contract reconciliation | `Millennium Capital Management LLC` |
| **Operator Email** | Official corporate domain email (no free email providers) | `quant-ops@millennium.internal` |
| **Commercial Plan Tier** | Must be one of `starter`, `growth`, or `enterprise` | `growth` |
| **Egress IP Whitelist** | Static IPv4/IPv6 CIDR blocks for ingress gating | `198.51.100.12/32, 203.0.113.0/24` |
| **Default Universe** | Pre-configured equity universe (default: US Equities Core) | `AAPL, MSFT, NVDA, GOOGL, AMZN` |
| **Retention Policy** | Regulatory books & records audit log retention (days) | `2555` (7 years) |

---

## 4. Automated Tenant Provisioning Procedure

### 4.1 Step 1: Pre-Flight Dry Run
Execute the provisioning script **without** the `--live` flag. This inspects the target database, validates email and slug constraints, generates prospective entity UUIDs, previews the atomic SQL transaction DDL, and exits with code `2`:

```bash
python scripts/provision_tenant.py \
  --slug org_alpha_fund \
  --name "Alpha Quant Partners LP" \
  --email "ops@alphaquant.internal" \
  --tier growth
```

**Expected Dry-Run Output:**
```text
===============================================================================
 FinText Alpha Vectorizer - Institutional Tenant Provisioning Engine
===============================================================================

>>> MODE: DRY-RUN PREVIEW (No changes committed to database) <<<

  Target Organization:   Alpha Quant Partners LP (org_alpha_fund)
  Primary Operator:       ops@alphaquant.internal
  Commercial Tier:        GROWTH (Private Beta Cohort 1)
  Generated Org ID:       b48a12dc-8a39-4467-8822-794691bc2aa1
  Generated User ID:      c98b23ad-5521-4f23-9912-881273ac3bb2
  Generated Key Prefix:   ft_8b91c (Key material: ft_8b91c***[MASKED]***)
  Password Hash Algo:     argon2id ($argon2id$v=19$m=19456,t=2,p=1...)
  Key Hash Algorithm:     sha256

Planned SQL Transaction DDL:
-------------------------------------------------------------------------------
  -- 1. Insert primary user
  INSERT INTO users (id, email, password_hash, role, created_at, is_active)
  VALUES ('c98b23ad-5521-4f23-9912-881273ac3bb2', 'ops@alphaquant.internal', '...', 'institutional', NOW(), true);
  ...
-------------------------------------------------------------------------------

Dry-run completed successfully. Re-run with --live to commit to database.
```
*(Exit code: `2`)*

---

### 4.2 Step 2: Live Provisioning & RLS Certification
When ready to commit to production, append the `--live` flag. The engine executes the atomic transaction via `DATABASE_ADMIN_URL`, verifies RLS confinement via `fintext_app`, queries the live Axum API to benchmark TTFV, writes `logs/tenant_provisioning_report.json`, and outputs the single-use credential handoff block:

```bash
python scripts/provision_tenant.py \
  --slug org_alpha_fund \
  --name "Alpha Quant Partners LP" \
  --email "ops@alphaquant.internal" \
  --tier growth \
  --live
```

**Execution Pipeline Steps:**
1. **Entity Graph Commit:** Atomically inserts records into `users`, `organizations`, `organization_members`, `api_keys`, `subscriptions`, `universes`, and `data_retention_policies`.
2. **RLS Self-Test:** Connects under unprivileged database role `fintext_app` (`NOBYPASSRLS`), sets `SET app.current_org_id = '<new_org_id>'`, and verifies that:
   - Exactly 1 organization record is returned.
   - Exactly 1 universe record is returned.
   - Zero rows belonging to other organizations are returned.
3. **TTFV Benchmark:** Exchanges a test token via `/v1/auth/token` and issues an authenticated `GET /v1/sentiment?ticker=AAPL` against the Axum gateway (`http://127.0.0.1:8000`). Measures wall-clock execution time and records millisecond latency.
4. **Structured Audit Report:** Persists `logs/tenant_provisioning_report.json` containing cryptographic fingerprints, TTFV metrics, and RLS test results—with **0 plaintext secrets**.
5. **Credential Handoff Block:** Outputs client API key and quickstart command sequence to `STDOUT`.

---

## 5. Secure Credential Handoff Protocol

### 5.1 Rules of Secret Transmission
- **NEVER** transmit plaintext API keys via unencrypted channels (e.g. Email, Slack, Microsoft Teams, Jira, GitHub tickets).
- **NEVER** commit API keys or environment `.env` files into source control.
- **ALWAYS** transmit the single-use credential block via one of the following approved secure vaults:
  1. **1Password / Bitwarden Secure Share:** 1-time view link with 24-hour expiration.
  2. **GPG / PGP Encryption:** Encrypted to the client technical lead's verified public key.
  3. **Keybase Encrypted Chat / Git:** Client-dedicated secure channel.

### 5.2 Credential Artifact Checklist
The handoff package must contain:
- `Organization Slug`: e.g. `org_alpha_fund`
- `Organization UUID`: e.g. `b48a12dc-8a39-4467-8822-794691bc2aa1`
- `Plaintext API Key`: `ft_<hex32>` (35 characters total)
- `Base API URL`: `https://api.fintext.internal` (or `http://127.0.0.1:8000` during VPN beta)
- `Documentation Link`: Institutional API Guide & OpenAPI schema

---

## 6. Client Verification & SDK Quickstart

Once credentials are transmitted, guide the client quant developer through these exact commands to verify connectivity:

### 6.1 Step 1: Gateway Health Verification
```bash
curl -s http://127.0.0.1:8000/v1/health
```
**Expected Response:**
```json
{"status":"healthy","database":"connected","version":"0.1.0"}
```

### 6.2 Step 2: Acquire JWT Bearer Token
```bash
export TOKEN=$(curl -s -X POST http://127.0.0.1:8000/v1/auth/token \
  -H "X-Admin-Token: fintext-admin-dev-secret-token" \
  -H "Content-Type: application/json" \
  -d '{"user_id": "<USER_UUID>", "org_id": "<ORG_UUID>"}' | jq -r .token)
```

### 6.3 Step 3: Fetch First Point-in-Time Alpha Signal
```bash
curl -s -H "Authorization: Bearer $TOKEN" \
  "http://127.0.0.1:8000/v1/sentiment?ticker=AAPL" | jq .
```
**Expected Response:**
```json
{
  "ticker": "AAPL",
  "composite_score": 0.84,
  "confidence": 0.95,
  "point_in_time": true,
  "as_of_utc": "2026-09-24T10:44:17Z",
  "signals": {
    "finbert_sentiment": 0.88,
    "roberta_sentiment": 0.81,
    "regulatory_risk": 0.05
  }
}
```

### 6.4 Python Integration Snippet
```python
import httpx
import os

BASE_URL = os.environ.get("FINTEXT_API_URL", "http://127.0.0.1:8000")
API_KEY = os.environ["FINTEXT_API_KEY"]

def get_alpha_signal(ticker: str) -> dict:
    headers = {
        "Authorization": f"Bearer {API_KEY}",
        "Content-Type": "application/json"
    }
    with httpx.Client(base_url=BASE_URL, timeout=5.0) as client:
        resp = client.get(f"/v1/sentiment", params={"ticker": ticker}, headers=headers)
        resp.raise_for_status()
        return resp.json()

if __name__ == "__main__":
    signal = get_alpha_signal("AAPL")
    print(f"AAPL Composite Score: {signal['composite_score']} (PIT: {signal['point_in_time']})")
```

### 6.5 Tenant Self-Service API Key Lifecycle & Quota Transparency (`/v1/account/keys`)

Private Beta clients can self-serve and automate key rotation without opening integration support tickets:

- **Create Key**: `POST /v1/account/keys` returns `{"id": "...", "plaintext_once": "ft_live_..."}`. Plaintext key is displayed strictly once. Max 10 active keys per tenant.
- **List Inventory**: `GET /v1/account/keys` returns active, revoked, and expired keys with safe 16-character prefixes (zero hash/secret exposure).
- **Rotate Key**: `POST /v1/account/keys/{key_id}/rotate` revokes the old key with status `rotated` and issues a replacement key preserving lineage via `rotated_from`.
- **Revoke Key**: `DELETE /v1/account/keys/{key_id}` revokes compromised or decommissioned keys immediately.
- **Quota Transparency**: All responses emit `X-RateLimit-Limit`, `X-RateLimit-Remaining`, and `X-RateLimit-Reset` (Unix epoch seconds) to facilitate algorithmic backoff.

---

## 7. Week-1 Proactive Operational Monitoring

During the initial 7 days of onboarding, the assigned Quantitative SRE must monitor the following telemetry signals:

| Telemetry Metric | Production Target | Alert Threshold | Remediation Action |
| :--- | :--- | :--- | :--- |
| **P95 Latency** | `< 250 ms` | `> 500 ms` for 3 consecutive intervals | Inspect TimescaleDB chunk exclusion; verify query plan |
| **HTTP 429 Rate Limits** | `0 per hour` | `> 5 per 15 min` | Contact client to verify burst concurrency or upgrade tier |
| **HTTP 401/403 Errors** | `< 0.01%` of total | `> 10 in 5 min` | Validate JWT expiry, token signature, or IP whitelist CIDR |
| **QuestDB Circuit Breaker** | `CLOSED` (Primary) | `OPEN` (Fallback active) | Check primary PostgreSQL connection pool health |
| **Data Ingestion Freshness**| `< 60s lag` | `> 300s lag` | Restart ingestion worker or check Redpanda Kafka lag |

---

## 8. Controlled Tenant Deprovisioning & Offboarding Protocol

When a client terminates their beta engagement, the following procedure guarantees zero-residual-access offboarding while maintaining strict regulatory compliance.

### 8.1 Step 1: Pre-Flight Dry Run
Run `scripts/deprovision_tenant.py` with only the `--slug` parameter:

```bash
python scripts/deprovision_tenant.py --slug org_alpha_fund
```

**Expected Output:**
- Prints tenant entity metadata, active API key count, associated members, and custom universes.
- Displays planned SQL revocation statements.
- Exits with code `2` without altering database state.

### 8.2 Step 2: Live Deprovisioning Execution
Execute with the mandatory safety confirmation flag `--i-understand-data-loss`:

```bash
python scripts/deprovision_tenant.py --slug org_alpha_fund --i-understand-data-loss
```

**Deprovisioning Actions Executed in Single Transaction:**
1. **API Key Revocation:** Sets `revoked_at = NOW()`, `rotation_status = 'revoked'` in `api_keys`.
2. **Account Suspension:** Sets `is_active = false` in `users`.
3. **Subscription Termination:** Sets `status = 'canceled'`, `updated_at = NOW()` in `subscriptions`.
4. **Membership Suspension:** Sets `role = 'suspended'` in `organization_members`.
5. **Retention Policy Deactivation:** Sets `is_active = false` in `data_retention_policies`.
6. **Regulatory Compliance Guarantee:** Audit log history (`api_request_logs`, `audit_logs`) is **NEVER deleted**, preserving SEC Rule 17a-4 / FINRA Rule 4511 compliance for 7 years.
7. **Verification:** Queries active API keys and asserts `0 active keys remaining`.
8. **Audit Certification:** Writes structured audit report to `logs/tenant_deprovisioning_report.json`.

---

## 9. Operational Troubleshooting & Error Code Matrix

| Error Status | Root Cause | Operator Diagnostic & Fix |
| :--- | :--- | :--- |
| **HTTP 401 Unauthorized** | Expired JWT token, invalid API key, or revoked credentials | Check `revoked_at` in `api_keys`. If revoked, generate new key via `users.rs` or re-provision. Re-exchange token via `/v1/auth/token`. |
| **HTTP 403 Forbidden** | RLS policy rejection or IP whitelist CIDR mismatch | Verify `app.current_org_id` matches the tenant's UUID. Ensure client egress IP falls within whitelisted CIDR blocks in `ip_whitelist`. |
| **HTTP 429 Too Many Requests** | Exceeded tier rate limit (Token bucket exhausted) | Review tier limits (Starter: 60/min, Growth: 300/min, Enterprise: 1,200/min). Adjust burst capacity or upgrade subscription plan. |
| **HTTP 500 Internal Error** | Database connection timeout or missing RLS context | Check PostgreSQL logs via `docker logs fintext-postgres`. Verify `fintext_app` role permissions on target table. |
| **HTTP 503 Service Unavailable** | Gateway overload or QuestDB circuit breaker tripped | Check API gateway container health (`docker ps`). Verify TimescaleDB / QuestDB container resource utilization. |

---

## 10. Escalation Path & Service Level Agreements (SLA)

| Severity | Definition | Response SLA | Target Resolution | Escalation Contact |
| :--- | :--- | :--- | :--- | :--- |
| **P1 - Critical** | Full API outage, cross-tenant isolation compromise, authentication down | `< 15 minutes` | `< 1 hour` | Lead Quant Architect / CTO On-Call |
| **P2 - Major** | Degraded performance (P95 > 500ms), fallback circuit-breaker active | `< 1 hour` | `< 4 hours` | Principal Platform Engineer / SRE |
| **P3 - Minor** | Rate limiting dispute, single ticker sentiment missing, universe update | `< 4 hours` | `< 1 business day`| Quant Support / Integration Engineer |
| **P4 - Request** | New universe onboarding, IP whitelist addition, key rotation assistance | `< 1 business day`| `< 2 business days`| Client Success / Operations Desk |

---

## 11. Support Playbook: Tenant Usage & Day-2 Operational Diagnostics

### 11.1 Support Inspection via Admin Diagnostic Surface
When a client files a ticket regarding rate-limits, access blocks, or unexpected quota exhaustion, technical support engineers must **never** run ad-hoc database queries using superuser credentials. Instead, use the token-gated admin diagnostic surface (`GET /v1/admin/tenants/{org_id}/usage`).

- **Accountability & Audit Trail (Safety Constraint S-2):** Every call generates an immutable `admin.tenant_usage_view` entry in `audit_logs` capturing the support operator, timestamp, and target `org_id`.
- **Zero Credential Exposure (Safety Constraint S-1):** Returns safe key metadata (prefix, created, last_seen, active); never leaks secret keys or password hashes.

### 11.2 Diagnostic Execution (Repair-Manual Style)

```bash
# Execute administrative inspection for tenant organization
curl -s -X GET "https://api.fintext.internal/v1/admin/tenants/org_quant_alpha_42/usage" \
  -H "X-Admin-Token: ${ADMIN_TOKEN}" | jq .
```

#### Expected Output Skeleton:
```json
{
  "usage": {
    "org_id": "org_quant_alpha_42",
    "period_utc": "2026-09",
    "plan": "growth",
    "plan_limit": 500000,
    "requests_total": 412500,
    "headroom_pct": 17.50,
    "daily": [
      { "date": "2026-09-24", "requests": 22400 },
      { "date": "2026-09-25", "requests": 24100 }
    ],
    "by_endpoint_group": [
      { "group": "sentiment", "requests": 280000 },
      { "group": "alpha", "requests": 95000 },
      { "group": "pit", "requests": 25000 },
      { "group": "analytics", "requests": 10000 },
      { "group": "other", "requests": 2500 }
    ],
    "keys": [
      {
        "prefix": "ak_live_a1b2",
        "name": "Production Execution Alpha",
        "created_utc": "2026-08-15T10:00:00Z",
        "last_seen_utc": "2026-09-25T14:35:00Z",
        "active": true
      }
    ],
    "recent_audit": [
      { "ts": "2026-09-25T14:00:00Z", "event_type": "api_key.create", "actor": "user_admin" }
    ],
    "ip_whitelist": [
      "198.51.100.0/24"
    ],
    "generated_utc": "2026-09-25T15:00:00Z"
  },
  "subscription": {
    "plan": "growth",
    "status": "active",
    "dunning_fail_count": 0,
    "grace_until_utc": null
  }
}
```

### 11.3 Support Escalation Matrix & Proactive Action Guide

| Symptom / Inspection Finding | Root Cause | Operator Action |
| :--- | :--- | :--- |
| **`headroom_pct <= 20.0%` (Quota >= 80% consumed)** | Rapid backtest or production expansion approaching monthly tier limit | Send proactive quota alert email to fund technical contact advising tier upgrade to `enterprise_monthly` before hard 429 enforcement. |
| **Client receives HTTP 403 Forbidden** | Egress IP not present in `ip_whitelist` | Check `ip_whitelist` array in payload. If client IP changed (e.g. AWS NAT gateway migration), guide client admin to add new CIDR via `POST /security/ip-whitelist`. |
| **Client receives HTTP 429 Too Many Requests** | Burst token-bucket exhaustion or `requests_total >= plan_limit` | Inspect `by_endpoint_group` and daily trends. If uncoordinated backtesting parallelization, recommend rate-limit backoff or provide custom burst allocation. |
| **`subscription.status == 'past_due'`** | Recurring Stripe card charge failed; dunning active | Notify fund billing contact of remaining grace period (`grace_until_utc`). Point client to Stripe customer portal URL (`POST /billing/portal`). |

---

## 12. Cohort-1 Comms Kit

> **Gate Reference:** This section satisfies [BETA_GO_NO_GO.md](file:///d:/FinText-Alpha-Vectorizer/docs/BETA_GO_NO_GO.md) Gate G8: Cohort-1 Comms Kit Approved.

### 12.1 Welcome Email Template

```
Subject: Welcome to FinText Alpha Vectorizer — Private Beta Access

Dear [Client Primary Operator Name],

We are pleased to confirm your Private Beta access to FinText Alpha
Vectorizer, the institutional-grade NLP and alternative data platform
for mid-frequency quantitative strategies.

ENDPOINT & ACCESS
─────────────────
  API Base URL:       {{BASE_URL}}/v1
  Health Check:       {{BASE_URL}}/v1/health
  Status Page:        {{STATUS_URL}}
  Documentation:      {{BASE_URL}}/docs

CREDENTIAL DELIVERY
───────────────────
Your API key has been delivered via a one-time secure note through
[1Password / Bitwarden / PGP-encrypted email — select applicable].
The secure note expires 72 hours after generation. Please:

  1. Retrieve and store the API key in your fund's secrets vault.
  2. Confirm receipt by pinging: GET {{BASE_URL}}/v1/health
     with header: X-API-Key: <your_key>
  3. The key prefix (first 8 chars) is included below for
     reconciliation: [PREFIX_HERE]

IMPORTANT: This is the ONLY time the plaintext key is transmitted.
FinText stores only SHA-256 hashes. If the key is lost, a new key
must be generated and the previous key revoked.

KEY ROTATION POLICY
───────────────────
  • Mandatory rotation cadence: every 90 calendar days.
  • Self-service rotation: POST {{BASE_URL}}/v1/keys/rotate
  • Upon rotation, the previous key enters a 24-hour grace window
    before hard revocation.
  • Maximum active keys per organization: 10.

SUPPORT MATRIX
──────────────
  Tier   │ Channel              │ Response Time SLA
  ───────┼──────────────────────┼───────────────────
  T1     │ Email / Slack        │ < 4 business hours
  T2     │ Scheduled Call       │ < 1 business day
  T3     │ Emergency Hotline    │ < 1 hour (critical)

  Escalation: support@fintext.io → cto@fintext.io (T3 only)

DATA FRESHNESS & DEGRADATION SEMANTICS
───────────────────────────────────────
All FinText API responses include a `mode` field indicating data
freshness:

  • mode=primary     — Live upstream feed; real-time freshness.
  • mode=degraded    — Upstream vendor temporary disruption; data
                       served via REST polling fallback with
                       bounded lag (Finnhub ≤2s, Polygon ≤5s).
  • mode=stale       — Extended upstream outage (>10 min for
                       SEC EDGAR, >5 min for FOMC/Corp Actions);
                       cached/replay data served with timestamp
                       of last known-good observation.

Your quantitative models should incorporate this mode flag for
position sizing and risk gating. Full degradation semantics and
circuit breaker architecture are documented in:
  docs/FEED_RESILIENCE_RUNBOOK.md

We look forward to supporting your alpha research.

Best regards,
FinText Alpha Vectorizer — Platform Engineering Team
```

### 12.2 Credential Delivery Standard Operating Procedure

| Step | Action | Owner | Verification |
| :---: | :--- | :--- | :--- |
| 1 | Generate key via `scripts/provision_tenant.py --live` | Platform Engineer | Exit code 0, TTFV < 300s |
| 2 | Copy plaintext key from STDOUT (appears exactly once) | Platform Engineer | Key starts with `ft_` |
| 3 | Create one-time secure note (1Password / Bitwarden) | Platform Engineer | 72h expiry, single-view |
| 4 | Send secure note link via pre-approved channel | Platform Engineer | Email/Slack to primary operator |
| 5 | Client confirms receipt + health ping success | Client Operator | HTTP 200 on /v1/health |
| 6 | Discard local plaintext key from clipboard/terminal | Platform Engineer | Verified cleared |

### 12.3 Ongoing Communication Cadence (Beta Period)

| Event | Template | Channel | Audience |
| :--- | :--- | :--- | :--- |
| Weekly Status Digest | Automated from status cron | Email | All Cohort-1 |
| Incident Notification | SEV-1/SEV-2 per ops runbook | Email + Slack | Affected tenants |
| Rotation Reminder | 14-day advance notice | Email | Key owner |
| Beta Feedback Survey | Monthly NPS + feature request | Email | Primary operators |

