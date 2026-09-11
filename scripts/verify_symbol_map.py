#!/usr/bin/env python3
"""
=====================================================================================
FinText-Alpha-Vectorizer — Security Symbol Mapping Service Verification
=====================================================================================
Validates that:
 1. POST /auth/token issues valid institutional JWT.
 2. GET /symbols/map?identifier=AAPL&output_type=all returns full cross-mapping.
 3. GET /symbols/map?identifier=US0378331005&input_type=isin&output_type=ticker resolves to AAPL.
 4. GET /symbols/map?identifier=BBG000BBJQV0 (FIGI) resolves to NVDA.
 5. GET /symbols/map?identifier=594918104 (CUSIP) resolves to MSFT.
 6. Single output_type filtering works (e.g. output_type=figi returns only figi).
 7. Non-existent identifier returns 404 Not Found with descriptive error message.
 8. Invalid input_type or output_type returns 400 Bad Request.
 9. Unauthenticated request returns 401 Unauthorized.
 10. Official Python SDK Client (sync & async) executes symbol_map() with structured models.
 11. OpenAPI 3.0 specification documents /symbols/map and SymbolMapResponse schemas.
=====================================================================================
"""

import asyncio
import os
import sys
import time
import subprocess
import httpx as requests

sys.path.insert(0, os.path.abspath("python_sdk/src"))
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
    env["PERMANENT_IDENTIFIERS_PATH"] = os.path.abspath("config/permanent_identifiers.json")
    
    proc = subprocess.Popen(
        [SERVER_EXE],
        env=env,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
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
    print(" FinText-Alpha-Vectorizer — Security Symbol Mapping Service Verification")
    print("=" * 85)
    
    print(f"[*] Starting API Server binary: {SERVER_EXE}")
    proc = start_server()
    try:
        # 1. Obtain JWT
        print("\n[1/11] Obtaining institutional JWT via POST /auth/token...")
        r_auth = requests.post(
            f"{BASE_URL}/auth/token",
            headers={"X-Admin-Token": DEV_ADMIN_TOKEN, "Content-Type": "application/json"},
            json={"user_id": "quant_data_standardization", "tier": "institutional"}
        )
        assert r_auth.status_code == 200, f"Failed auth: {r_auth.text}"
        token = r_auth.json()["token"]
        headers = {"Authorization": f"Bearer {token}"}
        print("       [OK] Token acquired for 'quant_data_standardization'")
        
        # 2. Test Ticker -> All (Auto Type Inference)
        print("\n[2/11] Testing GET /symbols/map?identifier=AAPL&output_type=all (Auto Inference)...")
        r_aapl = requests.get(
            f"{BASE_URL}/symbols/map?identifier=AAPL&output_type=all",
            headers=headers
        )
        assert r_aapl.status_code == 200, f"Expected 200, got {r_aapl.status_code}: {r_aapl.text}"
        data_aapl = r_aapl.json()
        print(f"       Response: {data_aapl}")
        assert data_aapl["input_identifier"] == "AAPL"
        assert data_aapl["input_type"] == "ticker"
        assert data_aapl["output_type"] == "all"
        assert data_aapl["result"]["ticker"] == "AAPL"
        assert data_aapl["result"]["figi"] == "BBG000B9XRY4"
        assert data_aapl["result"]["cusip"] == "037833100"
        assert data_aapl["result"]["isin"] == "US0378331005"
        print("       [OK] Successfully mapped Ticker 'AAPL' -> FIGI, CUSIP, ISIN")
        
        # 3. Test ISIN -> Ticker (Explicit Input Type)
        print("\n[3/11] Testing GET /symbols/map?identifier=US0378331005&input_type=isin&output_type=ticker...")
        r_isin = requests.get(
            f"{BASE_URL}/symbols/map?identifier=US0378331005&input_type=isin&output_type=ticker",
            headers=headers
        )
        assert r_isin.status_code == 200
        d_isin = r_isin.json()
        assert d_isin["input_type"] == "isin"
        assert d_isin["output_type"] == "ticker"
        assert d_isin["result"]["ticker"] == "AAPL"
        assert "figi" not in d_isin["result"] or d_isin["result"]["figi"] is None
        print(f"       [OK] Resolved ISIN 'US0378331005' -> Ticker '{d_isin['result']['ticker']}'")
        
        # 4. Test FIGI -> All (Auto Type Inference)
        print("\n[4/11] Testing GET /symbols/map?identifier=BBG000BBJQV0 (FIGI Auto Inference)...")
        r_figi = requests.get(
            f"{BASE_URL}/symbols/map?identifier=BBG000BBJQV0",
            headers=headers
        )
        assert r_figi.status_code == 200
        d_figi = r_figi.json()
        assert d_figi["input_type"] == "figi"
        assert d_figi["result"]["ticker"] == "NVDA"
        print(f"       [OK] Resolved FIGI 'BBG000BBJQV0' -> Ticker '{d_figi['result']['ticker']}'")
        
        # 5. Test CUSIP -> All
        print("\n[5/11] Testing GET /symbols/map?identifier=594918104 (CUSIP)...")
        r_cusip = requests.get(
            f"{BASE_URL}/symbols/map?identifier=594918104",
            headers=headers
        )
        assert r_cusip.status_code == 200
        d_cusip = r_cusip.json()
        assert d_cusip["result"]["ticker"] == "MSFT"
        print(f"       [OK] Resolved CUSIP '594918104' -> Ticker '{d_cusip['result']['ticker']}'")
        
        # 6. Test Single Output Projection (output_type=figi)
        print("\n[6/11] Testing GET /symbols/map?identifier=TSLA&output_type=figi...")
        r_proj = requests.get(
            f"{BASE_URL}/symbols/map?identifier=TSLA&output_type=figi",
            headers=headers
        )
        assert r_proj.status_code == 200
        d_proj = r_proj.json()
        assert d_proj["result"]["figi"] == "BBG000N9MNX3"
        assert "ticker" not in d_proj["result"] or d_proj["result"]["ticker"] is None
        print(f"       [OK] Single projection output_type=figi: {d_proj['result']}")
        
        # 7. Test Non-Existent Identifier -> 404 Not Found
        print("\n[7/11] Testing non-existent identifier (should return 404)...")
        r_404 = requests.get(
            f"{BASE_URL}/symbols/map?identifier=NONEXISTENT999",
            headers=headers
        )
        assert r_404.status_code == 404, f"Expected 404, got {r_404.status_code}"
        print(f"       [OK] Correctly returned 404 with error: {r_404.json().get('message')}")
        
        # 8. Test Invalid input_type / output_type -> 400 Bad Request
        print("\n[8/11] Testing invalid input_type format (should return 400)...")
        r_bad_type = requests.get(
            f"{BASE_URL}/symbols/map?identifier=AAPL&input_type=invalid_scheme",
            headers=headers
        )
        assert r_bad_type.status_code == 400
        print(f"       [OK] Correctly rejected invalid input_type with 400")
        
        # 9. Test Unauthenticated Request -> 401 Unauthorized
        print("\n[9/11] Testing unauthenticated request (should return 401)...")
        r_unauth = requests.get(f"{BASE_URL}/symbols/map?identifier=AAPL")
        assert r_unauth.status_code == 401
        print(f"       [OK] Correctly rejected unauthenticated call with 401")
        
        # 10. Test Official Python SDK Client (Sync & Async)
        print("\n[10/11] Testing Python SDK client.symbol_map()...")
        sdk = FinTextClient(base_url=BASE_URL, api_token=token)
        sdk_res = sdk.symbol_map(identifier="META", output_type="all")
        assert sdk_res.input_identifier == "META"
        assert sdk_res.result.ticker == "META"
        assert sdk_res.result.figi == "BBG000MM2P62"
        assert sdk_res.result.isin == "US30303M1027"
        print(f"       [OK] Synchronous SDK call succeeded: {sdk_res.input_identifier} -> {sdk_res.result}")
        
        async def run_async():
            async_client = FinTextAsyncClient(base_url=BASE_URL, api_token=token)
            res = await async_client.symbol_map(identifier="US0231351067", input_type="isin", output_type="ticker")
            assert res.result.ticker == "AMZN"
            await async_client.close()
            return res
            
        async_res = asyncio.run(run_async())
        print(f"       [OK] Asynchronous SDK call succeeded: {async_res.input_identifier} -> {async_res.result.ticker}")
        
        # 11. Test OpenAPI 3.0 Spec Documentation
        print("\n[11/11] Checking OpenAPI 3.0 specification for /symbols/map...")
        r_openapi = requests.get(f"{BASE_URL}/api-docs/openapi.json")
        assert r_openapi.status_code == 200
        spec = r_openapi.json()
        assert "/symbols/map" in spec["paths"]
        assert "SymbolMapResponse" in spec["components"]["schemas"]
        assert "SecurityIdentifiers" in spec["components"]["schemas"]
        print("       [OK] /symbols/map route and schemas present in OpenAPI spec")
        
    finally:
        proc.terminate()
        proc.wait()

    print("\n" + "=" * 85)
    print(" [OK] ALL SECURITY SYMBOL MAPPING CHECKS PASSED CLEANLY!")
    print("=" * 85)

if __name__ == "__main__":
    main()
