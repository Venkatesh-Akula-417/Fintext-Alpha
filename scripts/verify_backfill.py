#!/usr/bin/env python3
"""
===============================================================================
FinText-Alpha-Vectorizer — Suite #229: News Sentiment Backfill Certification
===============================================================================
Verifies:
  1.  Unauthenticated POST /sentiment/backfill returns 401 Unauthorized
  2.  Missing required 'ticker' parameter returns 400 Bad Request
  3.  Empty ticker string returns 400 Bad Request
  4.  Invalid ticker format returns 400 Bad Request
  5.  Invalid date format returns 400 Bad Request
  6.  Inverted dates (start_date > end_date) returns 400 Bad Request
  7.  Out-of-bounds limit (< 1 or > 10000) returns 400 Bad Request
  8.  Valid backfill request with overwrite=false returns 200 OK
  9.  Valid backfill request with overwrite=true returns 200 OK
  10. Verification of total_articles_found, processed_articles, and failed_articles counts
  11. Non-matching ticker / empty range returns 200 OK with 0 found & processed
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

PORT = 8129
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


def get_jwt_token(client: httpx.Client, user_id: str = "backfill_specialist") -> str:
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
    print(" FinText-Alpha-Vectorizer — Suite #229: News Sentiment Backfill Engine")
    print("═══════════════════════════════════════════════════════════════════════════════\n")

    with ServerContext():
        client = httpx.Client(base_url=BASE_URL, timeout=10.0)
        token = get_jwt_token(client)
        auth_headers = {
            "Authorization": f"Bearer {token}",
            "Content-Type": "application/json",
        }

        # ── Phase 1: Unauthenticated request returns 401 ─────────────────────
        r1 = client.post("/sentiment/backfill", json={"ticker": "AAPL", "start_date": "2025-01-01", "end_date": "2025-03-31"})
        report(1, "Unauthenticated POST /sentiment/backfill returns 401", r1.status_code == 401)

        # ── Phase 2: Missing ticker returns 400 ──────────────────────────────
        r2 = client.post("/sentiment/backfill", headers=auth_headers, json={"start_date": "2025-01-01", "end_date": "2025-03-31"})
        report(2, "Missing 'ticker' parameter returns 400 Bad Request", r2.status_code == 400 or r2.status_code == 422)

        # ── Phase 3: Empty ticker string returns 400 ─────────────────────────
        r3 = client.post("/sentiment/backfill", headers=auth_headers, json={"ticker": "", "start_date": "2025-01-01", "end_date": "2025-03-31"})
        report(3, "Empty ticker parameter returns 400 Bad Request", r3.status_code == 400)

        # ── Phase 4: Invalid ticker format returns 400 ───────────────────────
        r4 = client.post("/sentiment/backfill", headers=auth_headers, json={"ticker": "INVALID_TOOLONG_TICKER123", "start_date": "2025-01-01", "end_date": "2025-03-31"})
        report(4, "Invalid ticker format returns 400 Bad Request", r4.status_code == 400)

        # ── Phase 5: Invalid date format returns 400 ─────────────────────────
        r5 = client.post("/sentiment/backfill", headers=auth_headers, json={"ticker": "AAPL", "start_date": "2025/01/01", "end_date": "2025-03-31"})
        report(5, "Invalid date format returns 400 Bad Request", r5.status_code == 400)

        # ── Phase 6: Inverted dates returns 400 ──────────────────────────────
        r6 = client.post("/sentiment/backfill", headers=auth_headers, json={"ticker": "AAPL", "start_date": "2025-04-01", "end_date": "2025-03-01"})
        report(6, "Inverted dates (start_date > end_date) returns 400 Bad Request", r6.status_code == 400)

        # ── Phase 7: Out-of-bounds limit returns 400 ─────────────────────────
        r7_zero = client.post("/sentiment/backfill", headers=auth_headers, json={"ticker": "AAPL", "start_date": "2025-01-01", "end_date": "2025-03-31", "limit": 0})
        r7_large = client.post("/sentiment/backfill", headers=auth_headers, json={"ticker": "AAPL", "start_date": "2025-01-01", "end_date": "2025-03-31", "limit": 50000})
        report(7, "Out-of-bounds limit (< 1 or > 10000) returns 400 Bad Request", r7_zero.status_code == 400 and r7_large.status_code == 400)

        # ── Phase 8: Valid backfill with overwrite=false returns 200 OK ──────
        r8 = client.post(
            "/sentiment/backfill",
            headers=auth_headers,
            json={"ticker": "AAPL", "start_date": "2026-01-01", "end_date": "2026-12-31", "limit": 500, "overwrite": False},
        )
        data8 = r8.json()
        p8_ok = (
            r8.status_code == 200
            and data8.get("ticker") == "AAPL"
            and data8.get("start_date") == "2026-01-01"
            and data8.get("end_date") == "2026-12-31"
            and data8.get("overwrite") is False
            and isinstance(data8.get("total_articles_found"), int)
            and isinstance(data8.get("processed_articles"), int)
            and data8.get("failed_articles") == 0
            and bool(data8.get("generated_at"))
        )
        report(8, "Valid backfill with overwrite=false returns 200 OK", p8_ok)

        # ── Phase 9: Valid backfill with overwrite=true returns 200 OK ───────
        r9 = client.post(
            "/sentiment/backfill",
            headers=auth_headers,
            json={"ticker": "AAPL", "start_date": "2026-01-01", "end_date": "2026-12-31", "limit": 500, "overwrite": True},
        )
        data9 = r9.json()
        p9_ok = (
            r9.status_code == 200
            and data9.get("ticker") == "AAPL"
            and data9.get("overwrite") is True
            and data9.get("processed_articles", 0) >= data8.get("processed_articles", 0)
        )
        report(9, "Valid backfill with overwrite=true returns 200 OK and reprocesses articles", p9_ok)

        # ── Phase 10: Verification of counts and generated_at timestamp ──────
        found = data9.get("total_articles_found", 0)
        proc = data9.get("processed_articles", 0)
        failed_cnt = data9.get("failed_articles", -1)
        p10_ok = found >= 0 and proc >= 0 and failed_cnt == 0
        report(10, f"Found={found}, Processed={proc}, Failed={failed_cnt} counts verified", p10_ok)

        # ── Phase 11: Non-matching ticker returns 200 with 0 processed ───────
        r11 = client.post(
            "/sentiment/backfill",
            headers=auth_headers,
            json={"ticker": "NONEXIST", "start_date": "2020-01-01", "end_date": "2020-01-02"},
        )
        data11 = r11.json()
        p11_ok = (
            r11.status_code == 200
            and data11.get("total_articles_found") == 0
            and data11.get("processed_articles") == 0
            and data11.get("failed_articles") == 0
        )
        report(11, "Non-matching ticker returns 200 OK with 0 found & 0 processed", p11_ok)

        # ── Phase 12: Python SDK sync & async client parity ──────────────────
        from fintext import FinTextClient, FinTextAsyncClient, BackfillSentimentResponse

        sync_sdk = FinTextClient(base_url=BASE_URL, api_token=token)
        sync_res = sync_sdk.backfill_sentiment("MSFT", start_date="2026-01-01", end_date="2026-12-31", limit=100, overwrite=True)

        async def run_async_sdk():
            async_sdk = FinTextAsyncClient(base_url=BASE_URL, api_token=token)
            res = await async_sdk.backfill_sentiment("NVDA", start_date="2026-01-01", end_date="2026-12-31", limit=100, overwrite=True)
            await async_sdk.close()
            return res

        async_res = asyncio.run(run_async_sdk())

        p12_ok = (
            isinstance(sync_res, BackfillSentimentResponse)
            and sync_res.ticker == "MSFT"
            and sync_res.overwrite is True
            and isinstance(async_res, BackfillSentimentResponse)
            and async_res.ticker == "NVDA"
            and async_res.overwrite is True
        )
        report(12, "Python SDK sync and async client parity verified", p12_ok)

    print("\n═══════════════════════════════════════════════════════════════════════════════")
    print(f" Certification Results: {passed}/{total} Passed ({(passed/total)*100:.1f}%)")
    print("═══════════════════════════════════════════════════════════════════════════════\n")

    if failed > 0:
        sys.exit(1)


if __name__ == "__main__":
    main()
