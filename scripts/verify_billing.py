#!/usr/bin/env python3
"""
═══════════════════════════════════════════════════════════════════════════════
FinText-Alpha-Vectorizer — Test Suite #205: Stripe Subscription & Billing Engine
═══════════════════════════════════════════════════════════════════════════════
Validates:
  1. Unauthenticated Access Rejection on Protected Endpoints (401).
  2. Input Validation & Plan Validation (Free plan rejected, unknown plan rejected) (400).
  3. Stripe Checkout Session Creation (`POST /billing/checkout`) for Pro and Enterprise.
  4. Stripe Customer Portal Session Creation (`POST /billing/portal`).
  5. Subscription Status Retrieval (`GET /billing/subscription`) with Plan Limits & Usage.
  6. Stripe Webhook Ingestion (`POST /billing/webhook`) for Lifecycle Events:
     - checkout.session.completed
     - invoice.payment_succeeded
     - invoice.payment_failed
     - customer.subscription.deleted
     - malformed payload rejection (400)
  7. Webhook HMAC-SHA256 Signature Verification logic.
  8. Rate Limiting & Response Headers on Billing Endpoints.
  9. OpenAPI 3.0 Conformance & Schema Registration.
  10. Python SDK Sync & Async Client Execution.
═══════════════════════════════════════════════════════════════════════════════
"""

import asyncio
import hashlib
import hmac
import json
import os
import subprocess
import sys
import time
from pathlib import Path
from typing import Optional

import httpx

ROOT_DIR = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT_DIR / "python_sdk" / "src"))
if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")

from fintext import FinTextClient, FinTextAsyncClient
from fintext.models import CheckoutResponse, PortalResponse, SubscriptionResponse

SERVER_PORT = 8098
BASE_URL = f"http://127.0.0.1:{SERVER_PORT}"
ADMIN_TOKEN = "fintext-admin-dev-secret-token"
JWT_SECRET = "fintext-alpha-vectorizer-institutional-jwt-secret-key-2026"
STRIPE_WEBHOOK_SECRET = "whsec_fintext_test_webhook_signing_secret_2026"


class ServerContext:
    def __init__(self):
        self.process: Optional[subprocess.Popen] = None

    def __enter__(self):
        env = os.environ.copy()
        env["PORT"] = str(SERVER_PORT)
        env["ADMIN_TOKEN"] = ADMIN_TOKEN
        env["JWT_SECRET"] = JWT_SECRET
        env["QUESTDB_MOCK_FALLBACK"] = "1"
        env["POLYGON_MOCK_FALLBACK"] = "1"
        env["WHISPER_MOCK_FALLBACK"] = "1"
        env["NATS_MOCK_MODE"] = "1"
        env["STRIPE_WEBHOOK_SECRET"] = STRIPE_WEBHOOK_SECRET

        candidates = [
            ROOT_DIR / "rust" / "target" / "release" / "fintext_api.exe",
            ROOT_DIR / "rust" / "target" / "debug" / "fintext_api.exe",
        ]
        valid_candidates = [p for p in candidates if p.exists()]
        if not valid_candidates:
            raise RuntimeError(f"Server binary not found at any candidate location. Build it first.")
        exe_path = max(valid_candidates, key=lambda p: p.stat().st_mtime)

        print(f"[STARTING] Spawning FinText API Server from {exe_path} on port {SERVER_PORT}...")
        self.process = subprocess.Popen(
            [str(exe_path)],
            env=env,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )

        max_attempts = 50
        for i in range(max_attempts):
            try:
                resp = httpx.get(f"{BASE_URL}/health", timeout=0.5)
                if resp.status_code == 200:
                    print(f"[READY] Server is healthy and ready (attempt {i + 1})")
                    return self
            except Exception:
                time.sleep(0.1)

        if self.process.poll() is not None:
            raise RuntimeError(f"Server failed to start (exit code {self.process.poll()}).")
        raise RuntimeError("Server health check timed out.")

    def __exit__(self, exc_type, exc_val, exc_tb):
        if self.process:
            print("[STOPPING] Shutting down FinText API Server...")
            self.process.terminate()
            try:
                self.process.wait(timeout=3)
            except subprocess.TimeoutExpired:
                self.process.kill()


def get_token(user_id: str) -> str:
    resp = httpx.post(
        f"{BASE_URL}/auth/token",
        json={"user_id": user_id},
        headers={"X-Admin-Token": ADMIN_TOKEN},
    )
    assert resp.status_code == 200, f"Failed to get token for {user_id}: {resp.text}"
    return resp.json()["token"]


