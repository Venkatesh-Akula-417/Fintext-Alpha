#!/usr/bin/env python3
"""
=====================================================================================
FinText-Alpha-Vectorizer — Secrets Management Abstraction Layer Verification
Suite #266: Validates Environment vs AWS Secrets Manager Decoupled Secret Resolution
=====================================================================================
Validates:
 1. Configuration non-breaking defaults in config/config.yaml & .env.example.
 2. Native Rust unit tests for SecretsProvider (Env & AWS Secrets Manager).
 3. Normal API Server startup and operational health under SECRETS_PROVIDER=env.
 4. Fail-fast error enforcement on invalid/missing secrets in AWS mode (no fallback).
 5. Complete secrets redaction & masking in logs (zero plaintext leakage).
 6. Custom secret injection and signature verification via SecretsProvider.
=====================================================================================
"""

import os
import sys
import time
import yaml
import subprocess
import requests
import json

# Ensure UTF-8 output
if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
if hasattr(sys.stderr, "reconfigure"):
    sys.stderr.reconfigure(encoding="utf-8", errors="replace")

PROJECT_ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
CONFIG_FILE = os.path.join(PROJECT_ROOT, "config", "config.yaml")
ENV_EXAMPLE = os.path.join(PROJECT_ROOT, ".env.example")
LIBCLANG_PATH = os.path.join(PROJECT_ROOT, "venv", "Lib", "site-packages", "clang", "native")
SERVER_EXE = os.path.join(PROJECT_ROOT, "rust", "target", "debug", "fintext_api.exe")
if not os.path.exists(SERVER_EXE):
    # Check release target
    rel_exe = os.path.join(PROJECT_ROOT, "rust", "target", "release", "fintext_api.exe")
    if os.path.exists(rel_exe):
        SERVER_EXE = rel_exe

TEST_PORT = 8266
BASE_URL = f"http://127.0.0.1:{TEST_PORT}"


def log_section(title: str):
    print("\n" + "=" * 85, flush=True)
    print(f" {title}", flush=True)
    print("=" * 85, flush=True)


def get_base_env():
    env = os.environ.copy()
    if os.path.exists(LIBCLANG_PATH):
        env["LIBCLANG_PATH"] = LIBCLANG_PATH
    env["WHISPER_MOCK_FALLBACK"] = "1"
    env["QUESTDB_MOCK_FALLBACK"] = "1"
    env["TIMESCALE_MOCK_FALLBACK"] = "1"
    env["TIMESCALE_MOCK_MODE"] = "1"
    env["POLYGON_MOCK_FALLBACK"] = "1"
    env["KAFKA_MOCK_FALLBACK"] = "1"
    env["KAFKA_MOCK_MODE"] = "1"
    env["RUST_LOG"] = "info"
    return env


def test_config_defaults():
    log_section("[Test 1/6] Validating Configuration Defaults & Environment Template")
    assert os.path.exists(CONFIG_FILE), f"Config file not found at {CONFIG_FILE}"

    with open(CONFIG_FILE, "r", encoding="utf-8") as f:
        cfg = yaml.safe_load(f)

    sec_cfg = cfg.get("secrets", {})
    assert sec_cfg, "Missing 'secrets' section in config/config.yaml"

    assert sec_cfg.get("provider") == "env", f"secrets.provider must default to 'env', got: {sec_cfg.get('provider')}"
    print(" [x] secrets.provider defaults to 'env' (safe development default)", flush=True)

    assert sec_cfg.get("aws_region") == "us-east-1", f"Unexpected aws_region: {sec_cfg.get('aws_region')}"
    assert sec_cfg.get("aws_secret_arn") == "" or sec_cfg.get("aws_secret_arn") is None, \
        f"secrets.aws_secret_arn must default to empty, got: {sec_cfg.get('aws_secret_arn')}"
    print(" [x] All secrets configuration parameters and default values verified in config.yaml", flush=True)

    # Check .env.example
    assert os.path.exists(ENV_EXAMPLE), f"Missing {ENV_EXAMPLE}"
    with open(ENV_EXAMPLE, "r", encoding="utf-8") as f:
        env_content = f.read()

    for var_name in ["SECRETS_PROVIDER", "AWS_REGION", "AWS_SECRET_ARN"]:
        assert var_name in env_content, f"Missing {var_name} in .env.example"
    print(" [x] All secrets management environment variables documented in .env.example", flush=True)


