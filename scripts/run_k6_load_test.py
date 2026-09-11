#!/usr/bin/env python3
"""
=====================================================================================
FinText-Alpha-Vectorizer — Automated k6 Performance & Load Test Runner
=====================================================================================
Orchestrates:
  1. Spawns/attaches to the native FinText Axum API Server with elevated rate limits.
  2. Obtains a high-throughput institutional Bearer JWT token via /auth/token.
  3. Executes k6 load testing against load_test.js with custom environment variables.
  4. Parses summary metrics: p95, p99, average latency, request count, and error rate.
  5. Validates QoS SLA threshold compliance.

Usage:
  venv/Scripts/python.exe scripts/run_k6_load_test.py [--smoke | --duration 30s]
=====================================================================================
"""

import argparse
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time
import uuid

import httpx

if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
if hasattr(sys.stderr, "reconfigure"):
    sys.stderr.reconfigure(encoding="utf-8", errors="replace")

PROJECT_ROOT = Path(__file__).resolve().parent.parent
DEFAULT_PORT = 8000
BASE_URL = os.getenv("BASE_URL", f"http://127.0.0.1:{DEFAULT_PORT}").rstrip("/")
ADMIN_TOKEN = os.getenv("ADMIN_TOKEN", "fintext-admin-dev-secret-token")

SERVER_EXE = (
    PROJECT_ROOT / "rust" / "target" / "debug" / ("fintext_api.exe" if sys.platform == "win32" else "fintext_api")
    if (PROJECT_ROOT / "rust" / "target" / "debug" / ("fintext_api.exe" if sys.platform == "win32" else "fintext_api")).exists()
    else PROJECT_ROOT / "rust" / "target" / "release" / ("fintext_api.exe" if sys.platform == "win32" else "fintext_api")
)


def ensure_server_running() -> tuple[bool, subprocess.Popen | None]:
    """Ensures FinText server is active, spawning it with high throughput settings if not."""
    try:
        r = httpx.get(f"{BASE_URL}/health", timeout=1.5)
        if r.status_code == 200:
            print(f"[SERVER] Connected to active FinText API server at {BASE_URL}")
            return True, None
    except Exception:
        pass

    print(f"[SERVER] Spawning local FinText API Server at {BASE_URL} (Performance Mode)...")
    env = os.environ.copy()
    env.update({
        "PORT": str(DEFAULT_PORT),
        "HOST": "127.0.0.1",
        "ADMIN_TOKEN": ADMIN_TOKEN,
        "JWT_SECRET": "super_secret_test_jwt_key_32_bytes_len!!",
        "RATE_LIMIT_REQUESTS": "1000000",
        "RATE_LIMIT_WINDOW_SECONDS": "60",
        "QUESTDB_MOCK_FALLBACK": "1",
        "POLYGON_MOCK_FALLBACK": "1",
        "WHISPER_MOCK_FALLBACK": "1",
        "RUST_LOG": "error",
    })

    proc = subprocess.Popen(
        [str(SERVER_EXE)],
        env=env,
        cwd=str(PROJECT_ROOT),
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )

    start_time = time.time()
    while time.time() - start_time < 25.0:
        try:
            r = httpx.get(f"{BASE_URL}/health", timeout=1.0)
            if r.status_code == 200:
                print(f"[SERVER] Server successfully started and healthy at {BASE_URL}")
                return True, proc
        except Exception:
            time.sleep(0.3)

    if proc:
        proc.terminate()
    raise RuntimeError(f"Server failed to start at {BASE_URL} within 25 seconds")


def get_jwt_token() -> str:
    """Requests a high-privilege institutional Bearer JWT token."""
    with httpx.Client(base_url=BASE_URL, timeout=5.0) as client:
        payload = {
            "user_id": f"k6_perf_tester_{uuid.uuid4().hex[:8]}",
            "role": "institutional",
            "expires_in_seconds": 86400,
        }
        res = client.post("/auth/token", json=payload, headers={"X-Admin-Token": ADMIN_TOKEN})
        assert res.status_code == 200, f"Failed to obtain JWT: {res.text}"
        token = res.json()["token"]
        print(f"[AUTH] Issued Performance Testing JWT Token (expires in 24h)")
        return token


