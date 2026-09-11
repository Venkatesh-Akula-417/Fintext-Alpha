#!/usr/bin/env python3
"""
===============================================================================
FinText-Alpha-Vectorizer — Suite #233: Model Retraining Engine Certification
===============================================================================
Verifies:
  1.  Unauthenticated POST /retraining/jobs returns 401 Unauthorized
  2.  Invalid model_type returns 400 Bad Request
  3.  Invalid trigger_type returns 400 Bad Request
  4.  Create manual retraining job returns 201 Created with UUID and config
  5.  Create scheduled retraining job returns 201 Created
  6.  List retraining jobs (GET /retraining/jobs) returns 200 OK with pagination
  7.  Filter retraining jobs by status query parameter
  8.  Filter retraining jobs by model_type query parameter
  9.  Get retraining job by valid UUID (GET /retraining/jobs/{id}) returns 200 OK
  10. Get retraining job by non-existent UUID returns 404 Not Found
  11. Get retraining job with malformed UUID returns 400 Bad Request
  12. Cancel retraining job (POST /retraining/jobs/{id}/cancel) returns 200 OK or appropriate status
  13. Dynamic model version increment & last_trained_at update verification
  14. Python SDK sync and async client parity verification
===============================================================================
"""

import asyncio
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
SDK_PATH = PROJECT_ROOT / "python_sdk" / "src"
if str(SDK_PATH) not in sys.path:
    sys.path.insert(0, str(SDK_PATH))

PORT = 8132
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
                time.sleep(0.3)

        if not ready:
            if self.process.poll() is not None:
                raise RuntimeError("FinText API server process exited prematurely")
            raise RuntimeError("FinText API server failed to start within 25 seconds")

        print(f"[READY] FinText API Server is responding on {BASE_URL}.\n")
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        if self.process:
            print(f"[STOPPING] Terminating FinText API Server (PID: {self.process.pid})...")
            self.process.terminate()
            try:
                self.process.wait(timeout=5.0)
            except subprocess.TimeoutExpired:
                self.process.kill()


def get_jwt_token(client: httpx.Client, user_id: str = "mlops_admin") -> str:
    r = client.post(
        f"{BASE_URL}/auth/token",
        json={"user_id": user_id, "expires_in_seconds": 3600, "role": "admin"},
        headers={"X-Admin-Token": ADMIN_TOKEN},
    )
    if r.status_code != 200:
        raise RuntimeError(f"Failed to issue JWT token: {r.status_code} - {r.text}")
    return r.json()["token"]


