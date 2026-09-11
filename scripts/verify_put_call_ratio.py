#!/usr/bin/env python3
"""
═══════════════════════════════════════════════════════════════════════════════
FinText-Alpha-Vectorizer — Test Suite #198: Options Put/Call Ratio Engine
═══════════════════════════════════════════════════════════════════════════════
Validates:
  1. Unauthenticated & Invalid Token Access Rejection (401).
  2. Input Validation & Bounds Enforcement (dates, ratio_type, granularity).
  3. Single Ticker Daily Volume Put/Call Ratio (ticker=AAPL, daily).
  4. Market-Wide Daily Volume Put/Call Ratio (omitted ticker).
  5. Open Interest Put/Call Ratio (ratio_type=open_interest).
  6. Total Aggregate Ratio for Ticker (granularity=total).
  7. Total Aggregate Market-Wide Ratio (granularity=total).
  8. Point-in-Time (PIT) Delisted Ticker Validation (TWTR rejected).
  9. Rate Limiting & Response Headers.
  10. OpenAPI 3.0 Conformance & Python SDK Sync/Async Execution.
═══════════════════════════════════════════════════════════════════════════════
"""

import asyncio
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
from fintext.models import PutCallRatioResponse, PutCallRatioPoint

SERVER_PORT = 8099
BASE_URL = f"http://127.0.0.1:{SERVER_PORT}"
ADMIN_TOKEN = "fintext-admin-dev-secret-token"
JWT_SECRET = "fintext-alpha-vectorizer-institutional-jwt-secret-key-2026"


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

        exe_path = ROOT_DIR / "rust" / "target" / "release" / "fintext_api.exe"
        if not exe_path.exists():
            exe_path = ROOT_DIR / "rust" / "target" / "debug" / "fintext_api.exe"

        if not exe_path.exists():
            raise RuntimeError(f"Server binary not found at {exe_path}. Build it first.")

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


