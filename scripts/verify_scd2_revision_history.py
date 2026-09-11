#!/usr/bin/env python3
"""
===============================================================================
FinText-Alpha-Vectorizer — Verification Suite #259:
Slowly Changing Dimension Type 2 (SCD2) Point-in-Time Revision History
===============================================================================
Validates:
  1.  Pre-seeded AAPL baseline inspection (v1 superseded, v2 active)
  2.  Point-in-time as-of query for AAPL at v1 window returns v1 (is_current=False)
  3.  Point-in-time as-of query for AAPL at v2 window returns v2 (is_current=True)
  4.  Point-in-time boundary & chronological integrity invariants
  5.  Querying prior to any recorded validity window returns 404 Not Found
  6.  Ingest new revision (v3) via POST /sentiment/revision supersedes v2 and creates v3
  7.  Inspect revision lineage via GET /sentiment/revisions?ticker=AAPL (3 versions)
  8.  Batch multi-ticker sentiment (/sentiment/batch) with as_of_utc filter
  9.  Historical sentiment time series (/sentiment/history) with as_of_utc filter
  10. Sentiment feed (/sentiment/feed) with as_of_utc filter
  11. Parameter validation: Malformed RFC3339 as_of_utc rejects with 400 Bad Request
  12. Invalidation & Python SDK Sync/Async Client integration
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

PORT = 8159
BASE_URL = f"http://127.0.0.1:{PORT}"
ADMIN_TOKEN = "test_admin_token_xyz123_valid_32_bytes_length!"
SERVER_EXE = PROJECT_ROOT / "rust" / "target" / "debug" / "fintext_api.exe"

passed = 0
failed = 0
total_phases = 12


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
    global passed, failed
    print("=" * 80)
    print(" FinText-Alpha-Vectorizer — SCD2 Point-in-Time Revision History Verification")
    print("=" * 80)
    print(f" Project Root: {PROJECT_ROOT}")
    print(f" Server Port:  {PORT}")

    with ServerContext():
        client = httpx.Client(base_url=BASE_URL, timeout=10.0)

        # 0. Obtain JWT auth token
        auth_resp = client.post(
            "/auth/token",
            json={
                "user_id": "scd2_pit_researcher",
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

        # ---------------------------------------------------------------------
        # Phase 1: Pre-seeded AAPL baseline inspection (v1 superseded, v2 active)
        # ---------------------------------------------------------------------
        resp1 = client.get("/sentiment/revisions", params={"ticker": "AAPL"}, headers=headers)
        p1_ok = False
        p1_detail = ""
        if resp1.status_code == 200:
            d1 = resp1.json()
            revs = d1.get("revisions", [])
            if (
                d1.get("total_revisions") == 2
                and len(revs) == 2
                and revs[0].get("revision_number") == 1
                and revs[0].get("is_current") is False
                and revs[1].get("revision_number") == 2
                and revs[1].get("is_current") is True
            ):
                p1_ok = True
            else:
                p1_detail = f"Unexpected payload: {d1}"
        else:
            p1_detail = f"Status {resp1.status_code}: {resp1.text}"
        report(1, "Pre-seeded AAPL baseline inspection (v1 superseded, v2 active)", p1_ok, p1_detail)

        # ---------------------------------------------------------------------
        # Phase 2: Point-in-time as-of query for AAPL at v1 window returns v1
        # AAPL v1 is valid from 2025-06-15T14:30:00.100000Z to 2025-06-16T10:00:00.000000Z
        # ---------------------------------------------------------------------
        as_of_v1 = "2025-06-15T20:00:00Z"
        resp2 = client.get("/sentiment", params={"ticker": "AAPL", "as_of_utc": as_of_v1}, headers=headers)
        p2_ok = False
        p2_detail = ""
        if resp2.status_code == 200:
            d2 = resp2.json()
            if (
                abs(d2.get("sentiment_score", 0.0) - 0.45) < 1e-4
                and d2.get("revision_number") == 1
                and d2.get("is_current") is False
                and d2.get("valid_from") == "2025-06-15T14:30:00.100000Z"
                and d2.get("valid_to") == "2025-06-16T10:00:00.000000Z"
            ):
                p2_ok = True
            else:
                p2_detail = f"Score={d2.get('sentiment_score')}, rev={d2.get('revision_number')}, is_curr={d2.get('is_current')}, valid_to={d2.get('valid_to')}"
        else:
            p2_detail = f"Status {resp2.status_code}: {resp2.text}"
        report(2, "Point-in-time as-of query for AAPL at v1 window returns v1 (is_current=False)", p2_ok, p2_detail)

        # ---------------------------------------------------------------------
        # Phase 3: Point-in-time as-of query for AAPL at v2 window returns v2
        # AAPL v2 is valid from 2025-06-16T10:00:00.000000Z onwards
        # ---------------------------------------------------------------------
        as_of_v2 = "2025-06-16T15:00:00Z"
        resp3 = client.get("/sentiment", params={"ticker": "AAPL", "as_of_utc": as_of_v2}, headers=headers)
        p3_ok = False
        p3_detail = ""
        if resp3.status_code == 200:
            d3 = resp3.json()
            if (
                abs(d3.get("sentiment_score", 0.0) - 0.72) < 1e-4
                and d3.get("revision_number") == 2
                and d3.get("is_current") is True
                and d3.get("valid_from") == "2025-06-16T10:00:00.000000Z"
                and d3.get("valid_to") is None
            ):
                p3_ok = True
            else:
                p3_detail = f"Score={d3.get('sentiment_score')}, rev={d3.get('revision_number')}, is_curr={d3.get('is_current')}, valid_to={d3.get('valid_to')}"
        else:
            p3_detail = f"Status {resp3.status_code}: {resp3.text}"
        report(3, "Point-in-time as-of query for AAPL at v2 window returns v2 (is_current=True)", p3_ok, p3_detail)

        # ---------------------------------------------------------------------
        # Phase 4: Point-in-time boundary & chronological integrity invariants
        # ---------------------------------------------------------------------
        p4_ok = False
        p4_detail = ""
        try:
            d1 = resp1.json()
            v1 = d1["revisions"][0]
            v2 = d1["revisions"][1]

            v1_pub = parse_iso(v1["published_utc"])
            v1_ing = parse_iso(v1["ingested_utc"])
            v1_db = parse_iso(v1["db_commit_utc"])
            v1_vf = parse_iso(v1["valid_from"])
            v1_vt = parse_iso(v1["valid_to"])

            v2_pub = parse_iso(v2["published_utc"])
            v2_ing = parse_iso(v2["ingested_utc"])
            v2_db = parse_iso(v2["db_commit_utc"])
            v2_vf = parse_iso(v2["valid_from"])

            # Strict ordering invariants: published <= ingested <= db_commit <= valid_from
            order_v1 = v1_pub <= v1_ing <= v1_db <= v1_vf
            order_v2 = v2_pub <= v2_ing <= v2_db <= v2_vf
            # Temporal continuity: v1.valid_to == v2.valid_from
            continuity = v1_vt == v2_vf

            if order_v1 and order_v2 and continuity:
                p4_ok = True
            else:
                p4_detail = f"order_v1={order_v1}, order_v2={order_v2}, continuity={continuity}"
        except Exception as e:
            p4_detail = str(e)
        report(4, "Point-in-time boundary & chronological integrity invariants", p4_ok, p4_detail)

        # ---------------------------------------------------------------------
        # Phase 5: Querying prior to any recorded validity window returns 404
        # 2025-06-14 is before AAPL v1 (which started 2025-06-15)
        # ---------------------------------------------------------------------
        as_of_pre = "2025-06-14T00:00:00Z"
        resp5 = client.get("/sentiment", params={"ticker": "AAPL", "as_of_utc": as_of_pre}, headers=headers)
        p5_ok = resp5.status_code == 404 and "No sentiment events found as of" in resp5.text
        report(5, "Querying prior to any recorded validity window returns 404 Not Found", p5_ok, f"Status={resp5.status_code}, Body={resp5.text}")

        # ---------------------------------------------------------------------
        # Phase 6: Ingest new revision (v3) via POST /sentiment/revision
        # ---------------------------------------------------------------------
        v3_payload = {
            "ticker": "AAPL",
            "title": "Apple Q3 Guidance Revised Higher in SEC Form 8-K/A Amendment #2",
            "sentiment_score": 0.88,
            "source": "SEC EDGAR",
            "source_id": "0000320193-25-000099",
            "ingested_utc": "2025-06-17T11:00:00.050000Z",
            "db_commit_utc": "2025-06-17T11:00:00.100000Z",
        }
        resp6 = client.post("/sentiment/revision", json=v3_payload, headers=headers)
        p6_ok = False
        p6_detail = ""
        if resp6.status_code == 201:
            d6 = resp6.json()
            act = d6.get("active_revision", {})
            sup = d6.get("superseded_revision", {})
            if (
                act.get("revision_number") == 3
                and act.get("is_current") is True
                and abs(act.get("sentiment_score", 0.0) - 0.88) < 1e-4
                and sup.get("revision_number") == 2
                and sup.get("is_current") is False
                and sup.get("valid_to") == "2025-06-17T11:00:00.100000Z"
            ):
                p6_ok = True
            else:
                p6_detail = f"act={act}, sup={sup}"
        else:
            p6_detail = f"Status {resp6.status_code}: {resp6.text}"
        report(6, "Ingest new revision (v3) via POST /sentiment/revision supersedes v2 and creates v3", p6_ok, p6_detail)

        # ---------------------------------------------------------------------
        # Phase 7: Inspect revision lineage via GET /sentiment/revisions?ticker=AAPL
        # ---------------------------------------------------------------------
        resp7 = client.get("/sentiment/revisions", params={"ticker": "AAPL"}, headers=headers)
        p7_ok = False
        p7_detail = ""
        if resp7.status_code == 200:
            d7 = resp7.json()
            revs = d7.get("revisions", [])
            if (
                d7.get("total_revisions") == 3
                and len(revs) == 3
                and revs[0]["revision_number"] == 1 and revs[0]["is_current"] is False
                and revs[1]["revision_number"] == 2 and revs[1]["is_current"] is False
                and revs[2]["revision_number"] == 3 and revs[2]["is_current"] is True
            ):
                p7_ok = True
            else:
                p7_detail = f"Revisions structure mismatch: {d7}"
        else:
            p7_detail = f"Status {resp7.status_code}: {resp7.text}"
        report(7, "Inspect revision lineage via GET /sentiment/revisions?ticker=AAPL (3 versions)", p7_ok, p7_detail)

        # ---------------------------------------------------------------------
        # Phase 8: Batch multi-ticker sentiment (/sentiment/batch) with as_of_utc filter
        # ---------------------------------------------------------------------
        resp8 = client.get("/sentiment/batch", params={"tickers": "AAPL,MSFT", "as_of_utc": as_of_v1}, headers=headers)
        p8_ok = False
        p8_detail = ""
        if resp8.status_code == 200:
            d8 = resp8.json()
            res = d8.get("results", [])
            aapl_res = next((r for r in res if r.get("ticker") == "AAPL"), None)
            msft_res = next((r for r in res if r.get("ticker") == "MSFT"), None)
            if (
                aapl_res and aapl_res.get("revision_number") == 1 and abs(aapl_res.get("sentiment_score", 0.0) - 0.45) < 1e-4
                and msft_res and msft_res.get("revision_number") == 1
            ):
                p8_ok = True
            else:
                p8_detail = f"AAPL batch as-of result: {aapl_res}, MSFT: {msft_res}"
        else:
            p8_detail = f"Status {resp8.status_code}: {resp8.text}"
        report(8, "Batch multi-ticker sentiment (/sentiment/batch) with as_of_utc filter", p8_ok, p8_detail)

        # ---------------------------------------------------------------------
        # Phase 9: Historical sentiment time series (/sentiment/history) with as_of_utc filter
        # ---------------------------------------------------------------------
        resp9 = client.get(
            "/sentiment/history",
            params={
                "ticker": "AAPL",
                "start_date": "2025-06-01",
                "end_date": "2025-06-30",
                "as_of_utc": as_of_v1,
            },
            headers=headers,
        )
        p9_ok = False
        p9_detail = ""
        if resp9.status_code == 200:
            d9 = resp9.json()
            recs = d9.get("records", [])
            if len(recs) > 0:
                as_of_dt = parse_iso(as_of_v1)
                valid = True
                for r in recs:
                    if r.get("valid_from"):
                        vf = parse_iso(r["valid_from"])
                        if vf > as_of_dt:
                            valid = False
                    if r.get("valid_to"):
                        vt = parse_iso(r["valid_to"])
                        if vt <= as_of_dt:
                            valid = False
                p9_ok = valid
            else:
                p9_detail = f"Records empty: {d9}"
        else:
            p9_detail = f"Status {resp9.status_code}: {resp9.text}"
        report(9, "Historical sentiment time series (/sentiment/history) with as_of_utc filter", p9_ok, p9_detail)

        # ---------------------------------------------------------------------
        # Phase 10: Sentiment feed (/sentiment/feed) with as_of_utc filter
        # Feed bounded window 2025-06-15 to 2025-06-16 with as_of_utc 2025-06-15T18:00:00Z
        # ---------------------------------------------------------------------
        resp10 = client.get(
            "/sentiment/feed",
            params={
                "start_date": "2025-06-15T00:00:00Z",
                "end_date": "2025-06-16T12:00:00Z",
                "as_of_utc": "2025-06-15T18:00:00Z",
                "limit": 20,
            },
            headers=headers,
        )
        p10_ok = False
        p10_detail = ""
        if resp10.status_code == 200:
            d10 = resp10.json()
            items = d10.get("records", [])
            target_dt = parse_iso("2025-06-15T18:00:00Z")
            if len(items) > 0 and all(parse_iso(item["valid_from"]) <= target_dt for item in items if item.get("valid_from")):
                p10_ok = True
            else:
                p10_detail = f"Feed items count={len(items)}, payload={d10}"
        else:
            p10_detail = f"Status {resp10.status_code}: {resp10.text}"
        report(10, "Sentiment feed (/sentiment/feed) with as_of_utc filter", p10_ok, p10_detail)

        # ---------------------------------------------------------------------
        # Phase 11: Parameter validation: Malformed RFC3339 as_of_utc rejects with 400
        # ---------------------------------------------------------------------
        resp11a = client.get("/sentiment", params={"ticker": "AAPL", "as_of_utc": "invalid-timestamp"}, headers=headers)
        resp11b = client.get(
            "/sentiment/history",
            params={
                "ticker": "AAPL",
                "start_date": "2025-06-01",
                "end_date": "2025-06-30",
                "as_of_utc": "not_an_rfc3339_timestamp",
            },
            headers=headers,
        )
        p11_ok = (
            resp11a.status_code == 400
            and "as_of_utc" in resp11a.text
            and resp11b.status_code == 400
            and "as_of_utc" in resp11b.text
        )
        report(11, "Parameter validation: Malformed RFC3339 as_of_utc rejects with 400 Bad Request", p11_ok, f"a={resp11a.status_code}, b={resp11b.status_code}")

        # ---------------------------------------------------------------------
        # Phase 12: Invalidation & Python SDK Sync/Async Client integration
        # ---------------------------------------------------------------------
        # 12a: Invalid revision score (> 1.0) rejected by server
        resp12_invalid = client.post(
            "/sentiment/revision",
            json={"ticker": "AAPL", "title": "Invalid test", "sentiment_score": 2.5},
            headers=headers,
        )
        inv_score_ok = resp12_invalid.status_code == 400

        # 12b: Python SDK Sync client
        from fintext import FinTextClient, FinTextAsyncClient

        sdk_sync_ok = False
        p12_detail = ""
        try:
            sync_client = FinTextClient(base_url=BASE_URL, api_token=token)
            # Query revisions
            sdk_revs = sync_client.get_sentiment_revisions(ticker="AAPL")
            # Query as-of v1
            sdk_sent_v1 = sync_client.sentiment(ticker="AAPL", as_of_utc=as_of_v1)
            # Create revision via SDK
            sdk_ingest = sync_client.create_sentiment_revision(
                ticker="AAPL",
                title="Apple Q3 Guidance Revised Higher in SEC Form 8-K/A Amendment #3",
                sentiment_score=0.91,
                source="SEC EDGAR",
                source_id="0000320193-25-000105",
            )
            if (
                sdk_revs.total_revisions >= 3
                and sdk_sent_v1.revision_number == 1
                and sdk_ingest.active_revision.revision_number == 4
                and sdk_ingest.active_revision.is_current is True
            ):
                sdk_sync_ok = True
            else:
                p12_detail = f"Sync check: revs={sdk_revs.total_revisions}, v1_rev={sdk_sent_v1.revision_number}, new_rev={sdk_ingest.active_revision.revision_number}"
        except Exception as e:
            p12_detail = f"Sync SDK exception: {e}"

        # 12c: Python SDK Async client
        sdk_async_ok = False
        async def test_async():
            async with FinTextAsyncClient(base_url=BASE_URL, api_token=token) as aclient:
                arevs = await aclient.get_sentiment_revisions(ticker="AAPL")
                asent = await aclient.sentiment(ticker="AAPL", as_of_utc=as_of_v1)
                return arevs.total_revisions >= 4 and asent.revision_number == 1

        try:
            sdk_async_ok = asyncio.run(test_async())
        except Exception as e:
            p12_detail = f"Async SDK exception: {e}"

        p12_ok = inv_score_ok and sdk_sync_ok and sdk_async_ok
        report(12, "Invalidation & Python SDK Sync/Async Client integration", p12_ok, p12_detail)

    print("=" * 80)
    print(f" Summary: {passed}/{total_phases} Phases Passed")
    print("=" * 80)
    return 0 if passed == total_phases else 1


if __name__ == "__main__":
    sys.exit(run_all_phases())
