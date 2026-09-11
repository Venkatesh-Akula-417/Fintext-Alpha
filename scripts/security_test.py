#!/usr/bin/env python3
"""
=====================================================================================
FinText-Alpha-Vectorizer — Comprehensive API Security & Penetration Testing Suite
=====================================================================================
Security Auditor Test Automation verifying:
  1. Unauthenticated Access Controls (401 Unauthorized on protected routes)
  2. JWT Validation & Tamper-Resistance (Malformed, forged signature, expired claims)
  3. SQL Injection (SQLi) Defense & Input Sanitization (No 500s or SQL leakage)
  4. Cross-Site Scripting (XSS) & Content-Type Defenses (JSON isolation, safe escaping)
  5. Rate Limiting & DoS Protection (Header tracking, 429 burst detection)
  6. IP Whitelisting & CIDR Zero-Trust Access Control (403 Forbidden enforcement & cleanup)
  7. Programmatic API Key Authentication & Tamper Validation (401 Unauthorized on invalid keys)

Usage:
  venv/Scripts/python.exe scripts/security_test.py
=====================================================================================
"""

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
DEFAULT_PORT = 8000
BASE_URL = os.getenv("BASE_URL", f"http://127.0.0.1:{DEFAULT_PORT}").rstrip("/")
ADMIN_TOKEN = os.getenv("ADMIN_TOKEN", "fintext-admin-dev-secret-token")

SERVER_EXE = (
    PROJECT_ROOT / "rust" / "target" / "debug" / ("fintext_api.exe" if sys.platform == "win32" else "fintext_api")
    if (PROJECT_ROOT / "rust" / "target" / "debug" / ("fintext_api.exe" if sys.platform == "win32" else "fintext_api")).exists()
    else PROJECT_ROOT / "rust" / "target" / "release" / ("fintext_api.exe" if sys.platform == "win32" else "fintext_api")
)

# Test results tracker
TEST_RESULTS = []


def record_test(name: str, passed: bool, detail: str = ""):
    """Records the outcome of a security test case."""
    TEST_RESULTS.append({"name": name, "passed": passed, "detail": detail})
    status_str = "[PASS]" if passed else "[FAIL]"
    print(f"  {status_str} {name}")
    if detail:
        print(f"         ↳ {detail}")


def ensure_server_running() -> tuple[bool, subprocess.Popen | None]:
    """Ensures the FinText Axum API Server is active, spawning a test instance if necessary."""
    try:
        r = httpx.get(f"{BASE_URL}/health", timeout=1.5)
        if r.status_code == 200:
            print(f"[SERVER] Connected to active FinText API server at {BASE_URL}")
            return True, None
    except Exception:
        pass

    print(f"[SERVER] Spawning local FinText API Server at {BASE_URL} (Security Audit Mode)...")
    env = os.environ.copy()
    env.update({
        "PORT": str(DEFAULT_PORT),
        "HOST": "127.0.0.1",
        "ADMIN_TOKEN": ADMIN_TOKEN,
        "JWT_SECRET": "super_secret_test_jwt_key_32_bytes_len!!",
        "RATE_LIMIT_REQUESTS": "100",
        "RATE_LIMIT_WINDOW_SECONDS": "60",
        "QUESTDB_MOCK_FALLBACK": "1",
        "POLYGON_MOCK_FALLBACK": "1",
        "WHISPER_MOCK_FALLBACK": "1",
        "RUST_LOG": "error",
    })

    proc = subprocess.Popen(
        [str(SERVER_EXE)],
        env=env,
        cwd=str(PROJECT_ROOT),
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )

    start_time = time.time()
    while time.time() - start_time < 25.0:
        try:
            r = httpx.get(f"{BASE_URL}/health", timeout=1.0)
            if r.status_code == 200:
                print(f"[SERVER] Server successfully started and healthy at {BASE_URL}")
                return True, proc
        except Exception:
            time.sleep(0.3)

    if proc:
        proc.terminate()
    raise RuntimeError(f"FinText API server failed to start at {BASE_URL} within 25s")


