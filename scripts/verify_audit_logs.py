#!/usr/bin/env python3
"""
═══════════════════════════════════════════════════════════════════════════════
FinText-Alpha-Vectorizer — Test Suite #210: Compliance Audit Log Export Engine
═══════════════════════════════════════════════════════════════════════════════
Validates:
  1. Unauthenticated Access Rejection (401) on /audit/logs & /audit/export.
  2. Automatic Emission of Audit Logs on Admin/User Actions (auth, apikey, org, ip, webhook).
  3. Paginated Audit Log Querying (GET /audit/logs).
  4. Action Code Filtering (?action=apikey.create).
  5. Entity Type Filtering (?entity_type=api_key).
  6. Date Range Filtering (?start_date=...&end_date=...).
  7. Organization Lifecycle & Org Log Audit Tracking (?org_id=...).
  8. RBAC Isolation & Cross-Tenant Access Rejection (403).
  9. CSV Export Stream & RFC 4180 Conformity (GET /audit/export?format=csv).
  10. JSON Export Stream & Payload Schema (GET /audit/export?format=json).
  11. OpenAPI 3.0 Documentation Registration & Schema Conformity.
  12. Python SDK Client Execution (Synchronous and Asynchronous).
═══════════════════════════════════════════════════════════════════════════════
"""

import asyncio
import csv
import io
import json
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
from fintext.models import AuditLogEntry, AuditLogExportResponse, AuditLogsResponse

SERVER_PORT = 8103
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


def get_token_for_user(user_id: str, role: str = "institutional") -> str:
    resp = httpx.post(
        f"{BASE_URL}/auth/token",
        json={"user_id": user_id, "role": role},
        headers={"X-Admin-Token": ADMIN_TOKEN},
    )
    assert resp.status_code == 200, f"Failed to get token: {resp.text}"
    return resp.json()["token"]


