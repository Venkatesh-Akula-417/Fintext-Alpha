# FinText Alpha Vectorizer — Stripe Billing & Dunning Operations Runbook
═══════════════════════════════════════════════════════════════════════════════
**Audience:** Site Reliability Engineers, DevOps, Compliance Officers, Billing Ops  
**Authority:** Operational Source of Truth for Billing Procedures  
**Metering Reference:** [`docs/BILLING_METERING_GUIDE.md`](./BILLING_METERING_GUIDE.md) (metering internals reference)  
**Classification:** Confidential Institutional Operations  
**Compliance Standard:** SOC2 Type II CC6.1 / CC6.6, SEC Rule 17a-4, FINRA Rule 4511  
**Last Revised:** September 2026  
**Document Owner:** Chief Technology Officer & Principal Billing Architect  
═══════════════════════════════════════════════════════════════════════════════

> [!IMPORTANT]
> **Authority Notice — Operational Source of Truth for Billing Procedures**: This runbook is the definitive operational manual for Day-2 billing maintenance, dunning state mitigation, Stripe webhook verification, and automated reconciliation drills. For commercial tier definitions, plan economics, and usage metering pipeline architecture, refer to [`docs/BILLING_METERING_GUIDE.md`](./BILLING_METERING_GUIDE.md).

---

## 1. System Architecture: The Institutional Money Loop

The FinText Alpha Vectorizer billing and revenue assurance architecture connects Stripe subscription events, real-time cryptographic webhook verification, an idempotent dunning state machine, non-destructive access suspension, and monthly usage-to-invoice reconciliation.

```
       ┌────────────────────────────────────────────────────────┐
       │                 Stripe Cloud Platform                  │
       │   - Hosted Checkout Sessions                           │
       │   - Subscription Lifecycle (create, update, cancel)    │
       │   - Automated Invoicing & Recurring Card/ACH Debits    │
       └───────────────────────────┬────────────────────────────┘
                                   │
                                   │ POST /v1/billing/webhook
                                   │ Header: Stripe-Signature (t=..., v1=...)
                                   ▼
       ┌────────────────────────────────────────────────────────┐
       │             FinText Axum API Gateway                   │
       │  1. Unauthenticated Route (Public, Rate-Limited)       │
       │  2. Raw Body Byte Extraction (Pre-JSON Deserialization)│
       │  3. Constant-Time HMAC-SHA256 Signature Verification   │
       │     - Replay Window: |t_now - t_event| <= 300 seconds  │
       │     - Multi-Secret Rotation Window Support             │
       │  4. Idempotency Check:                                 │
       │     INSERT INTO billing_events ... ON CONFLICT DO NOTHING
       └───────────────────────────┬────────────────────────────┘
                                   │
            ┌──────────────────────┴──────────────────────┐
            ▼                                             ▼
 ┌──────────────────────┐                     ┌──────────────────────────┐
 │ Duplicate Event ID   │                     │ Valid New Event          │
 │ (Replay / Retry)     │                     │ (Checkout, Invoice, Sub) │
 └──────────┬───────────┘                     └───────────┬──────────────┘
            │                                             │
            ▼                                             ▼
  HTTP 200 (No Mutation)                      ┌──────────────────────────┐
                                              │   Dunning State Machine  │
                                              │      (PostgreSQL Tx)     │
                                              └───────────┬──────────────┘
                                                          │
      ┌───────────────────────────────────────────────────┼───────────────────────────────────────────────────┐
      ▼                                                   ▼                                                   ▼
┌───────────────────────────┐               ┌───────────────────────────┐               ┌───────────────────────────┐
│ checkout.session.completed│               │   invoice.payment_failed  │               │        invoice.paid       │
│ customer.sub.created/upd  │               │                           │               │                           │
├───────────────────────────┤               ├───────────────────────────┤               ├───────────────────────────┤
│ • Status: 'active'        │               │ • Status: 'past_due'      │               │ • Status: 'active'        │
│ • Bind Stripe Customer ID │               │ • Grace: now() + 72 hours │               │ • Reset fail_count = 0    │
│ • Bind Stripe Sub ID      │               │ • dunning_fail_count + 1  │               │ • Clear grace_until_utc   │
│ • Update plan_id tier     │               │ • If count>=3 or expired: │               │ • Reactivate API keys     │
│ • Audit Log Inserted      │               │   -> Phase-1 Suspension   │               │ • Audit Log Inserted      │
└───────────────────────────┘               └───────────────────────────┘               └───────────────────────────┘
                                                          │
                                                          ▼ (Grace Elapsed without Payment)
                                            ┌───────────────────────────┐
                                            │    Phase-1 Suspension     │
                                            │ (SEC 17a-4 Zero Data Loss)│
                                            ├───────────────────────────┤
                                            │ • subscriptions: canceled │
                                            │ • api_keys: revoked       │
                                            │ • users: is_active=false  │
                                            │ • data_retention: kept    │
                                            │ • audit logs: PRESERVED   │
                                            └───────────────────────────┘
                                                          │
                                                          │ Monthly Close (Day 1, 03:00 UTC)
                                                          ▼
                                            ┌───────────────────────────┐
                                            │ Usage-Invoice Reconciler  │
                                            │ (scripts/reconcile_...py) │
                                            ├───────────────────────────┤
                                            │ usage_events vs plan_id   │
                                            │ vs invoice.paid records   │
                                            │ -> CERTIFIED RECONCILED   │
                                            └───────────────────────────┘
```

