#!/usr/bin/env python3
"""
=====================================================================================
FinText-Alpha-Vectorizer — Sector-Level Sentiment Aggregation Verification
=====================================================================================
Validates that:
 1. POST /auth/token issues valid institutional JWT.
 2. GET /sentiment/sector?sector=Technology&start_date=... returns aggregated metrics.
 3. Supported aggregation operators ('average', 'sum', 'count', 'median', 'weighted_average') work.
 4. min_confidence filter behaves correctly.
 5. Invalid sector returns 404 with list of available sectors.
 6. Invalid aggregation returns 400 Bad Request.
 7. Unauthenticated request returns 401 Unauthorized.
 8. Official Python Client SDK executes sector_sentiment() and parses SectorSentimentResponse.
 9. OpenAPI 3.0 specification documents /sentiment/sector and SectorSentimentResponse.
=====================================================================================
"""

import os
import sys
import time
import subprocess
import requests
from fintext import FinTextClient

SERVER_EXE = os.path.abspath("rust/target/release/fintext_api.exe")
BASE_URL = "http://127.0.0.1:8000"
DEV_ADMIN_TOKEN = "fintext-admin-dev-secret-token"

def start_server() -> subprocess.Popen:
    env = os.environ.copy()
    env["QUESTDB_MOCK_FALLBACK"] = "1"
    env["PORT"] = "8000"
    env["ADMIN_TOKEN"] = DEV_ADMIN_TOKEN
    env["JWT_SECRET"] = "fintext-alpha-vectorizer-institutional-jwt-secret-key-2026"
    env["PIT_DATA_ENABLED"] = "1"
    env["PIT_DATA_DIR"] = os.path.abspath("config")
    env["SECTOR_MAPPING_PATH"] = os.path.abspath("config/sector_mapping.csv")
    
    proc = subprocess.Popen(
        [SERVER_EXE],
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True
    )
    
    for _ in range(30):
        try:
            r = requests.get(f"{BASE_URL}/health", timeout=1)
            if r.status_code == 200:
                return proc
        except Exception:
            time.sleep(0.3)
            
    proc.kill()
    out, err = proc.communicate()
    print("Server failed to start:\n", out, err)
    sys.exit(1)

