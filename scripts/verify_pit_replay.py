#!/usr/bin/env python3
"""
===============================================================================
FinText-Alpha-Vectorizer — Verification: Point-in-Time (PIT) Data Replay
===============================================================================
Verifies:
  1.  GET /pit/replay returns 200 OK with valid parameters
  2.  Response contains correct top-level fields
  3.  Look-Ahead-Bias Invariant: all records have published_utc <= as_of_utc
  4.  Storage Commit Invariant: all news & sentiment records have db_commit_utc <= as_of_utc
  5.  Strict Ingestion Ordering: published_utc <= ingested_utc <= db_commit_utc
  6.  Replay consistency report: all_records_consistent == True, violations_count == 0
  7.  Summary counts match actual array lengths and total_records
  8.  Model timestamp attribution: model_generated_at <= as_of_utc
  9.  Filter isolation: include_news=false returns empty news_articles
  10. Filter isolation: include_sentiment=false returns empty sentiment_records
  11. Parameter validation: empty ticker returns 400 Bad Request
  12. Parameter validation: invalid as_of_utc returns 400 Bad Request
  13. Python SDK Sync Client integration (client.pit_replay & client.replay)
  14. Python SDK Async Client integration & OpenAPI 3.0 Schema verification
===============================================================================
"""

import asyncio
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import subprocess
import sys
import time

import httpx

if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
if hasattr(sys.stderr, "reconfigure"):
    sys.stderr.reconfigure(encoding="utf-8", errors="replace")

PROJECT_ROOT = Path(__file__).resolve().parent.parent
SDK_PATH = PROJECT_ROOT / "python_sdk" / "src"
if str(SDK_PATH) not in sys.path:
    sys.path.insert(0, str(SDK_PATH))

PORT = 8147
BASE_URL = f"http://127.0.0.1:{PORT}"
ADMIN_TOKEN = "test_admin_token_xyz123_valid_32_bytes_length!"
SERVER_EXE = PROJECT_ROOT / "rust" / "target" / "debug" / "fintext_api.exe"

passed = 0
failed = 0
total = 14


def report(phase: int, name: str, ok: bool, detail: str = ""):
    global passed, failed
    if ok:
        passed += 1
        print(f"  ✅ Phase {phase:2d} │ {name}")
    else:
        failed += 1
        print(f"  ❌ Phase {phase:2d} │ {name}")
        if detail:
            print(f"     └─ {detail}")


class ServerContext:
    def __init__(self):
        self.process = None

    def __enter__(self):
        print(f"[STARTING] Spawning FinText API Server on port {PORT}...")
        env = os.environ.copy()
        env["PORT"] = str(PORT)
        env["HOST"] = "127.0.0.1"
        env["ADMIN_TOKEN"] = ADMIN_TOKEN
        env["JWT_SECRET"] = "super_secret_test_jwt_key_32_bytes_len!!"
        env["QUESTDB_MOCK_FALLBACK"] = "1"
        env["POLYGON_MOCK_FALLBACK"] = "1"
        env["WHISPER_MOCK_FALLBACK"] = "1"
        env["NATS_MOCK_MODE"] = "1"
        env["CHAT_ALERTS_MOCK"] = "1"
        env["CONFIG_PATH"] = str(PROJECT_ROOT / "config" / "config.yaml")

        self.process = subprocess.Popen(
            [str(SERVER_EXE)],
            env=env,
            cwd=str(PROJECT_ROOT),
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )

        start_t = time.time()
        ready = False
        while time.time() - start_t < 25:
            try:
                r = httpx.get(f"{BASE_URL}/health", timeout=1.0)
                if r.status_code == 200:
                    ready = True
                    break
            except Exception:
                time.sleep(0.3)

        if not ready:
            raise RuntimeError(f"Server on port {PORT} failed to start within 25 seconds.")
        print(f"[READY] API Server is healthy at {BASE_URL}")
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        if self.process:
            print("[CLEANUP] Terminating FinText API Server process...")
            self.process.terminate()
            try:
                self.process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.process.kill()


def parse_iso(ts_str: str) -> datetime:
    ts_str = ts_str.replace("Z", "+00:00")
    return datetime.fromisoformat(ts_str)