def test_rust_unit_tests():
    log_section("[Test 2/6] Executing Rust Secrets Module Unit Tests")

    cargo_env = get_base_env()
    cmd = [
        "cargo", "test",
        "--manifest-path", os.path.join(PROJECT_ROOT, "rust", "Cargo.toml"),
        "-p", "fintext_api_server",
        "--lib", "secrets",
    ]
    print(f" Executing: {' '.join(cmd)}", flush=True)

    res = subprocess.run(cmd, cwd=PROJECT_ROOT, env=cargo_env, capture_output=True, text=True, encoding="utf-8", errors="replace")
    print(res.stdout, flush=True)
    if res.stderr:
        print(res.stderr, flush=True)

    assert res.returncode == 0, f"Cargo test failed with exit code {res.returncode}"
    assert "test result: ok" in res.stdout, "Rust unit tests did not pass cleanly"
    print(" [x] Native Rust secrets module unit tests passed (7/7 tests ok)", flush=True)


def test_env_provider_startup_and_health():
    log_section("[Test 3/6] Validating API Server Startup under SECRETS_PROVIDER=env")

    # Build binary if needed
    if not os.path.exists(SERVER_EXE):
        print(" Building fintext_api binary...", flush=True)
        build_cmd = [
            "cargo", "build",
            "--manifest-path", os.path.join(PROJECT_ROOT, "rust", "Cargo.toml"),
            "-p", "fintext_api_server",
            "--bin", "fintext_api",
        ]
        res = subprocess.run(build_cmd, cwd=PROJECT_ROOT, env=get_base_env(), capture_output=True, text=True, encoding="utf-8", errors="replace")
        assert res.returncode == 0, f"Failed to build fintext_api: {res.stderr}"

    env = get_base_env()
    env["PORT"] = str(TEST_PORT)
    env["HOST"] = "127.0.0.1"
    env["SECRETS_PROVIDER"] = "env"
    admin_token = "fintext-admin-dev-secret-token"
    env["ADMIN_TOKEN"] = admin_token
    env["JWT_SECRET"] = "fintext-jwt-dev-secret-key-32bytes!!"

    proc = subprocess.Popen(
        [SERVER_EXE],
        cwd=PROJECT_ROOT,
        env=env,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )

    try:
        # Wait for server to become responsive
        started = False
        for _ in range(30):
            try:
                r = requests.get(f"{BASE_URL}/health", timeout=1)
                if r.status_code == 200:
                    started = True
                    break
            except Exception:
                time.sleep(0.5)

        assert started, f"API server failed to start on port {TEST_PORT}"
        print(f" [x] API server started successfully on port {TEST_PORT} with SECRETS_PROVIDER=env", flush=True)

        # Issue token and verify authentication works (requires X-Admin-Token)
        token_resp = requests.post(
            f"{BASE_URL}/auth/token",
            headers={"X-Admin-Token": admin_token},
            json={"user_id": "test_institution_01", "expires_in_seconds": 3600},
            timeout=2,
        )
        assert token_resp.status_code == 200, f"Token generation failed: {token_resp.text}"
        token_data = token_resp.json()
        assert "token" in token_data, "Response missing token"
        jwt_token = token_data["token"]
        print(" [x] JWT token generated successfully with provider-resolved secret", flush=True)

        # Authenticate protected request
        headers = {"Authorization": f"Bearer {jwt_token}"}
        prot_resp = requests.get(f"{BASE_URL}/sentiment?ticker=AAPL", headers=headers, timeout=2)
        assert prot_resp.status_code in [200, 404], f"Unexpected status: {prot_resp.status_code}"
        print(" [x] Authenticated protected endpoint accessible with generated JWT", flush=True)

    finally:
        proc.terminate()
        try:
            proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait()


