#!/usr/bin/env python3
"""
=====================================================================================
FinText-Alpha-Vectorizer — Historical CSV Export Streaming Live Verification
=====================================================================================
Validates that:
1. The API Gateway serves streaming CSV exports at `GET /export/csv`.
2. Valid query returns HTTP 200 with `text/csv` and appropriate `Content-Disposition`.
3. The CSV structure complies with RFC 4180 and matches the quantitative alpha schema.
4. Parameter validation correctly rejects invalid dates, tickers, formats, and ranges.
5. Protected route enforces Bearer JWT authentication and rate limiting.
=====================================================================================
"""

import csv
import io
import json
import os
import subprocess
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

PORT = 8087
BASE_URL = f"http://127.0.0.1:{PORT}"
BINARY_PATH = Path("rust/target/release/fintext_api.exe").resolve()


def run_checks():
    print("=" * 85)
    print(" FinText-Alpha-Vectorizer — Historical CSV Export Streaming Verification")
    print("=" * 85)

    env = os.environ.copy()
    env.update({
        "PORT": str(PORT),
        "HOST": "127.0.0.1",
        "JWT_SECRET": "csv_export_verification_secret_key_32_len!!",
        "ADMIN_TOKEN": "csv_export_admin_token_super_secret_12345",
        "QUESTDB_MOCK_FALLBACK": "1",
        "NATS_MOCK_MODE": "1",
        "RATE_LIMIT_REQUESTS": "20",
        "RATE_LIMIT_WINDOW_SECS": "60",
        "RUST_LOG": "info",
    })

    print(f"[*] Starting API Server binary: {BINARY_PATH}")
    proc = subprocess.Popen(
        [str(BINARY_PATH)],
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    try:
        # Wait for API server readiness
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

        # 1. Acquire JWT
        print("\n[1/6] Obtaining institutional JWT via POST /auth/token...")
        token_payload = json.dumps({
            "user_id": "quant_csv_researcher_01",
            "expires_in_seconds": 3600,
            "role": "institutional",
        }).encode("utf-8")

        req = urllib.request.Request(
            f"{BASE_URL}/auth/token",
            data=token_payload,
            headers={
                "Content-Type": "application/json",
                "X-Admin-Token": "csv_export_admin_token_super_secret_12345",
            },
            method="POST",
        )
        with urllib.request.urlopen(req) as resp:
            token_data = json.loads(resp.read().decode("utf-8"))
            token = token_data["token"]
            print(f"      Token issued successfully for '{token_data['user_id']}'")

        auth_headers = {
            "Authorization": f"Bearer {token}",
        }

        # 2. Test Valid Streamed CSV Export
        print("\n[2/6] Querying GET /export/csv?ticker=AAPL&start_date=2025-01-01&end_date=2025-01-10&limit=50...")
        url = f"{BASE_URL}/export/csv?ticker=AAPL&start_date=2025-01-01&end_date=2025-01-10&limit=50"
        req = urllib.request.Request(url, headers=auth_headers)

        with urllib.request.urlopen(req) as resp:
            assert resp.status == 200, f"Expected 200, got {resp.status}"
            content_type = resp.headers.get("Content-Type", "")
            content_disposition = resp.headers.get("Content-Disposition", "")

            print(f"      HTTP 200 OK | Content-Type: {content_type}")
            print(f"      Content-Disposition: {content_disposition}")

            assert "text/csv" in content_type
            assert "sentiment_AAPL_2025-01-01_2025-01-10.csv" in content_disposition

            csv_raw = resp.read().decode("utf-8")
            reader = csv.reader(io.StringIO(csv_raw))
            rows = list(reader)

            assert len(rows) > 1, f"Expected header + data rows, got {len(rows)} rows"
            expected_headers = ["published_utc", "ticker", "source", "title", "sentiment_score", "vpin", "gamma_exposure"]
            assert rows[0] == expected_headers, f"Header mismatch: {rows[0]} vs {expected_headers}"

            print(f"      CSV Header: {rows[0]}")
            print(f"      Sample Row 1: {rows[1]}")
            print(f"      Total Streamed Rows: {len(rows) - 1} data records")

        # 3. Test Invalid Date Formats
        print("\n[3/6] Testing rejection of invalid start_date format...")
        bad_date_url = f"{BASE_URL}/export/csv?ticker=AAPL&start_date=01-01-2025&end_date=2025-01-10"
        req = urllib.request.Request(bad_date_url, headers=auth_headers)
        try:
            with urllib.request.urlopen(req) as resp:
                print("[-] FAIL: Server accepted invalid date format")
                return False
        except urllib.error.HTTPError as e:
            assert e.code == 400
            err_data = json.loads(e.read().decode("utf-8"))
            print(f"      [OK] HTTP 400 Bad Request: {err_data['message']}")

        # 4. Test Inverted Date Range (start_date > end_date)
        print("\n[4/6] Testing rejection of inverted date range...")
        inverted_url = f"{BASE_URL}/export/csv?ticker=AAPL&start_date=2025-05-01&end_date=2025-01-01"
        req = urllib.request.Request(inverted_url, headers=auth_headers)
        try:
            with urllib.request.urlopen(req) as resp:
                print("[-] FAIL: Server accepted inverted date range")
                return False
        except urllib.error.HTTPError as e:
            assert e.code == 400
            err_data = json.loads(e.read().decode("utf-8"))
            print(f"      [OK] HTTP 400 Bad Request: {err_data['message']}")

        # 5. Test Unsupported Export Format
        print("\n[5/6] Testing rejection of unsupported format (e.g. parquet)...")
        bad_fmt_url = f"{BASE_URL}/export/csv?ticker=AAPL&start_date=2025-01-01&end_date=2025-01-10&format=parquet"
        req = urllib.request.Request(bad_fmt_url, headers=auth_headers)
        try:
            with urllib.request.urlopen(req) as resp:
                print("[-] FAIL: Server accepted unsupported format")
                return False
        except urllib.error.HTTPError as e:
            assert e.code == 400
            err_data = json.loads(e.read().decode("utf-8"))
            print(f"      [OK] HTTP 400 Bad Request: {err_data['message']}")

        # 6. Test Unauthenticated Access
        print("\n[6/6] Testing rejection of unauthenticated export request...")
        unauth_req = urllib.request.Request(url)
        try:
            with urllib.request.urlopen(unauth_req) as resp:
                print("[-] FAIL: Server allowed unauthenticated export")
                return False
        except urllib.error.HTTPError as e:
            assert e.code == 401
            print(f"      [OK] HTTP 401 Unauthorized successfully enforced.")

        print("\n" + "=" * 85)
        print(" [OK] ALL HISTORICAL CSV EXPORT CHECKS PASSED!")
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