def run_all_phases():
    print("=" * 80)
    print(" FinText-Alpha-Vectorizer — Point-in-Time (PIT) Data Replay Verification")
    print("=" * 80)
    print(f" Project Root: {PROJECT_ROOT}")
    print(f" Server Port:  {PORT}")

    with ServerContext():
        client = httpx.Client(base_url=BASE_URL, timeout=10.0)

        # 0. Obtain JWT auth token
        auth_resp = client.post(
            "/auth/token",
            json={
                "user_id": "quant_pit_researcher",
                "role": "institutional",
                "duration_seconds": 3600,
            },
            headers={"X-Admin-Token": ADMIN_TOKEN},
        )
        if auth_resp.status_code != 200:
            token = auth_resp.json().get("token") or "mock_jwt_token"
        else:
            token = auth_resp.json()["token"]

        headers = {"Authorization": f"Bearer {token}"}
        as_of_target = "2025-06-15T14:30:00Z"
        as_of_dt = parse_iso(as_of_target)

        # Phase 1: GET /pit/replay returns 200 OK
        resp = client.get(
            "/pit/replay",
            params={"ticker": "AAPL", "as_of_utc": as_of_target, "limit": 50},
            headers=headers,
        )
        report(1, "GET /pit/replay returns 200 OK", resp.status_code == 200, f"Status: {resp.status_code}, Body: {resp.text}")

        data = resp.json() if resp.status_code == 200 else {}

        # Phase 2: Response contains correct top-level fields
        expected_fields = {
            "ticker",
            "as_of_utc",
            "news_articles",
            "filings",
            "events",
            "sentiment_records",
            "summary",
            "replay_consistency",
            "model_generated_at",
            "message",
        }
        actual_fields = set(data.keys())
        has_fields = expected_fields.issubset(actual_fields)
        report(2, "Response contains correct top-level fields", has_fields, f"Missing: {expected_fields - actual_fields}")

        # Phase 3: Look-Ahead-Bias Invariant: all published_utc <= as_of_utc
        news = data.get("news_articles", [])
        filings = data.get("filings", [])
        events = data.get("events", [])
        sentiment = data.get("sentiment_records", [])

        pub_ok = True
        for item in news + filings + events + sentiment:
            pub_ts = parse_iso(item["published_utc"])
            if pub_ts > as_of_dt:
                pub_ok = False
                break
        report(3, "Look-Ahead-Bias Invariant: all published_utc <= as_of_utc", pub_ok and len(news) > 0)

        # Phase 4: Storage Commit Invariant: all db_commit_utc <= as_of_utc
        commit_ok = True
        for item in news + sentiment:
            db_ts = parse_iso(item["db_commit_utc"])
            if db_ts > as_of_dt:
                commit_ok = False
                break
        report(4, "Storage Commit Invariant: all db_commit_utc <= as_of_utc", commit_ok and len(sentiment) > 0)

        # Phase 5: Strict Ingestion Ordering: published_utc <= ingested_utc <= db_commit_utc
        ordering_ok = True
        for item in news + sentiment:
            p_ts = parse_iso(item["published_utc"])
            i_ts = parse_iso(item["ingested_utc"])
            d_ts = parse_iso(item["db_commit_utc"])
            if not (p_ts <= i_ts <= d_ts):
                ordering_ok = False
                break
        report(5, "Strict Ingestion Ordering: published_utc <= ingested_utc <= db_commit_utc", ordering_ok)

        # Phase 6: Replay consistency report
        rc = data.get("replay_consistency", {})
        consistent = (
            rc.get("all_records_consistent") is True
            and rc.get("violations_count") == 0
            and "published_utc" in rc.get("invariant", "")
        )
        report(6, "Replay consistency report: all_records_consistent == True, violations_count == 0", consistent)

        # Phase 7: Summary counts match actual array lengths
        summary = data.get("summary", {})
        counts_match = (
            summary.get("news_count") == len(news)
            and summary.get("filings_count") == len(filings)
            and summary.get("events_count") == len(events)
            and summary.get("sentiment_count") == len(sentiment)
            and summary.get("total_records") == (len(news) + len(filings) + len(events) + len(sentiment))
            and summary.get("total_records", 0) > 0
        )
        report(7, "Summary counts match actual array lengths and total_records", counts_match)

        # Phase 8: Model timestamp attribution: model_generated_at <= as_of_utc
        model_gen_ts = parse_iso(data.get("model_generated_at", as_of_target))
        gen_ok = model_gen_ts <= as_of_dt
        report(8, "Model timestamp attribution: model_generated_at <= as_of_utc", gen_ok)

        # Phase 9: Filter isolation: include_news=false returns empty news_articles
        r_no_news = client.get(
            "/pit/replay",
            params={"ticker": "AAPL", "as_of_utc": as_of_target, "include_news": False},
            headers=headers,
        )
        d_no_news = r_no_news.json() if r_no_news.status_code == 200 else {}
        no_news_ok = (
            r_no_news.status_code == 200
            and len(d_no_news.get("news_articles", [1])) == 0
            and len(d_no_news.get("sentiment_records", [])) > 0
        )
        report(9, "Filter isolation: include_news=false returns empty news_articles", no_news_ok)

        # Phase 10: Filter isolation: include_sentiment=false returns empty sentiment_records
        r_no_sent = client.get(
            "/pit/replay",
            params={"ticker": "AAPL", "as_of_utc": as_of_target, "include_sentiment": False},
            headers=headers,
        )
        d_no_sent = r_no_sent.json() if r_no_sent.status_code == 200 else {}
        no_sent_ok = (
            r_no_sent.status_code == 200
            and len(d_no_sent.get("sentiment_records", [1])) == 0
            and len(d_no_sent.get("news_articles", [])) > 0
        )
        report(10, "Filter isolation: include_sentiment=false returns empty sentiment_records", no_sent_ok)

        # Phase 11: Parameter validation: empty ticker returns 400 Bad Request
        r_empty_ticker = client.get(
            "/pit/replay",
            params={"ticker": "", "as_of_utc": as_of_target},
            headers=headers,
        )
        report(11, "Validation: empty ticker returns 400 Bad Request", r_empty_ticker.status_code == 400)

        # Phase 12: Parameter validation: invalid as_of_utc returns 400 Bad Request
        r_invalid_ts = client.get(
            "/pit/replay",
            params={"ticker": "AAPL", "as_of_utc": "invalid-timestamp-2025"},
            headers=headers,
        )
        report(12, "Validation: invalid as_of_utc returns 400 Bad Request", r_invalid_ts.status_code == 400)

        # Phase 13: Python SDK Sync Client integration
        from fintext import FinTextClient, PITReplayResponse
        sdk_client = FinTextClient(base_url=BASE_URL, api_token=token)
        sdk_replay = sdk_client.pit_replay(ticker="AAPL", as_of_utc=as_of_target, limit=20)
        sdk_alias_replay = sdk_client.replay(ticker="AAPL", as_of_utc=as_of_target, limit=20)

        sdk_sync_ok = (
            isinstance(sdk_replay, PITReplayResponse)
            and sdk_replay.ticker == "AAPL"
            and sdk_replay.replay_consistency.all_records_consistent is True
            and isinstance(sdk_alias_replay, PITReplayResponse)
            and sdk_alias_replay.summary.total_records == sdk_replay.summary.total_records
        )
        report(13, "Python SDK Sync Client integration (client.pit_replay & client.replay)", sdk_sync_ok)

        # Phase 14: Python SDK Async Client integration & OpenAPI 3.0 Schema verification
        async def verify_async_and_openapi():
            from fintext import FinTextAsyncClient
            async_c = FinTextAsyncClient(base_url=BASE_URL, api_token=token)
            async_replay = await async_c.pit_replay(ticker="AAPL", as_of_utc=as_of_target, limit=15)
            await async_c.close()

            r_docs = client.get("/api-docs/openapi.json")
            has_openapi = False
            if r_docs.status_code == 200:
                docs = r_docs.json()
                paths = docs.get("paths", {})
                schemas = docs.get("components", {}).get("schemas", {})
                tags = [t.get("name") for t in docs.get("tags", [])]

                has_openapi = (
                    "/pit/replay" in paths
                    and "PITReplayResponse" in schemas
                    and "PITReplayNewsItem" in schemas
                    and "PITReplayConsistency" in schemas
                    and "Point-in-Time & Compliance" in tags
                )

            return (
                isinstance(async_replay, PITReplayResponse)
                and async_replay.ticker == "AAPL"
                and has_openapi
            )

        async_and_openapi_ok = asyncio.run(verify_async_and_openapi())
        report(14, "Python SDK Async Client & OpenAPI 3.0 Schema registration", async_and_openapi_ok)


if __name__ == "__main__":
    run_all_phases()
    print("=" * 80)
    print(f" PIT Replay Verification Results: {passed}/{total} Phases Passed ({failed} Failed)")
    print("=" * 80)
    if failed > 0:
        sys.exit(1)
    sys.exit(0)
