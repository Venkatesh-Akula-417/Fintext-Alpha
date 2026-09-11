"""
================================================================================
🚀 FINTEXT ALPHA VECTORIZER — TEST SUITE #212: UNIFIED CROSS-DOMAIN SEARCH API
================================================================================
"""

import os
from pathlib import Path
import subprocess
import sys
import time
import uuid
import httpx

PROJECT_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(PROJECT_ROOT / "python_sdk" / "src"))

from fintext import FinTextClient, FinTextAsyncClient, SearchResponse, SearchResultItem

if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")

SERVER_EXE = PROJECT_ROOT / "rust" / "target" / "debug" / "fintext_api.exe"
PORT = 8105
BASE_URL = f"http://127.0.0.1:{PORT}"
ADMIN_TOKEN = "dev_admin_secret_token_123"

passed = 0
failed = 0


def check(desc: str, condition: bool, extra_info: str = ""):
    global passed, failed
    if condition:
        passed += 1
        print(f"  [PASS] {desc}")
    else:
        failed += 1
        print(f"  [FAIL] {desc} -> {extra_info}")
        raise AssertionError(f"Check failed: {desc} ({extra_info})")


class ServerContext:
    def __init__(self):
        self.process = None

    def __enter__(self):
        print(f"[STARTING] Spawning FinText API Server from {SERVER_EXE} on port {PORT}...")
        env = os.environ.copy()
        env["PORT"] = str(PORT)
        env["HOST"] = "127.0.0.1"
        env["ADMIN_TOKEN"] = ADMIN_TOKEN
        env["JWT_SECRET"] = "super_secret_test_jwt_key_32_bytes_len!!"
        env["QUESTDB_MOCK_FALLBACK"] = "1"
        env["POLYGON_MOCK_FALLBACK"] = "1"
        env["WHISPER_MOCK_FALLBACK"] = "1"
        env["NATS_MOCK_MODE"] = "1"

        self.process = subprocess.Popen(
            [str(SERVER_EXE)],
            env=env,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )

        # Wait for server readiness
        start_t = time.time()
        ready = False
        while time.time() - start_t < 15:
            try:
                r = httpx.get(f"{BASE_URL}/health", timeout=1.0)
                if r.status_code == 200:
                    ready = True
                    break
            except Exception:
                time.sleep(0.3)

        if not ready:
            if self.process.poll() is not None:
                out, err = self.process.communicate()
                print(f"[ERROR] Server died on startup:\nSTDOUT:\n{out}\nSTDERR:\n{err}")
            raise RuntimeError("FinText API server failed to start within 15 seconds")

        print(f"[RUNNING] FinText API Server is ready on {BASE_URL}")
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        if self.process:
            print("[STOPPING] Terminating FinText API server...")
            self.process.terminate()
            try:
                self.process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.process.kill()
            print("[STOPPED] Server terminated.")


