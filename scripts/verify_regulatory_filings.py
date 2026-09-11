#!/usr/bin/env python3
"""
═══════════════════════════════════════════════════════════════════════════════
FinText-Alpha-Vectorizer — Test Suite #204: Regulatory Filing Classifier Engine
═══════════════════════════════════════════════════════════════════════════════
Validates:
  1. Unauthenticated & Invalid Token Access Rejection (401).
  2. Input Validation & Parameter Bounds Enforcement (400).
  3. Single Ticker Regulatory Filing Query (AAPL).
  4. Form Type Filter (10-K -> Annual Report).
  5. Event Category Filter (M&A).
  6. Date Range Window Parameterization.
  7. Pagination with Offset and Limit Slicing (offset=2, limit=3).
  8. Point-in-Time (PIT) Delisted Ticker Enforcement (TWTR rejected).
  9. Rate Limiting & Response Headers.
  10. OpenAPI 3.0 Conformance & Python SDK Sync/Async Execution.
═══════════════════════════════════════════════════════════════════════════════
"""

import asyncio
import os
import subprocess
import sys
import time
from pathlib import Path
from typing import Optional

import httpx

ROOT_DIR = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT_DIR / "python_sdk" / "src"))
if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")

from fintext import FinTextClient, FinTextAsyncClient
from fintext.models import RegulatoryFilingsResponse, RegulatoryFilingItem

SERVER_PORT = 8099
BASE_URL = f"http://127.0.0.1:{SERVER_PORT}"
ADMIN_TOKEN = "fintext-admin-dev-secret-token"
JWT_SECRET = "fintext-alpha-vectorizer-institutional-jwt-secret-key-2026"


class ServerContext:
    def __init__(self):
        self.process: Optional[subprocess.Popen] = None

    def __enter__(self):
        env = os.environ.copy()
        env["PORT"] = str(SERVER_PORT)
        env["ADMIN_TOKEN"] = ADMIN_TOKEN
        env["JWT_SECRET"] = JWT_SECRET
        env["QUESTDB_MOCK_FALLBACK"] = "1"
        env["POLYGON_MOCK_FALLBACK"] = "1"
        env["WHISPER_MOCK_FALLBACK"] = "1"
        env["NATS_MOCK_MODE"] = "1"

        exe_path = ROOT_DIR / "rust" / "target" / "release" / "fintext_api.exe"
        if not exe_path.exists():
            exe_path = ROOT_DIR / "rust" / "target" / "debug" / "fintext_api.exe"

        if not exe_path.exists():
            raise RuntimeError(f"Server binary not found at {exe_path}. Build it first.")

        print(f"[STARTING] Spawning FinText API Server from {exe_path} on port {SERVER_PORT}...")
        self.process = subprocess.Popen(
            [str(exe_path)],
            env=env,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )

        max_attempts = 50
        for i in range(max_attempts):
            try:
                resp = httpx.get(f"{BASE_URL}/health", timeout=0.5)
                if resp.status_code == 200:
                    print(f"[READY] Server is healthy and ready (attempt {i + 1})")
                    return self
            except Exception:
                time.sleep(0.1)

        if self.process.poll() is not None:
            raise RuntimeError(f"Server failed to start (exit code {self.process.poll()}).")
        raise RuntimeError("Server health check timed out.")

    def __exit__(self, exc_type, exc_val, exc_tb):
        if self.process:
            print("[STOPPING] Shutting down FinText API Server...")
            self.process.terminate()
            try:
                self.process.wait(timeout=3)
            except subprocess.TimeoutExpired:
                self.process.kill()


def get_token(user_id: str) -> str:
    resp = httpx.post(
        f"{BASE_URL}/auth/token",
        json={"user_id": user_id},
        headers={"X-Admin-Token": ADMIN_TOKEN},
    )
    assert resp.status_code == 200, f"Failed to get token for {user_id}: {resp.text}"
    return resp.json()["token"]


