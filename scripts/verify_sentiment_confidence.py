#!/usr/bin/env python3
"""
=====================================================================================
FinText-Alpha-Vectorizer — Sentiment Confidence & Class Probabilities Verification
=====================================================================================
Validates that:
 1. GET /sentiment returns 'confidence' (0.0 to 1.0) and 'probabilities' object.
 2. Class probabilities sum to 1.0 and contain positive, neutral, negative fields.
 3. OpenAPI 3.0 specification includes SentimentProbabilities schema.
 4. Python SDK correctly deserializes confidence and probabilities.
=====================================================================================
"""

import os
import sys
import time
import subprocess
import requests
from fintext import FinTextClient

SERVER_EXE = os.path.abspath("rust/target/release/fintext_api.exe")
BASE_URL = "http://127.0.0.1:8000"
DEV_ADMIN_TOKEN = "fintext-admin-dev-secret-token"

def start_server() -> subprocess.Popen:
    env = os.environ.copy()
    env["QUESTDB_MOCK_FALLBACK"] = "1"
    env["PORT"] = "8000"
    env["ADMIN_TOKEN"] = DEV_ADMIN_TOKEN
    env["JWT_SECRET"] = "fintext-alpha-vectorizer-institutional-jwt-secret-key-2026"
    env["PIT_DATA_ENABLED"] = "1"
    env["PIT_DATA_DIR"] = os.path.abspath("config")
    
    proc = subprocess.Popen(
        [SERVER_EXE],
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True
    )
    
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

def main():
    print("=" * 85)
    print(" FinText-Alpha-Vectorizer — Sentiment Confidence & Probabilities Verification")
    print("=" * 85)
    
    print(f"[*] Starting API Server binary: {SERVER_EXE}")
    proc = start_server()
    try:
        # 1. Obtain JWT
        print("[1/5] Obtaining institutional JWT via POST /auth/token...")
        r_auth = requests.post(
            f"{BASE_URL}/auth/token",
            headers={"X-Admin-Token": DEV_ADMIN_TOKEN, "Content-Type": "application/json"},
            json={"user_id": "quant_ml_specialist", "tier": "institutional"}
        )
        assert r_auth.status_code == 200, f"Failed auth: {r_auth.text}"
        token = r_auth.json()["token"]
        headers = {"Authorization": f"Bearer {token}"}
        print("      [OK] Token acquired for 'quant_ml_specialist'")
        
        # 2. Query GET /sentiment?ticker=AAPL
        print("\n[2/5] Querying GET /sentiment?ticker=AAPL...")
        r_sent = requests.get(f"{BASE_URL}/sentiment?ticker=AAPL", headers=headers)
        assert r_sent.status_code == 200, f"Expected 200, got {r_sent.status_code}: {r_sent.text}"
        data = r_sent.json()
        print(f"      Response Payload: {data}")
        
        # 3. Verify Confidence & Probabilities Schema
        print("\n[3/5] Validating Confidence & Probabilities Fields...")
        assert "confidence" in data, "Missing 'confidence' field in sentiment response"
        assert "probabilities" in data, "Missing 'probabilities' field in sentiment response"
        
        conf = data["confidence"]
        assert isinstance(conf, (int, float)), f"Expected float confidence, got {type(conf)}"
        assert 0.0 <= conf <= 1.0, f"Confidence {conf} out of range [0.0, 1.0]"
        print(f"      [OK] Confidence score: {conf:.4f} (Valid in [0.0, 1.0])")
        
        probs = data["probabilities"]
        assert probs is not None, "Expected non-null probabilities object"
        assert "positive" in probs and "neutral" in probs and "negative" in probs
        p_pos, p_neu, p_neg = probs["positive"], probs["neutral"], probs["negative"]
        assert 0.0 <= p_pos <= 1.0 and 0.0 <= p_neu <= 1.0 and 0.0 <= p_neg <= 1.0
        prob_sum = p_pos + p_neu + p_neg
        assert abs(prob_sum - 1.0) < 0.01, f"Probabilities sum to {prob_sum}, expected 1.0"
        print(f"      [OK] Class Probabilities: Positive={p_pos:.4f}, Neutral={p_neu:.4f}, Negative={p_neg:.4f} (Sum: {prob_sum:.4f})")
        
        # 4. OpenAPI Specification Check
        print("\n[4/5] Checking OpenAPI 3.0 Schema Definition...")
        r_spec = requests.get(f"{BASE_URL}/api-docs/openapi.json")
        assert r_spec.status_code == 200
        spec = r_spec.json()
        schemas = spec.get("components", {}).get("schemas", {})
        assert "SentimentProbabilities" in schemas, "SentimentProbabilities missing from OpenAPI components.schemas"
        assert "confidence" in schemas["SentimentResponse"]["properties"], "confidence missing in SentimentResponse schema"
        assert "probabilities" in schemas["SentimentResponse"]["properties"], "probabilities missing in SentimentResponse schema"
        print("      [OK] OpenAPI spec correctly documents SentimentProbabilities and SentimentResponse fields")
        
        # 5. Official Python Client SDK Integration Check
        print("\n[5/5] Testing Official Python Client SDK with Confidence & Probabilities...")
        sdk_client = FinTextClient(base_url=BASE_URL, api_token=token)
        sdk_res = sdk_client.sentiment("AAPL")
        assert sdk_res.ticker == "AAPL"
        assert 0.0 <= sdk_res.confidence <= 1.0
        assert sdk_res.probabilities is not None
        assert abs(sdk_res.probabilities.positive + sdk_res.probabilities.neutral + sdk_res.probabilities.negative - 1.0) < 0.01
        print(f"      [OK] Python SDK parsed sentiment successfully: confidence={sdk_res.confidence:.4f}, label={sdk_res.sentiment_label}")
        
    finally:
        proc.terminate()
        proc.wait()

    print("\n" + "=" * 85)
    print(" [OK] ALL SENTIMENT CONFIDENCE & PROBABILITY CHECKS PASSED!")
    print("=" * 85)

if __name__ == "__main__":
    main()
