#!/usr/bin/env python3
"""
===============================================================================
FinText-Alpha-Vectorizer — Suite #222: Commodity News Sentiment & Raw Material Analytics Certification
===============================================================================
Verifies:
  1.  Unauthenticated GET /commodities/sentiment returns 401 Unauthorized
  2.  Invalid commodity (uranium) returns 400 Bad Request
  3.  Invalid date range (start_date > end_date) returns 400 Bad Request
  4.  Invalid date format returns 400 Bad Request
  5.  Invalid min_confidence bounds returns 400 Bad Request
  6.  Invalid limit bounds (0 or > 50) returns 400 Bad Request
  7.  Default crude_oil sentiment query returns 200 OK with summary & articles
  8.  Cross-commodity sentiment retrieval (gold, copper, natural_gas, wheat, silver -> 200 OK)
  9.  Confidence threshold filtering (min_confidence=0.85 -> 200 OK)
  10. Pagination and limit trimming (limit=3 -> len(top_articles) <= 3)
  11. Python SDK sync client integration (commodity_sentiment -> 200 OK)
  12. Python SDK async client integration (commodity_sentiment -> 200 OK)
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

PORT = 8122
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

        print("[READY] FinText API Server is responding to health checks.\n")
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        if self.process:
            print("\n[STOPPING] Terminating FinText API Server process...")
            self.process.terminate()
            try:
                self.process.wait(timeout=5.0)
            except subprocess.TimeoutExpired:
                self.process.kill()


def get_auth_token(user_id: str = "commodity_trader_01", role: str = "institutional") -> str:
    r = httpx.post(
        f"{BASE_URL}/auth/token",
        json={"user_id": user_id, "role": role, "expires_in_seconds": 3600},
        headers={"X-Admin-Token": ADMIN_TOKEN},
        timeout=5.0,
    )
    if r.status_code != 200:
        raise RuntimeError(f"Failed to obtain auth token: {r.status_code} - {r.text}")
    return r.json()["token"]


def main():
    print("=" * 79)
    print("  FinText-Alpha-Vectorizer — Suite #222: Commodity News Sentiment Certification")
    print("=" * 79)

    with ServerContext():
        token = get_auth_token("cta_portfolio_manager_01", role="institutional")
        headers = {"Authorization": f"Bearer {token}"}

        # ── Phase 1: Unauthenticated rejection ────────────────────────────────
        try:
            r = httpx.get(
                f"{BASE_URL}/commodities/sentiment?commodity=crude_oil",
                timeout=5.0,
            )
            ok = r.status_code == 401
            report(1, "Unauthenticated GET /commodities/sentiment returns 401 Unauthorized", ok, f"status={r.status_code}")
        except Exception as e:
            report(1, "Unauthenticated GET /commodities/sentiment returns 401 Unauthorized", False, str(e))

        # ── Phase 2: Invalid commodity rejection ──────────────────────────────
        try:
            r = httpx.get(
                f"{BASE_URL}/commodities/sentiment?commodity=uranium",
                headers=headers,
                timeout=5.0,
            )
            ok = r.status_code == 400
            report(2, "Invalid commodity (uranium) returns 400 Bad Request", ok, f"status={r.status_code}")
        except Exception as e:
            report(2, "Invalid commodity (uranium) returns 400 Bad Request", False, str(e))

        # ── Phase 3: Invalid date range (start > end) ─────────────────────────
        try:
            r = httpx.get(
                f"{BASE_URL}/commodities/sentiment?commodity=crude_oil&start_date=2025-08-30&end_date=2025-08-01",
                headers=headers,
                timeout=5.0,
            )
            ok = r.status_code == 400
            report(3, "Invalid date range (start_date > end_date) returns 400 Bad Request", ok, f"status={r.status_code}")
        except Exception as e:
            report(3, "Invalid date range returns 400 Bad Request", False, str(e))

        # ── Phase 4: Invalid date format ──────────────────────────────────────
        try:
            r = httpx.get(
                f"{BASE_URL}/commodities/sentiment?commodity=crude_oil&start_date=2025/08/01",
                headers=headers,
                timeout=5.0,
            )
            ok = r.status_code == 400
            report(4, "Invalid date format returns 400 Bad Request", ok, f"status={r.status_code}")
        except Exception as e:
            report(4, "Invalid date format returns 400 Bad Request", False, str(e))

        # ── Phase 5: Invalid min_confidence bounds ────────────────────────────
        try:
            r = httpx.get(
                f"{BASE_URL}/commodities/sentiment?commodity=crude_oil&min_confidence=1.5",
                headers=headers,
                timeout=5.0,
            )
            ok = r.status_code == 400
            report(5, "Invalid min_confidence bounds (> 1.0) returns 400 Bad Request", ok, f"status={r.status_code}")
        except Exception as e:
            report(5, "Invalid min_confidence bounds returns 400 Bad Request", False, str(e))

        # ── Phase 6: Invalid limit bounds ─────────────────────────────────────
        try:
            r = httpx.get(
                f"{BASE_URL}/commodities/sentiment?commodity=crude_oil&limit=0",
                headers=headers,
                timeout=5.0,
            )
            ok_0 = r.status_code == 400
            r_100 = httpx.get(
                f"{BASE_URL}/commodities/sentiment?commodity=crude_oil&limit=100",
                headers=headers,
                timeout=5.0,
            )
            ok_100 = r_100.status_code == 400
            report(6, "Invalid limit bounds (0 or > 50) returns 400 Bad Request", ok_0 and ok_100)
        except Exception as e:
            report(6, "Invalid limit bounds returns 400 Bad Request", False, str(e))

        # ── Phase 7: Default crude_oil sentiment query ────────────────────────
        try:
            r = httpx.get(
                f"{BASE_URL}/commodities/sentiment?commodity=crude_oil",
                headers=headers,
                timeout=5.0,
            )
            ok = r.status_code == 200
            data = r.json()
            ok = ok and data.get("commodity") == "crude_oil"
            summary = data.get("summary", {})
            ok = ok and summary.get("mention_count", 0) > 0
            avg_sent = summary.get("avg_sentiment", -99.0)
            ok = ok and -1.0 <= avg_sent <= 1.0
            ok = ok and 0.0 <= summary.get("positive_ratio", -1.0) <= 1.0
            ok = ok and 0.0 <= summary.get("negative_ratio", -1.0) <= 1.0
            articles = data.get("top_articles", [])
            ok = ok and len(articles) > 0
            report(7, f"Default crude_oil sentiment query (200 OK, Mentions={summary.get('mention_count')}, AvgSent={avg_sent:.4f})", ok)
        except Exception as e:
            report(7, "Default crude_oil sentiment query", False, str(e))

        # ── Phase 8: Cross-commodity sentiment retrieval ──────────────────────
        try:
            r_gold = httpx.get(f"{BASE_URL}/commodities/sentiment?commodity=gold", headers=headers, timeout=5.0)
            r_copper = httpx.get(f"{BASE_URL}/commodities/sentiment?commodity=copper", headers=headers, timeout=5.0)
            r_gas = httpx.get(f"{BASE_URL}/commodities/sentiment?commodity=natural_gas", headers=headers, timeout=5.0)
            r_wheat = httpx.get(f"{BASE_URL}/commodities/sentiment?commodity=wheat", headers=headers, timeout=5.0)
            r_silver = httpx.get(f"{BASE_URL}/commodities/sentiment?commodity=silver", headers=headers, timeout=5.0)

            ok = (
                r_gold.status_code == 200
                and r_copper.status_code == 200
                and r_gas.status_code == 200
                and r_wheat.status_code == 200
                and r_silver.status_code == 200
            )
            ok = ok and r_gold.json()["commodity"] == "gold"
            ok = ok and r_copper.json()["commodity"] == "copper"
            ok = ok and r_gas.json()["commodity"] == "natural_gas"
            ok = ok and r_wheat.json()["commodity"] == "wheat"
            ok = ok and r_silver.json()["commodity"] == "silver"
            report(8, "Cross-commodity sentiment retrieval (gold, copper, natural_gas, wheat, silver -> 200 OK)", ok)
        except Exception as e:
            report(8, "Cross-commodity sentiment retrieval", False, str(e))

        # ── Phase 9: Confidence threshold filtering ───────────────────────────
        try:
            r = httpx.get(
                f"{BASE_URL}/commodities/sentiment?commodity=crude_oil&min_confidence=0.85",
                headers=headers,
                timeout=5.0,
            )
            ok = r.status_code == 200
            data = r.json()
            articles = data.get("top_articles", [])
            ok = ok and all(a["confidence"] >= 0.85 for a in articles)
            report(9, f"Confidence threshold filtering (min_confidence=0.85 -> {len(articles)} filtered articles)", ok)
        except Exception as e:
            report(9, "Confidence threshold filtering", False, str(e))

        # ── Phase 10: Pagination and limit trimming ───────────────────────────
        try:
            r = httpx.get(
                f"{BASE_URL}/commodities/sentiment?commodity=crude_oil&limit=3",
                headers=headers,
                timeout=5.0,
            )
            ok = r.status_code == 200
            data = r.json()
            articles = data.get("top_articles", [])
            ok = ok and len(articles) <= 3
            report(10, f"Pagination and limit trimming (limit=3 -> {len(articles)} articles returned)", ok)
        except Exception as e:
            report(10, "Pagination and limit trimming", False, str(e))

        # ── Phase 11: Python SDK sync client integration ──────────────────────
        try:
            from fintext import FinTextClient

            token_sdk = get_auth_token("sdk_sync_commodity_user", role="institutional")
            with FinTextClient(base_url=BASE_URL, api_token=token_sdk) as client:
                res = client.commodity_sentiment(
                    commodity="copper",
                    min_confidence=0.5,
                    limit=5,
                )
                ok = res.commodity == "copper"
                ok = ok and res.summary.mention_count > 0
                ok = ok and -1.0 <= res.summary.avg_sentiment <= 1.0
                ok = ok and len(res.top_articles) <= 5
            report(11, "Python SDK sync client integration (commodity_sentiment -> 200 OK)", ok)
        except Exception as e:
            report(11, "Python SDK sync client integration (commodity_sentiment -> 200 OK)", False, str(e))

        # ── Phase 12: Python SDK async client integration ─────────────────────
        async def verify_async():
            from fintext import FinTextAsyncClient

            token_async = get_auth_token("sdk_async_commodity_user", role="institutional")
            async with FinTextAsyncClient(base_url=BASE_URL, api_token=token_async) as client:
                res = await client.commodity_sentiment(
                    commodity="natural_gas",
                    min_confidence=0.6,
                    limit=5,
                )
                ok = res.commodity == "natural_gas"
                ok = ok and res.summary.mention_count > 0
                ok = ok and -1.0 <= res.summary.avg_sentiment <= 1.0
                ok = ok and len(res.top_articles) <= 5
                return ok

        try:
            ok_async = asyncio.run(verify_async())
            report(12, "Python SDK async client integration (commodity_sentiment -> 200 OK)", ok_async)
        except Exception as e:
            report(12, "Python SDK async client integration (commodity_sentiment -> 200 OK)", False, str(e))

    # ── Summary ──────────────────────────────────────────────────────────────
    print("\n" + "=" * 79)
    pct = (passed / total) * 100.0
    print(f"  Suite #222 Results: {passed}/{total} Passed ({pct:.1f}%)")
    print("=" * 79)
    if failed == 0:
        print("\n🎉 ALL 12 PHASES PASSED CLEANLY!\n")
    else:
        print(f"\n❌ {failed} PHASES FAILED!\n")
        sys.exit(1)


if __name__ == "__main__":
    main()
