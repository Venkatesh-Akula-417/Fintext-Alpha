#!/usr/bin/env python3
"""
================================================================================
FinText-Alpha-Vectorizer -- Data Quality Scoring & Filtering Verifier
================================================================================
Verifies the quantitative data quality scoring algorithm, model schema
enrichment, minimum quality filtering on historical sentiment time series and
streamed CSV export, as well as single and batch sentiment quality metrics.
================================================================================
"""

import sys
import json
import time
from pathlib import Path
from typing import Any, Dict

# Ensure local python_sdk source is discoverable
SDK_SRC = Path(__file__).resolve().parent.parent / "python_sdk" / "src"
if str(SDK_SRC) not in sys.path:
    sys.path.insert(0, str(SDK_SRC))

# Color formatting
GREEN = "\033[92m"
RED = "\033[91m"
YELLOW = "\033[93m"
CYAN = "\033[96m"
BOLD = "\033[1m"
RESET = "\033[0m"

def print_header(title: str):
    print(f"\n{CYAN}{BOLD}{'=' * 80}{RESET}")
    print(f"{CYAN}{BOLD} {title}{RESET}")
    print(f"{CYAN}{BOLD}{'=' * 80}{RESET}")

def test_passed(name: str, detail: str = ""):
    det = f" ({detail})" if detail else ""
    print(f"  [{GREEN}PASS{RESET}] {name}{det}")

def test_failed(name: str, reason: str):
    print(f"  [{RED}FAIL{RESET}] {name} - {reason}")

def compute_quality_score_py(source: str, title: str, confidence: float) -> float:
    """Python reference implementation of the Rust scoring formula."""
    source_map = {
        "SEC EDGAR": 0.95,
        "SEC_EDGAR": 0.95,
        "POLYGON": 0.90,
        "POLYGON.IO": 0.90,
        "INSTITUTIONAL WIRE": 0.85,
        "WIRE": 0.85,
        "FINNHUB": 0.80,
        "TIINGO": 0.80,
        "ALPHAVANTAGE": 0.75,
        "ALPHA VANTAGE": 0.75,
        "MOCK": 0.70,
        "NEWSAPI": 0.60,
        "RSS": 0.50,
        "RSS FEEDS": 0.50,
    }
    src_rel = source_map.get(source.strip().upper(), 0.60)
    
    t_len = len(title.strip())
    if t_len == 0:
        len_fac = 0.1
    elif t_len < 15:
        len_fac = 0.3
    elif t_len < 50:
        len_fac = 0.7
    else:
        len_fac = 1.0

    spam_words = [
        "sponsored", "advertisement", "click here", "promoted",
        "buy now", "free trial", "subscribe now", "giveaway",
        "100% free", "guaranteed returns", "casino", "crypto pump", "risk-free"
    ]
    t_lower = title.lower()
    spam_penalty = 0.3 if any(w in t_lower for w in spam_words) else 0.0
    spam_fac = max(0.0, min(1.0, 1.0 - spam_penalty))
    conf_fac = max(0.0, min(1.0, confidence))

    raw_q = (0.4 * src_rel) + (0.2 * len_fac) + (0.2 * spam_fac) + (0.2 * conf_fac)
    return round(max(0.0, min(1.0, raw_q)), 4)


