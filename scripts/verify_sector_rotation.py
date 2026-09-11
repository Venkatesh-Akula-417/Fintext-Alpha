"""
===============================================================================
FinText-Alpha-Vectorizer — Suite #214: Sector Rotation Signals & Relative
                                       Strength Ranking Engine
===============================================================================

12-phase certification covering:
  1. Default parameters response structure
  2. Custom lookback_days=60
  3. include_momentum=false → price_momentum == 0.0
  4. Custom top_n=5
  5. min_confidence=0.8 filtering
  6. Validation error cases (400)
  7. Rank uniqueness and completeness
  8. relative_strength values in [0.0, 1.0]
  9. market_signal enum validation
 10. Model metadata fields present
 11. Python SDK sync client integration
 12. Python SDK async client integration
"""

import asyncio
import os
import subprocess
import sys
import time
import uuid
from pathlib import Path

import httpx

# ── Project bootstrap ────────────────────────────────────────────────────────
PROJECT_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(PROJECT_ROOT / "python_sdk" / "src"))

if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")

SERVER_EXE = PROJECT_ROOT / "rust" / "target" / "debug" / "fintext_api.exe"
PORT = 8114
BASE_URL = f"http://127.0.0.1:{PORT}"
ADMIN_TOKEN = "dev_admin_secret_token_214"

passed = 0
failed = 0
total  = 0


def report(phase: int, title: str, ok: bool, detail: str = ""):
    global passed, failed, total
    total += 1
    if ok:
        passed += 1
        print(f"  ✅ Phase {phase:>2d} │ {title}")
    else:
        failed += 1
        msg = f" — {detail}" if detail else ""
        print(f"  ❌ Phase {phase:>2d} │ {title}{msg}")


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
        while time.time() - start_t < 15:
            try:
                r = httpx.get(f"{BASE_URL}/health", timeout=1.0)
                if r.status_code == 200:
                    ready = True
                    break
            except Exception:
                time.sleep(0.3)

        if not ready:
            if self.process.poll() is not None:
                out, err = self.process.communicate()
                print(f"[ERROR] Server died on startup:\nSTDOUT:\n{out}\nSTDERR:\n{err}")
            raise RuntimeError("FinText API server failed to start within 15 seconds")

        print(f"[RUNNING] FinText API Server is ready on {BASE_URL}")
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        if self.process:
            print("[STOPPING] Terminating FinText API server...")
            self.process.terminate()
            try:
                self.process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.process.kill()
            print("[STOPPED] Server terminated.")


def get_auth_headers(client: httpx.Client) -> dict:
    """Issue a JWT using X-Admin-Token header."""
    uid = str(uuid.uuid4())
    resp = client.post(
        "/auth/token",
        json={"user_id": uid, "role": "institutional", "expires_in_seconds": 3600},
        headers={"X-Admin-Token": ADMIN_TOKEN},
    )
    assert resp.status_code == 200, f"Token issuance failed: {resp.text}"
    token = resp.json()["token"]
    return {"Authorization": f"Bearer {token}"}