def run_suite():
    print("=" * 80)
    print("🚀 FIN-TEXT ALPHA VECTORIZER: SUITE #212 - UNIFIED CROSS-DOMAIN SEARCH API")
    print("=" * 80)

    with ServerContext():
        client = httpx.Client(base_url=BASE_URL, timeout=10.0)

        # Step 0: User Setup & Authentication
        print("\n--- STEP 0: User Setup & Authentication ---")
        uid = str(uuid.uuid4())
        token_resp = client.post(
            "/auth/token",
            json={"user_id": uid, "role": "institutional"},
            headers={"X-Admin-Token": ADMIN_TOKEN},
        )
        check("Token issuance returns 200 OK", token_resp.status_code == 200, token_resp.text)
        user_jwt = token_resp.json()["token"]
        auth_headers = {"Authorization": f"Bearer {user_jwt}"}

        # Step 1: Unauthenticated request rejected
        print("\n--- STEP 1: Unauthenticated Request Rejection ---")
        unauth_resp = client.get("/search?q=AAPL")
        check("Unauthenticated search returns 401 Unauthorized", unauth_resp.status_code == 401, unauth_resp.text)

        # Step 2: Query Validation
        print("\n--- STEP 2: Query Parameter Validation ---")
        empty_q_resp = client.get("/search?q=", headers=auth_headers)
        check("Empty search query 'q' returns 400 Bad Request", empty_q_resp.status_code == 400, empty_q_resp.text)

        missing_q_resp = client.get("/search", headers=auth_headers)
        check("Missing search query 'q' returns 400 Bad Request", missing_q_resp.status_code == 400, missing_q_resp.text)

        bad_limit_0 = client.get("/search?q=AAPL&limit=0", headers=auth_headers)
        check("limit=0 returns 400 Bad Request", bad_limit_0.status_code == 400, bad_limit_0.text)

        bad_limit_101 = client.get("/search?q=AAPL&limit=101", headers=auth_headers)
        check("limit=101 returns 400 Bad Request", bad_limit_101.status_code == 400, bad_limit_101.text)

        # Step 3: Domain Type Validation
        print("\n--- STEP 3: Domain Type Validation ---")
        bad_type_resp = client.get("/search?q=AAPL&types=invalid_domain", headers=auth_headers)
        check("Invalid domain type returns 400 Bad Request", bad_type_resp.status_code == 400, bad_type_resp.text)
        check("Error message lists allowed domain types", "Allowed types:" in bad_type_resp.json().get("message", ""), bad_type_resp.text)

        # Step 4: Exact Ticker Search Across All Domains
        print("\n--- STEP 4: Exact Ticker Search (q=AAPL) ---")
        aapl_resp = client.get("/search?q=AAPL", headers=auth_headers)
        check("Search q=AAPL returns 200 OK", aapl_resp.status_code == 200, aapl_resp.text)
        aapl_data = aapl_resp.json()
        check("Response contains query 'AAPL'", aapl_data["query"] == "AAPL", str(aapl_data))
        check("Response contains 8 default domain types", len(aapl_data["types"]) == 8, str(aapl_data["types"]))
        check("Result count > 0", aapl_data["count"] > 0, str(aapl_data["count"]))
        results = aapl_data["results"]
        check("Top result has relevance score 1.0 (exact ticker match)", results[0]["score"] == 1.0, str(results[0]))
        check("Item structure has id, type, ticker, title, snippet, date, score", all(
            {"id", "type", "ticker", "title", "snippet", "date", "score"}.issubset(r.keys()) for r in results
        ), str(results[0]))

        # Step 5: Keyword Search across Titles and Content
        print("\n--- STEP 5: Keyword Search Across Titles & Snippets (q=Blackwell) ---")
        bw_resp = client.get("/search?q=Blackwell", headers=auth_headers)
        check("Search q=Blackwell returns 200 OK", bw_resp.status_code == 200, bw_resp.text)
        bw_data = bw_resp.json()
        check("Keyword search returns matches", bw_data["count"] > 0, str(bw_data))
        check("Top match contains Blackwell in title or snippet", any("blackwell" in r["title"].lower() or "blackwell" in r["snippet"].lower() for r in bw_data["results"]), str(bw_data["results"]))

        # Step 6: Multi-Domain Filtering
        print("\n--- STEP 6: Multi-Domain Filtering (types=news,transcripts) ---")
        news_trans_resp = client.get("/search?q=Apple&types=news,transcripts", headers=auth_headers)
        check("Search types=news,transcripts returns 200 OK", news_trans_resp.status_code == 200, news_trans_resp.text)
        nt_data = news_trans_resp.json()
        check("Queried types match requested domains", set(nt_data["types"]) == {"news", "transcripts"}, str(nt_data["types"]))
        check("All returned items are of type news or transcript", all(
            r["type"] in ["news", "transcript"] for r in nt_data["results"]
        ), str(nt_data["results"]))

        # Step 7: Single Domain Search (types=insider)
        print("\n--- STEP 7: Single Domain Search (types=insider) ---")
        insider_resp = client.get("/search?q=AAPL&types=insider", headers=auth_headers)
        check("Search types=insider returns 200 OK", insider_resp.status_code == 200, insider_resp.text)
        insider_data = insider_resp.json()
        check("All returned items are of type insider", all(
            r["type"] == "insider" for r in insider_data["results"]
        ), str(insider_data["results"]))

        # Step 8: Options Domain Search (types=options)
        print("\n--- STEP 8: Options Domain Search (types=options) ---")
        options_resp = client.get("/search?q=AAPL&types=options", headers=auth_headers)
        check("Search types=options returns 200 OK", options_resp.status_code == 200, options_resp.text)
        options_data = options_resp.json()
        check("All returned items are of type options", all(
            r["type"] == "options" for r in options_data["results"]
        ), str(options_data["results"]))

        # Step 9: Date Range Filtering
        print("\n--- STEP 9: Date Range Filtering ---")
        date_filtered_resp = client.get("/search?q=AAPL&start_date=2025-08-01&end_date=2025-08-25", headers=auth_headers)
        check("Search with start_date & end_date returns 200 OK", date_filtered_resp.status_code == 200, date_filtered_resp.text)
        df_data = date_filtered_resp.json()
        check("All returned items fall between 2025-08-01 and 2025-08-25", all(
            "2025-08-01" <= r["date"][:10] <= "2025-08-25" for r in df_data["results"]
        ), str(df_data["results"]))

        # Step 10: Pagination Verification
        print("\n--- STEP 10: Pagination Verification ---")
        page1_resp = client.get("/search?q=AAPL&limit=3&offset=0", headers=auth_headers)
        check("Page 1 returns 200 OK", page1_resp.status_code == 200, page1_resp.text)
        page1_data = page1_resp.json()
        check("Page 1 count is <= 3", page1_data["count"] <= 3, str(page1_data["count"]))

        page2_resp = client.get("/search?q=AAPL&limit=3&offset=3", headers=auth_headers)
        check("Page 2 returns 200 OK", page2_resp.status_code == 200, page2_resp.text)
        page2_data = page2_resp.json()

        page1_ids = {r["id"] for r in page1_data["results"]}
        page2_ids = {r["id"] for r in page2_data["results"]}
        if page1_ids and page2_ids:
            check("Pagination offsets return disjoint item sets", page1_ids.isdisjoint(page2_ids), f"p1={page1_ids}, p2={page2_ids}")

        # Step 11: OpenAPI Specification Verification
        print("\n--- STEP 11: OpenAPI 3.0 Documentation ---")
        openapi_resp = client.get("/api-docs/openapi.json")
        check("GET /api-docs/openapi.json returns 200 OK", openapi_resp.status_code == 200, openapi_resp.text)
        openapi_data = openapi_resp.json()
        paths = openapi_data.get("paths", {})
        check("OpenAPI spec registers /search path", "/search" in paths, str(list(paths.keys())))
        schemas = openapi_data.get("components", {}).get("schemas", {})
        check("OpenAPI spec registers SearchParams schema", "SearchParams" in schemas, str(list(schemas.keys())))
        check("OpenAPI spec registers SearchResultItem schema", "SearchResultItem" in schemas, str(list(schemas.keys())))
        check("OpenAPI spec registers SearchResponse schema", "SearchResponse" in schemas, str(list(schemas.keys())))
        tags = [t["name"] for t in openapi_data.get("tags", [])]
        check("OpenAPI spec registers 'Search' tag", "Search" in tags, str(tags))

        # Step 12: Python SDK Integration (Sync & Async)
        print("\n--- STEP 12: Python SDK Client Execution ---")
        sdk_client = FinTextClient(base_url=BASE_URL, api_token=user_jwt)
        sdk_resp = sdk_client.search(q="AAPL", types=["news", "filings", "insider"], limit=10)
        check("SDK search returns SearchResponse instance", isinstance(sdk_resp, SearchResponse), str(type(sdk_resp)))
        check("SDK query matches 'AAPL'", sdk_resp.query == "AAPL", sdk_resp.query)
        check("SDK results contain items", len(sdk_resp.results) > 0, str(len(sdk_resp.results)))
        check("SDK result items are SearchResultItem instances", isinstance(sdk_resp.results[0], SearchResultItem), str(type(sdk_resp.results[0])))
        sdk_client.close()

        import anyio
        async def run_async_sdk():
            async_client = FinTextAsyncClient(base_url=BASE_URL, api_token=user_jwt)
            async_resp = await async_client.search(q="NVDA", types=["news", "events", "options"], limit=5)
            check("Async SDK search returns SearchResponse instance", isinstance(async_resp, SearchResponse), str(type(async_resp)))
            check("Async SDK query matches 'NVDA'", async_resp.query == "NVDA", async_resp.query)
            check("Async SDK results contain items", len(async_resp.results) > 0, str(len(async_resp.results)))
            await async_client.close()

        anyio.run(run_async_sdk)

        print("\n" + "=" * 80)
        print(f"🎉 SUITE #212 SUMMARY: {passed} PASSED / {failed} FAILED")
        print("=" * 80)


if __name__ == "__main__":
    run_suite()