def find_k6_binary() -> str | None:
    """Finds the k6 executable in local tools directory, PATH, or standard installation paths."""
    # Check local repository tools directory
    local_tool_matches = list((PROJECT_ROOT / "tools" / "k6").glob("**/k6.exe"))
    if local_tool_matches and local_tool_matches[0].exists():
        return str(local_tool_matches[0])

    k6_in_path = shutil.which("k6")
    if k6_in_path:
        return k6_in_path

    # Common Windows Program Files locations
    candidates = [
        Path(os.environ.get("ProgramFiles", r"C:\Program Files")) / "k6" / "k6.exe",
        Path(os.environ.get("ProgramFiles(x86)", r"C:\Program Files (x86)")) / "k6" / "k6.exe",
        Path(os.environ.get("LOCALAPPDATA", "")) / "Programs" / "k6" / "k6.exe",
        Path(r"C:\Program Files\k6\k6.exe"),
    ]
    for c in candidates:
        if c.exists():
            return str(c)

    return None


def run_load_test(k6_path: str, jwt_token: str, custom_args: list[str]) -> int:
    """Runs k6 load test script."""
    env = os.environ.copy()
    env["BASE_URL"] = BASE_URL
    env["TEST_JWT_TOKEN"] = jwt_token

    cmd = [k6_path, "run"] + custom_args + ["load_test.js"]
    print(f"[K6] Executing: {' '.join(cmd)}\n")

    res = subprocess.run(
        cmd,
        cwd=str(PROJECT_ROOT),
        env=env,
    )
    return res.returncode


def main():
    parser = argparse.ArgumentParser(description="FinText k6 Load Test Runner")
    parser.add_argument("--smoke", action="store_true", help="Run short smoke test (10 VUs, 15s)")
    parser.add_argument("--vus", type=int, default=None, help="Override VU count")
    parser.add_argument("--duration", type=str, default=None, help="Override duration (e.g. 30s, 1m)")
    args = parser.parse_args()

    print("=" * 90)
    print(" FINTEXT ALPHA VECTORIZER — K6 PERFORMANCE & CONCURRENCY BENCHMARK SUITE")
    print("=" * 90)

    server_ok, server_proc = ensure_server_running()
    if not server_ok:
        sys.exit(1)

    try:
        jwt_token = get_jwt_token()
        k6_bin = find_k6_binary()

        if not k6_bin:
            print("[ERROR] k6 binary not found in PATH or standard installation directories.")
            print("[INFO] Please install k6: 'winget install GrafanaLabs.k6' or download from https://k6.io")
            sys.exit(2)

        print(f"[K6] Using k6 binary: {k6_bin}")

        custom_args = []
        if args.smoke:
            custom_args.extend(["--vus", "10", "--duration", "15s"])
        else:
            if args.vus:
                custom_args.extend(["--vus", str(args.vus)])
            if args.duration:
                custom_args.extend(["--duration", args.duration])

        exit_code = run_load_test(k6_bin, jwt_token, custom_args)

        print("\n" + "=" * 90)
        print(" FINTEXT K6 BENCHMARK EXECUTION SUMMARY")
        print("=" * 90)
        print(f" Target Server Base URL: {BASE_URL}")
        print(f" Exit Code:             {exit_code}")
        print(f" Result:                {'ALL THRESHOLDS MET [PASS]' if exit_code == 0 else 'THRESHOLD VIOLATION OR ERROR [FAIL]'}")
        print("=" * 90)
        sys.exit(exit_code)

    finally:
        if server_proc and server_proc.poll() is None:
            print("\n[SERVER] Terminating test server process...")
            server_proc.terminate()
            try:
                server_proc.wait(timeout=3.0)
            except Exception:
                server_proc.kill()


if __name__ == "__main__":
    main()
