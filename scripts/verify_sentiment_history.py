#!/usr/bin/env python3
"""
FinText-Alpha-Vectorizer — Historical Sentiment Time Series JSON Endpoint Verification
══════════════════════════════════════════════════════════════════════════════════════
Verifies:
  1. GET /sentiment/history with valid date range, pagination, and sorting
  2. Pagination: Page 1 (offset=0, limit=5) vs Page 2 (offset=5, limit=5)
  3. Sort order: 'asc' vs 'desc'
  4. Error handling (invalid dates, inverted range, limit > 1000, invalid sort)
  5. Authentication enforcement (401 on missing JWT/API key)
  6. Authentication via X-API-Key header
  7. Official Python SDK client.sentiment_history() integration
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
    print(" FinText-Alpha-Vectorizer — Historical Sentiment JSON Endpoint Verification")
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
            json={"user_id": "quant_history_analyst_01"},
            timeout=5,
        )
        assert token_resp.status_code == 200, f"Failed token issuance: {token_resp.text}"
        jwt_token = token_resp.json()["token"]
        headers = {"Authorization": f"Bearer {jwt_token}"}
        print(f"      Token issued successfully for 'quant_history_analyst_01'")

        # 2. Query GET /sentiment/history (Page 1)
        print("\n[2/7] Querying GET /sentiment/history?ticker=AAPL&start_date=2025-01-01&end_date=2025-01-10&limit=5&offset=0&sort=asc...")
        params_p1 = {
            "ticker": "AAPL",
            "start_date": "2025-01-01",
            "end_date": "2025-01-10",
            "limit": 5,
            "offset": 0,
            "sort": "asc",
        }
        resp_p1 = requests.get(f"{API_HOST}/sentiment/history", params=params_p1, headers=headers, timeout=5)
        assert resp_p1.status_code == 200, f"Expected 200 OK, got {resp_p1.status_code}: {resp_p1.text}"
        data_p1 = resp_p1.json()
        assert data_p1["ticker"] == "AAPL"
        assert data_p1["count"] == 5
        assert data_p1["total"] == 10
        assert data_p1["limit"] == 5
        assert data_p1["offset"] == 0
        assert data_p1["sort"] == "asc"
        assert len(data_p1["records"]) == 5
        print(f"      HTTP 200 OK | Count: {data_p1['count']} / Total: {data_p1['total']}")
        print(f"      First Record: {data_p1['records'][0]}")

        # 3. Query GET /sentiment/history (Page 2)
        print("\n[3/7] Querying Page 2 with offset=5...")
        params_p2 = {
            "ticker": "AAPL",
            "start_date": "2025-01-01",
            "end_date": "2025-01-10",
            "limit": 5,
            "offset": 5,
            "sort": "asc",
        }
        resp_p2 = requests.get(f"{API_HOST}/sentiment/history", params=params_p2, headers=headers, timeout=5)
        assert resp_p2.status_code == 200
        data_p2 = resp_p2.json()
        assert data_p2["count"] == 5
        assert data_p2["offset"] == 5
        # Ensure no overlap between page 1 and page 2
        p1_ts = {r["published_utc"] for r in data_p1["records"]}
        p2_ts = {r["published_utc"] for r in data_p2["records"]}
        assert p1_ts.isdisjoint(p2_ts), "Page 1 and Page 2 contain overlapping timestamps!"
        print(f"      [OK] Page 2 non-overlapping: {data_p2['records'][0]['published_utc']} to {data_p2['records'][-1]['published_utc']}")

        # 4. Query with sort=desc
        print("\n[4/7] Querying with sort=desc...")
        params_desc = {
            "ticker": "AAPL",
            "start_date": "2025-01-01",
            "end_date": "2025-01-10",
            "limit": 3,
            "sort": "desc",
        }
        resp_desc = requests.get(f"{API_HOST}/sentiment/history", params=params_desc, headers=headers, timeout=5)
        assert resp_desc.status_code == 200
        data_desc = resp_desc.json()
        assert data_desc["sort"] == "desc"
        assert data_desc["records"][0]["published_utc"] == "2025-01-10T14:30:00.000000Z"
        print(f"      [OK] Descending order verified: Newest timestamp = {data_desc['records'][0]['published_utc']}")

        # 5. Parameter Validation Tests
        print("\n[5/7] Testing parameter validation rejections (400 Bad Request)...")
        # Inverted date range
        r_inv = requests.get(f"{API_HOST}/sentiment/history?ticker=AAPL&start_date=2025-05-01&end_date=2025-01-01", headers=headers)
        assert r_inv.status_code == 400
        print(f"      [OK] Inverted date range rejected (400): {r_inv.json()['message']}")

        # Invalid limit > 1000
        r_lim = requests.get(f"{API_HOST}/sentiment/history?ticker=AAPL&start_date=2025-01-01&end_date=2025-01-05&limit=5000", headers=headers)
        assert r_lim.status_code == 400
        print(f"      [OK] Excessive limit rejected (400): {r_lim.json()['message']}")

        # Invalid sort
        r_sort = requests.get(f"{API_HOST}/sentiment/history?ticker=AAPL&start_date=2025-01-01&end_date=2025-01-05&sort=sideways", headers=headers)
        assert r_sort.status_code == 400
        print(f"      [OK] Invalid sort parameter rejected (400): {r_sort.json()['message']}")

        # 6. Test Authentication Enforcement & API Key Auth
        print("\n[6/7] Testing unauthenticated access and X-API-Key authentication...")
        r_unauth = requests.get(f"{API_HOST}/sentiment/history?ticker=AAPL&start_date=2025-01-01&end_date=2025-01-05")
        assert r_unauth.status_code == 401
        print("      [OK] Unauthenticated request rejected with 401 Unauthorized")

        # Create API key
        key_res = requests.post(f"{API_HOST}/auth/api-keys", json={"name": "History Bot"}, headers=headers)
        assert key_res.status_code == 201
        raw_key = key_res.json()["api_key"]

        # Call with X-API-Key
        r_apikey = requests.get(
            f"{API_HOST}/sentiment/history?ticker=AAPL&start_date=2025-01-01&end_date=2025-01-05&limit=2",
            headers={"X-API-Key": raw_key},
        )
        assert r_apikey.status_code == 200
        assert r_apikey.json()["count"] == 2
        print(f"      [OK] Authenticated successfully with X-API-Key header")

        # 7. Test Python Client SDK Integration
        print("\n[7/7] Testing official Python Client SDK sentiment_history() method...")
        sys.path.insert(0, os.path.abspath("python_sdk/src"))
        from fintext import FinTextClient

        client = FinTextClient(base_url=API_HOST, api_token=jwt_token)
        sdk_res = client.sentiment_history(
            ticker="AAPL",
            start_date="2025-01-01",
            end_date="2025-01-05",
            limit=3,
            offset=0,
            sort="asc",
        )
        assert sdk_res.ticker == "AAPL"
        assert sdk_res.count == 3
        assert len(sdk_res.records) == 3
        print(f"      [OK] Python SDK sentiment_history() returned: {sdk_res.ticker} ({sdk_res.count} records, total: {sdk_res.total})")

        print("\n" + "=" * 85)
        print(" [OK] ALL HISTORICAL SENTIMENT JSON ENDPOINT CHECKS PASSED!")
        print("=" * 85)

    finally:
        server_proc.terminate()
        server_proc.wait()

if __name__ == "__main__":
    run_checks()
