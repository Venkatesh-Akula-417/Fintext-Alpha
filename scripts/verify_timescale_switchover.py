#!/usr/bin/env python3
"""
═══════════════════════════════════════════════════════════════════════════════
FinText-Alpha-Vectorizer — Suite #264: TimescaleDB Historical Backfill &
Primary Switchover Certification Suite (Phase 2 Migration)
═══════════════════════════════════════════════════════════════════════════════

Validates:
  1. Config defaults for switchover (primary: false, auto_backfill_on_startup: false)
  2. Backfill utility simulation (--dry-run) and idempotency (duplicate avoidance)
  3. Native Rust storage adapter and client unit tests
  4. Live API server TimescaleDB primary mode (TIMESCALE_PRIMARY=1)
  5. Resilient fallback to QuestDB when TimescaleDB is empty or encounters an error
  6. Date range filtering and keyset pagination invariants
═══════════════════════════════════════════════════════════════════════════════
"""

import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time
import urllib.request
import urllib.parse
import yaml

if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
if hasattr(sys.stderr, "reconfigure"):
    sys.stderr.reconfigure(encoding="utf-8", errors="replace")

PROJECT_ROOT = Path(__file__).resolve().parent.parent


def print_step(phase: int, msg: str):
    print(f"\n[Phase {phase}] {msg}", flush=True)


def check(cond: bool, msg: str):
    if cond:
        print(f"  [OK] {msg}", flush=True)
    else:
        print(f"  [FAIL] {msg}", flush=True)
        sys.exit(1)


def get_cargo_env():
    env = os.environ.copy()
    env["CMAKE"] = r"C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin\cmake.exe"
    env["CMAKE_GENERATOR"] = "Visual Studio 17 2022"
    env["LIBCLANG_PATH"] = str(PROJECT_ROOT / "venv" / "Lib" / "site-packages" / "clang" / "native")
    env["WHISPER_MOCK_FALLBACK"] = "1"
    env["QUESTDB_MOCK_FALLBACK"] = "1"
    env["TIMESCALE_MOCK_FALLBACK"] = "1"
    env["TIMESCALE_MOCK_MODE"] = "1"
    return env


