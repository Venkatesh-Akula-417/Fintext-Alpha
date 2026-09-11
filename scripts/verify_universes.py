#!/usr/bin/env python3
"""
=====================================================================================
FinText-Alpha-Vectorizer — Custom Universe Builder Verification
=====================================================================================
Validates that:
 1. POST /auth/token issues valid institutional JWT for User 1 and User 2.
 2. POST /universes creates a new custom universe with tickers (201 Created).
 3. GET /universes lists custom universes for the authenticated user.
 4. GET /universes/{id} returns the specific custom universe.
 5. PUT /universes/{id} updates universe name and constituent tickers.
 6. GET /sentiment/batch?universe_id={id} executes batch sentiment on universe tickers.
 7. GET /sentiment/batch?tickers=NVDA&universe_id={id} returns 400 (exclusivity).
 8. GET /sentiment/batch (without tickers or universe_id) returns 400.
 9. Input validation rejects invalid ticker formats, empty names, and empty ticker arrays.
 10. Multi-user ownership isolation guarantees User 2 cannot read, modify, or delete User 1's universe.
 11. DELETE /universes/{id} removes the universe (200 OK) and subsequent GET returns 404.
 12. Python SDK (sync & async clients) successfully performs all universe operations.
 13. OpenAPI 3.0 specification documents all universe paths and schemas.
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
    raise RuntimeError(f"Server failed to start:\nSTDOUT: {out}\nSTDERR: {err}")

def main():
    print("=" * 80)
    print("FinText-Alpha-Vectorizer: Verifying Custom Universe Builder Endpoint & SDK")
    print("=" * 80)
    
    proc = start_server()
    print(">>> FinText API Server started on http://127.0.0.1:8000")
    
    try:
        # 1. Obtain JWT Tokens for two distinct users
        print(">>> 1. Authenticating User 1 ('quant_alpha_fund') and User 2 ('beta_rival_fund')...")
        r1 = requests.post(
            f"{BASE_URL}/auth/token",
            headers={"X-Admin-Token": DEV_ADMIN_TOKEN, "Content-Type": "application/json"},
            json={"user_id": "quant_alpha_fund", "tier": "institutional"}
        )
        assert r1.status_code == 200, f"Token generation failed: {r1.text}"
        token1 = r1.json()["token"]
        headers1 = {"Authorization": f"Bearer {token1}"}

        r2 = requests.post(
            f"{BASE_URL}/auth/token",
            headers={"X-Admin-Token": DEV_ADMIN_TOKEN, "Content-Type": "application/json"},
            json={"user_id": "beta_rival_fund", "tier": "institutional"}
        )
        assert r2.status_code == 200, f"Token generation failed: {r2.text}"
        token2 = r2.json()["token"]
        headers2 = {"Authorization": f"Bearer {token2}"}
        print("    [PASS] Authentication successful.")

        # 2. Create Universe
        print(">>> 2. Creating custom universe for User 1...")
        payload = {
            "name": "Semiconductor Leaders",
            "tickers": ["NVDA", "TSM", "ASML", "AMD", "QCOM"]
        }
        res_create = requests.post(f"{BASE_URL}/universes", json=payload, headers=headers1)
        assert res_create.status_code == 201, f"Create universe failed: {res_create.text}"
        u_data = res_create.json()
        u_id = u_data["id"]
        assert u_data["name"] == "Semiconductor Leaders"
        assert set(u_data["tickers"]) == {"NVDA", "TSM", "ASML", "AMD", "QCOM"}
        assert u_data["user_id"] == "quant_alpha_fund"
        print(f"    [PASS] Universe created with ID: {u_id}")

        # 3. List Universes
        print(">>> 3. Listing universes for User 1...")
        res_list = requests.get(f"{BASE_URL}/universes", headers=headers1)
        assert res_list.status_code == 200
        list_json = res_list.json()
        assert list_json["count"] >= 1
        assert any(u["id"] == u_id for u in list_json["universes"])
        print(f"    [PASS] List returned {list_json['count']} universe(s).")

        # 4. Get Universe by ID
        print(">>> 4. Fetching universe by ID...")
        res_get = requests.get(f"{BASE_URL}/universes/{u_id}", headers=headers1)
        assert res_get.status_code == 200
        assert res_get.json()["id"] == u_id
        print("    [PASS] Universe retrieved successfully.")

        # 5. Update Universe
        print(">>> 5. Updating universe name and tickers...")
        up_payload = {
            "name": "Global Semiconductor Giants",
            "tickers": ["NVDA", "TSM", "ASML", "AMD", "QCOM", "AVGO", "INTC"]
        }
        res_up = requests.put(f"{BASE_URL}/universes/{u_id}", json=up_payload, headers=headers1)
        assert res_up.status_code == 200
        up_data = res_up.json()
        assert up_data["name"] == "Global Semiconductor Giants"
        assert len(up_data["tickers"]) == 7
        print("    [PASS] Universe updated successfully.")

        # 6. Batch Sentiment via universe_id
        print(">>> 6. Querying GET /sentiment/batch?universe_id=...")
        res_batch = requests.get(f"{BASE_URL}/sentiment/batch?universe_id={u_id}", headers=headers1)
        assert res_batch.status_code == 200, f"Batch sentiment failed: {res_batch.text}"
        batch_json = res_batch.json()
        assert batch_json["count"] == 7
        tickers_returned = [item["ticker"] for item in batch_json["results"]]
        assert "NVDA" in tickers_returned and "INTC" in tickers_returned
        print(f"    [PASS] Batch sentiment returned {batch_json['count']} constituent signals.")

        # 7. Parameter exclusivity checks
        print(">>> 7. Testing parameter validation & exclusivity on /sentiment/batch...")
        res_conflict = requests.get(
            f"{BASE_URL}/sentiment/batch?tickers=NVDA&universe_id={u_id}",
            headers=headers1
        )
        assert res_conflict.status_code == 400
        assert "not both" in res_conflict.text.lower()

        res_missing = requests.get(f"{BASE_URL}/sentiment/batch", headers=headers1)
        assert res_missing.status_code == 400
        assert "either" in res_missing.text.lower()
        print("    [PASS] Exclusivity rules properly enforced.")

        # 8. Input validation on /universes
        print(">>> 8. Testing universe input validation...")
        res_empty_name = requests.post(f"{BASE_URL}/universes", json={"name": "", "tickers": ["AAPL"]}, headers=headers1)
        assert res_empty_name.status_code == 400

        res_empty_tickers = requests.post(f"{BASE_URL}/universes", json={"name": "Test", "tickers": []}, headers=headers1)
        assert res_empty_tickers.status_code == 400

        res_invalid_ticker = requests.post(f"{BASE_URL}/universes", json={"name": "Test", "tickers": ["INVALID$$$"]}, headers=headers1)
        assert res_invalid_ticker.status_code == 400
        print("    [PASS] Validation rejects invalid names and ticker strings.")

        # 9. Multi-user isolation
        print(">>> 9. Verifying multi-user ownership isolation...")
        res_other_get = requests.get(f"{BASE_URL}/universes/{u_id}", headers=headers2)
        assert res_other_get.status_code == 404

        res_other_put = requests.put(f"{BASE_URL}/universes/{u_id}", json={"name": "Hacked"}, headers=headers2)
        assert res_other_put.status_code == 404

        res_other_del = requests.delete(f"{BASE_URL}/universes/{u_id}", headers=headers2)
        assert res_other_del.status_code == 404

        res_other_batch = requests.get(f"{BASE_URL}/sentiment/batch?universe_id={u_id}", headers=headers2)
        assert res_other_batch.status_code == 404
        print("    [PASS] User 2 completely isolated from User 1's universes.")

        # 10. Delete Universe
        print(">>> 10. Deleting universe...")
        res_del = requests.delete(f"{BASE_URL}/universes/{u_id}", headers=headers1)
        assert res_del.status_code == 200
        assert res_del.json()["status"] == "deleted"

        res_verify_del = requests.get(f"{BASE_URL}/universes/{u_id}", headers=headers1)
        assert res_verify_del.status_code == 404
        print("    [PASS] Universe deleted and 404 confirmed.")

        # 11. Python SDK Verification (Sync)
        print(">>> 11. Testing Python SDK (Sync Client)...")
        with FinTextClient(base_url=BASE_URL, api_token=token1) as client:
            sdk_u = client.create_universe(name="SDK Tech Watchlist", tickers=["MSFT", "AAPL", "GOOGL"])
            assert sdk_u.name == "SDK Tech Watchlist"
            assert len(sdk_u.tickers) == 3

            sdk_list = client.list_universes()
            assert any(u.id == sdk_u.id for u in sdk_list.universes)

            sdk_batch = client.batch_sentiment(universe_id=sdk_u.id)
            assert sdk_batch.count == 3

            client.delete_universe(sdk_u.id)
        print("    [PASS] Sync Python SDK passed all assertions.")

        # 12. Python SDK Verification (Async)
        print(">>> 12. Testing Python SDK (Async Client)...")
        async def run_async():
            client = FinTextAsyncClient(base_url=BASE_URL, api_token=token1)
            try:
                sdk_u = await client.create_universe(name="Async Watchlist", tickers=["AMZN", "META"])
                assert sdk_u.name == "Async Watchlist"
                
                sdk_batch = await client.batch_sentiment(universe_id=sdk_u.id)
                assert sdk_batch.count == 2

                del_res = await client.delete_universe(sdk_u.id)
                assert del_res.status == "deleted"
            finally:
                await client.close()

        asyncio.run(run_async())
        print("    [PASS] Async Python SDK passed all assertions.")

        # 13. OpenAPI Documentation Verification
        print(">>> 13. Checking OpenAPI 3.0 Spec for Universe endpoints...")
        res_openapi = requests.get(f"{BASE_URL}/api-docs/openapi.json")
        assert res_openapi.status_code == 200
        spec = res_openapi.json()
        assert "/universes" in spec["paths"]
        assert "/universes/{id}" in spec["paths"]
        assert "Universe" in spec["components"]["schemas"]
        print("    [PASS] OpenAPI spec fully documents custom universes.")

        print("=" * 80)
        print("ALL 13 VERIFICATION CHECKS PASSED PERFECTLY!")
        print("=" * 80)

    finally:
        proc.terminate()
        try:
            proc.wait(timeout=3)
        except Exception:
            proc.kill()

if __name__ == "__main__":
    main()
