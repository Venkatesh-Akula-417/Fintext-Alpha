#!/usr/bin/env python3
"""
===============================================================================
FinText-Alpha-Vectorizer — Verification Suite #268:
PIT Certificate Cryptographic Proof Archival & Independent Hash Verifiability
===============================================================================
Verifies:
  1.  Configuration & Defaults Validation (config.yaml & .env.example)
  2.  Native Rust Unit Tests for Canonicalization, Pre-image & Atomic Write
  3.  Live API Server Startup & Health Probe on Port 8268
  4.  GET /pit/certificate returns 200 OK with archive_object_key and archive_timestamp
  5.  Archived Proof File Exists on Disk with Matching Filename Pattern
  6.  Byte-Exact Cryptographic Hash Reproducibility:
      sha256(archived_raw_bytes) == cert.signature
  7.  Canonical JSON Alphabetical Key Ordering & Deterministic Formatting
  8.  Multi-Request Parametric Archival (Distinct Versions, Universes, UUIDs)
  9.  Disabled Archival Mode Graceful Fallback (PIT_CERT_ARCHIVE_ENABLED=false)
  10. Python SDK Sync & Async Client Model Validation with Archive Fields
  11. OpenAPI 3.0 Schema Registration & Documentation Audit
===============================================================================
"""

import asyncio
import hashlib
import json
import os
from pathlib import Path
import shutil
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

PORT = 8268
PORT_DISABLED = 8269
BASE_URL = f"http://127.0.0.1:{PORT}"
BASE_URL_DISABLED = f"http://127.0.0.1:{PORT_DISABLED}"
ADMIN_TOKEN = "test_admin_token_xyz123_valid_32_bytes_length!"
SERVER_EXE = PROJECT_ROOT / "rust" / "target" / "debug" / "fintext_api.exe"
TEST_ARCHIVE_DIR = PROJECT_ROOT / "data" / "pit-cert-archive-test-8268"

passed = 0
failed = 0
total = 11


def report(phase: int, name: str, ok: bool, detail: str = ""):
    global passed, failed
    if ok:
        passed += 1
        print(f"  ✅ Phase {phase:2d} │ {name}")
    else:
        failed += 1
        print(f"  ❌ Phase {phase:2d} │ {name}")
        if detail:
            print(f"     └─ {detail}")


class ServerContext:
    def __init__(self, port: int, extra_env: dict = None):
        self.port = port
        self.base_url = f"http://127.0.0.1:{port}"
        self.extra_env = extra_env or {}
        self.process = None

    def __enter__(self):
        print(f"[STARTING] Spawning FinText API Server on port {self.port}...")
        env = os.environ.copy()
        env["PORT"] = str(self.port)
        env["HOST"] = "127.0.0.1"
        env["ADMIN_TOKEN"] = ADMIN_TOKEN
        env["JWT_SECRET"] = "super_secret_test_jwt_key_32_bytes_len!!"
        env["QUESTDB_MOCK_FALLBACK"] = "1"
        env["POLYGON_MOCK_FALLBACK"] = "1"
        env["WHISPER_MOCK_FALLBACK"] = "1"
        env["NATS_MOCK_MODE"] = "1"
        env["CHAT_ALERTS_MOCK"] = "1"
        env["CONFIG_PATH"] = str(PROJECT_ROOT / "config" / "config.yaml")

        for k, v in self.extra_env.items():
            env[k] = v

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
                r = httpx.get(f"{self.base_url}/health", timeout=1.0)
                if r.status_code == 200:
                    ready = True
                    break
            except Exception:
                pass
            time.sleep(0.3)

        if not ready:
            if self.process.poll() is not None:
                raise RuntimeError(f"Server on port {self.port} died prematurely with code {self.process.returncode}")
            raise TimeoutError(f"Server on port {self.port} failed to become healthy within 25 seconds")

        print(f"[READY] API Server is healthy at {self.base_url}")
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        if self.process:
            print(f"[CLEANUP] Terminating FinText API Server process on port {self.port}...")
            self.process.terminate()
            try:
                self.process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait(timeout=2)


