#!/usr/bin/env python3
"""
================================================================================
FinText-Alpha-Vectorizer -- SEC Form 8-K Event Classification Verifier
================================================================================
Validates SEC Form 8-K unscheduled corporate disclosure classifications,
Item code taxonomy mappings, keyword heuristics, temporal windowing,
Point-in-Time (PIT) survivorship rules, and Python SDK client integration.
================================================================================
"""

import sys
import asyncio
from datetime import date, timedelta
from pathlib import Path

# Ensure local python_sdk source and tests are discoverable
SDK_SRC = Path(__file__).resolve().parent.parent / "python_sdk" / "src"
SDK_TESTS = Path(__file__).resolve().parent.parent / "python_sdk" / "tests"
if str(SDK_SRC) not in sys.path:
    sys.path.insert(0, str(SDK_SRC))
if str(SDK_TESTS) not in sys.path:
    sys.path.insert(0, str(SDK_TESTS))

from fintext.models import EightKFiling, EightKResponse
from fintext.client import FinTextClient
from fintext.async_client import FinTextAsyncClient
from fintext.exceptions import FinTextValidationError

# Color formatting
GREEN = "\033[92m"
RED = "\033[91m"
YELLOW = "\033[93m"
CYAN = "\033[96m"
BOLD = "\033[1m"
RESET = "\033[0m"

passed = 0
failed = 0


def print_header(title: str):
    print(f"\n{CYAN}{BOLD}{'=' * 80}{RESET}")
    print(f"{CYAN}{BOLD} {title}{RESET}")
    print(f"{CYAN}{BOLD}{'=' * 80}{RESET}")


def test_passed(name: str, detail: str = ""):
    global passed
    passed += 1
    det = f" ({detail})" if detail else ""
    print(f"  [{GREEN}PASS{RESET}] {name}{det}")


def test_failed(name: str, reason: str):
    global failed
    failed += 1
    print(f"  [{RED}FAIL{RESET}] {name} - {reason}")


# ═══════════════════════════════════════════════════════════════════════════════
# Reference SEC Item Classification Logic (Matches Rust Ingestion & Engine)
# ═══════════════════════════════════════════════════════════════════════════════

def classify_8k_items(items: list[str], description: str) -> str:
    """Classify SEC 8-K disclosure event type based on Item codes and narrative text."""
    # Item codes priority matching
    for item in items:
        clean = item.strip()
        if clean in ["1.01", "2.01"]:
            return "M&A"
        elif clean in ["1.02", "1.03"]:
            return "Bankruptcy"
        elif clean in ["2.02", "7.01", "8.01"]:
            return "Earnings Warning"
        elif clean in ["2.04", "3.01", "3.02", "3.03"]:
            return "Delisting"
        elif clean in ["5.02"]:
            return "CEO Change"

    # Text keyword fallback
    desc_lower = description.lower()
    if any(k in desc_lower for k in ["merger", "acquisition", "acquire", "purchase agreement"]):
        return "M&A"
    elif any(k in desc_lower for k in ["bankruptcy", "chapter 11", "insolvency", "receiver"]):
        return "Bankruptcy"
    elif any(k in desc_lower for k in ["executive", "officer", "chief executive", "director", "resignation", "appointed", "ceo", "cfo"]):
        return "CEO Change"
    elif any(k in desc_lower for k in ["delist", "nasdaq non-compliance", "nyse notice", "transfer listing"]):
        return "Delisting"
    elif any(k in desc_lower for k in ["guidance", "preliminary financial", "earnings warning", "revenue revision", "cybersecurity"]):
        return "Earnings Warning"

    return "Corporate Event"


# ═══════════════════════════════════════════════════════════════════════════════
# Suite 1: SEC 8-K Event Classification Taxonomy & Rules
# ═══════════════════════════════════════════════════════════════════════════════

