#!/usr/bin/env python3
"""
===============================================================================
FinText-Alpha-Vectorizer — Suite #237: API Sandbox Environment & Isolation Engine
===============================================================================
Verifies:
  1.  Unauthenticated GET /sandbox/status returns 401 Unauthorized
  2.  Unauthenticated POST /sandbox/activate returns 401 Unauthorized
  3.  Unauthenticated POST /sandbox/deactivate returns 401 Unauthorized
  4.  Initial GET /sandbox/status for new user returns active: false with default mock version
  5.  POST /sandbox/activate activates sandbox mode (active: true, activated_at set, available_endpoints listed)
  6.  GET /sandbox/status verifies active status persistence for current user
  7.  Live API request under active sandbox mode returns X-FinText-Sandbox and X-FinText-Mock-Version headers
  8.  Token-level stateless sandbox claim (sandbox: true) triggers sandbox mode and headers
  9.  Multi-user sandbox isolation: User A in sandbox mode does not affect User B's live status
  10. POST /sandbox/deactivate deactivates sandbox mode (active: false, deactivated_at set)
  11. GET /sandbox/status after deactivation confirms active: false
  12. Python SDK Sync Client integration verification (client.get_sandbox_status, activate, deactivate)
  13. Python SDK Async Client integration verification (await async_client.get_sandbox_status, activate, deactivate)
  14. OpenAPI specification verification (/sandbox/* paths, schema SandboxStatusResponse, tag "Sandbox & Testing")
===============================================================================
"""

import asyncio
import os
from pathlib import Path
import subprocess
import sys
import time

import httpx

if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
if hasattr(sys.stderr, "reconfigure"):
    sys.stderr.reconfigure(encoding="utf-8", errors="replace")

PROJECT_ROOT = Path(__file__).resolve().parent.parent
SDK_PATH = PROJECT_ROOT / "python_sdk" / "src"
if str(SDK_PATH) not in sys.path:
    sys.path.insert(0, str(SDK_PATH))

PORT = 8136
BASE_URL = f"http://127.0.0.1:{PORT}"
ADMIN_TOKEN = "test_admin_token_xyz123_valid_32_bytes_length!"
SERVER_EXE = PROJECT_ROOT / "rust" / "target" / "debug" / "fintext_api.exe"

passed = 0
failed = 0
total = 14


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
                time.sleep(0.4)

        if not ready:
            if self.process.poll() is not None:
                raise RuntimeError(f"Server exited prematurely with return code {self.process.returncode}")
            raise TimeoutError("FinText API server failed to respond within 25 seconds.")

        print(f"[READY] API Server is healthy at {BASE_URL}")
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        if self.process and self.process.poll() is None:
            print("[CLEANUP] Terminating FinText API Server process...")
            self.process.terminate()
            try:
                self.process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait(timeout=2)


def get_token(user_id: str = "sandbox_tester_user", role: str = "institutional", sandbox: bool = None) -> str:
    payload = {"user_id": user_id, "role": role, "expiry_seconds": 3600}
    if sandbox is not None:
        payload["sandbox"] = sandbox
    headers = {"X-Admin-Token": ADMIN_TOKEN}
    r = httpx.post(f"{BASE_URL}/auth/token", json=payload, headers=headers, timeout=5.0)
    r.raise_for_status()
    return r.json()["token"]


