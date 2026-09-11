#!/usr/bin/env python3
"""
===============================================================================
FinText-Alpha-Vectorizer — Suite #235: Dead Letter Queue (DLQ) Monitoring & Auto-Reprocessing Engine
===============================================================================
Verifies:
  1.  Unauthenticated GET /dlq/events returns 401 Unauthorized
  2.  Retail user (non-admin role) GET /dlq/events returns 403 Forbidden
  3.  Admin user GET /dlq/events returns 200 OK with list of seeded DLQ events (total >= 4)
  4.  Filter DLQ events by source=sentiment returns matching events
  5.  Filter DLQ events by error_type=TimeoutError returns matching events
  6.  Filter DLQ events by status=reprocessed returns matching events
  7.  Paginate DLQ events with limit=2&offset=1 returns correct pagination metadata
  8.  Get specific DLQ event by UUID (GET /dlq/events/{id}) returns 200 OK with full payload
  9.  Get DLQ event by non-existent UUID returns 404 Not Found
  10. Reprocess DLQ event (POST /dlq/events/{id}/reprocess) returns 200 OK with status reprocessed
  11. Purge DLQ event (DELETE /dlq/events/{id}) returns 200 OK with status purged
  12. Attempt to reprocess purged event returns 400 Bad Request
  13. Python SDK Sync Client integration verification (list_dlq_events, get_dlq_event, reprocess_dlq_event, purge_dlq_event)
  14. Python SDK Async Client integration verification (list_dlq_events, get_dlq_event, reprocess_dlq_event, purge_dlq_event)
  15. Audit log verification (dlq.event_reprocessed and dlq.event_purged recorded)
===============================================================================
"""

import asyncio
import os
from pathlib import Path
import subprocess
import sys
import time
import uuid

import httpx

if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
if hasattr(sys.stderr, "reconfigure"):
    sys.stderr.reconfigure(encoding="utf-8", errors="replace")

PROJECT_ROOT = Path(__file__).resolve().parent.parent
SDK_PATH = PROJECT_ROOT / "python_sdk" / "src"
if str(SDK_PATH) not in sys.path:
    sys.path.insert(0, str(SDK_PATH))

PORT = 8134
BASE_URL = f"http://127.0.0.1:{PORT}"
ADMIN_TOKEN = "test_admin_token_xyz123_valid_32_bytes_length!"
SERVER_EXE = PROJECT_ROOT / "rust" / "target" / "debug" / "fintext_api.exe"

passed = 0
failed = 0
total = 15


def report(phase: int, name: str, ok: bool, detail: str = ""):
    global passed, failed
    if ok:
        passed += 1
        print(f"  \u2705 Phase {phase:2d} \u2502 {name}")
    else:
        failed += 1
        msg = f"  \u274c Phase {phase:2d} \u2502 {name}"
        if detail:
            msg += f" \u2014 {detail}"
        print(msg)


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
                pass
            time.sleep(0.5)

        if not ready:
            if self.process:
                self.process.kill()
            raise RuntimeError(f"Server on port {PORT} failed to start within timeout.")
        print(f"[ONLINE] FinText API Server is ready on {BASE_URL}")
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        if self.process:
            print(f"[CLEANUP] Stopping API Server on port {PORT}...")
            self.process.terminate()
            try:
                self.process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.process.kill()
            print("[CLEANUP] API Server stopped.")


def issue_token(user_id: str, role: str) -> str:
    r = httpx.post(
        f"{BASE_URL}/auth/token",
        headers={"X-Admin-Token": ADMIN_TOKEN, "Content-Type": "application/json"},
        json={"user_id": user_id, "role": role},
        timeout=5.0,
    )
    if r.status_code != 200:
        raise RuntimeError(f"Failed to issue token for {user_id}: {r.status_code} {r.text}")
    return r.json()["token"]


