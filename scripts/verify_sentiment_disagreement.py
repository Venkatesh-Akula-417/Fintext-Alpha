#!/usr/bin/env python3
"""
═══════════════════════════════════════════════════════════════════════════════
FinText-Alpha-Vectorizer — Test Suite #201: Sentiment Disagreement Index Engine
═══════════════════════════════════════════════════════════════════════════════
Validates:
  1. Unauthenticated & Invalid Token Access Rejection (401).
  2. Input Validation & Parameter Bounds Enforcement (400).
  3. Single Ticker Query with Default StdDev Aggregation (ticker=AAPL).
  4. Interquartile Range (IQR) Aggregation (aggregation=iqr).
  5. Median Absolute Deviation (MAD) Aggregation (aggregation=mad).
  6. News & Filing Source Filtering (source=Finnhub).
  7. Minimum Records Threshold Enforcement (insufficient records -> 400).
  8. Point-in-Time (PIT) Delisted Ticker Enforcement (TWTR rejected).
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
from fintext.models import SentimentDisagreementResponse, SourceBreakdown

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
    print("FINTEXT ALPHA VECTORIZER — TEST SUITE #201: SENTIMENT DISAGREEMENT INDEX")
    print("=" * 80)

    token = get_token("quant_macro_dispersion_desk")
    auth_headers = {"Authorization": f"Bearer {token}"}

    client = FinTextClient(base_url=BASE_URL, api_token=token)

    with httpx.Client(base_url=BASE_URL, timeout=10.0) as http:
        # ─────────────────────────────────────────────────────────────────────
        # Phase 1: Unauthenticated & Invalid Token Access Rejection
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 1] Testing Unauthenticated & Invalid Token Access Rejection...")
        r = http.get("/sentiment/disagreement?ticker=AAPL&start_date=2025-01-01&end_date=2025-03-31")
        assert r.status_code == 401, f"Expected 401, got {r.status_code}"

        r = http.get(
            "/sentiment/disagreement?ticker=AAPL&start_date=2025-01-01&end_date=2025-03-31",
            headers={"Authorization": "Bearer bad_token_123"},
        )
        assert r.status_code == 401, f"Expected 401, got {r.status_code}"
        print("[PASS] Phase 1 Passed: Unauthorized requests correctly rejected (401).")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 2: Input Validation & Parameter Bounds Enforcement
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 2] Testing Input Validation & Bounds Enforcement...")
        # Missing ticker
        r = http.get("/sentiment/disagreement?start_date=2025-01-01&end_date=2025-03-31", headers=auth_headers)
        assert r.status_code == 400 or r.status_code == 422, f"Expected 400/422 for missing ticker, got {r.status_code}"

        # Invalid start > end
        r = http.get("/sentiment/disagreement?ticker=AAPL&start_date=2025-12-31&end_date=2025-01-01", headers=auth_headers)
        assert r.status_code == 400, f"Expected 400 for start > end, got {r.status_code}"

        # min_records < 5
        r = http.get("/sentiment/disagreement?ticker=AAPL&start_date=2025-01-01&end_date=2025-03-31&min_records=2", headers=auth_headers)
        assert r.status_code == 400, f"Expected 400 for min_records < 5, got {r.status_code}"

        # Invalid aggregation
        r = http.get("/sentiment/disagreement?ticker=AAPL&start_date=2025-01-01&end_date=2025-03-31&aggregation=variance", headers=auth_headers)
        assert r.status_code == 400, f"Expected 400 for invalid aggregation, got {r.status_code}"
        print("[PASS] Phase 2 Passed: Parameter validation and bounds strictly enforced (400).")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 3: Single Ticker Query with Default StdDev Aggregation (AAPL)
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 3] Testing Single Ticker StdDev Disagreement Query (AAPL)...")
        r = http.get(
            "/sentiment/disagreement?ticker=AAPL&start_date=2025-01-01&end_date=2025-03-31&aggregation=stddev",
            headers=auth_headers,
        )
        assert r.status_code == 200, f"Expected 200, got {r.status_code}: {r.text}"
        data = r.json()
        assert data["ticker"] == "AAPL"
        assert data["aggregation"] == "stddev"
        assert data["start_date"] == "2025-01-01"
        assert data["end_date"] == "2025-03-31"
        assert data["disagreement_index"] >= 0.0
        assert data["record_count"] >= 10
        assert data["source_count"] > 1
        assert len(data["sources_breakdown"]) == data["source_count"]
        for sb in data["sources_breakdown"]:
            assert sb["count"] > 0
            assert -1.0 <= sb["mean"] <= 1.0
        print(f"[PASS] Phase 3 Passed: AAPL StdDev Disagreement Index = {data['disagreement_index']} (Mean: {data['mean_sentiment']}, Median: {data['median_sentiment']}, Sources: {data['source_count']}, Records: {data['record_count']}).")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 4: Interquartile Range (IQR) Aggregation (NVDA)
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 4] Testing Interquartile Range (IQR) Aggregation (NVDA)...")
        r_iqr = http.get(
            "/sentiment/disagreement?ticker=NVDA&start_date=2025-01-01&end_date=2025-03-31&aggregation=iqr",
            headers=auth_headers,
        )
        assert r_iqr.status_code == 200
        data_iqr = r_iqr.json()
        assert data_iqr["ticker"] == "NVDA"
        assert data_iqr["aggregation"] == "iqr"
        assert data_iqr["disagreement_index"] >= 0.0
        print(f"[PASS] Phase 4 Passed: NVDA IQR Disagreement Index = {data_iqr['disagreement_index']}.")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 5: Median Absolute Deviation (MAD) Aggregation (MSFT)
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 5] Testing Median Absolute Deviation (MAD) Aggregation (MSFT)...")
        r_mad = http.get(
            "/sentiment/disagreement?ticker=MSFT&start_date=2025-01-01&end_date=2025-03-31&aggregation=mad",
            headers=auth_headers,
        )
        assert r_mad.status_code == 200
        data_mad = r_mad.json()
        assert data_mad["ticker"] == "MSFT"
        assert data_mad["aggregation"] == "mad"
        assert data_mad["disagreement_index"] >= 0.0
        print(f"[PASS] Phase 5 Passed: MSFT MAD Disagreement Index = {data_mad['disagreement_index']}.")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 6: News & Filing Source Filtering
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 6] Testing Source Filtering...")
        r_src = http.get(
            "/sentiment/disagreement?ticker=AAPL&start_date=2025-01-01&end_date=2025-03-31&source=Finnhub&min_records=5",
            headers=auth_headers,
        )
        assert r_src.status_code == 200
        data_src = r_src.json()
        assert data_src["source_count"] == 1
        assert "finnhub" in data_src["sources_breakdown"][0]["source"].lower()
        print(f"[PASS] Phase 6 Passed: Source filter isolated single provider ({data_src['sources_breakdown'][0]['source']} with {data_src['record_count']} records).")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 7: Minimum Records Threshold Enforcement
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 7] Testing Minimum Records Threshold Enforcement...")
        r_insufficient = http.get(
            "/sentiment/disagreement?ticker=AAPL&start_date=2025-01-01&end_date=2025-01-03&min_records=500",
            headers=auth_headers,
        )
        assert r_insufficient.status_code == 400
        assert "Insufficient sentiment records" in r_insufficient.json()["message"]
        print("[PASS] Phase 7 Passed: Insufficient records threshold correctly rejected (400).")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 8: Point-in-Time (PIT) Delisted Ticker Enforcement
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 8] Testing Point-in-Time (PIT) Delisted Ticker Enforcement...")
        r_pit = http.get(
            "/sentiment/disagreement?ticker=TWTR&start_date=2025-01-01&end_date=2025-06-01",
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
        assert "/sentiment/disagreement" in spec["paths"]
        assert "SentimentDisagreementResponse" in spec["components"]["schemas"]
        assert "SourceBreakdown" in spec["components"]["schemas"]

        # Python SDK Sync
        sdk_sync = client.sentiment_disagreement(
            ticker="AAPL",
            start_date="2025-01-01",
            end_date="2025-03-31",
            aggregation="stddev",
        )
        assert isinstance(sdk_sync, SentimentDisagreementResponse)
        assert sdk_sync.ticker == "AAPL"
        assert sdk_sync.disagreement_index >= 0.0
        print(f"  + Python SDK Sync: Received AAPL disagreement index = {sdk_sync.disagreement_index} across {sdk_sync.source_count} sources.")

        # Python SDK Async
        async def run_async_sdk():
            async_client = FinTextAsyncClient(base_url=BASE_URL, api_token=token)
            async_resp = await async_client.sentiment_disagreement(
                ticker="NVDA",
                start_date="2025-01-01",
                end_date="2025-03-31",
                aggregation="iqr",
            )
            assert isinstance(async_resp, SentimentDisagreementResponse)
            assert async_resp.ticker == "NVDA"
            assert async_resp.aggregation == "iqr"
            await async_client.close()
            print(f"  + Python SDK Async: Received NVDA IQR disagreement index = {async_resp.disagreement_index}.")

        asyncio.run(run_async_sdk())
        print("[PASS] Phase 10 Passed: OpenAPI documentation and Python SDK sync/async certified.")

        client.close()

    print("\n" + "=" * 80)
    print("[SUCCESS] ALL 10 PHASES PASSED: SENTIMENT DISAGREEMENT INDEX ENGINE CERTIFIED!")
    print("=" * 80)


def main():
    with ServerContext():
        run_tests()


if __name__ == "__main__":
    main()
