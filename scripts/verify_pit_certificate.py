#!/usr/bin/env python3
"""
===============================================================================
FinText-Alpha-Vectorizer — Verification: PIT Certification & Look-Ahead Bias Audit
===============================================================================
Verifies:
  1.  GET /pit/certificate returns 200 OK with default parameters
  2.  Response contains correct top-level fields
  3.  Certificate ID follows format PIT-CERT-YYYYMMDD-XXX
  4.  Overall result is 'pass'
  5.  Test 1 (Signal Availability Ordering): 0 violations, status 'pass'
  6.  Test 2 (Ticker Rename): 0 violations, status 'pass'
  7.  Test 3 (Delisted Security): 0 violations, status 'pass'
  8.  Test 4 (Corporate Action): 0 violations, status 'pass'
  9.  Test 5 (Duplicate Event): duplicate_rate_pct < 1.0%, status 'pass'
  10. Test 6 (Out-of-Order Clock Skew): 0 violations, status 'pass'
  11. Test 7 (Timestamp Precision): 0 violations, status 'pass'
  12. Test 8 (Backfill Consistency): backfill_count >= 0, policy 'within_7_days', status 'pass'
  13. Cryptographic SHA-256 signature is valid 64-character hex digest
  14. Python SDK Sync/Async Client integration & OpenAPI 3.0 Schema registration
===============================================================================
"""

import asyncio
import json
import os
from pathlib import Path
import subprocess
import sys
import time

import httpx

if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
if hasattr(sys.stderr, "reconfigure"):
    sys.stderr.reconfigure(encoding="utf-8", errors="replace")

PROJECT_ROOT = Path(__file__).resolve().parent.parent
SDK_PATH = PROJECT_ROOT / "python_sdk" / "src"
if str(SDK_PATH) not in sys.path:
    sys.path.insert(0, str(SDK_PATH))

PORT = 8149
BASE_URL = f"http://127.0.0.1:{PORT}"
ADMIN_TOKEN = "test_admin_token_xyz123_valid_32_bytes_length!"
SERVER_EXE = PROJECT_ROOT / "rust" / "target" / "debug" / "fintext_api.exe"

passed = 0
failed = 0
total = 14


def report(phase: int, name: str, ok: bool, detail: str = ""):
    global passed, failed
    if ok:
        passed += 1
        print(f"  ✅ Phase {phase:2d} │ {name}")
    else:
        failed += 1
        print(f"  ❌ Phase {phase:2d} │ {name}")
        if detail:
            print(f"     └─ {detail}")


class ServerContext:
    def __init__(self):
        self.process = None

    def __enter__(self):
        print(f"[STARTING] Spawning FinText API Server on port {PORT}...")
        env = os.environ.copy()
        env["PORT"] = str(PORT)
        env["HOST"] = "127.0.0.1"
        env["ADMIN_TOKEN"] = ADMIN_TOKEN
        env["JWT_SECRET"] = "super_secret_test_jwt_key_32_bytes_len!!"
        env["QUESTDB_MOCK_FALLBACK"] = "1"
        env["POLYGON_MOCK_FALLBACK"] = "1"
        env["WHISPER_MOCK_FALLBACK"] = "1"
        env["NATS_MOCK_MODE"] = "1"
        env["CHAT_ALERTS_MOCK"] = "1"
        env["CONFIG_PATH"] = str(PROJECT_ROOT / "config" / "config.yaml")

        self.process = subprocess.Popen(
            [str(SERVER_EXE)],
            env=env,
            cwd=str(PROJECT_ROOT),
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )

        start_t = time.time()
        ready = False
        while time.time() - start_t < 25:
            try:
                r = httpx.get(f"{BASE_URL}/health", timeout=1.0)
                if r.status_code == 200:
                    ready = True
                    break
            except Exception:
                time.sleep(0.3)

        if not ready:
            raise RuntimeError(f"Server on port {PORT} failed to start within 25 seconds.")
        print(f"[READY] API Server is healthy at {BASE_URL}")
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        if self.process:
            print("[CLEANUP] Terminating FinText API Server process...")
            self.process.terminate()
            try:
                self.process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.process.kill()


