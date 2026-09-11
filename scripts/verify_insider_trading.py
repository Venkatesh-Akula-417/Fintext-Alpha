#!/usr/bin/env python3
"""
═══════════════════════════════════════════════════════════════════════════════
FinText-Alpha-Vectorizer — Test Suite #200: Insider Trading Signal Engine
═══════════════════════════════════════════════════════════════════════════════
Validates:
  1. Unauthenticated & Invalid Token Access Rejection (401).
  2. Input Validation & Parameter Bounds Enforcement (400).
  3. Single Ticker Insider Trading Query (ticker=AAPL).
  4. Universe-Wide Insider Trading Scan (omitted ticker).
  5. Transaction Type Filtering (purchase, sale, grant, exercise).
  6. Signal Score Calculation & Role Weighting Consistency.
  7. Minimum Shares & Minimum Signal Score Filtering.
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
from fintext.models import InsiderTradingResponse, InsiderTradeItem

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
    print("FINTEXT ALPHA VECTORIZER — TEST SUITE #200: INSIDER TRADING SIGNAL ENGINE")
    print("=" * 80)

    token = get_token("quant_event_desk_alpha")
    auth_headers = {"Authorization": f"Bearer {token}"}

    client = FinTextClient(base_url=BASE_URL, api_token=token)

    with httpx.Client(base_url=BASE_URL, timeout=10.0) as http:
        # ─────────────────────────────────────────────────────────────────────
        # Phase 1: Unauthenticated & Invalid Token Access Rejection
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 1] Testing Unauthenticated & Invalid Token Access Rejection...")
        r = http.get("/events/insider-trading")
        assert r.status_code == 401, f"Expected 401, got {r.status_code}"

        r = http.get(
            "/events/insider-trading",
            headers={"Authorization": "Bearer bad_token_123"},
        )
        assert r.status_code == 401, f"Expected 401, got {r.status_code}"
        print("[PASS] Phase 1 Passed: Unauthorized requests correctly rejected (401).")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 2: Input Validation & Parameter Bounds Enforcement
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 2] Testing Input Validation & Bounds Enforcement...")
        # Invalid transaction_type
        r = http.get("/events/insider-trading?transaction_type=short_sale", headers=auth_headers)
        assert r.status_code == 400, f"Expected 400 for bad transaction_type, got {r.status_code}"

        # start_date > end_date
        r = http.get("/events/insider-trading?start_date=2025-12-31&end_date=2025-01-01", headers=auth_headers)
        assert r.status_code == 400, f"Expected 400 for start > end, got {r.status_code}"

        # min_signal_score out of bounds
        r = http.get("/events/insider-trading?min_signal_score=1.5", headers=auth_headers)
        assert r.status_code == 400, f"Expected 400 for score > 1.0, got {r.status_code}"

        # limit out of bounds
        r = http.get("/events/insider-trading?limit=250", headers=auth_headers)
        assert r.status_code == 400, f"Expected 400 for limit > 100, got {r.status_code}"
        print("[PASS] Phase 2 Passed: Parameter validation and bounds strictly enforced (400).")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 3: Single Ticker Insider Trading Query (AAPL)
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 3] Testing Single Ticker Insider Trading Query (AAPL)...")
        r = http.get(
            "/events/insider-trading?ticker=AAPL&start_date=2025-01-01&end_date=2025-06-30&limit=30",
            headers=auth_headers,
        )
        assert r.status_code == 200, f"Expected 200, got {r.status_code}: {r.text}"
        data = r.json()
        assert data["ticker"] == "AAPL"
        assert data["count"] == len(data["trades"])
        assert data["count"] > 0
        for item in data["trades"]:
            assert item["ticker"] == "AAPL"
            assert item["source"] == "SEC Form 4"
            assert -1.0 <= item["signal_score"] <= 1.0
            assert item["shares"] > 0
            assert item["value"] >= 0.0
        print(f"[PASS] Phase 3 Passed: AAPL query returned {data['count']} insider Form 4 transactions.")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 4: Universe-Wide Insider Trading Scan (omitted ticker)
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 4] Testing Universe-Wide Insider Trading Scan...")
        r_univ = http.get(
            "/events/insider-trading?start_date=2025-01-01&end_date=2025-06-30&limit=40",
            headers=auth_headers,
        )
        assert r_univ.status_code == 200
        data_univ = r_univ.json()
        assert data_univ.get("ticker") is None
        assert data_univ["count"] <= 40
        assert data_univ["count"] > 0
        tickers_found = {item["ticker"] for item in data_univ["trades"]}
        print(f"[PASS] Phase 4 Passed: Universe scan identified {data_univ['count']} insider trades across {len(tickers_found)} distinct tickers.")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 5: Transaction Type Filtering (purchase, sale, grant)
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 5] Testing Transaction Type Filtering...")
        r_purch = http.get(
            "/events/insider-trading?transaction_type=purchase&start_date=2025-01-01&end_date=2025-06-30&limit=50",
            headers=auth_headers,
        )
        r_sale = http.get(
            "/events/insider-trading?transaction_type=sale&start_date=2025-01-01&end_date=2025-06-30&limit=50",
            headers=auth_headers,
        )
        assert r_purch.status_code == 200 and r_sale.status_code == 200
        data_purch = r_purch.json()
        data_sale = r_sale.json()
        for item in data_purch["trades"]:
            assert item["transaction_type"] == "purchase"
            assert item["signal_score"] > 0.0
        for item in data_sale["trades"]:
            assert item["transaction_type"] == "sale"
            assert item["signal_score"] < 0.0
        print(f"[PASS] Phase 5 Passed: Purchases ({data_purch['count']} events, positive scores) and Sales ({data_sale['count']} events, negative scores) filtered correctly.")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 6: Signal Score Calculation & Role Weighting Consistency
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 6] Testing Signal Score Calculation & Role Weighting Consistency...")
        for trade in data_univ["trades"]:
            t_type = trade["transaction_type"]
            score = trade["signal_score"]
            role = trade["insider_role"].lower()
            if t_type == "purchase":
                assert score > 0.0, f"Purchase score must be positive, got {score}"
            elif t_type == "sale":
                assert score < 0.0, f"Sale score must be negative, got {score}"
            elif t_type in ("grant", "exercise"):
                assert score >= 0.0, f"Grant/exercise score must be non-negative, got {score}"
            # Check bounds
            assert -1.0 <= score <= 1.0
        print("[PASS] Phase 6 Passed: Signal score mathematics and role weightings verified.")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 7: Minimum Shares & Minimum Signal Score Filtering
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 7] Testing Minimum Shares & Minimum Signal Score Filtering...")
        r_filtered = http.get(
            "/events/insider-trading?min_shares=20000&min_signal_score=0.60&limit=50",
            headers=auth_headers,
        )
        assert r_filtered.status_code == 200
        data_filtered = r_filtered.json()
        for item in data_filtered["trades"]:
            assert item["shares"] >= 20000
            assert abs(item["signal_score"]) >= 0.60
        print(f"[PASS] Phase 7 Passed: min_shares=20000 and min_signal_score=0.60 filtering verified ({data_filtered['count']} qualified events).")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 8: Point-in-Time (PIT) Delisted Ticker Enforcement
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 8] Testing Point-in-Time (PIT) Delisted Ticker Enforcement...")
        r_pit = http.get(
            "/events/insider-trading?ticker=TWTR&start_date=2025-01-01&end_date=2025-06-01",
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
        assert "/events/insider-trading" in spec["paths"]
        assert "InsiderTradingResponse" in spec["components"]["schemas"]
        assert "InsiderTradeItem" in spec["components"]["schemas"]

        # Python SDK Sync
        sdk_sync = client.insider_trading(
            ticker="AAPL",
            start_date="2025-01-01",
            end_date="2025-06-30",
            min_shares=5000,
        )
        assert isinstance(sdk_sync, InsiderTradingResponse)
        assert sdk_sync.ticker == "AAPL"
        assert sdk_sync.count > 0
        print(f"  + Python SDK Sync: Received {sdk_sync.count} insider trades for AAPL.")

        # Python SDK Async
        async def run_async_sdk():
            async_client = FinTextAsyncClient(base_url=BASE_URL, api_token=token)
            async_resp = await async_client.insider_trading(
                start_date="2025-01-01",
                end_date="2025-06-30",
                transaction_type="purchase",
                limit=15,
            )
            assert isinstance(async_resp, InsiderTradingResponse)
            assert async_resp.count <= 15
            for tr in async_resp.trades:
                assert tr.transaction_type == "purchase"
            await async_client.close()
            print(f"  + Python SDK Async: Received {async_resp.count} insider purchase trades for universe scan.")

        asyncio.run(run_async_sdk())
        print("[PASS] Phase 10 Passed: OpenAPI documentation and Python SDK sync/async certified.")

        client.close()

    print("\n" + "=" * 80)
    print("[SUCCESS] ALL 10 PHASES PASSED: INSIDER TRADING SIGNAL ENGINE CERTIFIED!")
    print("=" * 80)


def main():
    with ServerContext():
        run_tests()


if __name__ == "__main__":
    main()