def run_tests():
    print("=" * 80)
    print("FINTEXT ALPHA VECTORIZER — TEST SUITE #204: REGULATORY FILING CLASSIFIER ENGINE")
    print("=" * 80)

    token = get_token("quant_regulatory_compliance_desk")
    auth_headers = {"Authorization": f"Bearer {token}"}

    client = FinTextClient(base_url=BASE_URL, api_token=token)

    with httpx.Client(base_url=BASE_URL, timeout=10.0) as http:
        # ─────────────────────────────────────────────────────────────────────
        # Phase 1: Unauthenticated & Invalid Token Access Rejection
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 1] Testing Unauthenticated & Invalid Token Access Rejection...")
        r = http.get("/events/filings?ticker=AAPL")
        assert r.status_code == 401, f"Expected 401, got {r.status_code}"

        r = http.get(
            "/events/filings?ticker=AAPL",
            headers={"Authorization": "Bearer bad_token_123"},
        )
        assert r.status_code == 401, f"Expected 401, got {r.status_code}"
        print("[PASS] Phase 1 Passed: Unauthorized requests correctly rejected (401).")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 2: Input Validation & Parameter Bounds Enforcement
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 2] Testing Input Validation & Bounds Enforcement...")
        # start_date > end_date
        r = http.get("/events/filings?start_date=2025-12-31&end_date=2025-01-01", headers=auth_headers)
        assert r.status_code == 400, f"Expected 400 for start_date > end_date, got {r.status_code}"

        # limit > 100
        r = http.get("/events/filings?limit=200", headers=auth_headers)
        assert r.status_code == 400, f"Expected 400 for limit > 100, got {r.status_code}"
        print("[PASS] Phase 2 Passed: Parameter validation and bounds strictly enforced (400).")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 3: Single Ticker Regulatory Filing Query (AAPL)
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 3] Testing Single Ticker Regulatory Filing Query (AAPL)...")
        r = http.get("/events/filings?ticker=AAPL", headers=auth_headers)
        assert r.status_code == 200, f"Expected 200, got {r.status_code}: {r.text}"
        data = r.json()
        assert data["ticker"] == "AAPL"
        assert len(data["filings"]) > 0
        for f in data["filings"]:
            assert f["ticker"] == "AAPL"
            assert len(f["accession_number"]) > 0
            assert len(f["event_category"]) > 0
            assert f["url"].startswith("https://www.sec.gov")
        print(f"[PASS] Phase 3 Passed: AAPL Filings returned {data['count']} records across event categories {[f['event_category'] for f in data['filings'][:3]]}.")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 4: Form Type Filter (10-K -> Annual Report)
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 4] Testing Form Type Filter (form_type=10-K)...")
        r_10k = http.get("/events/filings?form_type=10-K", headers=auth_headers)
        assert r_10k.status_code == 200
        data_10k = r_10k.json()
        assert data_10k["form_type"] == "10-K"
        assert len(data_10k["filings"]) > 0
        for f in data_10k["filings"]:
            assert f["form_type"] == "10-K"
            assert f["event_category"] == "Annual Report"
        print(f"[PASS] Phase 4 Passed: 10-K form filter returned {data_10k['count']} filings, all mapped to 'Annual Report'.")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 5: Event Category Filter (M&A)
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 5] Testing Event Category Filter (event_category=M&A)...")
        r_ma = http.get("/events/filings", params={"event_category": "M&A"}, headers=auth_headers)
        assert r_ma.status_code == 200
        data_ma = r_ma.json()
        assert data_ma["event_category"] == "M&A"
        assert len(data_ma["filings"]) > 0
        for f in data_ma["filings"]:
            assert f["event_category"] == "M&A"
        print(f"[PASS] Phase 5 Passed: M&A category filter isolated {data_ma['count']} M&A disclosure filings.")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 6: Date Range Window Parameterization
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 6] Testing Date Range Window Parameterization...")
        r_dates = http.get("/events/filings?start_date=2025-01-01&end_date=2025-06-30", headers=auth_headers)
        assert r_dates.status_code == 200
        data_dates = r_dates.json()
        assert data_dates["start_date"] == "2025-01-01"
        assert data_dates["end_date"] == "2025-06-30"
        for f in data_dates["filings"]:
            assert "2025-01-01" <= f["filing_date"] <= "2025-06-30"
        print(f"[PASS] Phase 6 Passed: Date bounds correctly filtered records to 2025-01-01..2025-06-30.")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 7: Pagination with Offset & Limit Slicing
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 7] Testing Pagination with Offset & Limit Slicing (offset=2, limit=3)...")
        r_page1 = http.get("/events/filings?limit=5&offset=0", headers=auth_headers)
        r_page2 = http.get("/events/filings?limit=3&offset=2", headers=auth_headers)
        assert r_page1.status_code == 200 and r_page2.status_code == 200
        d1 = r_page1.json()
        d2 = r_page2.json()
        assert d2["offset"] == 2
        assert d2["limit"] == 3
        assert len(d2["filings"]) == 3
        assert d2["filings"][0]["accession_number"] == d1["filings"][2]["accession_number"]
        print(f"[PASS] Phase 7 Passed: Pagination slice offset=2, limit=3 matched parent list slice.")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 8: Point-in-Time (PIT) Delisted Ticker Enforcement
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 8] Testing Point-in-Time (PIT) Delisted Ticker Enforcement...")
        r_pit = http.get(
            "/events/filings?ticker=TWTR",
            headers=auth_headers,
        )
        assert r_pit.status_code == 400
        assert "not active or listed" in r_pit.json()["message"]
        print("[PASS] Phase 8 Passed: Delisted ticker TWTR correctly rejected under PIT verification (400).")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 9: Rate Limiting & Response Headers
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 9] Testing Rate Limiting & Response Headers...")
        assert "x-ratelimit-limit" in r.headers
        assert "x-ratelimit-remaining" in r.headers
        assert "x-ratelimit-reset" in r.headers
        print(f"[PASS] Phase 9 Passed: Rate limit headers verified (Limit: {r.headers['x-ratelimit-limit']}, Remaining: {r.headers['x-ratelimit-remaining']}).")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 10: OpenAPI 3.0 Conformance & Python SDK Execution
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 10] Testing OpenAPI 3.0 Spec Conformance & Python SDK Sync/Async...")
        r_spec = http.get("/api-docs/openapi.json")
        assert r_spec.status_code == 200
        spec = r_spec.json()
        assert "/events/filings" in spec["paths"]
        assert "RegulatoryFilingsResponse" in spec["components"]["schemas"]
        assert "RegulatoryFilingItem" in spec["components"]["schemas"]

        # Python SDK Sync
        sdk_sync = client.regulatory_filings(ticker="AAPL", form_type="10-K")
        assert isinstance(sdk_sync, RegulatoryFilingsResponse)
        assert sdk_sync.ticker == "AAPL"
        assert sdk_sync.form_type == "10-K"
        assert len(sdk_sync.filings) >= 1
        print(f"  + Python SDK Sync: Received {len(sdk_sync.filings)} AAPL 10-K filings (Category: {sdk_sync.filings[0].event_category}).")

        # Python SDK Async
        async def run_async_sdk():
            async_client = FinTextAsyncClient(base_url=BASE_URL, api_token=token)
            async_resp = await async_client.regulatory_filings(event_category="M&A", limit=5)
            assert isinstance(async_resp, RegulatoryFilingsResponse)
            assert async_resp.event_category == "M&A"
            assert len(async_resp.filings) <= 5
            await async_client.close()
            print(f"  + Python SDK Async: Received {len(async_resp.filings)} M&A filings across universe.")

        asyncio.run(run_async_sdk())
        print("[PASS] Phase 10 Passed: OpenAPI documentation and Python SDK sync/async certified.")

        client.close()

    print("\n" + "=" * 80)
    print("[SUCCESS] ALL 10 PHASES PASSED: REGULATORY FILING CLASSIFIER ENGINE CERTIFIED!")
    print("=" * 80)


def main():
    with ServerContext():
        run_tests()


if __name__ == "__main__":
    main()
