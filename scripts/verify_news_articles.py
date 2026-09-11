#!/usr/bin/env python3
"""
═══════════════════════════════════════════════════════════════════════════════
FinText-Alpha-Vectorizer — Test Suite #208: News Article Full Text Retrieval
═══════════════════════════════════════════════════════════════════════════════
Validates:
  1. Unauthenticated Access Rejection on News Articles Endpoints (401).
  2. Default News Article Listing with Pagination (`GET /news/articles`).
  3. Snippet Length Verification (maximum 200 characters).
  4. Ticker-Based Query Filtering (`?ticker=AAPL`, `?ticker=NVDA`).
  5. Source-Based Query Filtering (`?source=Institutional Wire`).
  6. Date Range Filtering (`?start_date=...&end_date=...`).
  7. Pagination Offset, Limit, and Input Bounds Validation (400 for out-of-range).
  8. Full Article Body Retrieval by UUID (`GET /news/articles/{id}`).
  9. Non-Existent Article ID Lookup Rejection (404).
  10. OpenAPI 3.0 Conformance, Paths, and Schema Registration.
  11. Python SDK Synchronous Client Methods.
  12. Python SDK Asynchronous Client Methods.
═══════════════════════════════════════════════════════════════════════════════
"""

import asyncio
import os
import subprocess
import sys
import time
from pathlib import Path
from typing import Optional

import httpx

ROOT_DIR = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT_DIR / "python_sdk" / "src"))
if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")

from fintext import FinTextAsyncClient, FinTextClient
from fintext.models import NewsArticleFull, NewsArticleMetadata, NewsArticlesListResponse

SERVER_PORT = 8101
BASE_URL = f"http://127.0.0.1:{SERVER_PORT}"
ADMIN_TOKEN = "fintext-admin-dev-secret-token"
JWT_SECRET = "fintext-alpha-vectorizer-institutional-jwt-secret-key-2026"


class ServerContext:
    def __init__(self):
        self.process: Optional[subprocess.Popen] = None

    def __enter__(self):
        env = os.environ.copy()
        env["PORT"] = str(SERVER_PORT)
        env["ADMIN_TOKEN"] = ADMIN_TOKEN
        env["JWT_SECRET"] = JWT_SECRET
        env["QUESTDB_MOCK_FALLBACK"] = "1"
        env["POLYGON_MOCK_FALLBACK"] = "1"
        env["WHISPER_MOCK_FALLBACK"] = "1"
        env["NATS_MOCK_MODE"] = "1"

        candidates = [
            ROOT_DIR / "rust" / "target" / "release" / "fintext_api.exe",
            ROOT_DIR / "rust" / "target" / "debug" / "fintext_api.exe",
        ]
        valid_candidates = [p for p in candidates if p.exists()]
        if not valid_candidates:
            raise RuntimeError("Server binary not found. Build it first.")
        exe_path = max(valid_candidates, key=lambda p: p.stat().st_mtime)

        print(f"[STARTING] Spawning FinText API Server from {exe_path} on port {SERVER_PORT}...")
        self.process = subprocess.Popen(
            [str(exe_path)],
            env=env,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )

        max_attempts = 50
        for i in range(max_attempts):
            try:
                resp = httpx.get(f"{BASE_URL}/health", timeout=0.5)
                if resp.status_code == 200:
                    print(f"[READY] Server is healthy and ready (attempt {i + 1})")
                    return self
            except Exception:
                time.sleep(0.1)

        if self.process.poll() is not None:
            raise RuntimeError(f"Server failed to start (exit code {self.process.poll()}).")
        raise RuntimeError("Server health check timed out.")

    def __exit__(self, exc_type, exc_val, exc_tb):
        if self.process:
            print("[STOPPING] Shutting down FinText API Server...")
            self.process.terminate()
            try:
                self.process.wait(timeout=3)
            except subprocess.TimeoutExpired:
                self.process.kill()


def get_token(user_id: str) -> str:
    resp = httpx.post(
        f"{BASE_URL}/auth/token",
        json={"user_id": user_id},
        headers={"X-Admin-Token": ADMIN_TOKEN},
    )
    assert resp.status_code == 200, f"Failed to get token for {user_id}: {resp.text}"
    return resp.json()["token"]


# ═════════════════════════════════════════════════════════════════════════════
# Test Runner & Reporting
# ═════════════════════════════════════════════════════════════════════════════

passed = 0
failed = 0


def phase(name: str):
    print(f"\n{'─' * 80}")
    print(f"  PHASE: {name}")
    print(f"{'─' * 80}")


def check(description: str, condition: bool, detail: str = ""):
    global passed, failed
    if condition:
        passed += 1
        print(f"  ✅ {description}")
    else:
        failed += 1
        msg = f"  ❌ {description}"
        if detail:
            msg += f" — {detail}"
        print(msg)


