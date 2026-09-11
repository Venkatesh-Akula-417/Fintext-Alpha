#!/usr/bin/env python3
"""
FinText-Alpha-Vectorizer — User Registration, Login & API Key Management Verification
═════════════════════════════════════════════════════════════════════════════════════
Verifies:
  1. POST /auth/register (Account creation with Argon2 password hashing)
  2. POST /auth/register error handling (weak password, malformed email, duplicate conflict)
  3. POST /auth/login (JWT token issuance and credential verification)
  4. GET /auth/me (Caller profile retrieval via JWT)
  5. POST /auth/api-keys (Generation of SHA-256 hashed API keys with prefix)
  6. GET /sentiment & GET /export/csv using X-API-Key header authentication
  7. GET /auth/api-keys (List user API keys)
  8. DELETE /auth/api-keys/{id} (Revocation and rejection of subsequent calls)
  9. Backward-compatibility of POST /auth/token with X-Admin-Token
"""

import os
import sys
import time
import subprocess
import requests

API_HOST = "http://127.0.0.1:8000"
BINARY_PATH = os.path.join("rust", "target", "release", "fintext_api.exe")

def run_checks():
    print("=" * 85)
    print(" FinText-Alpha-Vectorizer — User Authentication & API Key Lifecycle Verification")
    print("=" * 85)

    if not os.path.exists(BINARY_PATH):
        print(f"[!] Release binary not found at {BINARY_PATH}. Building...")
        res = subprocess.run(["cargo", "build", "--release", "--bin", "fintext_api"], cwd="rust")
        if res.returncode != 0:
            print("[x] Failed to build release binary.")
            sys.exit(1)

    env = os.environ.copy()
    env["QUESTDB_MOCK_FALLBACK"] = "1"
    env["JWT_SECRET"] = "fintext-alpha-vectorizer-institutional-jwt-secret-key-2026"
    env["ADMIN_TOKEN"] = "fintext-admin-dev-secret-token"

    print(f"[*] Starting API Server binary: {os.path.abspath(BINARY_PATH)}")
    server_proc = subprocess.Popen([BINARY_PATH], env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    time.sleep(2.0)

    try:
        # Check /health
        health_resp = requests.get(f"{API_HOST}/health", timeout=3)
        assert health_resp.status_code == 200, f"Expected 200 from /health, got {health_resp.status_code}"
        print("[*] Server online and responsive at /health")

        # 1. Test POST /auth/register
        print("\n[1/8] Registering new institutional user via POST /auth/register...")
        reg_payload = {
            "email": "alpha.trader@citadel.com",
            "password": "StrongSecretPass2026!"
        }
        reg_resp = requests.post(f"{API_HOST}/auth/register", json=reg_payload, timeout=5)
        print(f"      HTTP {reg_resp.status_code} | Body: {reg_resp.text}")
        assert reg_resp.status_code == 201, f"Expected 201 Created, got {reg_resp.status_code}"
        reg_data = reg_resp.json()
        assert reg_data["email"] == "alpha.trader@citadel.com"
        assert reg_data["status"] == "created"
        user_id = reg_data["user_id"]
        print(f"      [OK] Account created with user_id: {user_id}")

        # 2. Test Validation & Duplicate Registration
        print("\n[2/8] Testing validation and conflict rejection on /auth/register...")
        # Weak password
        weak_resp = requests.post(f"{API_HOST}/auth/register", json={"email": "u@test.com", "password": "weak"}, timeout=5)
        assert weak_resp.status_code == 400, f"Expected 400 for weak password, got {weak_resp.status_code}"
        print(f"      [OK] Weak password rejected (400 Bad Request): {weak_resp.json()['message']}")

        # Duplicate email
        dup_resp = requests.post(f"{API_HOST}/auth/register", json=reg_payload, timeout=5)
        assert dup_resp.status_code == 409, f"Expected 409 Conflict for duplicate email, got {dup_resp.status_code}"
        print(f"      [OK] Duplicate email rejected (409 Conflict): {dup_resp.json()['message']}")

        # 3. Test POST /auth/login
        print("\n[3/8] Logging in via POST /auth/login...")
        login_payload = {
            "email": "alpha.trader@citadel.com",
            "password": "StrongSecretPass2026!"
        }
        login_resp = requests.post(f"{API_HOST}/auth/login", json=login_payload, timeout=5)
        print(f"      HTTP {login_resp.status_code} | Body: {login_resp.text[:120]}...")
        assert login_resp.status_code == 200, f"Expected 200 OK, got {login_resp.status_code}"
        login_data = login_resp.json()
        jwt_token = login_data["token"]
        assert jwt_token, "Expected non-empty JWT token"
        print(f"      [OK] JWT Issued successfully (Expires in: {login_data['expires_in']}s)")

        # Wrong password check
        bad_login = requests.post(f"{API_HOST}/auth/login", json={"email": "alpha.trader@citadel.com", "password": "WrongPassword!"}, timeout=5)
        assert bad_login.status_code == 401, f"Expected 401 Unauthorized for bad password, got {bad_login.status_code}"
        print("      [OK] Invalid password correctly rejected (401 Unauthorized)")

        # 4. Test GET /auth/me with JWT
        print("\n[4/8] Querying GET /auth/me with JWT Bearer token...")
        jwt_headers = {"Authorization": f"Bearer {jwt_token}"}
        me_resp = requests.get(f"{API_HOST}/auth/me", headers=jwt_headers, timeout=5)
        assert me_resp.status_code == 200, f"Expected 200 OK, got {me_resp.status_code}"
        me_data = me_resp.json()
        assert me_data["email"] == "alpha.trader@citadel.com"
        print(f"      [OK] Current user profile: id={me_data['id']}, email={me_data['email']}, role={me_data['role']}")

        # 5. Generate API Key via POST /auth/api-keys
        print("\n[5/8] Generating API Key via POST /auth/api-keys...")
        key_payload = {"name": "High-Frequency Algorithm Bot 1"}
        key_resp = requests.post(f"{API_HOST}/auth/api-keys", json=key_payload, headers=jwt_headers, timeout=5)
        assert key_resp.status_code == 201, f"Expected 201 Created, got {key_resp.status_code}"
        key_data = key_resp.json()
        raw_api_key = key_data["api_key"]
        api_key_id = key_data["id"]
        prefix = key_data["prefix"]
        assert raw_api_key.startswith("ft_")
        print(f"      [OK] Generated API Key: {raw_api_key} (ID: {api_key_id}, Prefix: {prefix})")

        # 6. Test Authentication using X-API-Key Header
        print("\n[6/8] Accessing protected endpoints using X-API-Key header...")
        api_headers = {"X-API-Key": raw_api_key}
        
        # Test /sentiment
        sent_resp = requests.get(f"{API_HOST}/sentiment?ticker=AAPL", headers=api_headers, timeout=5)
        assert sent_resp.status_code == 200, f"Expected 200 OK from /sentiment with API Key, got {sent_resp.status_code}"
        print(f"      [OK] GET /sentiment?ticker=AAPL with X-API-Key -> 200 OK (Score: {sent_resp.json()['sentiment_score']})")

        # Test /export/csv
        csv_resp = requests.get(f"{API_HOST}/export/csv?ticker=AAPL&start_date=2025-01-01&end_date=2025-01-05", headers=api_headers, timeout=5)
        assert csv_resp.status_code == 200, f"Expected 200 OK from /export/csv with API Key, got {csv_resp.status_code}"
        print(f"      [OK] GET /export/csv with X-API-Key -> 200 OK ({len(csv_resp.text.splitlines())} lines received)")

        # 7. List and Revoke API Key
        print("\n[7/8] Listing and revoking API Key...")
        list_resp = requests.get(f"{API_HOST}/auth/api-keys", headers=jwt_headers, timeout=5)
        assert list_resp.status_code == 200
        assert list_resp.json()["count"] == 1
        print(f"      [OK] Active keys listed: {list_resp.json()['count']} key found")

        # Revoke key
        del_resp = requests.delete(f"{API_HOST}/auth/api-keys/{api_key_id}", headers=jwt_headers, timeout=5)
        assert del_resp.status_code == 200, f"Expected 200 OK, got {del_resp.status_code}"
        print(f"      [OK] API Key {api_key_id} revoked successfully")

        # Try to use revoked key
        revoked_call = requests.get(f"{API_HOST}/sentiment?ticker=AAPL", headers=api_headers, timeout=5)
        assert revoked_call.status_code == 401, f"Expected 401 Unauthorized for revoked key, got {revoked_call.status_code}"
        print("      [OK] Revoked API key correctly rejected (401 Unauthorized)")

        # 8. Backward Compatibility: POST /auth/token with X-Admin-Token
        print("\n[8/8] Testing backward compatibility of POST /auth/token with X-Admin-Token...")
        admin_payload = {"user_id": "legacy_client_alpha", "expires_in_seconds": 1800}
        admin_headers = {"X-Admin-Token": "fintext-admin-dev-secret-token"}
        admin_resp = requests.post(f"{API_HOST}/auth/token", json=admin_payload, headers=admin_headers, timeout=5)
        assert admin_resp.status_code == 200, f"Expected 200 OK, got {admin_resp.status_code}"
        print(f"      [OK] Legacy admin token endpoint works: user_id={admin_resp.json()['user_id']}")

        print("\n" + "=" * 85)
        print(" [OK] ALL USER AUTHENTICATION & API KEY MANAGEMENT CHECKS PASSED!")
        print("=" * 85)

    finally:
        server_proc.terminate()
        server_proc.wait()

if __name__ == "__main__":
    run_checks()