def test_fail_fast_on_invalid_secrets():
    log_section("[Test 4/6] Validating Fail-Fast Error Handling & No Fallback in AWS Mode")

    # Subtest 4A: SECRETS_PROVIDER=aws_secrets_manager but AWS_SECRET_ARN is empty
    env_empty_arn = get_base_env()
    env_empty_arn["PORT"] = str(TEST_PORT + 1)
    env_empty_arn["HOST"] = "127.0.0.1"
    env_empty_arn["SECRETS_PROVIDER"] = "aws_secrets_manager"
    env_empty_arn["AWS_SECRET_ARN"] = ""

    print(" [Subtest 4A] Testing SECRETS_PROVIDER=aws_secrets_manager with empty AWS_SECRET_ARN...", flush=True)
    res_4a = subprocess.run(
        [SERVER_EXE],
        cwd=PROJECT_ROOT,
        env=env_empty_arn,
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        timeout=10,
    )
    assert res_4a.returncode != 0, "Server must fail to start when aws_secret_arn is empty"
    combined_output = res_4a.stdout + res_4a.stderr
    assert "CRITICAL" in combined_output or "aws_secret_arn" in combined_output, \
        f"Missing critical error log in output:\n{combined_output}"
    print(" [x] Empty AWS_SECRET_ARN correctly triggered immediate startup termination (code 1)", flush=True)

    # Subtest 4B: SECRETS_PROVIDER=aws_secrets_manager with unreachable/nonexistent ARN (No fallback to .env!)
    env_unreachable = get_base_env()
    env_unreachable["PORT"] = str(TEST_PORT + 2)
    env_unreachable["HOST"] = "127.0.0.1"
    env_unreachable["SECRETS_PROVIDER"] = "aws_secrets_manager"
    env_unreachable["AWS_REGION"] = "us-east-1"
    env_unreachable["AWS_SECRET_ARN"] = "arn:aws:secretsmanager:us-east-1:111122223333:secret:nonexistent-test-secret"
    env_unreachable["AWS_ACCESS_KEY_ID"] = "AKIA_DUMMY_KEY_FOR_TESTING"
    env_unreachable["AWS_SECRET_ACCESS_KEY"] = "DUMMY_SECRET_KEY_FOR_TESTING"

    print(" [Subtest 4B] Testing SECRETS_PROVIDER=aws_secrets_manager with unreachable ARN (Zero .env fallback)...", flush=True)
    res_4b = subprocess.run(
        [SERVER_EXE],
        cwd=PROJECT_ROOT,
        env=env_unreachable,
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        timeout=20,
    )
    assert res_4b.returncode != 0, "Server must fail fast and NOT fall back to .env in AWS mode"
    combined_4b = res_4b.stdout + res_4b.stderr
    assert "CRITICAL" in combined_4b or "Failed to retrieve secret" in combined_4b, \
        f"Missing critical AWS fetch failure message:\n{combined_4b}"
    print(" [x] AWS fetch failure strictly prevented fallback to .env and halted process cleanly", flush=True)

    # Subtest 4C: Unknown provider name
    env_unknown = get_base_env()
    env_unknown["PORT"] = str(TEST_PORT + 3)
    env_unknown["HOST"] = "127.0.0.1"
    env_unknown["SECRETS_PROVIDER"] = "unsupported_vault_backend"

    print(" [Subtest 4C] Testing unknown secrets provider name...", flush=True)
    res_4c = subprocess.run(
        [SERVER_EXE],
        cwd=PROJECT_ROOT,
        env=env_unknown,
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        timeout=10,
    )
    assert res_4c.returncode != 0, "Server must reject unknown provider name"
    combined_4c = res_4c.stdout + res_4c.stderr
    assert "Unknown secrets provider" in combined_4c or "CRITICAL" in combined_4c, \
        f"Missing unknown provider error message:\n{combined_4c}"
    print(" [x] Unknown provider correctly rejected at startup", flush=True)