---

## 2. Stripe Test-Mode Account Setup & Configuration

FinText Alpha Vectorizer strictly enforces test-mode isolation for staging, continuous integration, and disaster recovery validation drills. Real live credit cards and production funds are NEVER used for engineering validation.

### 2.1 Stripe Dashboard Setup Procedure
1. Navigate to [Stripe Dashboard](https://dashboard.stripe.com).
2. Toggle the **Test Mode** switch in the top-right header (orange banner indicates test mode).
3. Under **Product catalog**, verify or create the three institutional billing tiers:
   - **Starter**: `$500.00 / month` recurring, Product ID: `prod_fintext_starter`, 100,000 requests included.
   - **Growth**: `$2,000.00 / month` recurring, Product ID: `prod_fintext_growth`, 1,000,000 requests included.
   - **Enterprise**: `$20,000.00 / month` recurring, Product ID: `prod_fintext_enterprise`, unlimited firehose.
4. Under **Developers > API keys**:
   - Obtain **Publishable key** (`pk_test_...`) and **Secret key** (`sk_test_...`).
   - Store `sk_test_...` in the Kubernetes namespace secret `stripe-secret` or `.env` (gitignored).

### 2.2 Stripe CLI Local Testing Proxy (Optional Alternative)
For real-time forwarding of test events directly to a local development machine without exposing public ports:
```bash
# 1. Authenticate Stripe CLI
stripe login

# 2. Forward webhooks to local API Gateway
stripe listen --forward-to http://127.0.0.1:8000/v1/billing/webhook

# Expected Output:
# > Ready! Your webhook signing secret is whsec_xxxxxxxxxxxxxxxxxxxxxxx (^C to quit)

# 3. Trigger simulated events
stripe trigger checkout.session.completed
stripe trigger invoice.payment_failed
stripe trigger invoice.paid
```

---

## 3. Webhook Registration & Quarterly Secret Rotation

To defend against compromised signing keys, FinText Alpha Vectorizer supports zero-downtime webhook secret rotation using an overlapping dual-secret acceptance window.

### 3.1 Webhook Endpoint Registration
1. In the Stripe Dashboard, navigate to **Developers > Webhooks > Add destination**.
2. **Endpoint URL**: `https://api.fintext.internal/v1/billing/webhook` (or production DNS).
3. **Listen to events on**: Select the following required events:
   - `checkout.session.completed`
   - `customer.subscription.created`
   - `customer.subscription.updated`
   - `customer.subscription.deleted`
   - `invoice.paid`
   - `invoice.payment_failed`
   - `invoice.payment_action_required`
4. Click **Add endpoint** and reveal the **Signing secret** (`whsec_...`).

### 3.2 Zero-Downtime Secret Rotation Procedure (Quarterly Cadence)
The FinText API gateway verifier (`verify_stripe_signature`) splits `STRIPE_WEBHOOK_SECRET` by commas and semicolons. It attempts verification against every configured secret. If *any* secret matches, the webhook is accepted.

```bash
# Step 1: In Stripe Dashboard, click "Roll secret" -> Select "Keep existing secret valid for 24 hours".
# You will now have Old Secret (whsec_AAA) and New Secret (whsec_BBB).

# Step 2: Update Kubernetes secret with comma-separated secrets:
kubectl create secret generic stripe-secret \
  --from-literal=api-key="sk_test_REPLACE_ME" \
  --from-literal=webhook-secret="whsec_NEW_SECRET_BBB,whsec_OLD_SECRET_AAA" \
  --dry-run=client -o yaml | kubectl apply -f -

# Step 3: Trigger rolling restart of gateway pods
kubectl rollout restart deployment/fintext-api-gateway

# Step 4: Verify live signature verification with offline drill
python scripts/test_billing_flow.py --webhook-secret "whsec_NEW_SECRET_BBB"

# Step 5: After 24 hours (Stripe expires Old Secret), remove whsec_OLD_SECRET_AAA from secret:
kubectl create secret generic stripe-secret \
  --from-literal=webhook-secret="whsec_NEW_SECRET_BBB" \
  --dry-run=client -o yaml | kubectl apply -f -
```

---

## 4. Local Offline Drill: Commands, Expected Outputs & Exit Codes

FinText provides `scripts/test_billing_flow.py` for automated, non-destructive, offline verification of all billing transitions without needing active internet access or live Stripe credentials.

### 4.1 Running the Drill
```bash
# On Linux / macOS (Bash):
export STRIPE_WEBHOOK_SECRET="${STRIPE_WEBHOOK_SECRET:-whsec_REPLACE_ME}"
python scripts/test_billing_flow.py \
  --base-url http://127.0.0.1:8000 \
  --json-report logs/billing_flow_report.json

# On Windows (PowerShell):
$env:STRIPE_WEBHOOK_SECRET = "whsec_REPLACE_ME"
python scripts/test_billing_flow.py `
  --base-url http://127.0.0.1:8000 `
  --json-report logs/billing_flow_report.json
```

### 4.2 Expected Console Output
```text
===============================================================================
 FinText Alpha Vectorizer - Stripe Billing & Dunning Verification Drill
===============================================================================

  Target Gateway:    http://127.0.0.1:8000
  Webhook Secret:    whsec_****01 (Masked)
  Output Report:     logs/billing_flow_report.json

[Phase 0] Setting up isolated test tenant in database...
  [PASS] Test tenant 'org_drill_test' initialized (customer: cus_test_drill_001, sub: sub_test_drill_001)

#   | Test Scenario                                 | Status   | Observed vs Expected
-----------------------------------------------------------------------------------------------
1   | Step A: checkout.session.completed -> active  | [PASS]   | HTTP 200, status=active, plan=growth
2   | Step B: invoice.payment_failed -> past_due    | [PASS]   | HTTP 200, status=past_due, fail_count=1, grace_in_future=true
3   | Step C: duplicate event id -> idempotent 200  | [PASS]   | HTTP 200, fail_count=1
4   | Step D: tampered signature -> 400 + counter++ | [PASS]   | HTTP 400, sig_err_before=0, sig_err_after=1
5   | Step E: replayed old timestamp (>300s) -> 400 | [PASS]   | HTTP 400, ts_err_before=0, ts_err_after=1
6   | Step F: grace elapsed + final failure -> cut  | [PASS]   | HTTP 200, status=canceled, active_keys=0, is_active=false
7   | Step G: invoice.paid recovery -> active + key | [PASS]   | HTTP 200, status=active, fail_count=0, grace_is_null=true, active_keys=1, is_active=true
-----------------------------------------------------------------------------------------------

Final Billing Flow Verdict: CERTIFIED
Certification audit report saved to: logs/billing_flow_report.json
```

### 4.3 Script Exit Codes
| Exit Code | Meaning | Remediation |
|---|---|---|
| `0` | **CERTIFIED** | All 7 lifecycle transitions and security assertions passed cleanly. |
| `1` | **Assertion Failure** | Database state did not transition as expected; inspect `logs/billing_flow_report.json`. |
| `2` | **Connection Error** | Gateway or PostgreSQL container is unreachable. |
| `3` | **Signature Regression** | HMAC verification failed on valid vectors or passed on tampered vectors. |

---

## 5. Dunning State Machine, Grace Periods & Customer Communications

Institutional quantitative funds require professional, contract-aligned communications upon billing events. Sudden, unannounced API access revocation disrupts live portfolio execution and violates institutional trust.

### 5.1 Dunning Timeline & State Transitions
| Day | Event / Trigger | Subscription Status | API Keys State | Customer Action Required |
|---|---|---|---|---|
| **D0** | Initial `invoice.payment_failed` received | `past_due` (fail_count: 1) | **Active** (Full access) | Update billing method via portal |
| **D0 + 48h** | Retry payment failure #2 | `past_due` (fail_count: 2) | **Active** (Full access) | Reminder email to Billing & Tech contacts |
| **D0 + 72h** | Final payment failure #3 OR grace elapsed | `canceled` (fail_count: >=3) | **Suspended** (`revoked_at` set) | Account suspended; contact support |
| **Recovery** | `invoice.paid` event received | `active` (fail_count: 0) | **Restored** (`revoked_at = NULL`) | Immediate automated restoration |

### 5.2 Institutional Customer Communications Templates

#### Template 1: D0 Initial Payment Failure (Grace Period Notice)
```text
Subject: [Notice] FinText Alpha Vectorizer — Payment Action Required (72-Hour Grace Period Active)
To: {billing_email}, {admin_email}

Dear {customer_name} Treasury & Quantitative Operations Team,

We were unable to process the recurring monthly subscription invoice ({invoice_id}) for your FinText Alpha Vectorizer {plan_name} Tier ($ {invoice_amount} USD).

In accordance with our Institutional Beta Agreement, your API services and data feeds remain FULLY OPERATIONAL under our 72-hour institutional grace period. No service interruption has occurred.

Grace Period Expiration: {grace_until_utc} UTC

To prevent automated service suspension upon expiration of this grace window, please update your payment method or approve the pending corporate debit:
  -> https://billing.fintext.internal/portal?session={session_token}

If you require an official invoice PDF, wire transfer instructions (ACH/Fedwire), or purchase order reconciliation, please reply directly to this notice.

Sincerely,
FinText Institutional Treasury & Revenue Assurance
```

#### Template 2: D0 + 48h Urgent Reminder (24 Hours Remaining)
```text
Subject: [URGENT] 24 Hours Remaining: FinText Alpha Vectorizer Grace Period Expiration
To: {billing_email}, {admin_email}, {secondary_technical_contact}

Dear {customer_name} Operations Team,

This is a reminder that 24 hours remain in the institutional grace period for invoice {invoice_id}. 

Grace Period Deadline: {grace_until_utc} UTC

Unless payment is confirmed prior to this deadline, your institutional API keys will be placed in Phase-1 administrative suspension at {grace_until_utc} UTC.

To resolve immediately via corporate card or ACH:
  -> https://billing.fintext.internal/portal?session={session_token}

Our institutional support desk is on standby to assist: support@fintext.internal.
```

#### Template 3: D0 + 72h Service Suspension Notice (Zero Data Loss)
```text
Subject: [Notice of Service Suspension] FinText Alpha Vectorizer Account Suspended
To: {billing_email}, {admin_email}, {executive_contact}

Dear {customer_name} Leadership,

As the 72-hour institutional grace period has elapsed without receipt of payment for invoice {invoice_id}, your FinText Alpha Vectorizer API keys have been temporarily suspended.

Compliance & Data Preservation Guarantee:
In strict compliance with SEC Rule 17a-4 and FINRA Rule 4511, all custom universes, historical query telemetry, audit records, and workspace configurations remain FULLY PRESERVED and protected under strict Row-Level Security isolation. ZERO CUSTOMER DATA HAS BEEN DESTROYED.

To restore service immediately:
Upon successful settlement of outstanding balance ({invoice_id}), all API keys and service access will be automatically re-enabled within 60 seconds without requiring key re-provisioning.

Settlement Portal: https://billing.fintext.internal/portal?session={session_token}
```

#### Template 4: Immediate Reactivation Notice
```text
Subject: [Reactivated] FinText Alpha Vectorizer — Service Restored
To: {billing_email}, {admin_email}

Dear {customer_name} Team,

Payment for invoice {invoice_id} ($ {invoice_amount} USD) has been successfully verified. 

Your account status has been restored to ACTIVE. All existing API keys have been re-enabled and are immediately operational across all endpoints.

Thank you for your partnership.
```

---

## 6. Outage & Disaster Recovery: Stripe Webhook Replay Playbook

If the FinText API Gateway experiences network partition, ingress failure, or maintenance downtime, Stripe automatically retries failed webhook deliveries according to an exponential backoff schedule (up to 3 days). 

### 6.1 Stripe Dashboard Manual Replay
1. Navigate to **Developers > Webhooks > Endpoint URL**.
2. Filter event logs by **Status: Failed** (HTTP 5xx or Connection Refused).
3. Click on the failed event, then click **Resend event**.
4. To bulk-resend events from an outage window:
   - Use the Stripe CLI to replay events over a specific timestamp interval:
   ```bash
   stripe events resend evt_1234567890
   ```

### 6.2 Idempotency Proof Under Outage Replay
FinText's database schema guarantees that replaying any event—regardless of how many times it is resent—is 100% idempotent.
- `billing_events` contains `stripe_event_id UNIQUE`.
- `INSERT INTO billing_events (stripe_event_id, ...) ON CONFLICT (stripe_event_id) DO NOTHING`.
- If the event was already recorded, the gateway responds with `HTTP 200 OK: Event already processed (idempotent duplicate)` and performs zero state mutations.
- Confirmed verified by Drill Scenario 3 (`Step C: duplicate event id -> idempotent 200`).

---

## 7. Refunds, Credits & Dispute Handling

1. **Credit / Proration Calculation**:
   - Upgrades between tiers during an active billing cycle are prorated by Stripe.
   - The resulting `customer.subscription.updated` event automatically updates `subscriptions.plan_id` in PostgreSQL without changing billing cycle dates.
2. **Refund Authorization**:
   - Refunds require dual authorization from CTO and Lead Quant Portfolio Manager.
   - Process refund in Stripe Dashboard under **Payments > Issue Refund**.
   - Record internal audit justification in PostgreSQL:
   ```sql
   INSERT INTO audit_logs (id, org_id, user_id, action, resource_type, details, created_at)
   VALUES (gen_random_uuid(), 'org_uuid', 'cto_user_id', 'BILLING_REFUND_ISSUED', 'subscription', '{"reason": "Service level agreement latency credit", "amount_usd": 250}', NOW());
   ```

---

## 8. Institutional Procurement: PO Numbers & Invoice PDFs

Quantitative funds frequently require formal Purchase Orders (POs) and vendor W-9 forms rather than direct credit card billing.
1. **Invoice PDF Retrieval**:
   - Institutional clients can retrieve tax-compliant invoice PDFs directly from Stripe or via `/v1/usage/stats`.
2. **Purchase Order Numbers**:
   - PO numbers can be attached to subscriptions via Stripe metadata: `metadata.po_number = "PO-2026-QUANT-881"`.
   - FinText includes the PO number on all generated invoices.
3. **Vendor Compliance Forms**:
   - Form W-9, banking wire instructions, and SOC2 Type II reports are accessible via the secure data room per onboarding guidelines.

---

## 9. Monthly Accounting Close & Reconciliation Procedure

On the 1st of every month at 03:00 UTC, the automated reconciliation engine runs via Kubernetes CronJob `fintext-monthly-billing-reconciliation`. Operations engineers can also run it manually.

```bash
# Execute manual monthly reconciliation for previous month:
python scripts/reconcile_billing.py --period 2026-08 --mode offline

# Verify reconciliation audit report:
cat logs/billing_reconciliation_report.json | jq .latest_verdict
# Expected output: "RECONCILED"
```

---

## 10. Operational Troubleshooting Matrix

| Symptom | Root Cause | Immediate Diagnostic Action | Remediation |
|---|---|---|---|
| **Stripe webhook returns HTTP 400** | Signature mismatch or clock skew | Check API logs for `Invalid signature` vs `Timestamp tolerance exceeded` | Verify `STRIPE_WEBHOOK_SECRET` in secret store; verify NTP clock synchronization on host. |
| **HTTP 400 on valid secret** | Raw body canonicalization bug | Check if gateway parsed JSON before signature check | Ensure `verify_stripe_signature` uses raw body bytes extracted directly from HTTP payload. |
| **Subscription stuck in `past_due`** | Customer paid but `invoice.paid` not received | Check Stripe Dashboard for payment status and event webhook delivery logs | Re-send `invoice.paid` event from Stripe Dashboard; check `billing_events` table for processing status. |
| **Duplicate delivery alarms** | Normal Stripe retry behavior | Inspect `billing_events` for duplicate row conflict | No action needed; idempotent 200 response is design invariant. |
| **Tenant suspended unexpectedly** | 72h grace expired after 3 failed retries | Inspect `subscriptions.dunning_fail_count` and `subscriptions.grace_until_utc` | If payment was made outside Stripe (e.g. wire transfer), manually update subscription status to `active` via admin tooling. |
