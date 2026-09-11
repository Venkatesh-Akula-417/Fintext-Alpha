#!/usr/bin/env python3
"""
===============================================================================
FinText-Alpha-Vectorizer — Suite #218: Factor Exposure Report Tool Certification
===============================================================================
Verifies:
  1.  Unauthenticated GET /risk/factor-exposure returns 401 Unauthorized
  2.  Missing required parameter (ticker) returns 400 Bad Request
  3.  Invalid date ordering (start_date > end_date) returns 400 Bad Request
  4.  Unsupported factor name returns 400 Bad Request
  5.  Default 4-factor exposure computation (POST/GET -> 200 OK)
  6.  Custom factors subset computation (market, sentiment -> 200 OK)
  7.  Custom benchmark ticker configuration (benchmark_ticker=QQQ -> 200 OK)
  8.  OLS statistical summary metrics validation (R^2 in [0,1], N >= 10, F >= 0)
  9.  Single-factor regression (market only -> 200 OK)
  10. Multi-ticker factor exposures (NVDA, MSFT -> 200 OK)
  11. Python SDK sync client integration (factor_exposure -> 200 OK)
  12. Python SDK async client integration (factor_exposure -> 200 OK)
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

PORT = 8118
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


def get_auth_token(user_id: str = "quant_risk_manager_01", role: str = "institutional") -> str:
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
    print("  FinText-Alpha-Vectorizer — Suite #218: Factor Exposure Report Certification")
    print("=" * 79)

    with ServerContext():
        token = get_auth_token("quant_risk_officer_alpha", role="institutional")
        headers = {"Authorization": f"Bearer {token}"}

        # ── Phase 1: Unauthenticated rejection ────────────────────────────────
        try:
            r = httpx.get(
                f"{BASE_URL}/risk/factor-exposure?ticker=AAPL&start_date=2025-01-01&end_date=2025-06-30",
                timeout=5.0,
            )
            ok = r.status_code == 401
            report(1, "Unauthenticated GET /risk/factor-exposure returns 401 Unauthorized", ok, f"status={r.status_code}")
        except Exception as e:
            report(1, "Unauthenticated GET /risk/factor-exposure returns 401 Unauthorized", False, str(e))

        # ── Phase 2: Missing required parameter (ticker) ──────────────────────
        try:
            r = httpx.get(
                f"{BASE_URL}/risk/factor-exposure?start_date=2025-01-01&end_date=2025-06-30",
                headers=headers,
                timeout=5.0,
            )
            ok = r.status_code == 400
            report(2, "Missing required parameter (ticker) returns 400 Bad Request", ok, f"status={r.status_code}")
        except Exception as e:
            report(2, "Missing required parameter (ticker) returns 400 Bad Request", False, str(e))

        # ── Phase 3: Invalid date ordering ────────────────────────────────────
        try:
            r = httpx.get(
                f"{BASE_URL}/risk/factor-exposure?ticker=AAPL&start_date=2025-06-30&end_date=2025-01-01",
                headers=headers,
                timeout=5.0,
            )
            ok = r.status_code == 400
            report(3, "Invalid date ordering (start_date > end_date) returns 400 Bad Request", ok, f"status={r.status_code}")
        except Exception as e:
            report(3, "Invalid date ordering (start_date > end_date) returns 400 Bad Request", False, str(e))

        # ── Phase 4: Unsupported factor name ──────────────────────────────────
        try:
            r = httpx.get(
                f"{BASE_URL}/risk/factor-exposure?ticker=AAPL&start_date=2025-01-01&end_date=2025-06-30&factors=market,unknown_factor",
                headers=headers,
                timeout=5.0,
            )
            ok = r.status_code == 400
            report(4, "Unsupported factor name returns 400 Bad Request", ok, f"status={r.status_code}")
        except Exception as e:
            report(4, "Unsupported factor name returns 400 Bad Request", False, str(e))

        # ── Phase 5: Default 4-factor exposure computation ────────────────────
        try:
            r = httpx.get(
                f"{BASE_URL}/risk/factor-exposure?ticker=AAPL&start_date=2025-01-01&end_date=2025-06-30",
                headers=headers,
                timeout=5.0,
            )
            ok = r.status_code == 200
            data = r.json()
            ok = ok and data.get("ticker") == "AAPL"
            ok = ok and data.get("benchmark_ticker") == "SPY"
            factors = data.get("factors_included", [])
            ok = ok and len(factors) == 4
            exposures = data.get("exposures", [])
            ok = ok and len(exposures) == 4
            report(5, "Default 4-factor exposure computation (AAPL -> 200 OK)", ok, f"factors={factors}")
        except Exception as e:
            report(5, "Default 4-factor exposure computation (AAPL -> 200 OK)", False, str(e))

        # ── Phase 6: Custom factors subset computation ────────────────────────
        try:
            r = httpx.get(
                f"{BASE_URL}/risk/factor-exposure?ticker=AAPL&start_date=2025-01-01&end_date=2025-06-30&factors=market,sentiment",
                headers=headers,
                timeout=5.0,
            )
            ok = r.status_code == 200
            data = r.json()
            factors = data.get("factors_included", [])
            exposures = data.get("exposures", [])
            ok = ok and factors == ["market", "sentiment"]
            ok = ok and len(exposures) == 2
            report(6, "Custom factors subset computation (market, sentiment -> 200 OK)", ok, f"factors={factors}")
        except Exception as e:
            report(6, "Custom factors subset computation (market, sentiment -> 200 OK)", False, str(e))

        # ── Phase 7: Custom benchmark ticker configuration ────────────────────
        try:
            r = httpx.get(
                f"{BASE_URL}/risk/factor-exposure?ticker=AAPL&start_date=2025-01-01&end_date=2025-06-30&benchmark_ticker=QQQ",
                headers=headers,
                timeout=5.0,
            )
            ok = r.status_code == 200
            data = r.json()
            ok = ok and data.get("benchmark_ticker") == "QQQ"
            report(7, "Custom benchmark ticker configuration (benchmark_ticker=QQQ -> 200 OK)", ok, f"benchmark={data.get('benchmark_ticker')}")
        except Exception as e:
            report(7, "Custom benchmark ticker configuration (benchmark_ticker=QQQ -> 200 OK)", False, str(e))

        # ── Phase 8: OLS statistical summary metrics validation ───────────────
        try:
            r = httpx.get(
                f"{BASE_URL}/risk/factor-exposure?ticker=AAPL&start_date=2025-01-01&end_date=2025-06-30",
                headers=headers,
                timeout=5.0,
            )
            ok = r.status_code == 200
            data = r.json()
            ols = data.get("ols_summary", {})
            r2 = ols.get("r_squared", -1.0)
            adj_r2 = ols.get("adjusted_r_squared", -1.0)
            n_obs = ols.get("num_observations", 0)
            f_stat = ols.get("f_statistic", -1.0)

            ok = ok and 0.0 <= r2 <= 1.0
            ok = ok and adj_r2 <= r2
            ok = ok and n_obs >= 10
            ok = ok and f_stat >= 0.0
            report(8, f"OLS statistical summary metrics validation (R^2={r2:.4f}, N={n_obs}, F={f_stat:.2f})", ok)
        except Exception as e:
            report(8, "OLS statistical summary metrics validation", False, str(e))

        # ── Phase 9: Single-factor regression ─────────────────────────────────
        try:
            r = httpx.get(
                f"{BASE_URL}/risk/factor-exposure?ticker=AAPL&start_date=2025-01-01&end_date=2025-06-30&factors=market",
                headers=headers,
                timeout=5.0,
            )
            ok = r.status_code == 200
            data = r.json()
            exposures = data.get("exposures", [])
            ok = ok and len(exposures) == 1
            ok = ok and exposures[0]["factor"] == "market"
            beta = exposures[0]["beta"]
            report(9, f"Single-factor regression (market only -> Beta={beta:.4f})", ok)
        except Exception as e:
            report(9, "Single-factor regression (market only)", False, str(e))

        # ── Phase 10: Multi-ticker factor exposures (NVDA, MSFT) ──────────────
        try:
            r_nvda = httpx.get(
                f"{BASE_URL}/risk/factor-exposure?ticker=NVDA&start_date=2025-01-01&end_date=2025-06-30",
                headers=headers,
                timeout=5.0,
            )
            r_msft = httpx.get(
                f"{BASE_URL}/risk/factor-exposure?ticker=MSFT&start_date=2025-01-01&end_date=2025-06-30",
                headers=headers,
                timeout=5.0,
            )
            ok = r_nvda.status_code == 200 and r_msft.status_code == 200
            d_nvda = r_nvda.json()
            d_msft = r_msft.json()
            ok = ok and d_nvda.get("ticker") == "NVDA" and d_msft.get("ticker") == "MSFT"
            report(10, "Multi-ticker factor exposures (NVDA, MSFT -> 200 OK)", ok)
        except Exception as e:
            report(10, "Multi-ticker factor exposures (NVDA, MSFT -> 200 OK)", False, str(e))

        # ── Phase 11: Python SDK sync client integration ──────────────────────
        try:
            from fintext import FinTextClient

            token_sdk = get_auth_token("sdk_sync_factor_user", role="institutional")
            with FinTextClient(base_url=BASE_URL, api_token=token_sdk) as client:
                res = client.factor_exposure(
                    ticker="AAPL",
                    start_date="2025-01-01",
                    end_date="2025-06-30",
                    factors=["market", "momentum", "sentiment", "volatility"],
                    benchmark_ticker="SPY",
                )
                ok = res.ticker == "AAPL"
                ok = ok and res.benchmark_ticker == "SPY"
                ok = ok and len(res.exposures) == 4
                ok = ok and res.ols_summary.num_observations >= 10
            report(11, "Python SDK sync client integration (factor_exposure -> 200 OK)", ok)
        except Exception as e:
            report(11, "Python SDK sync client integration (factor_exposure -> 200 OK)", False, str(e))

        # ── Phase 12: Python SDK async client integration ─────────────────────
        async def verify_async():
            from fintext import FinTextAsyncClient

            token_async = get_auth_token("sdk_async_factor_user", role="institutional")
            async with FinTextAsyncClient(base_url=BASE_URL, api_token=token_async) as client:
                res = await client.factor_exposure(
                    ticker="NVDA",
                    start_date="2025-01-01",
                    end_date="2025-06-30",
                    factors=["market", "volatility"],
                )
                ok = res.ticker == "NVDA"
                ok = ok and len(res.exposures) == 2
                ok = ok and res.ols_summary.num_observations >= 10
                return ok

        try:
            ok_async = asyncio.run(verify_async())
            report(12, "Python SDK async client integration (factor_exposure -> 200 OK)", ok_async)
        except Exception as e:
            report(12, "Python SDK async client integration (factor_exposure -> 200 OK)", False, str(e))

    # ── Summary ──────────────────────────────────────────────────────────────
    print("\n" + "=" * 79)
    pct = (passed / total) * 100.0
    print(f"  Suite #218 Results: {passed}/{total} Passed ({pct:.1f}%)")
    print("=" * 79)
    if failed == 0:
        print("\n🎉 ALL 12 PHASES PASSED CLEANLY!\n")
    else:
        print(f"\n❌ {failed} PHASES FAILED!\n")
        sys.exit(1)


if __name__ == "__main__":
    main()
