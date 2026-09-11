#!/usr/bin/env python3
"""
FinText-Alpha-Vectorizer — Aggregated News Sentiment Feed Verification Suite
═════════════════════════════════════════════════════════════════════════════
Verifies:
  1. Server initialization and `/health` probe
  2. Institutional JWT issuance via POST /auth/token
  3. GET /sentiment/feed with default parameters (last 24h, limit=100, sort=desc)
  4. GET /sentiment/feed with GICS sector filter (e.g. 'Technology')
  5. GET /sentiment/feed with min_confidence and min_quality thresholds
  6. GET /sentiment/feed pagination (limit & offset) and sort ordering (asc / desc)
  7. Input validation error handling (unknown sector 404, invalid sort 400, inverted dates 400, bounds 400)
  8. Authentication enforcement (401 on missing Bearer JWT)
  9. Official Python SDK sync (FinTextClient.sentiment_feed) and async (FinTextAsyncClient.sentiment_feed)
"""

import asyncio
import os
import sys
import time
import subprocess
import httpx

if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
if hasattr(sys.stderr, "reconfigure"):
    sys.stderr.reconfigure(encoding="utf-8", errors="replace")

# Ensure python_sdk is on sys.path
PROJECT_ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
SDK_PATH = os.path.join(PROJECT_ROOT, "python_sdk", "src")
if SDK_PATH not in sys.path:
    sys.path.insert(0, SDK_PATH)

from fintext import FinTextClient, FinTextAsyncClient, FinTextAuthError, FinTextValidationError, FinTextAPIError

PORT = 8095
API_HOST = f"http://127.0.0.1:{PORT}"
BINARY_PATH = os.path.join(PROJECT_ROOT, "rust", "target", "release", "fintext_api.exe")


