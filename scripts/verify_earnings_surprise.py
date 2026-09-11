#!/usr/bin/env python3
"""
═══════════════════════════════════════════════════════════════════════════════
FinText-Alpha-Vectorizer — Test Suite #199: Earnings Surprise Tracker Engine
═══════════════════════════════════════════════════════════════════════════════
Validates:
  1. Unauthenticated & Invalid Token Access Rejection (401).
  2. Input Validation & Parameter Bounds Enforcement.
  3. Single Ticker Earnings Surprise Query (ticker=AAPL).
  4. Universe-Wide Earnings Surprise Scan (omitted ticker).
  5. Threshold Sensitivity & Filtering (min_sentiment_shift).
  6. Direction Classification & Math Consistency.
  7. Custom Pre/Post Window Verification (pre_days, post_days).
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
from fintext.models import EarningsSurpriseResponse, EarningsSurpriseItem

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
    print("FINTEXT ALPHA VECTORIZER — TEST SUITE #199: EARNINGS SURPRISE TRACKER ENGINE")
    print("=" * 80)

    token = get_token("quant_event_desk_alpha")
    auth_headers = {"Authorization": f"Bearer {token}"}

    client = FinTextClient(base_url=BASE_URL, api_token=token)

    with httpx.Client(base_url=BASE_URL, timeout=10.0) as http:
        # ─────────────────────────────────────────────────────────────────────
        # Phase 1: Unauthenticated & Invalid Token Access Rejection
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 1] Testing Unauthenticated & Invalid Token Access Rejection...")
        r = http.get("/events/earnings-surprise")
        assert r.status_code == 401, f"Expected 401, got {r.status_code}"

        r = http.get(
            "/events/earnings-surprise",
            headers={"Authorization": "Bearer bad_token_123"},
        )
        assert r.status_code == 401, f"Expected 401, got {r.status_code}"
        print("[PASS] Phase 1 Passed: Unauthorized requests correctly rejected (401).")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 2: Input Validation & Parameter Bounds Enforcement
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 2] Testing Input Validation & Bounds Enforcement...")
        # Invalid date format
        r = http.get("/events/earnings-surprise?start_date=invalid-date", headers=auth_headers)
        assert r.status_code == 400, f"Expected 400 for bad start_date, got {r.status_code}"

        # start_date > end_date
        r = http.get("/events/earnings-surprise?start_date=2025-12-31&end_date=2025-01-01", headers=auth_headers)
        assert r.status_code == 400, f"Expected 400 for start > end, got {r.status_code}"

        # min_sentiment_shift out of bounds
        r = http.get("/events/earnings-surprise?min_sentiment_shift=0.01", headers=auth_headers)
        assert r.status_code == 400, f"Expected 400 for shift < 0.05, got {r.status_code}"

        r = http.get("/events/earnings-surprise?min_sentiment_shift=0.80", headers=auth_headers)
        assert r.status_code == 400, f"Expected 400 for shift > 0.50, got {r.status_code}"

        # pre_days out of bounds
        r = http.get("/events/earnings-surprise?pre_days=0", headers=auth_headers)
        assert r.status_code == 400, f"Expected 400 for pre_days == 0, got {r.status_code}"

        r = http.get("/events/earnings-surprise?pre_days=15", headers=auth_headers)
        assert r.status_code == 400, f"Expected 400 for pre_days > 10, got {r.status_code}"

        # post_days out of bounds
        r = http.get("/events/earnings-surprise?post_days=20", headers=auth_headers)
        assert r.status_code == 400, f"Expected 400 for post_days > 10, got {r.status_code}"

        # limit out of bounds
        r = http.get("/events/earnings-surprise?limit=200", headers=auth_headers)
        assert r.status_code == 400, f"Expected 400 for limit > 100, got {r.status_code}"
        print("[PASS] Phase 2 Passed: Parameter validation and bounds strictly enforced (400).")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 3: Single Ticker Earnings Surprise Query (AAPL)
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 3] Testing Single Ticker Earnings Surprise Query (AAPL)...")
        r = http.get(
            "/events/earnings-surprise?ticker=AAPL&start_date=2025-01-01&end_date=2025-12-31&min_sentiment_shift=0.10",
            headers=auth_headers,
        )
        assert r.status_code == 200, f"Expected 200, got {r.status_code}: {r.text}"
        data = r.json()
        assert data["ticker"] == "AAPL"
        assert data["start_date"] == "2025-01-01"
        assert data["end_date"] == "2025-12-31"
        assert data["count"] == len(data["surprises"])
        for item in data["surprises"]:
            assert item["ticker"] == "AAPL"
            assert item["pre_record_count"] >= 3
            assert item["post_record_count"] >= 3
            assert abs(item["surprise_score"]) >= 0.10
        print(f"[PASS] Phase 3 Passed: AAPL query returned {data['count']} earnings surprise events.")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 4: Universe-Wide Earnings Surprise Scan (omitted ticker)
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 4] Testing Universe-Wide Earnings Surprise Scan...")
        r_univ = http.get(
            "/events/earnings-surprise?start_date=2025-01-01&end_date=2025-12-31&min_sentiment_shift=0.15&limit=25",
            headers=auth_headers,
        )
        assert r_univ.status_code == 200
        data_univ = r_univ.json()
        assert data_univ.get("ticker") is None
        assert data_univ["count"] <= 25
        assert data_univ["count"] > 0
        tickers_found = {item["ticker"] for item in data_univ["surprises"]}
        print(f"[PASS] Phase 4 Passed: Universe scan identified {data_univ['count']} surprises across {len(tickers_found)} distinct tickers.")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 5: Threshold Sensitivity & Filtering
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 5] Testing Threshold Sensitivity & Filtering...")
        r_low = http.get(
            "/events/earnings-surprise?start_date=2025-01-01&end_date=2025-12-31&min_sentiment_shift=0.08&limit=100",
            headers=auth_headers,
        )
        r_high = http.get(
            "/events/earnings-surprise?start_date=2025-01-01&end_date=2025-12-31&min_sentiment_shift=0.35&limit=100",
            headers=auth_headers,
        )
        assert r_low.status_code == 200 and r_high.status_code == 200
        count_low = r_low.json()["count"]
        count_high = r_high.json()["count"]
        assert count_low >= count_high, f"Expected count_low ({count_low}) >= count_high ({count_high})"
        print(f"[PASS] Phase 5 Passed: Threshold sensitivity confirmed (Shift 0.08: {count_low} events vs Shift 0.35: {count_high} events).")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 6: Direction Classification & Math Consistency
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 6] Testing Direction Classification & Math Consistency...")
        for item in data_univ["surprises"]:
            pre_avg = item["pre_avg_sentiment"]
            post_avg = item["post_avg_sentiment"]
            expected_score = round(post_avg - pre_avg, 4)
            assert abs(item["surprise_score"] - expected_score) < 1e-3, f"Score mismatch: {item['surprise_score']} vs {expected_score}"
            if item["surprise_score"] >= 0.0:
                assert item["direction"] == "positive"
            else:
                assert item["direction"] == "negative"
        print("[PASS] Phase 6 Passed: Surprise scores and direction classifications are mathematically consistent.")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 7: Custom Pre/Post Window Verification (pre_days=3, post_days=3)
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 7] Testing Custom Pre/Post Window Verification...")
        r_win = http.get(
            "/events/earnings-surprise?ticker=NVDA&start_date=2025-01-01&end_date=2025-12-31&pre_days=3&post_days=3&min_sentiment_shift=0.10",
            headers=auth_headers,
        )
        assert r_win.status_code == 200
        data_win = r_win.json()
        assert data_win["pre_days"] == 3
        assert data_win["post_days"] == 3
        print(f"[PASS] Phase 7 Passed: Custom pre_days=3, post_days=3 executed correctly.")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 8: Point-in-Time (PIT) Delisted Ticker Enforcement
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 8] Testing Point-in-Time (PIT) Delisted Ticker Enforcement...")
        r_pit = http.get(
            "/events/earnings-surprise?ticker=TWTR&start_date=2025-01-01&end_date=2025-06-01",
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
        assert "/events/earnings-surprise" in spec["paths"]
        assert "EarningsSurpriseResponse" in spec["components"]["schemas"]
        assert "EarningsSurpriseItem" in spec["components"]["schemas"]

        # Python SDK Sync
        sdk_sync = client.earnings_surprise(
            ticker="AAPL",
            start_date="2025-01-01",
            end_date="2025-12-31",
            min_sentiment_shift=0.10,
        )
        assert isinstance(sdk_sync, EarningsSurpriseResponse)
        assert sdk_sync.ticker == "AAPL"
        assert sdk_sync.count > 0
        print(f"  + Python SDK Sync: Received {sdk_sync.count} surprises for AAPL.")

        # Python SDK Async
        async def run_async_sdk():
            async_client = FinTextAsyncClient(base_url=BASE_URL, api_token=token)
            async_resp = await async_client.earnings_surprise(
                start_date="2025-01-01",
                end_date="2025-12-31",
                min_sentiment_shift=0.15,
                limit=15,
            )
            assert isinstance(async_resp, EarningsSurpriseResponse)
            assert async_resp.count <= 15
            await async_client.close()
            print(f"  + Python SDK Async: Received {async_resp.count} surprises for universe scan.")

        asyncio.run(run_async_sdk())
        print("[PASS] Phase 10 Passed: OpenAPI documentation and Python SDK sync/async certified.")

        client.close()

    print("\n" + "=" * 80)
    print("[SUCCESS] ALL 10 PHASES PASSED: EARNINGS SURPRISE TRACKER ENGINE CERTIFIED!")
    print("=" * 80)


def main():
    with ServerContext():
        run_tests()


if __name__ == "__main__":
    main()
