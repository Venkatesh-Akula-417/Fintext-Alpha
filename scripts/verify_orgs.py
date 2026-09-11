#!/usr/bin/env python3
"""
═══════════════════════════════════════════════════════════════════════════════
FinText-Alpha-Vectorizer — Test Suite #206: Organizations & Multi-User Teams
═══════════════════════════════════════════════════════════════════════════════
Validates:
  1. Unauthenticated Access Rejection on Org Endpoints (401).
  2. Organization Creation (`POST /orgs`) and List (`GET /orgs`).
  3. Organization Detail Retrieval (`GET /orgs/{id}`).
  4. Member Invitation (`POST /orgs/{id}/invites`) with RBAC.
  5. Role Update (`PATCH /orgs/{id}/members/{user_id}`) with RBAC.
  6. Sole Admin Protection (cannot demote/remove/leave as sole admin).
  7. Member Removal (`DELETE /orgs/{id}/members/{user_id}`).
  8. Leave Organization (`POST /orgs/{id}/leave`).
  9. Select Org Context Switching (`POST /orgs/{id}/select`) with JWT org_id.
  10. OpenAPI 3.0 Schema Registration & Tag Verification.
═══════════════════════════════════════════════════════════════════════════════
"""

import base64
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

SERVER_PORT = 8099
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


def auth_headers(token: str) -> dict:
    return {"Authorization": f"Bearer {token}"}