def run_checks():
    print("=" * 85)
    print(" FinText-Alpha-Vectorizer — Aggregated News Sentiment Feed Verification")
    print("=" * 85)

    if not os.path.exists(BINARY_PATH):
        print(f"[*] Release binary not found at {BINARY_PATH}. Building...")
        res = subprocess.run(["cargo", "build", "--release", "--bin", "fintext_api"], cwd=os.path.join(PROJECT_ROOT, "rust"))
        if res.returncode != 0:
            print("[x] Failed to build release binary.")
            sys.exit(1)

    env = os.environ.copy()
    env["PORT"] = str(PORT)
    env["HOST"] = "127.0.0.1"
    env["QUESTDB_MOCK_FALLBACK"] = "1"
    env["KAFKA_MOCK_FALLBACK"] = "1"
    env["KAFKA_MOCK_MODE"] = "1"
    env["POLYGON_MOCK_FALLBACK"] = "1"
    env["WHISPER_MOCK_FALLBACK"] = "1"
    env["JWT_SECRET"] = "fintext-alpha-vectorizer-institutional-jwt-secret-key-2026"
    env["ADMIN_TOKEN"] = "fintext-admin-dev-secret-token"

    print(f"[*] Starting API Server binary on port {PORT}: {os.path.abspath(BINARY_PATH)}")
    server_proc = subprocess.Popen([BINARY_PATH], env=env, cwd=PROJECT_ROOT, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

    try:
        http = httpx.Client(base_url=API_HOST, trust_env=False, timeout=10.0)

        # Check /health with retry polling
        start_t = time.time()
        ready = False
        last_err = None
        while time.time() - start_t < 15:
            if server_proc.poll() is not None:
                out, err = server_proc.communicate()
                print(f"[!] Server exited prematurely with code {server_proc.returncode}")
                break
            try:
                health_resp = http.get("/health")
                if health_resp.status_code == 200:
                    ready = True
                    break
            except Exception as e:
                last_err = e
                time.sleep(0.5)
        assert ready, f"Server failed to become responsive at /health within 15s (last err: {last_err})"
        print("[*] Server online and responsive at /health")

        # 1. Obtain JWT token via POST /auth/token
        print("\n[1/8] Obtaining institutional JWT via POST /auth/token...")
        token_resp = http.post(
            "/auth/token",
            headers={"X-Admin-Token": "fintext-admin-dev-secret-token"},
            json={"user_id": "quant_feed_analyst_01", "role": "institutional"},
        )
        assert token_resp.status_code == 200, f"Failed token issuance: {token_resp.text}"
        jwt_token = token_resp.json()["token"]
        headers = {"Authorization": f"Bearer {jwt_token}"}
        print("      Token issued successfully for 'quant_feed_analyst_01'")

        # 2. Query GET /sentiment/feed (Default parameters)
        print("\n[2/8] Querying GET /sentiment/feed (Default parameters: last 24h, limit=100, sort=desc)...")
        resp_def = http.get("/sentiment/feed", headers=headers)
        assert resp_def.status_code == 200, f"Expected 200 OK, got {resp_def.status_code}: {resp_def.text}"
        data_def = resp_def.json()
        assert data_def["limit"] == 100
        assert data_def["offset"] == 0
        assert data_def["sort"] == "desc"
        assert data_def["count"] == len(data_def["records"])
        assert data_def["count"] > 0
        first_rec = data_def["records"][0]
        assert "published_utc" in first_rec
        assert "ticker" in first_rec
        assert "source" in first_rec
        assert "title" in first_rec
        assert "sentiment_score" in first_rec
        assert "sentiment_label" in first_rec
        assert "confidence" in first_rec
        assert "data_quality_score" in first_rec
        assert "vpin" in first_rec
        assert "gamma_exposure" in first_rec
        print(f"      HTTP 200 OK | Count: {data_def['count']} / Total: {data_def['total']}")
        print(f"      Sample Record: Ticker={first_rec['ticker']}, Score={first_rec['sentiment_score']:.4f}, Label={first_rec['sentiment_label']}, Conf={first_rec['confidence']:.2f}, Quality={first_rec['data_quality_score']:.2f}, VPIN={first_rec['vpin']:.4f}")

        # 3. Query GET /sentiment/feed with Sector Filter
        print("\n[3/8] Querying GET /sentiment/feed?sector=Technology...")
        resp_tech = http.get("/sentiment/feed?sector=Technology", headers=headers)
        assert resp_tech.status_code == 200, f"Expected 200 OK, got {resp_tech.status_code}: {resp_tech.text}"
        data_tech = resp_tech.json()
        assert data_tech["sector"] in ["Information Technology", "Technology"]
        assert data_tech["count"] > 0
        
        sector_mapping_file = os.path.join(PROJECT_ROOT, "config", "sector_mapping.csv")
        tech_tickers = set()
        if os.path.exists(sector_mapping_file):
            with open(sector_mapping_file, "r") as f:
                for line in f:
                    parts = line.strip().split(",")
                    if len(parts) >= 2 and parts[1].strip().lower() in ["technology", "information technology"]:
                        tech_tickers.add(parts[0].strip().upper())
        if not tech_tickers:
            tech_tickers = {"AAPL", "MSFT", "NVDA", "GOOGL", "GOOG", "AMZN", "META", "TSM", "AVGO", "ORCL", "CRM", "ADBE", "AMD", "INTC", "CSCO", "IBM", "NOW", "SHOP", "PLTR", "TCS", "INFY", "WIPRO"}

        for r in data_tech["records"]:
            assert r["ticker"] in tech_tickers, f"Ticker {r['ticker']} not in Technology sector set: {tech_tickers}"
        print(f"      HTTP 200 OK | Sector='{data_tech['sector']}' | Count: {data_tech['count']} (All tickers verified in GICS Technology set)")

        # 4. Query GET /sentiment/feed with Confidence and Quality Filters
        print("\n[4/8] Querying GET /sentiment/feed with min_confidence=0.6 and min_quality=0.7...")
        resp_filt = http.get(
            "/sentiment/feed?min_confidence=0.6&min_quality=0.7&limit=50",
            headers=headers,
        )
        assert resp_filt.status_code == 200, f"Expected 200 OK, got {resp_filt.status_code}: {resp_filt.text}"
        data_filt = resp_filt.json()
        assert data_filt["min_confidence"] == 0.6
        assert data_filt["min_quality"] == 0.7
        for r in data_filt["records"]:
            assert r["confidence"] >= 0.6
            assert r["data_quality_score"] >= 0.7
        print(f"      HTTP 200 OK | Filtered count: {data_filt['count']} records passed quality/confidence bars")

        # 5. Query Pagination (Offset & Cursor Keyset) and Sorting (asc vs desc)
        print("\n[5/8] Testing pagination (legacy offset and cursor keyset) and sorting...")
        # 5a. Legacy offset pagination
        resp_p1 = http.get("/sentiment/feed?limit=5&offset=0&sort=desc", headers=headers)
        resp_p2 = http.get("/sentiment/feed?limit=5&offset=5&sort=desc", headers=headers)
        assert resp_p1.status_code == 200 and resp_p2.status_code == 200
        p1_data = resp_p1.json()
        p2_data = resp_p2.json()
        assert p1_data["limit"] == 5 and p1_data["offset"] == 0
        assert p2_data["limit"] == 5 and p2_data["offset"] == 5
        assert len(p1_data["records"]) == 5
        assert len(p2_data["records"]) == 5
        assert p1_data["records"][0]["published_utc"] != p2_data["records"][0]["published_utc"]

        # 5b. Keyset/cursor-based pagination (DESC)
        cur_p1_resp = http.get("/sentiment/feed?limit=5&sort=desc", headers=headers)
        assert cur_p1_resp.status_code == 200
        cur_p1_data = cur_p1_resp.json()
        assert len(cur_p1_data["records"]) == 5
        assert "next_cursor" in cur_p1_data
        next_cur = cur_p1_data["next_cursor"]
        assert next_cur is not None
        assert next_cur == cur_p1_data["records"][-1]["published_utc"]

        cur_p2_resp = http.get(f"/sentiment/feed?limit=5&sort=desc&cursor={next_cur}", headers=headers)
        assert cur_p2_resp.status_code == 200
        cur_p2_data = cur_p2_resp.json()
        assert len(cur_p2_data["records"]) > 0
        p1_titles = {r["title"] for r in cur_p1_data["records"]}
        for r in cur_p2_data["records"]:
            assert r["title"] not in p1_titles, "Page 2 should not contain records from Page 1"
            assert r["published_utc"] < next_cur, "Page 2 items must have published_utc < cursor"

        # 5c. Keyset/cursor-based pagination (ASC)
        cur_asc_p1 = http.get("/sentiment/feed?limit=5&sort=asc", headers=headers)
        assert cur_asc_p1.status_code == 200
        asc_p1_data = cur_asc_p1.json()
        assert asc_p1_data["sort"] == "asc"
        if len(asc_p1_data["records"]) >= 2:
            assert asc_p1_data["records"][0]["published_utc"] <= asc_p1_data["records"][1]["published_utc"]
        next_cur_asc = asc_p1_data.get("next_cursor")
        if next_cur_asc:
            cur_asc_p2 = http.get(f"/sentiment/feed?limit=5&sort=asc&cursor={next_cur_asc}", headers=headers)
            assert cur_asc_p2.status_code == 200
            asc_p2_data = cur_asc_p2.json()
            for r in asc_p2_data["records"]:
                assert r["published_utc"] > next_cur_asc, "Page 2 items must have published_utc > cursor"

        print("      HTTP 200 OK | Cursor-based keyset pagination and sort ordering verified successfully")

        # 6. Error Handling & Validation
        print("\n[6/8] Testing parameter validation and error responses...")
        # 6a. Unknown sector -> 404
        r_sec = http.get("/sentiment/feed?sector=InvalidSectorXYZ", headers=headers)
        assert r_sec.status_code == 404, f"Expected 404, got {r_sec.status_code}"
        # 6b. Invalid sort -> 400
        r_sort = http.get("/sentiment/feed?sort=random", headers=headers)
        assert r_sort.status_code == 400, f"Expected 400, got {r_sort.status_code}"
        # 6c. Inverted date range -> 400
        r_dt = http.get("/sentiment/feed?start_date=2026-08-30&end_date=2026-08-01", headers=headers)
        assert r_dt.status_code == 400, f"Expected 400, got {r_dt.status_code}"
        # 6d. Bad confidence threshold -> 400
        r_conf = http.get("/sentiment/feed?min_confidence=1.5", headers=headers)
        assert r_conf.status_code == 400, f"Expected 400, got {r_conf.status_code}"
        # 6e. Mutual exclusion: cursor + offset -> 400
        r_conflict = http.get(f"/sentiment/feed?cursor={next_cur}&offset=5", headers=headers)
        assert r_conflict.status_code == 400, f"Expected 400 for cursor + offset conflict, got {r_conflict.status_code}"
        # 6f. Invalid cursor format -> 400
        r_inv_cur = http.get("/sentiment/feed?cursor=not-a-timestamp", headers=headers)
        assert r_inv_cur.status_code == 400, f"Expected 400 for invalid cursor, got {r_inv_cur.status_code}"
        # 6g. Empty cursor -> 400
        r_emp_cur = http.get("/sentiment/feed?cursor=", headers=headers)
        assert r_emp_cur.status_code == 400, f"Expected 400 for empty cursor, got {r_emp_cur.status_code}"
        print("      Error handling verified (404 Sector, 400 Sort, 400 Dates, 400 Conf, 400 Cursor+Offset, 400 Bad Cursor)")

        # 7. Authentication Enforcement
        print("\n[7/8] Verifying authentication enforcement without Bearer JWT...")
        resp_unauth = http.get("/sentiment/feed")
        assert resp_unauth.status_code == 401, f"Expected 401 Unauthorized, got {resp_unauth.status_code}"
        print("      HTTP 401 Unauthorized correctly enforced")

        # 8. Python SDK Integration (Sync & Async)
        print("\n[8/8] Testing official Python SDK FinTextClient and FinTextAsyncClient...")
        sdk_client = FinTextClient(base_url=API_HOST, api_token=jwt_token)
        feed_resp = sdk_client.sentiment_feed(sector="Technology", limit=20, min_confidence=0.5)
        assert feed_resp.count > 0
        assert feed_resp.sector in ["Technology", "Information Technology"]
        assert len(feed_resp.records) == feed_resp.count
        print(f"      FinTextClient.sentiment_feed() success | {feed_resp.count} records retrieved")

        # 8b. Python SDK Cursor Pagination (Sync)
        feed_p1 = sdk_client.sentiment_feed(limit=5, sort="desc")
        assert feed_p1.count == 5
        assert feed_p1.next_cursor is not None
        feed_p2 = sdk_client.sentiment_feed(cursor=feed_p1.next_cursor, limit=5, sort="desc")
        assert feed_p2.count > 0
        assert feed_p2.records[0].published_utc < feed_p1.next_cursor
        print(f"      FinTextClient cursor traversal success | Page 2 ({feed_p2.count} items) fetched via cursor")

        # 8c. Python SDK Mutual Exclusion Validation
        try:
            sdk_client.sentiment_feed(cursor=feed_p1.next_cursor, offset=5)
            assert False, "Expected FinTextValidationError when both cursor and offset provided"
        except FinTextValidationError:
            pass

        async def test_async():
            async_client = FinTextAsyncClient(base_url=API_HOST, api_token=jwt_token)
            async_resp = await async_client.sentiment_feed(limit=15, sort="asc")
            assert async_resp.count > 0
            assert async_resp.sort == "asc"

            # Async cursor flow
            async_p1 = await async_client.sentiment_feed(limit=5, sort="desc")
            assert async_p1.count == 5
            assert async_p1.next_cursor is not None
            async_p2 = await async_client.sentiment_feed(cursor=async_p1.next_cursor, limit=5, sort="desc")
            assert async_p2.count > 0

            # Async mutual exclusion
            try:
                await async_client.sentiment_feed(cursor=async_p1.next_cursor, offset=5)
                assert False, "Expected FinTextValidationError in async client"
            except FinTextValidationError:
                pass

            await async_client.close()
            return async_resp.count

        async_count = asyncio.run(test_async())
        print(f"      FinTextAsyncClient.sentiment_feed() success | {async_count} records retrieved asynchronously")

        print("\n" + "=" * 85)
        print(" [OK] ALL 8 SENTIMENT FEED TEST PHASES PASSED CLEANLY!")
        print("=" * 85)

    finally:
        print("\n[*] Terminating API Server background process...")
        server_proc.terminate()
        try:
            server_proc.wait(timeout=3)
        except subprocess.TimeoutExpired:
            server_proc.kill()
        print("[*] API Server process terminated.")


if __name__ == "__main__":
    run_checks()