def run_suite():
    global passed, failed
    print()
    print("=" * 80)
    print("  Suite #214: Sector Rotation Signals & Relative Strength Ranking Engine")
    print("=" * 80)
    print()

    with ServerContext():
        client = httpx.Client(base_url=BASE_URL, timeout=30.0)
        headers = get_auth_headers(client)

        # ── Phase 1: Default parameters ──────────────────────────────────────
        resp = client.get("/market/sector-rotation", headers=headers)
        ok = resp.status_code == 200
        data = resp.json() if ok else {}
        has_sectors = isinstance(data.get("sectors"), list) and len(data["sectors"]) > 0
        has_outperform = isinstance(data.get("outperform_sectors"), list)
        has_underperform = isinstance(data.get("underperform_sectors"), list)
        has_signal = data.get("market_signal") in ("risk-on", "risk-off", "neutral")
        has_generated = "generated_at" in data
        report(1, "Default parameters — 200 OK with full response structure",
               ok and has_sectors and has_outperform and has_underperform and has_signal and has_generated,
               f"status={resp.status_code}, sectors={len(data.get('sectors', []))}")

        # ── Phase 2: Custom lookback_days=60 ─────────────────────────────────
        resp2 = client.get("/market/sector-rotation",
                           params={"lookback_days": 60},
                           headers=headers)
        ok2 = resp2.status_code == 200
        data2 = resp2.json() if ok2 else {}
        report(2, "Custom lookback_days=60 — accepted and returns valid response",
               ok2 and data2.get("lookback_days") == 60,
               f"status={resp2.status_code}, lookback_days={data2.get('lookback_days')}")

        # ── Phase 3: include_momentum=false ──────────────────────────────────
        resp3 = client.get("/market/sector-rotation",
                           params={"include_momentum": "false"},
                           headers=headers)
        ok3 = resp3.status_code == 200
        data3 = resp3.json() if ok3 else {}
        all_zero_price_mom = all(
            s.get("price_momentum", 999) == 0.0
            for s in data3.get("sectors", [])
        ) if ok3 and data3.get("sectors") else False
        report(3, "include_momentum=false — all price_momentum values are 0.0",
               ok3 and data3.get("include_momentum") is False and all_zero_price_mom,
               f"status={resp3.status_code}, include_momentum={data3.get('include_momentum')}")

        # ── Phase 4: Custom top_n=5 ──────────────────────────────────────────
        resp4 = client.get("/market/sector-rotation",
                           params={"top_n": 5},
                           headers=headers)
        ok4 = resp4.status_code == 200
        data4 = resp4.json() if ok4 else {}
        n_sectors4 = len(data4.get("sectors", []))
        effective_top_n = min(5, n_sectors4) if n_sectors4 > 0 else 0
        report(4, f"Custom top_n=5 — top_n={data4.get('top_n')}",
               ok4 and data4.get("top_n") == effective_top_n,
               f"status={resp4.status_code}")

        # ── Phase 5: min_confidence=0.8 ──────────────────────────────────────
        resp5 = client.get("/market/sector-rotation",
                           params={"min_confidence": 0.8},
                           headers=headers)
        ok5 = resp5.status_code == 200
        data5 = resp5.json() if ok5 else {}
        report(5, "min_confidence=0.8 — accepted and returns valid sectors",
               ok5 and isinstance(data5.get("sectors"), list),
               f"status={resp5.status_code}")

        # ── Phase 6: Validation errors (400) ─────────────────────────────────
        cases_400 = [
            ("lookback_days=0", {"lookback_days": 0}),
            ("lookback_days=100", {"lookback_days": 100}),
            ("top_n=0", {"top_n": 0}),
            ("top_n=15", {"top_n": 15}),
            ("min_confidence=2.0", {"min_confidence": 2.0}),
            ("min_confidence=-0.5", {"min_confidence": -0.5}),
        ]
        all_400_ok = True
        fail_detail = ""
        for label, params_400 in cases_400:
            r400 = client.get("/market/sector-rotation",
                              params=params_400, headers=headers)
            if r400.status_code != 400:
                all_400_ok = False
                fail_detail = f"{label} → {r400.status_code}"
                break
        report(6, "Validation errors — 6 bad-param combos return 400",
               all_400_ok, fail_detail)

        # ── Phase 7: Rank uniqueness and completeness ────────────────────────
        sectors_p1 = data.get("sectors", [])
        ranks = [s.get("rank") for s in sectors_p1]
        expected_ranks = list(range(1, len(sectors_p1) + 1))
        report(7, f"Rank uniqueness/completeness — ranks 1..{len(sectors_p1)}",
               sorted(ranks) == expected_ranks,
               f"ranks={sorted(ranks)}, expected={expected_ranks}")

        # ── Phase 8: relative_strength in [0.0, 1.0] ────────────────────────
        rs_values = [s.get("relative_strength", -1) for s in sectors_p1]
        all_in_range = all(0.0 <= v <= 1.0 for v in rs_values)
        report(8, f"relative_strength in [0.0, 1.0] — {len(rs_values)} values checked",
               all_in_range and len(rs_values) > 0,
               f"values={rs_values}")

        # ── Phase 9: market_signal enum ──────────────────────────────────────
        valid_signals = {"risk-on", "risk-off", "neutral"}
        sig = data.get("market_signal", "")
        report(9, f"market_signal='{sig}' is valid enum",
               sig in valid_signals,
               f"got '{sig}'")

        # ── Phase 10: Model metadata fields ──────────────────────────────────
        first_sector = sectors_p1[0] if sectors_p1 else {}
        has_mv = "model_version" in first_sector
        has_pv = "pipeline_version" in first_sector
        has_dp = "data_provenance" in first_sector
        report(10, "Model metadata (model_version, pipeline_version, data_provenance) present",
               has_mv and has_pv and has_dp,
               f"mv={has_mv}, pv={has_pv}, dp={has_dp}")

        # ── Phase 11: Python SDK sync client ─────────────────────────────────
        sdk_ok = False
        sdk_detail = ""
        try:
            from fintext.client import FinTextClient
            ft_client = FinTextClient(base_url=BASE_URL, admin_token=ADMIN_TOKEN)
            result = ft_client.sector_rotation(lookback_days=20, top_n=2)
            sdk_ok = (
                hasattr(result, "sectors")
                and len(result.sectors) > 0
                and result.lookback_days == 20
                and result.top_n == 2
                and result.market_signal in valid_signals
            )
            ft_client.close()
        except Exception as exc:
            sdk_detail = str(exc)[:200]
        report(11, "Python SDK sync client — sector_rotation()",
               sdk_ok, sdk_detail)

        # ── Phase 12: Python SDK async client ────────────────────────────────
        async_ok = False
        async_detail = ""
        try:
            from fintext.async_client import FinTextAsyncClient

            async def _test_async():
                async with FinTextAsyncClient(base_url=BASE_URL, admin_token=ADMIN_TOKEN) as ac:
                    r = await ac.sector_rotation(lookback_days=15, include_momentum=False, top_n=2)
                    return (
                        hasattr(r, "sectors")
                        and len(r.sectors) > 0
                        and r.include_momentum is False
                        and all(s.price_momentum == 0.0 for s in r.sectors)
                    )

            async_ok = asyncio.run(_test_async())
        except Exception as exc:
            async_detail = str(exc)[:200]
        report(12, "Python SDK async client — sector_rotation(include_momentum=False)",
               async_ok, async_detail)

        client.close()

    # ── Summary ──────────────────────────────────────────────────────────────
    print()
    print("─" * 80)
    print(f"  Suite #214 Result: {passed}/{total} phases passed, {failed} failed")
    print("─" * 80)
    print()

    if failed > 0:
        sys.exit(1)


if __name__ == "__main__":
    run_suite()
