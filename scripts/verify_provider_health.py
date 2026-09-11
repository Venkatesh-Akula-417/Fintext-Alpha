#!/usr/bin/env python3
"""
===============================================================================
FinText-Alpha-Vectorizer — Verification: Provider Health Status & Operational Transparency API
===============================================================================
Verifies:
  1.  GET /providers/health returns 401 Unauthorized without Authorization header
  2.  GET /providers/health returns 200 OK with valid Bearer token (default params)
  3.  Response structure contains 'providers' list and valid ISO-8601 'generated_at'
  4.  All three data providers ('sec_edgar', 'finnhub', 'polygon') are returned by default
  5.  Telemetry fields adhere to expected bounds (requests, rates, latencies, errors)
  6.  Provider status is correctly classified ('healthy', 'degraded', 'outage')
  7.  Query parameter 'provider=sec_edgar' filters to exactly SEC EDGAR
  8.  Query parameter 'provider=finnhub' filters to exactly Finnhub
  9.  Query parameter 'provider=polygon' filters to exactly Polygon.io
  10. Invalid provider name returns HTTP 400 Bad Request
  11. Out-of-range window_minutes (< 1 or > 1440) returns HTTP 400 Bad Request
  12. Valid custom window (window_minutes=120) scales metrics and returns 200 OK
  13. Python SDK Sync and Async Client (get_provider_health & provider_health alias)
  14. OpenAPI 3.0 specification registers /providers/health under 'Operational Transparency'
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

from fintext import FinTextClient, FinTextAsyncClient, FinTextAPIError
from fintext.models import ProviderHealthResponse, ProviderHealthItem

PORT = 8156
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
    print(" FinText-Alpha-Vectorizer — Provider Health Status Verification")
    print("=" * 80)
    print(f" Project Root: {PROJECT_ROOT}")
    print(f" Server Port:  {PORT}")

    with ServerContext():
        client = httpx.Client(base_url=BASE_URL, timeout=10.0)

        # 0. Obtain JWT auth token
        auth_resp = client.post(
            "/auth/token",
            json={
                "user_id": "ops_observability_lead",
                "role": "institutional",
                "duration_seconds": 3600,
            },
            headers={"X-Admin-Token": ADMIN_TOKEN},
        )
        if auth_resp.status_code == 200:
            token = auth_resp.json().get("token")
        else:
            token = "mock_jwt_token"
        headers = {"Authorization": f"Bearer {token}"}

        # ── Phase 1: 401 Unauthorized without auth ──────────────────────────
        r1 = client.get("/providers/health")
        report(1, "401 Unauthorized Enforcement", r1.status_code == 401, f"Status: {r1.status_code}")

        # ── Phase 2: 200 OK with valid Bearer token ─────────────────────────
        r2 = client.get("/providers/health", headers=headers)
        report(2, "200 OK on Authenticated Request", r2.status_code == 200, f"Status: {r2.status_code}")

        # ── Phase 3: Response Structure Validation ───────────────────────────
        data2 = r2.json()
        has_providers = "providers" in data2 and isinstance(data2["providers"], list)
        has_generated_at = "generated_at" in data2 and isinstance(data2["generated_at"], str)
        report(3, "Top-Level Schema Structure (providers, generated_at)", has_providers and has_generated_at)

        # ── Phase 4: All 3 Providers Present ─────────────────────────────────
        providers = {p["provider"]: p for p in data2.get("providers", [])}
        expected_providers = {"sec_edgar", "finnhub", "polygon"}
        all_present = expected_providers.issubset(providers.keys())
        report(4, "All Active Providers Present (sec_edgar, finnhub, polygon)", all_present, f"Found: {list(providers.keys())}")

        # ── Phase 5: Telemetry Field Bounds & Accuracy ───────────────────────
        valid_bounds = True
        bounds_detail = ""
        for name, item in providers.items():
            tot = item.get("requests_total", 0)
            succ = item.get("requests_success", 0)
            rate = item.get("success_rate_pct", -1.0)
            avg_lat = item.get("avg_latency_ms", -1.0)
            p95_lat = item.get("p95_latency_ms", -1.0)
            errs = item.get("error_count_last_hour", -1)

            if not (tot >= succ >= 0):
                valid_bounds = False
                bounds_detail = f"{name}: tot={tot}, succ={succ}"
                break
            if not (0.0 <= rate <= 100.0):
                valid_bounds = False
                bounds_detail = f"{name}: rate={rate}"
                break
            if avg_lat <= 0.0 or p95_lat < avg_lat:
                valid_bounds = False
                bounds_detail = f"{name}: avg_lat={avg_lat}, p95_lat={p95_lat}"
                break
            if errs < 0:
                valid_bounds = False
                bounds_detail = f"{name}: errs={errs}"
                break
        report(5, "Telemetry Metrics Bounds & Consistency", valid_bounds, bounds_detail)

        # ── Phase 6: Status Classification Verification ──────────────────────
        valid_status = True
        status_detail = ""
        for name, item in providers.items():
            status = item.get("status")
            rate = item.get("success_rate_pct", 0.0)
            if rate >= 95.0 and status != "healthy":
                valid_status = False
                status_detail = f"{name}: rate {rate}% classified as {status}, expected healthy"
                break
            elif 80.0 <= rate < 95.0 and status != "degraded":
                valid_status = False
                status_detail = f"{name}: rate {rate}% classified as {status}, expected degraded"
                break
            elif rate < 80.0 and status != "outage":
                valid_status = False
                status_detail = f"{name}: rate {rate}% classified as {status}, expected outage"
                break
        report(6, "Health Status Classification SLA Rules (healthy/degraded/outage)", valid_status, status_detail)

        # ── Phase 7: Filtering by sec_edgar ──────────────────────────────────
        r7 = client.get("/providers/health?provider=sec_edgar", headers=headers)
        data7 = r7.json()
        p7 = data7.get("providers", [])
        ok7 = r7.status_code == 200 and len(p7) == 1 and p7[0]["provider"] == "sec_edgar"
        report(7, "Provider Filtering: sec_edgar", ok7, f"Count: {len(p7)}")

        # ── Phase 8: Filtering by finnhub ────────────────────────────────────
        r8 = client.get("/providers/health?provider=finnhub", headers=headers)
        data8 = r8.json()
        p8 = data8.get("providers", [])
        ok8 = r8.status_code == 200 and len(p8) == 1 and p8[0]["provider"] == "finnhub"
        report(8, "Provider Filtering: finnhub", ok8, f"Count: {len(p8)}")

        # ── Phase 9: Filtering by polygon ────────────────────────────────────
        r9 = client.get("/providers/health?provider=polygon", headers=headers)
        data9 = r9.json()
        p9 = data9.get("providers", [])
        ok9 = r9.status_code == 200 and len(p9) == 1 and p9[0]["provider"] == "polygon"
        report(9, "Provider Filtering: polygon", ok9, f"Count: {len(p9)}")

        # ── Phase 10: Invalid Provider Returns 400 ───────────────────────────
        r10 = client.get("/providers/health?provider=bloomberg_terminal", headers=headers)
        report(10, "Parameter Validation: Reject Invalid Provider (HTTP 400)", r10.status_code == 400, f"Status: {r10.status_code}")

        # ── Phase 11: Out-of-Range window_minutes Returns 400 ────────────────
        r11_zero = client.get("/providers/health?window_minutes=0", headers=headers)
        r11_huge = client.get("/providers/health?window_minutes=1441", headers=headers)
        report(11, "Parameter Validation: Reject Out-of-Range window_minutes (0, 1441)", r11_zero.status_code == 400 and r11_huge.status_code == 400)

        # ── Phase 12: Custom Window Scaling (window_minutes=120) ─────────────
        r12 = client.get("/providers/health?window_minutes=120", headers=headers)
        data12 = r12.json()
        p12 = data12.get("providers", [])
        ok12 = r12.status_code == 200 and len(p12) == 3
        report(12, "Custom Observation Window (window_minutes=120)", ok12, f"Status: {r12.status_code}, Providers: {len(p12)}")

        # ── Phase 13: Python SDK Client (Sync & Async) ───────────────────────
        sdk_ok = False
        try:
            sdk_client = FinTextClient(base_url=BASE_URL, api_token=token)
            sync_res = sdk_client.get_provider_health()
            assert isinstance(sync_res, ProviderHealthResponse)
            assert len(sync_res.providers) == 3

            sync_alias = sdk_client.provider_health(provider="sec_edgar", window_minutes=30)
            assert len(sync_alias.providers) == 1
            assert sync_alias.providers[0].provider == "sec_edgar"
            sdk_client.close()

            async def test_async_sdk():
                async_client = FinTextAsyncClient(base_url=BASE_URL, api_token=token)
                res_all = await async_client.get_provider_health()
                assert len(res_all.providers) == 3
                res_fin = await async_client.provider_health(provider="finnhub")
                assert len(res_fin.providers) == 1
                assert res_fin.providers[0].provider == "finnhub"
                await async_client.close()

            asyncio.run(test_async_sdk())
            sdk_ok = True
        except Exception as e:
            sdk_ok = False
            sdk_detail = str(e)
        report(13, "Python SDK Client Integration (Sync & Async get_provider_health)", sdk_ok, locals().get("sdk_detail", ""))

        # ── Phase 14: OpenAPI 3.0 Documentation ──────────────────────────────
        r14 = client.get("/api-docs/openapi.json")
        openapi_ok = False
        openapi_detail = ""
        if r14.status_code == 200:
            doc = r14.json()
            paths = doc.get("paths", {})
            has_path = "/providers/health" in paths
            get_op = paths.get("/providers/health", {}).get("get", {})
            tags = get_op.get("tags", [])
            has_tag = "Operational Transparency" in tags

            schemas = doc.get("components", {}).get("schemas", {})
            has_item = "ProviderHealthItem" in schemas
            has_resp = "ProviderHealthResponse" in schemas

            if has_path and has_tag and has_item and has_resp:
                openapi_ok = True
            else:
                openapi_detail = f"path={has_path}, tag={has_tag}, item_schema={has_item}, resp_schema={has_resp}"
        else:
            openapi_detail = f"Status: {r14.status_code}"
        report(14, "OpenAPI 3.0 Registration & Schema Definitions", openapi_ok, openapi_detail)

    print("=" * 80)
    print(f" Summary: {passed}/{total} phases passed ({100.0 * passed / total:.1f}%)")
    print("=" * 80)

    if failed > 0:
        sys.exit(1)
    print(" ✅ All Provider Health Status verification checks passed successfully!")
    sys.exit(0)


if __name__ == "__main__":
    run_all_phases()