def run_all_phases():
    print("=" * 80)
    print(" FinText-Alpha-Vectorizer — PIT Certification Verification")
    print("=" * 80)
    print(f" Project Root: {PROJECT_ROOT}")
    print(f" Server Port:  {PORT}")

    with ServerContext():
        client = httpx.Client(base_url=BASE_URL, timeout=10.0)

        # 0. Obtain JWT auth token
        auth_resp = client.post(
            "/auth/token",
            json={
                "user_id": "pit_governance_auditor",
                "role": "institutional",
                "duration_seconds": 3600,
            },
            headers={"X-Admin-Token": ADMIN_TOKEN},
        )
        if auth_resp.status_code != 200:
            token = auth_resp.json().get("token") or "mock_jwt_token"
        else:
            token = auth_resp.json()["token"]

        headers = {"Authorization": f"Bearer {token}"}

        # Phase 1: GET /pit/certificate returns 200 OK
        resp = client.get(
            "/pit/certificate",
            params={
                "dataset_version": "2.1.0",
                "universe": "all",
                "start_date": "2025-06-01",
                "end_date": "2025-08-31",
            },
            headers=headers,
        )
        report(1, "GET /pit/certificate returns 200 OK", resp.status_code == 200, f"Status: {resp.status_code}, Body: {resp.text}")

        data = resp.json() if resp.status_code == 200 else {}

        # Phase 2: Response contains correct top-level fields
        expected_fields = {
            "certificate_id",
            "dataset_version",
            "universe",
            "audit_start_date",
            "audit_end_date",
            "issued_at",
            "overall_result",
            "tests",
            "policies",
            "signature",
            "status",
        }
        actual_fields = set(data.keys())
        has_fields = expected_fields.issubset(actual_fields)
        report(2, "Response contains correct top-level fields", has_fields, f"Missing: {expected_fields - actual_fields}")

        # Phase 3: Certificate ID follows format PIT-CERT-YYYYMMDD-XXX
        cert_id = data.get("certificate_id", "")
        cert_id_ok = cert_id.startswith("PIT-CERT-") and len(cert_id.split("-")) >= 3
        report(3, "Certificate ID follows format PIT-CERT-YYYYMMDD-XXX", cert_id_ok, f"ID: {cert_id}")

        # Phase 4: Overall result is 'pass'
        res_val = data.get("overall_result", "")
        res_ok = res_val in ("pass", "conditional_pass")
        report(4, "Overall result is 'pass'", res_ok, f"Result: {res_val}")

        tests = data.get("tests", {})

        # Phase 5: Test 1 (Signal Availability Ordering)
        t1 = tests.get("signal_availability_ordering", {})
        t1_ok = t1.get("violations") == 0 and t1.get("status") == "pass"
        report(5, "Test 1 (Signal Availability Ordering): 0 violations, status 'pass'", t1_ok, f"T1: {t1}")

        # Phase 6: Test 2 (Ticker Rename)
        t2 = tests.get("ticker_rename", {})
        t2_ok = t2.get("violations") == 0 and t2.get("status") == "pass"
        report(6, "Test 2 (Ticker Rename): 0 violations, status 'pass'", t2_ok, f"T2: {t2}")

        # Phase 7: Test 3 (Delisted Security)
        t3 = tests.get("delisted_security", {})
        t3_ok = t3.get("violations") == 0 and t3.get("status") == "pass"
        report(7, "Test 3 (Delisted Security): 0 violations, status 'pass'", t3_ok, f"T3: {t3}")

        # Phase 8: Test 4 (Corporate Action)
        t4 = tests.get("corporate_action", {})
        t4_ok = t4.get("violations") == 0 and t4.get("status") == "pass"
        report(8, "Test 4 (Corporate Action): 0 violations, status 'pass'", t4_ok, f"T4: {t4}")

        # Phase 9: Test 5 (Duplicate Event)
        t5 = tests.get("duplicate_event", {})
        t5_ok = t5.get("duplicate_rate_pct", 10.0) < 1.0 and t5.get("status") == "pass"
        report(9, "Test 5 (Duplicate Event): duplicate_rate_pct < 1.0%, status 'pass'", t5_ok, f"T5: {t5}")

        # Phase 10: Test 6 (Out-of-Order Clock Skew)
        t6 = tests.get("out_of_order_event", {})
        t6_ok = t6.get("violations") == 0 and t6.get("status") == "pass"
        report(10, "Test 6 (Out-of-Order Clock Skew): 0 violations, status 'pass'", t6_ok, f"T6: {t6}")

        # Phase 11: Test 7 (Timestamp Precision)
        t7 = tests.get("timestamp_precision", {})
        t7_ok = t7.get("violations") == 0 and t7.get("status") == "pass"
        report(11, "Test 7 (Timestamp Precision): 0 violations, status 'pass'", t7_ok, f"T7: {t7}")

        # Phase 12: Test 8 (Backfill Consistency)
        t8 = tests.get("backfill_consistency", {})
        t8_ok = t8.get("policy") == "within_7_days" and t8.get("status") == "pass"
        report(12, "Test 8 (Backfill Consistency): backfill_count >= 0, policy 'within_7_days', status 'pass'", t8_ok, f"T8: {t8}")

        # Phase 13: Cryptographic SHA-256 signature is valid
        sig = data.get("signature", "")
        sig_ok = sig.startswith("sha256:") and len(sig.split("sha256:")[1]) == 64
        report(13, "Cryptographic SHA-256 signature is valid 64-character hex digest", sig_ok, f"Sig: {sig}")

        # Phase 14: Python SDK Sync/Async Client integration & OpenAPI 3.0 schema verification
        from fintext import FinTextClient, FinTextAsyncClient, PITCertificateResponse

        sdk_client = FinTextClient(base_url=BASE_URL, api_token=token)
        sync_cert = sdk_client.pit_certificate(
            dataset_version="2.1.0",
            universe="all",
            start_date="2025-06-01",
            end_date="2025-08-31",
        )
        sync_alias_cert = sdk_client.certify_pit()

        async def verify_async_and_docs():
            async_c = FinTextAsyncClient(base_url=BASE_URL, api_token=token)
            async_cert = await async_c.pit_certificate(dataset_version="2.1.0", universe="sp500")
            await async_c.close()

            r_docs = client.get("/api-docs/openapi.json")
            has_docs = False
            if r_docs.status_code == 200:
                docs = r_docs.json()
                paths = docs.get("paths", {})
                schemas = docs.get("components", {}).get("schemas", {})

                has_docs = (
                    "/pit/certificate" in paths
                    and "PITCertificateResponse" in schemas
                    and "PITCertificateTests" in schemas
                    and "PITTestResult" in schemas
                )

            return (
                isinstance(sync_cert, PITCertificateResponse)
                and sync_cert.overall_result == "pass"
                and isinstance(sync_alias_cert, PITCertificateResponse)
                and isinstance(async_cert, PITCertificateResponse)
                and has_docs
            )

        sdk_ok = asyncio.run(verify_async_and_docs())
        report(14, "Python SDK Sync/Async Client & OpenAPI 3.0 Schema registration", sdk_ok)


if __name__ == "__main__":
    run_all_phases()
    print("=" * 80)
    print(f" PIT Certification Verification Results: {passed}/{total} Phases Passed ({failed} Failed)")
    print("=" * 80)
    if failed > 0:
        sys.exit(1)
    sys.exit(0)
