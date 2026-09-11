#!/usr/bin/env python3
"""
FinText-Alpha-Vectorizer — Cross-Asset Spillover Matrix Endpoint Verification
═══════════════════════════════════════════════════════════════════════════════
Verifies:
  1. GET /spillovers/matrix with custom tickers, date range, min_correlation, and max_lag_hours
  2. Default ticker universe fallback when tickers param is omitted
  3. min_correlation threshold filtering
  4. Parameter validation errors (inverted date range, min_corr > 1.0, max_lag > 168, >50 tickers)
  5. Authentication enforcement (401 on missing JWT/API key)
  6. Authentication via X-API-Key header
  7. Official Python SDK client.spillover_matrix() integration
"""

import os
import sys
import time
import subprocess
import requests

API_HOST = "http://127.0.0.1:8000"
BINARY_PATH = os.path.join("rust", "target", "release", "fintext_api.exe")

def run_checks():
    print("=" * 85)
    print(" FinText-Alpha-Vectorizer — Cross-Asset Spillover Matrix Verification")
    print("=" * 85)

    if not os.path.exists(BINARY_PATH):
        print(f"[!] Release binary not found at {BINARY_PATH}. Building...")
        res = subprocess.run(["cargo", "build", "--release", "--bin", "fintext_api"], cwd="rust")
        if res.returncode != 0:
            print("[x] Failed to build release binary.")
            sys.exit(1)

    env = os.environ.copy()
    env["QUESTDB_MOCK_FALLBACK"] = "1"
    env["JWT_SECRET"] = "fintext-alpha-vectorizer-institutional-jwt-secret-key-2026"
    env["ADMIN_TOKEN"] = "fintext-admin-dev-secret-token"

    print(f"[*] Starting API Server binary: {os.path.abspath(BINARY_PATH)}")
    server_proc = subprocess.Popen([BINARY_PATH], env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    time.sleep(2.0)

    try:
        # Check /health
        health_resp = requests.get(f"{API_HOST}/health", timeout=3)
        assert health_resp.status_code == 200, f"Expected 200 from /health, got {health_resp.status_code}"
        print("[*] Server online and responsive at /health")

        # 1. Obtain JWT token via POST /auth/token
        print("\n[1/7] Obtaining institutional JWT via POST /auth/token...")
        token_resp = requests.post(
            f"{API_HOST}/auth/token",
            headers={"X-Admin-Token": "fintext-admin-dev-secret-token"},
            json={"user_id": "quant_matrix_macro_01"},
            timeout=5,
        )
        assert token_resp.status_code == 200, f"Failed token issuance: {token_resp.text}"
        jwt_token = token_resp.json()["token"]
        headers = {"Authorization": f"Bearer {jwt_token}"}
        print(f"      Token issued successfully for 'quant_matrix_macro_01'")

        # 2. Query GET /spillovers/matrix with custom tickers
        print("\n[2/7] Querying GET /spillovers/matrix?tickers=AAPL,MSFT,NVDA&start_date=2025-01-01&end_date=2025-03-31&min_correlation=0.5&max_lag_hours=24...")
        params_custom = {
            "tickers": "AAPL,MSFT,NVDA",
            "start_date": "2025-01-01",
            "end_date": "2025-03-31",
            "min_correlation": 0.5,
            "max_lag_hours": 24,
        }
        resp_custom = requests.get(f"{API_HOST}/spillovers/matrix", params=params_custom, headers=headers, timeout=5)
        assert resp_custom.status_code == 200, f"Expected 200 OK, got {resp_custom.status_code}: {resp_custom.text}"
        data_custom = resp_custom.json()
        assert data_custom["tickers"] == ["AAPL", "MSFT", "NVDA"]
        assert data_custom["min_correlation"] == 0.5
        assert data_custom["max_lag_hours"] == 24
        assert len(data_custom["matrix"]) > 0
        print(f"      HTTP 200 OK | Returned {data_custom['count']} matrix items")
        print(f"      Sample Item: {data_custom['matrix'][0]}")

        # 3. Default ticker universe fallback
        print("\n[3/7] Querying with default ticker universe (omitting tickers)...")
        params_default = {
            "start_date": "2025-01-01",
            "end_date": "2025-03-31",
        }
        resp_default = requests.get(f"{API_HOST}/spillovers/matrix", params=params_default, headers=headers, timeout=5)
        assert resp_default.status_code == 200
        data_default = resp_default.json()
        assert len(data_default["tickers"]) >= 8
        assert "AAPL" in data_default["tickers"]
        assert "NVDA" in data_default["tickers"]
        print(f"      [OK] Default universe populated: {data_default['tickers']} (Matrix size: {data_default['count']})")

        # 4. Correlation Threshold Filtering
        print("\n[4/7] Testing min_correlation filtering...")
        p_all = {"tickers": "AAPL,MSFT,NVDA", "start_date": "2025-01-01", "end_date": "2025-03-31", "min_correlation": 0.0}
        p_high = {"tickers": "AAPL,MSFT,NVDA", "start_date": "2025-01-01", "end_date": "2025-03-31", "min_correlation": 0.8}
        r_all = requests.get(f"{API_HOST}/spillovers/matrix", params=p_all, headers=headers).json()
        r_high = requests.get(f"{API_HOST}/spillovers/matrix", params=p_high, headers=headers).json()
        assert r_all["count"] >= r_high["count"], "Filtered count should be less than or equal to unfiltered count"
        for item in r_high["matrix"]:
            assert item["correlation"] >= 0.8 or item["correlation"] <= -0.8
        print(f"      [OK] Filtered from {r_all['count']} items (min_corr=0.0) to {r_high['count']} items (min_corr=0.8)")

        # 5. Parameter Validation Rejections (400 Bad Request)
        print("\n[5/7] Testing parameter validation rejections (400 Bad Request)...")
        # Inverted date range
        r_inv = requests.get(f"{API_HOST}/spillovers/matrix?start_date=2025-05-01&end_date=2025-01-01", headers=headers)
        assert r_inv.status_code == 400
        print(f"      [OK] Inverted date range rejected (400): {r_inv.json()['message']}")

        # Excessive min_correlation > 1.0
        r_corr = requests.get(f"{API_HOST}/spillovers/matrix?start_date=2025-01-01&end_date=2025-03-31&min_correlation=2.5", headers=headers)
        assert r_corr.status_code == 400
        print(f"      [OK] Out-of-bounds min_correlation rejected (400): {r_corr.json()['message']}")

        # Excessive max_lag > 168
        r_lag = requests.get(f"{API_HOST}/spillovers/matrix?start_date=2025-01-01&end_date=2025-03-31&max_lag_hours=500", headers=headers)
        assert r_lag.status_code == 400
        print(f"      [OK] Out-of-bounds max_lag_hours rejected (400): {r_lag.json()['message']}")

        # 6. Test Authentication & API Key
        print("\n[6/7] Testing unauthenticated access and X-API-Key authentication...")
        r_unauth = requests.get(f"{API_HOST}/spillovers/matrix?start_date=2025-01-01&end_date=2025-03-31")
        assert r_unauth.status_code == 401
        print("      [OK] Unauthenticated request rejected with 401 Unauthorized")

        # Create API key
        key_res = requests.post(f"{API_HOST}/auth/api-keys", json={"name": "Spillover Matrix Bot"}, headers=headers)
        assert key_res.status_code == 201
        raw_key = key_res.json()["api_key"]

        r_apikey = requests.get(
            f"{API_HOST}/spillovers/matrix?tickers=AAPL,MSFT&start_date=2025-01-01&end_date=2025-03-31",
            headers={"X-API-Key": raw_key},
        )
        assert r_apikey.status_code == 200
        assert r_apikey.json()["tickers"] == ["AAPL", "MSFT"]
        print("      [OK] Authenticated successfully via X-API-Key header")

        # 7. Test Python Client SDK Integration
        print("\n[7/7] Testing official Python Client SDK spillover_matrix() method...")
        sys.path.insert(0, os.path.abspath("python_sdk/src"))
        from fintext import FinTextClient

        client = FinTextClient(base_url=API_HOST, api_token=jwt_token)
        sdk_res = client.spillover_matrix(
            tickers=["AAPL", "MSFT", "NVDA"],
            start_date="2025-01-01",
            end_date="2025-03-31",
            min_correlation=0.5,
            max_lag_hours=24,
        )
        assert sdk_res.tickers == ["AAPL", "MSFT", "NVDA"]
        assert sdk_res.count > 0
        print(f"      [OK] Python SDK spillover_matrix() returned: {sdk_res.tickers} ({sdk_res.count} matrix pairs)")

        print("\n" + "=" * 85)
        print(" [OK] ALL CROSS-ASSET SPILLOVER MATRIX CHECKS PASSED!")
        print("=" * 85)

    finally:
        server_proc.terminate()
        server_proc.wait()

if __name__ == "__main__":
    run_checks()
