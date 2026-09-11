#!/usr/bin/env python3
"""
═══════════════════════════════════════════════════════════════════════════════
FinText-Alpha-Vectorizer — Test Suite #197: Return Correlation Matrix Engine
═══════════════════════════════════════════════════════════════════════════════
Validates:
  1. Unauthenticated & Invalid Token Access Rejection (401).
  2. Input Validation & Bounds Enforcement (tickers, dates, min_periods bounds).
  3. Default Correlation Matrix Query (AAPL, MSFT, NVDA, AMZN).
  4. Pairwise Mathematical Bounds & Period Alignment (-1.0 <= r <= 1.0).
  5. Self-Correlation (include_self=true -> r_i,i = 1.0).
  6. High min_periods Threshold & Null Handling.
  7. Point-in-Time (PIT) Ticker Validation.
  8. Rate Limiting & Response Headers.
  9. OpenAPI 3.0 Conformance & Schema Registration.
  10. Python SDK Synchronous & Asynchronous Client Execution.
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
from fintext.models import ReturnCorrelationResponse, ReturnCorrelationItem

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
    print("FINTEXT ALPHA VECTORIZER — TEST SUITE #197: RETURN CORRELATION MATRIX")
    print("=" * 80)

    token = get_token("quant_risk_two_sigma")
    auth_headers = {"Authorization": f"Bearer {token}"}

    client = FinTextClient(base_url=BASE_URL, api_token=token)

    with httpx.Client(base_url=BASE_URL, timeout=10.0) as http:
        # ─────────────────────────────────────────────────────────────────────
        # Phase 1: Unauthenticated & Invalid Token Access Rejection
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 1] Testing Unauthenticated & Invalid Token Access Rejection...")
        r = http.get("/market/correlation?tickers=AAPL,MSFT&start_date=2025-01-01&end_date=2025-03-31")
        assert r.status_code == 401, f"Expected 401, got {r.status_code}"

        r = http.get(
            "/market/correlation?tickers=AAPL,MSFT&start_date=2025-01-01&end_date=2025-03-31",
            headers={"Authorization": "Bearer invalid_token_xyz"},
        )
        assert r.status_code == 401, f"Expected 401, got {r.status_code}"
        print("[PASS] Phase 1 Passed: Unauthorized requests correctly rejected (401).")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 2: Input Validation & Bounds Enforcement
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 2] Testing Input Validation & Bounds Enforcement...")
        # Empty tickers
        r = http.get("/market/correlation?tickers=&start_date=2025-01-01&end_date=2025-03-31", headers=auth_headers)
        assert r.status_code == 400, f"Expected 400, got {r.status_code}"

        # > 50 tickers
        too_many = ",".join([f"TICK{i}" for i in range(55)])
        r = http.get(f"/market/correlation?tickers={too_many}&start_date=2025-01-01&end_date=2025-03-31", headers=auth_headers)
        assert r.status_code == 400, f"Expected 400 for >50 tickers, got {r.status_code}"

        # Invalid date format
        r = http.get("/market/correlation?tickers=AAPL,MSFT&start_date=invalid-date&end_date=2025-03-31", headers=auth_headers)
        assert r.status_code == 400, f"Expected 400 for bad start_date, got {r.status_code}"

        # start_date > end_date
        r = http.get("/market/correlation?tickers=AAPL,MSFT&start_date=2025-05-01&end_date=2025-01-01", headers=auth_headers)
        assert r.status_code == 400, f"Expected 400 for start > end, got {r.status_code}"

        # Date range > 730 days
        r = http.get("/market/correlation?tickers=AAPL,MSFT&start_date=2022-01-01&end_date=2025-01-01", headers=auth_headers)
        assert r.status_code == 400, f"Expected 400 for date range > 730 days, got {r.status_code}"

        # min_periods < 10
        r = http.get("/market/correlation?tickers=AAPL,MSFT&start_date=2025-01-01&end_date=2025-03-31&min_periods=5", headers=auth_headers)
        assert r.status_code == 400, f"Expected 400 for min_periods < 10, got {r.status_code}"
        print("[PASS] Phase 2 Passed: Parameter validation and bounds strictly enforced (400).")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 3: Default Correlation Matrix Query
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 3] Testing Default Correlation Matrix Query (AAPL, MSFT, NVDA, AMZN)...")
        r = http.get(
            "/market/correlation?tickers=AAPL,MSFT,NVDA,AMZN&start_date=2025-01-01&end_date=2025-03-31&min_periods=20",
            headers=auth_headers,
        )
        assert r.status_code == 200, f"Expected 200, got {r.status_code}: {r.text}"
        data = r.json()
        assert data["tickers"] == ["AAPL", "MSFT", "NVDA", "AMZN"]
        assert data["start_date"] == "2025-01-01"
        assert data["end_date"] == "2025-03-31"
        assert data["min_periods"] == 20
        # N=4 => N*(N-1)/2 = 6 pairwise combinations
        assert len(data["matrix"]) == 6
        print(f"[PASS] Phase 3 Passed: Computed {len(data['matrix'])} pairwise correlations for {len(data['tickers'])} tickers.")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 4: Pairwise Mathematical Bounds & Period Alignment
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 4] Testing Pairwise Mathematical Bounds (-1.0 <= r <= 1.0)...")
        for item in data["matrix"]:
            assert item["periods"] >= 20, f"Periods {item['periods']} < min_periods 20"
            assert item["correlation"] is not None
            c = item["correlation"]
            assert -1.0 <= c <= 1.0, f"Correlation {c} out of bounds"
            print(f"  + Pair ({item['ticker_a']}, {item['ticker_b']}): corr = {c:+.4f} (periods: {item['periods']})")
        print("[PASS] Phase 4 Passed: All pairwise correlation coefficients within valid theoretical bounds [-1.0, 1.0].")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 5: Self-Correlation (include_self=true -> r_i,i = 1.0)
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 5] Testing Self-Correlation with include_self=true...")
        r_self = http.get(
            "/market/correlation?tickers=AAPL,MSFT,NVDA&start_date=2025-01-01&end_date=2025-03-31&include_self=true",
            headers=auth_headers,
        )
        assert r_self.status_code == 200
        data_self = r_self.json()
        # N=3 with include_self => 3 self pairs + 3 cross pairs = 6 items
        assert len(data_self["matrix"]) == 6
        for t in ["AAPL", "MSFT", "NVDA"]:
            self_entry = next(i for i in data_self["matrix"] if i["ticker_a"] == t and i["ticker_b"] == t)
            assert self_entry["correlation"] == 1.0, f"Expected 1.0 for self-pair ({t}, {t}), got {self_entry['correlation']}"
            assert self_entry["periods"] >= 20
        print("[PASS] Phase 5 Passed: Self-correlations exactly equal 1.0000 on the diagonal.")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 6: High min_periods Threshold & Null Handling
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 6] Testing High min_periods Threshold & Null Handling...")
        # Queried window has ~89 days; if min_periods=200, correlation should be None
        r_high = http.get(
            "/market/correlation?tickers=AAPL,MSFT&start_date=2025-01-01&end_date=2025-03-31&min_periods=200",
            headers=auth_headers,
        )
        assert r_high.status_code == 200
        data_high = r_high.json()
        assert len(data_high["matrix"]) == 1
        assert data_high["matrix"][0]["correlation"] is None
        assert data_high["matrix"][0]["periods"] < 200
        print(f"[PASS] Phase 6 Passed: High threshold gracefully yields null correlation (periods: {data_high['matrix'][0]['periods']} < 200).")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 7: Point-in-Time (PIT) Ticker Validation
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 7] Testing Point-in-Time (PIT) Ticker Validation...")
        # Delisted ticker TWTR queried after delisting date (2022-10-28)
        r_pit = http.get(
            "/market/correlation?tickers=TWTR,AAPL&start_date=2025-01-01&end_date=2025-03-31",
            headers=auth_headers,
        )
        assert r_pit.status_code == 400
        assert "not active or listed" in r_pit.json()["message"]
        print("[PASS] Phase 7 Passed: Delisted securities rejected under Point-in-Time enforcement (400).")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 8: Rate Limiting & Response Headers
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 8] Testing Rate Limiting & Response Headers...")
        assert "x-ratelimit-limit" in r.headers
        assert "x-ratelimit-remaining" in r.headers
        assert "x-ratelimit-reset" in r.headers
        print(f"[PASS] Phase 8 Passed: Rate limit headers verified (Limit: {r.headers['x-ratelimit-limit']}, Remaining: {r.headers['x-ratelimit-remaining']}).")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 9: OpenAPI 3.0 Conformance
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 9] Testing OpenAPI 3.0 Spec Conformance...")
        r_spec = http.get("/api-docs/openapi.json")
        assert r_spec.status_code == 200
        spec = r_spec.json()
        assert "/market/correlation" in spec["paths"]
        assert "ReturnCorrelationResponse" in spec["components"]["schemas"]
        assert "ReturnCorrelationItem" in spec["components"]["schemas"]
        print("[PASS] Phase 9 Passed: OpenAPI 3.0 documentation registered and valid.")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 10: Python SDK Synchronous & Asynchronous Client Execution
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 10] Testing Python SDK Sync & Async Client Execution...")
        sdk_resp = client.return_correlation(
            tickers=["AAPL", "MSFT", "NVDA"],
            start_date="2025-01-01",
            end_date="2025-03-31",
            min_periods=20,
            include_self=True,
        )
        assert isinstance(sdk_resp, ReturnCorrelationResponse)
        assert len(sdk_resp.matrix) == 6
        print(f"  + Python SDK Sync: Received {len(sdk_resp.matrix)} correlation matrix items successfully.")

        async def run_async_sdk():
            async_client = FinTextAsyncClient(base_url=BASE_URL, api_token=token)
            async_resp = await async_client.return_correlation(
                tickers="AAPL,MSFT,NVDA",
                start_date="2025-01-01",
                end_date="2025-03-31",
                min_periods=20,
            )
            assert isinstance(async_resp, ReturnCorrelationResponse)
            assert len(async_resp.matrix) == 3
            await async_client.close()
            print(f"  + Python SDK Async: Received {len(async_resp.matrix)} correlation items.")

        asyncio.run(run_async_sdk())
        print("[PASS] Phase 10 Passed: Python SDK Synchronous and Asynchronous client executions certified.")

        client.close()

    print("\n" + "=" * 80)
    print("[SUCCESS] ALL 10 PHASES PASSED: RETURN CORRELATION MATRIX ENGINE CERTIFIED!")
    print("=" * 80)


def main():
    with ServerContext():
        run_tests()


if __name__ == "__main__":
    main()
