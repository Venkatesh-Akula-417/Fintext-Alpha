#!/usr/bin/env python3
"""
=====================================================================================
FinText-Alpha-Vectorizer — End-to-End User Journey Integration Workflow Test
=====================================================================================
Simulates a complete real-world quant workflow lifecycle:
  Step 1: User Signup (`POST /auth/register`)
  Step 2: Authentication & JWT Issuance (`POST /auth/login`)
  Step 3: Monetization Checkout Session (`POST /billing/checkout`)
  Step 4: Long-Lived API Key Provisioning (`POST /auth/api-keys`)
  Step 5: Point-in-Time Sentiment Analysis via API Key (`GET /sentiment`)
  Step 6: Multi-Asset Portfolio Alpha Backtest (`POST /backtest`)
  Step 7: Streamed Historical Financial CSV Data Export (`GET /export/csv`)

Usage:
  venv/Scripts/python.exe scripts/test_integration_workflow.py
=====================================================================================
"""

import os
from pathlib import Path
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
    """Ensures the FinText Axum API server is running, spawning it locally if necessary."""
    try:
        r = httpx.get(f"{BASE_URL}/health", timeout=1.5)
        if r.status_code == 200:
            print(f"[SERVER] Connected to active FinText API server at {BASE_URL}")
            return True, None
    except Exception:
        pass

    print(f"[SERVER] Spawning local FinText API Server at {BASE_URL}...")
    env = os.environ.copy()
    env.update({
        "PORT": str(DEFAULT_PORT),
        "HOST": "127.0.0.1",
        "ADMIN_TOKEN": ADMIN_TOKEN,
        "JWT_SECRET": "super_secret_test_jwt_key_32_bytes_len!!",
        "RATE_LIMIT_REQUESTS": "10000",
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


def request_with_retry(
    client: httpx.Client,
    method: str,
    endpoint: str,
    max_retries: int = 3,
    **kwargs,
) -> httpx.Response:
    """Executes an HTTP request with automatic retry on HTTP 429 Rate Limit responses."""
    for attempt in range(max_retries + 1):
        res = client.request(method, endpoint, **kwargs)
        if res.status_code == 429 and attempt < max_retries:
            print(f"    [RATE LIMIT] Encountered 429 on {endpoint}, waiting 2.0s (attempt {attempt+1}/{max_retries})...")
            time.sleep(2.0)
            continue
        return res
    return res


def run_integration_workflow() -> bool:
    """Executes the full step-by-step user journey integration test."""
    print("\n" + "=" * 90)
    print(" FINTEXT ALPHA VECTORIZER — END-TO-END USER JOURNEY INTEGRATION TEST")
    print("=" * 90)
    print(f" Target Server Base URL: {BASE_URL}\n")

    server_ok, server_proc = ensure_server_running()
    if not server_ok:
        print("[ERROR] API Server could not be reached or started.")
        return False

    client = httpx.Client(base_url=BASE_URL, timeout=15.0)

    # Unique test user credentials to guarantee test idempotency
    user_suffix = uuid.uuid4().hex[:8]
    user_email = f"integration_{user_suffix}@example.com"
    user_password = f"IntegrationPass123!_{user_suffix[:4]}"

    jwt_token = None
    api_key_str = None
    steps_passed = 0
    total_steps = 7

    try:
        # ─────────────────────────────────────────────────────────────────────────────
        # STEP 1: User Signup (POST /auth/register)
        # ─────────────────────────────────────────────────────────────────────────────
        print(f"[STEP 1/7] User Signup — Registering new user account: {user_email}")
        reg_payload = {"email": user_email, "password": user_password}
        res_reg = request_with_retry(client, "POST", "/auth/register", json=reg_payload)

        if res_reg.status_code not in [200, 201]:
            print(f"  [FAIL] Registration failed with status {res_reg.status_code}: {res_reg.text}")
            return False

        reg_data = res_reg.json()
        user_id = reg_data.get("user_id") or reg_data.get("id")
        print(f"  [PASS] User successfully registered. User ID: {user_id}")
        steps_passed += 1

        # ─────────────────────────────────────────────────────────────────────────────
        # STEP 2: Authentication & Login (POST /auth/login)
        # ─────────────────────────────────────────────────────────────────────────────
        print(f"\n[STEP 2/7] Authentication & Login — Logging in with user credentials")
        login_payload = {"email": user_email, "password": user_password}
        res_login = request_with_retry(client, "POST", "/auth/login", json=login_payload)

        if res_login.status_code != 200:
            print(f"  [FAIL] Login failed with status {res_login.status_code}: {res_login.text}")
            return False

        login_data = res_login.json()
        jwt_token = login_data.get("token")
        if not jwt_token:
            print(f"  [FAIL] Login response did not contain JWT 'token': {login_data}")
            return False

        jwt_headers = {"Authorization": f"Bearer {jwt_token}"}
        print(f"  [PASS] Logged in successfully. Received JWT Token (prefix: {jwt_token[:16]}...)")
        steps_passed += 1

        # ─────────────────────────────────────────────────────────────────────────────
        # STEP 3: Subscription Checkout (POST /billing/checkout)
        # ─────────────────────────────────────────────────────────────────────────────
        print(f"\n[STEP 3/7] Subscription Checkout — Upgrading to 'pro_monthly' Plan")
        checkout_payload = {
            "plan_id": "pro_monthly",
            "success_url": "https://example.com/checkout/success",
            "cancel_url": "https://example.com/checkout/cancel",
        }
        res_checkout = request_with_retry(
            client, "POST", "/billing/checkout", json=checkout_payload, headers=jwt_headers
        )

        if res_checkout.status_code != 200:
            print(f"  [FAIL] Checkout session creation failed with status {res_checkout.status_code}: {res_checkout.text}")
            return False

        checkout_data = res_checkout.json()
        checkout_url = checkout_data.get("checkout_url", "N/A")
        session_id = checkout_data.get("session_id", "N/A")
        print(f"  [PASS] Checkout session created. Session ID: {session_id}, URL: {checkout_url}")
        steps_passed += 1

        # ─────────────────────────────────────────────────────────────────────────────
        # STEP 4: API Key Generation (POST /auth/api-keys)
        # ─────────────────────────────────────────────────────────────────────────────
        print(f"\n[STEP 4/7] API Key Provisioning — Generating long-lived programmatic API key")
        key_payload = {"name": f"Integration Key {user_suffix}"}
        res_key = request_with_retry(client, "POST", "/auth/api-keys", json=key_payload, headers=jwt_headers)

        if res_key.status_code not in [200, 201]:
            print(f"  [FAIL] API key creation failed with status {res_key.status_code}: {res_key.text}")
            return False

        key_data = res_key.json()
        api_key_str = key_data.get("api_key")
        key_id = key_data.get("id")
        if not api_key_str:
            print(f"  [FAIL] API key response missing 'api_key' field: {key_data}")
            return False

        api_key_headers = {"X-API-Key": api_key_str}
        print(f"  [PASS] API key generated. Key ID: {key_id}, Prefix: {key_data.get('prefix')}")
        steps_passed += 1

        # ─────────────────────────────────────────────────────────────────────────────
        # STEP 5: Sentiment Analysis (GET /sentiment?ticker=AAPL)
        # ─────────────────────────────────────────────────────────────────────────────
        print(f"\n[STEP 5/7] Sentiment Analysis — Querying real-time sentiment for AAPL using API Key")
        res_sentiment = request_with_retry(
            client, "GET", "/sentiment", params={"ticker": "AAPL"}, headers=api_key_headers
        )

        if res_sentiment.status_code != 200:
            print(f"  [FAIL] Sentiment query failed with status {res_sentiment.status_code}: {res_sentiment.text}")
            return False

        sent_data = res_sentiment.json()
        if "sentiment_score" not in sent_data or "sentiment_label" not in sent_data:
            print(f"  [FAIL] Sentiment response missing 'sentiment_score' or 'sentiment_label': {sent_data}")
            return False

        print(
            f"  [PASS] Sentiment retrieved: Ticker={sent_data.get('ticker')}, "
            f"Score={sent_data.get('sentiment_score')}, Label={sent_data.get('sentiment_label')}, "
            f"Confidence={sent_data.get('confidence')}"
        )
        steps_passed += 1

        # ─────────────────────────────────────────────────────────────────────────────
        # STEP 6: Quantitative Alpha Backtest (POST /backtest)
        # ─────────────────────────────────────────────────────────────────────────────
        print(f"\n[STEP 6/7] Quantitative Backtest — Running multi-asset portfolio simulation (AAPL, MSFT, NVDA)")
        backtest_payload = {
            "tickers": ["AAPL", "MSFT", "NVDA"],
            "start_date": "2025-01-01",
            "end_date": "2025-03-31",
            "long_threshold": 0.2,
            "short_threshold": -0.2,
            "holding_days": 5,
            "initial_capital": 1000000.0,
        }
        res_backtest = request_with_retry(
            client, "POST", "/backtest", json=backtest_payload, headers=jwt_headers
        )

        if res_backtest.status_code != 200:
            print(f"  [FAIL] Backtest simulation failed with status {res_backtest.status_code}: {res_backtest.text}")
            return False

        bt_data = res_backtest.json()
        if "total_return" not in bt_data or "sharpe_ratio" not in bt_data:
            print(f"  [FAIL] Backtest response missing 'total_return' or 'sharpe_ratio': {bt_data}")
            return False

        total_ret = bt_data.get("total_return")
        sharpe = bt_data.get("sharpe_ratio")
        equity_pts = len(bt_data.get("equity_curve", []))
        print(
            f"  [PASS] Backtest completed: Total Return={total_ret:.4f}, "
            f"Sharpe Ratio={sharpe:.4f}, Equity Curve Points={equity_pts}"
        )
        steps_passed += 1

        # ─────────────────────────────────────────────────────────────────────────────
        # STEP 7: Streamed CSV Export (GET /export/csv)
        # ─────────────────────────────────────────────────────────────────────────────
        print(f"\n[STEP 7/7] Historical Data CSV Export — Exporting Jan-Mar 2025 sentiment for AAPL")
        csv_params = {
            "ticker": "AAPL",
            "start_date": "2025-01-01",
            "end_date": "2025-03-31",
        }
        csv_headers = {
            "Authorization": f"Bearer {jwt_token}",
            "Accept": "text/csv",
        }
        res_csv = request_with_retry(
            client, "GET", "/export/csv", params=csv_params, headers=csv_headers
        )

        if res_csv.status_code != 200:
            print(f"  [FAIL] CSV export failed with status {res_csv.status_code}: {res_csv.text}")
            return False

        csv_text = res_csv.text
        first_line = csv_text.splitlines()[0] if csv_text.splitlines() else ""
        if not (first_line.startswith("published_utc,ticker") or ("published_utc" in first_line and "ticker" in first_line)):
            print(f"  [FAIL] CSV header line does not contain expected columns 'published_utc,ticker':\n{first_line}")
            return False

        line_count = len(csv_text.splitlines())
        print(f"  [PASS] CSV export verified: Header='{first_line}', Total Lines Streamed={line_count}")
        steps_passed += 1

        # ─────────────────────────────────────────────────────────────────────────────
        # Final Summary
        # ─────────────────────────────────────────────────────────────────────────────
        print("\n" + "=" * 90)
        print(" FINTEXT ALPHA VECTORIZER — INTEGRATION TEST EXECUTION SUMMARY")
        print("=" * 90)
        print(f" Steps Executed: {total_steps}")
        print(f" Steps Passed:   {steps_passed}")
        print(f" Steps Failed:   {total_steps - steps_passed}")
        print(f" User Journey:   Signup -> Login -> Checkout -> API Key -> Sentiment -> Backtest -> CSV")
        print(" Final Verdict:  ALL STEPS PASSED [SUCCESS]")
        print("=" * 90)
        return True

    except Exception as exc:
        print(f"\n[EXCEPTION] Unexpected test failure during workflow execution: {exc}")
        import traceback
        traceback.print_exc()
        return False

    finally:
        client.close()
        if server_proc and server_proc.poll() is None:
            print("\n[SERVER] Terminating test server process...")
            server_proc.terminate()
            try:
                server_proc.wait(timeout=3.0)
            except Exception:
                server_proc.kill()


def main():
    success = run_integration_workflow()
    sys.exit(0 if success else 1)


if __name__ == "__main__":
    main()