def run_tests():
    print("=" * 80)
    print("FINTEXT ALPHA VECTORIZER — TEST SUITE #198: OPTIONS PUT/CALL RATIO ENGINE")
    print("=" * 80)

    token = get_token("options_vol_desk_sig")
    auth_headers = {"Authorization": f"Bearer {token}"}

    client = FinTextClient(base_url=BASE_URL, api_token=token)

    with httpx.Client(base_url=BASE_URL, timeout=10.0) as http:
        # ─────────────────────────────────────────────────────────────────────
        # Phase 1: Unauthenticated & Invalid Token Access Rejection
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 1] Testing Unauthenticated & Invalid Token Access Rejection...")
        r = http.get("/options/put-call-ratio?start_date=2025-01-01&end_date=2025-01-10")
        assert r.status_code == 401, f"Expected 401, got {r.status_code}"

        r = http.get(
            "/options/put-call-ratio?start_date=2025-01-01&end_date=2025-01-10",
            headers={"Authorization": "Bearer bad_token_123"},
        )
        assert r.status_code == 401, f"Expected 401, got {r.status_code}"
        print("[PASS] Phase 1 Passed: Unauthorized requests correctly rejected (401).")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 2: Input Validation & Bounds Enforcement
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 2] Testing Input Validation & Bounds Enforcement...")
        # Missing start_date
        r = http.get("/options/put-call-ratio?start_date=&end_date=2025-01-10", headers=auth_headers)
        assert r.status_code == 400, f"Expected 400 for empty start_date, got {r.status_code}"

        # Invalid date format
        r = http.get("/options/put-call-ratio?start_date=invalid-date&end_date=2025-01-10", headers=auth_headers)
        assert r.status_code == 400, f"Expected 400 for bad start_date, got {r.status_code}"

        # start_date > end_date
        r = http.get("/options/put-call-ratio?start_date=2025-05-01&end_date=2025-01-10", headers=auth_headers)
        assert r.status_code == 400, f"Expected 400 for start > end, got {r.status_code}"

        # Date range > 730 days
        r = http.get("/options/put-call-ratio?start_date=2022-01-01&end_date=2025-01-01", headers=auth_headers)
        assert r.status_code == 400, f"Expected 400 for range > 730 days, got {r.status_code}"

        # Invalid ratio_type
        r = http.get("/options/put-call-ratio?start_date=2025-01-01&end_date=2025-01-10&ratio_type=unsupported_type", headers=auth_headers)
        assert r.status_code == 400, f"Expected 400 for invalid ratio_type, got {r.status_code}"

        # Invalid granularity
        r = http.get("/options/put-call-ratio?start_date=2025-01-01&end_date=2025-01-10&granularity=yearly", headers=auth_headers)
        assert r.status_code == 400, f"Expected 400 for invalid granularity, got {r.status_code}"
        print("[PASS] Phase 2 Passed: Parameter validation and bounds strictly enforced (400).")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 3: Single Ticker Daily Volume Put/Call Ratio (ticker=AAPL, daily)
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 3] Testing Single Ticker Daily Volume Put/Call Ratio (AAPL)...")
        r = http.get(
            "/options/put-call-ratio?ticker=AAPL&start_date=2025-01-01&end_date=2025-01-15&ratio_type=volume&granularity=daily",
            headers=auth_headers,
        )
        assert r.status_code == 200, f"Expected 200, got {r.status_code}: {r.text}"
        data = r.json()
        assert data["ticker"] == "AAPL"
        assert data["ratio_type"] == "volume"
        assert data["granularity"] == "daily"
        assert data["points"] is not None
        assert len(data["points"]) > 0
        assert data["average_ratio"] is not None
        avg_r = data["average_ratio"]
        assert 0.0 < avg_r < 5.0, f"Average ratio {avg_r} unexpected"
        for pt in data["points"]:
            assert pt["call_volume"] > 0
            assert pt["put_volume"] > 0
            assert pt["ratio"] is not None
        print(f"[PASS] Phase 3 Passed: Computed AAPL daily Put/Call series with {len(data['points'])} days (Avg Ratio: {avg_r:.4f}).")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 4: Market-Wide Daily Volume Put/Call Ratio (omitted ticker)
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 4] Testing Market-Wide Daily Volume Put/Call Ratio...")
        r_mkt = http.get(
            "/options/put-call-ratio?start_date=2025-01-01&end_date=2025-01-15&ratio_type=volume&granularity=daily",
            headers=auth_headers,
        )
        assert r_mkt.status_code == 200
        data_mkt = r_mkt.json()
        assert data_mkt.get("ticker") is None
        assert data_mkt.get("points") is not None
        assert len(data_mkt["points"]) > 0
        # Market-wide volumes should be institutional scale
        assert data_mkt["points"][0]["call_volume"] > 1_000_000
        print(f"[PASS] Phase 4 Passed: Market-wide aggregate daily Put/Call ratio computed (Day 1 Call Vol: {data_mkt['points'][0]['call_volume']:,}).")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 5: Open Interest Put/Call Ratio (ratio_type=open_interest)
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 5] Testing Open Interest Put/Call Ratio...")
        r_oi = http.get(
            "/options/put-call-ratio?ticker=NVDA&start_date=2025-01-01&end_date=2025-01-15&ratio_type=open_interest&granularity=daily",
            headers=auth_headers,
        )
        assert r_oi.status_code == 200
        data_oi = r_oi.json()
        assert data_oi["ratio_type"] == "open_interest"
        assert data_oi["average_ratio"] is not None
        for pt in data_oi["points"]:
            assert pt["call_open_interest"] is not None
            assert pt["put_open_interest"] is not None
            assert pt["ratio"] is not None
        print(f"[PASS] Phase 5 Passed: NVDA Open Interest Put/Call ratio computed (Avg OI Ratio: {data_oi['average_ratio']:.4f}).")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 6: Total Aggregate Ratio for Ticker (granularity=total)
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 6] Testing Total Aggregate Ratio for Ticker (granularity=total)...")
        r_tot = http.get(
            "/options/put-call-ratio?ticker=MSFT&start_date=2025-01-01&end_date=2025-01-31&granularity=total",
            headers=auth_headers,
        )
        assert r_tot.status_code == 200
        data_tot = r_tot.json()
        assert data_tot["ticker"] == "MSFT"
        assert data_tot["granularity"] == "total"
        assert data_tot.get("points") is None
        assert data_tot["total_call_volume"] is not None
        assert data_tot["total_put_volume"] is not None
        assert data_tot["total_ratio"] is not None
        print(f"[PASS] Phase 6 Passed: MSFT total aggregate (Total Put: {data_tot['total_put_volume']:,}, Total Call: {data_tot['total_call_volume']:,}, Ratio: {data_tot['total_ratio']:.4f}).")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 7: Total Aggregate Market-Wide Ratio
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 7] Testing Total Aggregate Market-Wide Ratio...")
        r_mkt_tot = http.get(
            "/options/put-call-ratio?start_date=2025-01-01&end_date=2025-01-31&granularity=total",
            headers=auth_headers,
        )
        assert r_mkt_tot.status_code == 200
        data_mkt_tot = r_mkt_tot.json()
        assert data_mkt_tot.get("ticker") is None
        assert data_mkt_tot["total_call_volume"] > 10_000_000
        assert data_mkt_tot["total_ratio"] is not None
        print(f"[PASS] Phase 7 Passed: Market-wide total aggregate (Total Call Vol: {data_mkt_tot['total_call_volume']:,}, Ratio: {data_mkt_tot['total_ratio']:.4f}).")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 8: Point-in-Time (PIT) Delisted Ticker Enforcement
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 8] Testing Point-in-Time (PIT) Delisted Ticker Enforcement...")
        r_pit = http.get(
            "/options/put-call-ratio?ticker=TWTR&start_date=2025-01-01&end_date=2025-01-10",
            headers=auth_headers,
        )
        assert r_pit.status_code == 400
        assert "not active or listed" in r_pit.json()["message"]
        print("[PASS] Phase 8 Passed: Delisted ticker TWTR correctly rejected under PIT verification (400).")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 9: Rate Limiting & Response Headers
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 9] Testing Rate Limiting & Response Headers...")
        assert "x-ratelimit-limit" in r.headers
        assert "x-ratelimit-remaining" in r.headers
        assert "x-ratelimit-reset" in r.headers
        print(f"[PASS] Phase 9 Passed: Rate limit headers present (Limit: {r.headers['x-ratelimit-limit']}, Remaining: {r.headers['x-ratelimit-remaining']}).")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 10: OpenAPI 3.0 Conformance & Python SDK Execution
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 10] Testing OpenAPI 3.0 Spec Conformance & Python SDK Sync/Async...")
        r_spec = http.get("/api-docs/openapi.json")
        assert r_spec.status_code == 200
        spec = r_spec.json()
        assert "/options/put-call-ratio" in spec["paths"]
        assert "PutCallRatioResponse" in spec["components"]["schemas"]
        assert "PutCallRatioPoint" in spec["components"]["schemas"]

        # Python SDK Sync
        sdk_sync = client.put_call_ratio(
            ticker="AAPL",
            start_date="2025-01-01",
            end_date="2025-01-15",
            ratio_type="volume",
            granularity="daily",
        )
        assert isinstance(sdk_sync, PutCallRatioResponse)
        assert sdk_sync.ticker == "AAPL"
        assert sdk_sync.points is not None
        print(f"  + Python SDK Sync: Received {len(sdk_sync.points)} daily points (Avg Ratio: {sdk_sync.average_ratio:.4f}).")

        # Python SDK Async
        async def run_async_sdk():
            async_client = FinTextAsyncClient(base_url=BASE_URL, api_token=token)
            async_resp = await async_client.put_call_ratio(
                start_date="2025-01-01",
                end_date="2025-01-15",
                granularity="total",
            )
            assert isinstance(async_resp, PutCallRatioResponse)
            assert async_resp.total_ratio is not None
            await async_client.close()
            print(f"  + Python SDK Async: Total ratio = {async_resp.total_ratio:.4f}")

        asyncio.run(run_async_sdk())
        print("[PASS] Phase 10 Passed: OpenAPI documentation and Python SDK sync/async certified.")

        client.close()

    print("\n" + "=" * 80)
    print("[SUCCESS] ALL 10 PHASES PASSED: OPTIONS PUT/CALL RATIO ENGINE CERTIFIED!")
    print("=" * 80)


def main():
    with ServerContext():
        run_tests()


if __name__ == "__main__":
    main()