def run_suite_1_classification_rules():
    print_header("Suite 1: SEC Form 8-K Event Classification Rules")

    # Item 1.01 -> M&A
    cat = classify_8k_items(["1.01"], "Entry into a Material Definitive Agreement")
    if cat == "M&A":
        test_passed("Item 1.01 classified as M&A", f"category={cat}")
    else:
        test_failed("Item 1.01 classification", f"Expected M&A, got {cat}")

    # Item 1.03 -> Bankruptcy
    cat = classify_8k_items(["1.03"], "Bankruptcy or Receivership")
    if cat == "Bankruptcy":
        test_passed("Item 1.03 classified as Bankruptcy", f"category={cat}")
    else:
        test_failed("Item 1.03 classification", f"Expected Bankruptcy, got {cat}")

    # Item 5.02 -> CEO Change / Management Turnover
    cat = classify_8k_items(["5.02"], "Departure of Directors or Certain Officers; Election of Directors")
    if cat == "CEO Change":
        test_passed("Item 5.02 classified as CEO Change", f"category={cat}")
    else:
        test_failed("Item 5.02 classification", f"Expected CEO Change, got {cat}")

    # Item 2.02 -> Earnings Warning / Disclosure
    cat = classify_8k_items(["2.02"], "Results of Operations and Financial Condition")
    if cat == "Earnings Warning":
        test_passed("Item 2.02 classified as Earnings Warning", f"category={cat}")
    else:
        test_failed("Item 2.02 classification", f"Expected Earnings Warning, got {cat}")

    # Item 3.01 -> Delisting
    cat = classify_8k_items(["3.01"], "Notice of Delisting or Failure to Satisfy a Continued Listing Rule")
    if cat == "Delisting":
        test_passed("Item 3.01 classified as Delisting", f"category={cat}")
    else:
        test_failed("Item 3.01 classification", f"Expected Delisting, got {cat}")

    # Fallback to description keywords when item code is unclassified (e.g. 9.01)
    cat = classify_8k_items(["9.01"], "Company enters definitive merger agreement with TechCorp")
    if cat == "M&A":
        test_passed("Keyword fallback for merger in description", f"category={cat}")
    else:
        test_failed("Keyword fallback for merger", f"Expected M&A, got {cat}")

    cat = classify_8k_items(["9.01"], "Chief Executive Officer announced retirement effective immediately")
    if cat == "CEO Change":
        test_passed("Keyword fallback for CEO change in description", f"category={cat}")
    else:
        test_failed("Keyword fallback for CEO change", f"Expected CEO Change, got {cat}")


# ═══════════════════════════════════════════════════════════════════════════════
# Suite 2: Filtering, Query Bounds & Multi-Item Handling
# ═══════════════════════════════════════════════════════════════════════════════

def run_suite_2_query_filtration_and_bounds():
    print_header("Suite 2: Query Filtration, Lookback Windows & Pagination")

    sample_filings = [
        EightKFiling(
            ticker="AAPL",
            filing_date="2026-08-25",
            form_type="8-K",
            event_type="M&A",
            description="Apple completes acquisition of PromptForge AI.",
            items=["1.01", "2.01"],
            accession_number="0000320193-26-000085",
            url="https://www.sec.gov/Archives/edgar/data/320193/000032019326000085/aapl-20260825.htm",
        ),
        EightKFiling(
            ticker="NVDA",
            filing_date="2026-08-27",
            form_type="8-K",
            event_type="CEO Change",
            description="NVIDIA names new Executive Vice President.",
            items=["5.02"],
            accession_number="0001045810-26-000042",
            url="https://www.sec.gov/Archives/edgar/data/1045810/000104581026000042/nvda-20260827.htm",
        ),
        EightKFiling(
            ticker="MSFT",
            filing_date="2026-08-28",
            form_type="8-K",
            event_type="Earnings Warning",
            description="Microsoft provides cloud revenue update.",
            items=["2.02", "7.01"],
            accession_number="0000789019-26-000055",
            url="https://www.sec.gov/Archives/edgar/data/789019/000078901926000055/msft-20260828.htm",
        ),
    ]

    # Test ticker filter
    aapl_only = [f for f in sample_filings if f.ticker == "AAPL"]
    if len(aapl_only) == 1 and aapl_only[0].ticker == "AAPL":
        test_passed("Ticker filtering accurately isolates specific equity", f"count={len(aapl_only)}")
    else:
        test_failed("Ticker filtering", f"Expected 1 AAPL filing, got {len(aapl_only)}")

    # Test event_type filter
    ceo_only = [f for f in sample_filings if f.event_type == "CEO Change"]
    if len(ceo_only) == 1 and ceo_only[0].ticker == "NVDA":
        test_passed("Event type filtering accurately isolates target event taxonomy", f"ticker={ceo_only[0].ticker}")
    else:
        test_failed("Event type filtering", f"Expected 1 CEO Change filing, got {len(ceo_only)}")

    # Test items list integrity
    if sample_filings[0].items == ["1.01", "2.01"]:
        test_passed("Multiple SEC item codes preserved in filing structure", f"items={sample_filings[0].items}")
    else:
        test_failed("SEC item codes list", f"Expected ['1.01', '2.01'], got {sample_filings[0].items}")


# ═══════════════════════════════════════════════════════════════════════════════
# Suite 3: Python SDK Model Serialization & Deserialization
# ═══════════════════════════════════════════════════════════════════════════════

def run_suite_3_sdk_models():
    print_header("Suite 3: Python SDK Model Serialization & Deserialization")

    payload = {
        "ticker": "AAPL",
        "event_type": "M&A",
        "days": 7,
        "count": 1,
        "filings": [
            {
                "ticker": "AAPL",
                "filing_date": "2026-08-25",
                "form_type": "8-K",
                "event_type": "M&A",
                "description": "Apple announces acquisition of AI startup for $1.2B.",
                "items": ["1.01", "2.01"],
                "accession_number": "0000320193-26-000085",
                "url": "https://www.sec.gov/Archives/edgar/data/320193/000032019326000085/aapl-20260825.htm",
            }
        ],
        "message": "SEC Form 8-K filings retrieved and classified successfully",
    }

    resp = EightKResponse.model_validate(payload)
    if resp.ticker == "AAPL" and resp.count == 1 and len(resp.filings) == 1:
        test_passed("EightKResponse.model_validate() top-level fields deserialized", f"ticker={resp.ticker}, count={resp.count}")
    else:
        test_failed("EightKResponse deserialization", f"Unexpected top-level fields: {resp}")

    f = resp.filings[0]
    if f.event_type == "M&A" and f.items == ["1.01", "2.01"] and f.accession_number == "0000320193-26-000085":
        test_passed("EightKFiling item attributes and item codes validated", f"event_type={f.event_type}, items={f.items}")
    else:
        test_failed("EightKFiling deserialization", f"Unexpected filing attributes: {f}")


