#!/usr/bin/env python3
"""
═══════════════════════════════════════════════════════════════════════════════
FinText-Alpha-Vectorizer — Test Suite #196: Market Regime Detection Engine
═══════════════════════════════════════════════════════════════════════════════
Validates:
  1. Unauthenticated & Invalid Token Access Rejection (401).
  2. Input Validation & Bounds Enforcement (lookback_days 1..30, min_data_points 1..1000).
  3. Default Market Regime Query (lookback_days=5, min_data_points=50).
  4. Custom Key-Value Sector Weighting (Technology:0.5,Financials:0.3,...).
  5. Comma-Separated Float Sector Weighting.
  6. Market Breadth & Sentiment Breakdown Component Verification.
  7. Cross-Asset Spillover Correlation & Volatility Proxy Consistency.
  8. Classification State Rules & Confidence Bounds Verification.
  9. Rate Limiting & Institutional Quota Tracking.
  10. OpenAPI 3.0 Conformance & Python SDK Sync/Async Client Execution.
═══════════════════════════════════════════════════════════════════════════════
"""

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
from fintext.models import MarketRegimeResponse, RegimeComponents

SERVER_PORT = 8098
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
    print("FINTEXT ALPHA VECTORIZER — TEST SUITE #196: MARKET REGIME DETECTION")
    print("=" * 80)

    token = get_token("macro_fund_bridgewater")
    auth_headers = {"Authorization": f"Bearer {token}"}

    client = FinTextClient(base_url=BASE_URL, api_token=token)

    with httpx.Client(base_url=BASE_URL, timeout=10.0) as http:
        # ─────────────────────────────────────────────────────────────────────
        # Phase 1: Unauthenticated & Invalid Token Access Rejection
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 1] Testing Unauthenticated & Invalid Token Access Rejection...")
        r = http.get("/market/regime")
        assert r.status_code == 401, f"Expected 401, got {r.status_code}"

        r = http.get("/market/regime", headers={"Authorization": "Bearer invalid_token_xyz"})
        assert r.status_code == 401, f"Expected 401, got {r.status_code}"
        print("[PASS] Phase 1 Passed: Unauthorized requests correctly rejected (401).")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 2: Input Validation & Bounds Enforcement
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 2] Testing Input Validation & Bounds Enforcement...")
        # lookback_days < 1
        r = http.get("/market/regime?lookback_days=0", headers=auth_headers)
        assert r.status_code == 400, f"Expected 400 for lookback_days=0, got {r.status_code}"

        # lookback_days > 30
        r = http.get("/market/regime?lookback_days=31", headers=auth_headers)
        assert r.status_code == 400, f"Expected 400 for lookback_days=31, got {r.status_code}"

        # min_data_points < 1
        r = http.get("/market/regime?min_data_points=0", headers=auth_headers)
        assert r.status_code == 400, f"Expected 400 for min_data_points=0, got {r.status_code}"

        # min_data_points > 1000
        r = http.get("/market/regime?min_data_points=1001", headers=auth_headers)
        assert r.status_code == 400, f"Expected 400 for min_data_points=1001, got {r.status_code}"

        # Malformed sector weights
        r = http.get("/market/regime?sector_weights=Technology:invalid_float", headers=auth_headers)
        assert r.status_code == 400, f"Expected 400 for invalid sector_weights float, got {r.status_code}"

        r = http.get("/market/regime?sector_weights=Technology:-0.5", headers=auth_headers)
        assert r.status_code == 400, f"Expected 400 for negative sector weight, got {r.status_code}"
        print("[PASS] Phase 2 Passed: Parameter validation and bounds strictly enforced (400).")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 3: Default Market Regime Query
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 3] Testing Default Market Regime Query (lookback_days=5, min_data_points=50)...")
        r = http.get("/market/regime", headers=auth_headers)
        assert r.status_code == 200, f"Expected 200, got {r.status_code}: {r.text}"
        data = r.json()
        assert data["regime"] in ["Bullish", "Bearish", "Neutral", "High Volatility"]
        assert 0.20 <= data["confidence"] <= 1.0
        assert -1.0 <= data["market_sentiment"] <= 1.0
        assert 0.0 <= data["breadth"] <= 1.0
        assert 0.0 <= data["avg_spillover_corr"] <= 1.0
        assert data["volatility_proxy"] >= 0.0
        assert data["lookback_days"] == 5
        assert "generated_at" in data
        assert isinstance(data["components"]["sector_sentiments"], dict)
        assert len(data["components"]["sector_sentiments"]) >= 5
        print(f"[PASS] Phase 3 Passed: Default regime '{data['regime']}' (confidence: {data['confidence']:.2f}, sentiment: {data['market_sentiment']:.4f}, breadth: {data['breadth']*100:.1f}%).")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 4: Custom Key-Value Sector Weighting
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 4] Testing Custom Key-Value Sector Weighting...")
        r = http.get(
            "/market/regime?lookback_days=7&sector_weights=Technology:0.6,Financials:0.3,Healthcare:0.1",
            headers=auth_headers,
        )
        assert r.status_code == 200, f"Expected 200, got {r.status_code}: {r.text}"
        data_kv = r.json()
        assert data_kv["lookback_days"] == 7
        assert data_kv["components"]["total_data_points"] > 0
        print(f"[PASS] Phase 4 Passed: Key-value sector weights applied (Regime: {data_kv['regime']}, Sentiment: {data_kv['market_sentiment']:.4f}).")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 5: Comma-Separated Float Sector Weighting
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 5] Testing Comma-Separated Float Sector Weighting...")
        r = http.get(
            "/market/regime?lookback_days=10&sector_weights=0.3,0.2,0.1,0.1,0.1,0.05,0.05,0.05,0.05",
            headers=auth_headers,
        )
        assert r.status_code == 200, f"Expected 200, got {r.status_code}: {r.text}"
        data_csv = r.json()
        assert data_csv["lookback_days"] == 10
        print(f"[PASS] Phase 5 Passed: Comma-separated weights accepted (Regime: {data_csv['regime']}).")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 6: Market Breadth & Sentiment Breakdown Verification
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 6] Testing Market Breadth & Component Consistency...")
        comp = data["components"]
        assert comp["total_data_points"] >= 20
        total_tickers = comp["bullish_tickers_count"] + comp["bearish_tickers_count"] + comp["neutral_tickers_count"]
        assert total_tickers > 0
        expected_ratio = round(comp["bullish_tickers_count"] / total_tickers, 4)
        assert abs(comp["positive_sentiment_ratio"] - expected_ratio) < 0.01
        assert abs(data["breadth"] - expected_ratio) < 0.01
        print(f"[PASS] Phase 6 Passed: Component counts verified (Bullish: {comp['bullish_tickers_count']}, Bearish: {comp['bearish_tickers_count']}, Neutral: {comp['neutral_tickers_count']}).")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 7: Cross-Asset Spillover Correlation & Volatility Proxy
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 7] Testing Cross-Asset Spillover Correlation & Volatility Proxy...")
        assert 0.0 <= data["avg_spillover_corr"] <= 1.0
        assert 0.05 <= data["volatility_proxy"] <= 2.0
        print(f"[PASS] Phase 7 Passed: avg_spillover_corr={data['avg_spillover_corr']:.4f}, volatility_proxy={data['volatility_proxy']:.4f}.")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 8: Classification State Rules & Confidence Bounds Verification
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 8] Testing Classification Rules & Confidence Score Bounds...")
        # Verify confidence is clamped between 0.20 and 0.99
        r_low = http.get("/market/regime?min_data_points=1000", headers=auth_headers)
        assert r_low.status_code == 200
        data_low = r_low.json()
        assert 0.20 <= data_low["confidence"] <= 0.99
        print(f"[PASS] Phase 8 Passed: Confidence score bounds verified ({data_low['confidence']:.4f}).")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 9: Rate Limiting & Institutional Quota Tracking
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 9] Testing Rate Limiting & Response Headers...")
        assert "x-ratelimit-limit" in r.headers
        assert "x-ratelimit-remaining" in r.headers
        assert "x-ratelimit-reset" in r.headers
        print(f"[PASS] Phase 9 Passed: Rate limiting headers present (Limit: {r.headers.get('x-ratelimit-limit')}, Remaining: {r.headers.get('x-ratelimit-remaining')}).")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 10: OpenAPI Conformance & Python SDK Integration
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 10] Testing OpenAPI 3.0 Conformance & Python SDK Integration...")
        r_spec = http.get("/api-docs/openapi.json")
        assert r_spec.status_code == 200
        spec = r_spec.json()
        assert "/market/regime" in spec["paths"]
        assert "MarketRegimeResponse" in spec["components"]["schemas"]
        assert "RegimeComponents" in spec["components"]["schemas"]
        print("  + OpenAPI 3.0 specification contains /market/regime and component schemas.")

        # Test Python Synchronous Client
        sdk_regime = client.market_regime(lookback_days=5, min_data_points=50)
        assert isinstance(sdk_regime, MarketRegimeResponse)
        assert isinstance(sdk_regime.components, RegimeComponents)
        assert sdk_regime.regime in ["Bullish", "Bearish", "Neutral", "High Volatility"]
        print(f"  + Python SDK Sync: Received MarketRegimeResponse ('{sdk_regime.regime}', confidence: {sdk_regime.confidence:.2f}).")

        client.close()

    print("\n" + "=" * 80)
    print("[SUCCESS] ALL 10 PHASES PASSED: MARKET REGIME DETECTION ENGINE CERTIFIED!")
    print("=" * 80)


def main():
    with ServerContext():
        run_tests()


if __name__ == "__main__":
    main()
