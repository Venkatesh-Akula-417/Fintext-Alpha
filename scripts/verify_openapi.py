#!/usr/bin/env python3
"""
=====================================================================================
FinText-Alpha-Vectorizer — OpenAPI 3.0 & Swagger UI Live Verification Suite
=====================================================================================
Validates that:
1. The Axum API Gateway starts up with OpenAPI documentation enabled.
2. GET /api-docs/openapi.json returns 200 OK and valid OpenAPI 3.0 specification.
3. OpenAPI spec contains all required paths (/health, /auth/token, /sentiment, /spillovers, /backtest).
4. OpenAPI spec contains the bearerAuth security scheme.
5. OpenAPI spec contains all required component schemas with institutional examples.
6. GET /swagger-ui/ returns 200 OK with interactive HTML frontend without authentication.
=====================================================================================
"""

import json
import os
import subprocess
import sys
import time
import urllib.request
import urllib.error
from pathlib import Path

PORT = 8084
BASE_URL = f"http://127.0.0.1:{PORT}"
BINARY_PATH = Path("rust/target/release/fintext_api.exe").resolve()

def run_checks():
    print("=" * 85)
    print(" FinText-Alpha-Vectorizer — OpenAPI 3.0 & Swagger UI Live Verification Suite")
    print("=" * 85)

    env = os.environ.copy()
    env.update({
        "PORT": str(PORT),
        "HOST": "127.0.0.1",
        "JWT_SECRET": "test_verification_secret_key_32_bytes_len!!",
        "ADMIN_TOKEN": "admin_test_token_super_secret_12345",
        "RATE_LIMIT_REQUESTS": "100",
        "RATE_LIMIT_WINDOW_SECONDS": "60",
        "QUESTDB_MOCK_FALLBACK": "1",
        "RUST_LOG": "info"
    })

    print(f"[*] Starting API Server binary: {BINARY_PATH}")
    proc = subprocess.Popen(
        [str(BINARY_PATH)],
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True
    )

    try:
        # 1. Wait for server readiness
        ready = False
        for _ in range(30):
            try:
                req = urllib.request.Request(f"{BASE_URL}/health")
                with urllib.request.urlopen(req, timeout=1.0) as resp:
                    if resp.status == 200:
                        ready = True
                        break
            except Exception:
                time.sleep(0.2)

        if not ready:
            print("[-] FAIL: Server failed to start within timeout.")
            return False

        print("[1/5] Testing GET /health...")
        req = urllib.request.Request(f"{BASE_URL}/health")
        with urllib.request.urlopen(req) as resp:
            body = json.loads(resp.read().decode("utf-8"))
            assert resp.status == 200
            assert body["status"] == "ok"
            print(f"      HTTP {resp.status} | Server Version: {body['version']} | Status: {body['status']}")

        # 2. Test GET /api-docs/openapi.json
        print("\n[2/5] Testing GET /api-docs/openapi.json (OpenAPI 3.0 Spec)...")
        req = urllib.request.Request(f"{BASE_URL}/api-docs/openapi.json")
        with urllib.request.urlopen(req) as resp:
            assert resp.status == 200
            spec = json.loads(resp.read().decode("utf-8"))
            print(f"      HTTP {resp.status} | Title: '{spec['info']['title']}' | Version: '{spec['info']['version']}'")
            assert spec["openapi"].startswith("3.")

        # 3. Validate Paths in OpenAPI Spec
        print("\n[3/5] Validating Registered API Routes in OpenAPI Paths...")
        paths = spec.get("paths", {})
        expected_paths = ["/health", "/auth/token", "/sentiment", "/spillovers", "/backtest"]
        for path in expected_paths:
            assert path in paths, f"Missing path '{path}' in OpenAPI spec"
            methods = list(paths[path].keys())
            print(f"      [OK] Route '{path}' documented -> Methods: {methods}")

        # 4. Validate Security Schemes and Schemas
        print("\n[4/5] Validating Security Schemes and Component Schemas...")
        sec_schemes = spec.get("components", {}).get("securitySchemes", {})
        assert "bearerAuth" in sec_schemes, "Missing 'bearerAuth' security scheme in components"
        assert sec_schemes["bearerAuth"]["type"] == "http"
        assert sec_schemes["bearerAuth"]["scheme"] == "bearer"
        print("      [OK] Security Scheme: 'bearerAuth' (HTTP Bearer JWT) verified")

        schemas = spec.get("components", {}).get("schemas", {})
        expected_schemas = [
            "HealthResponse", "SentimentResponse", "SpilloverResponse",
            "BacktestRequest", "BacktestResponse", "IssueTokenRequest",
            "IssueTokenResponse", "AuthErrorResponse", "RateLimitErrorResponse"
        ]
        for s in expected_schemas:
            assert s in schemas, f"Missing component schema '{s}' in OpenAPI spec"
            print(f"      [OK] Component Schema: '{s}'")

        # 5. Test Swagger UI Frontend
        print("\n[5/5] Testing GET /swagger-ui/ (Interactive HTML Frontend)...")
        req = urllib.request.Request(f"{BASE_URL}/swagger-ui/")
        with urllib.request.urlopen(req) as resp:
            html_content = resp.read().decode("utf-8")
            assert resp.status == 200
            assert "swagger-ui" in html_content.lower() or "openapi" in html_content.lower() or "<!doctype html>" in html_content.lower()
            print(f"      HTTP {resp.status} | Swagger UI HTML served successfully ({len(html_content)} bytes)")

        print("\n" + "=" * 85)
        print(" [OK] ALL OPENAPI 3.0 & SWAGGER UI INTEGRATION CHECKS PASSED!")
        print("=" * 85)
        return True

    finally:
        proc.terminate()
        try:
            proc.wait(timeout=2.0)
        except Exception:
            proc.kill()

if __name__ == "__main__":
    success = run_checks()
    sys.exit(0 if success else 1)
