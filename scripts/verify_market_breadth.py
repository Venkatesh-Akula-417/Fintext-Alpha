#!/usr/bin/env python3
"""
===============================================================================
FinText-Alpha-Vectorizer — Suite #226: Market Breadth & Advance/Decline Certification
===============================================================================
Verifies:
  1.  Unauthenticated GET /market/breadth returns 401 Unauthorized
  2.  Missing required start_date/end_date returns 400 Bad Request
  3.  Invalid start_date/end_date format returns 400 Bad Request
  4.  Inverted date range (start_date > end_date) returns 400 Bad Request
  5.  Out-of-bounds limit (< 1 or > 200) returns 400 Bad Request
  6.  Custom universe exceeding 100 tickers returns 400 Bad Request
  7.  Invalid ticker symbol in custom universe returns 400 Bad Request
  8.  Default universe="all" breadth analysis returns 200 OK with valid metrics
  9.  universe="sp500" constituent breadth analysis returns 200 OK
  10. Custom constituent universe (AAPL,MSFT,NVDA,AMZN) returns 200 OK
  11. include_new_highs_lows=false correctly omits 52-week high/low metrics
  12. Python SDK sync and async client parity verification
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

PORT = 8126
BASE_URL = f"http://127.0.0.1:{PORT}"
ADMIN_TOKEN = "test_admin_token_xyz123_valid_32_bytes_length!"
SERVER_EXE = PROJECT_ROOT / "rust" / "target" / "debug" / "fintext_api.exe"

passed = 0
failed = 0
total = 12


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


def get_jwt_token(client: httpx.Client, user_id: str = "breadth_quant") -> str:
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
    print(" FinText-Alpha-Vectorizer — Suite #226: Market Breadth & Advance/Decline")
    print("═══════════════════════════════════════════════════════════════════════════════\n")

    with ServerContext():
        client = httpx.Client(base_url=BASE_URL, timeout=10.0)
        token = get_jwt_token(client)
        auth_headers = {"Authorization": f"Bearer {token}"}

        # ── Phase 1: Unauthenticated GET /market/breadth returns 401 ────────
        r = client.get("/market/breadth", params={"start_date": "2025-01-01", "end_date": "2025-01-15"})
        report(1, "Unauthenticated GET /market/breadth returns 401", r.status_code == 401)

        # ── Phase 2: Missing required start_date returns 400 ─────────────────
        r = client.get("/market/breadth", headers=auth_headers, params={"end_date": "2025-01-15"})
        report(2, "Missing parameter 'start_date' returns 400 Bad Request", r.status_code == 400)

        # ── Phase 3: Invalid start_date format returns 400 ──────────────────
        r = client.get(
            "/market/breadth",
            headers=auth_headers,
            params={"start_date": "01-01-2025", "end_date": "2025-01-15"},
        )
        report(3, "Invalid start_date format returns 400 Bad Request", r.status_code == 400)

        # ── Phase 4: Inverted date range (start_date > end_date) returns 400 ─
        r = client.get(
            "/market/breadth",
            headers=auth_headers,
            params={"start_date": "2025-02-01", "end_date": "2025-01-01"},
        )
        report(4, "Inverted date range returns 400 Bad Request", r.status_code == 400)

        # ── Phase 5: Out-of-bounds limit returns 400 ────────────────────────
        r = client.get(
            "/market/breadth",
            headers=auth_headers,
            params={"start_date": "2025-01-01", "end_date": "2025-01-15", "limit": 500},
        )
        report(5, "Out-of-bounds limit (> 200) returns 400 Bad Request", r.status_code == 400)

        # ── Phase 6: Custom universe exceeding 100 tickers returns 400 ──────
        huge_universe = ",".join([f"TICK{i}" for i in range(120)])
        r = client.get(
            "/market/breadth",
            headers=auth_headers,
            params={"start_date": "2025-01-01", "end_date": "2025-01-15", "universe": huge_universe},
        )
        report(6, "Custom universe with > 100 tickers returns 400 Bad Request", r.status_code == 400)

        # ── Phase 7: Invalid ticker symbol returns 400 ──────────────────────
        r = client.get(
            "/market/breadth",
            headers=auth_headers,
            params={"start_date": "2025-01-01", "end_date": "2025-01-15", "universe": "AAPL,INVALID$$TICKER"},
        )
        report(7, "Invalid ticker symbol in custom universe returns 400 Bad Request", r.status_code == 400)

        # ── Phase 8: Default universe='all' returns valid 200 OK breadth ────
        r = client.get(
            "/market/breadth",
            headers=auth_headers,
            params={"start_date": "2025-01-01", "end_date": "2025-01-15", "universe": "all", "limit": 10},
        )
        data = r.json()
        p8_ok = (
            r.status_code == 200
            and data["universe"] == "all"
            and data["total_tickers"] > 0
            and data["count"] > 0
            and len(data["points"]) == data["count"]
            and data["points"][0]["advancers"] >= 0
            and data["points"][0]["decliners"] >= 0
            and 0.0 <= data["points"][0]["advance_decline_ratio"] <= 1.0
            and -1.0 <= data["points"][0]["breadth_index"] <= 1.0
            and data["points"][0].get("new_52w_highs") is not None
            and data["points"][0].get("new_52w_lows") is not None
        )
        report(8, "Default universe='all' returns valid advance/decline metrics and 52w highs/lows", p8_ok)

        # ── Phase 9: universe='sp500' returns valid 200 OK ──────────────────
        r = client.get(
            "/market/breadth",
            headers=auth_headers,
            params={"start_date": "2025-01-01", "end_date": "2025-01-15", "universe": "sp500", "limit": 10},
        )
        data = r.json()
        p9_ok = (
            r.status_code == 200
            and data["universe"] == "sp500"
            and data["total_tickers"] > 0
            and len(data["points"]) > 0
        )
        report(9, "universe='sp500' calculates market breadth across index constituents", p9_ok)

        # ── Phase 10: Custom constituent universe (AAPL,MSFT,NVDA,AMZN) ─────
        r = client.get(
            "/market/breadth",
            headers=auth_headers,
            params={"start_date": "2025-01-01", "end_date": "2025-01-15", "universe": "AAPL,MSFT,NVDA,AMZN"},
        )
        data = r.json()
        p10_ok = (
            r.status_code == 200
            and data["total_tickers"] == 4
            and len(data["points"]) > 0
            and (data["points"][0]["advancers"] + data["points"][0]["decliners"] + data["points"][0]["unchanged"]) <= 4
        )
        report(10, "Custom constituent universe computes portfolio-level breadth", p10_ok)

        # ── Phase 11: include_new_highs_lows=false omits 52w fields ─────────
        r = client.get(
            "/market/breadth",
            headers=auth_headers,
            params={"start_date": "2025-01-01", "end_date": "2025-01-15", "include_new_highs_lows": "false"},
        )
        data = r.json()
        p11_ok = (
            r.status_code == 200
            and len(data["points"]) > 0
            and "new_52w_highs" not in data["points"][0]
            and "new_52w_lows" not in data["points"][0]
        )
        report(11, "include_new_highs_lows=false correctly omits 52-week high/low fields", p11_ok)

        # ── Phase 12: Python SDK sync & async parity ────────────────────────
        from fintext import FinTextClient, FinTextAsyncClient, MarketBreadthResponse

        sync_sdk = FinTextClient(base_url=BASE_URL, api_token=token)
        sync_res = sync_sdk.market_breadth(
            start_date="2025-01-01",
            end_date="2025-01-15",
            universe="all",
            limit=10,
            include_new_highs_lows=True,
        )

        async def run_async_sdk():
            async_sdk = FinTextAsyncClient(base_url=BASE_URL, api_token=token)
            res = await async_sdk.market_breadth(
                start_date="2025-01-01",
                end_date="2025-01-15",
                universe="all",
                limit=10,
                include_new_highs_lows=True,
            )
            await async_sdk.close()
            return res

        async_res = asyncio.run(run_async_sdk())

        p12_ok = (
            isinstance(sync_res, MarketBreadthResponse)
            and isinstance(async_res, MarketBreadthResponse)
            and sync_res.universe == "all"
            and async_res.universe == "all"
            and sync_res.count == async_res.count
            and len(sync_res.points) > 0
            and len(async_res.points) > 0
            and sync_res.points[0].advancers == async_res.points[0].advancers
        )
        report(12, "Python SDK sync and async client parity verified", p12_ok)

    print("\n═══════════════════════════════════════════════════════════════════════════════")
    print(f" Certification Results: {passed}/{total} Passed ({(passed/total)*100:.1f}%)")
    print("═══════════════════════════════════════════════════════════════════════════════\n")

    if failed > 0:
        sys.exit(1)


if __name__ == "__main__":
    main()
