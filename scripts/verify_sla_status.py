#!/usr/bin/env python3
"""
===============================================================================
FinText-Alpha-Vectorizer — Suite #236: Latency SLA Reporting & Compliance Analytics Engine
===============================================================================
Verifies:
  1.  Unauthenticated GET /sla/status returns 401 Unauthorized
  2.  Invalid start date format (start_date=invalid-date) returns 400 Bad Request
  3.  start_date > end_date returns 400 Bad Request
  4.  sla_target_ms < 10 returns 400 Bad Request
  5.  sla_target_ms > 5000 returns 400 Bad Request
  6.  Invalid percentile value (percentiles=50,150) returns 400 Bad Request
  7.  Authenticated GET /sla/status with default parameters returns 200 OK with SLA metrics
  8.  Custom percentiles (percentiles=50,75,90,99.5) returns matching keys in percentiles dict
  9.  Tight SLA target (sla_target_ms=10) results in sla_status == "breached"
  10. Generous SLA target (sla_target_ms=2000) results in sla_status == "met" (100% compliance)
  11. Date range parameters (start_date=2025-08-01, end_date=2025-08-15) properly reflected in response
  12. Python SDK Sync Client integration verification (client.sla_status())
  13. Python SDK Async Client integration verification (await client.sla_status())
  14. OpenAPI specification verification (/sla/status path and SLAStatusResponse schema)
===============================================================================
"""

import asyncio
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

PORT = 8135
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
        print(f"  \u2705 Phase {phase:2d} \u2502 {name}")
    else:
        failed += 1
        msg = f"  \u274c Phase {phase:2d} \u2502 {name}"
        if detail:
            msg += f" \u2014 {detail}"
        print(msg)


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
                time.sleep(0.4)

        if not ready:
            if self.process:
                self.process.terminate()
            raise RuntimeError(f"Server on port {PORT} failed to become healthy within 25 seconds")

        print(f"[READY] FinText API Server healthy on {BASE_URL}")
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        if self.process:
            print("[STOPPING] Shutting down FinText API Server...")
            self.process.terminate()
            try:
                self.process.wait(timeout=5)
            except Exception:
                self.process.kill()


def get_auth_token(user_id: str = "sla_tester", role: str = "institutional") -> str:
    r = httpx.post(
        f"{BASE_URL}/auth/token",
        json={"user_id": user_id, "expires_in_seconds": 3600, "role": role},
        headers={"X-Admin-Token": ADMIN_TOKEN},
        timeout=5.0,
    )
    if r.status_code != 200:
        raise RuntimeError(f"Failed to obtain auth token: {r.text}")
    return r.json()["token"]