def run_all_phases():
    print("=" * 80)
    print(" FinText-Alpha-Vectorizer — Suite #268: PIT Certificate Cryptographic Archival")
    print("=" * 80)
    print(f" Project Root: {PROJECT_ROOT}")
    print(f" Server Port:  {PORT}")

    # Ensure clean test archive directory
    if TEST_ARCHIVE_DIR.exists():
        shutil.rmtree(TEST_ARCHIVE_DIR, ignore_errors=True)
    TEST_ARCHIVE_DIR.mkdir(parents=True, exist_ok=True)

    # ── Phase 1: Configuration & Defaults Validation ──────────────────────────
    config_yaml_path = PROJECT_ROOT / "config" / "config.yaml"
    env_example_path = PROJECT_ROOT / ".env.example"

    p1_ok = False
    p1_detail = ""
    try:
        cfg_text = config_yaml_path.read_text(encoding="utf-8")
        env_text = env_example_path.read_text(encoding="utf-8")

        has_cfg_block = (
            "pit_certificate_archive:" in cfg_text
            and "enabled: true" in cfg_text
            and ("provider: \"local\"" in cfg_text or "provider: local" in cfg_text)
            and ("bucket: \"fintext-pit-cert-archive\"" in cfg_text or "bucket: fintext-pit-cert-archive" in cfg_text)
            and ("local_path: \"data/pit-cert-archive\"" in cfg_text or "local_path: data/pit-cert-archive" in cfg_text)
        )
        has_env_docs = (
            "PIT_CERT_ARCHIVE_ENABLED" in env_text
            and "PIT_CERT_ARCHIVE_PROVIDER" in env_text
            and "PIT_CERT_ARCHIVE_BUCKET" in env_text
            and "PIT_CERT_ARCHIVE_LOCAL_PATH" in env_text
        )
        p1_ok = has_cfg_block and has_env_docs
        p1_detail = f"has_cfg_block={has_cfg_block}, has_env_docs={has_env_docs}"
    except Exception as e:
        p1_detail = str(e)
    report(1, "Configuration & Defaults Validation (config.yaml & .env.example)", p1_ok, p1_detail)

    # ── Phase 2: Native Rust Unit Tests for Canonicalization & Determinism ───
    p2_ok = False
    p2_detail = ""
    try:
        env = os.environ.copy()
        env["LIBCLANG_PATH"] = str(PROJECT_ROOT / "venv" / "Lib" / "site-packages" / "clang" / "native")
        res = subprocess.run(
            ["cargo", "test", "--manifest-path", "rust/Cargo.toml", "-p", "fintext_api_server", "--lib", "pit_archive"],
            cwd=str(PROJECT_ROOT),
            env=env,
            capture_output=True,
            text=True,
            timeout=90,
        )
        p2_ok = (res.returncode == 0) and ("4 passed" in res.stdout or "ok" in res.stdout)
        p2_detail = f"exit_code={res.returncode}"
    except Exception as e:
        p2_detail = str(e)
    report(2, "Native Rust Unit Tests for Canonicalization, Pre-image & Atomic Write", p2_ok, p2_detail)

    # ── Phase 3-8: Live Server Archival & Byte-Exact Verification ─────────────
    server_env = {
        "PIT_CERT_ARCHIVE_LOCAL_PATH": str(TEST_ARCHIVE_DIR).replace("\\", "/"),
        "PIT_CERT_ARCHIVE_ENABLED": "true",
        "PIT_CERT_ARCHIVE_PROVIDER": "local",
    }

    with ServerContext(PORT, server_env):
        report(3, f"Live API Server Startup & Health Probe on Port {PORT}", True)

        client = httpx.Client(base_url=BASE_URL, timeout=15.0)

        # Obtain valid authentication token
        login_resp = client.post(
            "/auth/token",
            json={"user_id": "auditor_sec_01", "role": "compliance"},
            headers={"X-Admin-Token": ADMIN_TOKEN},
        )
        token = login_resp.json().get("token")
        auth_headers = {"Authorization": f"Bearer {token}"}

        # ── Phase 4: GET /pit/certificate Returns 200 with Archive Fields ─────
        r = client.get(
            "/pit/certificate",
            params={
                "dataset_version": "2.1.0",
                "universe": "all",
                "start_date": "2025-06-01",
                "end_date": "2025-08-31",
            },
            headers=auth_headers,
        )
        data = r.json()
        p4_ok = (
            r.status_code == 200
            and data.get("overall_result") == "pass"
            and data.get("signature", "").startswith("sha256:")
            and data.get("archive_object_key") is not None
            and data.get("archive_timestamp") is not None
        )
        report(
            4,
            "GET /pit/certificate returns 200 OK with archive_object_key and archive_timestamp",
            p4_ok,
            f"status={r.status_code}, key={data.get('archive_object_key')}, ts={data.get('archive_timestamp')}",
        )

        # ── Phase 5: Confirm Archived Proof File Exists on Disk ───────────────
        object_key = data.get("archive_object_key", "")
        # The key is like "pit-cert/pit-cert-2.1.0-all-2025-06-01-2025-08-31-<uuid>.json"
        filename = object_key.split("/")[-1] if "/" in object_key else object_key
        archived_file_path = TEST_ARCHIVE_DIR / filename

        p5_ok = (
            archived_file_path.exists()
            and archived_file_path.is_file()
            and archived_file_path.stat().st_size > 100
        )
        report(
            5,
            f"Archived Proof File Exists on Disk ({filename})",
            p5_ok,
            f"path={archived_file_path}, exists={archived_file_path.exists()}, size={archived_file_path.stat().st_size if archived_file_path.exists() else 0}",
        )

        # ── Phase 6: Byte-Exact Cryptographic Hash Reproducibility ────────────
        # Read exact raw bytes directly from the file written to disk
        raw_bytes = archived_file_path.read_bytes()
        calculated_sha256 = hashlib.sha256(raw_bytes).hexdigest()
        expected_sig = f"sha256:{calculated_sha256}"
        actual_sig = data.get("signature", "")

        p6_ok = (actual_sig == expected_sig) and (len(calculated_sha256) == 64)
        report(
            6,
            "Byte-Exact Cryptographic Hash Reproducibility: sha256(raw_bytes) == cert.signature",
            p6_ok,
            f"actual={actual_sig} vs expected={expected_sig}",
        )

        # ── Phase 7: Canonical JSON Alphabetical Key Ordering ─────────────────
        # In canonical JSON, all object keys must be sorted alphabetically at every depth
        raw_str = raw_bytes.decode("utf-8")
        parsed_json = json.loads(raw_str)

        def verify_keys_sorted(obj) -> bool:
            if isinstance(obj, dict):
                keys = list(obj.keys())
                if keys != sorted(keys):
                    return False
                return all(verify_keys_sorted(v) for v in obj.values())
            elif isinstance(obj, list):
                return all(verify_keys_sorted(item) for item in obj)
            return True

        p7_sorted = verify_keys_sorted(parsed_json)
        top_keys = list(parsed_json.keys())
        expected_top_keys = sorted([
            "dataset_version",
            "universe",
            "audit_start_date",
            "audit_end_date",
            "overall_result",
            "tests",
            "policies",
        ])
        p7_ok = p7_sorted and (top_keys == expected_top_keys)
        report(
            7,
            "Canonical JSON Alphabetical Key Ordering & Deterministic Formatting",
            p7_ok,
            f"top_keys={top_keys}, strictly_sorted={p7_sorted}",
        )

        # ── Phase 8: Multi-Request Parametric Archival ────────────────────────
        r2 = client.get(
            "/pit/certificate",
            params={
                "dataset_version": "2.2.0-rc1",
                "universe": "AAPL,MSFT,NVDA",
                "start_date": "2025-01-01",
                "end_date": "2025-03-31",
            },
            headers=auth_headers,
        )
        data2 = r2.json()
        key2 = data2.get("archive_object_key", "")
        fn2 = key2.split("/")[-1]
        file2_path = TEST_ARCHIVE_DIR / fn2

        p8_ok = False
        p8_detail = ""
        if file2_path.exists() and file2_path != archived_file_path:
            raw2 = file2_path.read_bytes()
            hash2 = f"sha256:{hashlib.sha256(raw2).hexdigest()}"
            p8_ok = (
                data2.get("signature") == hash2
                and "2.2.0-rc1" in fn2
                and "AAPL_MSFT_NVDA" in fn2
                and data2.get("signature") != actual_sig
            )
            p8_detail = f"Distinct archive created: {fn2}, sha256 matches returned signature"
        else:
            p8_detail = f"File {file2_path} missing or identical"
        report(8, "Multi-Request Parametric Archival (Distinct Versions, Universes, UUIDs)", p8_ok, p8_detail)

        # ── Phase 10: Python SDK Sync & Async Client Model Validation ─────────
        from fintext import FinTextClient, FinTextAsyncClient, PITCertificateResponse

        sdk_client = FinTextClient(base_url=BASE_URL, api_token=token)
        sync_cert = sdk_client.pit_certificate(
            dataset_version="2.1.0",
            universe="all",
            start_date="2025-06-01",
            end_date="2025-08-31",
        )

        async def verify_async():
            async_c = FinTextAsyncClient(base_url=BASE_URL, api_token=token)
            async_cert = await async_c.pit_certificate(dataset_version="2.1.0", universe="sp500")
            await async_c.close()
            return async_cert

        async_cert = asyncio.run(verify_async())

        p10_ok = (
            isinstance(sync_cert, PITCertificateResponse)
            and sync_cert.archive_object_key is not None
            and sync_cert.archive_timestamp is not None
            and isinstance(async_cert, PITCertificateResponse)
            and async_cert.archive_object_key is not None
            and async_cert.archive_timestamp is not None
        )
        report(
            10,
            "Python SDK Sync & Async Client Model Validation with Archive Fields",
            p10_ok,
            f"sync_key={sync_cert.archive_object_key}, async_key={async_cert.archive_object_key}",
        )

        # ── Phase 11: OpenAPI 3.0 Documentation & Specification Registration ──
        r_docs = client.get("/api-docs/openapi.json")
        p11_ok = False
        p11_detail = ""
        if r_docs.status_code == 200:
            docs = r_docs.json()
            schemas = docs.get("components", {}).get("schemas", {})
            cert_schema = schemas.get("PITCertificateResponse", {})
            props = cert_schema.get("properties", {})

            has_archive_key = "archive_object_key" in props
            has_archive_ts = "archive_timestamp" in props
            p11_ok = has_archive_key and has_archive_ts
            p11_detail = f"archive_object_key in schema: {has_archive_key}, archive_timestamp in schema: {has_archive_ts}"
        else:
            p11_detail = f"status={r_docs.status_code}"
        report(11, "OpenAPI 3.0 Schema Registration & Documentation Audit", p11_ok, p11_detail)

    # ── Phase 9: Verification of Disabled Archival Mode ───────────────────────
    disabled_env = {
        "PIT_CERT_ARCHIVE_ENABLED": "false",
    }
    with ServerContext(PORT_DISABLED, disabled_env):
        client_dis = httpx.Client(base_url=BASE_URL_DISABLED, timeout=15.0)
        login_res = client_dis.post(
            "/auth/token",
            json={"user_id": "auditor_sec_02", "role": "compliance"},
            headers={"X-Admin-Token": ADMIN_TOKEN},
        )
        tok_dis = login_res.json().get("token")

        r_dis = client_dis.get(
            "/pit/certificate",
            headers={"Authorization": f"Bearer {tok_dis}"},
        )
        data_dis = r_dis.json()
        p9_ok = (
            r_dis.status_code == 200
            and data_dis.get("overall_result") == "pass"
            and data_dis.get("signature", "").startswith("sha256:")
            and data_dis.get("archive_object_key") is None
            and data_dis.get("archive_timestamp") is None
        )
        report(
            9,
            "Disabled Archival Mode Graceful Fallback (PIT_CERT_ARCHIVE_ENABLED=false)",
            p9_ok,
            f"status={r_dis.status_code}, key={data_dis.get('archive_object_key')}, sig={data_dis.get('signature')}",
        )

    # Cleanup temporary test directory
    if TEST_ARCHIVE_DIR.exists():
        shutil.rmtree(TEST_ARCHIVE_DIR, ignore_errors=True)


if __name__ == "__main__":
    run_all_phases()
    print("=" * 80)
    print(f" Verification Suite #268 Results: {passed}/{total} Phases Passed ({failed} Failed)")
    print("=" * 80)
    if failed > 0:
        sys.exit(1)
    sys.exit(0)
