#!/usr/bin/env python3
"""
═══════════════════════════════════════════════════════════════════════════════
FinText-Alpha-Vectorizer — Test Suite #203: M&A Rumor Detection Engine
═══════════════════════════════════════════════════════════════════════════════
Validates:
  1. Unauthenticated & Invalid Token Access Rejection (401).
  2. Input Validation & Parameter Bounds Enforcement (400).
  3. Single Ticker M&A Rumor Signal Query (NVDA, threshold 0.40).
  4. Universe-Wide M&A Rumor Detection Scan (all tracked tickers).
  5. High-Conviction Rumor Score Threshold Filtering (min_rumor_score=0.75).
  6. Lookback Window Parameterization (lookback_days=14).
  7. Result Pagination / Limit Slicing (limit=3).
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
from fintext.models import MARumorsResponse, MARumorItem

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
    print("FINTEXT ALPHA VECTORIZER — TEST SUITE #203: M&A RUMOR DETECTION ENGINE")
    print("=" * 80)

    token = get_token("quant_event_arbitrage_desk")
    auth_headers = {"Authorization": f"Bearer {token}"}

    client = FinTextClient(base_url=BASE_URL, api_token=token)

    with httpx.Client(base_url=BASE_URL, timeout=10.0) as http:
        # ─────────────────────────────────────────────────────────────────────
        # Phase 1: Unauthenticated & Invalid Token Access Rejection
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 1] Testing Unauthenticated & Invalid Token Access Rejection...")
        r = http.get("/events/ma-rumors?ticker=NVDA")
        assert r.status_code == 401, f"Expected 401, got {r.status_code}"

        r = http.get(
            "/events/ma-rumors?ticker=NVDA",
            headers={"Authorization": "Bearer bad_token_123"},
        )
        assert r.status_code == 401, f"Expected 401, got {r.status_code}"
        print("[PASS] Phase 1 Passed: Unauthorized requests correctly rejected (401).")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 2: Input Validation & Parameter Bounds Enforcement
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 2] Testing Input Validation & Bounds Enforcement...")
        # min_rumor_score > 1.0
        r = http.get("/events/ma-rumors?min_rumor_score=1.5", headers=auth_headers)
        assert r.status_code == 400, f"Expected 400 for min_rumor_score > 1.0, got {r.status_code}"

        # min_rumor_score < 0.0
        r = http.get("/events/ma-rumors?min_rumor_score=-0.1", headers=auth_headers)
        assert r.status_code == 400, f"Expected 400 for min_rumor_score < 0.0, got {r.status_code}"

        # lookback_days > 30
        r = http.get("/events/ma-rumors?lookback_days=45", headers=auth_headers)
        assert r.status_code == 400, f"Expected 400 for lookback_days > 30, got {r.status_code}"

        # limit > 100
        r = http.get("/events/ma-rumors?limit=150", headers=auth_headers)
        assert r.status_code == 400, f"Expected 400 for limit > 100, got {r.status_code}"
        print("[PASS] Phase 2 Passed: Parameter validation and bounds strictly enforced (400).")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 3: Single Ticker M&A Rumor Signal Query (NVDA)
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 3] Testing Single Ticker M&A Rumor Query (NVDA)...")
        r = http.get("/events/ma-rumors?ticker=NVDA&min_rumor_score=0.4", headers=auth_headers)
        assert r.status_code == 200, f"Expected 200, got {r.status_code}: {r.text}"
        data = r.json()
        assert data["ticker"] == "NVDA"
        assert len(data["items"]) == 1
        item = data["items"][0]
        assert item["ticker"] == "NVDA"
        assert item["rumor_score"] >= 0.40
        assert item["sentiment_zscore"] > 0.0
        assert len(item["supply_chain_related_tickers"]) > 0
        print(f"[PASS] Phase 3 Passed: NVDA Rumor Score: {item['rumor_score']:.4f}, z-score={item['sentiment_zscore']}, keywords={item['keyword_hits']}, SC={item['supply_chain_related_tickers']}.")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 4: Universe-Wide M&A Rumor Detection Scan
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 4] Testing Universe-Wide M&A Rumor Detection Scan...")
        r_uni = http.get("/events/ma-rumors?min_rumor_score=0.30", headers=auth_headers)
        assert r_uni.status_code == 200
        data_uni = r_uni.json()
        assert data_uni.get("ticker") is None
        assert data_uni["count"] > 1
        for it in data_uni["items"]:
            assert it["rumor_score"] >= 0.30
            assert len(it["supply_chain_related_tickers"]) > 0
        # Verify descending sort
        scores = [it["rumor_score"] for it in data_uni["items"]]
        assert scores == sorted(scores, reverse=True), "Expected items sorted descending by rumor_score"
        print(f"[PASS] Phase 4 Passed: Tracked universe scan flagged {data_uni['count']} M&A candidate tickers (top score: {scores[0]:.4f}).")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 5: High-Conviction Rumor Score Threshold Filtering
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 5] Testing High-Conviction Rumor Score Threshold (0.75)...")
        r_high = http.get("/events/ma-rumors?min_rumor_score=0.75", headers=auth_headers)
        assert r_high.status_code == 200
        data_high = r_high.json()
        for it in data_high["items"]:
            assert it["rumor_score"] >= 0.75
        print(f"[PASS] Phase 5 Passed: High-conviction filter isolated {data_high['count']} candidates with score >= 0.75.")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 6: Lookback Window Parameterization
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 6] Testing Lookback Window Parameterization (14 days)...")
        r_lookback = http.get("/events/ma-rumors?lookback_days=14&min_rumor_score=0.40", headers=auth_headers)
        assert r_lookback.status_code == 200
        data_lookback = r_lookback.json()
        assert data_lookback["lookback_days"] == 14
        print(f"[PASS] Phase 6 Passed: Lookback window set to {data_lookback['lookback_days']} days.")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 7: Result Pagination / Limit Slicing
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 7] Testing Limit Slicing (limit=3)...")
        r_lim = http.get("/events/ma-rumors?limit=3&min_rumor_score=0.20", headers=auth_headers)
        assert r_lim.status_code == 200
        data_lim = r_lim.json()
        assert len(data_lim["items"]) <= 3
        print(f"[PASS] Phase 7 Passed: Slicing returned exactly {len(data_lim['items'])} items (limit=3).")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 8: Point-in-Time (PIT) Delisted Ticker Enforcement
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 8] Testing Point-in-Time (PIT) Delisted Ticker Enforcement...")
        r_pit = http.get(
            "/events/ma-rumors?ticker=TWTR",
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
        print(f"[PASS] Phase 9 Passed: Rate limit headers verified (Limit: {r.headers['x-ratelimit-limit']}, Remaining: {r.headers['x-ratelimit-remaining']}).")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 10: OpenAPI 3.0 Conformance & Python SDK Execution
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 10] Testing OpenAPI 3.0 Spec Conformance & Python SDK Sync/Async...")
        r_spec = http.get("/api-docs/openapi.json")
        assert r_spec.status_code == 200
        spec = r_spec.json()
        assert "/events/ma-rumors" in spec["paths"]
        assert "MARumorsResponse" in spec["components"]["schemas"]
        assert "MARumorItem" in spec["components"]["schemas"]

        # Python SDK Sync
        sdk_sync = client.ma_rumors(ticker="PYPL", min_rumor_score=0.60)
        assert isinstance(sdk_sync, MARumorsResponse)
        assert sdk_sync.ticker == "PYPL"
        assert len(sdk_sync.items) >= 1
        print(f"  + Python SDK Sync: Received PYPL rumor score={sdk_sync.items[0].rumor_score:.4f}, title='{sdk_sync.items[0].latest_news_title[:45]}...'.")

        # Python SDK Async
        async def run_async_sdk():
            async_client = FinTextAsyncClient(base_url=BASE_URL, api_token=token)
            async_resp = await async_client.ma_rumors(min_rumor_score=0.40, limit=5)
            assert isinstance(async_resp, MARumorsResponse)
            assert async_resp.count <= 5
            await async_client.close()
            print(f"  + Python SDK Async: Received universe scan with {async_resp.count} candidates (top ticker: {async_resp.items[0].ticker}).")

        asyncio.run(run_async_sdk())
        print("[PASS] Phase 10 Passed: OpenAPI documentation and Python SDK sync/async certified.")

        client.close()

    print("\n" + "=" * 80)
    print("[SUCCESS] ALL 10 PHASES PASSED: M&A RUMOR DETECTION ENGINE CERTIFIED!")
    print("=" * 80)


def main():
    with ServerContext():
        run_tests()


if __name__ == "__main__":
    main()