def generate_stripe_signature(payload_bytes: bytes, secret: str, timestamp: Optional[int] = None) -> str:
    if timestamp is None:
        timestamp = int(time.time())
    signed_payload = f"{timestamp}.".encode("utf-8") + payload_bytes
    sig = hmac.new(secret.encode("utf-8"), signed_payload, hashlib.sha256).hexdigest()
    return f"t={timestamp},v1={sig}"


def run_tests():
    print("=" * 80)
    print("FINTEXT ALPHA VECTORIZER — TEST SUITE #205: STRIPE SUBSCRIPTION & BILLING ENGINE")
    print("=" * 80)

    token = get_token("quant_billing_analyst_01")
    auth_headers = {"Authorization": f"Bearer {token}"}

    client = FinTextClient(base_url=BASE_URL, api_token=token)

    with httpx.Client(base_url=BASE_URL, timeout=10.0) as http:
        # ─────────────────────────────────────────────────────────────────────
        # Phase 1: Unauthenticated Access Rejection (401)
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 1] Testing Unauthenticated Access Rejection on Protected Routes...")
        r = http.post("/billing/checkout", json={"plan_id": "pro_monthly"})
        assert r.status_code == 401, f"Expected 401 on checkout, got {r.status_code}"

        r = http.post("/billing/portal", json={})
        assert r.status_code == 401, f"Expected 401 on portal, got {r.status_code}"

        r = http.get("/billing/subscription")
        assert r.status_code == 401, f"Expected 401 on subscription, got {r.status_code}"

        r = http.get("/billing/subscription", headers={"Authorization": "Bearer invalid_token"})
        assert r.status_code == 401, f"Expected 401 on invalid token, got {r.status_code}"
        print("[PASS] Phase 1 Passed: Unauthorized requests correctly rejected (401).")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 2: Input Validation & Plan Validation
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 2] Testing Input Validation & Plan Validation...")
        # Free plan does not require checkout
        r = http.post("/billing/checkout", json={"plan_id": "free"}, headers=auth_headers)
        assert r.status_code == 400, f"Expected 400 for free plan checkout, got {r.status_code}"
        assert "does not require checkout" in r.text

        # Unknown plan
        r = http.post("/billing/checkout", json={"plan_id": "nonexistent_plan_xyz"}, headers=auth_headers)
        assert r.status_code == 400, f"Expected 400 for invalid plan, got {r.status_code}"
        assert "Unknown plan" in r.text

        print("[PASS] Phase 2 Passed: Invalid plan requests rejected with clear diagnostic (400).")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 3: Stripe Checkout Session Creation
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 3] Testing Stripe Checkout Session Creation...")
        # Pro monthly plan
        r = http.post(
            "/billing/checkout",
            json={
                "plan_id": "pro_monthly",
                "success_url": "https://app.fintext.io/billing/success",
                "cancel_url": "https://app.fintext.io/billing/cancel",
            },
            headers=auth_headers,
        )
        assert r.status_code == 200, f"Expected 200, got {r.status_code}: {r.text}"
        data = r.json()
        assert "checkout_url" in data
        assert "stripe.com" in data["checkout_url"]
        assert "session_id" in data
        print(f"[PASS] Phase 3a: Pro Checkout session created -> URL: {data['checkout_url'][:45]}... (session: {data['session_id']})")

        # Enterprise monthly plan
        r = http.post(
            "/billing/checkout",
            json={"plan_id": "enterprise_monthly"},
            headers=auth_headers,
        )
        assert r.status_code == 200, f"Expected 200, got {r.status_code}: {r.text}"
        data = r.json()
        assert "checkout_url" in data
        print(f"[PASS] Phase 3b: Enterprise Checkout session created -> URL: {data['checkout_url'][:45]}...")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 4: Stripe Customer Portal Session Creation
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 4] Testing Stripe Customer Portal Session Creation...")
        r = http.post(
            "/billing/portal",
            json={"return_url": "https://app.fintext.io/dashboard"},
            headers=auth_headers,
        )
        assert r.status_code == 200, f"Expected 200, got {r.status_code}: {r.text}"
        data = r.json()
        assert "portal_url" in data
        assert "stripe.com" in data["portal_url"]
        print(f"[PASS] Phase 4 Passed: Customer portal session created -> URL: {data['portal_url']}")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 5: Subscription Status Retrieval
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 5] Testing Subscription Status Retrieval...")
        r = http.get("/billing/subscription", headers=auth_headers)
        assert r.status_code == 200, f"Expected 200, got {r.status_code}: {r.text}"
        sub = r.json()
        assert sub["plan_id"] == "free"
        assert sub["plan_name"] == "Free Tier"
        assert sub["status"] == "active"
        assert sub["monthly_request_limit"] == 1000
        assert isinstance(sub["current_usage"], int)
        print(f"[PASS] Phase 5 Passed: Default subscription -> Plan: {sub['plan_name']} (Limit: {sub['monthly_request_limit']} reqs/mo, Usage: {sub['current_usage']})")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 6: Stripe Webhook Ingestion & Lifecycle Events
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 6] Testing Stripe Webhook Ingestion & Signatures...")

        # 6a. Valid webhook with HMAC-SHA256 signature
        event_payload = {
            "id": "evt_test_checkout_001",
            "type": "checkout.session.completed",
            "data": {
                "object": {
                    "id": "cs_test_123",
                    "customer": "cus_test_quant_01",
                    "subscription": "sub_test_quant_01",
                    "client_reference_id": "quant_billing_analyst_01",
                    "metadata": {
                        "fintext_user_id": "quant_billing_analyst_01",
                        "plan_id": "pro_monthly",
                    },
                }
            },
        }
        body_bytes = json.dumps(event_payload).encode("utf-8")
        sig_header = generate_stripe_signature(body_bytes, STRIPE_WEBHOOK_SECRET)

        r = http.post(
            "/billing/webhook",
            content=body_bytes,
            headers={"Content-Type": "application/json", "Stripe-Signature": sig_header},
        )
        assert r.status_code == 200, f"Expected 200 for valid webhook, got {r.status_code}: {r.text}"
        assert r.json()["received"] is True
        print("[PASS] Phase 6a: checkout.session.completed webhook processed successfully.")

        # 6b. invoice.payment_succeeded
        invoice_payload = {
            "id": "evt_test_inv_001",
            "type": "invoice.payment_succeeded",
            "data": {
                "object": {
                    "customer": "cus_test_quant_01",
                    "subscription": "sub_test_quant_01",
                    "period_start": 1787940000,
                    "period_end": 1790532000,
                }
            },
        }
        body_bytes = json.dumps(invoice_payload).encode("utf-8")
        sig_header = generate_stripe_signature(body_bytes, STRIPE_WEBHOOK_SECRET)

        r = http.post(
            "/billing/webhook",
            content=body_bytes,
            headers={"Content-Type": "application/json", "Stripe-Signature": sig_header},
        )
        assert r.status_code == 200
        print("[PASS] Phase 6b: invoice.payment_succeeded webhook processed successfully.")

        # 6c. invoice.payment_failed
        failed_payload = {
            "id": "evt_test_inv_fail",
            "type": "invoice.payment_failed",
            "data": {
                "object": {
                    "customer": "cus_test_quant_01",
                }
            },
        }
        body_bytes = json.dumps(failed_payload).encode("utf-8")
        sig_header = generate_stripe_signature(body_bytes, STRIPE_WEBHOOK_SECRET)

        r = http.post(
            "/billing/webhook",
            content=body_bytes,
            headers={"Content-Type": "application/json", "Stripe-Signature": sig_header},
        )
        assert r.status_code == 200
        print("[PASS] Phase 6c: invoice.payment_failed webhook processed successfully.")

        # 6d. customer.subscription.deleted
        deleted_payload = {
            "id": "evt_test_sub_del",
            "type": "customer.subscription.deleted",
            "data": {
                "object": {
                    "customer": "cus_test_quant_01",
                }
            },
        }
        body_bytes = json.dumps(deleted_payload).encode("utf-8")
        sig_header = generate_stripe_signature(body_bytes, STRIPE_WEBHOOK_SECRET)

        r = http.post(
            "/billing/webhook",
            content=body_bytes,
            headers={"Content-Type": "application/json", "Stripe-Signature": sig_header},
        )
        assert r.status_code == 200
        print("[PASS] Phase 6d: customer.subscription.deleted webhook processed successfully.")

        # 6e. Tampered signature rejection
        tampered_sig = generate_stripe_signature(b"tampered_body", STRIPE_WEBHOOK_SECRET)
        r = http.post(
            "/billing/webhook",
            content=body_bytes,
            headers={"Content-Type": "application/json", "Stripe-Signature": tampered_sig},
        )
        assert r.status_code == 400, f"Expected 400 for tampered signature, got {r.status_code}"
        print("[PASS] Phase 6e: Tampered webhook signature correctly rejected (400).")

        # 6f. Malformed JSON payload rejection
        r = http.post(
            "/billing/webhook",
            content=b"not_valid_json",
            headers={"Content-Type": "application/json", "Stripe-Signature": generate_stripe_signature(b"not_valid_json", STRIPE_WEBHOOK_SECRET)},
        )
        assert r.status_code == 400
        print("[PASS] Phase 6f: Malformed JSON payload rejected (400).")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 7: Rate Limit Headers on Billing Endpoints
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 7] Testing Rate Limit Headers on Billing Endpoints...")
        r = http.get("/billing/subscription", headers=auth_headers)
        assert r.status_code == 200
        assert "x-ratelimit-limit" in r.headers
        assert "x-ratelimit-remaining" in r.headers
        assert "x-ratelimit-reset" in r.headers
        print(f"[PASS] Phase 7 Passed: Rate limit headers present -> Limit: {r.headers.get('x-ratelimit-limit')}, Remaining: {r.headers.get('x-ratelimit-remaining')}")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 8: OpenAPI 3.0 Conformance
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 8] Testing OpenAPI 3.0 Conformance for Billing...")
        r = http.get("/api-docs/openapi.json")
        assert r.status_code == 200
        spec = r.json()
        paths = spec["paths"]
        assert "/billing/checkout" in paths
        assert "/billing/portal" in paths
        assert "/billing/subscription" in paths
        assert "/billing/webhook" in paths

        schemas = spec["components"]["schemas"]
        assert "CheckoutRequest" in schemas
        assert "CheckoutResponse" in schemas
        assert "PortalRequest" in schemas
        assert "PortalResponse" in schemas
        assert "SubscriptionResponse" in schemas
        assert "BillingWebhookResponse" in schemas
        print("[PASS] Phase 8 Passed: OpenAPI spec contains all 4 billing endpoints and 6 schemas.")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 9: Python SDK Client (Synchronous)
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 9] Testing Python SDK Client (Synchronous)...")
        # 9a. Checkout session
        checkout = client.create_checkout_session(
            "pro_monthly",
            success_url="https://app.fintext.io/success",
            cancel_url="https://app.fintext.io/cancel",
        )
        assert isinstance(checkout, CheckoutResponse)
        assert "stripe.com" in checkout.checkout_url
        print(f"[PASS] Phase 9a: SDK client.create_checkout_session -> {checkout.checkout_url[:50]}...")

        # 9b. Customer portal
        portal = client.create_portal_session(return_url="https://app.fintext.io/dash")
        assert isinstance(portal, PortalResponse)
        assert "stripe.com" in portal.portal_url
        print(f"[PASS] Phase 9b: SDK client.create_portal_session -> {portal.portal_url}")

        # 9c. Subscription
        sub_resp = client.get_subscription()
        assert isinstance(sub_resp, SubscriptionResponse)
        assert sub_resp.plan_id == "free"
        assert sub_resp.monthly_request_limit == 1000
        print(f"[PASS] Phase 9c: SDK client.get_subscription -> Plan: {sub_resp.plan_name}")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 10: Python SDK Client (Asynchronous)
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 10] Testing Python SDK Client (Asynchronous)...")

        async def run_async_sdk_tests():
            async with FinTextAsyncClient(base_url=BASE_URL, api_token=token) as aclient:
                # Async checkout
                co = await aclient.create_checkout_session("enterprise_monthly")
                assert isinstance(co, CheckoutResponse)

                # Async portal
                po = await aclient.create_portal_session()
                assert isinstance(po, PortalResponse)

                # Async subscription
                su = await aclient.get_subscription()
                assert isinstance(su, SubscriptionResponse)
                assert su.plan_id == "free"

        asyncio.run(run_async_sdk_tests())
        print("[PASS] Phase 10 Passed: Async SDK methods executed and validated.")

    print("\n" + "=" * 80)
    print("ALL 10 PHASES PASSED — STRIPE SUBSCRIPTION & BILLING INTEGRATION FULLY VERIFIED")
    print("=" * 80)


if __name__ == "__main__":
    with ServerContext():
        run_tests()
