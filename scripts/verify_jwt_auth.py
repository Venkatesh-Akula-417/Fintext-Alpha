#!/usr/bin/env python3
"""
═══════════════════════════════════════════════════════════════════════════════
FinText-Alpha-Vectorizer — Live API JWT Authentication Verification Suite
═══════════════════════════════════════════════════════════════════════════════
Validates all REST and WebSocket endpoints:
1. GET /health (Public) -> 200 OK
2. GET /sentiment (Protected without token) -> 401 Unauthorized
3. POST /auth/token (Without X-Admin-Token) -> 401 Unauthorized
4. POST /auth/token (With valid X-Admin-Token) -> 200 OK + JWT
5. GET /sentiment (With Bearer JWT) -> 200 OK
6. GET /spillovers (With Bearer JWT) -> 200 OK
7. POST /backtest (With Bearer JWT) -> 200 OK
8. Token expiration validation (Short-lived token expired) -> 401 Unauthorized
═══════════════════════════════════════════════════════════════════════════════
"""

import os
import sys
import time
import subprocess
import requests
from pathlib import Path

if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")


def main():
    print("=" * 85)
    print(" FinText-Alpha-Vectorizer — Live JWT Authentication Verification Suite")
    print("=" * 85)

    env = os.environ.copy()
    env.update({
        "PORT": "8009",
        "HOST": "127.0.0.1",
        "RUST_LOG": "info",
        "JWT_SECRET": "verification-test-jwt-secret-99999",
        "ADMIN_TOKEN": "verification-admin-token-11111",
        "QUESTDB_MOCK_FALLBACK": "1",
        "NATS_MOCK_MODE": "1",
    })

    bin_path = Path("rust/target/release/fintext_api.exe").resolve()
    if not bin_path.exists():
        bin_path = Path("rust/target/debug/fintext_api.exe").resolve()

    print(f"[*] Starting API Server binary: {bin_path}")
    proc = subprocess.Popen(
        [str(bin_path)],
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        encoding="utf-8",
        errors="replace",
    )
    time.sleep(2.0)

    base_url = "http://127.0.0.1:8009"

    try:
        # 1. Public Health Check
        print("\n[1/8] Testing GET /health (Public Probe)...")
        r = requests.get(f"{base_url}/health", timeout=5)
        print(f"      HTTP Status: {r.status_code} | Body: {r.json()}")
        assert r.status_code == 200, f"Expected 200, got {r.status_code}"

        # 2. Protected Sentiment without Token
        print("\n[2/8] Testing GET /sentiment without Authorization Header (Should Fail)...")
        r = requests.get(f"{base_url}/sentiment?ticker=AAPL", timeout=5)
        print(f"      HTTP Status: {r.status_code} | Body: {r.json()}")
        assert r.status_code == 401, f"Expected 401, got {r.status_code}"

        # 3. Token Issuance without Admin Token
        print("\n[3/8] Testing POST /auth/token without Admin Token (Should Fail)...")
        r = requests.post(f"{base_url}/auth/token", json={"user_id": "quant_trader_1"}, timeout=5)
        print(f"      HTTP Status: {r.status_code} | Body: {r.json()}")
        assert r.status_code == 401, f"Expected 401, got {r.status_code}"

        # 4. Token Issuance with Valid Admin Token
        print("\n[4/8] Testing POST /auth/token with Valid X-Admin-Token...")
        r = requests.post(
            f"{base_url}/auth/token",
            headers={"X-Admin-Token": "verification-admin-token-11111"},
            json={"user_id": "quant_trader_1", "expires_in_seconds": 3600, "role": "institutional"},
            timeout=5,
        )
        print(f"      HTTP Status: {r.status_code}")
        token_data = r.json()
        token = token_data["token"]
        masked_token = f"{token[:12]}...{token[-12:]}"
        print(f"      Issued Token: {masked_token} (expires in: {token_data['expires_in']}s, role: {token_data['role']})")
        assert r.status_code == 200, f"Expected 200, got {r.status_code}"

        # 5. Protected Sentiment with Valid Bearer Token
        print("\n[5/8] Testing GET /sentiment with Valid Bearer Token...")
        r = requests.get(
            f"{base_url}/sentiment?ticker=AAPL",
            headers={"Authorization": f"Bearer {token}"},
            timeout=5,
        )
        print(f"      HTTP Status: {r.status_code} | Data: {r.json()}")
        assert r.status_code == 200, f"Expected 200, got {r.status_code}"

        # 6. Protected Spillovers with Valid Bearer Token
        print("\n[6/8] Testing GET /spillovers with Valid Bearer Token...")
        r = requests.get(
            f"{base_url}/spillovers?ticker=AAPL",
            headers={"Authorization": f"Bearer {token}"},
            timeout=5,
        )
        print(f"      HTTP Status: {r.status_code} | Spillover Count: {r.json().get('count')}")
        assert r.status_code == 200, f"Expected 200, got {r.status_code}"

        # 7. Protected Backtest with Valid Bearer Token
        print("\n[7/8] Testing POST /backtest with Valid Bearer Token...")
        bt_payload = {
            "ticker": "AAPL",
            "start_date": "2025-01-01",
            "end_date": "2025-03-31",
            "long_threshold": 0.2,
            "short_threshold": -0.2,
            "holding_days": 5,
            "initial_capital": 1000000.0,
        }
        r = requests.post(
            f"{base_url}/backtest",
            headers={"Authorization": f"Bearer {token}"},
            json=bt_payload,
            timeout=5,
        )
        print(f"      HTTP Status: {r.status_code} | Backtest Trades: {r.json().get('num_trades')}")
        assert r.status_code == 200, f"Expected 200, got {r.status_code}"

        # 8. Token Expiration Enforcement
        print("\n[8/8] Testing Token Expiration Enforcement (Short-Lived Token)...")
        r_short = requests.post(
            f"{base_url}/auth/token",
            headers={"X-Admin-Token": "verification-admin-token-11111"},
            json={"user_id": "quick_expiry_user", "expires_in_seconds": 1},
            timeout=5,
        )
        short_token = r_short.json()["token"]
        print("      Waiting 7 seconds for token expiration and leeway window...")
        time.sleep(7.0)

        r_expired = requests.get(
            f"{base_url}/sentiment?ticker=AAPL",
            headers={"Authorization": f"Bearer {short_token}"},
            timeout=5,
        )
        print(f"      HTTP Status: {r_expired.status_code} | Body: {r_expired.json()}")
        assert r_expired.status_code == 401, f"Expected 401, got {r_expired.status_code}"
        assert "expired" in r_expired.json().get("message", "").lower()

        print("\n" + "=" * 85)
        print(" [OK] ALL 8 JWT AUTHENTICATION END-TO-END VERIFICATION CHECKS PASSED!")
        print("=" * 85)

    finally:
        proc.terminate()
        proc.wait(timeout=5)


if __name__ == "__main__":
    main()
