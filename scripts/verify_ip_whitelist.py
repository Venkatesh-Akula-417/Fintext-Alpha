#!/usr/bin/env python3
"""
═══════════════════════════════════════════════════════════════════════════════
FinText-Alpha-Vectorizer — Test Suite #207: IP Whitelisting & CIDR Access Control
═══════════════════════════════════════════════════════════════════════════════
Validates:
  1. Unauthenticated Access Rejection on IP Whitelist Endpoints (401).
  2. Default Open Behavior (when whitelist is empty, any IP is permitted) (200).
  3. IPv4 Single IP and CIDR Subnet Addition (`POST /security/ip-whitelist`).
  4. IPv6 Subnet and Address Addition.
  5. Invalid IP Address and CIDR Format Rejection (400).
  6. IP Whitelist Retrieval & Listing (`GET /security/ip-whitelist`).
  7. IP Whitelist Enforcement Middleware:
     - Whitelisted subnet IP (203.0.113.50) -> Permitted (200).
     - Whitelisted static IP (198.51.100.42) -> Permitted (200).
     - Non-whitelisted IP (10.99.88.77) -> Forbidden (403).
     - Non-whitelisted IP via X-Real-IP -> Forbidden (403).
  8. Whitelist Entry Deletion (`DELETE /security/ip-whitelist/{id}`).
  9. Default Open Restoration (after all whitelist entries are removed).
  10. User Isolation (User A's whitelist does not restrict User B).
  11. OpenAPI 3.0 Schema and Endpoint Registration.
  12. Python SDK Client Execution.
═══════════════════════════════════════════════════════════════════════════════
"""

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

from fintext import FinTextClient
from fintext.models import IpWhitelistEntry, ListIpWhitelistResponse, DeleteIpWhitelistResponse

SERVER_PORT = 8100
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