def run_tests():
    print("=" * 80)
    print("FINTEXT ALPHA VECTORIZER — TEST SUITE #208: NEWS ARTICLE FULL TEXT RETRIEVAL")
    print("=" * 80)

    # ── Phase 1: Unauthenticated Access Rejection ──
    phase("1. Unauthenticated Access Rejection (401)")

    resp = httpx.get(f"{BASE_URL}/news/articles")
    check("GET /news/articles without token → 401", resp.status_code == 401, f"got {resp.status_code}")

    dummy_id = "550e8400-e29b-41d4-a716-446655440000"
    resp = httpx.get(f"{BASE_URL}/news/articles/{dummy_id}")
    check("GET /news/articles/{id} without token → 401", resp.status_code == 401, f"got {resp.status_code}")

    # Acquire test token
    token = get_token("quant_researcher_alpha")
    auth_headers = {"Authorization": f"Bearer {token}"}

    # ── Phase 2: Default News Article Listing ──
    phase("2. Default News Article Listing (GET /news/articles)")

    resp = httpx.get(f"{BASE_URL}/news/articles", headers=auth_headers)
    check("GET /news/articles returns 200", resp.status_code == 200, f"got {resp.status_code}")
    data = resp.json()
    check("Response contains 'articles' array", isinstance(data.get("articles"), list))
    check("Total article count >= 8", data.get("total", 0) >= 8, f"got {data.get('total')}")
    check("Default limit is 20", data.get("limit") == 20)
    check("Default offset is 0", data.get("offset") == 0)

    articles = data.get("articles", [])
    first_article = articles[0]
    sample_id = first_article.get("id")

    # ── Phase 3: Snippet Length Verification ──
    phase("3. Snippet Length Verification (<= 200 chars)")

    all_snippets_valid = True
    for art in articles:
        snip = art.get("snippet", "")
        if len(snip) > 200:
            all_snippets_valid = False
            break
    check("All article previews have snippet <= 200 characters", all_snippets_valid)

    # ── Phase 4: Ticker-Based Filtering ──
    phase("4. Ticker-Based Filtering (?ticker=AAPL)")

    resp_aapl = httpx.get(f"{BASE_URL}/news/articles?ticker=AAPL", headers=auth_headers)
    check("Filter by ticker AAPL returns 200", resp_aapl.status_code == 200)
    aapl_articles = resp_aapl.json().get("articles", [])
    check("Filtered AAPL count >= 2", len(aapl_articles) >= 2, f"got {len(aapl_articles)}")
    check("All returned items have ticker AAPL", all(a.get("ticker") == "AAPL" for a in aapl_articles))

    resp_nvda = httpx.get(f"{BASE_URL}/news/articles?ticker=NVDA", headers=auth_headers)
    nvda_articles = resp_nvda.json().get("articles", [])
    check("Filter by ticker NVDA matches", all(a.get("ticker") == "NVDA" for a in nvda_articles))

    # ── Phase 5: Source-Based Filtering ──
    phase("5. Source-Based Filtering (?source=Institutional Wire)")

    resp_src = httpx.get(f"{BASE_URL}/news/articles?source=Institutional Wire", headers=auth_headers)
    check("Filter by source returns 200", resp_src.status_code == 200)
    src_articles = resp_src.json().get("articles", [])
    check("Source filter count >= 1", len(src_articles) >= 1)
    check("All items contain source 'Institutional Wire'", all("institutional wire" in a.get("source", "").lower() for a in src_articles))

    # ── Phase 6: Date Range Filtering ──
    phase("6. Date Range Filtering")

    resp_date = httpx.get(f"{BASE_URL}/news/articles?start_date=2020-01-01&end_date=2030-12-31", headers=auth_headers)
    check("Date range filter returns 200", resp_date.status_code == 200)
    check("Date range returns articles", len(resp_date.json().get("articles", [])) >= 8)

    # ── Phase 7: Pagination & Bounds Validation ──
    phase("7. Pagination & Bounds Validation")

    resp_p1 = httpx.get(f"{BASE_URL}/news/articles?limit=3&offset=0", headers=auth_headers)
    check("Page 1 with limit=3 returns 3 articles", len(resp_p1.json().get("articles", [])) == 3)

    resp_p2 = httpx.get(f"{BASE_URL}/news/articles?limit=3&offset=3", headers=auth_headers)
    check("Page 2 with limit=3 returns 3 articles", len(resp_p2.json().get("articles", [])) == 3)
    check("Page 1 and Page 2 contain distinct articles", resp_p1.json()["articles"][0]["id"] != resp_p2.json()["articles"][0]["id"])

    # Bounds validation
    resp_bad_limit = httpx.get(f"{BASE_URL}/news/articles?limit=0", headers=auth_headers)
    check("limit=0 returns 400 Bad Request", resp_bad_limit.status_code == 400)

    resp_high_limit = httpx.get(f"{BASE_URL}/news/articles?limit=150", headers=auth_headers)
    check("limit=150 returns 400 Bad Request", resp_high_limit.status_code == 400)

    # ── Phase 8: Full Article Body Retrieval ──
    phase("8. Full Article Body Retrieval (GET /news/articles/{id})")

    resp_full = httpx.get(f"{BASE_URL}/news/articles/{sample_id}", headers=auth_headers)
    check("GET /news/articles/{id} returns 200", resp_full.status_code == 200, f"got {resp_full.status_code}")
    full_data = resp_full.json()
    check("Full article ID matches requested UUID", full_data.get("id") == sample_id)
    check("Full article contains 'full_text'", "full_text" in full_data and len(full_data["full_text"]) > 200)
    check("Full article contains sentiment score", full_data.get("sentiment_score") is not None)
    check("Full article contains sentiment label", full_data.get("sentiment_label") in ["positive", "negative", "neutral"])
    check("Full article contains data quality score", full_data.get("data_quality_score") is not None)

    # ── Phase 9: Non-Existent Article ID Lookup ──
    phase("9. Non-Existent Article ID Lookup (404 Not Found)")

    resp_404 = httpx.get(f"{BASE_URL}/news/articles/00000000-0000-0000-0000-000000000000", headers=auth_headers)
    check("Non-existent UUID returns 404", resp_404.status_code == 404, f"got {resp_404.status_code}")
    check("Error payload has 'Not Found'", "not found" in resp_404.json().get("error", "").lower())

    # ── Phase 10: OpenAPI 3.0 Documentation Registration ──
    phase("10. OpenAPI 3.0 Documentation Registration")

    spec_resp = httpx.get(f"{BASE_URL}/api-docs/openapi.json")
    check("OpenAPI spec returns 200", spec_resp.status_code == 200)
    spec = spec_resp.json()

    paths = spec.get("paths", {})
    check("Path '/news/articles' registered", "/news/articles" in paths)
    check("Path '/news/articles/{id}' registered", "/news/articles/{id}" in paths)

    schemas = spec.get("components", {}).get("schemas", {})
    for s in ["NewsArticleMetadata", "NewsArticleFull", "NewsArticlesListResponse"]:
        check(f"Schema '{s}' registered", s in schemas)

    tags = [t.get("name") for t in spec.get("tags", [])]
    check("Tag 'Financial News & Full Text' present", "Financial News & Full Text" in tags)

    # ── Phase 11: Python SDK Synchronous Client Execution ──
    phase("11. Python SDK Synchronous Client Execution")

    sdk_client = FinTextClient(base_url=BASE_URL, api_token=token)
    sdk_list = sdk_client.list_news_articles(ticker="AAPL", limit=5)
    check("SDK list_news_articles() returns NewsArticlesListResponse", isinstance(sdk_list, NewsArticlesListResponse))
    check("SDK list count >= 2", sdk_list.total >= 2)
    check("SDK list first article snippet <= 200 chars", len(sdk_list.articles[0].snippet) <= 200)

    sdk_article = sdk_client.get_news_article(sdk_list.articles[0].id)
    check("SDK get_news_article() returns NewsArticleFull", isinstance(sdk_article, NewsArticleFull))
    check("SDK full_text is populated", len(sdk_article.full_text) > 100)

    # ── Phase 12: Python SDK Asynchronous Client Execution ──
    phase("12. Python SDK Asynchronous Client Execution")

    async def run_async_sdk():
        async with FinTextAsyncClient(base_url=BASE_URL, api_token=token) as async_client:
            async_list = await async_client.list_news_articles(ticker="NVDA", limit=5)
            check("Async SDK list_news_articles() returns response", isinstance(async_list, NewsArticlesListResponse))
            check("Async SDK list items match NVDA", all(a.ticker == "NVDA" for a in async_list.articles))

            async_article = await async_client.get_news_article(async_list.articles[0].id)
            check("Async SDK get_news_article() returns NewsArticleFull", isinstance(async_article, NewsArticleFull))
            check("Async SDK article full_text populated", len(async_article.full_text) > 100)

    asyncio.run(run_async_sdk())

    # ── Summary ──
    print("\n" + "=" * 80)
    total = passed + failed
    print(f"SUITE #208 RESULTS: {passed}/{total} passed, {failed} failed")
    if failed == 0:
        print("🎉 ALL PHASES PASSED — News Article Full Text Retrieval CERTIFIED.")
    else:
        print(f"⚠️  {failed} CHECKS FAILED — review output above.")
    print("=" * 80)

    return failed == 0


if __name__ == "__main__":
    with ServerContext():
        success = run_tests()
    sys.exit(0 if success else 1)
