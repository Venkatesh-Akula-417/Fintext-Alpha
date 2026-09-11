#!/usr/bin/env python3
"""
=====================================================================================
FinText-Alpha-Vectorizer — Point-in-Time (PIT) & Survivorship Bias Verification
=====================================================================================
Verifies Point-in-Time data integrity rules:
 1. Symbol History & Rename: FB (valid <2022-06-09) vs META (valid >=2022-06-09).
 2. Delisted Securities: TWTR (delisted 2022-10-28) rejected/empty in 2024.
 3. Date Clamping: TWTR queries in 2022 clamp query window to 2022-10-28.
 4. CSV Export PIT: Delisted/unlisted tickers return empty CSV header stream.
 5. Cross-Asset Spillover Matrix: Excludes unlisted/delisted tickers per era.
 6. Backtest Simulation: Rejects inactive tickers at backtest start date.
 7. Bypass Capability: Verifies PIT_DATA_ENABLED=0 toggle for benchmarking.
=====================================================================================
"""

import os
import sys
import time
import subprocess
import requests
from typing import Dict, Any

SERVER_EXE = os.path.abspath("rust/target/release/fintext_api.exe")
BASE_URL = "http://127.0.0.1:8000"
DEV_ADMIN_TOKEN = "fintext-admin-dev-secret-token"

def start_server(pit_enabled: bool = True) -> subprocess.Popen:
    env = os.environ.copy()
    env["QUESTDB_MOCK_FALLBACK"] = "1"
    env["PORT"] = "8000"
    env["ADMIN_TOKEN"] = DEV_ADMIN_TOKEN
    env["JWT_SECRET"] = "fintext-alpha-vectorizer-institutional-jwt-secret-key-2026"
    env["PIT_DATA_ENABLED"] = "1" if pit_enabled else "0"
    env["PIT_DATA_DIR"] = os.path.abspath("config")
    
    proc = subprocess.Popen(
        [SERVER_EXE],
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True
    )
    
    # Wait for /health
    for _ in range(30):
        try:
            r = requests.get(f"{BASE_URL}/health", timeout=1)
            if r.status_code == 200:
                return proc
        except Exception:
            time.sleep(0.3)
            
    proc.kill()
    out, err = proc.communicate()
    print("Server failed to start:\n", out, err)
    sys.exit(1)

def get_jwt() -> str:
    r = requests.post(
        f"{BASE_URL}/auth/token",
        headers={"X-Admin-Token": DEV_ADMIN_TOKEN, "Content-Type": "application/json"},
        json={"user_id": "quant_pit_specialist", "tier": "institutional"}
    )
    assert r.status_code == 200, f"Failed to get token: {r.text}"
    return r.json()["token"]

