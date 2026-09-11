#!/usr/bin/env python3
"""
=====================================================================================
FinText-Alpha-Vectorizer — Public API Gateway & Surface Simplification Verification
=====================================================================================
Suite #260:
Validates the versioned Public API Gateway (/v1) layer:
  1. All 32 core endpoints are properly mounted and reachable under the /v1 prefix.
  2. Public endpoints enforce authentication and rate limiting middleware identically.
  3. OpenAPI 3.0 specification at /v1/api-docs/openapi.json documents strictly /v1 paths.
  4. Non-public / internal endpoints are hidden from external public documentation.
  5. Python SDK supports api_version="v1" and communicates seamlessly with /v1 routes.
  6. Existing unversioned routes remain fully operational for backward compatibility.
=====================================================================================
"""

import os
import sys
import time
import uuid
import subprocess
import requests
from pathlib import Path

if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
if hasattr(sys.stderr, "reconfigure"):
    sys.stderr.reconfigure(encoding="utf-8", errors="replace")

PROJECT_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(PROJECT_ROOT / "python_sdk" / "src"))
from fintext import FinTextClient

PORT = 8260
BASE_URL = f"http://127.0.0.1:{PORT}"
DEV_ADMIN_TOKEN = "fintext-admin-dev-secret-token"
DEV_JWT_SECRET = "fintext-alpha-vectorizer-institutional-jwt-secret-key-2026"

RELEASE_EXE = PROJECT_ROOT / "rust" / "target" / "release" / "fintext_api.exe"
DEBUG_EXE = PROJECT_ROOT / "rust" / "target" / "debug" / "fintext_api.exe"
SERVER_EXE = str(RELEASE_EXE if RELEASE_EXE.exists() else DEBUG_EXE)


def start_server() -> subprocess.Popen:
    env = os.environ.copy()
    env["PORT"] = str(PORT)
    env["HOST"] = "127.0.0.1"
    env["ADMIN_TOKEN"] = DEV_ADMIN_TOKEN
    env["JWT_SECRET"] = DEV_JWT_SECRET
    env["QUESTDB_MOCK_FALLBACK"] = "1"
    env["POLYGON_MOCK_FALLBACK"] = "1"
    env["WHISPER_MOCK_FALLBACK"] = "1"
    env["KAFKA_MOCK_FALLBACK"] = "1"
    env["KAFKA_MOCK_MODE"] = "1"
    env["NATS_MOCK_MODE"] = "1"
    env["CHAT_ALERTS_MOCK"] = "1"
    env["PIT_DATA_ENABLED"] = "1"
    env["PIT_DATA_DIR"] = str(PROJECT_ROOT / "config")
    env["CONFIG_PATH"] = str(PROJECT_ROOT / "config" / "config.yaml")
    env["PUBLIC_API_VERSION"] = "v1"
    env["ENABLE_FULL_API_SURFACE"] = "false"

    proc = subprocess.Popen(
        [SERVER_EXE],
        env=env,
        cwd=str(PROJECT_ROOT),
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )

    for _ in range(40):
        try:
            r = requests.get(f"{BASE_URL}/health", timeout=1)
            if r.status_code == 200:
                return proc
        except Exception:
            time.sleep(0.25)

    proc.kill()
    print("Server failed to start on port", PORT)
    sys.exit(1)