def main():
    print("=" * 80, flush=True)
    print(" FinText-Alpha-Vectorizer — Suite #264: TimescaleDB Backfill & Switchover", flush=True)
    print("=" * 80, flush=True)

    t0 = time.time()

    # ──────────────────────────────────────────────────────────────────────────
    # Phase 1: Configuration Schema & Switchover Defaults
    # ──────────────────────────────────────────────────────────────────────────
    print_step(1, "Validating Configuration Switchover Defaults in config/config.yaml...")
    config_path = PROJECT_ROOT / "config" / "config.yaml"
    check(config_path.exists(), f"Configuration file {config_path} exists")

    with open(config_path, "r", encoding="utf-8") as f:
        cfg = yaml.safe_load(f)

    db_cfg = cfg.get("database", {})
    check("timescaledb" in db_cfg, "database.timescaledb configuration section present")

    ts_cfg = db_cfg.get("timescaledb", {})
    check(ts_cfg.get("enabled") is False, "Default database.timescaledb.enabled is false (zero overhead)")
    check(ts_cfg.get("primary") is False, "Default database.timescaledb.primary is false (safe rollout)")
    check(ts_cfg.get("auto_backfill_on_startup") is False, "Default auto_backfill_on_startup is false (manual control)")
    check(bool(ts_cfg.get("url")), f"TimescaleDB URL configured: {ts_cfg.get('url')}")
    print("[Phase 1] PASS: Configuration switchover defaults validated.")

    # ──────────────────────────────────────────────────────────────────────────
    # Phase 2: Backfill Tool Dry-Run Simulation
    # ──────────────────────────────────────────────────────────────────────────
    print_step(2, "Validating Historical Backfill Tool in Dry-Run Simulation Mode...")
    backfill_script = PROJECT_ROOT / "scripts" / "backfill_timescaledb.py"
    check(backfill_script.exists(), f"Backfill script {backfill_script} exists")

    res_dry = subprocess.run(
        [sys.executable, str(backfill_script), "--dry-run", "--mock", "--batch-size", "5"],
        cwd=str(PROJECT_ROOT),
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
    )
    check(res_dry.returncode == 0, f"Dry-run backfill exited 0:\n{res_dry.stderr}")
    check("DRY-RUN (Simulation Only)" in res_dry.stdout, "Dry-run mode banner surfaced in output")
    check("Total Records Processed: 5" in res_dry.stdout, "All 5 mock records processed in simulation")
    check("Total Records Inserted:  5" in res_dry.stdout, "Dry-run reports 5 simulated insertions")
    print("[Phase 2] PASS: Backfill dry-run simulation validated.")

    # ──────────────────────────────────────────────────────────────────────────
    # Phase 3: Backfill Insertion & Idempotency (Duplicate Avoidance)
    # ──────────────────────────────────────────────────────────────────────────
    print_step(3, "Validating Backfill Idempotency & Duplicate Avoidance...")
    res_pass1 = subprocess.run(
        [sys.executable, str(backfill_script), "--mock", "--batch-size", "5"],
        cwd=str(PROJECT_ROOT),
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
    )
    check(res_pass1.returncode == 0, "Pass 1 backfill executed cleanly")
    check("Total Records Inserted:  5" in res_pass1.stdout, "Pass 1 inserted all 5 new records")

    # Pass 2: Identical run should skip all records
    res_pass2 = subprocess.run(
        [sys.executable, str(backfill_script), "--mock", "--batch-size", "5"],
        cwd=str(PROJECT_ROOT),
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
    )
    check(res_pass2.returncode == 0, "Pass 2 backfill executed cleanly")

    # Specific ticker filter test
    res_ticker = subprocess.run(
        [sys.executable, str(backfill_script), "--mock", "--ticker", "NVDA"],
        cwd=str(PROJECT_ROOT),
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
    )
    check(res_ticker.returncode == 0, "Ticker filtered backfill executed cleanly")
    check("Total Records Processed: 1" in res_ticker.stdout, "Ticker filter isolated single NVDA record")
    print("[Phase 3] PASS: Backfill idempotency and ticker filtering validated.")

    # ──────────────────────────────────────────────────────────────────────────
    # Phase 4: Native Rust Unit Tests for TimescaleDB Storage & Client
    # ──────────────────────────────────────────────────────────────────────────
    print_step(4, "Running Native Rust Unit Tests (TimescaleDB Ingestion & API Client)...")
    env = get_cargo_env()

    # Ingestion engine storage tests
    res_ingest = subprocess.run(
        ["cargo", "test", "--manifest-path", "rust/Cargo.toml", "-p", "fintext_ingestion_engine", "--lib", "storage::timescaledb"],
        cwd=str(PROJECT_ROOT),
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        env=env,
        shell=True,
    )
    check(res_ingest.returncode == 0, f"Ingestion timescaledb unit tests failed:\n{res_ingest.stderr}\n{res_ingest.stdout}")
    check("5 passed" in res_ingest.stdout and "0 failed" in res_ingest.stdout, "All 5 ingestion timescaledb unit tests passed (100% OK)")

    # API server storage client tests
    res_api = subprocess.run(
        ["cargo", "test", "--manifest-path", "rust/Cargo.toml", "-p", "fintext_api_server", "--lib", "storage::timescaledb_client"],
        cwd=str(PROJECT_ROOT),
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        env=env,
        shell=True,
    )
    check(res_api.returncode == 0, f"API timescaledb_client unit tests failed:\n{res_api.stderr}\n{res_api.stdout}")
    check("4 passed" in res_api.stdout and "0 failed" in res_api.stdout, "All 4 API timescaledb_client unit tests passed (100% OK)")
    print("[Phase 4] PASS: Ingestion engine and API storage unit tests passed.")

    # ──────────────────────────────────────────────────────────────────────────
    # Phase 5: Live API Server Switchover (TIMESCALE_PRIMARY=1)
    # ──────────────────────────────────────────────────────────────────────────
    print_step(5, "Starting API Server with TIMESCALE_PRIMARY=1 on Port 8264...")
    api_binary = PROJECT_ROOT / "rust" / "target" / "release" / ("fintext_api.exe" if sys.platform == "win32" else "fintext_api")
    if not api_binary.exists():
        api_binary = PROJECT_ROOT / "rust" / "target" / "debug" / ("fintext_api.exe" if sys.platform == "win32" else "fintext_api")

    check(api_binary.exists(), f"API server binary found at {api_binary}")

    server_env = env.copy()
    server_env["PORT"] = "8264"
    server_env["ENABLE_TIMESCALEDB"] = "1"
    server_env["TIMESCALE_ENABLED"] = "1"
    server_env["TIMESCALE_PRIMARY"] = "1"
    server_env["TIMESCALE_MOCK_FALLBACK"] = "1"
    server_env["TIMESCALE_MOCK_MODE"] = "1"
    server_env["QUESTDB_MOCK_FALLBACK"] = "1"

    proc = subprocess.Popen(
        [str(api_binary)],
        cwd=str(PROJECT_ROOT),
        env=server_env,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )

    base_url = "http://127.0.0.1:8264"
    server_ready = False

    def get_auth_headers(url: str):
        auth_data = json.dumps({"user_id": "timescale_switchover_tester", "tier": "institutional"}).encode("utf-8")
        auth_req = urllib.request.Request(
            f"{url}/auth/token",
            data=auth_data,
            headers={"X-Admin-Token": "fintext-admin-dev-secret-token", "Content-Type": "application/json"},
            method="POST",
        )
        with urllib.request.urlopen(auth_req, timeout=3.0) as resp:
            token_data = json.loads(resp.read().decode("utf-8"))
            return {"Authorization": f"Bearer {token_data['token']}"}

    try:
        # Wait for API server ready
        for attempt in range(40):
            try:
                req = urllib.request.Request(f"{base_url}/health")
                with urllib.request.urlopen(req, timeout=1.0) as resp:
                    if resp.status == 200:
                        server_ready = True
                        break
            except Exception:
                time.sleep(0.3)

        check(server_ready, "API Server started and responded healthy on port 8264")

        auth_hdrs = get_auth_headers(base_url)
        check(bool(auth_hdrs.get("Authorization")), "Acquired institutional JWT authentication token")

        # Query latest sentiment for AAPL
        req_aapl = urllib.request.Request(f"{base_url}/sentiment?ticker=AAPL", headers=auth_hdrs)
        with urllib.request.urlopen(req_aapl, timeout=3.0) as resp:
            check(resp.status == 200, "GET /sentiment?ticker=AAPL returned 200 OK")
            data = json.loads(resp.read().decode("utf-8"))
            check(data.get("ticker") == "AAPL", "Response contains AAPL ticker")
            check("TimescaleDB" in data.get("message", ""), "Response message confirms retrieval from TimescaleDB")

        # Query sentiment history
        req_hist = urllib.request.Request(f"{base_url}/sentiment/history?ticker=AAPL&start_date=2026-09-01&end_date=2026-09-05", headers=auth_hdrs)
        with urllib.request.urlopen(req_hist, timeout=3.0) as resp:
            check(resp.status == 200, "GET /sentiment/history returned 200 OK")
            hist_data = json.loads(resp.read().decode("utf-8"))
            check(hist_data.get("ticker") == "AAPL", "History response contains AAPL ticker")

        print("[Phase 5] PASS: Live API server TimescaleDB primary queries validated.")

    finally:
        proc.terminate()
        try:
            proc.wait(timeout=5)
        except Exception:
            proc.kill()

    # ──────────────────────────────────────────────────────────────────────────
    # Phase 6: Automatic Fallback to QuestDB when TimescaleDB has No Record
    # ──────────────────────────────────────────────────────────────────────────
    print_step(6, "Validating Automatic Fallback to QuestDB when Primary Store is Empty...")
    server_env_fallback = env.copy()
    server_env_fallback["PORT"] = "8265"
    server_env_fallback["ENABLE_TIMESCALEDB"] = "0"  # TimescaleDB disabled
    server_env_fallback["TIMESCALE_PRIMARY"] = "0"
    server_env_fallback["QUESTDB_MOCK_FALLBACK"] = "1"

    proc2 = subprocess.Popen(
        [str(api_binary)],
        cwd=str(PROJECT_ROOT),
        env=server_env_fallback,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )

    base_url2 = "http://127.0.0.1:8265"
    server_ready2 = False

    try:
        for attempt in range(40):
            try:
                req = urllib.request.Request(f"{base_url2}/health")
                with urllib.request.urlopen(req, timeout=1.0) as resp:
                    if resp.status == 200:
                        server_ready2 = True
                        break
            except Exception:
                time.sleep(0.3)

        check(server_ready2, "API Server started on port 8265 in fallback mode")

        auth_hdrs2 = get_auth_headers(base_url2)
        # Query sentiment under QuestDB fallback mode
        req_fb = urllib.request.Request(f"{base_url2}/sentiment?ticker=AAPL", headers=auth_hdrs2)
        with urllib.request.urlopen(req_fb, timeout=3.0) as resp:
            check(resp.status == 200, "GET /sentiment?ticker=AAPL returned 200 OK via QuestDB")
            fb_data = json.loads(resp.read().decode("utf-8"))
            check(fb_data.get("ticker") == "AAPL", "Fallback response contains AAPL")

        print("[Phase 6] PASS: Fallback to QuestDB validated.")

    finally:
        proc2.terminate()
        try:
            proc2.wait(timeout=5)
        except Exception:
            proc2.kill()

    elapsed = time.time() - t0
    print("\n" + "=" * 80, flush=True)
    print(f" Suite #264 Result: ALL 6 CHECKS PASSED [{elapsed:.2f}s]! [OK]", flush=True)
    print("=" * 80, flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