def run_tests():
    print("=" * 80)
    print("FINTEXT ALPHA VECTORIZER — SUITE #237: API SANDBOX ENVIRONMENT & ISOLATION ENGINE")
    print("=" * 80)

    with ServerContext():
        # Phase 1: Unauthenticated GET /sandbox/status returns 401
        try:
            r = httpx.get(f"{BASE_URL}/sandbox/status", timeout=5.0)
            ok = r.status_code == 401
            report(1, "Unauthenticated GET /sandbox/status returns 401 Unauthorized", ok, f"status={r.status_code}")
        except Exception as e:
            report(1, "Unauthenticated GET /sandbox/status returns 401 Unauthorized", False, str(e))

        # Phase 2: Unauthenticated POST /sandbox/activate returns 401
        try:
            r = httpx.post(f"{BASE_URL}/sandbox/activate", timeout=5.0)
            ok = r.status_code == 401
            report(2, "Unauthenticated POST /sandbox/activate returns 401 Unauthorized", ok, f"status={r.status_code}")
        except Exception as e:
            report(2, "Unauthenticated POST /sandbox/activate returns 401 Unauthorized", False, str(e))

        # Phase 3: Unauthenticated POST /sandbox/deactivate returns 401
        try:
            r = httpx.post(f"{BASE_URL}/sandbox/deactivate", timeout=5.0)
            ok = r.status_code == 401
            report(3, "Unauthenticated POST /sandbox/deactivate returns 401 Unauthorized", ok, f"status={r.status_code}")
        except Exception as e:
            report(3, "Unauthenticated POST /sandbox/deactivate returns 401 Unauthorized", False, str(e))

        token_user_a = get_token("sandbox_user_alpha")
        auth_a = {"Authorization": f"Bearer {token_user_a}"}

        # Phase 4: Initial GET /sandbox/status for new user returns active: false
        try:
            r = httpx.get(f"{BASE_URL}/sandbox/status", headers=auth_a, timeout=5.0)
            data = r.json()
            ok = (
                r.status_code == 200
                and data.get("active") is False
                and data.get("mock_data_version") == "sandbox-v1.0"
                and isinstance(data.get("available_endpoints"), list)
            )
            report(4, "Initial GET /sandbox/status returns active: false and version sandbox-v1.0", ok, str(data))
        except Exception as e:
            report(4, "Initial GET /sandbox/status returns active: false and version sandbox-v1.0", False, str(e))

        # Phase 5: POST /sandbox/activate activates sandbox mode
        try:
            r = httpx.post(f"{BASE_URL}/sandbox/activate", headers=auth_a, timeout=5.0)
            data = r.json()
            ok = (
                r.status_code == 200
                and data.get("active") is True
                and data.get("mock_data_version") == "sandbox-v1.0"
                and data.get("activated_at") is not None
                and len(data.get("available_endpoints", [])) > 0
            )
            report(5, "POST /sandbox/activate activates sandbox mode successfully", ok, str(data))
        except Exception as e:
            report(5, "POST /sandbox/activate activates sandbox mode successfully", False, str(e))

        # Phase 6: GET /sandbox/status verifies active status persistence for current user
        try:
            r = httpx.get(f"{BASE_URL}/sandbox/status", headers=auth_a, timeout=5.0)
            data = r.json()
            ok = (
                r.status_code == 200
                and data.get("active") is True
                and data.get("activated_at") is not None
            )
            report(6, "GET /sandbox/status reflects active sandbox state", ok, str(data))
        except Exception as e:
            report(6, "GET /sandbox/status reflects active sandbox state", False, str(e))

        # Phase 7: Live API request under active sandbox mode returns X-FinText-Sandbox & Mock-Version headers
        try:
            r = httpx.get(f"{BASE_URL}/sentiment?ticker=AAPL", headers=auth_a, timeout=5.0)
            sb_hdr = r.headers.get("X-FinText-Sandbox")
            ver_hdr = r.headers.get("X-FinText-Mock-Version")
            ok = (
                r.status_code == 200
                and sb_hdr == "true"
                and ver_hdr == "sandbox-v1.0"
            )
            report(7, "API request under active sandbox injects X-FinText-Sandbox & X-FinText-Mock-Version headers", ok, f"Sandbox={sb_hdr}, Version={ver_hdr}")
        except Exception as e:
            report(7, "API request under active sandbox injects X-FinText-Sandbox & X-FinText-Mock-Version headers", False, str(e))

        # Phase 8: Token-level stateless sandbox claim (sandbox: true) triggers sandbox mode and headers
        try:
            token_sb_claim = get_token("stateless_sandbox_user", sandbox=True)
            auth_sb_claim = {"Authorization": f"Bearer {token_sb_claim}"}
            r = httpx.get(f"{BASE_URL}/sentiment?ticker=NVDA", headers=auth_sb_claim, timeout=5.0)
            sb_hdr = r.headers.get("X-FinText-Sandbox")
            ver_hdr = r.headers.get("X-FinText-Mock-Version")
            ok = (
                r.status_code == 200
                and sb_hdr == "true"
                and ver_hdr == "sandbox-v1.0"
            )
            report(8, "Stateless token-level sandbox claim (sandbox: true) activates sandbox headers", ok, f"Sandbox={sb_hdr}, Version={ver_hdr}")
        except Exception as e:
            report(8, "Stateless token-level sandbox claim (sandbox: true) activates sandbox headers", False, str(e))

        # Phase 9: Multi-user sandbox isolation: User A active does not affect User B
        try:
            token_user_b = get_token("production_user_beta")
            auth_b = {"Authorization": f"Bearer {token_user_b}"}
            r_b_status = httpx.get(f"{BASE_URL}/sandbox/status", headers=auth_b, timeout=5.0)
            data_b = r_b_status.json()

            r_b_req = httpx.get(f"{BASE_URL}/sentiment?ticker=MSFT", headers=auth_b, timeout=5.0)
            sb_hdr_b = r_b_req.headers.get("X-FinText-Sandbox")

            ok = (
                r_b_status.status_code == 200
                and data_b.get("active") is False
                and sb_hdr_b is None
            )
            report(9, "Multi-user sandbox isolation: User B remains in production mode", ok, f"User B active={data_b.get('active')}, SandboxHdr={sb_hdr_b}")
        except Exception as e:
            report(9, "Multi-user sandbox isolation: User B remains in production mode", False, str(e))

        # Phase 10: POST /sandbox/deactivate deactivates sandbox mode
        try:
            r = httpx.post(f"{BASE_URL}/sandbox/deactivate", headers=auth_a, timeout=5.0)
            data = r.json()
            ok = (
                r.status_code == 200
                and data.get("active") is False
                and data.get("deactivated_at") is not None
            )
            report(10, "POST /sandbox/deactivate deactivates sandbox mode successfully", ok, str(data))
        except Exception as e:
            report(10, "POST /sandbox/deactivate deactivates sandbox mode successfully", False, str(e))

        # Phase 11: GET /sandbox/status after deactivation confirms active: false
        try:
            r = httpx.get(f"{BASE_URL}/sandbox/status", headers=auth_a, timeout=5.0)
            data = r.json()
            r_sent = httpx.get(f"{BASE_URL}/sentiment?ticker=AAPL", headers=auth_a, timeout=5.0)
            sb_hdr = r_sent.headers.get("X-FinText-Sandbox")
            ok = (
                r.status_code == 200
                and data.get("active") is False
                and sb_hdr is None
            )
            report(11, "GET /sandbox/status confirms deactivated state and subsequent calls omit sandbox headers", ok, f"active={data.get('active')}, SandboxHdr={sb_hdr}")
        except Exception as e:
            report(11, "GET /sandbox/status confirms deactivated state and subsequent calls omit sandbox headers", False, str(e))

        # Phase 12: Python SDK Sync Client integration verification
        try:
            from fintext import FinTextClient, SandboxStatusResponse

            token_sdk_sync = get_token("sdk_sync_tester")
            with FinTextClient(base_url=BASE_URL, api_token=token_sdk_sync) as client:
                st1 = client.get_sandbox_status()
                assert isinstance(st1, SandboxStatusResponse)
                assert st1.active is False

                st2 = client.activate_sandbox()
                assert isinstance(st2, SandboxStatusResponse)
                assert st2.active is True
                assert st2.activated_at is not None

                st3 = client.deactivate_sandbox()
                assert isinstance(st3, SandboxStatusResponse)
                assert st3.active is False
                assert st3.deactivated_at is not None

            report(12, "Python SDK Sync Client (client.get_sandbox_status, activate, deactivate) verification", True)
        except Exception as e:
            report(12, "Python SDK Sync Client verification", False, str(e))

        # Phase 13: Python SDK Async Client integration verification
        async def verify_async_sdk():
            from fintext import FinTextAsyncClient, SandboxStatusResponse

            token_sdk_async = get_token("sdk_async_tester")
            async with FinTextAsyncClient(base_url=BASE_URL, api_token=token_sdk_async) as async_client:
                st1 = await async_client.get_sandbox_status()
                assert isinstance(st1, SandboxStatusResponse)
                assert st1.active is False

                st2 = await async_client.activate_sandbox()
                assert isinstance(st2, SandboxStatusResponse)
                assert st2.active is True
                assert st2.activated_at is not None

                st3 = await async_client.deactivate_sandbox()
                assert isinstance(st3, SandboxStatusResponse)
                assert st3.active is False
                assert st3.deactivated_at is not None

        try:
            asyncio.run(verify_async_sdk())
            report(13, "Python SDK Async Client (async_client.get_sandbox_status, activate, deactivate) verification", True)
        except Exception as e:
            report(13, "Python SDK Async Client verification", False, str(e))

        # Phase 14: OpenAPI specification verification
        try:
            r = httpx.get(f"{BASE_URL}/api-docs/openapi.json", timeout=5.0)
            spec = r.json()
            paths = spec.get("paths", {})
            has_status_path = "/sandbox/status" in paths
            has_activate_path = "/sandbox/activate" in paths
            has_deactivate_path = "/sandbox/deactivate" in paths

            schemas = spec.get("components", {}).get("schemas", {})
            has_schema = "SandboxStatusResponse" in schemas

            # Check tag
            tags = [t.get("name") for t in spec.get("tags", [])]
            has_tag = "Sandbox & Testing" in tags

            ok = has_status_path and has_activate_path and has_deactivate_path and has_schema and has_tag
            detail = f"status_path={has_status_path}, activate={has_activate_path}, deactivate={has_deactivate_path}, schema={has_schema}, tag={has_tag}"
            report(14, "OpenAPI specification contains /sandbox/* paths, schema SandboxStatusResponse, and tag", ok, detail)
        except Exception as e:
            report(14, "OpenAPI specification verification", False, str(e))

    print("-" * 80)
    print(f"Results: {passed}/{total} passed, {failed} failed")
    print("=" * 80)
    if failed > 0:
        sys.exit(1)


if __name__ == "__main__":
    run_tests()