def main():
    print("=" * 85)
    print(" FinText-Alpha-Vectorizer — Point-in-Time (PIT) Data Integrity Verification")
    print("=" * 85)
    
    # Start server with PIT enabled
    print(f"[*] Starting API Server binary: {SERVER_EXE}")
    proc = start_server(pit_enabled=True)
    try:
        token = get_jwt()
        headers = {"Authorization": f"Bearer {token}"}
        print("[*] Authentication token acquired for 'quant_pit_specialist'\n")
        
        # 1. Symbol History & Rename Check (2021 Era: FB valid, META invalid)
        print("[1/7] Testing Symbol Rename (2021 Era: FB vs META)...")
        r_fb = requests.get(f"{BASE_URL}/sentiment/history?ticker=FB&start_date=2021-01-01&end_date=2021-03-31", headers=headers)
        assert r_fb.status_code == 200, f"Expected 200 for FB in 2021, got {r_fb.status_code}"
        fb_data = r_fb.json()
        assert fb_data["count"] > 0, "Expected positive record count for FB in 2021"
        print(f"      [OK] FB in 2021: {fb_data['count']} historical sentiment records returned")
        
        r_meta = requests.get(f"{BASE_URL}/sentiment/history?ticker=META&start_date=2021-01-01&end_date=2021-03-31", headers=headers)
        assert r_meta.status_code == 200, f"Expected 200 for META query in 2021, got {r_meta.status_code}"
        meta_data = r_meta.json()
        assert meta_data["count"] == 0, f"Expected 0 records for META in 2021, got {meta_data['count']}"
        print("      [OK] META in 2021: Correctly returned 0 records (symbol did not exist until June 2022)")
        
        # 2. Symbol History & Rename Check (2023 Era: META valid, FB invalid)
        print("\n[2/7] Testing Symbol Rename (2023 Era: META vs FB)...")
        r_meta23 = requests.get(f"{BASE_URL}/sentiment/history?ticker=META&start_date=2023-01-01&end_date=2023-03-31", headers=headers)
        assert r_meta23.status_code == 200
        assert r_meta23.json()["count"] > 0, "Expected positive records for META in 2023"
        print(f"      [OK] META in 2023: {r_meta23.json()['count']} historical records returned")
        
        r_fb23 = requests.get(f"{BASE_URL}/sentiment/history?ticker=FB&start_date=2023-01-01&end_date=2023-03-31", headers=headers)
        assert r_fb23.status_code == 200
        assert r_fb23.json()["count"] == 0, "Expected 0 records for retired FB symbol in 2023"
        print("      [OK] FB in 2023: Correctly returned 0 records (symbol retired post-2022)")
        
        # 3. Delisted Securities Verification (TWTR delisted Oct 28, 2022)
        print("\n[3/7] Testing Delisted Security Survivorship Bias (TWTR)...")
        r_twtr24 = requests.get(f"{BASE_URL}/sentiment/history?ticker=TWTR&start_date=2024-01-01&end_date=2024-03-31", headers=headers)
        assert r_twtr24.status_code == 200
        assert r_twtr24.json()["count"] == 0, "Expected 0 records for TWTR in 2024"
        print("      [OK] TWTR in 2024: Correctly returned 0 records (delisted Oct 2022)")
        
        # In 2022, TWTR query from Jan 1 to Dec 31 should be clamped to Oct 28 (approx 301 days)
        r_twtr22 = requests.get(f"{BASE_URL}/sentiment/history?ticker=TWTR&start_date=2022-01-01&end_date=2022-12-31&limit=1000", headers=headers)
        assert r_twtr22.status_code == 200
        twtr22_recs = r_twtr22.json()["records"]
        assert len(twtr22_recs) > 0, "Expected records for active portion of 2022"
        last_rec_date = twtr22_recs[-1]["published_utc"][:10]
        assert last_rec_date <= "2022-10-28", f"Expected last record on/before delisting date 2022-10-28, got {last_rec_date}"
        print(f"      [OK] TWTR in 2022: Clamped query window stopped cleanly at delisting date ({last_rec_date})")
        
        # 4. CSV Export Point-in-Time Streaming
        print("\n[4/7] Testing Point-in-Time CSV Export Streaming (/export/csv)...")
        r_csv_twtr = requests.get(f"{BASE_URL}/export/csv?ticker=TWTR&start_date=2024-01-01&end_date=2024-03-31", headers=headers)
        assert r_csv_twtr.status_code == 200
        lines = r_csv_twtr.text.strip().split("\r\n")
        assert len(lines) == 1, f"Expected header-only CSV for delisted TWTR in 2024, got {len(lines)} lines"
        print("      [OK] CSV Export returned header-only empty stream for delisted security")
        
        # 5. Cross-Asset Spillover Matrix Universe Filtering
        print("\n[5/7] Testing Cross-Asset Spillover Matrix Universe Filtering...")
        # In 2021: AAPL, FB, META, TWTR -> META should be omitted
        r_mat21 = requests.get(
            f"{BASE_URL}/spillovers/matrix?tickers=AAPL,FB,META,TWTR&start_date=2021-01-01&end_date=2021-03-31",
            headers=headers
        )
        assert r_mat21.status_code == 200
        mat21_tickers = r_mat21.json()["tickers"]
        assert "FB" in mat21_tickers and "TWTR" in mat21_tickers and "AAPL" in mat21_tickers
        assert "META" not in mat21_tickers, f"META should have been excluded from 2021 matrix, got {mat21_tickers}"
        print(f"      [OK] 2021 Spillover Universe: Correctly filtered to {mat21_tickers} (META excluded)")
        
        # In 2024: AAPL, FB, META, TWTR -> FB and TWTR should be omitted
        r_mat24 = requests.get(
            f"{BASE_URL}/spillovers/matrix?tickers=AAPL,FB,META,TWTR&start_date=2024-01-01&end_date=2024-03-31",
            headers=headers
        )
        assert r_mat24.status_code == 200
        mat24_tickers = r_mat24.json()["tickers"]
        assert "AAPL" in mat24_tickers and "META" in mat24_tickers
        assert "FB" not in mat24_tickers and "TWTR" not in mat24_tickers
        print(f"      [OK] 2024 Spillover Universe: Correctly filtered to {mat24_tickers} (FB and TWTR excluded)")
        
        # 6. Backtest Simulation PIT Validation
        print("\n[6/7] Testing Point-in-Time Alpha Backtest Simulation (/backtest)...")
        # Inactive ticker at start date should be rejected with 400 Bad Request
        r_bt_invalid = requests.post(
            f"{BASE_URL}/backtest",
            headers=headers,
            json={
                "ticker": "META",
                "start_date": "2021-01-01",
                "end_date": "2021-12-31",
                "long_threshold": 0.3,
                "short_threshold": -0.3,
                "holding_days": 5,
                "initial_capital": 1000000.0
            }
        )
        assert r_bt_invalid.status_code == 400, f"Expected 400 Bad Request for inactive ticker backtest, got {r_bt_invalid.status_code}"
        print(f"      [OK] Backtest properly rejected inactive symbol: {r_bt_invalid.json().get('error')}")
        
        # Valid ticker (AAPL in 2021) should execute successfully
        r_bt_valid = requests.post(
            f"{BASE_URL}/backtest",
            headers=headers,
            json={
                "ticker": "AAPL",
                "start_date": "2021-01-01",
                "end_date": "2021-12-31",
                "long_threshold": 0.3,
                "short_threshold": -0.3,
                "holding_days": 5,
                "initial_capital": 1000000.0
            }
        )
        assert r_bt_valid.status_code == 200, f"Expected 200 for valid backtest, got {r_bt_valid.status_code}"
        print(f"      [OK] Backtest executed for valid PIT ticker: Sharpe = {r_bt_valid.json().get('sharpe_ratio')}")
        
    finally:
        proc.terminate()
        proc.wait()
        
    # 7. Testing PIT Bypass via PIT_DATA_ENABLED=0
    print("\n[7/7] Testing PIT Bypass (PIT_DATA_ENABLED=0)...")
    proc_bypass = start_server(pit_enabled=False)
    try:
        token = get_jwt()
        headers = {"Authorization": f"Bearer {token}"}
        # With PIT disabled, querying META in 2021 should return records without filtering
        r_bypass = requests.get(f"{BASE_URL}/sentiment/history?ticker=META&start_date=2021-01-01&end_date=2021-03-31", headers=headers)
        assert r_bypass.status_code == 200
        assert r_bypass.json()["count"] > 0, "Expected records when PIT is disabled"
        print(f"      [OK] PIT Bypass active: META returned {r_bypass.json()['count']} records when PIT_DATA_ENABLED=0")
    finally:
        proc_bypass.terminate()
        proc_bypass.wait()

    print("\n" + "=" * 85)
    print(" [OK] ALL POINT-IN-TIME (PIT) DATA INTEGRITY CHECKS PASSED!")
    print("=" * 85)

if __name__ == "__main__":
    main()
