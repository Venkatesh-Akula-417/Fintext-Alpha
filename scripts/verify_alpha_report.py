#!/usr/bin/env python3
"""
===============================================================================
FinText-Alpha-Vectorizer — Verification: Alpha Signal Validation Report
===============================================================================
Verifies:
  1.  POST /signals/alpha-report returns 200 OK with valid request body
  2.  Response contains correct top-level fields (tickers, dates, signal_config, initial_capital)
  3.  Performance metrics: risk attribution invariants (volatility > 0, drawdown >= 0, win_rate 0-100%)
  4.  Performance metrics: alpha = total_return - benchmark_total_return
  5.  Performance metrics: Sharpe & Sortino ratio sign consistency
  6.  Performance metrics: benchmark fields populated and consistent
  7.  Equity curve structure and daily time series coverage
  8.  Equity curve position values are within valid range (-1, 0, 1)
  9.  Validation: empty tickers returns 400 Bad Request
  10. Validation: >10 tickers returns 400 Bad Request
  11. Python SDK Sync Client integration (client.generate_alpha_report)
  12. Python SDK Async Client integration (await async_client.generate_alpha_report)
  13. OpenAPI specification verification (/signals/alpha-report path, schemas, tag)
  14. Default signal_config parameters are correctly applied
===============================================================================
"""

import asyncio
import json
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

PORT = 8146
BASE_URL = f"http://127.0.0.1:{PORT}"
ADMIN_TOKEN = "test_admin_token_xyz123_valid_32_bytes_length!"
SERVER_EXE = PROJECT_ROOT / "rust" / "target" / "debug" / "fintext_api.exe"

passed = 0
failed = 0
total = 14


def report(phase: int, name: str, ok: bool, detail: str = ""):
    global passed, failed
    if ok:
        passed += 1
        print(f"  ✅ Phase {phase:2d} │ {name}")
    else:
        failed += 1
        print(f"  ❌ Phase {phase:2d} │ {name}")
        if detail:
            print(f"     └─ {detail}")


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
        env["CONFIG_PATH"] = str(PROJECT_ROOT / "config" / "config.yaml")

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
                time.sleep(0.4)

        if not ready:
            if self.process.poll() is not None:
                raise RuntimeError(f"Server exited prematurely with return code {self.process.returncode}")
            raise TimeoutError("FinText API server failed to respond within 25 seconds.")

        print(f"[READY] API Server is healthy at {BASE_URL}")
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        if self.process and self.process.poll() is None:
            print("[CLEANUP] Terminating FinText API Server process...")
            self.process.terminate()
            try:
                self.process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait(timeout=2)


def get_auth_token(client: httpx.Client) -> str:
    """Register a test user and obtain a JWT token."""
    reg = client.post("/auth/register", json={
        "email": "alpha_test@fintext.dev",
        "password": "Alpha$ecureP@ss2024!",
    })
    if reg.status_code not in (200, 201, 409):
        raise RuntimeError(f"Failed to register: {reg.status_code} {reg.text}")
    
    login = client.post("/auth/login", json={
        "email": "alpha_test@fintext.dev",
        "password": "Alpha$ecureP@ss2024!",
    })
    if login.status_code != 200:
        # Fall back to issue_token
        token_resp = client.post("/auth/token", json={
            "admin_token": ADMIN_TOKEN,
        })
        return token_resp.json().get("token", "")
    return login.json().get("token", "")


def main():
    global passed, failed
    print("=" * 80)
    print(" FinText-Alpha-Vectorizer — Alpha Signal Validation Report Verification")
    print("=" * 80)
    print(f" Project Root: {PROJECT_ROOT}")
    print(f" Server Port:  {PORT}")

    with ServerContext():
        run_tests()

    print("=" * 80)
    print(f" Alpha Report Verification Results: {passed}/{total} Phases Passed ({failed} Failed)")
    print("=" * 80)
    return 0 if failed == 0 else 1