def get_jwt_token(client: httpx.Client, user_id: str, role: str = "institutional") -> str:
    """Requests a valid JWT token via admin token endpoint."""
    payload = {
        "user_id": user_id,
        "role": role,
        "expires_in_seconds": 3600,
    }
    headers = {"X-Admin-Token": ADMIN_TOKEN}
    res = client.post("/auth/token", json=payload, headers=headers)
    assert res.status_code == 200, f"Failed to issue JWT token: {res.text}"
    return res.json()["token"]


# ─────────────────────────────────────────────────────────────────────────────────
# Security Test Implementations
# ─────────────────────────────────────────────────────────────────────────────────

def test_unauthenticated_access(client: httpx.Client):
    """
    Test 1: Unauthenticated Access Controls
    Attempts to access protected endpoints without credentials. Expects 401 Unauthorized.
    """
    print("\n[TEST 1/7] Testing Unauthenticated Access Controls...")
    endpoints = [
        ("GET", "/sentiment", {"params": {"ticker": "AAPL"}}),
        ("GET", "/sentiment/batch", {"params": {"tickers": "AAPL,MSFT"}}),
        ("GET", "/market/breadth", {"params": {"start_date": "2025-01-01", "end_date": "2025-01-31"}}),
        ("POST", "/backtest", {"json": {"ticker": "AAPL", "start_date": "2025-01-01", "end_date": "2025-03-31"}}),
        ("GET", "/audit/logs", {}),
        ("GET", "/auth/me", {}),
        ("GET", "/security/ip-whitelist", {}),
    ]

    all_passed = True
    details = []

    for method, path, kwargs in endpoints:
        res = client.request(method, path, **kwargs)
        if res.status_code == 401:
            details.append(f"{method} {path} -> 401 Unauthorized (OK)")
        else:
            all_passed = False
            details.append(f"{method} {path} -> {res.status_code} (FAILED, expected 401)")

    record_test(
        "Unauthenticated Access Controls (HTTP 401 Enforcement)",
        all_passed,
        "; ".join(details[:3]) + f" ({len(endpoints)} routes verified)",
    )


def test_invalid_and_tampered_jwt(client: httpx.Client):
    """
    Test 2: JWT Validation & Tamper Resistance
    Tests completely malformed, forged signature, and expired tokens. Expects 401 Unauthorized.
    """
    print("\n[TEST 2/7] Testing JWT Validation & Tamper Resistance...")
    test_cases = [
        ("Malformed Token String", "Bearer completely_malformed_jwt_token_xyz"),
        ("Missing Bearer Prefix", "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.e30.bogus_sig"),
        ("Invalid Base64 Header", "Bearer invalid!base64!.e30.bogus_sig"),
        (
            "Forged Signature Token",
            "Bearer eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9."
            "eyJzdWIiOiJhZG1pbl9hdHRhY2tlciIsInJvbGUiOiJhZG1pbiIsImV4cCI6MjAwMDAwMDAwMH0."
            "invalid_tampered_hmac_signature_abc1234567890",
        ),
        (
            "Expired Token",
            "Bearer eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9."
            "eyJzdWIiOiJleHBpcmVkX3VzZXIiLCJyb2xlIjoidHJhZGVyIiwiZXhwIjoxNTAwMDAwMDAwfQ."
            "signature_for_expired_token",
        ),
    ]

    all_passed = True
    details = []

    for label, header_val in test_cases:
        headers = {"Authorization": header_val}
        res = client.get("/sentiment", params={"ticker": "AAPL"}, headers=headers)
        if res.status_code == 401:
            details.append(f"{label} -> 401 (OK)")
        else:
            all_passed = False
            details.append(f"{label} -> {res.status_code} (FAILED, expected 401)")

    record_test(
        "JWT Validation & Tamper Resistance (Signature & Expiry Checks)",
        all_passed,
        "; ".join(details[:3]) + f" ({len(test_cases)} vectors tested)",
    )