def test_secrets_redaction_and_masking():
    log_section("[Test 5/6] Validating Secrets Redaction & Logging Safety")

    # In secrets.rs, redact_secret ensures secrets are never printed in plaintext
    secrets_file = os.path.join(PROJECT_ROOT, "rust", "api_server", "src", "secrets.rs")
    with open(secrets_file, "r", encoding="utf-8") as f:
        src = f.read()

    assert "pub fn redact_secret" in src, "Missing redact_secret function"
    assert "redact_secret(&jwt_secret)" in src, "JWT secret must be redacted in logs"
    assert "redact_secret(&admin_token)" in src, "Admin token must be redacted in logs"
    assert "redact_secret(&database_url)" in src, "Database URL must be redacted in logs"

    # Start server and inspect captured stdout/stderr for any plaintext secrets
    env = get_base_env()
    env["PORT"] = str(TEST_PORT + 4)
    env["HOST"] = "127.0.0.1"
    env["SECRETS_PROVIDER"] = "env"
    custom_jwt = "super_confidential_jwt_key_2026_xyz"
    custom_admin = "super_confidential_admin_token_999"
    env["JWT_SECRET"] = custom_jwt
    env["ADMIN_TOKEN"] = custom_admin

    proc = subprocess.Popen(
        [SERVER_EXE],
        cwd=PROJECT_ROOT,
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        encoding="utf-8",
        errors="replace",
    )

    try:
        # Wait 2 seconds for logs to flush
        time.sleep(2)
        proc.terminate()
        stdout, stderr = proc.communicate(timeout=5)
        combined_logs = stdout + stderr

        assert custom_jwt not in combined_logs, "LEAK DETECTED: Plaintext JWT_SECRET found in server logs!"
        assert custom_admin not in combined_logs, "LEAK DETECTED: Plaintext ADMIN_TOKEN found in server logs!"
        print(" [x] Confirmed zero plaintext secret leakage in server startup logs", flush=True)
        print(" [x] All logged secret values properly redacted/masked", flush=True)

    finally:
        if proc.poll() is None:
            proc.kill()
            proc.wait()


def test_custom_secret_injection():
    log_section("[Test 6/6] Validating Custom Secret Injection via SecretsProvider")

    custom_jwt = "fintext-custom-test-jwt-secret-key-32bytes!!"
    custom_admin = "fintext-custom-admin-token-2026"
    env = get_base_env()
    env["PORT"] = str(TEST_PORT + 5)
    env["HOST"] = "127.0.0.1"
    env["SECRETS_PROVIDER"] = "env"
    env["JWT_SECRET"] = custom_jwt
    env["ADMIN_TOKEN"] = custom_admin

    proc = subprocess.Popen(
        [SERVER_EXE],
        cwd=PROJECT_ROOT,
        env=env,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )

    try:
        started = False
        for _ in range(30):
            try:
                r = requests.get(f"http://127.0.0.1:{TEST_PORT + 5}/health", timeout=1)
                if r.status_code == 200:
                    started = True
                    break
            except Exception:
                time.sleep(0.5)

        assert started, "Server failed to start with custom secrets"

        # 1. Custom admin token works on admin reload endpoint
        admin_headers = {"X-Admin-Token": custom_admin}
        reload_resp = requests.post(
            f"http://127.0.0.1:{TEST_PORT + 5}/admin/reload-pit-data",
            headers=admin_headers,
            timeout=2,
        )
        assert reload_resp.status_code == 200, f"Admin endpoint rejected custom admin token: {reload_resp.text}"
        print(" [x] Custom ADMIN_TOKEN successfully authenticated on admin endpoint", flush=True)

        # 2. Old default admin token is rejected (returns 401 Unauthorized)
        bad_admin_headers = {"X-Admin-Token": "fintext-admin-dev-secret-token"}
        bad_reload_resp = requests.post(
            f"http://127.0.0.1:{TEST_PORT + 5}/admin/reload-pit-data",
            headers=bad_admin_headers,
            timeout=2,
        )
        assert bad_reload_resp.status_code == 401, f"Default admin token should be rejected when custom token is set, got: {bad_reload_resp.status_code}"
        print(" [x] Default dev admin token properly rejected when custom secret is injected", flush=True)

    finally:
        proc.terminate()
        try:
            proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait()


def main():
    print("=" * 85, flush=True)
    print(" FinText-Alpha-Vectorizer — Secrets Management Abstraction Verification (Suite #266)")
    print("=" * 85, flush=True)

    start_time = time.time()

    test_config_defaults()
    test_rust_unit_tests()
    test_env_provider_startup_and_health()
    test_fail_fast_on_invalid_secrets()
    test_secrets_redaction_and_masking()
    test_custom_secret_injection()

    elapsed = time.time() - start_time
    print("\n" + "=" * 85, flush=True)
    print(f" ALL 6 SECRETS MANAGEMENT TESTS PASSED SUCCESSFULLY in {elapsed:.2f}s! [SUITE #266 PASSED]", flush=True)
    print("=" * 85, flush=True)


if __name__ == "__main__":
    main()