def run_tests():
    from fintext import FinTextClient, FinTextAsyncClient
    from fintext.models import (
        AlphaSignalConfig,
        AlphaReportResponse,
        PerformanceMetrics,
        EquityCurvePoint,
    )

    client = httpx.Client(base_url=BASE_URL, timeout=10.0)
    auth_token = get_auth_token(client)
    auth_headers = {"Authorization": f"Bearer {auth_token}"}

    alpha_payload = {
        "tickers": ["AAPL", "MSFT"],
        "start_date": "2024-01-01",
        "end_date": "2024-12-31",
        "signal_config": {
            "signal_type": "sentiment",
            "threshold_long": 0.2,
            "threshold_short": -0.2,
            "holding_days": 5,
            "smoothing_window_days": 3,
        },
        "benchmark_ticker": "SPY",
        "initial_capital": 1000000.0,
    }

    # ── Phase 1: POST /signals/alpha-report returns 200 ──────────────────────
    resp = client.post("/signals/alpha-report", json=alpha_payload, headers=auth_headers)
    report(1, "POST /signals/alpha-report returns 200 OK", resp.status_code == 200, f"Status: {resp.status_code}")
    data = resp.json()

    # ── Phase 2: Top-level fields ────────────────────────────────────────────
    expected_fields = [
        "tickers", "start_date", "end_date", "signal_config",
        "initial_capital", "metrics", "equity_curve", "generated_at", "message"
    ]
    fields_ok = all(f in data for f in expected_fields)
    tickers_ok = data.get("tickers") == ["AAPL", "MSFT"]
    dates_ok = data.get("start_date") == "2024-01-01" and data.get("end_date") == "2024-12-31"
    cap_ok = data.get("initial_capital") == 1000000.0
    top_ok = fields_ok and tickers_ok and dates_ok and cap_ok
    report(2, "Response contains correct top-level fields", top_ok,
           f"fields={fields_ok}, tickers={tickers_ok}, dates={dates_ok}, capital={cap_ok}")

    # ── Phase 3: Risk attribution invariants ─────────────────────────────────
    m = data.get("metrics", {})
    vol_ok = isinstance(m.get("annualized_volatility"), (int, float)) and m["annualized_volatility"] > 0.0
    dd_ok = isinstance(m.get("max_drawdown"), (int, float)) and m["max_drawdown"] >= 0.0
    wr_ok = isinstance(m.get("win_rate"), (int, float)) and 0.0 <= m["win_rate"] <= 100.0
    trades_ok = isinstance(m.get("total_trades"), int) and m["total_trades"] > 0
    hold_ok = isinstance(m.get("avg_holding_period_days"), (int, float)) and m["avg_holding_period_days"] > 0
    risk_ok = vol_ok and dd_ok and wr_ok and trades_ok and hold_ok
    report(3, "Performance metrics: risk attribution invariants",
           risk_ok,
           f"vol={m.get('annualized_volatility')}, dd={m.get('max_drawdown')}, "
           f"wr={m.get('win_rate')}, trades={m.get('total_trades')}, hold={m.get('avg_holding_period_days')}")

    # ── Phase 4: Alpha = total_return - benchmark_total_return ────────────────
    total_ret = m.get("total_return", 0.0)
    bench_ret = m.get("benchmark_total_return", 0.0)
    reported_alpha = m.get("alpha", 0.0)
    expected_alpha = total_ret - bench_ret
    alpha_ok = abs(reported_alpha - expected_alpha) < 1e-6
    report(4, "Alpha = total_return − benchmark_total_return", alpha_ok,
           f"alpha={reported_alpha:.6f}, expected={expected_alpha:.6f}")

    # ── Phase 5: Sharpe & Sortino ratio sign consistency ─────────────────────
    sharpe = m.get("sharpe_ratio", 0.0)
    sortino = m.get("sortino_ratio", 0.0)
    ann_ret = m.get("annualized_return", 0.0)
    # If annualized return is positive, Sharpe and Sortino should be positive (and vice versa)
    sign_ok = (ann_ret > 0 and sharpe > 0 and sortino > 0) or \
              (ann_ret < 0 and sharpe < 0) or \
              (ann_ret == 0.0)
    report(5, "Sharpe & Sortino ratio sign consistency with returns", sign_ok,
           f"annualized_return={ann_ret:.4f}, sharpe={sharpe:.4f}, sortino={sortino:.4f}")

    # ── Phase 6: Benchmark fields populated ──────────────────────────────────
    bench_ticker = m.get("benchmark_ticker", "")
    bench_ann = m.get("benchmark_annualized_return")
    bench_vol = m.get("benchmark_annualized_volatility")
    beta = m.get("beta")
    ir = m.get("information_ratio")
    te = m.get("tracking_error")
    bench_ok = (
        bench_ticker == "SPY" and
        isinstance(bench_ann, (int, float)) and
        isinstance(bench_vol, (int, float)) and bench_vol > 0.0 and
        isinstance(beta, (int, float)) and
        isinstance(ir, (int, float)) and
        isinstance(te, (int, float)) and te > 0.0
    )
    report(6, "Benchmark fields populated and consistent", bench_ok,
           f"ticker={bench_ticker}, beta={beta}, IR={ir}, TE={te}")

    # ── Phase 7: Equity curve structure ──────────────────────────────────────
    ec = data.get("equity_curve", [])
    ec_nonempty = len(ec) > 0
    ec_fields = all(
        all(k in pt for k in ["date", "portfolio_value", "benchmark_value",
                               "strategy_daily_return", "benchmark_daily_return", "position"])
        for pt in ec
    ) if ec_nonempty else False
    ec_dates_sorted = all(ec[i]["date"] <= ec[i + 1]["date"] for i in range(len(ec) - 1)) if len(ec) > 1 else True
    ec_ok = ec_nonempty and ec_fields and ec_dates_sorted
    report(7, "Equity curve structure and daily time series coverage", ec_ok,
           f"points={len(ec)}, fields_ok={ec_fields}, sorted={ec_dates_sorted}")

    # ── Phase 8: Position values in valid range ──────────────────────────────
    positions_valid = all(pt.get("position") in [-1, 0, 1] for pt in ec) if ec else False
    report(8, "Equity curve position values within valid range (-1, 0, 1)", positions_valid,
           f"positions={set(pt.get('position') for pt in ec[:20])}")

    # ── Phase 9: Validation — empty tickers ──────────────────────────────────
    empty_payload = {**alpha_payload, "tickers": []}
    empty_resp = client.post("/signals/alpha-report", json=empty_payload, headers=auth_headers)
    report(9, "Validation: empty tickers returns 400 Bad Request",
           empty_resp.status_code == 400, f"Status: {empty_resp.status_code}")

    # ── Phase 10: Validation — too many tickers ──────────────────────────────
    many_payload = {**alpha_payload, "tickers": [f"T{i}" for i in range(11)]}
    many_resp = client.post("/signals/alpha-report", json=many_payload, headers=auth_headers)
    report(10, "Validation: >10 tickers returns 400 Bad Request",
           many_resp.status_code == 400, f"Status: {many_resp.status_code}")

    # ── Phase 11: Python SDK Sync Client ─────────────────────────────────────
    try:
        sdk_client = FinTextClient(base_url=BASE_URL, api_token=auth_token)
        sdk_report = sdk_client.generate_alpha_report(
            tickers=["AAPL", "MSFT"],
            start_date="2024-01-01",
            end_date="2024-12-31",
            signal_config=AlphaSignalConfig(
                signal_type="sentiment",
                threshold_long=0.2,
                threshold_short=-0.2,
                holding_days=5,
            ),
        )
        sdk_ok = (
            isinstance(sdk_report, AlphaReportResponse) and
            sdk_report.tickers == ["AAPL", "MSFT"] and
            isinstance(sdk_report.metrics, PerformanceMetrics) and
            sdk_report.metrics.annualized_volatility > 0.0 and
            len(sdk_report.equity_curve) > 0 and
            isinstance(sdk_report.equity_curve[0], EquityCurvePoint)
        )
        # Also test alias
        alias_report = sdk_client.alpha_report(
            tickers=["NVDA"],
            start_date="2024-01-01",
            end_date="2024-12-31",
        )
        sdk_ok = sdk_ok and isinstance(alias_report, AlphaReportResponse)
        report(11, "Python SDK Sync Client (client.generate_alpha_report / client.alpha_report)", sdk_ok)
    except Exception as exc:
        report(11, "Python SDK Sync Client (client.generate_alpha_report)", False, str(exc))

    # ── Phase 12: Python SDK Async Client ────────────────────────────────────
    async def test_async_sdk():
        async_client = FinTextAsyncClient(base_url=BASE_URL, api_token=auth_token)
        async_report = await async_client.generate_alpha_report(
            tickers=["AAPL"],
            start_date="2024-01-01",
            end_date="2024-12-31",
        )
        async_alias = await async_client.alpha_report(
            tickers=["MSFT"],
            start_date="2024-01-01",
            end_date="2024-12-31",
        )
        await async_client.close()
        return (
            isinstance(async_report, AlphaReportResponse) and
            async_report.tickers == ["AAPL"] and
            isinstance(async_report.metrics, PerformanceMetrics) and
            isinstance(async_alias, AlphaReportResponse)
        )

    try:
        sdk_async_ok = asyncio.run(test_async_sdk())
        report(12, "Python SDK Async Client (await async_client.generate_alpha_report)", sdk_async_ok)
    except Exception as exc:
        report(12, "Python SDK Async Client (await async_client.generate_alpha_report)", False, str(exc))

    # ── Phase 13: OpenAPI specification verification ─────────────────────────
    openapi_resp = client.get("/api-docs/openapi.json")
    if openapi_resp.status_code == 200:
        spec = openapi_resp.json()
        paths = spec.get("paths", {})
        schemas = spec.get("components", {}).get("schemas", {})
        tags = [t.get("name") for t in spec.get("tags", [])]

        openapi_ok = (
            "/signals/alpha-report" in paths and
            "AlphaReportResponse" in schemas and
            "AlphaSignalConfig" in schemas and
            "PerformanceMetrics" in schemas and
            "EquityCurvePoint" in schemas and
            "AlphaReportRequest" in schemas and
            "Alpha Signal Validation" in tags
        )
        report(13, "OpenAPI 3.0 Documentation & Component Schema Registration", openapi_ok,
               f"/signals/alpha-report in paths: {'/signals/alpha-report' in paths}, "
               f"Alpha Signal Validation tag: {'Alpha Signal Validation' in tags}")
    else:
        report(13, "OpenAPI 3.0 Documentation & Component Schema Registration", False,
               f"openapi status: {openapi_resp.status_code}")

    # ── Phase 14: Default signal_config correctly applied ────────────────────
    default_payload = {
        "tickers": ["NVDA"],
        "start_date": "2024-06-01",
        "end_date": "2024-12-31",
        "signal_config": {
            "signal_type": "sentiment",
            "threshold_long": 0.2,
            "threshold_short": -0.2,
            "holding_days": 5,
        },
    }
    default_resp = client.post("/signals/alpha-report", json=default_payload, headers=auth_headers)
    if default_resp.status_code == 200:
        d_data = default_resp.json()
        sc = d_data.get("signal_config", {})
        bm = d_data.get("metrics", {}).get("benchmark_ticker", "")
        cap = d_data.get("initial_capital", 0.0)
        default_ok = (
            sc.get("signal_type") == "sentiment" and
            sc.get("holding_days") == 5 and
            bm == "SPY" and
            cap == 1000000.0
        )
        report(14, "Default signal_config and capital parameters applied", default_ok,
               f"signal={sc.get('signal_type')}, holding={sc.get('holding_days')}, benchmark={bm}, capital={cap}")
    else:
        report(14, "Default signal_config and capital parameters applied", False,
               f"Status: {default_resp.status_code}")


if __name__ == "__main__":
    sys.exit(main())
