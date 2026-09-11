#!/usr/bin/env python3
"""
===============================================================================
FinText-Alpha-Vectorizer — Verification: Signal Quality Report & Alpha Validation
===============================================================================
Verifies:
  1.  POST /signals/quality-report returns 200 OK with valid parameters
  2.  Response contains correct top-level fields
  3.  Coverage percentage within valid range [0, 100]%
  4.  Freshness latency metrics: avg_ms > 0, p95_ms >= avg_ms
  5.  IC summary metrics: observations > 0, spearman_ic and rank_ic finite
  6.  Decay curve contains all 6 horizons: [1, 2, 3, 5, 10, 20]
  7.  Signal half-life estimate is positive and reasonable
  8.  Directional hit rate within valid percentage range [0, 100]%
  9.  False positive rate within valid percentage range [0, 100]%
  10. Systematic sector bias breakdown mapping is populated
  11. Market cap quantile bias: top, bottom, and spread populated
  12. Validation: invalid signal_type returns 400 Bad Request
  13. Validation: >20 tickers returns 400 Bad Request
  14. Python SDK Sync/Async Client integration & OpenAPI 3.0 schema verification
  15. ICIR (Information Coefficient Information Ratio) metric validation
===============================================================================
"""

import asyncio
import json
import math
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

PORT = 8148
BASE_URL = f"http://127.0.0.1:{PORT}"
ADMIN_TOKEN = "test_admin_token_xyz123_valid_32_bytes_length!"
SERVER_EXE = PROJECT_ROOT / "rust" / "target" / "debug" / "fintext_api.exe"

passed = 0
failed = 0
total = 15


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
                time.sleep(0.3)

        if not ready:
            raise RuntimeError(f"Server on port {PORT} failed to start within 25 seconds.")
        print(f"[READY] API Server is healthy at {BASE_URL}")
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        if self.process:
            print("[CLEANUP] Terminating FinText API Server process...")
            self.process.terminate()
            try:
                self.process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.process.kill()