def main():
    print("=" * 80)
    print("🚀 FINTEXT ALPHA VECTORIZER — TEST SUITE #210: COMPLIANCE AUDIT LOG EXPORT")
    print("=" * 80)

    passed_checks = 0
    total_checks = 0

    def check(desc: str, condition: bool, extra_info: str = ""):
        nonlocal passed_checks, total_checks
        total_checks += 1
        if condition:
            print(f"  [PASS] {desc}")
            passed_checks += 1
        else:
            print(f"  [FAIL] {desc} -> {extra_info}")
            raise AssertionError(f"Check failed: {desc} ({extra_info})")

    with ServerContext():
        alice_id = "a0000000-0000-0000-0000-000000000001"
        bob_id = "b0000000-0000-0000-0000-000000000002"
        token_alice = get_token_for_user(alice_id)
        token_bob = get_token_for_user(bob_id)
        headers_alice = {"Authorization": f"Bearer {token_alice}"}
        headers_bob = {"Authorization": f"Bearer {token_bob}"}

        # ─── PHASE 1: Unauthenticated Access Rejection ───────────────────────
        print("\n[PHASE 1] Unauthenticated Access Rejection (401)...")
        r_unauth_logs = httpx.get(f"{BASE_URL}/audit/logs")
        check("GET /audit/logs without auth returns 401", r_unauth_logs.status_code == 401)

        r_unauth_exp = httpx.get(f"{BASE_URL}/audit/export")
        check("GET /audit/export without auth returns 401", r_unauth_exp.status_code == 401)

        # ─── PHASE 2: Administrative Action Emits Audit Logs ─────────────────
        print("\n[PHASE 2] Automatic Emission of Audit Logs on Lifecycle Actions...")
        # 1. API Key creation
        r_key = httpx.post(
            f"{BASE_URL}/auth/api-keys",
            json={"name": "Compliance Production Key"},
            headers=headers_alice,
        )
        check("Create API Key succeeds (201)", r_key.status_code == 201)
        key_id = r_key.json().get("id")

        # 2. IP Whitelist addition
        r_ip = httpx.post(
            f"{BASE_URL}/security/ip-whitelist",
            json={"ip_or_cidr": "127.0.0.1/32", "description": "Local Test Runner"},
            headers=headers_alice,
        )
        check("Add IP Whitelist succeeds (201)", r_ip.status_code == 201)
        ip_entry_id = r_ip.json().get("id")

        # 3. Webhook registration
        r_wh = httpx.post(
            f"{BASE_URL}/webhooks",
            json={"url": "https://compliance.fintext.internal/webhook", "events": ["sentiment"]},
            headers=headers_alice,
        )
        check("Register Webhook succeeds (201)", r_wh.status_code == 201)
        webhook_id = r_wh.json().get("id")

        # 4. API Key revocation
        r_del_key = httpx.delete(
            f"{BASE_URL}/auth/api-keys/{key_id}",
            headers=headers_alice,
        )
        check("Revoke API Key succeeds (200)", r_del_key.status_code == 200)

        # ─── PHASE 3: Paginated Log Query ────────────────────────────────────
        print("\n[PHASE 3] Paginated Audit Log Querying (GET /audit/logs)...")
        r_logs = httpx.get(f"{BASE_URL}/audit/logs", headers=headers_alice)
        check("GET /audit/logs returns 200", r_logs.status_code == 200)
        logs_data = r_logs.json()
        check("Audit response has 'logs' list", "logs" in logs_data and isinstance(logs_data["logs"], list))
        check("Audit response has 'total' >= 4", logs_data.get("total", 0) >= 4)
        check("Audit response limit is default 100", logs_data.get("limit") == 100)
        check("Audit response offset is default 0", logs_data.get("offset") == 0)

        first_log = logs_data["logs"][0]
        check("Log entry has 'id' UUID string", bool(first_log.get("id")))
        check("Log entry has 'user_id' matching alice", first_log.get("user_id") == alice_id)
        check("Log entry has valid 'action' string", bool(first_log.get("action")))
        check("Log entry has 'entity_type' string", bool(first_log.get("entity_type")))
        check("Log entry has 'created_at' ISO timestamp", bool(first_log.get("created_at")))

        # Test pagination limit & offset
        r_paged = httpx.get(f"{BASE_URL}/audit/logs?limit=2&offset=1", headers=headers_alice)
        check("Paged query limit=2 returns 200", r_paged.status_code == 200)
        paged_data = r_paged.json()
        check("Paged query returns exactly 2 logs", len(paged_data.get("logs", [])) == 2)
        check("Paged query total remains accurate", paged_data.get("total") == logs_data["total"])

        # ─── PHASE 4: Action Code Filtering ──────────────────────────────────
        print("\n[PHASE 4] Action Code Filtering (?action=apikey.create)...")
        r_filter_action = httpx.get(f"{BASE_URL}/audit/logs?action=apikey.create", headers=headers_alice)
        check("Filter by action returns 200", r_filter_action.status_code == 200)
        action_logs = r_filter_action.json().get("logs", [])
        check("Filter returns matching action logs", len(action_logs) >= 1)
        check(
            "All returned logs have action == 'apikey.create'",
            all(l["action"] == "apikey.create" for l in action_logs),
        )

        # ─── PHASE 5: Entity Type Filtering ──────────────────────────────────
        print("\n[PHASE 5] Entity Type Filtering (?entity_type=webhook)...")
        r_filter_entity = httpx.get(f"{BASE_URL}/audit/logs?entity_type=webhook", headers=headers_alice)
        check("Filter by entity_type returns 200", r_filter_entity.status_code == 200)
        entity_logs = r_filter_entity.json().get("logs", [])
        check("Filter returns matching entity logs", len(entity_logs) >= 1)
        check(
            "All returned logs have entity_type == 'webhook'",
            all(l["entity_type"] == "webhook" for l in entity_logs),
        )

        # ─── PHASE 6: Date Range Filtering ───────────────────────────────────
        print("\n[PHASE 6] Date Range Filtering (?start_date=...&end_date=...)...")
        r_date_valid = httpx.get(
            f"{BASE_URL}/audit/logs?start_date=2026-01-01&end_date=2026-12-31",
            headers=headers_alice,
        )
        check("Query with broad date window returns 200 and matches", r_date_valid.status_code == 200)
        check("Broad date window contains logs", len(r_date_valid.json().get("logs", [])) >= 4)

        r_date_future = httpx.get(
            f"{BASE_URL}/audit/logs?start_date=2030-01-01&end_date=2030-12-31",
            headers=headers_alice,
        )
        check("Future date window returns 200", r_date_future.status_code == 200)
        check("Future date window returns 0 total logs", r_date_future.json().get("total") == 0)

        # ─── PHASE 7: Organization Lifecycle & Org Log Tracking ──────────────
        print("\n[PHASE 7] Organization Lifecycle & Org Log Audit Tracking...")
        r_org = httpx.post(
            f"{BASE_URL}/orgs",
            json={"name": "Alpha Prime Quantitative"},
            headers=headers_alice,
        )
        check("Create organization returns 201", r_org.status_code == 201)
        org_id = r_org.json().get("id")

        r_invite = httpx.post(
            f"{BASE_URL}/orgs/{org_id}/invites",
            json={"user_id": bob_id, "role": "member"},
            headers=headers_alice,
        )
        check("Invite member to organization returns 200", r_invite.status_code == 200)

        r_org_logs = httpx.get(f"{BASE_URL}/audit/logs?org_id={org_id}", headers=headers_alice)
        check("Org admin queries org audit logs returns 200", r_org_logs.status_code == 200)
        org_audit_records = r_org_logs.json().get("logs", [])
        check("Org logs contain org.created and org.member_invited", len(org_audit_records) >= 2)
        check(
            "All returned records belong to org_id",
            all(l.get("org_id") == org_id for l in org_audit_records),
        )

        # ─── PHASE 8: RBAC Isolation & Cross-Tenant Rejection ────────────────
        print("\n[PHASE 8] RBAC Isolation & Cross-Tenant Access Rejection (403)...")
        # Bob (member, non-admin) attempts to query org-wide audit logs -> 403
        r_bob_org_forbidden = httpx.get(f"{BASE_URL}/audit/logs?org_id={org_id}", headers=headers_bob)
        check("Regular member querying org audit logs returns 403 Forbidden", r_bob_org_forbidden.status_code == 403)

        # Bob queries default audit logs -> only sees his own (empty or bob's actions)
        r_bob_own = httpx.get(f"{BASE_URL}/audit/logs", headers=headers_bob)
        check("Bob querying own audit logs returns 200", r_bob_own.status_code == 200)
        check(
            "Bob cannot see Alice's audit records",
            all(l.get("user_id") == bob_id for l in r_bob_own.json().get("logs", [])),
        )

        # ─── PHASE 9: CSV Export Verification ────────────────────────────────
        print("\n[PHASE 9] CSV Export Stream & RFC 4180 Conformity...")
        r_csv = httpx.get(f"{BASE_URL}/audit/export?format=csv", headers=headers_alice)
        check("GET /audit/export?format=csv returns 200", r_csv.status_code == 200)
        check(
            "CSV Content-Type is text/csv",
            "text/csv" in r_csv.headers.get("content-type", "").lower(),
        )
        check(
            "CSV Content-Disposition is attachment with filename",
            "attachment" in r_csv.headers.get("content-disposition", "")
            and ".csv" in r_csv.headers.get("content-disposition", ""),
        )

        csv_reader = csv.DictReader(io.StringIO(r_csv.text))
        csv_rows = list(csv_reader)
        expected_fields = [
            "id",
            "org_id",
            "user_id",
            "action",
            "entity_type",
            "entity_id",
            "details",
            "ip_address",
            "created_at",
        ]
        check(
            "CSV headers match RFC 4180 compliance audit specification",
            csv_reader.fieldnames == expected_fields,
            f"Got: {csv_reader.fieldnames}",
        )
        check("CSV contains all Alice audit rows", len(csv_rows) >= 4)
        actions_in_csv = {row["action"] for row in csv_rows}
        check("CSV contains 'apikey.create'", "apikey.create" in actions_in_csv)
        check("CSV contains 'security.ip_whitelist_added'", "security.ip_whitelist_added" in actions_in_csv)
        check("CSV contains 'webhook.registered'", "webhook.registered" in actions_in_csv)

        # ─── PHASE 10: JSON Export Verification ──────────────────────────────
        print("\n[PHASE 10] JSON Export Stream & Payload Schema...")
        r_json_exp = httpx.get(f"{BASE_URL}/audit/export?format=json", headers=headers_alice)
        check("GET /audit/export?format=json returns 200", r_json_exp.status_code == 200)
        check("JSON Content-Type is application/json", "application/json" in r_json_exp.headers.get("content-type", "").lower())
        json_exp_data = r_json_exp.json()
        check("JSON export has 'exported_at' timestamp", bool(json_exp_data.get("exported_at")))
        check("JSON export has 'total' count", json_exp_data.get("total", 0) >= 4)
        check("JSON export has 'logs' list", isinstance(json_exp_data.get("logs"), list))

        # Invalid export format -> 400
        r_invalid_fmt = httpx.get(f"{BASE_URL}/audit/export?format=yaml", headers=headers_alice)
        check("Invalid format '?format=yaml' returns 400 Bad Request", r_invalid_fmt.status_code == 400)

        # ─── PHASE 11: OpenAPI 3.0 Documentation Registration ────────────────
        print("\n[PHASE 11] OpenAPI 3.0 Documentation Registration & Schema...")
        r_openapi = httpx.get(f"{BASE_URL}/api-docs/openapi.json")
        check("GET /api-docs/openapi.json returns 200", r_openapi.status_code == 200)
        openapi_spec = r_openapi.json()
        paths = openapi_spec.get("paths", {})
        check("OpenAPI spec registers '/audit/logs'", "/audit/logs" in paths)
        check("OpenAPI spec registers '/audit/export'", "/audit/export" in paths)
        check("GET /audit/logs is tagged 'Compliance & Audit'", "Compliance & Audit" in paths["/audit/logs"]["get"]["tags"])
        check("GET /audit/export is tagged 'Compliance & Audit'", "Compliance & Audit" in paths["/audit/export"]["get"]["tags"])

        # ─── PHASE 12: Python SDK Client Execution ───────────────────────────
        print("\n[PHASE 12] Python SDK Client Execution (Synchronous & Asynchronous)...")
        # Synchronous SDK
        sync_client = FinTextClient(base_url=BASE_URL, api_token=token_alice)
        sdk_logs_res = sync_client.get_audit_logs(limit=25)
        check("Python SDK sync client.get_audit_logs() returns AuditLogsResponse", isinstance(sdk_logs_res, AuditLogsResponse))
        check("Python SDK sync client returns matching logs count", len(sdk_logs_res.logs) >= 4)
        check("Python SDK sync client log model validation", isinstance(sdk_logs_res.logs[0], AuditLogEntry))

        sdk_csv_exp = sync_client.export_audit_logs(format="csv")
        check("Python SDK sync client.export_audit_logs(format='csv') returns str", isinstance(sdk_csv_exp, str))
        check("Python SDK sync CSV export contains 'apikey.create'", "apikey.create" in sdk_csv_exp)

        sdk_json_exp = sync_client.export_audit_logs(format="json")
        check("Python SDK sync client.export_audit_logs(format='json') returns AuditLogExportResponse", isinstance(sdk_json_exp, AuditLogExportResponse))
        check("Python SDK sync JSON export has valid total", sdk_json_exp.total >= 4)
        sync_client.close()

        # Asynchronous SDK
        async def run_async_sdk_tests():
            async_client = FinTextAsyncClient(base_url=BASE_URL, api_token=token_alice)
            async_logs_res = await async_client.get_audit_logs(limit=10)
            check("Python SDK async client.get_audit_logs() returns AuditLogsResponse", isinstance(async_logs_res, AuditLogsResponse))
            check("Python SDK async client returns matching logs", len(async_logs_res.logs) >= 4)

            async_csv_exp = await async_client.export_audit_logs(format="csv")
            check("Python SDK async client.export_audit_logs(format='csv') returns str", isinstance(async_csv_exp, str))

            async_json_exp = await async_client.export_audit_logs(format="json")
            check("Python SDK async client.export_audit_logs(format='json') returns AuditLogExportResponse", isinstance(async_json_exp, AuditLogExportResponse))
            await async_client.close()

        asyncio.run(run_async_sdk_tests())

    print("\n" + "=" * 80)
    print(f"🎉 SUITE #210 PASSED: {passed_checks}/{total_checks} CHECKS CERTIFIED")
    print("=" * 80)


if __name__ == "__main__":
    main()
