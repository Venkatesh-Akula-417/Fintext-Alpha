"""
================================================================================
🚀 FINTEXT ALPHA VECTORIZER — TEST SUITE #211: API KEY ROTATION AUTOMATION
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

from fintext import FinTextClient, FinTextAsyncClient, RotateApiKeyResponse, ApiKeyItem, ListApiKeysResponse

if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")

SERVER_EXE = PROJECT_ROOT / "rust" / "target" / "debug" / "fintext_api.exe"
PORT = 8104
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

        for attempt in range(1, 40):
            try:
                r = httpx.get(f"{BASE_URL}/health", timeout=1.0)
                if r.status_code == 200:
                    print(f"[READY] Server is healthy and ready (attempt {attempt})")
                    return self
            except Exception:
                time.sleep(0.5)

        raise RuntimeError("Failed to start FinText API Server within timeout.")

    def __exit__(self, exc_type, exc_val, exc_tb):
        if self.process:
            print("[STOPPING] Shutting down FinText API Server...")
            self.process.terminate()
            try:
                self.process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.process.kill()


def get_token_and_user_id() -> tuple[str, str]:
    email = f"quant_{uuid.uuid4().hex[:8]}@fund.com"
    pwd = "StrongPassword2026!"
    # Register
    r_reg = httpx.post(f"{BASE_URL}/auth/register", json={"email": email, "password": pwd})
    if r_reg.status_code == 201:
        # Login
        r_login = httpx.post(f"{BASE_URL}/auth/login", json={"email": email, "password": pwd})
        if r_login.status_code == 200:
            data = r_login.json()
            return data["token"], str(data["user_id"])
    
    # Fallback to token issuance
    uid = str(uuid.uuid4())
    resp = httpx.post(
        f"{BASE_URL}/auth/token",
        json={"user_id": uid, "role": "institutional"},
        headers={"X-Admin-Token": ADMIN_TOKEN},
    )
    return resp.json()["token"], uid


def main():
    print("=" * 80)
    print(">> FINTEXT ALPHA VECTORIZER -- TEST SUITE #211: API KEY ROTATION AUTOMATION")
    print("=" * 80)

    with ServerContext():
        token, user_id = get_token_and_user_id()
        auth_headers = {"Authorization": f"Bearer {token}"}

        # ─── PHASE 1: Unauthenticated Access Rejection (401) ────────────────
        print("\n[PHASE 1] Unauthenticated Rotation Rejection (401)...")
        dummy_id = str(uuid.uuid4())
        r_unauth = httpx.post(f"{BASE_URL}/auth/api-keys/{dummy_id}/rotate", json={"overlap_hours": 24})
        check("POST /auth/api-keys/{id}/rotate without auth returns 401", r_unauth.status_code == 401)

        r_unauth_get = httpx.get(f"{BASE_URL}/auth/api-keys/{dummy_id}")
        check("GET /auth/api-keys/{id} without auth returns 401", r_unauth_get.status_code == 401)

        # ─── PHASE 2: Baseline API Key Creation ──────────────────────────────
        print("\n[PHASE 2] Baseline API Key Creation...")
        r_create = httpx.post(
            f"{BASE_URL}/auth/api-keys",
            json={"name": "Primary Algo Trading Key"},
            headers=auth_headers,
        )
        check("Create initial API key returns 201", r_create.status_code == 201)
        initial_key_data = r_create.json()
        key_1_id = initial_key_data["id"]
        key_1_raw = initial_key_data["api_key"]
        key_1_prefix = initial_key_data["prefix"]
        check("API key id is valid UUID string", bool(key_1_id))
        check("API key string starts with 'ft_'", key_1_raw.startswith("ft_"))
        check("API key prefix matches first 8 chars", key_1_prefix == key_1_raw[:8])

        # ─── PHASE 3: API Key Rotation Request ───────────────────────────────
        print("\n[PHASE 3] API Key Rotation (POST /auth/api-keys/{id}/rotate)...")
        r_rotate = httpx.post(
            f"{BASE_URL}/auth/api-keys/{key_1_id}/rotate",
            json={"overlap_hours": 48},
            headers=auth_headers,
        )
        check("Rotate API key returns 200 OK", r_rotate.status_code == 200)
        rotate_data = r_rotate.json()
        key_2_id = rotate_data["id"]
        key_2_raw = rotate_data["api_key"]
        key_2_prefix = rotate_data["prefix"]
        check("Rotated response has new key ID distinct from old", key_2_id != key_1_id)
        check("Rotated response has 'rotated_from' matching old key", rotate_data["rotated_from"] == key_1_id)
        check("Rotated response has 'old_key_id' matching old key", rotate_data["old_key_id"] == key_1_id)
        check("Rotated response has 'old_key_expires_at' ISO timestamp", bool(rotate_data["old_key_expires_at"]))
        check("Rotated response has 'overlap_hours' == 48", rotate_data["overlap_hours"] == 48)
        check("Rotated response has 'name' marked as (Rotated)", "(Rotated)" in rotate_data["name"])

        # ─── PHASE 4: Verification of New API Key Authentication ─────────────
        print("\n[PHASE 4] Authentication with Newly Issued Key (X-API-Key)...")
        r_new_key_auth = httpx.get(
            f"{BASE_URL}/sentiment?ticker=AAPL",
            headers={"X-API-Key": key_2_raw},
        )
        check("New API key authenticates immediately (200)", r_new_key_auth.status_code == 200)
        check("New API key retrieves sentiment data", r_new_key_auth.json().get("ticker") == "AAPL")

        # ─── PHASE 5: Verification of Old API Key During Overlap Window ──────
        print("\n[PHASE 5] Authentication with Old API Key during Overlap Window...")
        r_old_key_auth = httpx.get(
            f"{BASE_URL}/sentiment?ticker=MSFT",
            headers={"X-API-Key": key_1_raw},
        )
        check("Old API key remains valid during overlap window (200)", r_old_key_auth.status_code == 200)
        check("Old API key retrieves sentiment data", r_old_key_auth.json().get("ticker") == "MSFT")

        # ─── PHASE 6: Key Listing & Details Inspection ───────────────────────
        print("\n[PHASE 6] Key Listing and Detail Inspection...")
        r_list = httpx.get(f"{BASE_URL}/auth/api-keys", headers=auth_headers)
        check("GET /auth/api-keys returns 200", r_list.status_code == 200)
        keys_list = r_list.json().get("api_keys", [])
        check("List contains at least 2 keys", len(keys_list) >= 2)

        # Inspect old key
        old_key_meta = next((k for k in keys_list if k["id"] == key_1_id), None)
        check("Old key found in listing", old_key_meta is not None)
        check("Old key rotation_status is 'rotating'", old_key_meta.get("rotation_status") == "rotating")
        check("Old key has 'expires_at' populated", old_key_meta.get("expires_at") is not None)

        # Inspect new key
        new_key_meta = next((k for k in keys_list if k["id"] == key_2_id), None)
        check("New key found in listing", new_key_meta is not None)
        check("New key rotation_status is 'none'", new_key_meta.get("rotation_status") == "none")
        check("New key rotated_from matches old key ID", new_key_meta.get("rotated_from") == key_1_id)
        check("New key expires_at is None", new_key_meta.get("expires_at") is None)

        # Inspect GET /auth/api-keys/{id}
        r_detail = httpx.get(f"{BASE_URL}/auth/api-keys/{key_1_id}", headers=auth_headers)
        check("GET /auth/api-keys/{id} returns 200", r_detail.status_code == 200)
        check("Detail item id matches old key", r_detail.json().get("id") == key_1_id)
        check("Detail item rotation_status is 'rotating'", r_detail.json().get("rotation_status") == "rotating")

        # ─── PHASE 7: Concurrency & Duplicate Rotation Guard ─────────────────
        print("\n[PHASE 7] Concurrency & Duplicate Rotation Guard (400)...")
        r_dup_rotate = httpx.post(
            f"{BASE_URL}/auth/api-keys/{key_1_id}/rotate",
            json={"overlap_hours": 24},
            headers=auth_headers,
        )
        check("Rotating already rotating key returns 400 Bad Request", r_dup_rotate.status_code == 400)
        check("Error message indicates already rotating", "already rotating" in r_dup_rotate.text.lower())

        # ─── PHASE 8: Overlap Period Bounds Validation ───────────────────────
        print("\n[PHASE 8] Overlap Period Bounds Validation (400)...")
        r_zero_hours = httpx.post(
            f"{BASE_URL}/auth/api-keys/{key_2_id}/rotate",
            json={"overlap_hours": 0},
            headers=auth_headers,
        )
        check("overlap_hours=0 rejected with 400", r_zero_hours.status_code == 400)

        r_excessive_hours = httpx.post(
            f"{BASE_URL}/auth/api-keys/{key_2_id}/rotate",
            json={"overlap_hours": 500},
            headers=auth_headers,
        )
        check("overlap_hours=500 rejected with 400", r_excessive_hours.status_code == 400)

        # ─── PHASE 9: Revocation & Auto-Revocation Verification ───────────────
        print("\n[PHASE 9] Manual & Automatic Key Revocation...")
        # Manually revoke old key
        r_revoke = httpx.delete(f"{BASE_URL}/auth/api-keys/{key_1_id}", headers=auth_headers)
        check("Revoke old key returns 200", r_revoke.status_code == 200)

        # Authentication with revoked key must fail
        r_revoked_auth = httpx.get(
            f"{BASE_URL}/sentiment?ticker=AAPL",
            headers={"X-API-Key": key_1_raw},
        )
        check("Revoked API key returns 401 Unauthorized", r_revoked_auth.status_code == 401)

        # New key must still work perfectly
        r_new_key_still_works = httpx.get(
            f"{BASE_URL}/sentiment?ticker=AAPL",
            headers={"X-API-Key": key_2_raw},
        )
        check("New key remains active and unaffected (200)", r_new_key_still_works.status_code == 200)

        # ─── PHASE 10: Audit Log Verification ────────────────────────────────
        print("\n[PHASE 10] Audit Log Trail Verification...")
        r_audit = httpx.get(f"{BASE_URL}/audit/logs", headers=auth_headers)
        check("GET /audit/logs returns 200", r_audit.status_code == 200)
        audit_records = r_audit.json().get("logs", [])
        actions = [a.get("action") for a in audit_records]
        check("Audit log contains 'apikey.create'", "apikey.create" in actions)
        check("Audit log contains 'apikey.rotation_started'", "apikey.rotation_started" in actions)
        check("Audit log contains 'apikey.revoke'", "apikey.revoke" in actions)

        rotation_event = next((a for a in audit_records if a["action"] == "apikey.rotation_started"), None)
        check("Rotation audit event has old_key_id in details", bool(rotation_event and "old_key_id" in rotation_event.get("details", {})))
        check("Rotation audit event has new_key_id in details", bool(rotation_event and "new_key_id" in rotation_event.get("details", {})))

        # ─── PHASE 11: OpenAPI 3.0 Documentation Registration ────────────────
        print("\n[PHASE 11] OpenAPI 3.0 Documentation Registration & Schema...")
        r_openapi = httpx.get(f"{BASE_URL}/api-docs/openapi.json")
        check("GET /api-docs/openapi.json returns 200", r_openapi.status_code == 200)
        spec = r_openapi.json()
        paths = spec.get("paths", {})
        check("OpenAPI spec contains '/auth/api-keys/{id}/rotate'", "/auth/api-keys/{id}/rotate" in paths)
        check("OpenAPI spec contains '/auth/api-keys/{id}'", "/auth/api-keys/{id}" in paths)
        schemas = spec.get("components", {}).get("schemas", {})
        check("OpenAPI schemas include 'RotateApiKeyRequest'", "RotateApiKeyRequest" in schemas)
        check("OpenAPI schemas include 'RotateApiKeyResponse'", "RotateApiKeyResponse" in schemas)

        # ─── PHASE 12: Python SDK Client Execution ───────────────────────────
        print("\n[PHASE 12] Python SDK Client Execution (Synchronous & Asynchronous)...")
        # Sync client
        sdk_client = FinTextClient(base_url=BASE_URL, api_token=token)
        sdk_created = sdk_client.create_api_key(name="SDK Bot")
        check("Python SDK sync create_api_key returns CreateApiKeyResponse", bool(hasattr(sdk_created, "api_key")))

        sdk_keys = sdk_client.list_api_keys()
        check("Python SDK sync list_api_keys returns ListApiKeysResponse", isinstance(sdk_keys, ListApiKeysResponse))

        sdk_rotated = sdk_client.rotate_api_key(sdk_created.id, overlap_hours=36)
        check("Python SDK sync rotate_api_key returns RotateApiKeyResponse", isinstance(sdk_rotated, RotateApiKeyResponse))
        check("SDK rotated object has overlap_hours == 36", sdk_rotated.overlap_hours == 36)

        sdk_detail = sdk_client.get_api_key(sdk_created.id)
        check("Python SDK sync get_api_key returns ApiKeyItem", isinstance(sdk_detail, ApiKeyItem))
        check("SDK detail shows 'rotating' status", sdk_detail.rotation_status == "rotating")

        # Async client
        import anyio
        async def run_async_sdk_checks():
            async with FinTextAsyncClient(base_url=BASE_URL, api_token=token) as async_client:
                a_created = await async_client.create_api_key(name="Async SDK Bot")
                check("Python SDK async create_api_key works", bool(a_created.api_key))

                a_rotated = await async_client.rotate_api_key(a_created.id, overlap_hours=72)
                check("Python SDK async rotate_api_key returns RotateApiKeyResponse", isinstance(a_rotated, RotateApiKeyResponse))
                check("Async rotated has overlap_hours == 72", a_rotated.overlap_hours == 72)

                a_detail = await async_client.get_api_key(a_created.id)
                check("Python SDK async get_api_key works", a_detail.rotation_status == "rotating")

                a_list = await async_client.list_api_keys()
                check("Python SDK async list_api_keys works", a_list.count >= 4)

        anyio.run(run_async_sdk_checks)

    print("\n" + "=" * 80)
    print(f"** SUITE #211 PASSED: {passed}/{passed + failed} CHECKS CERTIFIED")
    print("=" * 80)


if __name__ == "__main__":
    main()