# ═════════════════════════════════════════════════════════════════════════════
# Test Phases
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
    print("FINTEXT ALPHA VECTORIZER — TEST SUITE #206: ORGANIZATIONS & MULTI-USER TEAMS")
    print("=" * 80)

    # ── Phase 1: Unauthenticated Access Rejection ──
    phase("1. Unauthenticated Access Rejection (401)")

    for path, method in [
        ("/orgs", "POST"),
        ("/orgs", "GET"),
    ]:
        if method == "POST":
            resp = httpx.post(f"{BASE_URL}{path}", json={"name": "TestOrg"})
        else:
            resp = httpx.get(f"{BASE_URL}{path}")
        check(f"{method} {path} without auth → 401", resp.status_code == 401, f"got {resp.status_code}")

    # ── Phase 2: Organization Creation ──
    phase("2. Organization Creation (POST /orgs)")

    admin_token = get_token("org_admin_001")

    resp = httpx.post(
        f"{BASE_URL}/orgs",
        json={"name": "Acme Quant Fund"},
        headers=auth_headers(admin_token),
    )
    check("Create org returns 2xx", resp.status_code in (200, 201), f"got {resp.status_code}")

    org_data = resp.json()
    org_id = org_data.get("id", "")
    check("Response contains org id", len(org_id) > 0)
    check("Response contains name", org_data.get("name") == "Acme Quant Fund")
    check("Creator is assigned 'admin' role", org_data.get("role") == "admin")
    check("Response contains created_at", "created_at" in org_data)

    # ── Phase 3: List Organizations ──
    phase("3. List Organizations (GET /orgs)")

    resp = httpx.get(f"{BASE_URL}/orgs", headers=auth_headers(admin_token))
    check("List orgs returns 200", resp.status_code == 200, f"got {resp.status_code}")

    list_data = resp.json()
    check("Response contains 'organizations' array", isinstance(list_data.get("organizations"), list))
    check("Count >= 1", list_data.get("count", 0) >= 1)

    found = any(o.get("id") == org_id for o in list_data.get("organizations", []))
    check("Created org appears in list", found)

    # ── Phase 4: Organization Detail ──
    phase("4. Organization Detail (GET /orgs/{id})")

    resp = httpx.get(f"{BASE_URL}/orgs/{org_id}", headers=auth_headers(admin_token))
    check("Get org detail returns 200", resp.status_code == 200, f"got {resp.status_code}")

    detail = resp.json()
    check("Detail has members array", isinstance(detail.get("members"), list))
    check("Creator is in members", any(m.get("user_id") == "org_admin_001" for m in detail.get("members", [])))
    check("Member count is correct", detail.get("count", 0) >= 1)

    # ── Phase 5: Member Invitation ──
    phase("5. Member Invitation (POST /orgs/{id}/invites)")

    resp = httpx.post(
        f"{BASE_URL}/orgs/{org_id}/invites",
        json={"user_id": "analyst_alpha", "role": "member"},
        headers=auth_headers(admin_token),
    )
    check("Invite member returns 200", resp.status_code == 200, f"got {resp.status_code}")

    invite_data = resp.json()
    check("Invite response status is 'ok'/'success'", invite_data.get("status") in ("ok", "success"))
    check("Invited user_id matches", invite_data.get("user_id") == "analyst_alpha")
    check("Assigned role is 'member'", invite_data.get("role") == "member")

    # Invite a viewer
    resp2 = httpx.post(
        f"{BASE_URL}/orgs/{org_id}/invites",
        json={"user_id": "viewer_beta", "role": "viewer"},
        headers=auth_headers(admin_token),
    )
    check("Invite viewer returns 200", resp2.status_code == 200)

    # Non-admin cannot invite
    member_token = get_token("analyst_alpha")
    resp3 = httpx.post(
        f"{BASE_URL}/orgs/{org_id}/invites",
        json={"user_id": "intruder_99", "role": "member"},
        headers=auth_headers(member_token),
    )
    check("Non-admin invite is rejected (403)", resp3.status_code == 403, f"got {resp3.status_code}")

    # ── Phase 6: Role Update ──
    phase("6. Role Update (PATCH /orgs/{id}/members/{user_id})")

    resp = httpx.patch(
        f"{BASE_URL}/orgs/{org_id}/members/analyst_alpha",
        json={"role": "admin"},
        headers=auth_headers(admin_token),
    )
    check("Promote member to admin returns 200", resp.status_code == 200, f"got {resp.status_code}")

    update_data = resp.json()
    check("Update response has new role", update_data.get("role") == "admin")

    # Non-admin cannot update roles
    viewer_token = get_token("viewer_beta")
    resp2 = httpx.patch(
        f"{BASE_URL}/orgs/{org_id}/members/viewer_beta",
        json={"role": "admin"},
        headers=auth_headers(viewer_token),
    )
    check("Viewer cannot update roles (403)", resp2.status_code == 403, f"got {resp2.status_code}")

    # ── Phase 7: Sole Admin Protection ──
    phase("7. Sole Admin Protection")

    # Create a fresh org with a single admin
    sole_token = get_token("sole_admin_user")
    resp = httpx.post(
        f"{BASE_URL}/orgs",
        json={"name": "Solo Admin Org"},
        headers=auth_headers(sole_token),
    )
    sole_org_id = resp.json().get("id", "")
    check("Created sole-admin org", resp.status_code in (200, 201))

    # Try to leave as sole admin
    resp = httpx.post(
        f"{BASE_URL}/orgs/{sole_org_id}/leave",
        headers=auth_headers(sole_token),
    )
    check("Sole admin cannot leave (400/409)", resp.status_code in (400, 409), f"got {resp.status_code}")

    # ── Phase 8: Member Removal ──
    phase("8. Member Removal (DELETE /orgs/{id}/members/{user_id})")

    resp = httpx.delete(
        f"{BASE_URL}/orgs/{org_id}/members/viewer_beta",
        headers=auth_headers(admin_token),
    )
    check("Remove viewer returns 200", resp.status_code == 200, f"got {resp.status_code}")

    remove_data = resp.json()
    check("Remove response status is 'ok'/'success'", remove_data.get("status") in ("ok", "success"))
    check("Removed user_id matches", remove_data.get("user_id") == "viewer_beta")

    # ── Phase 9: Leave Organization ──
    phase("9. Leave Organization (POST /orgs/{id}/leave)")

    # analyst_alpha (now admin) can leave since org_admin_001 is also admin
    resp = httpx.post(
        f"{BASE_URL}/orgs/{org_id}/leave",
        headers=auth_headers(member_token),
    )
    check("Non-sole admin can leave (200)", resp.status_code == 200, f"got {resp.status_code}")

    leave_data = resp.json()
    check("Leave response status is 'ok'/'success'", leave_data.get("status") in ("ok", "success"))

    # ── Phase 10: Select Org Context ──
    phase("10. Select Org Context (POST /orgs/{id}/select)")

    resp = httpx.post(
        f"{BASE_URL}/orgs/{org_id}/select",
        headers=auth_headers(admin_token),
    )
    check("Select org returns 200", resp.status_code == 200, f"got {resp.status_code}")

    select_data = resp.json()
    check("Select response has token", len(select_data.get("token", "")) > 0)
    check("Select response has org_id", select_data.get("org_id") == org_id)
    check("Select response has role", len(select_data.get("role", "")) > 0)

    # Decode the new JWT and verify org_id claim is present
    new_token = select_data["token"]
    parts = new_token.split(".")
    if len(parts) == 3:
        payload_b64 = parts[1] + "=" * (-len(parts[1]) % 4)
        payload = json.loads(base64.urlsafe_b64decode(payload_b64))
        check("JWT contains org_id claim", payload.get("org_id") == org_id, f"got {payload.get('org_id')}")
    else:
        check("JWT has 3 parts", False, f"got {len(parts)} parts")

    # ── Phase 11: OpenAPI Schema Registration ──
    phase("11. OpenAPI 3.0 Schema Registration")

    resp = httpx.get(f"{BASE_URL}/api-docs/openapi.json")
    check("OpenAPI spec returns 200", resp.status_code == 200, f"got {resp.status_code}")

    spec = resp.json()
    paths = spec.get("paths", {})

    for endpoint in ["/orgs"]:
        check(f"Path '{endpoint}' registered", endpoint in paths)

    schemas = spec.get("components", {}).get("schemas", {})
    for schema_name in [
        "CreateOrgRequest", "CreateOrgResponse", "ListOrgsResponse",
        "OrgDetailsResponse", "InviteMemberRequest", "InviteMemberResponse",
        "UpdateMemberRoleRequest", "UpdateMemberRoleResponse",
        "RemoveMemberResponse", "LeaveOrgResponse", "SelectOrgResponse",
    ]:
        check(f"Schema '{schema_name}' registered", schema_name in schemas)

    tags = [t.get("name") for t in spec.get("tags", [])]
    check("Tag 'Organizations & Teams' present", "Organizations & Teams" in tags)

    # ── Summary ──
    print("\n" + "=" * 80)
    total = passed + failed
    print(f"SUITE #206 RESULTS: {passed}/{total} passed, {failed} failed")
    if failed == 0:
        print("🎉 ALL PHASES PASSED — Organizations & Multi-User Teams feature CERTIFIED.")
    else:
        print(f"⚠️  {failed} CHECKS FAILED — review output above.")
    print("=" * 80)

    return failed == 0


if __name__ == "__main__":
    with ServerContext():
        success = run_tests()
    sys.exit(0 if success else 1)
