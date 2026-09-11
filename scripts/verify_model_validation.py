#!/usr/bin/env python3
"""
===============================================================================
FinText-Alpha-Vectorizer — Verification Suite #257:
FinBERT Model Validation & Probability Calibration Suite
===============================================================================
Verifies:
  1.  Unauthorized GET /model-validation returns 401 Unauthorized
  2.  Authorized GET /model-validation returns 200 OK with full JSON payload
  3.  Root metadata (model_id, model_version, dataset_version, dataset_size >= 100, evaluated_at)
  4.  Overall classification metrics (accuracy >= 0.80, macro_f1 >= 0.80, precision/recall)
  5.  Per-class metrics (positive, negative, neutral PRF scores and class supports)
  6.  Confusion matrix 3x3 structure, conservation of sample counts, and diagonal consistency
  7.  Probability calibration curve deciles (10 bins spanning [0.0, 1.0])
  8.  Calibration metrics (Expected Calibration Error <= 0.15, Brier score <= 0.25)
  9.  Query parameter handling (?recalibrate=true&dataset_version=1.0.0)
  10. Python SDK Sync Client integration (client.get_model_validation & client.model_validation)
  11. Python SDK Async Client integration (async_client.get_model_validation & client.model_validation)
  12. OpenAPI 3.0 schema verification (/model-validation under 'Model Governance' tag)
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

PORT = 8146
BASE_URL = f"http://127.0.0.1:{PORT}"
ADMIN_TOKEN = "test_admin_token_xyz123_valid_32_bytes_length!"
JWT_SECRET = "super_secret_test_jwt_key_32_bytes_len!!"
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
        env["JWT_SECRET"] = JWT_SECRET
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
    print(" Suite #257: FinBERT Model Validation & Calibration Suite Verification")
    print("=" * 80)
    print(f" Project Root: {PROJECT_ROOT}")
    print(f" Server Port:  {PORT}")

    with ServerContext():
        run_tests()

    print("=" * 80)
    print(f" Model Validation Suite Results: {passed}/{total} Phases Passed ({failed} Failed)")
    print("=" * 80)
    return 0 if failed == 0 else 1


def run_tests():
    from fintext import FinTextClient, FinTextAsyncClient
    from fintext.models import ModelValidationResponse, ClassificationMetrics, ConfusionMatrix, CalibrationPoint

    client = httpx.Client(base_url=BASE_URL, timeout=5.0)

    # 0. Obtain JWT auth token
    auth_resp = client.post(
        "/auth/token",
        json={
            "user_id": "model_validation_auditor",
            "role": "institutional",
            "duration_seconds": 3600,
        },
        headers={"X-Admin-Token": ADMIN_TOKEN},
    )
    if auth_resp.status_code == 200:
        valid_token = auth_resp.json().get("token")
    else:
        valid_token = "mock_jwt_token"
    headers = {"Authorization": f"Bearer {valid_token}"}

    # 1. Unauthorized GET /model-validation returns 401 Unauthorized
    resp_unauth = client.get("/model-validation")
    report(
        1,
        "Unauthorized GET /model-validation returns 401 Unauthorized",
        resp_unauth.status_code == 401,
        f"Status: {resp_unauth.status_code}, Body: {resp_unauth.text[:100]}",
    )

    # 2. Authorized GET /model-validation returns 200 OK with full JSON payload
    resp = client.get("/model-validation", headers=headers)
    report(
        2,
        "Authorized GET /model-validation returns 200 OK",
        resp.status_code == 200,
        f"Status: {resp.status_code}, Body: {resp.text[:100]}",
    )
    data = resp.json()

    # 3. Root metadata verification
    root_fields = ["model_id", "model_version", "dataset_version", "dataset_size", "evaluated_at", "metrics", "confusion_matrix", "calibration_curve", "brier_score", "expected_calibration_error"]
    has_root_fields = all(k in data for k in root_fields)
    valid_model_id = "finbert" in data.get("model_id", "").lower()
    valid_dataset_size = data.get("dataset_size", 0) >= 100
    report(
        3,
        "Root metadata validation (model_id, dataset_size >= 100, evaluated_at)",
        has_root_fields and valid_model_id and valid_dataset_size,
        f"model_id={data.get('model_id')}, dataset_size={data.get('dataset_size')}",
    )

    # 4. Overall classification metrics
    metrics = data.get("metrics", {})
    acc = metrics.get("accuracy", 0.0)
    macro_f1 = metrics.get("macro_f1", 0.0)
    macro_p = metrics.get("macro_precision", 0.0)
    macro_r = metrics.get("macro_recall", 0.0)
    metrics_ok = acc >= 0.80 and macro_f1 >= 0.80 and macro_p >= 0.70 and macro_r >= 0.70
    report(
        4,
        f"Overall classification metrics (Accuracy={acc:.1%}, Macro-F1={macro_f1:.3f} >= 0.80)",
        metrics_ok,
        f"acc={acc}, macro_f1={macro_f1}, macro_p={macro_p}, macro_r={macro_r}",
    )

    # 5. Per-class metrics
    pos = metrics.get("positive", {})
    neg = metrics.get("negative", {})
    neu = metrics.get("neutral", {})
    per_class_ok = (
        pos.get("support", 0) >= 40
        and neg.get("support", 0) >= 40
        and neu.get("support", 0) >= 20
        and pos.get("f1_score", 0.0) >= 0.75
        and neg.get("f1_score", 0.0) >= 0.75
        and neu.get("f1_score", 0.0) >= 0.60
    )
    report(
        5,
        "Per-class PRF metrics (Pos, Neg, Neu supports and F1 scores)",
        per_class_ok,
        f"Pos(sup={pos.get('support')}, f1={pos.get('f1_score')}), Neg(sup={neg.get('support')}, f1={neg.get('f1_score')}), Neu(sup={neu.get('support')}, f1={neu.get('f1_score')})",
    )

    # 6. Confusion matrix 3x3 structure and conservation
    cm = data.get("confusion_matrix", {})
    tp = cm.get("true_positive", {})
    tn = cm.get("true_negative", {})
    tneu = cm.get("true_neutral", {})
    total_cm_samples = (
        tp.get("predicted_positive", 0) + tp.get("predicted_negative", 0) + tp.get("predicted_neutral", 0) +
        tn.get("predicted_positive", 0) + tn.get("predicted_negative", 0) + tn.get("predicted_neutral", 0) +
        tneu.get("predicted_positive", 0) + tneu.get("predicted_negative", 0) + tneu.get("predicted_neutral", 0)
    )
    diagonal_correct = (
        tp.get("predicted_positive", 0) +
        tn.get("predicted_negative", 0) +
        tneu.get("predicted_neutral", 0)
    )
    cm_ok = (total_cm_samples == data.get("dataset_size")) and (diagonal_correct > 0)
    report(
        6,
        f"Confusion matrix 3x3 consistency (total={total_cm_samples}, diagonal correct={diagonal_correct})",
        cm_ok,
        f"Total samples: {total_cm_samples}, expected: {data.get('dataset_size')}",
    )

    # 7. Probability calibration curve deciles
    curve = data.get("calibration_curve", [])
    has_10_bins = len(curve) == 10
    bin_indices = [pt.get("bin_index") for pt in curve] == list(range(10))
    monotonic_mins = all(curve[i].get("confidence_min", 0) <= curve[i+1].get("confidence_min", 0) for i in range(len(curve)-1)) if len(curve) == 10 else False
    curve_ok = has_10_bins and bin_indices and monotonic_mins
    report(
        7,
        "Probability calibration curve (10 decile bins [0.0, 1.0])",
        curve_ok,
        f"Bin count: {len(curve)}, indices: {bin_indices}",
    )

    # 8. Calibration metrics (ECE <= 0.15, Brier score <= 0.25)
    ece = data.get("expected_calibration_error", 1.0)
    brier = data.get("brier_score", 1.0)
    calib_metrics_ok = ece <= 0.15 and brier <= 0.25
    report(
        8,
        f"Calibration quality metrics (ECE={ece:.4f} <= 0.15, Brier={brier:.4f} <= 0.25)",
        calib_metrics_ok,
        f"ECE={ece}, Brier={brier}",
    )

    # 9. Query parameters handling (?recalibrate=true&dataset_version=1.0.0)
    resp_query = client.get("/model-validation?recalibrate=true&dataset_version=1.0.0", headers=headers)
    query_ok = resp_query.status_code == 200 and resp_query.json().get("dataset_version") == "1.0.0"
    report(
        9,
        "Query parameter handling (?recalibrate=true&dataset_version=1.0.0)",
        query_ok,
        f"Status: {resp_query.status_code}, version: {resp_query.json().get('dataset_version')}",
    )

    # 10. Python SDK Sync Client integration
    sdk_sync_client = FinTextClient(base_url=BASE_URL, api_token=valid_token)
    sdk_resp = sdk_sync_client.get_model_validation()
    sdk_alias_resp = sdk_sync_client.model_validation(recalibrate=True)
    sdk_sync_ok = (
        isinstance(sdk_resp, ModelValidationResponse)
        and sdk_resp.metrics.accuracy >= 0.80
        and isinstance(sdk_alias_resp, ModelValidationResponse)
    )
    report(
        10,
        "Python SDK Sync Client integration (client.get_model_validation & client.model_validation)",
        sdk_sync_ok,
        f"SDK accuracy={sdk_resp.metrics.accuracy}",
    )

    # 11. Python SDK Async Client integration
    async def test_async_sdk():
        sdk_async_client = FinTextAsyncClient(base_url=BASE_URL, api_token=valid_token)
        async_resp = await sdk_async_client.get_model_validation()
        async_alias_resp = await sdk_async_client.model_validation()
        return (
            isinstance(async_resp, ModelValidationResponse)
            and async_resp.metrics.macro_f1 >= 0.80
            and isinstance(async_alias_resp, ModelValidationResponse)
        )

    try:
        sdk_async_ok = asyncio.run(test_async_sdk())
    except Exception as e:
        sdk_async_ok = False
        print(f"Async error: {e}")

    report(
        11,
        "Python SDK Async Client integration (async_client.get_model_validation & client.model_validation)",
        sdk_async_ok,
    )

    # 12. OpenAPI 3.0 specification verification
    resp_openapi = client.get("/api-docs/openapi.json")
    openapi_ok = False
    detail = ""
    if resp_openapi.status_code == 200:
        spec = resp_openapi.json()
        paths = spec.get("paths", {})
        mv_path = paths.get("/model-validation", {}).get("get", {})
        has_path = bool(mv_path)
        tags = mv_path.get("tags", [])
        has_tag = "Model Governance" in tags
        schemas = spec.get("components", {}).get("schemas", {})
        has_schema = "ModelValidationResponse" in schemas
        openapi_ok = has_path and has_tag and has_schema
        detail = f"path={has_path}, tag={has_tag}, schema={has_schema}"
    else:
        detail = f"Status: {resp_openapi.status_code}"

    report(
        12,
        "OpenAPI 3.0 specification verification (/model-validation under 'Model Governance')",
        openapi_ok,
        detail,
    )


if __name__ == "__main__":
    sys.exit(main())