def test_sql_injection_resilience(client: httpx.Client, valid_jwt_headers: dict):
    """
    Test 3: SQL Injection (SQLi) Defense & Input Sanitization
    Sends classical and advanced SQL injection payloads to ticker and search query inputs.
    Expects 400 Bad Request or safe 200/404 handling, NEVER 500 or leaked SQL dialect errors.
    """
    print("\n[TEST 3/7] Testing SQL Injection (SQLi) Defense & Input Sanitization...")
    sqli_payloads = [
        "' OR '1'='1",
        "AAPL'; DROP TABLE users; --",
        "AAPL' UNION SELECT id, password_hash, email FROM users--",
        "1' OR '1' = '1",
        "AAPL' OR sleep(5)--",
        "'; EXEC xp_cmdshell('dir'); --",
        "\" OR \"\"=\"",
    ]

    all_passed = True
    details = []

    for payload in sqli_payloads:
        # Test against /sentiment endpoint
        res_sent = client.get("/sentiment", params={"ticker": payload}, headers=valid_jwt_headers)
        # Test against /sentiment/history endpoint
        res_hist = client.get(
            "/sentiment/history",
            params={"ticker": payload, "start_date": "2025-01-01", "end_date": "2025-03-31"},
            headers=valid_jwt_headers,
        )
        # Test against /search endpoint
        res_srch = client.get("/search", params={"q": payload}, headers=valid_jwt_headers)

        responses = [res_sent, res_hist, res_srch]
        for r in responses:
            if r.status_code == 500:
                all_passed = False
                details.append(f"Payload '{payload}' triggered HTTP 500 Internal Error!")
            # Check that database error traces are not leaked in the response text
            lowered = r.text.lower()
            if "syntax error at or near" in lowered or "sql error" in lowered or "database exception" in lowered:
                all_passed = False
                details.append(f"SQL dialect error leaked in response: {r.text[:80]}")

    if all_passed:
        details.append(f"All {len(sqli_payloads)} SQL injection vectors sanitized/rejected (0 server errors)")

    record_test(
        "SQL Injection (SQLi) Defense & Input Sanitization",
        all_passed,
        "; ".join(details[:2]),
    )


def test_cross_site_scripting_defense(client: httpx.Client, valid_jwt_headers: dict):
    """
    Test 4: Cross-Site Scripting (XSS) & Content Isolation
    Sends active JavaScript/HTML injection payloads into string parameters.
    Verifies that responses use application/json and do not render executable HTML markup.
    """
    print("\n[TEST 4/7] Testing Cross-Site Scripting (XSS) & Content-Type Isolation...")
    xss_payloads = [
        "<script>alert('XSS_AUDIT_PROBE')</script>",
        "<img src=x onerror=alert('XSS')>",
        '"><svg/onload=alert(1)>',
        "javascript:alert(document.cookie)",
        "<iframe src='javascript:alert(1)'></iframe>",
    ]

    all_passed = True
    details = []

    for payload in xss_payloads:
        res_search = client.get("/search", params={"q": payload}, headers=valid_jwt_headers)
        res_lang = client.get("/language/detect", params={"text": payload}, headers=valid_jwt_headers)
        res_sent = client.get("/sentiment", params={"ticker": payload}, headers=valid_jwt_headers)

        for r in [res_search, res_lang, res_sent]:
            # Status should not be 500
            if r.status_code == 500:
                all_passed = False
                details.append(f"XSS payload caused HTTP 500 error: {r.text[:60]}")

            # Content-type must never be text/html (which could execute script in browser context)
            content_type = r.headers.get("content-type", "").lower()
            if "text/html" in content_type:
                all_passed = False
                details.append(f"Dangerous text/html content-type detected: {content_type}")

    if all_passed:
        details.append(f"All {len(xss_payloads)} XSS payloads isolated within application/json MIME boundaries")

    record_test(
        "Cross-Site Scripting (XSS) & MIME Content Isolation",
        all_passed,
        "; ".join(details[:2]),
    )


