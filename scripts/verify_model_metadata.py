"""
================================================================================
🚀 FINTEXT ALPHA VECTORIZER — TEST SUITE #213: MODEL VERSIONING & DATA PROVENANCE
================================================================================

Comprehensive automated verification suite for:
- Point-in-time sentiment model metadata tagging (`model_version`, `pipeline_version`, `data_provenance`)
- Historical sentiment time-series records & metadata lineage
- Real-time news sentiment feed items & envelope provenance
- GICS Sector aggregate sentiment lineage
- Batch multi-ticker sentiment envelope & constituent item lineage
- Statistical sentiment anomalies scanner & item lineage
- Multi-source sentiment disagreement index lineage
- Entity sentiment breakdown analytics & item lineage
- Dynamic environment variable configuration overrides (MODEL_VERSION, PIPELINE_VERSION, DATA_PROVENANCE)
- Backward compatibility with legacy schema (missing optional fields parse as None)
- OpenAPI 3.0 component schemas & response property registration
- Python SDK async & sync client end-to-end integration
================================================================================
"""

import os
from pathlib import Path
import subprocess
import sys
import time
import httpx
import anyio

PROJECT_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(PROJECT_ROOT / "python_sdk" / "src"))

from fintext import (
    FinTextClient,
    FinTextAsyncClient,
    ModelMetadata,
    SentimentResponse,
    SentimentHistoryResponse,
    SentimentFeedResponse,
    SentimentAnomaliesResponse,
    SectorSentimentResponse,
    BatchSentimentResponse,
    SentimentDisagreementResponse,
    EntitySentimentResponse,
)

if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")

RELEASE_EXE = PROJECT_ROOT / "rust" / "target" / "release" / "fintext_api.exe"
DEBUG_EXE = PROJECT_ROOT / "rust" / "target" / "debug" / "fintext_api.exe"
SERVER_EXE = RELEASE_EXE if RELEASE_EXE.exists() else DEBUG_EXE

PORT_1 = 8110
PORT_2 = 8111
PORT_3 = 8112
BASE_URL_1 = f"http://127.0.0.1:{PORT_1}"
BASE_URL_2 = f"http://127.0.0.1:{PORT_2}"
BASE_URL_3 = f"http://127.0.0.1:{PORT_3}"
ADMIN_TOKEN = "dev_admin_secret_token_123"

passed = 0
failed = 0


def check(desc: str, condition: bool, extra_info: str = ""):
    global passed, failed
    if condition:
        passed += 1
        print(f"  [PASS] {desc}")
    else:
        failed += 1
        print(f"  [FAIL] {desc} -> {extra_info}")
        raise AssertionError(f"Check failed: {desc} ({extra_info})")


def get_auth_token(base_url: str) -> str:
    r = httpx.post(
        f"{base_url}/auth/token",
        json={"user_id": "metadata_auditor_01", "role": "institutional", "expires_in_seconds": 3600},
        headers={"X-Admin-Token": ADMIN_TOKEN},
        timeout=10.0,
    )
    assert r.status_code == 200, f"Failed to get auth token: {r.text}"
    return r.json()["token"]