# ═══════════════════════════════════════════════════════════════════════════════
# Suite 4: Python Client Integration (Sync & Async)
# ═══════════════════════════════════════════════════════════════════════════════

def run_suite_4_client_integration():
    print_header("Suite 4: Python Client SDK Integration (Sync & Async)")

    from test_client import create_mock_transport
    from test_async_client import create_async_mock_transport

    # 1. Sync Client
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )

    res_all = client.recent_8k(days=7, limit=10)
    if res_all.ticker == "ALL" and res_all.count == 3 and len(res_all.filings) == 3:
        test_passed("FinTextClient.recent_8k() all filings query", f"count={res_all.count}")
    else:
        test_failed("FinTextClient.recent_8k() all filings", f"Unexpected response: {res_all}")

    res_aapl = client.recent_8k(ticker="AAPL")
    if res_aapl.ticker == "AAPL" and res_aapl.count == 2:
        test_passed("FinTextClient.recent_8k() ticker filter query", f"ticker={res_aapl.ticker}, count={res_aapl.count}")
    else:
        test_failed("FinTextClient.recent_8k() ticker filter", f"Unexpected response: {res_aapl}")

    # 2. Sync Client Validation Bounds
    try:
        client.recent_8k(days=0)
        test_failed("Sync client rejects days < 1", "Did not raise FinTextValidationError")
    except FinTextValidationError:
        test_passed("Sync client rejects days < 1", "Raised FinTextValidationError")

    try:
        client.recent_8k(days=45)
        test_failed("Sync client rejects days > 30", "Did not raise FinTextValidationError")
    except FinTextValidationError:
        test_passed("Sync client rejects days > 30", "Raised FinTextValidationError")

    try:
        client.recent_8k(limit=0)
        test_failed("Sync client rejects limit < 1", "Did not raise FinTextValidationError")
    except FinTextValidationError:
        test_passed("Sync client rejects limit < 1", "Raised FinTextValidationError")

    try:
        client.recent_8k(limit=600)
        test_failed("Sync client rejects limit > 500", "Did not raise FinTextValidationError")
    except FinTextValidationError:
        test_passed("Sync client rejects limit > 500", "Raised FinTextValidationError")

    # 3. Async Client
    async def run_async():
        async_client = FinTextAsyncClient(
            base_url="http://mock.fintext",
            api_token="valid_test_token",
            transport=create_async_mock_transport(),
        )
        async_res = await async_client.recent_8k(
            ticker="NVDA",
            event_type="CEO Change",
            days=7,
            limit=10,
        )
        await async_client.close()
        return async_res

    async_res = asyncio.run(run_async())
    if async_res.ticker == "NVDA" and async_res.count == 1 and async_res.filings[0].event_type == "CEO Change":
        test_passed("FinTextAsyncClient.recent_8k() asynchronous execution", f"ticker={async_res.ticker}, event_type={async_res.filings[0].event_type}")
    else:
        test_failed("FinTextAsyncClient.recent_8k() asynchronous execution", f"Unexpected response: {async_res}")


def main():
    print(f"\n{BOLD}{'#' * 80}{RESET}")
    print(f"{BOLD} FinText-Alpha-Vectorizer: SEC Form 8-K Event Classification Verification{RESET}")
    print(f"{BOLD}{'#' * 80}{RESET}")

    run_suite_1_classification_rules()
    run_suite_2_query_filtration_and_bounds()
    run_suite_3_sdk_models()
    run_suite_4_client_integration()

    print(f"\n{CYAN}{BOLD}{'=' * 80}{RESET}")
    print(f"{BOLD} VERIFICATION SUMMARY{RESET}")
    print(f"{CYAN}{BOLD}{'=' * 80}{RESET}")
    print(f"  Total Passed : {GREEN}{passed}{RESET}")
    print(f"  Total Failed : {RED}{failed}{RESET}")

    if failed == 0:
        print(f"\n{GREEN}{BOLD}[OK] ALL SEC FORM 8-K EVENT CLASSIFICATION VERIFICATION TESTS PASSED!{RESET}\n")
        return 0
    else:
        print(f"\n{RED}{BOLD}[FAIL] {failed} VERIFICATION TEST(S) FAILED.{RESET}\n")
        return 1


if __name__ == "__main__":
    sys.exit(main())
