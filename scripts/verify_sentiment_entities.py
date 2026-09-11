#!/usr/bin/env python3
"""
═══════════════════════════════════════════════════════════════════════════════
FinText-Alpha-Vectorizer — Test Suite #209: Entity Sentiment Breakdown
═══════════════════════════════════════════════════════════════════════════════
Validates:
  1. Unauthenticated Access Rejection on Entity Sentiment Endpoint (401).
  2. Default Parameters Execution (`GET /sentiment/entities`).
  3. Minimum Mentions Filtering & Thresholding (`?min_mentions=10`).
  4. Entity Type Filtering (company, person, product, organization, location).
  5. Sorting Capabilities (avg_sentiment, mentions, positive_ratio, negative_ratio).
  6. Date Range Filtering (`?start_date=...&end_date=...`).
  7. Pagination Limit Bounds & Validation (`limit=5`, 400 for out-of-bounds).
  8. Input Validation & Error Rejection (400 for invalid dates, bounds, types).
  9. Signal & Mathematical Invariants (ratios <= 1.0, sentiment in [-1, 1]).
  10. OpenAPI 3.0 Documentation Registration & Schema Conformity.
  11. Python SDK Synchronous Client Execution.
  12. Python SDK Asynchronous Client Execution.
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
from fintext.models import EntitySentimentItem, EntitySentimentResponse

SERVER_PORT = 8102
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
    print("FINTEXT ALPHA VECTORIZER — TEST SUITE #209: ENTITY SENTIMENT BREAKDOWN")
    print("=" * 80)

    # ── Phase 1: Unauthenticated Access Rejection ──
    phase("1. Unauthenticated Access Rejection (401)")

    resp = httpx.get(f"{BASE_URL}/sentiment/entities")
    check("GET /sentiment/entities without token → 401", resp.status_code == 401, f"got {resp.status_code}")

    # Acquire test token
    token = get_token("quant_macro_fund")
    auth_headers = {"Authorization": f"Bearer {token}"}

    # ── Phase 2: Default Parameters Execution ──
    phase("2. Default Parameters Execution (GET /sentiment/entities)")

    resp = httpx.get(f"{BASE_URL}/sentiment/entities", headers=auth_headers)
    check("GET /sentiment/entities returns 200", resp.status_code == 200, f"got {resp.status_code}")
    data = resp.json()
    check("Response contains 'entities' list", isinstance(data.get("entities"), list))
    check("Response entity_type is 'all'", data.get("entity_type") == "all")
    check("Response min_mentions is 5", data.get("min_mentions") == 5)
    check("Generated count >= 5", data.get("count", 0) >= 5, f"got {data.get('count')}")

    entities = data.get("entities", [])
    first = entities[0]
    check("Entity item contains entity_text", bool(first.get("entity_text")))
    check("Entity item contains entity_type", bool(first.get("entity_type")))
    check("Entity item contains avg_sentiment", first.get("avg_sentiment") is not None)
    check("Entity item contains positive_ratio", first.get("positive_ratio") is not None)
    check("Entity item contains negative_ratio", first.get("negative_ratio") is not None)
    check("Entity item contains mention_count", first.get("mention_count") is not None)
    check("Entity item contains latest_mention_date", bool(first.get("latest_mention_date")))

    # ── Phase 3: Minimum Mentions Filtering ──
    phase("3. Minimum Mentions Filtering (?min_mentions=10)")

    resp_m10 = httpx.get(f"{BASE_URL}/sentiment/entities?min_mentions=10", headers=auth_headers)
    check("Query min_mentions=10 returns 200", resp_m10.status_code == 200)
    m10_entities = resp_m10.json().get("entities", [])
    check("All returned entities have mention_count >= 10", all(e.get("mention_count", 0) >= 10 for e in m10_entities))

    # Query with very high min_mentions -> empty list with 200 OK
    resp_m99 = httpx.get(f"{BASE_URL}/sentiment/entities?min_mentions=99", headers=auth_headers)
    check("Query min_mentions=99 returns 200", resp_m99.status_code == 200)
    check("Count is 0 for unreachable threshold", resp_m99.json().get("count") == 0)

    # ── Phase 4: Entity Type Filtering ──
    phase("4. Entity Type Filtering (company, person, product, organization, location)")

    for etype in ["company", "person", "product", "organization", "location"]:
        r = httpx.get(f"{BASE_URL}/sentiment/entities?entity_type={etype}&min_mentions=5", headers=auth_headers)
        check(f"Filter by entity_type='{etype}' returns 200", r.status_code == 200)
        items = r.json().get("entities", [])
        check(f"All items for entity_type='{etype}' match type", all(e.get("entity_type") == etype for e in items))
        check(f"Entity count for '{etype}' >= 1", len(items) >= 1)

    # ── Phase 5: Sorting Capabilities ──
    phase("5. Sorting Capabilities (avg_sentiment, mentions, positive_ratio, negative_ratio)")

    # 1. Sort by avg_sentiment (absolute magnitude descending)
    r_sent = httpx.get(f"{BASE_URL}/sentiment/entities?sort_by=avg_sentiment&min_mentions=5", headers=auth_headers)
    s_items = r_sent.json().get("entities", [])
    s_abs = [abs(e["avg_sentiment"]) for e in s_items]
    check("sort_by=avg_sentiment orders by absolute value descending", s_abs == sorted(s_abs, reverse=True))

    # 2. Sort by mentions descending
    r_men = httpx.get(f"{BASE_URL}/sentiment/entities?sort_by=mentions&min_mentions=5", headers=auth_headers)
    m_items = r_men.json().get("entities", [])
    m_counts = [e["mention_count"] for e in m_items]
    check("sort_by=mentions orders by mention_count descending", m_counts == sorted(m_counts, reverse=True))

    # 3. Sort by positive_ratio descending
    r_pos = httpx.get(f"{BASE_URL}/sentiment/entities?sort_by=positive_ratio&min_mentions=5", headers=auth_headers)
    p_items = r_pos.json().get("entities", [])
    p_ratios = [e["positive_ratio"] for e in p_items]
    check("sort_by=positive_ratio orders descending", p_ratios == sorted(p_ratios, reverse=True))

    # 4. Sort by negative_ratio descending
    r_neg = httpx.get(f"{BASE_URL}/sentiment/entities?sort_by=negative_ratio&min_mentions=5", headers=auth_headers)
    n_items = r_neg.json().get("entities", [])
    n_ratios = [e["negative_ratio"] for e in n_items]
    check("sort_by=negative_ratio orders descending", n_ratios == sorted(n_ratios, reverse=True))

    # ── Phase 6: Date Range Filtering ──
    phase("6. Date Range Filtering (?start_date=...&end_date=...)")

    r_date = httpx.get(f"{BASE_URL}/sentiment/entities?start_date=2026-08-20&end_date=2026-08-30&min_mentions=5", headers=auth_headers)
    check("Date range filter returns 200", r_date.status_code == 200)
    d_items = r_date.json().get("entities", [])
    check("Date range returns valid aggregated entities", len(d_items) >= 5)

    # ── Phase 7: Pagination & Limit Bounds ──
    phase("7. Pagination & Limit Bounds")

    r_lim5 = httpx.get(f"{BASE_URL}/sentiment/entities?limit=5", headers=auth_headers)
    check("limit=5 returns <= 5 entities", len(r_lim5.json().get("entities", [])) <= 5)

    r_lim0 = httpx.get(f"{BASE_URL}/sentiment/entities?limit=0", headers=auth_headers)
    check("limit=0 rejected with 400 Bad Request", r_lim0.status_code == 400)

    r_lim150 = httpx.get(f"{BASE_URL}/sentiment/entities?limit=150", headers=auth_headers)
    check("limit=150 rejected with 400 Bad Request", r_lim150.status_code == 400)

    # ── Phase 8: Input Validation & Error Rejection ──
    phase("8. Input Validation & Error Rejection (400)")

    r_bad_m = httpx.get(f"{BASE_URL}/sentiment/entities?min_mentions=0", headers=auth_headers)
    check("min_mentions=0 rejected with 400", r_bad_m.status_code == 400)

    r_bad_t = httpx.get(f"{BASE_URL}/sentiment/entities?entity_type=invalid_type", headers=auth_headers)
    check("Invalid entity_type rejected with 400", r_bad_t.status_code == 400)

    r_bad_s = httpx.get(f"{BASE_URL}/sentiment/entities?sort_by=invalid_sort", headers=auth_headers)
    check("Invalid sort_by rejected with 400", r_bad_s.status_code == 400)

    r_bad_d = httpx.get(f"{BASE_URL}/sentiment/entities?start_date=not-a-date", headers=auth_headers)
    check("Invalid start_date string rejected with 400", r_bad_d.status_code == 400)

    r_inv_range = httpx.get(f"{BASE_URL}/sentiment/entities?start_date=2026-08-30&end_date=2026-08-20", headers=auth_headers)
    check("start_date > end_date rejected with 400", r_inv_range.status_code == 400)

    # ── Phase 9: Signal & Mathematical Invariants ──
    phase("9. Signal & Mathematical Invariants")

    all_ratios_valid = True
    all_sentiment_valid = True
    for ent in entities:
        pos = ent.get("positive_ratio", 0.0)
        neg = ent.get("negative_ratio", 0.0)
        sent = ent.get("avg_sentiment", 0.0)
        if pos + neg > 1.0001:
            all_ratios_valid = False
        if not (-1.0 <= sent <= 1.0):
            all_sentiment_valid = False

    check("positive_ratio + negative_ratio <= 1.0 for all entities", all_ratios_valid)
    check("avg_sentiment is bounded in [-1.0, 1.0] for all entities", all_sentiment_valid)

    # ── Phase 10: OpenAPI 3.0 Documentation Registration ──
    phase("10. OpenAPI 3.0 Documentation Registration")

    spec_resp = httpx.get(f"{BASE_URL}/api-docs/openapi.json")
    check("OpenAPI spec returns 200", spec_resp.status_code == 200)
    spec = spec_resp.json()

    paths = spec.get("paths", {})
    check("Path '/sentiment/entities' registered", "/sentiment/entities" in paths)

    schemas = spec.get("components", {}).get("schemas", {})
    for s in ["SentimentEntitiesParams", "EntitySentimentItem", "EntitySentimentResponse"]:
        check(f"Schema '{s}' registered", s in schemas)

    # ── Phase 11: Python SDK Synchronous Client Execution ──
    phase("11. Python SDK Synchronous Client Execution")

    sdk_client = FinTextClient(base_url=BASE_URL, api_token=token)
    sdk_res = sdk_client.sentiment_entities(entity_type="company", min_mentions=5, limit=5)
    check("SDK sentiment_entities() returns EntitySentimentResponse", isinstance(sdk_res, EntitySentimentResponse))
    check("SDK response contains entities", len(sdk_res.entities) > 0)
    check("SDK response entity matches company type", all(e.entity_type == "company" for e in sdk_res.entities))

    # ── Phase 12: Python SDK Asynchronous Client Execution ──
    phase("12. Python SDK Asynchronous Client Execution")

    async def run_async_sdk():
        async with FinTextAsyncClient(base_url=BASE_URL, api_token=token) as async_client:
            res = await async_client.sentiment_entities(entity_type="person", min_mentions=5, limit=5)
            check("Async SDK sentiment_entities() returns EntitySentimentResponse", isinstance(res, EntitySentimentResponse))
            check("Async SDK items match person type", all(e.entity_type == "person" for e in res.entities))

    asyncio.run(run_async_sdk())

    # ── Summary ──
    print("\n" + "=" * 80)
    total = passed + failed
    print(f"SUITE #209 RESULTS: {passed}/{total} passed, {failed} failed")
    if failed == 0:
        print("🎉 ALL PHASES PASSED — Entity Sentiment Breakdown CERTIFIED.")
    else:
        print(f"⚠️  {failed} CHECKS FAILED — review output above.")
    print("=" * 80)

    return failed == 0


if __name__ == "__main__":
    with ServerContext():
        success = run_tests()
    sys.exit(0 if success else 1)
