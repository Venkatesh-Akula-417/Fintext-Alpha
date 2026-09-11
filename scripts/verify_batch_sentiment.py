#!/usr/bin/env python3
"""
=====================================================================================
FinText-Alpha-Vectorizer — Batch Multi-Ticker Sentiment Verification
=====================================================================================
Validates that:
 1. POST /auth/token issues valid institutional JWT.
 2. GET /sentiment/batch?tickers=AAPL,MSFT,NVDA returns 3 sentiment signals in a single call.
 3. Historical batch query with date (e.g. date=2025-01-15) returns correct signals.
 4. Excessive ticker batch (> 50) is rejected with 400 Bad Request.
 5. Empty or whitespace ticker list is rejected with 400 Bad Request.
 6. Invalid ticker format is rejected with 400 Bad Request.
 7. Unauthenticated request returns 401 Unauthorized.
 8. Official Python Client SDK executes batch_sentiment() with list & string inputs.
 9. Official Async Python Client SDK executes batch_sentiment() with async/await.
 10. OpenAPI 3.0 specification documents /sentiment/batch and BatchSentimentResponse.
=====================================================================================
"""

import asyncio
import os
import sys
import time
import subprocess
import requests
from fintext import FinTextClient, FinTextAsyncClient

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
    print(" FinText-Alpha-Vectorizer — Batch Multi-Ticker Sentiment Verification")
    print("=" * 85)
    
    print(f"[*] Starting API Server binary: {SERVER_EXE}")
    proc = start_server()
    try:
        # 1. Obtain JWT
        print("\n[1/10] Obtaining institutional JWT via POST /auth/token...")
        r_auth = requests.post(
            f"{BASE_URL}/auth/token",
            headers={"X-Admin-Token": DEV_ADMIN_TOKEN, "Content-Type": "application/json"},
            json={"user_id": "portfolio_batch_quant", "tier": "institutional"}
        )
        assert r_auth.status_code == 200, f"Failed auth: {r_auth.text}"
        token = r_auth.json()["token"]
        headers = {"Authorization": f"Bearer {token}"}
        print("       [OK] Token acquired for 'portfolio_batch_quant'")
        
        # 2. Test Batch Sentiment (3 tickers)
        print("\n[2/10] Testing GET /sentiment/batch?tickers=AAPL,MSFT,NVDA...")
        r_batch = requests.get(
            f"{BASE_URL}/sentiment/batch?tickers=AAPL,MSFT,NVDA",
            headers=headers
        )
        assert r_batch.status_code == 200, f"Expected 200, got {r_batch.status_code}: {r_batch.text}"
        data = r_batch.json()
        assert data["count"] == 3
        assert len(data["results"]) == 3
        returned_tickers = [item["ticker"] for item in data["results"]]
        assert "AAPL" in returned_tickers
        assert "MSFT" in returned_tickers
        assert "NVDA" in returned_tickers
        for item in data["results"]:
            assert "confidence" in item
            assert "probabilities" in item
            assert item["confidence"] > 0.0
            print(f"       -> {item['ticker']}: score={item['sentiment_score']}, label={item['sentiment_label']}, confidence={item['confidence']}")
        print("       [OK] Successfully retrieved 3 tickers in single HTTP request")
        
        # 3. Test Historical Batch with Date
        print("\n[3/10] Testing GET /sentiment/batch?tickers=AAPL,GOOGL,TSLA&date=2025-01-15...")
        r_hist = requests.get(
            f"{BASE_URL}/sentiment/batch?tickers=AAPL,GOOGL,TSLA&date=2025-01-15",
            headers=headers
        )
        assert r_hist.status_code == 200
        d_hist = r_hist.json()
        assert d_hist["count"] == 3
        assert d_hist["results"][0]["date"] == "2025-01-15"
        print(f"       [OK] Historical batch date match: date={d_hist['results'][0]['date']}")
        
        # 4. Test Excessive Batch Size (>50 tickers) -> 400 Bad Request
        print("\n[4/10] Testing excessive batch size (55 tickers, should return 400)...")
        tickers_55 = ",".join([f"TICK{i}" for i in range(55)])
        r_too_many = requests.get(
            f"{BASE_URL}/sentiment/batch?tickers={tickers_55}",
            headers=headers
        )
        assert r_too_many.status_code == 400, f"Expected 400, got {r_too_many.status_code}"
        print(f"       [OK] Correctly rejected 55 tickers with 400 Bad Request")
        
        # 5. Test Empty Ticker Parameter -> 400 Bad Request
        print("\n[5/10] Testing empty ticker parameter (should return 400)...")
        r_empty = requests.get(
            f"{BASE_URL}/sentiment/batch?tickers=",
            headers=headers
        )
        assert r_empty.status_code == 400
        print(f"       [OK] Correctly rejected empty tickers parameter with 400")
        
        # 6. Test Invalid Ticker Format -> 400 Bad Request
        print("\n[6/10] Testing invalid ticker format (should return 400)...")
        r_inv = requests.get(
            f"{BASE_URL}/sentiment/batch?tickers=AAPL,INVALID$$$SYMBOL",
            headers=headers
        )
        assert r_inv.status_code == 400
        print(f"       [OK] Correctly rejected invalid ticker symbol with 400")
        
        # 7. Test Unauthenticated Request -> 401 Unauthorized
        print("\n[7/10] Testing unauthenticated request (should return 401)...")
        r_unauth = requests.get(
            f"{BASE_URL}/sentiment/batch?tickers=AAPL,MSFT"
        )
        assert r_unauth.status_code == 401
        print(f"       [OK] Correctly rejected unauthenticated call with 401")
        
        # 8. Test Python SDK Client (List & String input)
        print("\n[8/10] Testing Python SDK client.batch_sentiment()...")
        sdk = FinTextClient(base_url=BASE_URL, api_token=token)
        sdk_res_list = sdk.batch_sentiment(tickers=["AAPL", "MSFT", "AMZN", "META"])
        assert sdk_res_list.count == 4
        assert len(sdk_res_list.results) == 4
        print(f"       [OK] List input: retrieved {sdk_res_list.count} tickers via Python SDK")
        
        sdk_res_str = sdk.batch_sentiment(tickers="JPM,BAC,GS", date="2025-02-01")
        assert sdk_res_str.count == 3
        print(f"       [OK] String input: retrieved {sdk_res_str.count} tickers via Python SDK")
        
        # 9. Test Async Python SDK Client
        print("\n[9/10] Testing Async Python SDK client.batch_sentiment()...")
        async def run_async_test():
            async_client = FinTextAsyncClient(base_url=BASE_URL, api_token=token)
            res = await async_client.batch_sentiment(tickers=["TSLA", "NFLX", "AMD"])
            assert res.count == 3
            assert len(res.results) == 3
            await async_client.close()
            return res
            
        async_res = asyncio.run(run_async_test())
        print(f"       [OK] Async client retrieved {async_res.count} tickers successfully")
        
        # 10. Test OpenAPI 3.0 Spec Documentation
        print("\n[10/10] Checking OpenAPI 3.0 specification for /sentiment/batch...")
        r_openapi = requests.get(f"{BASE_URL}/api-docs/openapi.json")
        assert r_openapi.status_code == 200
        spec = r_openapi.json()
        assert "/sentiment/batch" in spec["paths"]
        assert "BatchSentimentResponse" in spec["components"]["schemas"]
        print("       [OK] /sentiment/batch route and schemas present in OpenAPI spec")
        
    finally:
        proc.terminate()
        proc.wait()

    print("\n" + "=" * 85)
    print(" [OK] ALL BATCH MULTI-TICKER SENTIMENT CHECKS PASSED CLEANLY!")
    print("=" * 85)

if __name__ == "__main__":
    main()