def run_tests():
    global passed, failed

    print("\n" + "═" * 79)
    print("  FINTEXT ALPHA VECTORIZER — DEAD LETTER QUEUE (DLQ) MONITORING SUITE")
    print("═" * 79)

    with ServerContext():
        client = httpx.Client(base_url=BASE_URL, timeout=10.0)

        admin_token = issue_token("admin_ops_user", "admin")
        retail_token = issue_token("retail_trader_user", "retail")

        admin_headers = {"Authorization": f"Bearer {admin_token}"}
        retail_headers = {"Authorization": f"Bearer {retail_token}"}

        # ─── Phase 1: Unauthenticated GET /dlq/events ─────────────────────────
        try:
            r1 = client.get("/dlq/events")
            report(1, "Unauthenticated GET /dlq/events returns 401 Unauthorized", r1.status_code == 401)
        except Exception as e:
            report(1, "Unauthenticated GET /dlq/events", False, str(e))

        # ─── Phase 2: Retail user GET /dlq/events ────────────────────────────
        try:
            r2 = client.get("/dlq/events", headers=retail_headers)
            report(2, "Retail user GET /dlq/events returns 403 Forbidden", r2.status_code == 403)
        except Exception as e:
            report(2, "Retail user GET /dlq/events", False, str(e))

        # ─── Phase 3: Admin user GET /dlq/events ─────────────────────────────
        seed_events = []
        try:
            r3 = client.get("/dlq/events?status=all", headers=admin_headers)
            data3 = r3.json()
            ok3 = r3.status_code == 200 and data3.get("total", 0) >= 4 and len(data3.get("events", [])) >= 4
            seed_events = data3.get("events", [])
            report(3, "Admin user GET /dlq/events returns 200 OK with seeded records", ok3, f"Total: {data3.get('total')}")
        except Exception as e:
            report(3, "Admin user GET /dlq/events", False, str(e))

        # ─── Phase 4: Filter DLQ events by source=sentiment ──────────────────
        try:
            r4 = client.get("/dlq/events?source=sentiment&status=all", headers=admin_headers)
            data4 = r4.json()
            events4 = data4.get("events", [])
            ok4 = r4.status_code == 200 and len(events4) >= 1 and all(e["source"] == "sentiment" for e in events4)
            report(4, "Filter DLQ events by source=sentiment returns matching events", ok4, f"Count: {len(events4)}")
        except Exception as e:
            report(4, "Filter DLQ events by source=sentiment", False, str(e))

        # ─── Phase 5: Filter DLQ events by error_type=TimeoutError ───────────
        try:
            r5 = client.get("/dlq/events?error_type=TimeoutError&status=all", headers=admin_headers)
            data5 = r5.json()
            events5 = data5.get("events", [])
            ok5 = r5.status_code == 200 and len(events5) >= 1 and all("TimeoutError" in (e.get("error_type") or "") for e in events5)
            report(5, "Filter DLQ events by error_type=TimeoutError", ok5, f"Count: {len(events5)}")
        except Exception as e:
            report(5, "Filter DLQ events by error_type=TimeoutError", False, str(e))

        # ─── Phase 6: Filter DLQ events by status=reprocessed ────────────────
        try:
            r6 = client.get("/dlq/events?status=reprocessed", headers=admin_headers)
            data6 = r6.json()
            events6 = data6.get("events", [])
            ok6 = r6.status_code == 200 and len(events6) >= 1 and all(e["status"] == "reprocessed" for e in events6)
            report(6, "Filter DLQ events by status=reprocessed returns matching events", ok6, f"Count: {len(events6)}")
        except Exception as e:
            report(6, "Filter DLQ events by status=reprocessed", False, str(e))

        # ─── Phase 7: Paginate DLQ events with limit=2&offset=1 ──────────────
        try:
            r7 = client.get("/dlq/events?limit=2&offset=1&status=all", headers=admin_headers)
            data7 = r7.json()
            ok7 = r7.status_code == 200 and data7.get("limit") == 2 and data7.get("offset") == 1 and len(data7.get("events", [])) == 2
            report(7, "Paginate DLQ events with limit=2&offset=1 returns correct page", ok7, f"Returned: {len(data7.get('events', []))}")
        except Exception as e:
            report(7, "Paginate DLQ events", False, str(e))

        # ─── Phase 8: Get specific DLQ event by UUID ─────────────────────────
        target_uuid = "550e8400-e29b-41d4-a716-446655440101"
        try:
            r8 = client.get(f"/dlq/events/{target_uuid}", headers=admin_headers)
            data8 = r8.json()
            ok8 = r8.status_code == 200 and data8.get("id") == target_uuid and isinstance(data8.get("payload"), dict)
            report(8, f"Get DLQ event by UUID (GET /dlq/events/{target_uuid}) returns full payload", ok8)
        except Exception as e:
            report(8, "Get DLQ event by UUID", False, str(e))

        # ─── Phase 9: Get DLQ event by non-existent UUID ─────────────────────
        random_uuid = str(uuid.uuid4())
        try:
            r9 = client.get(f"/dlq/events/{random_uuid}", headers=admin_headers)
            report(9, "Get DLQ event by non-existent UUID returns 404 Not Found", r9.status_code == 404)
        except Exception as e:
            report(9, "Get DLQ event by non-existent UUID", False, str(e))

        # ─── Phase 10: Reprocess DLQ event ───────────────────────────────────
        try:
            r10 = client.post(f"/dlq/events/{target_uuid}/reprocess", headers=admin_headers)
            data10 = r10.json()
            ok10 = r10.status_code == 200 and data10.get("status") == "reprocessed" and data10.get("retry_count") == 3
            report(10, "Reprocess DLQ event (POST /dlq/events/{id}/reprocess) returns 200 OK", ok10, f"Status: {data10.get('status')}, RetryCount: {data10.get('retry_count')}")
        except Exception as e:
            report(10, "Reprocess DLQ event", False, str(e))

        # ─── Phase 11: Purge DLQ event ───────────────────────────────────────
        try:
            r11 = client.delete(f"/dlq/events/{target_uuid}", headers=admin_headers)
            data11 = r11.json()
            ok11 = r11.status_code == 200 and data11.get("status") == "purged"
            report(11, "Purge DLQ event (DELETE /dlq/events/{id}) marks status as purged", ok11, f"Status: {data11.get('status')}")
        except Exception as e:
            report(11, "Purge DLQ event", False, str(e))

        # ─── Phase 12: Reprocess purged event returns 400 Bad Request ────────
        try:
            r12 = client.post(f"/dlq/events/{target_uuid}/reprocess", headers=admin_headers)
            report(12, "Reprocess purged event returns 400 Bad Request", r12.status_code == 400)
        except Exception as e:
            report(12, "Reprocess purged event returns 400", False, str(e))

        # ─── Phase 13: Python SDK Sync Client integration verification ───────
        try:
            from fintext import (
                FinTextClient,
                DLQEventsListResponse,
                DLQEventDetail,
                ReprocessDLQResponse,
                PurgeDLQResponse,
            )

            sdk_client = FinTextClient(
                base_url=BASE_URL,
                api_token=admin_token,
            )

            # 1. list_dlq_events
            sdk_list = sdk_client.list_dlq_events(source="ingestion", status="all", limit=5)
            assert isinstance(sdk_list, DLQEventsListResponse)
            assert len(sdk_list.events) >= 1

            # 2. get_dlq_event
            target_ingest_id = "550e8400-e29b-41d4-a716-446655440102"
            sdk_detail = sdk_client.get_dlq_event(target_ingest_id)
            assert isinstance(sdk_detail, DLQEventDetail)
            assert sdk_detail.source == "ingestion"

            # 3. reprocess_dlq_event
            sdk_reproc = sdk_client.reprocess_dlq_event(target_ingest_id)
            assert isinstance(sdk_reproc, ReprocessDLQResponse)
            assert sdk_reproc.status == "reprocessed"

            # 4. purge_dlq_event
            sdk_purge = sdk_client.purge_dlq_event(target_ingest_id)
            assert isinstance(sdk_purge, PurgeDLQResponse)
            assert sdk_purge.status == "purged"

            sdk_client.close()
            report(13, "Python SDK Sync Client integration verified (list, get, reprocess, purge)", True)
        except Exception as e:
            report(13, "Python SDK Sync Client integration", False, str(e))

        # ─── Phase 14: Python SDK Async Client integration verification ──────
        async def verify_async_sdk():
            from fintext import (
                FinTextAsyncClient,
                DLQEventsListResponse,
                DLQEventDetail,
                ReprocessDLQResponse,
                PurgeDLQResponse,
            )

            async_client = FinTextAsyncClient(
                base_url=BASE_URL,
                api_token=admin_token,
            )

            # 1. list_dlq_events
            async_list = await async_client.list_dlq_events(source="options", status="all", limit=5)
            assert isinstance(async_list, DLQEventsListResponse)
            assert len(async_list.events) >= 1

            # 2. get_dlq_event
            target_opt_id = "550e8400-e29b-41d4-a716-446655440103"
            async_detail = await async_client.get_dlq_event(target_opt_id)
            assert isinstance(async_detail, DLQEventDetail)
            assert async_detail.source == "options"

            # 3. reprocess_dlq_event
            async_reproc = await async_client.reprocess_dlq_event(target_opt_id)
            assert isinstance(async_reproc, ReprocessDLQResponse)
            assert async_reproc.status == "reprocessed"

            # 4. purge_dlq_event
            async_purge = await async_client.purge_dlq_event(target_opt_id)
            assert isinstance(async_purge, PurgeDLQResponse)
            assert async_purge.status == "purged"

            await async_client.close()

        try:
            asyncio.run(verify_async_sdk())
            report(14, "Python SDK Async Client integration verified (list, get, reprocess, purge)", True)
        except Exception as e:
            report(14, "Python SDK Async Client integration", False, str(e))

        # ─── Phase 15: Audit log verification ────────────────────────────────
        try:
            r15 = client.get("/audit/logs?limit=50", headers=admin_headers)
            logs_data = r15.json()
            actions = [log.get("action") for log in logs_data.get("logs", [])]
            reprocessed_logged = "dlq.event_reprocessed" in actions
            purged_logged = "dlq.event_purged" in actions
            ok15 = r15.status_code == 200 and reprocessed_logged and purged_logged
            report(15, "Audit log verification: dlq.event_reprocessed & dlq.event_purged recorded", ok15, f"Logged actions count: {len(actions)}")
        except Exception as e:
            report(15, "Audit log verification", False, str(e))

    print("═" * 79)
    print(f"  DLQ MONITORING ENGINE TEST RESULTS: {passed}/{total} PHASES PASSED")
    print("═" * 79 + "\n")

    if failed > 0:
        sys.exit(1)


if __name__ == "__main__":
    run_tests()