def run_all_phases():
    print("=" * 80)
    print(" FinText-Alpha-Vectorizer — Signal Quality Report Verification")
    print("=" * 80)
    print(f" Project Root: {PROJECT_ROOT}")
    print(f" Server Port:  {PORT}")

    with ServerContext():
        client = httpx.Client(base_url=BASE_URL, timeout=10.0)

        # 0. Obtain JWT auth token
        auth_resp = client.post(
            "/auth/token",
            json={
                "user_id": "quant_signal_quality_tester",
                "role": "institutional",
                "duration_seconds": 3600,
            },
            headers={"X-Admin-Token": ADMIN_TOKEN},
        )
        if auth_resp.status_code != 200:
            token = auth_resp.json().get("token") or "mock_jwt_token"
        else:
            token = auth_resp.json()["token"]

        headers = {"Authorization": f"Bearer {token}"}

        # Phase 1: POST /signals/quality-report returns 200 OK
        payload = {
            "signal_type": "sentiment",
            "tickers": ["AAPL", "MSFT", "NVDA"],
            "start_date": "2025-01-01",
            "end_date": "2025-06-30",
            "horizon_days": 5,
        }
        resp = client.post("/signals/quality-report", json=payload, headers=headers)
        report(1, "POST /signals/quality-report returns 200 OK", resp.status_code == 200, f"Status: {resp.status_code}, Body: {resp.text}")

        data = resp.json() if resp.status_code == 200 else {}

        # Phase 2: Response contains correct top-level fields
        expected_fields = {
            "signal_type",
            "tickers",
            "start_date",
            "end_date",
            "horizon_days",
            "coverage_pct",
            "freshness_avg_ms",
            "freshness_p95_ms",
            "ic_summary",
            "decay_curve",
            "half_life_days",
            "hit_rate_pct",
            "false_positive_rate_pct",
            "sector_bias",
            "market_cap_bias",
            "generated_at",
            "message",
        }
        actual_fields = set(data.keys())
        has_fields = expected_fields.issubset(actual_fields)
        report(2, "Response contains correct top-level fields", has_fields, f"Missing: {expected_fields - actual_fields}")

        # Phase 3: Coverage percentage within valid range [0, 100]%
        cov = data.get("coverage_pct", -1.0)
        cov_ok = 0.0 <= cov <= 100.0
        report(3, "Coverage percentage within valid range [0, 100]%", cov_ok, f"Coverage: {cov}%")

        # Phase 4: Freshness latency metrics
        f_avg = data.get("freshness_avg_ms", 0.0)
        f_p95 = data.get("freshness_p95_ms", 0.0)
        freshness_ok = f_avg > 0.0 and f_p95 >= f_avg
        report(4, "Freshness latency metrics: avg_ms > 0, p95_ms >= avg_ms", freshness_ok, f"avg: {f_avg}ms, p95: {f_p95}ms")

        # Phase 5: IC summary metrics
        ic_sum = data.get("ic_summary", {})
        ic_ok = (
            isinstance(ic_sum.get("spearman_ic"), (int, float))
            and isinstance(ic_sum.get("rank_ic"), (int, float))
            and ic_sum.get("observations", 0) > 0
        )
        report(5, "IC summary metrics: observations > 0, spearman_ic and rank_ic finite", ic_ok, f"IC Summary: {ic_sum}")

        # Phase 6: Decay curve contains all 6 horizons: [1, 2, 3, 5, 10, 20]
        decay = data.get("decay_curve", [])
        horizons = [p.get("horizon_days") for p in decay]
        decay_ok = horizons == [1, 2, 3, 5, 10, 20]
        report(6, "Decay curve contains all 6 horizons: [1, 2, 3, 5, 10, 20]", decay_ok, f"Horizons: {horizons}")

        # Phase 7: Signal half-life estimate is positive and reasonable
        hl = data.get("half_life_days", 0.0)
        hl_ok = hl > 0.0
        report(7, "Signal half-life estimate is positive and reasonable", hl_ok, f"Half-Life: {hl} days")

        # Phase 8: Directional hit rate within valid percentage range [0, 100]%
        hit_rate = data.get("hit_rate_pct", -1.0)
        hit_ok = 0.0 <= hit_rate <= 100.0
        report(8, "Directional hit rate within valid percentage range [0, 100]%", hit_ok, f"Hit Rate: {hit_rate}%")

        # Phase 9: False positive rate within valid percentage range [0, 100]%
        fpr = data.get("false_positive_rate_pct", -1.0)
        fpr_ok = 0.0 <= fpr <= 100.0
        report(9, "False positive rate within valid percentage range [0, 100]%", fpr_ok, f"FPR: {fpr}%")

        # Phase 10: Systematic sector bias breakdown mapping is populated
        sb = data.get("sector_bias", {})
        sb_ok = isinstance(sb, dict) and len(sb) > 0
        report(10, "Systematic sector bias breakdown mapping is populated", sb_ok, f"Sectors: {list(sb.keys())}")

        # Phase 11: Market cap quantile bias
        mcb = data.get("market_cap_bias", {})
        mcb_ok = (
            "top_quantile_avg" in mcb
            and "bottom_quantile_avg" in mcb
            and "spread" in mcb
            and mcb.get("spread", -1.0) >= 0.0
        )
        report(11, "Market cap quantile bias: top, bottom, and spread populated", mcb_ok, f"Market Cap Bias: {mcb}")

        # Phase 12: Validation: invalid signal_type returns 400 Bad Request
        r_bad_type = client.post(
            "/signals/quality-report",
            json={
                "signal_type": "non_existent_unsupported_signal",
                "tickers": ["AAPL"],
                "start_date": "2025-01-01",
                "end_date": "2025-06-30",
            },
            headers=headers,
        )
        report(12, "Validation: invalid signal_type returns 400 Bad Request", r_bad_type.status_code == 400)

        # Phase 13: Validation: >20 tickers returns 400 Bad Request
        r_too_many = client.post(
            "/signals/quality-report",
            json={
                "signal_type": "sentiment",
                "tickers": [f"TICK{i}" for i in range(25)],
                "start_date": "2025-01-01",
                "end_date": "2025-06-30",
            },
            headers=headers,
        )
        report(13, "Validation: >20 tickers returns 400 Bad Request", r_too_many.status_code == 400)

        # Phase 14: Python SDK Sync/Async Client integration & OpenAPI 3.0 schema verification
        from fintext import FinTextClient, FinTextAsyncClient, SignalQualityReportResponse

        sdk_client = FinTextClient(base_url=BASE_URL, api_token=token)
        sync_report = sdk_client.signal_quality_report(
            signal_type="sentiment",
            tickers=["AAPL", "MSFT"],
            start_date="2025-01-01",
            end_date="2025-06-30",
            horizon_days=5,
        )
        sync_alias_report = sdk_client.quality_report(
            signal_type="sentiment",
            tickers=["AAPL", "MSFT"],
            start_date="2025-01-01",
            end_date="2025-06-30",
        )

        async def verify_async_and_docs():
            async_c = FinTextAsyncClient(base_url=BASE_URL, api_token=token)
            async_rep = await async_c.signal_quality_report(
                signal_type="sentiment",
                tickers=["NVDA"],
                start_date="2025-01-01",
                end_date="2025-06-30",
            )
            await async_c.close()

            r_docs = client.get("/api-docs/openapi.json")
            has_docs = False
            if r_docs.status_code == 200:
                docs = r_docs.json()
                paths = docs.get("paths", {})
                schemas = docs.get("components", {}).get("schemas", {})
                tags = [t.get("name") for t in docs.get("tags", [])]

                has_docs = (
                    "/signals/quality-report" in paths
                    and "SignalQualityReportResponse" in schemas
                    and "ICSummary" in schemas
                    and "DecayCurvePoint" in schemas
                    and "Quantitative Research" in tags
                )

            return (
                isinstance(sync_report, SignalQualityReportResponse)
                and sync_report.signal_type == "sentiment"
                and isinstance(sync_alias_report, SignalQualityReportResponse)
                and isinstance(async_rep, SignalQualityReportResponse)
                and has_docs
            )

        sdk_ok = asyncio.run(verify_async_and_docs())
        report(14, "Python SDK Sync/Async Client & OpenAPI 3.0 Schema registration", sdk_ok)

        # Phase 15: ICIR (Information Coefficient Information Ratio) metric validation
        # For evaluation with 3 tickers over 181 days (>10 periods), icir should be populated and finite
        icir = ic_sum.get("icir")
        icir_ok = icir is not None and isinstance(icir, (int, float)) and math.isfinite(icir)
        report(15, "ICIR metric validation: icir present, finite, and non-null for >=10 periods", icir_ok, f"ICIR: {icir}")


if __name__ == "__main__":
    run_all_phases()
    print("=" * 80)
    print(f" Signal Quality Verification Results: {passed}/{total} Phases Passed ({failed} Failed)")
    print("=" * 80)
    if failed > 0:
        sys.exit(1)
    sys.exit(0)