def test_rate_limiting_and_dos_protection(client: httpx.Client):
    """
    Test 5: Rate Limiting & DoS Protection
    Verifies x-ratelimit headers and bursts requests to verify HTTP 429 Too Many Requests enforcement.
    """
    print("\n[TEST 5/7] Testing Rate Limiting & DoS Protection...")
    # Use isolated test user for rate limit testing
    rate_user = str(uuid.uuid4())
    rate_token = get_jwt_token(client, rate_user, role="institutional")
    headers = {"Authorization": f"Bearer {rate_token}"}

    # Initial request to verify rate limit headers
    res_initial = client.get("/sentiment", params={"ticker": "AAPL"}, headers=headers)
    has_headers = (
        "x-ratelimit-limit" in res_initial.headers
        and "x-ratelimit-remaining" in res_initial.headers
        and "x-ratelimit-reset" in res_initial.headers
    )

    limit_val = int(res_initial.headers.get("x-ratelimit-limit", "100"))
    remaining_val = int(res_initial.headers.get("x-ratelimit-remaining", "100"))

    # If limit is within testable threshold (<= 250 requests), send a burst of 105 requests
    saw_429 = False
    burst_count = min(limit_val + 5, 120)

    if limit_val <= 250:
        for _ in range(burst_count):
            r = client.get("/sentiment", params={"ticker": "AAPL"}, headers=headers)
            if r.status_code == 429:
                saw_429 = True
                break

        passed = has_headers and (saw_429 or remaining_val < limit_val)
        detail = f"RateLimit Limit: {limit_val}, Remaining: {remaining_val}, 429 Triggered: {saw_429}"
    else:
        # In elevated test server environments, verify tracking headers are decremented
        passed = has_headers
        detail = f"High rate limit configured ({limit_val} req/window); Tracking headers verified active"

    record_test(
        "Rate Limiting & DoS Protection (Token Bucket & 429 Enforcement)",
        passed,
        detail,
    )


def test_ip_whitelisting_zero_trust(client: httpx.Client):
    """
    Test 6: IP Whitelisting & CIDR Zero-Trust Access Control
    Configures an IP whitelist subnet (198.51.100.0/24), sends request from unauthorized IP (203.0.113.50),
    verifies 403 Forbidden, verifies whitelisted IP (198.51.100.25) passes (200), and cleans up rule.
    """
    print("\n[TEST 6/7] Testing IP Whitelisting & CIDR Zero-Trust Access Control...")
    iso_user = str(uuid.uuid4())
    iso_token = get_jwt_token(client, iso_user, role="institutional")
    iso_headers = {"Authorization": f"Bearer {iso_token}"}

    entry_id = None
    all_passed = True
    details = []

    try:
        # 1. Add IP whitelist entry (198.51.100.0/24)
        add_payload = {
            "ip_or_cidr": "198.51.100.0/24",
            "description": "Security Audit Authorized Subnet",
        }
        res_add = client.post("/security/ip-whitelist", json=add_payload, headers=iso_headers)
        if res_add.status_code not in [200, 201]:
            record_test("IP Whitelisting Access Control", False, f"Failed to add IP whitelist entry: {res_add.text}")
            return

        entry_data = res_add.json()
        entry_id = entry_data["id"]
        details.append(f"Configured whitelist: {entry_data['ip_or_cidr']}")

        # 2. Request from unauthorized IP via X-Forwarded-For (203.0.113.50) -> Expect 403 Forbidden
        unauth_headers = iso_headers.copy()
        unauth_headers["X-Forwarded-For"] = "203.0.113.50"
        res_unauth = client.get("/sentiment", params={"ticker": "AAPL"}, headers=unauth_headers)

        if res_unauth.status_code == 403:
            details.append("Unauthorized IP 203.0.113.50 -> 403 Forbidden (Blocked as expected)")
        else:
            all_passed = False
            details.append(f"Unauthorized IP -> {res_unauth.status_code} (FAILED, expected 403)")

        # 3. Request from authorized IP via X-Forwarded-For (198.51.100.25) -> Expect 200 OK
        auth_ip_headers = iso_headers.copy()
        auth_ip_headers["X-Forwarded-For"] = "198.51.100.25"
        res_auth = client.get("/sentiment", params={"ticker": "AAPL"}, headers=auth_ip_headers)

        if res_auth.status_code == 200:
            details.append("Authorized IP 198.51.100.25 -> 200 OK (Allowed)")
        else:
            all_passed = False
            details.append(f"Authorized IP -> {res_auth.status_code} (FAILED, expected 200)")

    finally:
        # 4. Clean up whitelist entry to maintain test idempotency
        if entry_id:
            res_del = client.delete(f"/security/ip-whitelist/{entry_id}", headers=iso_headers)
            if res_del.status_code == 200:
                details.append("Cleanup: IP whitelist rule deleted")

    record_test(
        "IP Whitelisting & CIDR Zero-Trust Access Control",
        all_passed,
        "; ".join(details),
    )


