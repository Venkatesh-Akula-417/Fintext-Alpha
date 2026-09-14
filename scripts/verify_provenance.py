#!/usr/bin/env python3
"""
===============================================================================
FinText-Alpha-Vectorizer — Suite #238: Data Lineage & Provenance Tracking Engine
===============================================================================
Verifies:
  1.  Unauthenticated GET /provenance/sentiment/AAPL_2026-08-30 returns 401 Unauthorized
  2.  Invalid record_type (GET /provenance/invalid_type/12345) returns 400 Bad Request
  3.  Case-insensitive record_type parsing (GET /provenance/SENTIMENT/AAPL_2026-08-30) returns 200 OK
  4.  Query sentiment record provenance (record_type = "sentiment", record_id = "AAPL_2026-08-30T10:15:00Z")
  5.  Sentiment processing steps validation (5 ordered steps: fetch_article, clean_text, tokenize, infer_sentiment, quality_audit)
  6.  Query news record provenance (record_type = "news", record_id = "doc-sec-edgar-8k-2026-001")
  7.  News processing steps validation (5 ordered steps: fetch_source, deduplication, entity_extraction, sentiment_tagging, publish_index)
  8.  Deterministic reproducibility verification across multiple requests
  9.  Data Quality Score boundary and validity check (0.0 <= score <= 1.0)
  10. Preloaded mock seed data registry lookup
  11. Python SDK Sync Client integration (client.get_provenance) returning DataProvenanceResponse
  12. Python SDK Async Client integration (async_client.get_provenance) returning DataProvenanceResponse
  13. Python SDK client-side validation errors (empty record_type, empty record_id, invalid record_type)
  14. OpenAPI specification verification (/provenance/* paths, schemas, tag "Data Lineage & Governance")
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

PORT = 8137
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
        env["CHAT_ALERTS_MOCK"] = "1"

        if not SERVER_EXE.exists():
            raise RuntimeError(f"Server binary not found at {SERVER_EXE}. Run `cargo build -p fintext_api_server` first.")

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
            raise TimeoutError(f"FinText API server failed to respond within 25 seconds.")

        print(f"[READY] API Server is healthy at {BASE_URL}\n")
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        if self.process:
            print("\n[STOPPING] Terminating FinText API Server process...")
            self.process.terminate()
            try:
                self.process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.process.kill()
            print("[STOPPING] Server terminated cleanly.")


def get_auth_token(user_id: str = "provenance_test_user", role: str = "institutional") -> str:
    payload = {"user_id": user_id, "role": role, "expiry_seconds": 3600}
    headers = {"X-Admin-Token": ADMIN_TOKEN}
    r = httpx.post(f"{BASE_URL}/auth/token", json=payload, headers=headers, timeout=5.0)
    if r.status_code != 200:
        raise RuntimeError(f"Failed to obtain auth token: {r.status_code} - {r.text}")
    return r.json()["token"]


def main():
    global passed, failed
    print("=" * 80)
    print("Suite #238: Data Lineage & Provenance Tracking Engine Verification")
    print("=" * 80)

    with ServerContext():
        token = get_auth_token()
        headers = {"Authorization": f"Bearer {token}"}

        # ---------------------------------------------------------------------
        # Phase 1: Unauthenticated request rejected with 401
        # ---------------------------------------------------------------------
        r = httpx.get(f"{BASE_URL}/provenance/sentiment/AAPL_2026-08-30", timeout=5.0)
        report(1, "Unauthenticated GET /provenance/sentiment/:id returns 401", r.status_code == 401)

        # ---------------------------------------------------------------------
        # Phase 2: Invalid record_type returns 400 Bad Request
        # ---------------------------------------------------------------------
        r = httpx.get(f"{BASE_URL}/provenance/invalid_type/AAPL_2026-08-30", headers=headers, timeout=5.0)
        report(2, "Invalid record_type returns 400 Bad Request", r.status_code == 400 and "Invalid record_type" in r.text)

        # ---------------------------------------------------------------------
        # Phase 3: Case-insensitive record_type parsing
        # ---------------------------------------------------------------------
        r = httpx.get(f"{BASE_URL}/provenance/SENTIMENT/AAPL_2026-08-30T10:15:00Z", headers=headers, timeout=5.0)
        report(3, "Case-insensitive record_type ('SENTIMENT') returns 200 OK", r.status_code == 200 and r.json().get("record_type") == "sentiment")

        # ---------------------------------------------------------------------
        # Phase 4: Sentiment provenance trace validation
        # ---------------------------------------------------------------------
        r = httpx.get(f"{BASE_URL}/provenance/sentiment/AAPL_2026-08-30T10:15:00Z", headers=headers, timeout=5.0)
        data = r.json() if r.status_code == 200 else {}
        entries = data.get("provenance_entries", [])
        ok_sent = (
            r.status_code == 200
            and data.get("record_type") == "sentiment"
            and data.get("record_id") == "AAPL_2026-08-30T10:15:00Z"
            and len(entries) >= 1
            and entries[0].get("source_type") == "finnhub"
            and entries[0].get("model_version") == "finbert-v3.1.0"
            and entries[0].get("pipeline_version") == "2.0.0"
            and entries[0].get("data_quality_score") is not None
        )
        report(4, "Sentiment provenance record metadata & models validation", ok_sent, f"Status: {r.status_code}, entries: {len(entries)}")

        # ---------------------------------------------------------------------
        # Phase 5: Sentiment processing steps sequence
        # ---------------------------------------------------------------------
        steps = entries[0].get("processing_steps", []) if entries else []
        step_names = [s.get("step") for s in steps]
        expected_sent_steps = ["fetch_article", "clean_text", "tokenize", "infer_sentiment", "quality_audit"]
        ok_steps = (
            step_names == expected_sent_steps
            and all(s.get("timestamp") and s.get("description") for s in steps)
            and steps[3].get("details") is not None
        )
        report(5, "Sentiment processing steps sequence & timestamps validation", ok_steps, f"Steps: {step_names}")

        # ---------------------------------------------------------------------
        # Phase 6: News provenance trace validation
        # ---------------------------------------------------------------------
        r = httpx.get(f"{BASE_URL}/provenance/news/doc-sec-edgar-8k-2026-001", headers=headers, timeout=5.0)
        news_data = r.json() if r.status_code == 200 else {}
        news_entries = news_data.get("provenance_entries", [])
        ok_news = (
            r.status_code == 200
            and news_data.get("record_type") == "news"
            and news_data.get("record_id") == "doc-sec-edgar-8k-2026-001"
            and len(news_entries) >= 1
            and news_entries[0].get("source_type") == "sec_edgar"
            and bool(news_entries[0].get("model_version"))
            and bool(news_entries[0].get("pipeline_version"))
        )
        report(6, "News article provenance metadata & source origin validation", ok_news, f"Status: {r.status_code}, entries: {len(news_entries)}")

        # ---------------------------------------------------------------------
        # Phase 7: News processing steps sequence
        # ---------------------------------------------------------------------
        news_steps = news_entries[0].get("processing_steps", []) if news_entries else []
        news_step_names = [s.get("step") for s in news_steps]
        expected_news_steps = ["fetch_source", "deduplication", "entity_extraction", "sentiment_tagging", "publish_index"]
        ok_news_steps = (
            news_step_names == expected_news_steps
            and all(s.get("timestamp") and s.get("description") for s in news_steps)
            and news_steps[2].get("details") is not None
        )
        report(7, "News processing steps sequence & entity tagging validation", ok_news_steps, f"Steps: {news_step_names}")

        # ---------------------------------------------------------------------
        # Phase 8: Deterministic reproducibility
        # ---------------------------------------------------------------------
        r1 = httpx.get(f"{BASE_URL}/provenance/sentiment/MSFT_2026-08-30T16:00:00Z", headers=headers, timeout=5.0)
        r2 = httpx.get(f"{BASE_URL}/provenance/sentiment/MSFT_2026-08-30T16:00:00Z", headers=headers, timeout=5.0)
        d1 = r1.json().get("provenance_entries", [{}])[0]
        d2 = r2.json().get("provenance_entries", [{}])[0]
        ok_det = (
            r1.status_code == 200
            and r2.status_code == 200
            and d1.get("id") == d2.get("id")
            and d1.get("source_id") == d2.get("source_id")
            and d1.get("data_quality_score") == d2.get("data_quality_score")
        )
        report(8, "Deterministic reproducibility across repeated queries", ok_det, f"ID1: {d1.get('id')}, ID2: {d2.get('id')}")

        # ---------------------------------------------------------------------
        # Phase 9: Data quality scores validation
        # ---------------------------------------------------------------------
        q_score = d1.get("data_quality_score")
        ok_q = q_score is not None and 0.0 <= q_score <= 1.0
        report(9, "Data quality score validity (0.0 <= score <= 1.0)", ok_q, f"Quality Score: {q_score}")

        # ---------------------------------------------------------------------
        # Phase 10: Preloaded mock seed data registry lookup
        # ---------------------------------------------------------------------
        r_seed = httpx.get(f"{BASE_URL}/provenance/sentiment/NVDA_2026-08-30T14:30:00Z", headers=headers, timeout=5.0)
        seed_data = r_seed.json() if r_seed.status_code == 200 else {}
        ok_seed = (
            r_seed.status_code == 200
            and seed_data.get("record_id") == "NVDA_2026-08-30T14:30:00Z"
            and len(seed_data.get("provenance_entries", [])) >= 1
        )
        report(10, "Preloaded mock seed data registry lookup", ok_seed)

        # ---------------------------------------------------------------------
        # Phase 11: Python SDK Sync Client integration
        # ---------------------------------------------------------------------
        from fintext import FinTextClient, DataProvenanceResponse, DataProvenanceItem, ProcessingStep, FinTextValidationError

        sync_client = FinTextClient(base_url=BASE_URL, api_token=token)
        try:
            sdk_resp = sync_client.get_provenance("sentiment", "AAPL_2026-08-30T10:15:00Z")
            ok_sdk_sync = (
                isinstance(sdk_resp, DataProvenanceResponse)
                and sdk_resp.record_type == "sentiment"
                and sdk_resp.record_id == "AAPL_2026-08-30T10:15:00Z"
                and len(sdk_resp.provenance_entries) >= 1
                and isinstance(sdk_resp.provenance_entries[0], DataProvenanceItem)
                and len(sdk_resp.provenance_entries[0].processing_steps) == 5
                and isinstance(sdk_resp.provenance_entries[0].processing_steps[0], ProcessingStep)
            )
        finally:
            sync_client.close()
        report(11, "Python SDK Sync Client integration (client.get_provenance)", ok_sdk_sync)

        # ---------------------------------------------------------------------
        # Phase 12: Python SDK Async Client integration
        # ---------------------------------------------------------------------
        from fintext import FinTextAsyncClient

        async def test_async_sdk():
            async_client = FinTextAsyncClient(base_url=BASE_URL, api_token=token)
            try:
                resp = await async_client.get_provenance("news", "doc-sec-edgar-8k-2026-001")
                return (
                    isinstance(resp, DataProvenanceResponse)
                    and resp.record_type == "news"
                    and resp.record_id == "doc-sec-edgar-8k-2026-001"
                    and len(resp.provenance_entries) >= 1
                    and resp.provenance_entries[0].source_type == "sec_edgar"
                )
            finally:
                await async_client.close()

        ok_sdk_async = asyncio.run(test_async_sdk())
        report(12, "Python SDK Async Client integration (async_client.get_provenance)", ok_sdk_async)

        # ---------------------------------------------------------------------
        # Phase 13: Python SDK client-side validation errors
        # ---------------------------------------------------------------------
        val_err_caught = 0
        cli_temp = FinTextClient(base_url=BASE_URL, api_token=token)
        try:
            try:
                cli_temp.get_provenance("", "valid_id")
            except FinTextValidationError:
                val_err_caught += 1

            try:
                cli_temp.get_provenance("sentiment", "")
            except FinTextValidationError:
                val_err_caught += 1

            try:
                cli_temp.get_provenance("unsupported_type", "valid_id")
            except FinTextValidationError:
                val_err_caught += 1
        finally:
            cli_temp.close()

        report(13, "Python SDK client-side input validation errors", val_err_caught == 3, f"Caught: {val_err_caught}/3")

        # ---------------------------------------------------------------------
        # Phase 14: OpenAPI specification verification
        # ---------------------------------------------------------------------
        r_openapi = httpx.get(f"{BASE_URL}/api-docs/openapi.json", timeout=5.0)
        openapi_spec = r_openapi.json() if r_openapi.status_code == 200 else {}
        paths = openapi_spec.get("paths", {})
        schemas = openapi_spec.get("components", {}).get("schemas", {})
        route_tags = paths.get("/provenance/{record_type}/{record_id}", {}).get("get", {}).get("tags", [])
        top_tags = [t.get("name") if isinstance(t, dict) else t for t in openapi_spec.get("tags", [])]
        tag_matched = "Data Lineage & Governance" in route_tags or "Data Lineage & Governance" in top_tags

        ok_openapi = (
            r_openapi.status_code == 200
            and "/provenance/{record_type}/{record_id}" in paths
            and "DataProvenanceResponse" in schemas
            and "DataProvenanceItem" in schemas
            and "ProcessingStep" in schemas
            and tag_matched
        )
        report(14, "OpenAPI specification schema & endpoint path verification", ok_openapi, f"Tag matched: {tag_matched}")

    print("\n" + "=" * 80)
    print(f"Suite #238 Summary: {passed}/{total} phases passed ({failed} failed)")
    print("=" * 80)

    if failed > 0:
        sys.exit(1)


if __name__ == "__main__":
    main()