def run_tests():
    global passed, failed
    print("=" * 80)
    print("🚀 STARTING SUITE #213: MODEL VERSIONING & DATA PROVENANCE VERIFICATION")
    print("=" * 80)

    # --------------------------------------------------------------------------
    # PART 1: Default Configuration Verification
    # --------------------------------------------------------------------------
    print(f"\n[STARTING] Spawning FinText API Server on port {PORT_1}...")
    env1 = os.environ.copy()
    env1["PORT"] = str(PORT_1)
    env1["HOST"] = "127.0.0.1"
    env1["ADMIN_TOKEN"] = ADMIN_TOKEN
    env1["JWT_SECRET"] = "super_secret_test_jwt_key_32_bytes_len!!"
    env1["QUESTDB_MOCK_FALLBACK"] = "1"
    env1["POLYGON_MOCK_FALLBACK"] = "1"
    env1["WHISPER_MOCK_FALLBACK"] = "1"
    env1["KAFKA_MOCK_FALLBACK"] = "1"
    env1["KAFKA_MOCK_MODE"] = "1"

    proc1 = subprocess.Popen([str(SERVER_EXE)], env=env1)
    try:
        start_t = time.time()
        ready = False
        while time.time() - start_t < 15:
            try:
                r = httpx.get(f"{BASE_URL_1}/health", timeout=1.0)
                if r.status_code == 200:
                    ready = True
                    break
            except Exception:
                time.sleep(0.3)
        if not ready:
            raise RuntimeError(f"Server on port {PORT_1} failed to respond on /health within 15s.")
        print(f"[READY] Server listening on {BASE_URL_1}")

        token1 = get_auth_token(BASE_URL_1)
        headers1 = {"Authorization": f"Bearer {token1}"}
        client1 = FinTextClient(base_url=BASE_URL_1, api_token=token1)

        with httpx.Client(base_url=BASE_URL_1, timeout=10.0) as http:
            # Phase 1: Point-in-Time Sentiment (/sentiment)
            print("\n--- PHASE 1: Point-in-Time Sentiment Metadata Lineage ---")
            r = http.get("/sentiment?ticker=AAPL", headers=headers1)
            check("GET /sentiment returns 200 OK", r.status_code == 200)
            data = r.json()
            check("model_version is 'finbert-minilm-v2.1'", data.get("model_version") == "finbert-minilm-v2.1", str(data))
            check("pipeline_version is '2.0.0'", data.get("pipeline_version") == "2.0.0", str(data))
            check("data_provenance contains expected sources", data.get("data_provenance") == ["SEC EDGAR", "Finnhub", "Polygon"], str(data))

            # SDK Deserialization
            sdk_sentiment = client1.sentiment(ticker="AAPL")
            check("SDK SentimentResponse has model_version", sdk_sentiment.model_version == "finbert-minilm-v2.1")
            check("SDK SentimentResponse has pipeline_version", sdk_sentiment.pipeline_version == "2.0.0")
            check("SDK SentimentResponse has data_provenance", sdk_sentiment.data_provenance == ["SEC EDGAR", "Finnhub", "Polygon"])

            # Phase 2: Historical Sentiment Time-Series (/sentiment/history)
            print("\n--- PHASE 2: Historical Sentiment Time-Series Metadata ---")
            r = http.get("/sentiment/history?ticker=AAPL&start_date=2026-08-01&end_date=2026-08-30&limit=5", headers=headers1)
            check("GET /sentiment/history returns 200 OK", r.status_code == 200)
            hist_data = r.json()
            check("History envelope has model_version", hist_data.get("model_version") == "finbert-minilm-v2.1")
            check("History envelope has pipeline_version", hist_data.get("pipeline_version") == "2.0.0")
            check("History envelope has data_provenance", hist_data.get("data_provenance") == ["SEC EDGAR", "Finnhub", "Polygon"])

            records = hist_data.get("records", [])
            check("History returns non-empty records", len(records) > 0)
            if records:
                first_rec = records[0]
                check("Individual record has model_version", first_rec.get("model_version") == "finbert-minilm-v2.1")
                check("Individual record has pipeline_version", first_rec.get("pipeline_version") == "2.0.0")
                check("Individual record has data_provenance", bool(first_rec.get("data_provenance")))

            sdk_hist = client1.sentiment_history(ticker="AAPL", start_date="2026-08-01", end_date="2026-08-30", limit=5)
            check("SDK SentimentHistoryResponse has model_version", sdk_hist.model_version == "finbert-minilm-v2.1")
            check("SDK SentimentHistoryResponse record has data_provenance", len(sdk_hist.records) > 0 and bool(sdk_hist.records[0].data_provenance))

            # Phase 3: Real-Time News Sentiment Feed (/sentiment/feed)
            print("\n--- PHASE 3: Real-Time News Sentiment Feed Metadata ---")
            r = http.get("/sentiment/feed?limit=5", headers=headers1)
            check("GET /sentiment/feed returns 200 OK", r.status_code == 200)
            feed_data = r.json()
            check("Feed envelope has model_version", feed_data.get("model_version") == "finbert-minilm-v2.1")
            check("Feed envelope has pipeline_version", feed_data.get("pipeline_version") == "2.0.0")
            check("Feed envelope has data_provenance", feed_data.get("data_provenance") == ["SEC EDGAR", "Finnhub", "Polygon"])

            feed_items = feed_data.get("records", [])
            check("Feed returns non-empty records", len(feed_items) > 0)
            if feed_items:
                first_item = feed_items[0]
                check("Feed item has model_version", first_item.get("model_version") == "finbert-minilm-v2.1")
                check("Feed item has pipeline_version", first_item.get("pipeline_version") == "2.0.0")
                check("Feed item has data_provenance", bool(first_item.get("data_provenance")))

            sdk_feed = client1.sentiment_feed(limit=5)
            check("SDK SentimentFeedResponse has model_version", sdk_feed.model_version == "finbert-minilm-v2.1")
            check("SDK SentimentFeedItem has model_version", len(sdk_feed.records) > 0 and sdk_feed.records[0].model_version == "finbert-minilm-v2.1")

            # Phase 4: GICS Sector Aggregate Sentiment (/sentiment/sector)
            print("\n--- PHASE 4: GICS Sector Aggregate Sentiment Metadata ---")
            r = http.get("/sentiment/sector?sector=Technology&start_date=2026-08-01&end_date=2026-08-30", headers=headers1)
            check("GET /sentiment/sector returns 200 OK", r.status_code == 200)
            sec_data = r.json()
            check("Sector envelope has model_version", sec_data.get("model_version") == "finbert-minilm-v2.1")
            check("Sector envelope has pipeline_version", sec_data.get("pipeline_version") == "2.0.0")
            check("Sector envelope has data_provenance", sec_data.get("data_provenance") == ["SEC EDGAR", "Finnhub", "Polygon"])

            sdk_sec = client1.sector_sentiment(sector="Technology", start_date="2026-08-01", end_date="2026-08-30")
            check("SDK SectorSentimentResponse has model_version", sdk_sec.model_version == "finbert-minilm-v2.1")

            # Phase 5: Batch Multi-Ticker Sentiment (/sentiment/batch)
            print("\n--- PHASE 5: Batch Multi-Ticker Sentiment Metadata ---")
            r = http.get("/sentiment/batch?tickers=AAPL,MSFT,NVDA", headers=headers1)
            check("GET /sentiment/batch returns 200 OK", r.status_code == 200)
            batch_data = r.json()
            check("Batch envelope has model_version", batch_data.get("model_version") == "finbert-minilm-v2.1")
            check("Batch envelope has pipeline_version", batch_data.get("pipeline_version") == "2.0.0")
            check("Batch envelope has data_provenance", batch_data.get("data_provenance") == ["SEC EDGAR", "Finnhub", "Polygon"])

            batch_results = batch_data.get("results", [])
            check("Batch returns 3 results", len(batch_results) == 3)
            if batch_results:
                check("Batch item has model_version", batch_results[0].get("model_version") == "finbert-minilm-v2.1")

            sdk_batch = client1.batch_sentiment(tickers=["AAPL", "MSFT", "NVDA"])
            check("SDK BatchSentimentResponse has model_version", sdk_batch.model_version == "finbert-minilm-v2.1")
            check("SDK BatchSentimentResponse result has model_version", len(sdk_batch.results) == 3 and sdk_batch.results[0].model_version == "finbert-minilm-v2.1")

            # Phase 6: Statistical Sentiment Anomalies Scanner (/sentiment/anomalies)
            print("\n--- PHASE 6: Statistical Sentiment Anomalies Scanner Metadata ---")
            r = http.get("/sentiment/anomalies?lookback_days=30&limit=5", headers=headers1)
            check("GET /sentiment/anomalies returns 200 OK", r.status_code == 200)
            anom_data = r.json()
            check("Anomalies envelope has model_version", anom_data.get("model_version") == "finbert-minilm-v2.1")
            check("Anomalies envelope has pipeline_version", anom_data.get("pipeline_version") == "2.0.0")
            check("Anomalies envelope has data_provenance", anom_data.get("data_provenance") == ["SEC EDGAR", "Finnhub", "Polygon"])

            anom_items = anom_data.get("items", [])
            if anom_items:
                check("Anomaly item has model_version", anom_items[0].get("model_version") == "finbert-minilm-v2.1")

            sdk_anom = client1.sentiment_anomalies(lookback_days=30, limit=5)
            check("SDK SentimentAnomaliesResponse has model_version", sdk_anom.model_version == "finbert-minilm-v2.1")

            # Phase 7: Multi-Source Sentiment Disagreement Index (/sentiment/disagreement)
            print("\n--- PHASE 7: Sentiment Disagreement Index Metadata ---")
            r = http.get("/sentiment/disagreement?ticker=AAPL&start_date=2025-01-01&end_date=2025-03-31&aggregation=stddev", headers=headers1)
            check("GET /sentiment/disagreement returns 200 OK", r.status_code == 200)
            dis_data = r.json()
            check("Disagreement envelope has model_version", dis_data.get("model_version") == "finbert-minilm-v2.1")
            check("Disagreement envelope has pipeline_version", dis_data.get("pipeline_version") == "2.0.0")
            check("Disagreement envelope has data_provenance", dis_data.get("data_provenance") == ["SEC EDGAR", "Finnhub", "Polygon"])

            sdk_dis = client1.sentiment_disagreement(ticker="AAPL", start_date="2025-01-01", end_date="2025-03-31")
            check("SDK SentimentDisagreementResponse has model_version", sdk_dis.model_version == "finbert-minilm-v2.1")

            # Phase 8: Entity Sentiment Breakdown Analytics (/sentiment/entities)
            print("\n--- PHASE 8: Entity Sentiment Breakdown Metadata ---")
            r = http.get("/sentiment/entities?limit=5", headers=headers1)
            check("GET /sentiment/entities returns 200 OK", r.status_code == 200)
            ent_data = r.json()
            check("Entities envelope has model_version", ent_data.get("model_version") == "finbert-minilm-v2.1")
            check("Entities envelope has pipeline_version", ent_data.get("pipeline_version") == "2.0.0")
            check("Entities envelope has data_provenance", ent_data.get("data_provenance") == ["SEC EDGAR", "Finnhub", "Polygon"])

            ent_items = ent_data.get("entities", [])
            check("Entities returns valid list", isinstance(ent_items, list))
            if ent_items:
                check("Entity item has model_version", ent_items[0].get("model_version") == "finbert-minilm-v2.1")

            sdk_ent = client1.sentiment_entities(limit=5)
            check("SDK EntitySentimentResponse has model_version", sdk_ent.model_version == "finbert-minilm-v2.1")

            # Phase 9: OpenAPI 3.0 Component Schemas & Property Registration
            print("\n--- PHASE 9: OpenAPI 3.0 Documentation Registration ---")
            r = http.get("/api-docs/openapi.json")
            check("GET /api-docs/openapi.json returns 200 OK", r.status_code == 200)
            spec = r.json()
            schemas = spec.get("components", {}).get("schemas", {})
            check("ModelMetadata schema registered in OpenAPI spec", "ModelMetadata" in schemas)
            sentiment_resp_schema = schemas.get("SentimentResponse", {}).get("properties", {})
            check("model_version property present in SentimentResponse schema", "model_version" in sentiment_resp_schema)
            check("pipeline_version property present in SentimentResponse schema", "pipeline_version" in sentiment_resp_schema)
            check("data_provenance property present in SentimentResponse schema", "data_provenance" in sentiment_resp_schema)

            # Phase 10: Backward Compatibility Parsing
            print("\n--- PHASE 10: Backward Compatibility Pydantic Parsing ---")
            legacy_raw = {
                "ticker": "TSLA",
                "date": "2026-08-30",
                "sentiment_score": -0.42,
                "sentiment_label": "BEARISH",
                "confidence": 0.81,
                "signal_available_ts_us": 1787940389786186,
                "data_quality_score": 0.85,
                "message": "Legacy signal",
            }
            legacy_obj = SentimentResponse.model_validate(legacy_raw)
            check("Legacy payload parses without errors", legacy_obj.ticker == "TSLA")
            check("Legacy payload model_version is None", legacy_obj.model_version is None)
            check("Legacy payload pipeline_version is None", legacy_obj.pipeline_version is None)
            check("Legacy payload data_provenance is None", legacy_obj.data_provenance is None)

    finally:
        print(f"[STOPPING] Terminating server on port {PORT_1}...")
        proc1.terminate()
        try:
            proc1.wait(timeout=5)
        except subprocess.TimeoutExpired:
            proc1.kill()
        print("[STOPPED] Server terminated cleanly.")

    # --------------------------------------------------------------------------
    # PART 2: Environment Variable Overrides Verification
    # --------------------------------------------------------------------------
    print("\n--- PHASE 11: Dynamic Environment Variable Overrides ---")
    env2 = env1.copy()
    env2["PORT"] = str(PORT_2)
    env2["MODEL_VERSION"] = "finbert-v3.2-institutional"
    env2["PIPELINE_VERSION"] = "3.5.0"
    env2["DATA_PROVENANCE"] = "Bloomberg, SEC EDGAR, Reuters, FactSet"

    print(f"\n[STARTING] Spawning FinText API Server on port {PORT_2} (overridden env)...")
    proc2 = subprocess.Popen([str(SERVER_EXE)], env=env2)
    try:
        start_t = time.time()
        ready = False
        while time.time() - start_t < 15:
            try:
                r = httpx.get(f"{BASE_URL_2}/health", timeout=1.0)
                if r.status_code == 200:
                    ready = True
                    break
            except Exception:
                time.sleep(0.3)
        if not ready:
            raise RuntimeError(f"Server on port {PORT_2} failed to respond on /health within 15s.")
        print(f"[READY] Server listening on {BASE_URL_2}")

        token2 = get_auth_token(BASE_URL_2)
        headers2 = {"Authorization": f"Bearer {token2}"}

        with httpx.Client(base_url=BASE_URL_2, timeout=10.0) as http_override:
            r = http_override.get("/sentiment?ticker=NVDA", headers=headers2)
            check("GET /sentiment on overridden server returns 200", r.status_code == 200)
            data = r.json()
            check("Custom model_version is applied", data.get("model_version") == "finbert-v3.2-institutional", str(data))
            check("Custom pipeline_version is applied", data.get("pipeline_version") == "3.5.0", str(data))
            check(
                "Custom data_provenance is parsed as array",
                data.get("data_provenance") == ["Bloomberg", "SEC EDGAR", "Reuters", "FactSet"],
                str(data),
            )

            r_feed = http_override.get("/sentiment/feed?limit=3", headers=headers2)
            check("Custom feed envelope has overridden model_version", r_feed.json().get("model_version") == "finbert-v3.2-institutional")

    finally:
        print(f"[STOPPING] Terminating server on port {PORT_2}...")
        proc2.terminate()
        try:
            proc2.wait(timeout=5)
        except subprocess.TimeoutExpired:
            proc2.kill()
        print("[STOPPED] Server terminated cleanly.")

    # --------------------------------------------------------------------------
    # PART 3: Async Client Integration
    # --------------------------------------------------------------------------
    print("\n--- PHASE 12: Python SDK Async Client Integration ---")
    env3 = env1.copy()
    env3["PORT"] = str(PORT_3)

    print(f"\n[STARTING] Spawning FinText API Server on port {PORT_3} (async client)...")
    proc3 = subprocess.Popen([str(SERVER_EXE)], env=env3)
    try:
        start_t = time.time()
        ready = False
        while time.time() - start_t < 15:
            try:
                r = httpx.get(f"{BASE_URL_3}/health", timeout=1.0)
                if r.status_code == 200:
                    ready = True
                    break
            except Exception:
                time.sleep(0.3)
        if not ready:
            raise RuntimeError(f"Server on port {PORT_3} failed to respond on /health within 15s.")
        print(f"[READY] Server listening on {BASE_URL_3}")

        token3 = get_auth_token(BASE_URL_3)

        async def async_test():
            async with FinTextAsyncClient(base_url=BASE_URL_3, api_token=token3) as async_client:
                sent = await async_client.sentiment(ticker="MSFT")
                check("Async client sentiment has model_version", sent.model_version == "finbert-minilm-v2.1")
                check("Async client sentiment has data_provenance", sent.data_provenance == ["SEC EDGAR", "Finnhub", "Polygon"])

                batch = await async_client.batch_sentiment(tickers=["AAPL", "GOOGL"])
                check("Async client batch_sentiment has model_version", batch.model_version == "finbert-minilm-v2.1")

                feed = await async_client.sentiment_feed(limit=3)
                check("Async client sentiment_feed has model_version", feed.model_version == "finbert-minilm-v2.1")

        anyio.run(async_test)

    finally:
        print(f"[STOPPING] Terminating server on port {PORT_3}...")
        proc3.terminate()
        try:
            proc3.wait(timeout=5)
        except subprocess.TimeoutExpired:
            proc3.kill()
        print("[STOPPED] Server terminated cleanly.")

    print("\n" + "=" * 80)
    print(f"🎉 SUITE #213 VERIFICATION COMPLETE: {passed} PASSED, {failed} FAILED")
    print("=" * 80)
    if failed > 0:
        sys.exit(1)


if __name__ == "__main__":
    run_tests()
