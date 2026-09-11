#!/usr/bin/env python3
"""
===============================================================================
FinText-Alpha-Vectorizer — Suite #230: Portfolio Optimization Certification
===============================================================================
Verifies:
  1.  Unauthenticated POST /portfolio/optimize returns 401 Unauthorized
  2.  Missing required 'tickers' field returns 400 Bad Request
  3.  Single ticker (< 2 tickers) returns 400 Bad Request
  4.  Empty / whitespace tickers parameter returns 400 Bad Request
  5.  Too many tickers (> 20 tickers) returns 400 Bad Request
  6.  Invalid ticker format returns 400 Bad Request
  7.  Invalid date format returns 400 Bad Request
  8.  Inverted date range (start_date > end_date) returns 400 Bad Request
  9.  Out-of-bounds risk_free_rate (< 0.0 or > 0.10) returns 400 Bad Request
  10. Invalid optimization_type returns 400 Bad Request
  11. Valid max_sharpe optimization returns 200 OK with valid weights and metrics
  12. Valid risk_parity optimization returns 200 OK with balanced positive weights
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

PORT = 8130
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


def get_jwt_token(client: httpx.Client, user_id: str = "portfolio_architect") -> str:
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
    print(" FinText-Alpha-Vectorizer — Suite #230: Portfolio Optimization Engine")
    print("═══════════════════════════════════════════════════════════════════════════════\n")

    with ServerContext():
        client = httpx.Client(base_url=BASE_URL, timeout=10.0)
        token = get_jwt_token(client)
        auth_headers = {
            "Authorization": f"Bearer {token}",
            "Content-Type": "application/json",
        }

        # ── Phase 1: Unauthenticated request returns 401 ─────────────────────
        r1 = client.post("/portfolio/optimize", json={"tickers": ["AAPL", "MSFT"], "start_date": "2025-01-01", "end_date": "2025-06-30"})
        report(1, "Unauthenticated POST /portfolio/optimize returns 401", r1.status_code == 401)

        # ── Phase 2: Missing tickers parameter returns 400 ───────────────────
        r2 = client.post("/portfolio/optimize", headers=auth_headers, json={"start_date": "2025-01-01", "end_date": "2025-06-30"})
        report(2, "Missing 'tickers' parameter returns 400 Bad Request", r2.status_code == 400 or r2.status_code == 422)

        # ── Phase 3: Single ticker (< 2) returns 400 ─────────────────────────
        r3 = client.post("/portfolio/optimize", headers=auth_headers, json={"tickers": ["AAPL"], "start_date": "2025-01-01", "end_date": "2025-06-30"})
        report(3, "Single ticker (< 2 tickers) returns 400 Bad Request", r3.status_code == 400)

        # ── Phase 4: Empty tickers list returns 400 ──────────────────────────
        r4 = client.post("/portfolio/optimize", headers=auth_headers, json={"tickers": [], "start_date": "2025-01-01", "end_date": "2025-06-30"})
        report(4, "Empty tickers parameter returns 400 Bad Request", r4.status_code == 400)

        # ── Phase 5: Too many tickers (> 20) returns 400 ─────────────────────
        too_many = [f"TICK{i}" for i in range(25)]
        r5 = client.post("/portfolio/optimize", headers=auth_headers, json={"tickers": too_many, "start_date": "2025-01-01", "end_date": "2025-06-30"})
        report(5, "Too many tickers (> 20 tickers) returns 400 Bad Request", r5.status_code == 400)

        # ── Phase 6: Invalid ticker format returns 400 ───────────────────────
        r6 = client.post("/portfolio/optimize", headers=auth_headers, json={"tickers": ["AAPL", "INVALID$$$"], "start_date": "2025-01-01", "end_date": "2025-06-30"})
        report(6, "Invalid ticker format returns 400 Bad Request", r6.status_code == 400)

        # ── Phase 7: Invalid date format returns 400 ─────────────────────────
        r7 = client.post("/portfolio/optimize", headers=auth_headers, json={"tickers": ["AAPL", "MSFT"], "start_date": "2025/01/01", "end_date": "2025-06-30"})
        report(7, "Invalid date format returns 400 Bad Request", r7.status_code == 400)

        # ── Phase 8: Inverted dates returns 400 ──────────────────────────────
        r8 = client.post("/portfolio/optimize", headers=auth_headers, json={"tickers": ["AAPL", "MSFT"], "start_date": "2025-07-01", "end_date": "2025-06-01"})
        report(8, "Inverted dates (start_date > end_date) returns 400 Bad Request", r8.status_code == 400)

        # ── Phase 9: Out-of-bounds risk-free rate returns 400 ────────────────
        r9_neg = client.post("/portfolio/optimize", headers=auth_headers, json={"tickers": ["AAPL", "MSFT"], "start_date": "2025-01-01", "end_date": "2025-06-30", "risk_free_rate": -0.05})
        r9_large = client.post("/portfolio/optimize", headers=auth_headers, json={"tickers": ["AAPL", "MSFT"], "start_date": "2025-01-01", "end_date": "2025-06-30", "risk_free_rate": 0.25})
        report(9, "Out-of-bounds risk_free_rate returns 400 Bad Request", r9_neg.status_code == 400 and r9_large.status_code == 400)

        # ── Phase 10: Invalid optimization_type returns 400 ──────────────────
        r10 = client.post("/portfolio/optimize", headers=auth_headers, json={"tickers": ["AAPL", "MSFT"], "start_date": "2025-01-01", "end_date": "2025-06-30", "optimization_type": "unknown_opt"})
        report(10, "Invalid optimization_type returns 400 Bad Request", r10.status_code == 400)

        # ── Phase 11: Valid max_sharpe optimization returns 200 OK ───────────
        r11 = client.post(
            "/portfolio/optimize",
            headers=auth_headers,
            json={
                "tickers": ["AAPL", "MSFT", "NVDA", "AMZN"],
                "start_date": "2025-01-01",
                "end_date": "2025-06-30",
                "optimization_type": "max_sharpe",
                "risk_free_rate": 0.03,
                "constraints": {"long_only": True},
            },
        )
        data11 = r11.json()
        weights11 = data11.get("weights", [])
        sum_w11 = sum(w.get("weight", 0.0) for w in weights11)
        p11_ok = (
            r11.status_code == 200
            and len(data11.get("tickers", [])) == 4
            and data11.get("optimization_type") == "max_sharpe"
            and data11.get("risk_free_rate") == 0.03
            and len(weights11) == 4
            and abs(sum_w11 - 1.0) < 0.01
            and isinstance(data11.get("expected_annual_return"), (int, float))
            and isinstance(data11.get("expected_annual_volatility"), (int, float))
            and isinstance(data11.get("sharpe_ratio"), (int, float))
            and bool(data11.get("generated_at"))
        )
        report(11, "Valid max_sharpe optimization returns 200 OK with valid weights & metrics", p11_ok)

        # ── Phase 12: Valid risk_parity optimization returns 200 OK ──────────
        r12 = client.post(
            "/portfolio/optimize",
            headers=auth_headers,
            json={
                "tickers": ["AAPL", "MSFT", "NVDA", "GOOGL"],
                "start_date": "2025-01-01",
                "end_date": "2025-06-30",
                "optimization_type": "risk_parity",
                "risk_free_rate": 0.02,
            },
        )
        data12 = r12.json()
        weights12 = data12.get("weights", [])
        sum_w12 = sum(w.get("weight", 0.0) for w in weights12)
        p12_ok = (
            r12.status_code == 200
            and len(data12.get("tickers", [])) == 4
            and data12.get("optimization_type") == "risk_parity"
            and len(weights12) == 4
            and abs(sum_w12 - 1.0) < 0.01
            and all(w.get("weight", 0.0) > 0.0 for w in weights12)
        )
        report(12, "Valid risk_parity optimization returns 200 OK with balanced positive weights", p12_ok)



        # ── Phase 13: Python SDK sync & async client parity ──────────────────
        from fintext import FinTextClient, FinTextAsyncClient, PortfolioOptimizeResponse

        sync_sdk = FinTextClient(base_url=BASE_URL, api_token=token)
        sync_res = sync_sdk.portfolio_optimize(
            tickers=["AAPL", "MSFT", "TSLA"],
            start_date="2025-01-01",
            end_date="2025-06-30",
            optimization_type="max_sharpe",
            risk_free_rate=0.04,
        )

        async def run_async_sdk():
            async_sdk = FinTextAsyncClient(base_url=BASE_URL, api_token=token)
            res = await async_sdk.portfolio_optimize(
                tickers=["AAPL", "META", "NVDA"],
                start_date="2025-01-01",
                end_date="2025-06-30",
                optimization_type="risk_parity",
                risk_free_rate=0.03,
            )
            await async_sdk.close()
            return res

        async_res = asyncio.run(run_async_sdk())

        p13_ok = (
            isinstance(sync_res, PortfolioOptimizeResponse)
            and len(sync_res.tickers) == 3
            and sync_res.optimization_type == "max_sharpe"
            and isinstance(async_res, PortfolioOptimizeResponse)
            and len(async_res.tickers) == 3
            and async_res.optimization_type == "risk_parity"
        )
        report(13, "Python SDK sync and async client parity verified", p13_ok)

    print("\n═══════════════════════════════════════════════════════════════════════════════")
    print(f" Certification Results: {passed}/{total} Passed ({(passed/total)*100:.1f}%)")
    print("═══════════════════════════════════════════════════════════════════════════════\n")

    if failed > 0:
        sys.exit(1)


if __name__ == "__main__":
    main()
