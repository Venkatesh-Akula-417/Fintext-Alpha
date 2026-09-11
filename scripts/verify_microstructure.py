#!/usr/bin/env python3
"""
===============================================================================
FinText-Alpha-Vectorizer — Suite #225: Options Market Microstructure (VPIN/GEX) Certification
===============================================================================
Verifies:
  1.  Unauthenticated GET /options/microstructure returns 401 Unauthorized
  2.  Missing required parameter 'ticker' returns 400 Bad Request
  3.  Invalid start_date/end_date format returns 400 Bad Request
  4.  Inverted date range (start_date > end_date) returns 400 Bad Request
  5.  Invalid metric ('invalid_metric') returns 400 Bad Request
  6.  Invalid interval ('monthly') returns 400 Bad Request
  7.  Out-of-bounds limit (< 1 or > 1000) returns 400 Bad Request
  8.  Default daily microstructure query for AAPL returns 200 OK with both VPIN & GEX
  9.  VPIN-only metric filter returns 200 OK (vpin present, gex omitted)
  10. GEX-only metric filter returns 200 OK (gex present, vpin omitted)
  11. Intraday interval calculation returns 200 OK with hourly/intraday points
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

PORT = 8125
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
                self.process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.process.kill()


def get_jwt_token(client: httpx.Client, user_id: str = "microstructure_trader") -> str:
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
    print(" FinText-Alpha-Vectorizer — Suite #225: Options Microstructure (VPIN/GEX)")
    print("═══════════════════════════════════════════════════════════════════════════════\n")

    with ServerContext():
        client = httpx.Client(base_url=BASE_URL, timeout=10.0)
        token = get_jwt_token(client)
        auth_headers = {"Authorization": f"Bearer {token}"}

        # ── Phase 1: Unauthenticated GET /options/microstructure returns 401 ─
        r = client.get("/options/microstructure", params={"ticker": "AAPL"})
        report(1, "Unauthenticated GET /options/microstructure returns 401", r.status_code == 401)

        # ── Phase 2: Missing required parameter 'ticker' returns 400 ────────
        r = client.get("/options/microstructure", headers=auth_headers)
        report(2, "Missing parameter 'ticker' returns 400 Bad Request", r.status_code == 400)

        # ── Phase 3: Invalid start_date format returns 400 ──────────────────
        r = client.get(
            "/options/microstructure",
            headers=auth_headers,
            params={"ticker": "AAPL", "start_date": "invalid-date", "end_date": "2025-08-31"},
        )
        report(3, "Invalid start_date format returns 400 Bad Request", r.status_code == 400)

        # ── Phase 4: Inverted date range (start_date > end_date) returns 400 ─
        r = client.get(
            "/options/microstructure",
            headers=auth_headers,
            params={"ticker": "AAPL", "start_date": "2025-09-01", "end_date": "2025-08-01"},
        )
        report(4, "Inverted date range returns 400 Bad Request", r.status_code == 400)

        # ── Phase 5: Invalid metric parameter returns 400 ───────────────────
        r = client.get(
            "/options/microstructure",
            headers=auth_headers,
            params={"ticker": "AAPL", "start_date": "2025-08-01", "end_date": "2025-08-31", "metric": "invalid_metric"},
        )
        report(5, "Invalid metric returns 400 Bad Request", r.status_code == 400)

        # ── Phase 6: Invalid interval parameter returns 400 ─────────────────
        r = client.get(
            "/options/microstructure",
            headers=auth_headers,
            params={"ticker": "AAPL", "start_date": "2025-08-01", "end_date": "2025-08-31", "interval": "monthly"},
        )
        report(6, "Invalid interval returns 400 Bad Request", r.status_code == 400)

        # ── Phase 7: Out-of-bounds limit returns 400 ────────────────────────
        r = client.get(
            "/options/microstructure",
            headers=auth_headers,
            params={"ticker": "AAPL", "start_date": "2025-08-01", "end_date": "2025-08-31", "limit": 2000},
        )
        report(7, "Out-of-bounds limit (> 1000) returns 400 Bad Request", r.status_code == 400)

        # ── Phase 8: Default daily microstructure query for AAPL returns 200 OK
        r = client.get(
            "/options/microstructure",
            headers=auth_headers,
            params={"ticker": "AAPL", "start_date": "2025-08-01", "end_date": "2025-08-15", "metric": "both", "interval": "daily"},
        )
        data = r.json()
        p8_ok = (
            r.status_code == 200
            and data["ticker"] == "AAPL"
            and data["metric"] == "both"
            and data["interval"] == "daily"
            and data["count"] > 0
            and len(data["points"]) == data["count"]
            and data["points"][0].get("vpin") is not None
            and data["points"][0].get("gex") is not None
        )
        report(8, "Default daily microstructure returns valid VPIN and GEX points", p8_ok)

        # ── Phase 9: VPIN-only metric filter returns 200 OK ─────────────────
        r = client.get(
            "/options/microstructure",
            headers=auth_headers,
            params={"ticker": "NVDA", "start_date": "2025-08-01", "end_date": "2025-08-10", "metric": "vpin"},
        )
        data = r.json()
        p9_ok = (
            r.status_code == 200
            and data["ticker"] == "NVDA"
            and data["metric"] == "vpin"
            and len(data["points"]) > 0
            and data["points"][0].get("vpin") is not None
            and data["points"][0].get("gex") is None
        )
        report(9, "VPIN-only metric filter correctly includes VPIN and omits GEX", p9_ok)

        # ── Phase 10: GEX-only metric filter returns 200 OK ─────────────────
        r = client.get(
            "/options/microstructure",
            headers=auth_headers,
            params={"ticker": "SPY", "start_date": "2025-08-01", "end_date": "2025-08-10", "metric": "gex"},
        )
        data = r.json()
        p10_ok = (
            r.status_code == 200
            and data["ticker"] == "SPY"
            and data["metric"] == "gex"
            and len(data["points"]) > 0
            and data["points"][0].get("gex") is not None
            and data["points"][0].get("vpin") is None
        )
        report(10, "GEX-only metric filter correctly includes GEX and omits VPIN", p10_ok)

        # ── Phase 11: Intraday interval calculation returns 200 OK ──────────
        r = client.get(
            "/options/microstructure",
            headers=auth_headers,
            params={"ticker": "TSLA", "start_date": "2025-08-01", "end_date": "2025-08-03", "interval": "intraday", "limit": 50},
        )
        data = r.json()
        p11_ok = (
            r.status_code == 200
            and data["ticker"] == "TSLA"
            and data["interval"] == "intraday"
            and len(data["points"]) > 0
            and ":" in data["points"][0]["timestamp"]
        )
        report(11, "Intraday interval query returns sub-daily time points", p11_ok)

        # ── Phase 12: Python SDK sync & async client parity ─────────────────
        from fintext import FinTextClient, FinTextAsyncClient, MicrostructureResponse

        sync_sdk = FinTextClient(base_url=BASE_URL, api_token=token)
        sync_res = sync_sdk.options_microstructure(
            ticker="AAPL",
            start_date="2025-08-01",
            end_date="2025-08-15",
            metric="both",
            interval="daily",
        )

        async def run_async_sdk():
            async_sdk = FinTextAsyncClient(base_url=BASE_URL, api_token=token)
            res = await async_sdk.options_microstructure(
                ticker="AAPL",
                start_date="2025-08-01",
                end_date="2025-08-15",
                metric="both",
                interval="daily",
            )
            await async_sdk.close()
            return res

        async_res = asyncio.run(run_async_sdk())

        p12_ok = (
            isinstance(sync_res, MicrostructureResponse)
            and isinstance(async_res, MicrostructureResponse)
            and sync_res.ticker == "AAPL"
            and async_res.ticker == "AAPL"
            and sync_res.count == async_res.count
            and len(sync_res.points) > 0
            and len(async_res.points) > 0
        )
        report(12, "Python SDK sync and async client parity verified", p12_ok)

    print("\n═══════════════════════════════════════════════════════════════════════════════")
    print(f" Certification Results: {passed}/{total} Passed ({(passed/total)*100:.1f}%)")
    print("═══════════════════════════════════════════════════════════════════════════════\n")

    if failed > 0:
        sys.exit(1)


if __name__ == "__main__":
    main()
