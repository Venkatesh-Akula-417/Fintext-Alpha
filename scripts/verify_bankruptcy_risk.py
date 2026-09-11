#!/usr/bin/env python3
"""
===============================================================================
FinText-Alpha-Vectorizer — Suite #220: Bankruptcy Risk Signals Tool Certification
===============================================================================
Verifies:
  1.  Unauthenticated GET /risk/bankruptcy returns 401 Unauthorized
  2.  Missing required parameter (ticker) returns 400 Bad Request
  3.  Invalid ticker format returns 400 Bad Request
  4.  Lookback days out of bounds (> 90) returns 400 Bad Request
  5.  Default 30-day bankruptcy risk calculation (AAPL -> 200 OK)
  6.  Detailed 6-pillar distress components breakdown (include_components=true)
  7.  Component omission option (include_components=false -> components is None)
  8.  Custom lookback window (lookback_days=60 -> 200 OK)
  9.  Distressed company critical risk classification (BBBY -> CRITICAL >= 70)
  10. Multi-ticker cross-sectional distress comparison (MSFT, NVDA, TSLA -> 200 OK)
  11. Python SDK sync client integration (bankruptcy_risk -> 200 OK)
  12. Python SDK async client integration (bankruptcy_risk -> 200 OK)
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

PORT = 8120
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


def get_auth_token(user_id: str = "distressed_debt_analyst_01", role: str = "institutional") -> str:
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
    print("  FinText-Alpha-Vectorizer — Suite #220: Bankruptcy Risk Signals Certification")
    print("=" * 79)

    with ServerContext():
        token = get_auth_token("credit_risk_officer_01", role="institutional")
        headers = {"Authorization": f"Bearer {token}"}

        # ── Phase 1: Unauthenticated rejection ────────────────────────────────
        try:
            r = httpx.get(
                f"{BASE_URL}/risk/bankruptcy?ticker=AAPL",
                timeout=5.0,
            )
            ok = r.status_code == 401
            report(1, "Unauthenticated GET /risk/bankruptcy returns 401 Unauthorized", ok, f"status={r.status_code}")
        except Exception as e:
            report(1, "Unauthenticated GET /risk/bankruptcy returns 401 Unauthorized", False, str(e))

        # ── Phase 2: Missing required parameter (ticker) ──────────────────────
        try:
            r = httpx.get(
                f"{BASE_URL}/risk/bankruptcy",
                headers=headers,
                timeout=5.0,
            )
            ok = r.status_code == 400
            report(2, "Missing required parameter (ticker) returns 400 Bad Request", ok, f"status={r.status_code}")
        except Exception as e:
            report(2, "Missing required parameter (ticker) returns 400 Bad Request", False, str(e))

        # ── Phase 3: Invalid ticker format ────────────────────────────────────
        try:
            r = httpx.get(
                f"{BASE_URL}/risk/bankruptcy?ticker=INVALID$%^",
                headers=headers,
                timeout=5.0,
            )
            ok = r.status_code == 400
            report(3, "Invalid ticker format returns 400 Bad Request", ok, f"status={r.status_code}")
        except Exception as e:
            report(3, "Invalid ticker format returns 400 Bad Request", False, str(e))

        # ── Phase 4: Lookback days out of bounds ───────────────────────────────
        try:
            r = httpx.get(
                f"{BASE_URL}/risk/bankruptcy?ticker=AAPL&lookback_days=150",
                headers=headers,
                timeout=5.0,
            )
            ok = r.status_code == 400
            report(4, "Lookback days out of bounds (> 90) returns 400 Bad Request", ok, f"status={r.status_code}")
        except Exception as e:
            report(4, "Lookback days out of bounds (> 90) returns 400 Bad Request", False, str(e))

        # ── Phase 5: Default 30-day bankruptcy risk calculation ────────────────
        try:
            r = httpx.get(
                f"{BASE_URL}/risk/bankruptcy?ticker=AAPL",
                headers=headers,
                timeout=5.0,
            )
            ok = r.status_code == 200
            data = r.json()
            ok = ok and data.get("ticker") == "AAPL"
            ok = ok and data.get("lookback_days") == 30
            score = data.get("bankruptcy_risk_score", -1.0)
            category = data.get("risk_category")
            ok = ok and 0.0 <= score <= 100.0
            ok = ok and category in ["LOW", "MODERATE", "HIGH", "CRITICAL"]
            report(5, f"Default 30-day bankruptcy risk calculation (AAPL -> 200 OK, Score={score:.2f}, Category={category})", ok)
        except Exception as e:
            report(5, "Default 30-day bankruptcy risk calculation", False, str(e))

        # ── Phase 6: Detailed 6-pillar distress components breakdown ───────────
        try:
            r = httpx.get(
                f"{BASE_URL}/risk/bankruptcy?ticker=AAPL&include_components=true",
                headers=headers,
                timeout=5.0,
            )
            ok = r.status_code == 200
            data = r.json()
            comp = data.get("components")
            ok = ok and isinstance(comp, dict)
            s_8k = comp.get("eight_k_distress_score", -1.0)
            s_sent = comp.get("sentiment_deterioration_score", -1.0)
            s_pcr = comp.get("put_call_ratio_score", -1.0)
            s_iv = comp.get("implied_volatility_score", -1.0)
            s_sc = comp.get("supply_chain_risk_score", -1.0)
            s_insider = comp.get("insider_selling_score", -1.0)

            ok = ok and 0.0 <= s_8k <= 40.0
            ok = ok and 0.0 <= s_sent <= 20.0
            ok = ok and 0.0 <= s_pcr <= 15.0
            ok = ok and 0.0 <= s_iv <= 15.0
            ok = ok and 0.0 <= s_sc <= 10.0
            ok = ok and 0.0 <= s_insider <= 10.0

            total_comp = s_8k + s_sent + s_pcr + s_iv + s_sc + s_insider
            actual_score = data.get("bankruptcy_risk_score", -99.0)
            ok = ok and abs(total_comp - actual_score) < 0.1
            report(6, "Detailed 6-pillar distress components breakdown verified", ok)
        except Exception as e:
            report(6, "Detailed 6-pillar distress components breakdown", False, str(e))

        # ── Phase 7: Component omission option ────────────────────────────────
        try:
            r = httpx.get(
                f"{BASE_URL}/risk/bankruptcy?ticker=AAPL&include_components=false",
                headers=headers,
                timeout=5.0,
            )
            ok = r.status_code == 200
            data = r.json()
            ok = ok and data.get("components") is None
            report(7, "Component omission option (include_components=false -> components is None)", ok)
        except Exception as e:
            report(7, "Component omission option", False, str(e))

        # ── Phase 8: Custom lookback window ───────────────────────────────────
        try:
            r = httpx.get(
                f"{BASE_URL}/risk/bankruptcy?ticker=AAPL&lookback_days=60",
                headers=headers,
                timeout=5.0,
            )
            ok = r.status_code == 200
            data = r.json()
            ok = ok and data.get("lookback_days") == 60
            report(8, "Custom lookback window (lookback_days=60 -> 200 OK)", ok)
        except Exception as e:
            report(8, "Custom lookback window", False, str(e))

        # ── Phase 9: Distressed company critical risk classification ──────────
        try:
            r = httpx.get(
                f"{BASE_URL}/risk/bankruptcy?ticker=BBBY",
                headers=headers,
                timeout=5.0,
            )
            ok = r.status_code == 200
            data = r.json()
            score = data.get("bankruptcy_risk_score", 0.0)
            cat = data.get("risk_category")
            ok = ok and score >= 70.0 and cat == "CRITICAL"
            report(9, f"Distressed company critical risk classification (BBBY -> Score={score:.2f}, Category={cat})", ok)
        except Exception as e:
            report(9, "Distressed company critical risk classification", False, str(e))

        # ── Phase 10: Multi-ticker cross-sectional distress comparison ────────
        try:
            r_msft = httpx.get(f"{BASE_URL}/risk/bankruptcy?ticker=MSFT", headers=headers, timeout=5.0)
            r_nvda = httpx.get(f"{BASE_URL}/risk/bankruptcy?ticker=NVDA", headers=headers, timeout=5.0)
            r_tsla = httpx.get(f"{BASE_URL}/risk/bankruptcy?ticker=TSLA", headers=headers, timeout=5.0)

            ok = r_msft.status_code == 200 and r_nvda.status_code == 200 and r_tsla.status_code == 200
            ok = ok and r_msft.json()["ticker"] == "MSFT"
            ok = ok and r_nvda.json()["ticker"] == "NVDA"
            ok = ok and r_tsla.json()["ticker"] == "TSLA"
            report(10, "Multi-ticker cross-sectional distress comparison (MSFT, NVDA, TSLA -> 200 OK)", ok)
        except Exception as e:
            report(10, "Multi-ticker cross-sectional distress comparison", False, str(e))

        # ── Phase 11: Python SDK sync client integration ──────────────────────
        try:
            from fintext import FinTextClient

            token_sdk = get_auth_token("sdk_sync_distress_user", role="institutional")
            with FinTextClient(base_url=BASE_URL, api_token=token_sdk) as client:
                res = client.bankruptcy_risk("AAPL", lookback_days=30, include_components=True)
                ok = res.ticker == "AAPL"
                ok = ok and 0.0 <= res.bankruptcy_risk_score <= 100.0
                ok = ok and res.components is not None
                ok = ok and res.components.eight_k_distress_score >= 0.0
            report(11, "Python SDK sync client integration (bankruptcy_risk -> 200 OK)", ok)
        except Exception as e:
            report(11, "Python SDK sync client integration (bankruptcy_risk -> 200 OK)", False, str(e))

        # ── Phase 12: Python SDK async client integration ─────────────────────
        async def verify_async():
            from fintext import FinTextAsyncClient

            token_async = get_auth_token("sdk_async_distress_user", role="institutional")
            async with FinTextAsyncClient(base_url=BASE_URL, api_token=token_async) as client:
                res = await client.bankruptcy_risk("BBBY", lookback_days=45, include_components=True)
                ok = res.ticker == "BBBY"
                ok = ok and res.bankruptcy_risk_score >= 70.0
                ok = ok and res.risk_category == "CRITICAL"
                return ok

        try:
            ok_async = asyncio.run(verify_async())
            report(12, "Python SDK async client integration (bankruptcy_risk -> 200 OK)", ok_async)
        except Exception as e:
            report(12, "Python SDK async client integration (bankruptcy_risk -> 200 OK)", False, str(e))

    # ── Summary ──────────────────────────────────────────────────────────────
    print("\n" + "=" * 79)
    pct = (passed / total) * 100.0
    print(f"  Suite #220 Results: {passed}/{total} Passed ({pct:.1f}%)")
    print("=" * 79)
    if failed == 0:
        print("\n🎉 ALL 12 PHASES PASSED CLEANLY!\n")
    else:
        print(f"\n❌ {failed} PHASES FAILED!\n")
        sys.exit(1)


if __name__ == "__main__":
    main()