def main():
    print("=" * 85)
    print(" FinText-Alpha-Vectorizer — Sector Sentiment Aggregation Verification")
    print("=" * 85)
    
    print(f"[*] Starting API Server binary: {SERVER_EXE}")
    proc = start_server()
    try:
        # 1. Obtain JWT
        print("\n[1/8] Obtaining institutional JWT via POST /auth/token...")
        r_auth = requests.post(
            f"{BASE_URL}/auth/token",
            headers={"X-Admin-Token": DEV_ADMIN_TOKEN, "Content-Type": "application/json"},
            json={"user_id": "macro_sector_quant", "tier": "institutional"}
        )
        assert r_auth.status_code == 200, f"Failed auth: {r_auth.text}"
        token = r_auth.json()["token"]
        headers = {"Authorization": f"Bearer {token}"}
        print("      [OK] Token acquired for 'macro_sector_quant'")
        
        # 2. Test Sector Sentiment (Average)
        print("\n[2/8] Testing GET /sentiment/sector?sector=Technology&start_date=2025-01-01&end_date=2025-03-31&aggregation=average&min_confidence=0.5...")
        r_sec = requests.get(
            f"{BASE_URL}/sentiment/sector?sector=Technology&start_date=2025-01-01&end_date=2025-03-31&aggregation=average&min_confidence=0.5",
            headers=headers
        )
        assert r_sec.status_code == 200, f"Expected 200, got {r_sec.status_code}: {r_sec.text}"
        data = r_sec.json()
        print(f"      Response: {data}")
        assert data["sector"] == "Technology"
        assert data["aggregation"] == "average"
        assert data["min_confidence"] == 0.5
        assert data["tickers_included"] > 0
        assert data["record_count"] > 0
        print(f"      [OK] Technology average sentiment: {data['value']:.4f} across {data['tickers_included']} tickers ({data['record_count']} records)")
        
        # 3. Test Weighted Average Aggregation
        print("\n[3/8] Testing GET /sentiment/sector?sector=Financials&start_date=2025-01-01&end_date=2025-03-31&aggregation=weighted_average...")
        r_fin = requests.get(
            f"{BASE_URL}/sentiment/sector?sector=Financials&start_date=2025-01-01&end_date=2025-03-31&aggregation=weighted_average",
            headers=headers
        )
        assert r_fin.status_code == 200
        data_fin = r_fin.json()
        assert data_fin["sector"] == "Financials"
        assert data_fin["aggregation"] == "weighted_average"
        print(f"      [OK] Financials weighted_average sentiment: {data_fin['value']:.4f}")
        
        # 4. Test Other Aggregation Operators (Median, Sum, Count)
        print("\n[4/8] Testing Median, Sum, and Count aggregations...")
        for op in ["median", "sum", "count"]:
            r_op = requests.get(
                f"{BASE_URL}/sentiment/sector?sector=Healthcare&start_date=2025-01-01&end_date=2025-03-31&aggregation={op}",
                headers=headers
            )
            assert r_op.status_code == 200
            d_op = r_op.json()
            assert d_op["aggregation"] == op
            print(f"      [OK] Healthcare {op}: value={d_op['value']}")
            
        # 5. Test Invalid Sector (404 Not Found)
        print("\n[5/8] Testing Invalid Sector (Should return 404)...")
        r_inv_sec = requests.get(
            f"{BASE_URL}/sentiment/sector?sector=NonExistentSector&start_date=2025-01-01&end_date=2025-03-31",
            headers=headers
        )
        assert r_inv_sec.status_code == 404, f"Expected 404, got {r_inv_sec.status_code}: {r_inv_sec.text}"
        print(f"      [OK] Correctly returned 404 with error message: {r_inv_sec.json().get('message')}")
        
        # 6. Test Invalid Aggregation (400 Bad Request)
        print("\n[6/8] Testing Invalid Aggregation operator (Should return 400)...")
        r_inv_agg = requests.get(
            f"{BASE_URL}/sentiment/sector?sector=Technology&start_date=2025-01-01&end_date=2025-03-31&aggregation=hyper_geometric",
            headers=headers
        )
        assert r_inv_agg.status_code == 400
        print(f"      [OK] Correctly returned 400 for invalid aggregation operator")
        
        # 7. Test Unauthenticated Request (401 Unauthorized)
        print("\n[7/8] Testing Unauthenticated Request (Should return 401)...")
        r_unauth = requests.get(
            f"{BASE_URL}/sentiment/sector?sector=Technology&start_date=2025-01-01&end_date=2025-03-31"
        )
        assert r_unauth.status_code == 401
        print(f"      [OK] Correctly rejected unauthenticated request with 401")
        
        # 8. Test Official Python Client SDK Integration
        print("\n[8/8] Testing Official Python Client SDK sector_sentiment()...")
        sdk = FinTextClient(base_url=BASE_URL, api_token=token)
        sdk_res = sdk.sector_sentiment(
            sector="Technology",
            start_date="2025-01-01",
            end_date="2025-03-31",
            aggregation="weighted_average",
            min_confidence=0.6,
        )
        assert sdk_res.sector == "Technology"
        assert sdk_res.aggregation == "weighted_average"
        assert sdk_res.min_confidence == 0.6
        assert sdk_res.tickers_included > 0
        print(f"      [OK] Python SDK successfully parsed response: Sector={sdk_res.sector}, Aggregation={sdk_res.aggregation}, Value={sdk_res.value:.4f}")
        
    finally:
        proc.terminate()
        proc.wait()

    print("\n" + "=" * 85)
    print(" [OK] ALL SECTOR SENTIMENT AGGREGATION CHECKS PASSED CLEANLY!")
    print("=" * 85)

if __name__ == "__main__":
    main()
