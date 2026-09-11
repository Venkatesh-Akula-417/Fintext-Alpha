#!/usr/bin/env python3
"""
===============================================================================
FinText-Alpha-Vectorizer — Suite #232: Portfolio Factor Exposure Certification
===============================================================================
Verifies:
  1.  Unauthenticated POST /risk/portfolio-factor-exposure returns 401 Unauthorized
  2.  Missing required parameter ('tickers'/'weights') returns 400 Bad Request
  3.  Single ticker (< 2 tickers) returns 400 Bad Request
  4.  Empty tickers list returns 400 Bad Request
  5.  Too many tickers (> 20 tickers) returns 400 Bad Request
  6.  Weights array length mismatch returns 400 Bad Request
  7.  Weights sum out of range (sum < 0.95 or sum > 1.05) returns 400 Bad Request
  8.  Duplicate ticker in constituent list returns 400 Bad Request
  9.  Inverted date range (start_date > end_date) returns 400 Bad Request
  10. Unsupported factor name returns 400 Bad Request
  11. Valid default 4-factor regression returns 200 OK with Betas, t-stats, and R^2
  12. Valid custom 6-factor regression with custom benchmark returns 200 OK
  13. Python SDK sync and async client parity verification
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

PORT = 8131
BASE_URL = f"http://127.0.0.1:{PORT}"
ADMIN_TOKEN = "test_admin_token_xyz123_valid_32_bytes_length!"
SERVER_EXE = PROJECT_ROOT / "rust" / "target" / "debug" / "fintext_api.exe"

passed = 0
failed = 0
total = 13


def report(phase: int, name: str, ok: bool, detail: str = ""):
    global passed, failed
    if ok:
        passed += 1
        print(f"  ✅ Phase {phase:2d} │ {name}")
    else:
        failed += 1
        msg = f"  ❌ Phase {phase:2d} │ {name}"
        if detail:
            msg += f" — {detail}"
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
                time.sleep(0.3)

        if not ready:
            if self.process.poll() is not None:
                raise RuntimeError("FinText API server process exited prematurely")
            raise RuntimeError("FinText API server failed to start within 25 seconds")

        print(f"[READY] FinText API Server is responding on {BASE_URL}.\n")
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        if self.process:
            print(f"[STOPPING] Terminating FinText API Server (PID: {self.process.pid})...")
            self.process.terminate()
            try:
                self.process.wait(timeout=5.0)
            except subprocess.TimeoutExpired:
                self.process.kill()


def get_jwt_token(client: httpx.Client, user_id: str = "portfolio_risk_desk") -> str:
    r = client.post(
        f"{BASE_URL}/auth/token",
        headers={"X-Admin-Token": ADMIN_TOKEN},
        json={"user_id": user_id},
    )
    assert r.status_code == 200, f"Failed to acquire JWT: {r.text}"
    return r.json()["token"]


def main():
    global passed, failed

    print("═══════════════════════════════════════════════════════════════════════════════")
    print(" FinText-Alpha-Vectorizer — Suite #232: Portfolio Factor Exposure Engine")
    print("═══════════════════════════════════════════════════════════════════════════════\n")

    with ServerContext():
        client = httpx.Client(base_url=BASE_URL, timeout=10.0)
        token = get_jwt_token(client)
        auth_headers = {
            "Authorization": f"Bearer {token}",
            "Content-Type": "application/json",
        }

        # ── Phase 1: Unauthenticated request returns 401 ─────────────────────
        r1 = client.post(
            "/risk/portfolio-factor-exposure",
            json={
                "tickers": ["AAPL", "MSFT"],
                "weights": [0.5, 0.5],
                "start_date": "2025-01-01",
                "end_date": "2025-06-30",
            },
        )
        report(1, "Unauthenticated POST /risk/portfolio-factor-exposure returns 401", r1.status_code == 401)

        # ── Phase 2: Missing required parameter returns 400/422 ──────────────
        r2 = client.post(
            "/risk/portfolio-factor-exposure",
            headers=auth_headers,
            json={"tickers": ["AAPL", "MSFT"], "start_date": "2025-01-01", "end_date": "2025-06-30"},
        )
        report(2, "Missing 'weights' parameter returns 400/422 Bad Request", r2.status_code in [400, 422])

        # ── Phase 3: Single ticker (< 2) returns 400 ─────────────────────────
        r3 = client.post(
            "/risk/portfolio-factor-exposure",
            headers=auth_headers,
            json={
                "tickers": ["AAPL"],
                "weights": [1.0],
                "start_date": "2025-01-01",
                "end_date": "2025-06-30",
            },
        )
        report(3, "Single ticker (< 2 tickers) returns 400 Bad Request", r3.status_code == 400)

        # ── Phase 4: Empty tickers list returns 400 ──────────────────────────
        r4 = client.post(
            "/risk/portfolio-factor-exposure",
            headers=auth_headers,
            json={
                "tickers": [],
                "weights": [],
                "start_date": "2025-01-01",
                "end_date": "2025-06-30",
            },
        )
        report(4, "Empty tickers parameter returns 400 Bad Request", r4.status_code == 400)

        # ── Phase 5: Too many tickers (> 20) returns 400 ─────────────────────
        too_many = [f"TICK{i}" for i in range(25)]
        weights_25 = [0.04] * 25
        r5 = client.post(
            "/risk/portfolio-factor-exposure",
            headers=auth_headers,
            json={
                "tickers": too_many,
                "weights": weights_25,
                "start_date": "2025-01-01",
                "end_date": "2025-06-30",
            },
        )
        report(5, "Too many tickers (> 20 tickers) returns 400 Bad Request", r5.status_code == 400)

        # ── Phase 6: Weights array length mismatch returns 400 ───────────────
        r6 = client.post(
            "/risk/portfolio-factor-exposure",
            headers=auth_headers,
            json={
                "tickers": ["AAPL", "MSFT", "NVDA"],
                "weights": [0.5, 0.5],
                "start_date": "2025-01-01",
                "end_date": "2025-06-30",
            },
        )
        report(6, "Weights length mismatch (3 tickers vs 2 weights) returns 400 Bad Request", r6.status_code == 400)

        # ── Phase 7: Weights sum out of bounds returns 400 ───────────────────
        r7 = client.post(
            "/risk/portfolio-factor-exposure",
            headers=auth_headers,
            json={
                "tickers": ["AAPL", "MSFT"],
                "weights": [0.3, 0.3],  # sum = 0.60
                "start_date": "2025-01-01",
                "end_date": "2025-06-30",
            },
        )
        report(7, "Weights sum out of range (sum = 0.60 < 0.95) returns 400 Bad Request", r7.status_code == 400)

        # ── Phase 8: Duplicate ticker returns 400 ────────────────────────────
        r8 = client.post(
            "/risk/portfolio-factor-exposure",
            headers=auth_headers,
            json={
                "tickers": ["AAPL", "AAPL"],
                "weights": [0.5, 0.5],
                "start_date": "2025-01-01",
                "end_date": "2025-06-30",
            },
        )
        report(8, "Duplicate ticker 'AAPL' in portfolio returns 400 Bad Request", r8.status_code == 400)

        # ── Phase 9: Inverted dates returns 400 ──────────────────────────────
        r9 = client.post(
            "/risk/portfolio-factor-exposure",
            headers=auth_headers,
            json={
                "tickers": ["AAPL", "MSFT"],
                "weights": [0.5, 0.5],
                "start_date": "2025-07-01",
                "end_date": "2025-01-01",
            },
        )
        report(9, "Inverted dates (start_date > end_date) returns 400 Bad Request", r9.status_code == 400)

        # ── Phase 10: Unsupported factor name returns 400 ────────────────────
        r10 = client.post(
            "/risk/portfolio-factor-exposure",
            headers=auth_headers,
            json={
                "tickers": ["AAPL", "MSFT"],
                "weights": [0.5, 0.5],
                "start_date": "2025-01-01",
                "end_date": "2025-06-30",
                "factors": "market,invalid_factor",
            },
        )
        report(10, "Unsupported factor 'invalid_factor' returns 400 Bad Request", r10.status_code == 400)

        # ── Phase 11: Valid default 4-factor regression returns 200 OK ───────
        r11 = client.post(
            "/risk/portfolio-factor-exposure",
            headers=auth_headers,
            json={
                "tickers": ["AAPL", "MSFT", "NVDA"],
                "weights": [0.4, 0.3, 0.3],
                "start_date": "2025-01-01",
                "end_date": "2025-06-30",
            },
        )
        data11 = r11.json()
        exposures11 = data11.get("exposures", [])
        ols11 = data11.get("ols_summary", {})
        p11_ok = (
            r11.status_code == 200
            and data11.get("tickers") == ["AAPL", "MSFT", "NVDA"]
            and data11.get("benchmark_ticker") == "SPY"
            and len(data11.get("factors_included", [])) == 4
            and len(exposures11) == 4
            and isinstance(ols11.get("r_squared"), (int, float))
            and isinstance(ols11.get("num_observations"), int)
            and ols11.get("num_observations", 0) >= 10
            and all(isinstance(exp.get("beta"), (int, float)) for exp in exposures11)
            and all(isinstance(exp.get("t_stat"), (int, float)) for exp in exposures11)
            and all(isinstance(exp.get("p_value"), (int, float)) for exp in exposures11)
            and bool(data11.get("generated_at"))
        )
        report(11, "Valid default 4-factor regression returns 200 OK with Betas & R^2", p11_ok)

        # ── Phase 12: Valid custom 6-factor regression with custom benchmark ─
        r12 = client.post(
            "/risk/portfolio-factor-exposure",
            headers=auth_headers,
            json={
                "tickers": ["AAPL", "MSFT", "GOOGL", "AMZN"],
                "weights": [0.25, 0.25, 0.25, 0.25],
                "start_date": "2025-01-01",
                "end_date": "2025-06-30",
                "benchmark_ticker": "SPY",
                "factors": "market,momentum,sentiment,volatility,size,value",
            },
        )
        data12 = r12.json()
        exposures12 = data12.get("exposures", [])
        p12_ok = (
            r12.status_code == 200
            and len(data12.get("tickers", [])) == 4
            and len(data12.get("factors_included", [])) == 6
            and len(exposures12) == 6
            and {e.get("factor") for e in exposures12} == {"market", "momentum", "sentiment", "volatility", "size", "value"}
        )
        report(12, "Valid custom 6-factor regression returns 200 OK with all 6 factors", p12_ok)

        # ── Phase 13: Python SDK sync & async client parity ──────────────────
        from fintext import FinTextClient, FinTextAsyncClient, PortfolioFactorExposureResponse

        sync_sdk = FinTextClient(base_url=BASE_URL, api_token=token)
        sync_res = sync_sdk.portfolio_factor_exposure(
            tickers=["AAPL", "MSFT", "TSLA"],
            weights=[0.4, 0.3, 0.3],
            start_date="2025-01-01",
            end_date="2025-06-30",
            benchmark_ticker="SPY",
            factors=["market", "momentum", "sentiment", "volatility"],
        )

        async def run_async_sdk():
            async_sdk = FinTextAsyncClient(base_url=BASE_URL, api_token=token)
            res = await async_sdk.portfolio_factor_exposure(
                tickers=["AAPL", "META", "NVDA"],
                weights=[0.34, 0.33, 0.33],
                start_date="2025-01-01",
                end_date="2025-06-30",
                benchmark_ticker="SPY",
                factors="market,momentum,sentiment,volatility",
            )
            await async_sdk.close()
            return res

        async_res = asyncio.run(run_async_sdk())

        p13_ok = (
            isinstance(sync_res, PortfolioFactorExposureResponse)
            and len(sync_res.tickers) == 3
            and len(sync_res.exposures) == 4
            and isinstance(async_res, PortfolioFactorExposureResponse)
            and len(async_res.tickers) == 3
            and len(async_res.exposures) == 4
        )
        report(13, "Python SDK sync and async client parity verified", p13_ok)

    print("\n═══════════════════════════════════════════════════════════════════════════════")
    print(f" Certification Results: {passed}/{total} Passed ({(passed/total)*100:.1f}%)")
    print("═══════════════════════════════════════════════════════════════════════════════\n")

    if failed > 0:
        sys.exit(1)


if __name__ == "__main__":
    main()