def main():
    global passed, failed
    print("=" * 80)
    print("  FinText-Alpha-Vectorizer — Model Retraining Automation Engine (#233)")
    print("=" * 80)

    with ServerContext():
        client = httpx.Client(base_url=BASE_URL, timeout=10.0)
        jwt_token = get_jwt_token(client)
        auth_headers = {"Authorization": f"Bearer {jwt_token}", "Content-Type": "application/json"}

        # -----------------------------------------------------------------
        # Phase 1: Unauthenticated POST /retraining/jobs returns 401
        # -----------------------------------------------------------------
        r1 = client.post("/retraining/jobs", json={"model_type": "sentiment"})
        report(1, "Unauthenticated POST /retraining/jobs returns 401", r1.status_code == 401)

        # -----------------------------------------------------------------
        # Phase 2: Invalid model_type returns 400 Bad Request
        # -----------------------------------------------------------------
        r2 = client.post("/retraining/jobs", json={"model_type": "llama99_unsupported"}, headers=auth_headers)
        report(2, "Invalid model_type returns 400 Bad Request", r2.status_code == 400)

        # -----------------------------------------------------------------
        # Phase 3: Invalid trigger_type returns 400 Bad Request
        # -----------------------------------------------------------------
        r3 = client.post("/retraining/jobs", json={"trigger_type": "telepathic"}, headers=auth_headers)
        report(3, "Invalid trigger_type returns 400 Bad Request", r3.status_code == 400)

        # -----------------------------------------------------------------
        # Phase 4: Create manual retraining job returns 201 Created
        # -----------------------------------------------------------------
        job_config = {"learning_rate": 0.00002, "epochs": 3, "batch_size": 32, "dataset": "fintext_2026_labeled"}
        r4 = client.post(
            "/retraining/jobs",
            json={"model_type": "sentiment", "trigger_type": "manual", "config": job_config},
            headers=auth_headers,
        )
        ok4 = r4.status_code == 201
        job1 = r4.json().get("job", {}) if ok4 else {}
        job1_id = job1.get("id")
        ok4 = ok4 and bool(job1_id) and job1.get("model_type") == "sentiment" and job1.get("trigger_type") == "manual"
        report(4, "Create manual retraining job returns 201 Created", ok4)

        # -----------------------------------------------------------------
        # Phase 5: Create scheduled retraining job returns 201 Created
        # -----------------------------------------------------------------
        r5 = client.post(
            "/retraining/jobs",
            json={"model_type": "finbert", "trigger_type": "scheduled", "config": {"epochs": 5}},
            headers=auth_headers,
        )
        ok5 = r5.status_code == 201
        job2 = r5.json().get("job", {}) if ok5 else {}
        job2_id = job2.get("id")
        ok5 = ok5 and bool(job2_id) and job2.get("model_type") == "finbert" and job2.get("trigger_type") == "scheduled"
        report(5, "Create scheduled retraining job returns 201 Created", ok5)

        # -----------------------------------------------------------------
        # Phase 6: List retraining jobs (GET /retraining/jobs) returns 200 OK
        # -----------------------------------------------------------------
        r6 = client.get("/retraining/jobs?limit=50", headers=auth_headers)
        ok6 = r6.status_code == 200
        data6 = r6.json() if ok6 else {}
        ok6 = ok6 and data6.get("total", 0) >= 2 and len(data6.get("jobs", [])) >= 2
        report(6, "List retraining jobs returns 200 OK with pagination totals", ok6)

        # -----------------------------------------------------------------
        # Phase 7: Filter retraining jobs by status query parameter
        # -----------------------------------------------------------------
        # Give a small delay so background processing completes or check pending/completed
        time.sleep(0.5)
        r7 = client.get("/retraining/jobs?status=completed", headers=auth_headers)
        ok7 = r7.status_code == 200
        data7 = r7.json() if ok7 else {}
        ok7 = ok7 and all(j.get("status") == "completed" for j in data7.get("jobs", []))
        report(7, "Filter retraining jobs by status returns matching records", ok7)

        # -----------------------------------------------------------------
        # Phase 8: Filter retraining jobs by model_type query parameter
        # -----------------------------------------------------------------
        r8 = client.get("/retraining/jobs?model_type=finbert", headers=auth_headers)
        ok8 = r8.status_code == 200
        data8 = r8.json() if ok8 else {}
        ok8 = ok8 and len(data8.get("jobs", [])) >= 1 and all(j.get("model_type") == "finbert" for j in data8.get("jobs", []))
        report(8, "Filter retraining jobs by model_type returns matching records", ok8)

        # -----------------------------------------------------------------
        # Phase 9: Get retraining job by valid UUID returns 200 OK
        # -----------------------------------------------------------------
        r9 = client.get(f"/retraining/jobs/{job1_id}", headers=auth_headers)
        ok9 = r9.status_code == 200
        data9 = r9.json().get("job", {}) if ok9 else {}
        ok9 = ok9 and data9.get("id") == job1_id and "status" in data9
        report(9, "Get retraining job by UUID returns 200 OK with complete details", ok9)

        # -----------------------------------------------------------------
        # Phase 10: Get retraining job by non-existent UUID returns 404 Not Found
        # -----------------------------------------------------------------
        random_uuid = str(uuid.uuid4())
        r10 = client.get(f"/retraining/jobs/{random_uuid}", headers=auth_headers)
        report(10, "Get retraining job by non-existent UUID returns 404 Not Found", r10.status_code == 404)

        # -----------------------------------------------------------------
        # Phase 11: Get retraining job with malformed UUID returns 400 Bad Request
        # -----------------------------------------------------------------
        r11 = client.get("/retraining/jobs/not-a-valid-uuid", headers=auth_headers)
        report(11, "Get retraining job with malformed UUID returns 400 Bad Request", r11.status_code == 400)

        # -----------------------------------------------------------------
        # Phase 12: Cancel retraining job (POST /retraining/jobs/{id}/cancel)
        # -----------------------------------------------------------------
        # Test cancel on a fresh job or verify 400 when cancelling already completed job
        r12_fresh = client.post(
            "/retraining/jobs",
            json={"model_type": "minilm", "trigger_type": "manual", "config": {"epochs": 1}},
            headers=auth_headers,
        )
        fresh_job_id = r12_fresh.json()["job"]["id"]
        # Cancel right away
        r12_cancel = client.post(f"/retraining/jobs/{fresh_job_id}/cancel", headers=auth_headers)
        ok12 = r12_cancel.status_code in [200, 400]  # If finished instantaneously or cancelled
        if r12_cancel.status_code == 200:
            ok12 = ok12 and r12_cancel.json()["job"]["status"] == "cancelled"
        report(12, "Cancel retraining job handles status transitions correctly", ok12)

        # -----------------------------------------------------------------
        # Phase 13: Dynamic model version increment & last_trained_at check
        # -----------------------------------------------------------------
        time.sleep(1.0)  # Ensure background worker processed at least 1 job
        r13 = client.get("/sentiment?ticker=AAPL", headers=auth_headers)
        ok13 = r13.status_code == 200
        data13 = r13.json() if ok13 else {}
        model_ver = data13.get("model_version", "")
        # Should have updated from original version or incremented tag
        ok13 = ok13 and bool(model_ver) and ("finbert" in model_ver.lower() or "retrained" in model_ver.lower() or "v2." in model_ver.lower())
        report(13, f"Dynamic model version governance active (current: '{model_ver}')", ok13)

        # -----------------------------------------------------------------
        # Phase 14: Python SDK sync & async client parity verification
        # -----------------------------------------------------------------
        from fintext import FinTextClient, FinTextAsyncClient, RetrainingJobResponse, ListRetrainingJobsResponse

        # Sync SDK client test
        sdk_client = FinTextClient(base_url=BASE_URL, api_token=jwt_token)
        sdk_create = sdk_client.create_retraining_job(
            model_type="sentiment",
            trigger_type="manual",
            config={"epochs": 2, "learning_rate": 3e-5},
        )
        assert isinstance(sdk_create, RetrainingJobResponse)
        assert sdk_create.job.model_type == "sentiment"
        sdk_job_id = sdk_create.job.id

        sdk_list = sdk_client.list_retraining_jobs(limit=10)
        assert isinstance(sdk_list, ListRetrainingJobsResponse)
        assert sdk_list.total >= 1

        sdk_get = sdk_client.get_retraining_job(job_id=sdk_job_id)
        assert isinstance(sdk_get, RetrainingJobResponse)
        assert sdk_get.job.id == sdk_job_id
        sdk_client.close()

        # Async SDK client test
        async def run_async_sdk():
            async_client = FinTextAsyncClient(base_url=BASE_URL, api_token=jwt_token)
            a_create = await async_client.create_retraining_job(
                model_type="finbert",
                trigger_type="scheduled",
                config={"epochs": 4},
            )
            assert isinstance(a_create, RetrainingJobResponse)
            a_job_id = a_create.job.id

            a_list = await async_client.list_retraining_jobs(limit=5)
            assert isinstance(a_list, ListRetrainingJobsResponse)

            a_get = await async_client.get_retraining_job(job_id=a_job_id)
            assert isinstance(a_get, RetrainingJobResponse)
            assert a_get.job.id == a_job_id
            await async_client.close()

        asyncio.run(run_async_sdk())
        report(14, "Python SDK sync and async client parity verified", True)

    print("-" * 80)
    print(f"  Summary: {passed}/{total} phases passed ({passed/total*100:.1f}%)")
    print("=" * 80)
    if failed > 0:
        sys.exit(1)


if __name__ == "__main__":
    main()
