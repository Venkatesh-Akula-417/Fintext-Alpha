#!/usr/bin/env python3
"""
===============================================================================
FinText-Alpha-Vectorizer — Suite #217: Data Retention Policy Tool Certification
===============================================================================
Verifies:
  1.  Unauthenticated GET /retention/policies returns 401 Unauthorized
  2.  Initial GET /retention/policies returns 200 OK
  3.  Create retention policy for usage_events (POST /retention/policies -> 200 OK)
  4.  Create retention policy for audit_logs (POST /retention/policies -> 200 OK)
  5.  Retrieve and verify configured policies list (GET /retention/policies -> 200 OK)
  6.  Update existing policy (Upsert usage_events to 60 days -> 200 OK)
  7.  Input validation error handling (invalid category, days < 1, days > 3650 -> 400)
  8.  Multi-tenant policy isolation & cross-tenant access rejection
  9.  Additional categories configuration (transcripts, news_articles)
  10. Delete retention policy & 404 on subsequent deletion (DELETE /retention/policies/{id})
  11. Python SDK sync client integration (get, create, delete retention policy)
  12. Python SDK async client integration (get, create, delete retention policy)
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

PORT = 8117
BASE_URL = f"http://127.0.0.1:{PORT}"
ADMIN_TOKEN = "test_admin_token_xyz123_valid_32_bytes_length!"
SERVER_EXE = PROJECT_ROOT / "rust" / "target" / "debug" / "fintext_api.exe"

passed = 0
failed = 0
total = 12


def report(phase: int, name: str, ok: bool, detail: str = ""):
    global passed, failed
    if ok:
        passed += 1
        print(f"  ✅ Phase {phase:2d} │ {name}")
    else:
        failed += 1
        msg = f"  ❌ Phase {phase:2d} │ {name}"
        if detail:
            msg += f" — {detail}"
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

        self.process = subprocess.Popen(
            [str(SERVER_EXE)],
            env=env,
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
            if self.process.poll() is not None:
                raise RuntimeError("FinText API server process exited prematurely")
            raise RuntimeError("FinText API server failed to start within 25 seconds")

        print("[READY] FinText API Server is responding to health checks.\n")
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        if self.process:
            print("\n[STOPPING] Terminating FinText API Server process...")
            self.process.terminate()
            try:
                self.process.wait(timeout=5.0)
            except subprocess.TimeoutExpired:
                self.process.kill()


def get_auth_token(user_id: str = "compliance_officer_01", role: str = "institutional") -> str:
    r = httpx.post(
        f"{BASE_URL}/auth/token",
        json={"user_id": user_id, "role": role, "expires_in_seconds": 3600},
        headers={"X-Admin-Token": ADMIN_TOKEN},
        timeout=5.0,
    )
    if r.status_code != 200:
        raise RuntimeError(f"Failed to obtain auth token: {r.status_code} - {r.text}")
    return r.json()["token"]


def main():
    print("=" * 79)
    print("  FinText-Alpha-Vectorizer — Suite #217: Data Retention Policy Tool Certification")
    print("=" * 79)

    with ServerContext():
        token_u1 = get_auth_token("compliance_officer_alpha", role="institutional")
        token_u2 = get_auth_token("compliance_officer_beta", role="institutional")

        headers_u1 = {"Authorization": f"Bearer {token_u1}"}
        headers_u2 = {"Authorization": f"Bearer {token_u2}"}

        # ── Phase 1: Unauthenticated rejection ────────────────────────────────
        try:
            r = httpx.get(f"{BASE_URL}/retention/policies", timeout=5.0)
            ok = r.status_code == 401
            report(1, "Unauthenticated GET /retention/policies returns 401 Unauthorized", ok, f"status={r.status_code}")
        except Exception as e:
            report(1, "Unauthenticated GET /retention/policies returns 401 Unauthorized", False, str(e))

        # ── Phase 2: Initial policy listing ───────────────────────────────────
        try:
            r = httpx.get(f"{BASE_URL}/retention/policies", headers=headers_u1, timeout=5.0)
            ok = r.status_code == 200
            data = r.json()
            ok = ok and "policies" in data and "total_policies" in data
            report(2, "Initial GET /retention/policies returns 200 OK", ok, f"total={data.get('total_policies')}")
        except Exception as e:
            report(2, "Initial GET /retention/policies returns 200 OK", False, str(e))

        # ── Phase 3: Create retention policy for usage_events ──────────────────
        try:
            r_create1 = httpx.post(
                f"{BASE_URL}/retention/policies",
                json={"data_category": "usage_events", "retention_days": 90, "is_active": True},
                headers=headers_u1,
                timeout=5.0,
            )
            ok = r_create1.status_code == 200
            policy1 = r_create1.json()
            policy1_id = policy1.get("id")
            ok = ok and bool(policy1_id)
            ok = ok and policy1.get("data_category") == "usage_events"
            ok = ok and policy1.get("retention_days") == 90
            ok = ok and policy1.get("is_active") is True
            ok = ok and policy1.get("user_id") == "compliance_officer_alpha"
            report(3, "Create retention policy for usage_events (90 days -> 200 OK)", ok, f"id={policy1_id}")
        except Exception as e:
            report(3, "Create retention policy for usage_events (90 days -> 200 OK)", False, str(e))

        # ── Phase 4: Create retention policy for audit_logs ───────────────────
        try:
            r_create2 = httpx.post(
                f"{BASE_URL}/retention/policies",
                json={"data_category": "audit_logs", "retention_days": 180},
                headers=headers_u1,
                timeout=5.0,
            )
            ok = r_create2.status_code == 200
            policy2 = r_create2.json()
            policy2_id = policy2.get("id")
            ok = ok and bool(policy2_id)
            ok = ok and policy2.get("data_category") == "audit_logs"
            ok = ok and policy2.get("retention_days") == 180
            report(4, "Create retention policy for audit_logs (180 days -> 200 OK)", ok, f"id={policy2_id}")
        except Exception as e:
            report(4, "Create retention policy for audit_logs (180 days -> 200 OK)", False, str(e))

        # ── Phase 5: Retrieve and verify configured policies list ─────────────
        try:
            r_list = httpx.get(f"{BASE_URL}/retention/policies", headers=headers_u1, timeout=5.0)
            ok = r_list.status_code == 200
            list_data = r_list.json()
            cats = [p["data_category"] for p in list_data.get("policies", [])]
            ok = ok and list_data.get("total_policies") == 2
            ok = ok and "usage_events" in cats and "audit_logs" in cats
            report(5, "Retrieve and verify configured policies list (2 policies)", ok, f"categories={cats}")
        except Exception as e:
            report(5, "Retrieve and verify configured policies list (2 policies)", False, str(e))

        # ── Phase 6: Update existing policy (Upsert usage_events to 60 days) ───
        try:
            r_update = httpx.post(
                f"{BASE_URL}/retention/policies",
                json={"data_category": "usage_events", "retention_days": 60},
                headers=headers_u1,
                timeout=5.0,
            )
            ok = r_update.status_code == 200
            updated_p1 = r_update.json()
            ok = ok and updated_p1.get("id") == policy1_id
            ok = ok and updated_p1.get("retention_days") == 60
            report(6, "Update existing policy (Upsert usage_events to 60 days -> 200 OK)", ok, f"days={updated_p1.get('retention_days')}")
        except Exception as e:
            report(6, "Update existing policy (Upsert usage_events to 60 days -> 200 OK)", False, str(e))

        # ── Phase 7: Input validation error handling ──────────────────────────
        try:
            # 1. Invalid category
            r_bad_cat = httpx.post(
                f"{BASE_URL}/retention/policies",
                json={"data_category": "crypto_transactions", "retention_days": 90},
                headers=headers_u1,
                timeout=5.0,
            )
            # 2. Days < 1
            r_days_low = httpx.post(
                f"{BASE_URL}/retention/policies",
                json={"data_category": "usage_events", "retention_days": 0},
                headers=headers_u1,
                timeout=5.0,
            )
            # 3. Days > 3650
            r_days_high = httpx.post(
                f"{BASE_URL}/retention/policies",
                json={"data_category": "usage_events", "retention_days": 5000},
                headers=headers_u1,
                timeout=5.0,
            )
            ok = (
                r_bad_cat.status_code == 400
                and r_days_low.status_code == 400
                and r_days_high.status_code == 400
            )
            report(7, "Input validation error handling (invalid category, bounds -> 400)", ok,
                   f"bad_cat={r_bad_cat.status_code}, days_low={r_days_low.status_code}, days_high={r_days_high.status_code}")
        except Exception as e:
            report(7, "Input validation error handling (invalid category, bounds -> 400)", False, str(e))

        # ── Phase 8: Multi-tenant policy isolation ────────────────────────────
        try:
            # User 2 attempts to delete User 1's policy -> 404
            r_u2_del = httpx.delete(
                f"{BASE_URL}/retention/policies/{policy1_id}",
                headers=headers_u2,
                timeout=5.0,
            )
            # User 2 list should be empty
            r_u2_list = httpx.get(f"{BASE_URL}/retention/policies", headers=headers_u2, timeout=5.0)
            u2_data = r_u2_list.json()

            ok = r_u2_del.status_code == 404 and u2_data.get("total_policies") == 0
            report(8, "Multi-tenant policy isolation & cross-tenant deletion rejection", ok, f"del_status={r_u2_del.status_code}")
        except Exception as e:
            report(8, "Multi-tenant policy isolation & cross-tenant deletion rejection", False, str(e))

        # ── Phase 9: Additional categories configuration ──────────────────────
        try:
            r_transcripts = httpx.post(
                f"{BASE_URL}/retention/policies",
                json={"data_category": "transcripts", "retention_days": 365},
                headers=headers_u1,
                timeout=5.0,
            )
            r_news = httpx.post(
                f"{BASE_URL}/retention/policies",
                json={"data_category": "news_articles", "retention_days": 30},
                headers=headers_u1,
                timeout=5.0,
            )
            ok = r_transcripts.status_code == 200 and r_news.status_code == 200
            report(9, "Additional categories configuration (transcripts, news_articles)", ok)
        except Exception as e:
            report(9, "Additional categories configuration (transcripts, news_articles)", False, str(e))

        # ── Phase 10: Delete retention policy & 404 on subsequent deletion ────
        try:
            # 1. Delete usage_events policy
            r_del = httpx.delete(f"{BASE_URL}/retention/policies/{policy1_id}", headers=headers_u1, timeout=5.0)
            ok = r_del.status_code == 200
            del_resp = r_del.json()
            ok = ok and del_resp.get("status") == "deleted"
            ok = ok and del_resp.get("id") == policy1_id

            # 2. Subsequent DELETE -> 404
            r_del_again = httpx.delete(f"{BASE_URL}/retention/policies/{policy1_id}", headers=headers_u1, timeout=5.0)
            ok = ok and r_del_again.status_code == 404

            # 3. Non-existent UUID -> 404
            r_non_exist = httpx.delete(f"{BASE_URL}/retention/policies/{uuid.uuid4()}", headers=headers_u1, timeout=5.0)
            ok = ok and r_non_exist.status_code == 404

            report(10, "Delete retention policy & 404 on subsequent deletion (DELETE -> 200 OK, then 404)", ok)
        except Exception as e:
            report(10, "Delete retention policy & 404 on subsequent deletion (DELETE -> 200 OK, then 404)", False, str(e))

        # ── Phase 11: Python SDK sync client integration ──────────────────────
        try:
            from fintext import FinTextClient

            token_sdk_sync = get_auth_token("sdk_sync_compliance_user", role="institutional")
            with FinTextClient(base_url=BASE_URL, api_token=token_sdk_sync) as client:
                # 1. Get policies
                policies_resp = client.get_retention_policies()
                ok = isinstance(policies_resp.total_policies, int)

                # 2. Create policy
                new_policy = client.create_retention_policy(
                    data_category="email_digests",
                    retention_days=45,
                    is_active=True,
                )
                ok = ok and new_policy.data_category == "email_digests"
                ok = ok and new_policy.retention_days == 45

                # 3. Delete policy
                del_p = client.delete_retention_policy(new_policy.id)
                ok = ok and del_p.status == "deleted"
                ok = ok and del_p.id == new_policy.id

            report(11, "Python SDK sync client integration (get, create, delete)", ok)
        except Exception as e:
            report(11, "Python SDK sync client integration (get, create, delete)", False, str(e))

        # ── Phase 12: Python SDK async client integration ─────────────────────
        async def verify_async():
            from fintext import FinTextAsyncClient

            token_sdk_async = get_auth_token("sdk_async_compliance_user", role="institutional")
            async with FinTextAsyncClient(base_url=BASE_URL, api_token=token_sdk_async) as client:
                # 1. Get policies
                policies_resp = await client.get_retention_policies()
                ok = isinstance(policies_resp.total_policies, int)

                # 2. Create policy
                new_policy = await client.create_retention_policy(
                    data_category="kafka_credentials",
                    retention_days=14,
                )
                ok = ok and new_policy.data_category == "kafka_credentials"
                ok = ok and new_policy.retention_days == 14

                # 3. Delete policy
                del_p = await client.delete_retention_policy(new_policy.id)
                ok = ok and del_p.status == "deleted"
                ok = ok and del_p.id == new_policy.id
                return ok

        try:
            ok_async = asyncio.run(verify_async())
            report(12, "Python SDK async client integration (get, create, delete)", ok_async)
        except Exception as e:
            report(12, "Python SDK async client integration (get, create, delete)", False, str(e))

    # ── Summary ──────────────────────────────────────────────────────────────
    print("\n" + "=" * 79)
    pct = (passed / total) * 100.0
    print(f"  Suite #217 Results: {passed}/{total} Passed ({pct:.1f}%)")
    print("=" * 79)
    if failed == 0:
        print("\n🎉 ALL 12 PHASES PASSED CLEANLY!\n")
    else:
        print(f"\n❌ {failed} PHASES FAILED!\n")
        sys.exit(1)


if __name__ == "__main__":
    main()
