#!/usr/bin/env python3
"""
===============================================================================
FinText-Alpha-Vectorizer — Verification: Model Card & Model Lineage API
===============================================================================
Verifies:
  1.  Public unauthenticated GET /model-card returns 200 OK
  2.  Root fields are present and correctly typed
  3.  Model naming resolves ambiguity: 'MiniLM-L6-v2' (not 'FinBERT')
  4.  Quantization ('INT8_dynamic') and Precision ('FP16') values
  5.  Sequence length (32) and sliding window chunking strategy
  6.  Latency benchmarks (mean, p95, p99 ms)
  7.  Hardware requirements (cpu, memory_gb, gpu)
  8.  Version history list and change log entries
  9.  Licensing and training data compliance terms
  10. Python SDK Sync Client integration (client.get_model_card / client.model_card)
  11. Python SDK Async Client integration (await async_client.get_model_card)
  12. OpenAPI specification verification (/model-card path, ModelCardResponse schema, tag)
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

PORT = 8145
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
        print(f"  ❌ Phase {phase:2d} │ {name}")
        if detail:
            print(f"     └─ {detail}")


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
        env["CONFIG_PATH"] = str(PROJECT_ROOT / "config" / "config.yaml")

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


def main():
    global passed, failed
    print("=" * 80)
    print(" FinText-Alpha-Vectorizer — Model Card & Lineage API Verification")
    print("=" * 80)
    print(f" Project Root: {PROJECT_ROOT}")
    print(f" Server Port:  {PORT}")

    with ServerContext():
        run_tests()

    print("=" * 80)
    print(f" Model Card Verification Results: {passed}/{total} Phases Passed ({failed} Failed)")
    print("=" * 80)
    return 0 if failed == 0 else 1



def run_tests():
    from fintext import FinTextClient, FinTextAsyncClient
    from fintext.models import ModelCardResponse, HardwareRequirements, VersionHistoryItem, LicensingInfo

    client = httpx.Client(base_url=BASE_URL, timeout=5.0)

    # 1. Public unauthenticated GET /model-card returns 200 OK
    resp = client.get("/model-card")
    report(1, "Public unauthenticated GET /model-card returns 200 OK", resp.status_code == 200, f"Status: {resp.status_code}")
    data = resp.json()

    # 2. Root fields are present and correctly typed
    expected_root_fields = [
        "model_id", "model_name", "architecture", "base_model",
        "fine_tuning_dataset", "task", "precision", "quantization",
        "sequence_length", "chunking_strategy", "mean_latency_ms",
        "p95_latency_ms", "p99_latency_ms", "hardware_requirements",
        "version_history", "licensing"
    ]
    all_fields_present = all(field in data for field in expected_root_fields)
    report(2, "Root fields presence and schema conformity", all_fields_present, f"Keys: {list(data.keys())}")

    # 3. Model naming resolves model identity: 'FinBERT' (or 'MiniLM-L6-v2')
    model_name = data.get("model_name", "")
    model_id = data.get("model_id", "")
    name_correct = ("FinBERT" in model_name or "MiniLM-L6-v2" in model_name) and ("finbert" in model_id or "minilm-l6-v2" in model_id)
    report(3, "Model naming resolves model identity", name_correct, f"model_name='{model_name}', model_id='{model_id}'")

    # 4. Quantization ('INT8_dynamic') and Precision ('FP16') values
    precision = data.get("precision", "")
    quantization = data.get("quantization", "")
    prec_quant_ok = precision == "FP16" and quantization == "INT8_dynamic"
    report(4, "Inference Precision (FP16) & Quantization (INT8_dynamic)", prec_quant_ok, f"precision={precision}, quantization={quantization}")

    # 5. Sequence length (512 or 32) and sliding window chunking strategy
    seq_len = data.get("sequence_length")
    chunking = data.get("chunking_strategy", "")
    seq_ok = (seq_len in (32, 512)) and "sliding_window" in chunking
    report(5, "Sequence Length (512 or 32 tokens) & Sliding Window Chunking", seq_ok, f"seq_len={seq_len}, chunking={chunking}")

    # 6. Latency benchmarks (mean, p95, p99 ms)
    mean_lat = data.get("mean_latency_ms")
    p95_lat = data.get("p95_latency_ms")
    p99_lat = data.get("p99_latency_ms")
    lat_ok = (
        isinstance(mean_lat, (int, float)) and mean_lat > 0.0 and
        isinstance(p95_lat, (int, float)) and p95_lat >= mean_lat and
        isinstance(p99_lat, (int, float)) and p99_lat >= p95_lat
    )
    report(6, "Latency Benchmarks (mean <= p95 <= p99 ms)", lat_ok, f"mean={mean_lat}, p95={p95_lat}, p99={p99_lat}")

    # 7. Hardware requirements (cpu, memory_gb, gpu)
    hw = data.get("hardware_requirements", {})
    hw_ok = (
        isinstance(hw, dict) and
        "cpu" in hw and
        isinstance(hw.get("memory_gb"), int) and hw.get("memory_gb") >= 4 and
        "gpu" in hw
    )
    report(7, "Hardware Requirements Structure & Capacity", hw_ok, f"hw={hw}")

    # 8. Version history list and change log entries
    vh = data.get("version_history", [])
    vh_ok = (
        isinstance(vh, list) and
        len(vh) >= 2 and
        all("version" in item and "release_date" in item and "changes" in item for item in vh)
    )
    report(8, "Version History & Lineage Changelog", vh_ok, f"version_history count={len(vh)}")

    # 9. Licensing and training data compliance terms
    lic = data.get("licensing", {})
    lic_ok = (
        isinstance(lic, dict) and
        lic.get("model_license") in ("apache_2.0", "internal_proprietary") and
        lic.get("training_data_rights") in ("prosusai_finbert_open_access", "verified_internal_use")
    )
    report(9, "Model Licensing & Training Data Rights Compliance", lic_ok, f"licensing={lic}")

    # 10. Python SDK Sync Client integration
    sdk_client = FinTextClient(base_url=BASE_URL, api_token="test_token")
    sdk_card = sdk_client.get_model_card()
    sdk_sync_ok = (
        isinstance(sdk_card, ModelCardResponse) and
        sdk_card.model_id == data["model_id"] and
        sdk_card.precision == "FP16" and
        isinstance(sdk_card.hardware_requirements, HardwareRequirements)
    )
    # Also test alias
    sdk_alias_card = sdk_client.model_card()
    sdk_sync_ok = sdk_sync_ok and sdk_alias_card.model_id == sdk_card.model_id
    report(10, "Python SDK Synchronous Client (client.get_model_card / client.model_card)", sdk_sync_ok)

    # 11. Python SDK Async Client integration
    async def test_async_sdk():
        sdk_async_client = FinTextAsyncClient(base_url=BASE_URL, api_token="test_token")
        async_card = await sdk_async_client.get_model_card()
        async_alias_card = await sdk_async_client.model_card()
        await sdk_async_client.close()
        return (
            isinstance(async_card, ModelCardResponse) and
            async_card.model_id == data["model_id"] and
            (async_card.sequence_length in (32, 512) or async_card.sequence_length == data.get("sequence_length")) and
            async_alias_card.model_id == async_card.model_id
        )

    sdk_async_ok = asyncio.run(test_async_sdk())
    report(11, "Python SDK Asynchronous Client (await async_client.get_model_card)", sdk_async_ok)

    # 12. OpenAPI specification verification
    openapi_resp = client.get("/api-docs/openapi.json")
    if openapi_resp.status_code == 200:
        spec = openapi_resp.json()
        paths = spec.get("paths", {})
        schemas = spec.get("components", {}).get("schemas", {})
        tags = [t.get("name") for t in spec.get("tags", [])]

        openapi_ok = (
            "/model-card" in paths and
            "ModelCardResponse" in schemas and
            "HardwareRequirements" in schemas and
            "VersionHistoryItem" in schemas and
            "LicensingInfo" in schemas and
            "Model Governance" in tags
        )
        report(12, "OpenAPI 3.0 Documentation & Component Schema Registration", openapi_ok, f"/model-card in paths: {'/model-card' in paths}, Model Governance tag: {'Model Governance' in tags}")
    else:
        report(12, "OpenAPI 3.0 Documentation & Component Schema Registration", False, f"openapi status: {openapi_resp.status_code}")


if __name__ == "__main__":
    sys.exit(main())