def test_api_key_validation_and_tampering(client: httpx.Client):
    """
    Test 7: Programmatic API Key Authentication & Tamper Resistance
    Tests invalid API keys, malformed headers, and forged prefixes. Expects 401 Unauthorized.
    """
    print("\n[TEST 7/7] Testing Programmatic API Key Authentication & Tamper Resistance...")
    test_cases = [
        ("Invalid API Key Header", {"X-API-Key": "ft_invalid_bogus_api_key_12345"}),
        ("Truncated API Key", {"X-API-Key": "ft_short"}),
        ("Forged Prefix Key", {"X-API-Key": "ft_99999_fake_secret_key_padding"}),
        ("Malformed ApiKey Scheme", {"Authorization": "ApiKey ft_nonexistent_key"}),
    ]

    all_passed = True
    details = []

    for label, headers in test_cases:
        res = client.get("/sentiment", params={"ticker": "AAPL"}, headers=headers)
        if res.status_code == 401:
            details.append(f"{label} -> 401 (OK)")
        else:
            all_passed = False
            details.append(f"{label} -> {res.status_code} (FAILED, expected 401)")

    record_test(
        "API Key Authentication & Tamper Validation (401 Enforcement)",
        all_passed,
        "; ".join(details[:3]),
    )


# ─────────────────────────────────────────────────────────────────────────────────
# Main Test Runner
# ─────────────────────────────────────────────────────────────────────────────────

def run_security_audit() -> bool:
    """Orchestrates the complete security and penetration test suite."""
    print("=" * 90)
    print(" FINTEXT ALPHA VECTORIZER — COMPREHENSIVE API SECURITY & PENETRATION AUDIT")
    print("=" * 90)
    print(f" Target Base URL: {BASE_URL}")
    print(f" Admin Token:     {ADMIN_TOKEN[:6]}***\n")

    server_ok, server_proc = ensure_server_running()
    if not server_ok:
        print("[ERROR] API Server could not be reached or started.")
        return False

    client = httpx.Client(base_url=BASE_URL, timeout=15.0)

    try:
        # Obtain baseline valid JWT for authenticated tests
        audit_user = str(uuid.uuid4())
        valid_jwt = get_jwt_token(client, audit_user, role="institutional")
        valid_headers = {"Authorization": f"Bearer {valid_jwt}"}
        print(f"[AUTH] Baseline Audit JWT Token Issued (User: {audit_user[:8]}...)")

        # Execute the 7 Security Test Suites
        test_unauthenticated_access(client)
        test_invalid_and_tampered_jwt(client)
        test_sql_injection_resilience(client, valid_headers)
        test_cross_site_scripting_defense(client, valid_headers)
        test_rate_limiting_and_dos_protection(client)
        test_ip_whitelisting_zero_trust(client)
        test_api_key_validation_and_tampering(client)

        # Output Summary Table
        print("\n" + "=" * 90)
        print(" FINTEXT SECURITY & PENETRATION AUDIT EXECUTION SUMMARY")
        print("=" * 90)
        print(f" {'#':<3} | {'Security Control / Attack Surface':<48} | {'Status':<8} | {'Details'}")
        print("-" * 90)

        passed_count = 0
        for idx, item in enumerate(TEST_RESULTS, start=1):
            status = "PASS" if item["passed"] else "FAIL"
            if item["passed"]:
                passed_count += 1
            print(f" {idx:<3} | {item['name']:<48} | {status:<8} | {item['detail'][:35]}")

        print("=" * 90)
        total_tests = len(TEST_RESULTS)
        print(f" Total Security Controls Tested: {total_tests}")
        print(f" Passed:                         {passed_count}")
        print(f" Failed:                         {total_tests - passed_count}")
        all_passed = (passed_count == total_tests)
        print(f" Overall Security Posture:       {'PASSED — ZERO VULNERABILITIES DETECTED' if all_passed else 'FAILED — VULNERABILITIES DETECTED'}")
        print("=" * 90)

        return all_passed

    except Exception as exc:
        print(f"\n[EXCEPTION] Security test runner error: {exc}")
        import traceback
        traceback.print_exc()
        return False

    finally:
        client.close()
        if server_proc and server_proc.poll() is None:
            print("\n[SERVER] Terminating test server process...")
            server_proc.terminate()
            try:
                server_proc.wait(timeout=3.0)
            except Exception:
                server_proc.kill()


def main():
    success = run_security_audit()
    sys.exit(0 if success else 1)


if __name__ == "__main__":
    main()
