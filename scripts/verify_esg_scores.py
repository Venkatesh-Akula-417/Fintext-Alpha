#!/usr/bin/env python3
"""
===============================================================================
FinText-Alpha-Vectorizer — Suite #219: ESG Sentiment Scores Tool Certification
===============================================================================
Verifies:
  1.  Unauthenticated GET /esg/scores returns 401 Unauthorized
  2.  Invalid ticker format returns 400 Bad Request
  3.  Invalid sector name returns 400 Bad Request
  4.  Invalid date ordering (start_date > end_date) returns 400 Bad Request
  5.  Invalid min_confidence range (> 1.0) returns 400 Bad Request
  6.  Ticker-level ESG score calculation (AAPL -> 200 OK)
  7.  Sector-level ESG score aggregation (Technology -> 200 OK)
  8.  Market-wide ESG score aggregation (no ticker or sector -> 200 OK)
  9.  Confidence threshold filtering (min_confidence=0.85 -> 200 OK)
  10. Pillar dimension metrics verification (E, S, G in [-1,1], positive/negative ratios)
  11. Python SDK sync client integration (esg_scores -> 200 OK)
  12. Python SDK async client integration (esg_scores -> 200 OK)
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

PORT = 8119
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


def get_auth_token(user_id: str = "esg_quant_analyst_01", role: str = "institutional") -> str:
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
    print("  FinText-Alpha-Vectorizer — Suite #219: ESG Sentiment Scores Certification")
    print("=" * 79)

    with ServerContext():
        token = get_auth_token("esg_sustainability_officer", role="institutional")
        headers = {"Authorization": f"Bearer {token}"}

        # ── Phase 1: Unauthenticated rejection ────────────────────────────────
        try:
            r = httpx.get(
                f"{BASE_URL}/esg/scores?ticker=AAPL",
                timeout=5.0,
            )
            ok = r.status_code == 401
            report(1, "Unauthenticated GET /esg/scores returns 401 Unauthorized", ok, f"status={r.status_code}")
        except Exception as e:
            report(1, "Unauthenticated GET /esg/scores returns 401 Unauthorized", False, str(e))

        # ── Phase 2: Invalid ticker format ────────────────────────────────────
        try:
            r = httpx.get(
                f"{BASE_URL}/esg/scores?ticker=INVALID$%^",
                headers=headers,
                timeout=5.0,
            )
            ok = r.status_code == 400
            report(2, "Invalid ticker format returns 400 Bad Request", ok, f"status={r.status_code}")
        except Exception as e:
            report(2, "Invalid ticker format returns 400 Bad Request", False, str(e))

        # ── Phase 3: Invalid sector name ──────────────────────────────────────
        try:
            r = httpx.get(
                f"{BASE_URL}/esg/scores?sector=NonExistentSector999",
                headers=headers,
                timeout=5.0,
            )
            ok = r.status_code == 400
            report(3, "Invalid sector name returns 400 Bad Request", ok, f"status={r.status_code}")
        except Exception as e:
            report(3, "Invalid sector name returns 400 Bad Request", False, str(e))

        # ── Phase 4: Invalid date ordering ────────────────────────────────────
        try:
            r = httpx.get(
                f"{BASE_URL}/esg/scores?ticker=AAPL&start_date=2025-08-30&end_date=2025-06-01",
                headers=headers,
                timeout=5.0,
            )
            ok = r.status_code == 400
            report(4, "Invalid date ordering (start_date > end_date) returns 400 Bad Request", ok, f"status={r.status_code}")
        except Exception as e:
            report(4, "Invalid date ordering (start_date > end_date) returns 400 Bad Request", False, str(e))

        # ── Phase 5: Invalid min_confidence range ─────────────────────────────
        try:
            r = httpx.get(
                f"{BASE_URL}/esg/scores?ticker=AAPL&min_confidence=1.5",
                headers=headers,
                timeout=5.0,
            )
            ok = r.status_code == 400
            report(5, "Invalid min_confidence range (> 1.0) returns 400 Bad Request", ok, f"status={r.status_code}")
        except Exception as e:
            report(5, "Invalid min_confidence range (> 1.0) returns 400 Bad Request", False, str(e))

        # ── Phase 6: Ticker-level ESG score calculation ───────────────────────
        try:
            r = httpx.get(
                f"{BASE_URL}/esg/scores?ticker=AAPL&start_date=2025-06-01&end_date=2025-08-30",
                headers=headers,
                timeout=5.0,
            )
            ok = r.status_code == 200
            data = r.json()
            ok = ok and data.get("ticker") == "AAPL"
            score = data.get("overall_esg_score", -1.0)
            ok = ok and 0.0 <= score <= 100.0
            report(6, f"Ticker-level ESG score calculation (AAPL -> 200 OK, Score={score:.2f})", ok)
        except Exception as e:
            report(6, "Ticker-level ESG score calculation", False, str(e))

        # ── Phase 7: Sector-level ESG score aggregation ───────────────────────
        try:
            r = httpx.get(
                f"{BASE_URL}/esg/scores?sector=Technology&start_date=2025-06-01&end_date=2025-08-30",
                headers=headers,
                timeout=5.0,
            )
            ok = r.status_code == 200
            data = r.json()
            ok = ok and data.get("sector") == "Technology"
            score = data.get("overall_esg_score", -1.0)
            ok = ok and 0.0 <= score <= 100.0
            report(7, f"Sector-level ESG score aggregation (Technology -> 200 OK, Score={score:.2f})", ok)
        except Exception as e:
            report(7, "Sector-level ESG score aggregation", False, str(e))

        # ── Phase 8: Market-wide ESG score aggregation ────────────────────────
        try:
            r = httpx.get(
                f"{BASE_URL}/esg/scores?start_date=2025-06-01&end_date=2025-08-30",
                headers=headers,
                timeout=5.0,
            )
            ok = r.status_code == 200
            data = r.json()
            ok = ok and data.get("ticker") is None
            score = data.get("overall_esg_score", -1.0)
            ok = ok and 0.0 <= score <= 100.0
            report(8, f"Market-wide ESG score aggregation (All Tickers -> 200 OK, Score={score:.2f})", ok)
        except Exception as e:
            report(8, "Market-wide ESG score aggregation", False, str(e))

        # ── Phase 9: Confidence threshold filtering ───────────────────────────
        try:
            r = httpx.get(
                f"{BASE_URL}/esg/scores?ticker=AAPL&min_confidence=0.85",
                headers=headers,
                timeout=5.0,
            )
            ok = r.status_code == 200
            data = r.json()
            ok = ok and data.get("min_confidence") == 0.85
            report(9, "Confidence threshold filtering (min_confidence=0.85 -> 200 OK)", ok)
        except Exception as e:
            report(9, "Confidence threshold filtering", False, str(e))

        # ── Phase 10: Pillar dimension metrics verification ───────────────────
        try:
            r = httpx.get(
                f"{BASE_URL}/esg/scores?ticker=AAPL&start_date=2025-06-01&end_date=2025-08-30",
                headers=headers,
                timeout=5.0,
            )
            ok = r.status_code == 200
            data = r.json()
            dims = data.get("dimensions", {})
            env = dims.get("environmental", {})
            soc = dims.get("social", {})
            gov = dims.get("governance", {})

            for d in [env, soc, gov]:
                ok = ok and -1.0 <= d.get("score", 99.0) <= 1.0
                ok = ok and d.get("mention_count", 0) > 0
                ok = ok and 0.0 <= d.get("positive_ratio", -1.0) <= 1.0
                ok = ok and 0.0 <= d.get("negative_ratio", -1.0) <= 1.0

            e_scaled = (env["score"] + 1.0) * 50.0
            s_scaled = (soc["score"] + 1.0) * 50.0
            g_scaled = (gov["score"] + 1.0) * 50.0
            expected_overall = round(0.40 * e_scaled + 0.30 * s_scaled + 0.30 * g_scaled, 2)
            actual_overall = data.get("overall_esg_score", -99.0)

            ok = ok and abs(expected_overall - actual_overall) < 0.1
            report(10, f"Pillar dimension metrics verification (E={env['score']:.2f}, S={soc['score']:.2f}, G={gov['score']:.2f} -> Overall={actual_overall:.2f})", ok)
        except Exception as e:
            report(10, "Pillar dimension metrics verification", False, str(e))

        # ── Phase 11: Python SDK sync client integration ──────────────────────
        try:
            from fintext import FinTextClient

            token_sdk = get_auth_token("sdk_sync_esg_user", role="institutional")
            with FinTextClient(base_url=BASE_URL, api_token=token_sdk) as client:
                res = client.esg_scores(
                    ticker="AAPL",
                    start_date="2025-06-01",
                    end_date="2025-08-30",
                    min_confidence=0.5,
                )
                ok = res.ticker == "AAPL"
                ok = ok and 0.0 <= res.overall_esg_score <= 100.0
                ok = ok and res.dimensions.environmental.mention_count > 0
            report(11, "Python SDK sync client integration (esg_scores -> 200 OK)", ok)
        except Exception as e:
            report(11, "Python SDK sync client integration (esg_scores -> 200 OK)", False, str(e))

        # ── Phase 12: Python SDK async client integration ─────────────────────
        async def verify_async():
            from fintext import FinTextAsyncClient

            token_async = get_auth_token("sdk_async_esg_user", role="institutional")
            async with FinTextAsyncClient(base_url=BASE_URL, api_token=token_async) as client:
                res = await client.esg_scores(
                    sector="Energy",
                    start_date="2025-06-01",
                    end_date="2025-08-30",
                )
                ok = res.sector == "Energy"
                ok = ok and 0.0 <= res.overall_esg_score <= 100.0
                ok = ok and res.dimensions.governance.mention_count > 0
                return ok

        try:
            ok_async = asyncio.run(verify_async())
            report(12, "Python SDK async client integration (esg_scores -> 200 OK)", ok_async)
        except Exception as e:
            report(12, "Python SDK async client integration (esg_scores -> 200 OK)", False, str(e))

    # ── Summary ──────────────────────────────────────────────────────────────
    print("\n" + "=" * 79)
    pct = (passed / total) * 100.0
    print(f"  Suite #219 Results: {passed}/{total} Passed ({pct:.1f}%)")
    print("=" * 79)
    if failed == 0:
        print("\n🎉 ALL 12 PHASES PASSED CLEANLY!\n")
    else:
        print(f"\n❌ {failed} PHASES FAILED!\n")
        sys.exit(1)


if __name__ == "__main__":
    main()