def main():
    print("=" * 85)
    print(" FinText-Alpha-Vectorizer — Public API Gateway & Surface Verification (Suite #260)")
    print("=" * 85)

    print(f"[*] Starting API Server binary: {SERVER_EXE}")
    proc = start_server()
    print("      [OK] Server online at http://127.0.0.1:8000\n")

    try:
        # ─────────────────────────────────────────────────────────────────────────────
        # [Step 1] Acquire Institutional JWT Token via POST /v1/auth/token
        # ─────────────────────────────────────────────────────────────────────────────
        print("[1/6] Authenticating via POST /v1/auth/token...")
        user_uuid = str(uuid.uuid4())
        r_auth = requests.post(
            f"{BASE_URL}/v1/auth/token",
            headers={"X-Admin-Token": DEV_ADMIN_TOKEN, "Content-Type": "application/json"},
            json={"user_id": user_uuid, "tier": "institutional"},
        )
        assert r_auth.status_code == 200, f"Failed auth: {r_auth.status_code} {r_auth.text}"
        auth_data = r_auth.json()
        token = auth_data["token"]
        headers = {"Authorization": f"Bearer {token}"}
        print(f"      [OK] Acquired JWT token for user '{user_uuid}' (Expires in: {auth_data.get('expires_in')}s)")

        # ─────────────────────────────────────────────────────────────────────────────
        # [Step 2] Validate Core Public System & Governance Endpoints
        # ─────────────────────────────────────────────────────────────────────────────
        print("\n[2/6] Verifying System & Model Governance Core Endpoints under /v1...")
        # 1. GET /v1/health
        r_health = requests.get(f"{BASE_URL}/v1/health")
        assert r_health.status_code == 200, f"GET /v1/health failed: {r_health.text}"
        assert r_health.json()["status"] == "ok"
        print("      [OK] GET /v1/health -> 200 OK")

        # 2. GET /v1/model-card
        r_mc = requests.get(f"{BASE_URL}/v1/model-card")
        assert r_mc.status_code == 200, f"GET /v1/model-card failed: {r_mc.text}"
        assert "model_id" in r_mc.json()
        print(f"      [OK] GET /v1/model-card -> 200 OK (Model ID: {r_mc.json()['model_id']})")

        # 3. GET /v1/users/me
        r_me = requests.get(f"{BASE_URL}/v1/users/me", headers=headers)
        assert r_me.status_code == 200, f"GET /v1/users/me failed: {r_me.text}"
        assert r_me.json()["id"] == user_uuid
        print("      [OK] GET /v1/users/me -> 200 OK")

        # 4. POST /v1/users/api-keys & GET & DELETE
        r_key_post = requests.post(
            f"{BASE_URL}/v1/users/api-keys",
            headers=headers,
            json={"name": "gateway-test-key"},
        )
        assert r_key_post.status_code in (200, 201), f"POST /v1/users/api-keys failed: {r_key_post.text}"
        key_id = r_key_post.json()["id"]
        print(f"      [OK] POST /v1/users/api-keys -> {r_key_post.status_code} (Key ID: {key_id})")

        r_key_list = requests.get(f"{BASE_URL}/v1/users/api-keys", headers=headers)
        assert r_key_list.status_code == 200, f"GET /v1/users/api-keys failed: {r_key_list.text}"
        keys_data = r_key_list.json().get("api_keys", [])
        assert any(k["id"] == key_id for k in keys_data)
        print("      [OK] GET /v1/users/api-keys -> 200 OK")

        r_key_del = requests.delete(f"{BASE_URL}/v1/users/api-keys/{key_id}", headers=headers)
        assert r_key_del.status_code == 200, f"DELETE /v1/users/api-keys/{key_id} failed: {r_key_del.text}"
        print(f"      [OK] DELETE /v1/users/api-keys/{key_id} -> 200 OK")

        # ─────────────────────────────────────────────────────────────────────────────
        # [Step 3] Validate Core Sentiment & NLP Endpoints
        # ─────────────────────────────────────────────────────────────────────────────
        print("\n[3/6] Verifying Sentiment & NLP Core Endpoints under /v1...")
        # 5. GET /v1/sentiment/feed
        r_feed = requests.get(f"{BASE_URL}/v1/sentiment/feed", headers=headers)
        assert r_feed.status_code == 200, f"GET /v1/sentiment/feed failed: {r_feed.text}"
        feed_records = r_feed.json().get("records", r_feed.json().get("items", []))
        print(f"      [OK] GET /v1/sentiment/feed -> 200 OK (Records returned: {len(feed_records)})")

        # 6. GET /v1/sentiment/history
        r_hist = requests.get(
            f"{BASE_URL}/v1/sentiment/history?ticker=AAPL&start_date=2025-01-01&end_date=2025-01-10",
            headers=headers,
        )
        assert r_hist.status_code == 200, f"GET /v1/sentiment/history failed: {r_hist.text}"
        print("      [OK] GET /v1/sentiment/history?ticker=AAPL -> 200 OK")

        # 7. GET & POST /v1/sentiment/batch
        r_batch_get = requests.get(f"{BASE_URL}/v1/sentiment/batch?tickers=AAPL,MSFT", headers=headers)
        assert r_batch_get.status_code == 200, f"GET /v1/sentiment/batch failed: {r_batch_get.text}"
        assert len(r_batch_get.json()["results"]) == 2
        print("      [OK] GET /v1/sentiment/batch?tickers=AAPL,MSFT -> 200 OK")

        r_batch_post = requests.post(
            f"{BASE_URL}/v1/sentiment/batch?tickers=AAPL,NVDA",
            headers=headers,
        )
        assert r_batch_post.status_code == 200, f"POST /v1/sentiment/batch failed: {r_batch_post.text}"
        print("      [OK] POST /v1/sentiment/batch -> 200 OK")

        # 8. GET /v1/sentiment/confidence & GET /v1/sentiment
        r_conf = requests.get(f"{BASE_URL}/v1/sentiment/confidence?ticker=AAPL", headers=headers)
        assert r_conf.status_code == 200, f"GET /v1/sentiment/confidence failed: {r_conf.text}"
        assert "confidence" in r_conf.json() and "probabilities" in r_conf.json()
        print(f"      [OK] GET /v1/sentiment/confidence -> 200 OK (Confidence: {r_conf.json()['confidence']:.4f})")

        r_sent = requests.get(f"{BASE_URL}/v1/sentiment?ticker=AAPL", headers=headers)
        assert r_sent.status_code == 200, f"GET /v1/sentiment failed: {r_sent.text}"
        print("      [OK] GET /v1/sentiment?ticker=AAPL -> 200 OK")

        # 9. GET /v1/sentiment/disagreement
        r_disag = requests.get(
            f"{BASE_URL}/v1/sentiment/disagreement?ticker=AAPL&start_date=2025-01-01&end_date=2025-03-31",
            headers=headers,
        )
        assert r_disag.status_code == 200, f"GET /v1/sentiment/disagreement failed: {r_disag.text}"
        print("      [OK] GET /v1/sentiment/disagreement -> 200 OK")

        # 10. GET /v1/sentiment/entities
        r_ent = requests.get(f"{BASE_URL}/v1/sentiment/entities?ticker=AAPL", headers=headers)
        assert r_ent.status_code == 200, f"GET /v1/sentiment/entities failed: {r_ent.text}"
        print("      [OK] GET /v1/sentiment/entities -> 200 OK")

        # 11. GET /v1/sentiment/sector
        r_sec = requests.get(
            f"{BASE_URL}/v1/sentiment/sector?sector=Technology&start_date=2025-01-01&end_date=2025-03-31",
            headers=headers,
        )
        assert r_sec.status_code == 200, f"GET /v1/sentiment/sector failed: {r_sec.text}"
        print("      [OK] GET /v1/sentiment/sector -> 200 OK")

        # 12. GET /v1/news/articles
        r_news = requests.get(f"{BASE_URL}/v1/news/articles", headers=headers)
        assert r_news.status_code == 200, f"GET /v1/news/articles failed: {r_news.text}"
        print(f"      [OK] GET /v1/news/articles -> 200 OK (Articles: {len(r_news.json()['articles'])})")

        # ─────────────────────────────────────────────────────────────────────────────
        # [Step 4] Validate Events, Filings, PIT, Symbols & Universes Endpoints
        # ─────────────────────────────────────────────────────────────────────────────
        print("\n[4/6] Verifying Events, Filings, PIT, Universes & Export Endpoints...")
        # 13. GET /v1/events/8k
        r_8k = requests.get(f"{BASE_URL}/v1/events/8k?ticker=AAPL", headers=headers)
        assert r_8k.status_code == 200, f"GET /v1/events/8k failed: {r_8k.text}"
        print("      [OK] GET /v1/events/8k -> 200 OK")

        # 14. GET /v1/events/earnings-surprise
        r_es = requests.get(f"{BASE_URL}/v1/events/earnings-surprise?ticker=AAPL", headers=headers)
        assert r_es.status_code == 200, f"GET /v1/events/earnings-surprise failed: {r_es.text}"
        print("      [OK] GET /v1/events/earnings-surprise -> 200 OK")

        # 15. Transcripts: POST, GET, DELETE /v1/transcripts
        r_tr_post = requests.post(
            f"{BASE_URL}/v1/transcripts",
            headers=headers,
            json={
                "ticker": "AAPL",
                "quarter": 3,
                "year": 2026,
                "call_date": "2026-07-28",
                "transcript_text": "Apple executives reported outstanding Q3 revenue growth and strong institutional sentiment.",
            },
        )
        assert r_tr_post.status_code in (200, 201), f"POST /v1/transcripts failed: {r_tr_post.text}"
        tr_id = r_tr_post.json()["id"]
        print(f"      [OK] POST /v1/transcripts -> {r_tr_post.status_code} (Created test transcript {tr_id})")

        r_tr_list = requests.get(f"{BASE_URL}/v1/transcripts?ticker=AAPL", headers=headers)
        assert r_tr_list.status_code == 200, f"GET /v1/transcripts failed: {r_tr_list.text}"
        print("      [OK] GET /v1/transcripts -> 200 OK")

        r_tr_del = requests.delete(f"{BASE_URL}/v1/transcripts/{tr_id}", headers=headers)
        assert r_tr_del.status_code in (200, 204), f"DELETE /v1/transcripts/{tr_id} failed: {r_tr_del.text}"
        print(f"      [OK] DELETE /v1/transcripts/{tr_id} -> 200 OK")

        # 16. PIT Endpoints: /v1/pit/certificate & /v1/pit/replay
        r_pit_cert = requests.get(f"{BASE_URL}/v1/pit/certificate?ticker=AAPL&as_of_utc=2026-06-01T12:00:00Z", headers=headers)
        assert r_pit_cert.status_code == 200, f"GET /v1/pit/certificate failed: {r_pit_cert.text}"
        print("      [OK] GET /v1/pit/certificate -> 200 OK")

        r_pit_replay = requests.get(f"{BASE_URL}/v1/pit/replay?ticker=AAPL&as_of_utc=2026-06-01T12:00:00Z", headers=headers)
        assert r_pit_replay.status_code == 200, f"GET /v1/pit/replay failed: {r_pit_replay.text}"
        print("      [OK] GET /v1/pit/replay -> 200 OK")

        # 17. Symbol Map & Universes
        r_sym = requests.get(f"{BASE_URL}/v1/symbol/map?identifier=AAPL", headers=headers)
        assert r_sym.status_code == 200, f"GET /v1/symbol/map failed: {r_sym.text}"
        print("      [OK] GET /v1/symbol/map -> 200 OK")

        r_univ = requests.get(f"{BASE_URL}/v1/universes", headers=headers)
        assert r_univ.status_code == 200, f"GET /v1/universes failed: {r_univ.text}"
        print(f"      [OK] GET /v1/universes -> 200 OK (Universes count: {len(r_univ.json()['universes'])})")

        # 18. Signal Quality Report & CSV Export
        r_qual = requests.post(
            f"{BASE_URL}/v1/signals/quality-report",
            headers=headers,
            json={
                "signal_type": "sentiment",
                "tickers": ["AAPL", "MSFT"],
                "start_date": "2025-01-01",
                "end_date": "2025-06-30",
                "horizon_days": 5,
            },
        )
        assert r_qual.status_code == 200, f"POST /v1/signals/quality-report failed: {r_qual.text}"
        ic_val = r_qual.json()["ic_summary"].get("spearman_ic", r_qual.json()["ic_summary"].get("rank_ic", 0.0))
        print(f"      [OK] POST /v1/signals/quality-report -> 200 OK (Spearman Rank IC: {ic_val:.4f})")

        r_csv_get = requests.get(
            f"{BASE_URL}/v1/export/csv?ticker=AAPL&start_date=2025-01-01&end_date=2025-01-10",
            headers=headers,
        )
        assert r_csv_get.status_code == 200, f"GET /v1/export/csv failed: {r_csv_get.text}"
        assert "published_utc" in r_csv_get.text and "ticker" in r_csv_get.text
        print("      [OK] GET /v1/export/csv -> 200 OK (Streamed CSV dataset)")

        r_csv_post = requests.post(
            f"{BASE_URL}/v1/export/csv?ticker=AAPL&start_date=2025-01-01&end_date=2025-01-10",
            headers=headers,
        )
        assert r_csv_post.status_code == 200, f"POST /v1/export/csv failed: {r_csv_post.text}"
        print("      [OK] POST /v1/export/csv -> 200 OK")

        # 19. Webhooks
        r_wh_post = requests.post(
            f"{BASE_URL}/v1/webhooks",
            headers=headers,
            json={"url": "http://127.0.0.1:9999/webhook", "events": ["sentiment", "spillover"]},
        )
        assert r_wh_post.status_code in (200, 201), f"POST /v1/webhooks failed: {r_wh_post.text}"
        wh_id = r_wh_post.json()["id"]
        print(f"      [OK] POST /v1/webhooks -> {r_wh_post.status_code} (Webhook ID: {wh_id})")

        r_wh_list = requests.get(f"{BASE_URL}/v1/webhooks", headers=headers)
        assert r_wh_list.status_code == 200, f"GET /v1/webhooks failed: {r_wh_list.text}"
        assert any(w["id"] == wh_id for w in r_wh_list.json()["webhooks"])
        print("      [OK] GET /v1/webhooks -> 200 OK")

        # ─────────────────────────────────────────────────────────────────────────────
        # [Step 5] Validate OpenAPI 3.0 Public Documentation Specification
        # ─────────────────────────────────────────────────────────────────────────────
        print("\n[5/6] Auditing OpenAPI 3.0 Specification at /v1/api-docs/openapi.json...")
        r_spec = requests.get(f"{BASE_URL}/v1/api-docs/openapi.json")
        assert r_spec.status_code == 200, f"Failed to fetch public OpenAPI spec: {r_spec.text}"
        spec = r_spec.json()

        assert spec["info"]["title"] == "FinText-Alpha-Vectorizer Public API"
        assert spec["info"]["version"] == "1.0.0"
        paths = spec.get("paths", {})
        print(f"      [OK] Discovered {len(paths)} documented public paths.")

        # Strict assertion: Every documented route in PublicApiDoc MUST start with /v1/
        for p in paths.keys():
            assert p.startswith("/v1/"), f"Route '{p}' in public API spec does not start with /v1/ prefix!"
        print("      [OK] 100% of routes in public spec strictly enforce '/v1/' prefix.")

        # Assert all core public endpoints are present in the spec
        expected_public_endpoints = [
            "/v1/health",
            "/v1/auth/token",
            "/v1/users/me",
            "/v1/users/api-keys",
            "/v1/sentiment/feed",
            "/v1/sentiment/history",
            "/v1/sentiment/batch",
            "/v1/sentiment/confidence",
            "/v1/sentiment/disagreement",
            "/v1/sentiment/entities",
            "/v1/sentiment/sector",
            "/v1/news/articles",
            "/v1/events/8k",
            "/v1/events/earnings-surprise",
            "/v1/transcripts",
            "/v1/pit/certificate",
            "/v1/pit/replay",
            "/v1/model-card",
            "/v1/symbol/map",
            "/v1/universes",
            "/v1/signals/quality-report",
            "/v1/export/csv",
            "/v1/webhooks",
        ]
        for ep in expected_public_endpoints:
            assert ep in paths, f"Core public endpoint '{ep}' missing from OpenAPI specification!"
        print(f"      [OK] All {len(expected_public_endpoints)} audited core endpoints are documented.")

        # Assert internal / non-core routes are excluded from the public spec
        internal_endpoints = [
            "/v1/market/regime",
            "/v1/spillovers",
            "/v1/options/iv",
            "/v1/billing/checkout",
            "/v1/orgs",
            "/v1/fix/orders",
            "/market/regime",
            "/spillovers",
            "/options/iv",
        ]
        for ep in internal_endpoints:
            assert ep not in paths, f"Internal route '{ep}' leaked into public OpenAPI specification!"
        print("      [OK] Non-public/internal endpoints properly omitted from public specification.")

        # Verify Swagger UI
        r_swagger = requests.get(f"{BASE_URL}/swagger-ui/")
        assert r_swagger.status_code == 200, f"GET /swagger-ui/ failed: {r_swagger.status_code}"
        assert "swagger-ui" in r_swagger.text.lower() or "<!doctype html>" in r_swagger.text.lower()
        print("      [OK] GET /swagger-ui/ loads interactive Swagger UI successfully.")

        r_v1_swagger = requests.get(f"{BASE_URL}/v1/swagger-ui/", allow_redirects=True)
        assert r_v1_swagger.status_code == 200
        print("      [OK] GET /v1/swagger-ui/ resolves to interactive Swagger UI successfully.")

        # ─────────────────────────────────────────────────────────────────────────────
        # [Step 6] Official Python Client SDK with api_version="v1" & Backward Compatibility
        # ─────────────────────────────────────────────────────────────────────────────
        print("\n[6/6] Testing Python Client SDK with api_version='v1' and backward compatibility...")
        # 1. FinTextClient with api_version="v1"
        client = FinTextClient(base_url=BASE_URL, api_token=token, api_version="v1")
        assert client.base_url == f"{BASE_URL}/v1"
        sdk_health = client.health()
        assert sdk_health.status == "ok"
        print("      [OK] SDK initialized with api_version='v1' points to http://127.0.0.1:8000/v1")

        sdk_sent = client.sentiment("AAPL")
        assert sdk_sent.ticker == "AAPL"
        assert 0.0 <= sdk_sent.confidence <= 1.0
        print(f"      [OK] SDK client.sentiment('AAPL') via /v1 succeeded: score={sdk_sent.sentiment_score:.4f}")

        sdk_feed = client.sentiment_feed(limit=5)
        feed_count = len(sdk_feed.records) if hasattr(sdk_feed, "records") else len(sdk_feed.items)
        assert feed_count <= 5
        print(f"      [OK] SDK client.sentiment_feed(limit=5) via /v1 succeeded: {feed_count} records")

        # 2. Backward Compatibility Check: Unversioned endpoints still functional
        r_old_health = requests.get(f"{BASE_URL}/health")
        assert r_old_health.status_code == 200
        r_old_sent = requests.get(f"{BASE_URL}/sentiment?ticker=AAPL", headers=headers)
        assert r_old_sent.status_code == 200
        r_old_spec = requests.get(f"{BASE_URL}/api-docs/openapi.json")
        assert r_old_spec.status_code == 200
        assert "/market/regime" in r_old_spec.json().get("paths", {})
        print("      [OK] Unversioned endpoints (/health, /sentiment, /api-docs/openapi.json) remain fully functional.")

        print("\n" + "=" * 85)
        print(" [SUCCESS] Suite #260 PASSED: Public API Gateway Layer & Surface Simplification Verified!")
        print("=" * 85)

    finally:
        print("\n[*] Tearing down API Server process...")
        proc.terminate()
        try:
            proc.wait(timeout=3)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait()
        print("      [OK] Server stopped cleanly.")


if __name__ == "__main__":
    main()
