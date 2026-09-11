#!/usr/bin/env python3
"""
===============================================================================
FinText-Alpha-Vectorizer — Suite #228: Credit Default Sentiment Certification
===============================================================================
Verifies:
  1.  Unauthenticated GET /risk/credit-sentiment returns 401 Unauthorized
  2.  Missing required 'ticker' parameter returns 400 Bad Request
  3.  Empty ticker string returns 400 Bad Request
  4.  Invalid ticker format returns 400 Bad Request
  5.  Out-of-bounds lookback_days (0 or > 90) returns 400 Bad Request
  6.  Default lookback_days (30) calculation returns 200 OK
  7.  Custom lookback_days (60) calculation returns 200 OK
  8.  Composite credit_sentiment_score clamped within [-1.0, 1.0]
  9.  News_sentiment_avg clamped within [-1.0, 1.0]
  10. 8k_distress_count non-negative integer, PCR and IV valid positive metrics
  11. Distressed ticker (BBBY) produces elevated distress vs healthy ticker (AAPL)
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

PORT = 8128
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


def get_jwt_token(client: httpx.Client, user_id: str = "credit_analyst") -> str:
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
    print(" FinText-Alpha-Vectorizer — Suite #228: Credit Default Sentiment Engine")
    print("═══════════════════════════════════════════════════════════════════════════════\n")

    with ServerContext():
        client = httpx.Client(base_url=BASE_URL, timeout=10.0)
        token = get_jwt_token(client)
        auth_headers = {"Authorization": f"Bearer {token}"}

        # ── Phase 1: Unauthenticated request returns 401 ─────────────────────
        r1 = client.get("/risk/credit-sentiment?ticker=AAPL")
        report(1, "Unauthenticated GET /risk/credit-sentiment returns 401", r1.status_code == 401)

        # ── Phase 2: Missing ticker parameter returns 400 ────────────────────
        r2 = client.get("/risk/credit-sentiment", headers=auth_headers)
        report(2, "Missing 'ticker' parameter returns 400 Bad Request", r2.status_code == 400)

        # ── Phase 3: Empty ticker string returns 400 ─────────────────────────
        r3 = client.get("/risk/credit-sentiment?ticker=", headers=auth_headers)
        report(3, "Empty ticker parameter returns 400 Bad Request", r3.status_code == 400)

        # ── Phase 4: Invalid ticker format returns 400 ───────────────────────
        r4 = client.get("/risk/credit-sentiment?ticker=INVALID_TOOLONG_TICKER123", headers=auth_headers)
        report(4, "Invalid ticker format returns 400 Bad Request", r4.status_code == 400)

        # ── Phase 5: Out-of-bounds lookback_days returns 400 ─────────────────
        r5_zero = client.get("/risk/credit-sentiment?ticker=AAPL&lookback_days=0", headers=auth_headers)
        r5_large = client.get("/risk/credit-sentiment?ticker=AAPL&lookback_days=180", headers=auth_headers)
        report(5, "Out-of-bounds lookback_days (< 1 or > 90) returns 400 Bad Request", r5_zero.status_code == 400 and r5_large.status_code == 400)

        # ── Phase 6: Default lookback_days calculation returns 200 OK ────────
        r6 = client.get("/risk/credit-sentiment?ticker=AAPL", headers=auth_headers)
        data6 = r6.json()
        p6_ok = (
            r6.status_code == 200
            and data6.get("ticker") == "AAPL"
            and data6.get("lookback_days") == 30
            and isinstance(data6.get("credit_sentiment_score"), (int, float))
            and isinstance(data6.get("news_sentiment_avg"), (int, float))
            and isinstance(data6.get("eight_k_distress_count"), int)
            and isinstance(data6.get("put_call_ratio"), (int, float))
            and isinstance(data6.get("implied_volatility"), (int, float))
            and bool(data6.get("generated_at"))
        )
        report(6, "Default lookback_days (30) calculation returns 200 OK", p6_ok)

        # ── Phase 7: Custom lookback_days (60) returns 200 OK ────────────────
        r7 = client.get("/risk/credit-sentiment?ticker=MSFT&lookback_days=60", headers=auth_headers)
        data7 = r7.json()
        p7_ok = (
            r7.status_code == 200
            and data7.get("ticker") == "MSFT"
            and data7.get("lookback_days") == 60
        )
        report(7, "Custom lookback_days (60) returns 200 OK", p7_ok)

        # ── Phase 8: Composite credit_sentiment_score bounds [-1.0, 1.0] ─────
        score = data6.get("credit_sentiment_score", 0.0)
        report(8, f"Composite credit_sentiment_score ({score:+.4f}) clamped in [-1.0, 1.0]", -1.0 <= score <= 1.0)

        # ── Phase 9: News_sentiment_avg bounds [-1.0, 1.0] ───────────────────
        news_avg = data6.get("news_sentiment_avg", 0.0)
        report(9, f"News sentiment average ({news_avg:+.4f}) clamped in [-1.0, 1.0]", -1.0 <= news_avg <= 1.0)

        # ── Phase 10: 8-K distress count, PCR, and IV validation ─────────────
        k_distress = data6.get("eight_k_distress_count", -1)
        pcr = data6.get("put_call_ratio", 0.0)
        iv = data6.get("implied_volatility", 0.0)
        p10_ok = k_distress >= 0 and pcr > 0.0 and iv > 0.0
        report(10, f"8-K distress count ({k_distress}), PCR ({pcr:.2f}), IV ({iv:.2f}) valid", p10_ok)

        # ── Phase 11: Distressed vs. investment grade ticker comparative ──────
        r11_dist = client.get("/risk/credit-sentiment?ticker=BBBY", headers=auth_headers)
        data11 = r11_dist.json()
        dist_score = data11.get("credit_sentiment_score", 0.0)
        p11_ok = (
            r11_dist.status_code == 200
            and dist_score < score
            and data11.get("eight_k_distress_count", 0) > 0
            and data11.get("put_call_ratio", 0.0) > 1.5
        )
        report(11, f"Distressed ticker BBBY (score {dist_score:+.4f}) shows severe distress vs AAPL ({score:+.4f})", p11_ok)

        # ── Phase 12: Python SDK sync & async client parity ──────────────────
        from fintext import FinTextClient, FinTextAsyncClient, CreditSentimentResponse

        sync_sdk = FinTextClient(base_url=BASE_URL, api_token=token)
        sync_res = sync_sdk.credit_sentiment("NVDA", lookback_days=45)

        async def run_async_sdk():
            async_sdk = FinTextAsyncClient(base_url=BASE_URL, api_token=token)
            res = await async_sdk.credit_sentiment("GOOGL", lookback_days=30)
            await async_sdk.close()
            return res

        async_res = asyncio.run(run_async_sdk())

        p12_ok = (
            isinstance(sync_res, CreditSentimentResponse)
            and sync_res.ticker == "NVDA"
            and sync_res.lookback_days == 45
            and isinstance(async_res, CreditSentimentResponse)
            and async_res.ticker == "GOOGL"
            and async_res.lookback_days == 30
        )
        report(12, "Python SDK sync and async client parity verified", p12_ok)

    print("\n═══════════════════════════════════════════════════════════════════════════════")
    print(f" Certification Results: {passed}/{total} Passed ({(passed/total)*100:.1f}%)")
    print("═══════════════════════════════════════════════════════════════════════════════\n")

    if failed > 0:
        sys.exit(1)


if __name__ == "__main__":
    main()