def auth_headers(token: str, ip: Optional[str] = None) -> dict:
    headers = {"Authorization": f"Bearer {token}"}
    if ip:
        headers["X-Forwarded-For"] = ip
    return headers


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
    print("FINTEXT ALPHA VECTORIZER — TEST SUITE #207: IP WHITELISTING & CIDR ACCESS CONTROL")
    print("=" * 80)

    # ── Phase 1: Unauthenticated Access Rejection ──
    phase("1. Unauthenticated Access Rejection (401)")

    resp = httpx.get(f"{BASE_URL}/security/ip-whitelist")
    check("GET /security/ip-whitelist without token → 401", resp.status_code == 401, f"got {resp.status_code}")

    resp = httpx.post(f"{BASE_URL}/security/ip-whitelist", json={"ip_or_cidr": "203.0.113.0/24"})
    check("POST /security/ip-whitelist without token → 401", resp.status_code == 401, f"got {resp.status_code}")

    # ── Phase 2: Default Open Access ──
    phase("2. Default Open Access (Empty Whitelist)")

    user_a_token = get_token("enterprise_trader_a")

    resp = httpx.get(
        f"{BASE_URL}/sentiment?ticker=AAPL",
        headers=auth_headers(user_a_token, ip="198.51.100.5"),
    )
    check("Empty whitelist allows access from arbitrary IP (200)", resp.status_code == 200, f"got {resp.status_code}")

    # ── Phase 3: IPv4 Single IP and CIDR Addition ──
    phase("3. Adding IPv4 IP and CIDR Whitelist Entries (POST /security/ip-whitelist)")

    resp = httpx.post(
        f"{BASE_URL}/security/ip-whitelist",
        json={"ip_or_cidr": "203.0.113.0/24", "description": "Primary Office VPN"},
        headers=auth_headers(user_a_token),
    )
    check("Add CIDR 203.0.113.0/24 returns 201", resp.status_code in (200, 201), f"got {resp.status_code}")
    cidr_entry = resp.json()
    cidr_entry_id = cidr_entry.get("id", "")
    check("CIDR entry has id", len(cidr_entry_id) > 0)
    check("CIDR entry ip_or_cidr normalized", cidr_entry.get("ip_or_cidr") == "203.0.113.0/24")

    # Add single IP (now calling from inside whitelisted CIDR 203.0.113.1)
    resp2 = httpx.post(
        f"{BASE_URL}/security/ip-whitelist",
        json={"ip_or_cidr": "198.51.100.42", "description": "Static Backup Gateway"},
        headers=auth_headers(user_a_token, ip="203.0.113.1"),
    )
    check("Add single IP 198.51.100.42 returns 201", resp2.status_code in (200, 201), f"got {resp2.status_code}")
    ip_entry = resp2.json()
    ip_entry_id = ip_entry.get("id", "")
    check("Single IP converted to /32 format", ip_entry.get("ip_or_cidr") == "198.51.100.42/32")

    # ── Phase 4: IPv6 Subnet Addition ──
    phase("4. Adding IPv6 Whitelist Entry")

    resp3 = httpx.post(
        f"{BASE_URL}/security/ip-whitelist",
        json={"ip_or_cidr": "2001:db8::/32", "description": "Global Datacenter IPv6"},
        headers=auth_headers(user_a_token, ip="203.0.113.1"),
    )
    check("Add IPv6 CIDR returns 201", resp3.status_code in (200, 201), f"got {resp3.status_code}")
    ipv6_entry = resp3.json()
    ipv6_entry_id = ipv6_entry.get("id", "")
    check("IPv6 CIDR preserved", ipv6_entry.get("ip_or_cidr") == "2001:db8::/32")

    # ── Phase 5: Invalid CIDR/IP Rejection ──
    phase("5. Invalid Format Rejection (400)")

    for bad_ip in ["invalid.ip.string", "256.256.256.256", "192.168.1.0/33", ""]:
        resp = httpx.post(
            f"{BASE_URL}/security/ip-whitelist",
            json={"ip_or_cidr": bad_ip},
            headers=auth_headers(user_a_token, ip="203.0.113.1"),
        )
        check(f"Reject invalid input '{bad_ip}' with 400", resp.status_code == 400, f"got {resp.status_code}")

    # ── Phase 6: List Whitelist Entries ──
    phase("6. Listing Whitelist Entries (GET /security/ip-whitelist)")

    resp = httpx.get(
        f"{BASE_URL}/security/ip-whitelist",
        headers=auth_headers(user_a_token, ip="203.0.113.1"),
    )
    check("List whitelist returns 200", resp.status_code == 200, f"got {resp.status_code}")
    whitelist_data = resp.json()
    check("Entries count is 3", whitelist_data.get("count") == 3, f"got {whitelist_data.get('count')}")
    check("Entries array length is 3", len(whitelist_data.get("entries", [])) == 3)

    # ── Phase 7: IP Whitelist Enforcement Middleware ──
    phase("7. IP Whitelist Enforcement Middleware (200 vs 403)")

    # 7a. Request from within whitelisted CIDR 203.0.113.0/24 (e.g. 203.0.113.50)
    resp = httpx.get(
        f"{BASE_URL}/sentiment?ticker=AAPL",
        headers=auth_headers(user_a_token, ip="203.0.113.50"),
    )
    check("Request from whitelisted CIDR (203.0.113.50) → Allowed (200)", resp.status_code == 200, f"got {resp.status_code}")

    # 7b. Request from exact static IP (198.51.100.42)
    resp = httpx.get(
        f"{BASE_URL}/sentiment?ticker=AAPL",
        headers=auth_headers(user_a_token, ip="198.51.100.42"),
    )
    check("Request from exact static IP (198.51.100.42) → Allowed (200)", resp.status_code == 200, f"got {resp.status_code}")

    # 7c. Request from unauthorized IP (10.99.88.77)
    resp = httpx.get(
        f"{BASE_URL}/sentiment?ticker=AAPL",
        headers=auth_headers(user_a_token, ip="10.99.88.77"),
    )
    check("Request from unauthorized IP (10.99.88.77) → Forbidden (403)", resp.status_code == 403, f"got {resp.status_code}")
    err_body = resp.json()
    check("Error message indicates IP not allowed", "IP address not allowed" in err_body.get("message", ""))

    # 7d. Request via X-Real-IP header with unauthorized IP
    headers_real_ip = {"Authorization": f"Bearer {user_a_token}", "X-Real-IP": "172.16.0.99"}
    resp = httpx.get(f"{BASE_URL}/sentiment?ticker=AAPL", headers=headers_real_ip)
    check("Request with unauthorized X-Real-IP → Forbidden (403)", resp.status_code == 403, f"got {resp.status_code}")

    # ── Phase 8: Delete Whitelist Entries ──
    phase("8. Delete Whitelist Entries (DELETE /security/ip-whitelist/{id})")

    # Delete IPv6 entry first while IPv4 198.51.100.42 is still active
    resp_v6 = httpx.delete(
        f"{BASE_URL}/security/ip-whitelist/{ipv6_entry_id}",
        headers=auth_headers(user_a_token, ip="198.51.100.42"),
    )
    check("Delete IPv6 entry returns 200", resp_v6.status_code == 200, f"got {resp_v6.status_code}")

    # Delete CIDR subnet entry
    resp = httpx.delete(
        f"{BASE_URL}/security/ip-whitelist/{cidr_entry_id}",
        headers=auth_headers(user_a_token, ip="198.51.100.42"),
    )
    check("Delete CIDR entry returns 200", resp.status_code == 200, f"got {resp.status_code}")

    # After deleting 203.0.113.0/24, requests from 203.0.113.50 should now be blocked
    resp_blocked = httpx.get(
        f"{BASE_URL}/sentiment?ticker=AAPL",
        headers=auth_headers(user_a_token, ip="203.0.113.50"),
    )
    check("Deleted subnet IP (203.0.113.50) is now Forbidden (403)", resp_blocked.status_code == 403, f"got {resp_blocked.status_code}")

    # Delete final remaining static IP entry (calling from 198.51.100.42)
    resp_ip = httpx.delete(
        f"{BASE_URL}/security/ip-whitelist/{ip_entry_id}",
        headers=auth_headers(user_a_token, ip="198.51.100.42"),
    )
    check("Delete static IP entry returns 200", resp_ip.status_code == 200, f"got {resp_ip.status_code}")

    # Whitelist is now completely empty -> delete non-existent entry returns 404 from any IP
    resp_404 = httpx.delete(
        f"{BASE_URL}/security/ip-whitelist/00000000-0000-0000-0000-000000000000",
        headers=auth_headers(user_a_token),
    )
    check("Delete non-existent ID returns 404", resp_404.status_code == 404, f"got {resp_404.status_code}")

    # ── Phase 9: Default Open Restoration ──
    phase("9. Default Open Restoration (Empty Whitelist)")

    resp = httpx.get(
        f"{BASE_URL}/sentiment?ticker=AAPL",
        headers=auth_headers(user_a_token, ip="10.99.88.77"),
    )
    check("All entries deleted → access restored from any IP (200)", resp.status_code == 200, f"got {resp.status_code}")

    # ── Phase 10: User Isolation ──
    phase("10. User Isolation")

    user_b_token = get_token("enterprise_trader_b")

    # Set user A whitelist to 192.168.1.0/24
    httpx.post(
        f"{BASE_URL}/security/ip-whitelist",
        json={"ip_or_cidr": "192.168.1.0/24"},
        headers=auth_headers(user_a_token),
    )

    # User A blocked from 10.0.0.1
    resp_a = httpx.get(
        f"{BASE_URL}/sentiment?ticker=AAPL",
        headers=auth_headers(user_a_token, ip="10.0.0.1"),
    )
    check("User A blocked from unlisted IP (403)", resp_a.status_code == 403)

    # User B (no whitelist) allowed from 10.0.0.1
    resp_b = httpx.get(
        f"{BASE_URL}/sentiment?ticker=AAPL",
        headers=auth_headers(user_b_token, ip="10.0.0.1"),
    )
    check("User B (no whitelist) permitted from 10.0.0.1 (200)", resp_b.status_code == 200)

    # ── Phase 11: OpenAPI 3.0 Documentation Registration ──
    phase("11. OpenAPI 3.0 Documentation Registration")

    spec_resp = httpx.get(f"{BASE_URL}/api-docs/openapi.json")
    check("OpenAPI spec returns 200", spec_resp.status_code == 200)
    spec = spec_resp.json()

    paths = spec.get("paths", {})
    check("Path '/security/ip-whitelist' registered", "/security/ip-whitelist" in paths)
    check("Path '/security/ip-whitelist/{id}' registered", "/security/ip-whitelist/{id}" in paths)

    schemas = spec.get("components", {}).get("schemas", {})
    for s in ["IpWhitelistEntry", "AddIpWhitelistRequest", "ListIpWhitelistResponse", "DeleteIpWhitelistResponse"]:
        check(f"Schema '{s}' registered", s in schemas)

    tags = [t.get("name") for t in spec.get("tags", [])]
    check("Tag 'Security & IP Whitelisting' present", "Security & IP Whitelisting" in tags)

    # ── Phase 12: Python SDK Client Execution ──
    phase("12. Python SDK Client Execution")

    user_sdk_token = get_token("sdk_trader_user")
    sdk_client = FinTextClient(base_url=BASE_URL, api_token=user_sdk_token)
    sdk_list = sdk_client.get_ip_whitelist()
    check("SDK get_ip_whitelist() returns ListIpWhitelistResponse", isinstance(sdk_list, ListIpWhitelistResponse))
    check("SDK initial count is 0", sdk_list.count == 0)

    # Whitelist loopback range so local SDK client remains authorized for management calls
    sdk_add = sdk_client.add_ip_whitelist("127.0.0.1/32", description="SDK Loopback Gateway")
    check("SDK add_ip_whitelist() returns IpWhitelistEntry", isinstance(sdk_add, IpWhitelistEntry))
    check("SDK added entry matches CIDR", sdk_add.ip_or_cidr == "127.0.0.1/32")

    sdk_del = sdk_client.delete_ip_whitelist(sdk_add.id)
    check("SDK delete_ip_whitelist() returns DeleteIpWhitelistResponse", isinstance(sdk_del, DeleteIpWhitelistResponse))
    check("SDK delete response status is 'success'", sdk_del.status == "success")

    # ── Summary ──
    print("\n" + "=" * 80)
    total = passed + failed
    print(f"SUITE #207 RESULTS: {passed}/{total} passed, {failed} failed")
    if failed == 0:
        print("🎉 ALL PHASES PASSED — IP Whitelisting & CIDR Access Control CERTIFIED.")
    else:
        print(f"⚠️  {failed} CHECKS FAILED — review output above.")
    print("=" * 80)

    return failed == 0


if __name__ == "__main__":
    with ServerContext():
        success = run_tests()
    sys.exit(0 if success else 1)