def run_tests():
    from fintext.client import FinTextClient
    from fintext.models import SentimentRecord, SentimentResponse, SentimentHistoryResponse
    import httpx

    print_header("FinText Data Quality Scoring & Filtering Verification Suite")

    passed = 0
    failed = 0

    # -------------------------------------------------------------------------
    # Test 1: Mathematical Formula Verification
    # -------------------------------------------------------------------------
    try:
        # SEC EDGAR 10-K report (clean, long, high confidence)
        q1 = compute_quality_score_py(
            "SEC EDGAR",
            "Apple Inc. Files Form 10-K Annual Report for Fiscal Year 2025 with Full Audited Financials",
            0.90,
        )
        assert q1 == 0.96, f"Expected 0.96, got {q1}"

        # RSS Clickbait spam
        q2 = compute_quality_score_py(
            "RSS",
            "Click here for sponsored trading tips and guaranteed returns",
            0.50,
        )
        assert q2 == 0.64, f"Expected 0.64, got {q2}"

        # Short headline
        q3 = compute_quality_score_py("Finnhub", "AAPL up", 0.70)
        assert q3 == 0.72, f"Expected 0.72, got {q3}"

        test_passed("Heuristic Scoring: Mathematical weight & factor verification", f"SEC={q1}, Spam={q2}, Short={q3}")
        passed += 1
    except Exception as e:
        test_failed("Heuristic Scoring: Mathematical weight & factor verification", str(e))
        failed += 1

    # -------------------------------------------------------------------------
    # Test 2: Pydantic Model Deserialization with data_quality_score
    # -------------------------------------------------------------------------
    try:
        record_json = {
            "published_utc": "2025-01-01T14:30:00.000000Z",
            "ticker": "AAPL",
            "source": "SEC EDGAR",
            "title": "Form 10-K Annual Report",
            "sentiment_score": 0.35,
            "vpin": 0.42,
            "gamma_exposure": 120000.0,
            "data_quality_score": 0.92,
        }
        rec = SentimentRecord.model_validate(record_json)
        assert rec.data_quality_score == 0.92
        assert rec.ticker == "AAPL"

        resp_json = {
            "ticker": "AAPL",
            "date": "2026-08-25",
            "sentiment_score": 0.25,
            "sentiment_label": "BULLISH",
            "confidence": 0.85,
            "signal_available_ts_us": 1787940389786186,
            "data_quality_score": 0.88,
            "message": "Point-in-time sentiment signal retrieved",
        }
        resp = SentimentResponse.model_validate(resp_json)
        assert resp.data_quality_score == 0.88
        assert resp.confidence == 0.85

        test_passed("Pydantic Models: Deserialization & validation of data_quality_score", "SentimentRecord & SentimentResponse")
        passed += 1
    except Exception as e:
        test_failed("Pydantic Models: Deserialization & validation of data_quality_score", str(e))
        failed += 1

    # -------------------------------------------------------------------------
    # Test 3: SDK Client Historical Query with min_quality Filtering
    # -------------------------------------------------------------------------
    try:
        def custom_handler(request: httpx.Request) -> httpx.Response:
            url_path = request.url.path
            if url_path == "/sentiment/history":
                min_q = float(request.url.params.get("min_quality", 0.0))
                all_records = [
                    {
                        "published_utc": "2025-01-01T14:30:00.000000Z",
                        "ticker": "AAPL",
                        "source": "SEC EDGAR",
                        "title": "Form 10-K Annual Report",
                        "sentiment_score": 0.35,
                        "vpin": 0.42,
                        "gamma_exposure": 120000.0,
                        "data_quality_score": 0.95,
                    },
                    {
                        "published_utc": "2025-01-02T14:30:00.000000Z",
                        "ticker": "AAPL",
                        "source": "RSS",
                        "title": "Click here for stock picks",
                        "sentiment_score": 0.10,
                        "vpin": 0.65,
                        "gamma_exposure": 0.0,
                        "data_quality_score": 0.52,
                    },
                ]
                filtered = [r for r in all_records if r["data_quality_score"] >= min_q]
                return httpx.Response(
                    200,
                    json={
                        "ticker": "AAPL",
                        "start_date": "2025-01-01",
                        "end_date": "2025-01-10",
                        "count": len(filtered),
                        "total": len(filtered),
                        "limit": 100,
                        "offset": 0,
                        "sort": "asc",
                        "records": filtered,
                    },
                )
            elif url_path == "/export/csv":
                min_q = float(request.url.params.get("min_quality", 0.0))
                header = "published_utc,ticker,source,title,sentiment_score,vpin,gamma_exposure,data_quality_score\r\n"
                row1 = "2025-01-01T14:30:00.000000Z,AAPL,SEC EDGAR,Form 10-K,0.3500,0.4200,120000,0.9500\r\n"
                row2 = "2025-01-02T14:30:00.000000Z,AAPL,RSS,Click here,0.1000,0.6500,0,0.5200\r\n"
                rows = header
                if 0.95 >= min_q:
                    rows += row1
                if 0.52 >= min_q:
                    rows += row2
                return httpx.Response(200, text=rows, headers={"Content-Type": "text/csv"})
            elif url_path == "/sentiment":
                return httpx.Response(
                    200,
                    json={
                        "ticker": "AAPL",
                        "date": "2026-08-25",
                        "sentiment_score": 0.25,
                        "sentiment_label": "BULLISH",
                        "confidence": 0.85,
                        "signal_available_ts_us": 1787940389786186,
                        "data_quality_score": 0.88,
                        "message": "Point-in-time sentiment signal retrieved",
                    },
                )
            return httpx.Response(404, json={"error": "Not Found"})

        transport = httpx.MockTransport(custom_handler)
        client = FinTextClient(base_url="http://mock.fintext", api_token="test_jwt", transport=transport)

        # 1. Query history with min_quality = 0.80 (should only return SEC EDGAR record)
        hist = client.sentiment_history("AAPL", "2025-01-01", "2025-01-10", min_quality=0.80)
        assert hist.count == 1
        assert hist.records[0].source == "SEC EDGAR"
        assert hist.records[0].data_quality_score == 0.95

        # 2. Export CSV with min_quality = 0.80
        csv_data = client.export_csv("AAPL", "2025-01-01", "2025-01-10", min_quality=0.80)
        assert "data_quality_score" in csv_data
        assert "SEC EDGAR" in csv_data
        assert "RSS" not in csv_data

        # 3. Single ticker sentiment
        sent = client.sentiment("AAPL")
        assert sent.data_quality_score == 0.88

        test_passed("SDK Client: min_quality filtering & CSV export integration", "History count=1 filtered, CSV includes quality header")
        passed += 1
    except Exception as e:
        test_failed("SDK Client: min_quality filtering & CSV export integration", str(e))
        failed += 1

    # -------------------------------------------------------------------------
    # Summary
    # -------------------------------------------------------------------------
    print_header("Data Quality Verification Summary")
    print(f" Total Tests Run: {passed + failed}")
    print(f" Passed: {passed}")
    print(f" Failed: {failed}")

    if failed == 0:
        print(f"\n {GREEN}{BOLD}ALL DATA QUALITY SCORING & FILTERING VERIFICATIONS PASSED!{RESET}\n")
        return 0
    else:
        print(f"\n {RED}{BOLD}SOME VERIFICATIONS FAILED!{RESET}\n")
        return 1


if __name__ == "__main__":
    sys.exit(run_tests())