def run_tests():
    global passed, failed

    print("=" * 80)
    print(" FinText-Alpha-Vectorizer — Suite #236: Latency SLA Reporting & Analytics")
    print("=" * 80)

    token = get_auth_token("sla_quant_enterprise")
    auth_hdr = {"Authorization": f"Bearer {token}"}

    # Phase 1: Unauthenticated GET /sla/status
    try:
        r = httpx.get(f"{BASE_URL}/sla/status", timeout=5.0)
        report(1, "Unauthenticated GET /sla/status returns 401", r.status_code == 401)
    except Exception as e:
        report(1, "Unauthenticated GET /sla/status returns 401", False, str(e))

    # Phase 2: Invalid start date format
    try:
        r = httpx.get(f"{BASE_URL}/sla/status?start_date=invalid-date", headers=auth_hdr, timeout=5.0)
        report(2, "Invalid start_date format returns 400 Bad Request", r.status_code == 400)
    except Exception as e:
        report(2, "Invalid start_date format returns 400 Bad Request", False, str(e))

    # Phase 3: start_date > end_date
    try:
        r = httpx.get(f"{BASE_URL}/sla/status?start_date=2025-09-01&end_date=2025-08-01", headers=auth_hdr, timeout=5.0)
        report(3, "start_date > end_date returns 400 Bad Request", r.status_code == 400)
    except Exception as e:
        report(3, "start_date > end_date returns 400 Bad Request", False, str(e))

    # Phase 4: sla_target_ms < 10
    try:
        r = httpx.get(f"{BASE_URL}/sla/status?sla_target_ms=5", headers=auth_hdr, timeout=5.0)
        report(4, "sla_target_ms < 10 returns 400 Bad Request", r.status_code == 400)
    except Exception as e:
        report(4, "sla_target_ms < 10 returns 400 Bad Request", False, str(e))

    # Phase 5: sla_target_ms > 5000
    try:
        r = httpx.get(f"{BASE_URL}/sla/status?sla_target_ms=6000", headers=auth_hdr, timeout=5.0)
        report(5, "sla_target_ms > 5000 returns 400 Bad Request", r.status_code == 400)
    except Exception as e:
        report(5, "sla_target_ms > 5000 returns 400 Bad Request", False, str(e))

    # Phase 6: Invalid percentile value (percentiles=50,150)
    try:
        r = httpx.get(f"{BASE_URL}/sla/status?percentiles=50,150", headers=auth_hdr, timeout=5.0)
        report(6, "Invalid percentile value (150) returns 400 Bad Request", r.status_code == 400)
    except Exception as e:
        report(6, "Invalid percentile value (150) returns 400 Bad Request", False, str(e))

    # Phase 7: Authenticated GET /sla/status with default parameters
    try:
        r = httpx.get(f"{BASE_URL}/sla/status", headers=auth_hdr, timeout=5.0)
        if r.status_code == 200:
            data = r.json()
            ok = (
                data.get("total_requests", 0) > 0
                and data.get("average_latency_ms", 0.0) > 0.0
                and "p50" in data.get("percentiles", {})
                and "p95" in data.get("percentiles", {})
                and "p99" in data.get("percentiles", {})
                and data.get("sla_target_ms") == 100
                and "sla_compliance_rate" in data
                and data.get("sla_status") in ("met", "breached")
            )
            report(7, "Authenticated GET /sla/status defaults returns 200 with SLA metrics", ok, str(data))
        else:
            report(7, "Authenticated GET /sla/status defaults returns 200 with SLA metrics", False, f"Status: {r.status_code}")
    except Exception as e:
        report(7, "Authenticated GET /sla/status defaults returns 200 with SLA metrics", False, str(e))

    # Phase 8: Custom percentiles (percentiles=50,75,90,99.5)
    try:
        r = httpx.get(f"{BASE_URL}/sla/status?percentiles=50,75,90,99.5", headers=auth_hdr, timeout=5.0)
        if r.status_code == 200:
            data = r.json()
            pcts = data.get("percentiles", {})
            ok = (
                "p50" in pcts
                and "p75" in pcts
                and "p90" in pcts
                and "p99.5" in pcts
                and len(pcts) == 4
            )
            report(8, "Custom percentiles (50,75,90,99.5) returned accurately", ok, str(pcts))
        else:
            report(8, "Custom percentiles (50,75,90,99.5) returned accurately", False, f"Status: {r.status_code}")
    except Exception as e:
        report(8, "Custom percentiles (50,75,90,99.5) returned accurately", False, str(e))

    # Phase 9: Tight SLA target (sla_target_ms=10) results in "breached"
    try:
        r = httpx.get(f"{BASE_URL}/sla/status?sla_target_ms=10", headers=auth_hdr, timeout=5.0)
        if r.status_code == 200:
            data = r.json()
            ok = data.get("sla_status") == "breached" and data.get("sla_compliance_rate", 100.0) < 99.9
            report(9, "Tight SLA target (10ms) reports breached status", ok, f"Compliance: {data.get('sla_compliance_rate')}%, Status: {data.get('sla_status')}")
        else:
            report(9, "Tight SLA target (10ms) reports breached status", False, f"Status: {r.status_code}")
    except Exception as e:
        report(9, "Tight SLA target (10ms) reports breached status", False, str(e))

    # Phase 10: Generous SLA target (sla_target_ms=2000) results in "met" (100% compliance)
    try:
        r = httpx.get(f"{BASE_URL}/sla/status?sla_target_ms=2000", headers=auth_hdr, timeout=5.0)
        if r.status_code == 200:
            data = r.json()
            ok = data.get("sla_status") == "met" and data.get("sla_compliance_rate", 0.0) >= 99.9
            report(10, "Generous SLA target (2000ms) reports met status", ok, f"Compliance: {data.get('sla_compliance_rate')}%, Status: {data.get('sla_status')}")
        else:
            report(10, "Generous SLA target (2000ms) reports met status", False, f"Status: {r.status_code}")
    except Exception as e:
        report(10, "Generous SLA target (2000ms) reports met status", False, str(e))

    # Phase 11: Date range filtering (start_date=2025-08-01, end_date=2025-08-15)
    try:
        r = httpx.get(f"{BASE_URL}/sla/status?start_date=2025-08-01&end_date=2025-08-15", headers=auth_hdr, timeout=5.0)
        if r.status_code == 200:
            data = r.json()
            ok = data.get("start_date") == "2025-08-01" and data.get("end_date") == "2025-08-15"
            report(11, "Date range filtering properly reflected in response", ok, f"{data.get('start_date')} to {data.get('end_date')}")
        else:
            report(11, "Date range filtering properly reflected in response", False, f"Status: {r.status_code}")
    except Exception as e:
        report(11, "Date range filtering properly reflected in response", False, str(e))

    # Phase 12: Python SDK Sync Client integration
    try:
        from fintext import FinTextClient, SLAStatusResponse

        client = FinTextClient(base_url=BASE_URL, api_token=token)
        sdk_resp = client.sla_status(percentiles="50,95,99", sla_target_ms=100)
        ok = isinstance(sdk_resp, SLAStatusResponse) and sdk_resp.total_requests > 0 and "p50" in sdk_resp.percentiles
        client.close()
        report(12, "Python SDK Sync Client (client.sla_status()) integration verified", ok, f"Total reqs: {sdk_resp.total_requests}")
    except Exception as e:
        report(12, "Python SDK Sync Client (client.sla_status()) integration verified", False, str(e))

    # Phase 13: Python SDK Async Client integration
    try:
        from fintext import FinTextAsyncClient, SLAStatusResponse

        async def check_async_sdk():
            async_client = FinTextAsyncClient(base_url=BASE_URL, api_token=token)
            sdk_resp = await async_client.sla_status(percentiles="50,90,99", sla_target_ms=150)
            await async_client.close()
            return sdk_resp

        sdk_resp = asyncio.run(check_async_sdk())
        ok = isinstance(sdk_resp, SLAStatusResponse) and sdk_resp.sla_target_ms == 150 and "p90" in sdk_resp.percentiles
        report(13, "Python SDK Async Client (await client.sla_status()) integration verified", ok, f"SLA target: {sdk_resp.sla_target_ms}ms")
    except Exception as e:
        report(13, "Python SDK Async Client (await client.sla_status()) integration verified", False, str(e))

    # Phase 14: OpenAPI specification verification
    try:
        r = httpx.get(f"{BASE_URL}/api-docs/openapi.json", timeout=5.0)
        if r.status_code == 200:
            spec = r.json()
            paths = spec.get("paths", {})
            schemas = spec.get("components", {}).get("schemas", {})
            has_path = "/sla/status" in paths
            has_schema = "SLAStatusResponse" in schemas
            report(14, "OpenAPI Spec contains /sla/status path and SLAStatusResponse schema", has_path and has_schema)
        else:
            report(14, "OpenAPI Spec contains /sla/status path and SLAStatusResponse schema", False, f"Status: {r.status_code}")
    except Exception as e:
        report(14, "OpenAPI Spec contains /sla/status path and SLAStatusResponse schema", False, str(e))

    print("-" * 80)
    print(f" Verification Summary: {passed}/{total} Passed | {failed}/{total} Failed")
    print("=" * 80)

    if failed > 0:
        sys.exit(1)


def main():
    with ServerContext():
        run_tests()


if __name__ == "__main__":
    main()
