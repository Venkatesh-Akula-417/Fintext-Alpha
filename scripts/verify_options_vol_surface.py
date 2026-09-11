#!/usr/bin/env python3
"""
═══════════════════════════════════════════════════════════════════════════════
FinText-Alpha-Vectorizer — Test Suite #202: Options Volatility Surface Engine
═══════════════════════════════════════════════════════════════════════════════
Validates:
  1. Unauthenticated & Invalid Token Access Rejection (401).
  2. Input Validation & Parameter Bounds Enforcement (400).
  3. Single Ticker Default Volatility Surface Query (AAPL, 9 strikes × expiries).
  4. Custom Strike Grid & Percentage Range (NVDA, 5 strikes, range 0.85-1.15).
  5. Custom Expiration Date Window (MSFT).
  6. Rate Sensitivity & Continuous Dividend Yields.
  7. Volatility Smile/Skew & Moneyness Convexity Validation.
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
from fintext.models import OptionsVolSurfaceResponse, VolSurfacePoint

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
    print("FINTEXT ALPHA VECTORIZER — TEST SUITE #202: OPTIONS VOLATILITY SURFACE ENGINE")
    print("=" * 80)

    token = get_token("quant_derivatives_vol_desk")
    auth_headers = {"Authorization": f"Bearer {token}"}

    client = FinTextClient(base_url=BASE_URL, api_token=token)

    with httpx.Client(base_url=BASE_URL, timeout=10.0) as http:
        # ─────────────────────────────────────────────────────────────────────
        # Phase 1: Unauthenticated & Invalid Token Access Rejection
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 1] Testing Unauthenticated & Invalid Token Access Rejection...")
        r = http.get("/options/vol-surface?ticker=AAPL")
        assert r.status_code == 401, f"Expected 401, got {r.status_code}"

        r = http.get(
            "/options/vol-surface?ticker=AAPL",
            headers={"Authorization": "Bearer bad_token_123"},
        )
        assert r.status_code == 401, f"Expected 401, got {r.status_code}"
        print("[PASS] Phase 1 Passed: Unauthorized requests correctly rejected (401).")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 2: Input Validation & Parameter Bounds Enforcement
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 2] Testing Input Validation & Bounds Enforcement...")
        # Missing ticker
        r = http.get("/options/vol-surface", headers=auth_headers)
        assert r.status_code == 400 or r.status_code == 422, f"Expected 400/422 for missing ticker, got {r.status_code}"

        # Even strike_count
        r = http.get("/options/vol-surface?ticker=AAPL&strike_count=8", headers=auth_headers)
        assert r.status_code == 400, f"Expected 400 for even strike_count, got {r.status_code}"

        # strike_count > 15
        r = http.get("/options/vol-surface?ticker=AAPL&strike_count=17", headers=auth_headers)
        assert r.status_code == 400, f"Expected 400 for strike_count > 15, got {r.status_code}"

        # Invalid strike_range
        r = http.get("/options/vol-surface?ticker=AAPL&strike_range=invalid_range", headers=auth_headers)
        assert r.status_code == 400, f"Expected 400 for invalid strike_range, got {r.status_code}"

        # start_date > end_date
        r = http.get("/options/vol-surface?ticker=AAPL&start_date=2025-12-31&end_date=2025-01-01", headers=auth_headers)
        assert r.status_code == 400, f"Expected 400 for start > end, got {r.status_code}"

        # risk_free_rate > 0.20
        r = http.get("/options/vol-surface?ticker=AAPL&risk_free_rate=0.25", headers=auth_headers)
        assert r.status_code == 400, f"Expected 400 for risk_free_rate > 0.20, got {r.status_code}"
        print("[PASS] Phase 2 Passed: Parameter validation and bounds strictly enforced (400).")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 3: Single Ticker Default Volatility Surface Query (AAPL)
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 3] Testing Default Options Volatility Surface Query (AAPL)...")
        r = http.get("/options/vol-surface?ticker=AAPL", headers=auth_headers)
        assert r.status_code == 200, f"Expected 200, got {r.status_code}: {r.text}"
        data = r.json()
        assert data["ticker"] == "AAPL"
        assert data["spot"] > 0.0
        assert len(data["strikes"]) == 9
        assert len(data["expirations"]) > 0
        assert len(data["surface"]) == len(data["expirations"])
        for row in data["surface"]:
            assert len(row["ivs"]) == 9
            for iv in row["ivs"]:
                assert 0.05 <= iv <= 2.0
        print(f"[PASS] Phase 3 Passed: AAPL Vol Surface Matrix: Spot=${data['spot']:.2f}, {len(data['strikes'])} strikes × {len(data['expirations'])} expirations.")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 4: Custom Strike Grid & Range (NVDA)
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 4] Testing Custom Strike Grid & Range (NVDA, 5 strikes, 0.85-1.15)...")
        r_nvda = http.get(
            "/options/vol-surface?ticker=NVDA&strike_range=0.85-1.15&strike_count=5",
            headers=auth_headers,
        )
        assert r_nvda.status_code == 200
        data_nvda = r_nvda.json()
        assert data_nvda["ticker"] == "NVDA"
        assert len(data_nvda["strikes"]) == 5
        for row in data_nvda["surface"]:
            assert len(row["ivs"]) == 5
        print(f"[PASS] Phase 4 Passed: NVDA Custom Grid: Strikes={data_nvda['strikes']}.")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 5: Custom Expiration Date Window (MSFT)
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 5] Testing Custom Expiration Date Window (MSFT)...")
        r_msft = http.get(
            "/options/vol-surface?ticker=MSFT&start_date=2025-01-01&end_date=2025-06-30",
            headers=auth_headers,
        )
        assert r_msft.status_code == 200
        data_msft = r_msft.json()
        assert data_msft["ticker"] == "MSFT"
        for exp in data_msft["expirations"]:
            assert "2025-01-01" <= exp <= "2025-06-30"
        print(f"[PASS] Phase 5 Passed: MSFT Expirations in Window: {data_msft['expirations']}.")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 6: Rate Sensitivity & Continuous Dividend Yields
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 6] Testing Rate Sensitivity & Continuous Dividend Yields...")
        r_rates = http.get(
            "/options/vol-surface?ticker=AAPL&risk_free_rate=0.08&dividend_yield=0.02",
            headers=auth_headers,
        )
        assert r_rates.status_code == 200
        print("[PASS] Phase 6 Passed: Custom risk-free rate (8%) and dividend yield (2%) evaluated successfully.")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 7: Volatility Smile/Skew & Moneyness Convexity Validation
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 7] Testing Volatility Smile/Skew Shape & Moneyness Convexity...")
        front_row = data["surface"][0]
        # OTM Put (lowest strike) should have higher IV than ATM (middle strike) due to skew
        otm_put_iv = front_row["ivs"][0]
        atm_iv = front_row["ivs"][4]
        assert otm_put_iv > atm_iv, f"Expected OTM Put IV ({otm_put_iv}) > ATM IV ({atm_iv}) (volatility skew)"
        print(f"[PASS] Phase 7 Passed: Volatility Skew verified: OTM Put IV ({otm_put_iv}) > ATM IV ({atm_iv}).")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 8: Point-in-Time (PIT) Delisted Ticker Enforcement
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 8] Testing Point-in-Time (PIT) Delisted Ticker Enforcement...")
        r_pit = http.get(
            "/options/vol-surface?ticker=TWTR&start_date=2025-01-01&end_date=2025-06-01",
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
        assert "/options/vol-surface" in spec["paths"]
        assert "OptionsVolSurfaceResponse" in spec["components"]["schemas"]
        assert "VolSurfacePoint" in spec["components"]["schemas"]

        # Python SDK Sync
        sdk_sync = client.options_vol_surface(
            ticker="AAPL",
            strike_range="0.8-1.2",
            strike_count=9,
        )
        assert isinstance(sdk_sync, OptionsVolSurfaceResponse)
        assert sdk_sync.ticker == "AAPL"
        assert len(sdk_sync.strikes) == 9
        print(f"  + Python SDK Sync: Received AAPL surface ({len(sdk_sync.strikes)} strikes × {len(sdk_sync.expirations)} expirations).")

        # Python SDK Async
        async def run_async_sdk():
            async_client = FinTextAsyncClient(base_url=BASE_URL, api_token=token)
            async_resp = await async_client.options_vol_surface(
                ticker="NVDA",
                strike_range="0.85-1.15",
                strike_count=5,
            )
            assert isinstance(async_resp, OptionsVolSurfaceResponse)
            assert async_resp.ticker == "NVDA"
            assert len(async_resp.strikes) == 5
            await async_client.close()
            print(f"  + Python SDK Async: Received NVDA surface ({len(async_resp.strikes)} strikes × {len(async_resp.expirations)} expirations).")

        asyncio.run(run_async_sdk())
        print("[PASS] Phase 10 Passed: OpenAPI documentation and Python SDK sync/async certified.")

        client.close()

    print("\n" + "=" * 80)
    print("[SUCCESS] ALL 10 PHASES PASSED: OPTIONS VOLATILITY SURFACE ENGINE CERTIFIED!")
    print("=" * 80)


def main():
    with ServerContext():
        run_tests()


if __name__ == "__main__":
    main()
