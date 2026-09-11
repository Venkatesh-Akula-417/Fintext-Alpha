"""
═══════════════════════════════════════════════════════════════════════════════
FinText-Alpha-Vectorizer — Statistical Sentiment Anomaly Detection Verification
═══════════════════════════════════════════════════════════════════════════════

Suite #193: Sentiment Anomaly Detection Engine (GET /sentiment/anomalies)
═══════════════════════════════════════════════════════════════════════════════
"""

import asyncio
import json
import os
from pathlib import Path
import subprocess
import sys
import time
import urllib.error
import urllib.request

PROJECT_ROOT = Path(__file__).resolve().parent.parent
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))
if str(PROJECT_ROOT / "python_sdk" / "src") not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT / "python_sdk" / "src"))

from fintext import FinTextClient, FinTextAsyncClient, SentimentAnomaliesResponse


def run_verification():
    print("=" * 80)
    print(" Suite #193: Sentiment Anomaly Detection Engine Verification (GET /sentiment/anomalies)")
    print("=" * 80)

    bin_name = "fintext_api.exe" if sys.platform == "win32" else "fintext_api"
    bin_path = PROJECT_ROOT / "rust" / "target" / "release" / bin_name

    if not bin_path.exists():
        print(f"[-] Release binary not found at {bin_path}. Please build it first.")
        sys.exit(1)

    port = 8016
    base_url = f"http://127.0.0.1:{port}"
    jwt_secret = "suite193-anomalies-secret-key-2026"
    admin_token = "suite193-admin-token"

    env = os.environ.copy()
    env["QUESTDB_MOCK_FALLBACK"] = "1"
    env["NATS_MOCK_MODE"] = "1"
    env["JWT_SECRET"] = jwt_secret
    env["ADMIN_TOKEN"] = admin_token
    env["PORT"] = str(port)

    print(f"[*] Starting native Axum API server on port {port}...")
    proc = subprocess.Popen(
        [str(bin_path)],
        cwd=str(PROJECT_ROOT),
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        env=env,
    )

    time.sleep(1.5)

    try:
        # Phase 1: Obtain institutional JWT
        print("\n[1/8] Obtaining institutional JWT via POST /auth/token...")
        token_payload = json.dumps({
            "user_id": "suite193_quant_trader",
            "expires_in_seconds": 3600,
            "role": "institutional",
        }).encode("utf-8")

        req = urllib.request.Request(
            f"{base_url}/auth/token",
            data=token_payload,
            headers={
                "Content-Type": "application/json",
                "X-Admin-Token": admin_token,
            },
            method="POST",
        )
        with urllib.request.urlopen(req, timeout=5) as resp:
            assert resp.status == 200
            token_data = json.loads(resp.read().decode("utf-8"))
            token = token_data["token"]
            assert len(token) > 20
            print(f"    [+] Successfully issued institutional JWT (len={len(token)})")

        auth_headers = {
            "Authorization": f"Bearer {token}",
            "Content-Type": "application/json",
        }

        # Phase 2: Default GET /sentiment/anomalies
        print("\n[2/8] Querying GET /sentiment/anomalies (Default parameters)...")
        req = urllib.request.Request(f"{base_url}/sentiment/anomalies", headers=auth_headers)
        with urllib.request.urlopen(req, timeout=5) as resp:
            assert resp.status == 200
            data = json.loads(resp.read().decode("utf-8"))
            assert "items" in data
            assert data["lookback_days"] == 30
            assert data["zscore_threshold"] == 2.0
            assert data["min_records"] == 20
            assert data["count"] == len(data["items"])
            assert data["scanned_tickers"] > 0
            print(f"    [+] Scanned {data['scanned_tickers']} tickers | Found {data['total_anomalies_detected']} anomalies (Returned {data['count']})")

            # Check sorting by absolute z-score descending
            zscores = [abs(item["zscore"]) for item in data["items"]]
            for i in range(len(zscores) - 1):
                assert zscores[i] >= zscores[i + 1] - 1e-4, "Anomalies must be sorted descending by |z-score|"

            for item in data["items"]:
                assert abs(item["zscore"]) >= 2.0
                assert item["direction"] in ("bullish", "bearish")
                assert item["record_count"] >= 20
                print(f"        - {item['ticker']}: {item['direction'].upper()} (z={item['zscore']:+.2f}, latest={item['latest_score']:.2f}, mean={item['mean_score']:.2f}, std={item['stddev']:.2f})")

        # Phase 3: Sector-filtered query
        print("\n[3/8] Querying GET /sentiment/anomalies?sector=Technology&lookback_days=45&zscore_threshold=2.2...")
        req = urllib.request.Request(
            f"{base_url}/sentiment/anomalies?sector=Technology&lookback_days=45&zscore_threshold=2.2&min_records=25&limit=5",
            headers=auth_headers,
        )
        with urllib.request.urlopen(req, timeout=5) as resp:
            assert resp.status == 200
            data = json.loads(resp.read().decode("utf-8"))
            assert data["sector"] in ("Information Technology", "Technology")
            assert data["lookback_days"] == 45
            assert data["zscore_threshold"] == 2.2
            assert data["min_records"] == 25
            assert data["count"] <= 5
            print(f"    [+] Sector {data['sector']} scanned: {data['count']} anomalies returned (limit=5)")

        # Phase 4: High z-score threshold query
        print("\n[4/8] Querying GET /sentiment/anomalies with high zscore_threshold=2.8...")
        req = urllib.request.Request(
            f"{base_url}/sentiment/anomalies?zscore_threshold=2.8",
            headers=auth_headers,
        )
        with urllib.request.urlopen(req, timeout=5) as resp:
            assert resp.status == 200
            data = json.loads(resp.read().decode("utf-8"))
            for item in data["items"]:
                assert abs(item["zscore"]) >= 2.8
            print(f"    [+] High-threshold filter verified ({data['count']} extreme anomalies found with |z| >= 2.8)")

        # Phase 5: Pagination limit filter
        print("\n[5/8] Testing limit boundary (limit=2)...")
        req = urllib.request.Request(
            f"{base_url}/sentiment/anomalies?limit=2",
            headers=auth_headers,
        )
        with urllib.request.urlopen(req, timeout=5) as resp:
            assert resp.status == 200
            data = json.loads(resp.read().decode("utf-8"))
            assert data["count"] <= 2
            assert len(data["items"]) <= 2
            print(f"    [+] Limit truncation verified: {data['count']} items returned")

        # Phase 6: Parameter validation & error responses
        print("\n[6/8] Testing parameter validation and error responses...")
        
        # 6a. Unknown sector -> 404
        req = urllib.request.Request(f"{base_url}/sentiment/anomalies?sector=InvalidSector999", headers=auth_headers)
        try:
            urllib.request.urlopen(req, timeout=5)
            raise AssertionError("Expected 404 for unknown sector")
        except urllib.error.HTTPError as err:
            assert err.code == 404
            err_data = json.loads(err.read().decode("utf-8"))
            assert "Unknown sector" in err_data["message"]
            print("    [+] 404 Not Found correctly returned for unknown sector")

        # 6b. Out of bounds lookback_days (> 90) -> 400
        req = urllib.request.Request(f"{base_url}/sentiment/anomalies?lookback_days=120", headers=auth_headers)
        try:
            urllib.request.urlopen(req, timeout=5)
            raise AssertionError("Expected 400 for lookback_days > 90")
        except urllib.error.HTTPError as err:
            assert err.code == 400
            err_data = json.loads(err.read().decode("utf-8"))
            assert "lookback_days" in err_data["message"]
            print("    [+] 400 Bad Request correctly returned for lookback_days > 90")

        # 6c. Out of bounds zscore_threshold (< 1.0) -> 400
        req = urllib.request.Request(f"{base_url}/sentiment/anomalies?zscore_threshold=0.5", headers=auth_headers)
        try:
            urllib.request.urlopen(req, timeout=5)
            raise AssertionError("Expected 400 for zscore_threshold < 1.0")
        except urllib.error.HTTPError as err:
            assert err.code == 400
            err_data = json.loads(err.read().decode("utf-8"))
            assert "zscore_threshold" in err_data["message"]
            print("    [+] 400 Bad Request correctly returned for zscore_threshold < 1.0")

        # 6d. Out of bounds limit (> 100) -> 400
        req = urllib.request.Request(f"{base_url}/sentiment/anomalies?limit=200", headers=auth_headers)
        try:
            urllib.request.urlopen(req, timeout=5)
            raise AssertionError("Expected 400 for limit > 100")
        except urllib.error.HTTPError as err:
            assert err.code == 400
            err_data = json.loads(err.read().decode("utf-8"))
            assert "limit" in err_data["message"]
            print("    [+] 400 Bad Request correctly returned for limit > 100")

        # Phase 7: Authentication enforcement
        print("\n[7/8] Verifying authentication enforcement without Bearer JWT...")
        req = urllib.request.Request(f"{base_url}/sentiment/anomalies")
        try:
            urllib.request.urlopen(req, timeout=5)
            raise AssertionError("Expected 401 Unauthorized for missing JWT")
        except urllib.error.HTTPError as err:
            assert err.code == 401
            print("    [+] 401 Unauthorized correctly enforced for unauthenticated requests")

        # Phase 8: Python SDK Live Integration
        print("\n[8/8] Testing official Python SDK FinTextClient and FinTextAsyncClient...")
        sdk_sync = FinTextClient(base_url=base_url, api_token=token)
        sync_resp = sdk_sync.sentiment_anomalies(sector="Technology", zscore_threshold=2.0, limit=5)
        assert isinstance(sync_resp, SentimentAnomaliesResponse)
        assert sync_resp.count <= 5
        print(f"    [+] Python SDK (Sync): {sync_resp.count} anomalies retrieved via FinTextClient")

        async def test_sdk_async():
            sdk_async = FinTextAsyncClient(base_url=base_url, api_token=token)
            async_resp = await sdk_async.sentiment_anomalies(lookback_days=30, zscore_threshold=2.0, limit=5)
            assert isinstance(async_resp, SentimentAnomaliesResponse)
            assert async_resp.count <= 5
            await sdk_async.close()
            return async_resp.count

        async_count = asyncio.run(test_sdk_async())
        print(f"    [+] Python SDK (Async): {async_count} anomalies retrieved via FinTextAsyncClient")

        print("\n" + "=" * 80)
        print(" [OK] ALL 8 SENTIMENT ANOMALY DETECTION TEST PHASES PASSED CLEANLY!")
        print("=" * 80)

    finally:
        proc.terminate()
        try:
            proc.wait(timeout=2)
        except Exception:
            proc.kill()


if __name__ == "__main__":
    run_verification()
