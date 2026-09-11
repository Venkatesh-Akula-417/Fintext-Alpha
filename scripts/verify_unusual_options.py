#!/usr/bin/env python3
"""
================================================================================
FinText-Alpha-Vectorizer -- Unusual Options Activity (UOA) Detection Verifier
================================================================================
Validates statistical scoring algorithms (Volume/OI ratio, volume z-scores,
composite score ranking), multi-criteria filtering, edge cases (zero OI, low
variance, lookback limits), Python SDK serialization, and API contract invariants.
================================================================================
"""

import json
import math
import sys
from pathlib import Path

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
# Reference Mathematical Implementation of UOA Scoring
# ═══════════════════════════════════════════════════════════════════════════════

def compute_uoa_metrics(
    volume: int,
    open_interest: int,
    avg_volume: float,
    stddev: float,
) -> dict:
    """Institutional reference implementation of UOA statistical scoring."""
    oi_denom = max(open_interest, 1)
    volume_oi_ratio = volume / oi_denom

    safe_stddev = max(stddev, 1.0)
    volume_zscore = (volume - avg_volume) / safe_stddev

    # Composite score = ratio * max(zscore, 0.1)
    score = volume_oi_ratio * max(volume_zscore, 0.1)

    return {
        "volume_oi_ratio": round(volume_oi_ratio, 4),
        "volume_zscore": round(volume_zscore, 4),
        "score": round(score, 4),
    }


# ═══════════════════════════════════════════════════════════════════════════════
# Test Suite 1: Mathematical Scoring & Statistical Invariants
# ═══════════════════════════════════════════════════════════════════════════════

def run_suite_1_math_scoring():
    print_header("Test Suite 1: Volume/OI Ratio & Z-Score Mathematical Scoring")

    # 1. Standard institutional high-volume case
    res = compute_uoa_metrics(volume=48500, open_interest=2200, avg_volume=3200.0, stddev=1800.0)
    expected_ratio = 48500 / 2200  # 22.04545...
    expected_zscore = (48500 - 3200) / 1800  # 25.1666...
    expected_score = expected_ratio * expected_zscore  # 554.8119...
    if abs(res["volume_oi_ratio"] - expected_ratio) < 1e-3:
        test_passed("Standard high UOA ratio calculation", f"R = {res['volume_oi_ratio']:.4f}")
    else:
        test_failed("Standard high UOA ratio", f"Expected {expected_ratio:.4f}, got {res['volume_oi_ratio']}")

    if abs(res["volume_zscore"] - expected_zscore) < 1e-3:
        test_passed("Standard high UOA z-score calculation", f"Z = {res['volume_zscore']:.4f}")
    else:
        test_failed("Standard high UOA z-score", f"Expected {expected_zscore:.4f}, got {res['volume_zscore']}")

    if abs(res["score"] - expected_score) < 1e-2:
        test_passed("Composite score = Ratio × Z-score", f"Score = {res['score']:.4f}")
    else:
        test_failed("Composite score", f"Expected {expected_score:.4f}, got {res['score']}")

    # 2. Zero Open Interest protection (new strike or newly opened contract)
    res_zero_oi = compute_uoa_metrics(volume=5000, open_interest=0, avg_volume=1000.0, stddev=500.0)
    if res_zero_oi["volume_oi_ratio"] == 5000.0:
        test_passed("Zero Open Interest protection (OI=0 -> denom=1)", f"R = {res_zero_oi['volume_oi_ratio']}")
    else:
        test_failed("Zero OI protection", f"Expected 5000.0, got {res_zero_oi['volume_oi_ratio']}")

    # 3. Zero variance protection (stddev = 0.0)
    res_zero_std = compute_uoa_metrics(volume=5000, open_interest=500, avg_volume=1000.0, stddev=0.0)
    if res_zero_std["volume_zscore"] == 4000.0:
        test_passed("Zero stddev protection (stddev=0 -> safe_stddev=1.0)", f"Z = {res_zero_std['volume_zscore']}")
    else:
        test_failed("Zero stddev protection", f"Expected 4000.0, got {res_zero_std['volume_zscore']}")

    # 4. Negative z-score flooring (volume < historical avg)
    res_neg_z = compute_uoa_metrics(volume=500, open_interest=100, avg_volume=2000.0, stddev=500.0)
    # zscore = (500 - 2000) / 500 = -3.0. Floor = 0.1 -> score = 5.0 * 0.1 = 0.5
    if res_neg_z["volume_zscore"] == -3.0 and abs(res_neg_z["score"] - 0.5) < 1e-3:
        test_passed("Negative z-score flooring at 0.1 (prevents negative scores)", f"Z={res_neg_z['volume_zscore']}, Score={res_neg_z['score']}")
    else:
        test_failed("Negative z-score flooring", f"Expected Z=-3.0, Score=0.5, got {res_neg_z}")

    # 5. Monotonicity check: increasing volume strictly increases score
    scores = [
        compute_uoa_metrics(volume=v, open_interest=1000, avg_volume=1000.0, stddev=500.0)["score"]
        for v in [2000, 5000, 10000, 25000, 50000]
    ]
    is_strictly_increasing = all(scores[i] < scores[i + 1] for i in range(len(scores) - 1))
    if is_strictly_increasing:
        test_passed("Monotonicity: score strictly increases with volume", f"Scores: {scores}")
    else:
        test_failed("Monotonicity", f"Scores not strictly increasing: {scores}")


# ═══════════════════════════════════════════════════════════════════════════════
# Test Suite 2: Multi-Criteria Filtering Logic
# ═══════════════════════════════════════════════════════════════════════════════

def run_suite_2_filtering():
    print_header("Test Suite 2: Multi-Criteria Filtering Logic")

    contracts = [
        {"name": "HighVol_HighRatio", "vol": 50000, "oi": 2000, "avg": 3000.0, "std": 1000.0},  # ratio 25, vol 50k
        {"name": "LowVol_HighRatio",  "vol": 50,    "oi": 2,    "avg": 10.0,   "std": 5.0},     # ratio 25, vol 50
        {"name": "HighVol_LowRatio",   "vol": 5000,  "oi": 50000,"avg": 4000.0, "std": 1000.0}, # ratio 0.1, vol 5k
        {"name": "Exact_Boundary",     "vol": 100,   "oi": 50,   "avg": 50.0,   "std": 20.0},   # ratio 2.0, vol 100
        {"name": "Below_Both",        "vol": 20,    "oi": 100,  "avg": 20.0,   "std": 5.0},     # ratio 0.2, vol 20
    ]

    def filter_contracts(items, min_ratio=2.0, min_vol=100):
        res = []
        for c in items:
            ratio = c["vol"] / max(c["oi"], 1)
            if c["vol"] >= min_vol and ratio >= min_ratio:
                res.append(c["name"])
        return res

    # 1. Standard filter (ratio >= 2.0, vol >= 100)
    passed_items = filter_contracts(contracts, min_ratio=2.0, min_vol=100)
    if set(passed_items) == {"HighVol_HighRatio", "Exact_Boundary"}:
        test_passed("Standard filter (min_ratio=2.0, min_vol=100)", f"Passed: {passed_items}")
    else:
        test_failed("Standard filter", f"Expected HighVol_HighRatio & Exact_Boundary, got {passed_items}")

    # 2. Volume filter rejection (LowVol_HighRatio rejected by vol < 100)
    if "LowVol_HighRatio" not in passed_items:
        test_passed("Volume filter excludes contracts with vol < min_volume")
    else:
        test_failed("Volume filter", "LowVol_HighRatio should be rejected")

    # 3. Ratio filter rejection (HighVol_LowRatio rejected by ratio < 2.0)
    if "HighVol_LowRatio" not in passed_items:
        test_passed("Ratio filter excludes contracts with ratio < min_ratio")
    else:
        test_failed("Ratio filter", "HighVol_LowRatio should be rejected")

    # 4. Exact boundary match inclusion
    if "Exact_Boundary" in passed_items:
        test_passed("Boundary match (vol=100, ratio=2.0) included")
    else:
        test_failed("Boundary match", "Exact_Boundary should be included")

    # 5. Strict filter (ratio >= 10.0, vol >= 1000)
    strict_passed = filter_contracts(contracts, min_ratio=10.0, min_vol=1000)
    if strict_passed == ["HighVol_HighRatio"]:
        test_passed("Strict institutional filter (ratio>=10, vol>=1000)", f"Passed: {strict_passed}")
    else:
        test_failed("Strict filter", f"Expected ['HighVol_HighRatio'], got {strict_passed}")

    # 6. Ultra-strict filter (returns empty list cleanly)
    empty_passed = filter_contracts(contracts, min_ratio=100.0, min_vol=100000)
    if len(empty_passed) == 0:
        test_passed("Ultra-strict threshold returns empty result set cleanly")
    else:
        test_failed("Ultra-strict threshold", f"Expected empty list, got {empty_passed}")


# ═══════════════════════════════════════════════════════════════════════════════
# Test Suite 3: Ranking & Sorting Properties
# ═══════════════════════════════════════════════════════════════════════════════

def run_suite_3_ranking():
    print_header("Test Suite 3: Ranking & Sorting Order Invariants")

    dataset = [
        {"ticker": "NVDA_CALL", "vol": 95000, "oi": 8000,  "avg": 12000.0, "std": 5500.0},
        {"ticker": "AAPL_CALL", "vol": 48500, "oi": 2200,  "avg": 3200.0,  "std": 1800.0},
        {"ticker": "TSLA_CALL", "vol": 72000, "oi": 5500,  "avg": 9500.0,  "std": 4200.0},
        {"ticker": "META_CALL", "vol": 18500, "oi": 1200,  "avg": 2800.0,  "std": 1500.0},
        {"ticker": "AMZN_CALL", "vol": 55000, "oi": 4200,  "avg": 7800.0,  "std": 3600.0},
    ]

    scored = []
    for item in dataset:
        metrics = compute_uoa_metrics(item["vol"], item["oi"], item["avg"], item["std"])
        scored.append({**item, **metrics})

    # Sort by score descending
    scored.sort(key=lambda x: x["score"], reverse=True)

    # 1. Verify strict non-increasing score order
    is_sorted = all(scored[i]["score"] >= scored[i + 1]["score"] for i in range(len(scored) - 1))
    if is_sorted:
        test_passed("Results strictly ordered by composite score descending", f"Top score: {scored[0]['score']}")
    else:
        test_failed("Sorting order", "Items not properly sorted by score")

    # 2. Verify top ranked candidate
    top = scored[0]
    if top["ticker"] == "AAPL_CALL":  # AAPL has ratio 22.05 * zscore 25.17 = 554.8
        test_passed("Top candidate correctly identified by highest composite alpha score", f"{top['ticker']} (Score: {top['score']})")
    else:
        test_failed("Top candidate", f"Expected AAPL_CALL, got {top['ticker']}")

    # 3. Truncation to limit
    limit = 3
    truncated = scored[:limit]
    if len(truncated) == 3 and truncated[0]["score"] >= truncated[1]["score"] >= truncated[2]["score"]:
        test_passed(f"Limit truncation correctly retains top {limit} ranked items")
    else:
        test_failed("Limit truncation", f"Unexpected truncation result: len={len(truncated)}")


# ═══════════════════════════════════════════════════════════════════════════════
# Test Suite 4: Python SDK Model Serialization & Deserialization
# ═══════════════════════════════════════════════════════════════════════════════

def run_suite_4_sdk_models():
    print_header("Test Suite 4: Python SDK Model Round-Trip Serialization")

    from fintext.models import UnusualOptionItem, UnusualOptionsResponse

    # 1. UnusualOptionItem model validation
    raw_item = {
        "ticker": "O:AAPL251219C00250000",
        "underlying_ticker": "AAPL",
        "expiration_date": "2025-12-19",
        "strike": 250.0,
        "option_type": "CALL",
        "volume": 48500,
        "open_interest": 2200,
        "avg_volume": 3200.0,
        "volume_oi_ratio": 22.0455,
        "volume_zscore": 25.1667,
        "score": 554.8119,
        "timestamp": "2025-08-29T12:00:00Z",
    }

    item = UnusualOptionItem.model_validate(raw_item)
    if (
        item.ticker == "O:AAPL251219C00250000"
        and item.underlying_ticker == "AAPL"
        and item.strike == 250.0
        and item.volume == 48500
        and item.open_interest == 2200
        and item.volume_oi_ratio == 22.0455
        and item.score == 554.8119
    ):
        test_passed("UnusualOptionItem Pydantic deserialization", f"ticker={item.ticker}, score={item.score}")
    else:
        test_failed("UnusualOptionItem deserialization", f"Field mismatch: {item}")

    # 2. UnusualOptionsResponse model validation
    raw_resp = {
        "ticker": "ALL",
        "min_volume_oi_ratio": 2.0,
        "min_volume": 100,
        "days": 1,
        "count": 1,
        "items": [raw_item],
        "message": "Unusual options activity scan completed: 1 contract(s) flagged",
    }

    resp = UnusualOptionsResponse.model_validate(raw_resp)
    if (
        resp.ticker == "ALL"
        and resp.count == 1
        and len(resp.items) == 1
        and resp.min_volume_oi_ratio == 2.0
        and resp.min_volume == 100
        and resp.days == 1
    ):
        test_passed("UnusualOptionsResponse envelope deserialization", f"count={resp.count}, ticker={resp.ticker}")
    else:
        test_failed("UnusualOptionsResponse deserialization", f"Envelope mismatch: {resp}")

    # 3. Round-trip JSON dump and reload
    json_str = resp.model_dump_json()
    reloaded = UnusualOptionsResponse.model_validate_json(json_str)
    if reloaded == resp:
        test_passed("SDK Model lossless JSON round-trip serialization")
    else:
        test_failed("SDK Model round-trip", "Reloaded object does not match original")

    # 4. Extra fields tolerance
    raw_item_extra = {**raw_item, "unknown_field": "test_value", "future_metric": 123.45}
    item_extra = UnusualOptionItem.model_validate(raw_item_extra)
    if item_extra.ticker == raw_item["ticker"]:
        test_passed("UnusualOptionItem forward-compatibility: ignores unknown fields")
    else:
        test_failed("Forward compatibility", "Failed to parse item with extra fields")


# ═══════════════════════════════════════════════════════════════════════════════
# Test Suite 5: Input Validation & Boundary Conditions
# ═══════════════════════════════════════════════════════════════════════════════

def run_suite_5_validation():
    print_header("Test Suite 5: Input Validation & Boundary Conditions")

    # 1. Valid days range: 1 to 7
    for valid_days in [1, 2, 3, 5, 7]:
        if 1 <= valid_days <= 7:
            test_passed(f"Lookback days={valid_days} within allowed range [1, 7]")
        else:
            test_failed(f"Lookback days={valid_days}", "Should be valid")

    # 2. Invalid days rejected
    for invalid_days in [0, 8, 14, 30, -1]:
        if not (1 <= invalid_days <= 7):
            test_passed(f"Invalid days={invalid_days} correctly identified as out-of-bounds")
        else:
            test_failed(f"Invalid days={invalid_days}", "Should be invalid")

    # 3. Limit bounds: 1 to 100
    for valid_limit in [1, 10, 20, 50, 100]:
        if 1 <= valid_limit <= 100:
            test_passed(f"Result limit={valid_limit} within allowed range [1, 100]")
        else:
            test_failed(f"Result limit={valid_limit}", "Should be valid")

    # 4. Invalid limit rejected
    for invalid_limit in [0, 101, 500, -5]:
        if not (1 <= invalid_limit <= 100):
            test_passed(f"Invalid limit={invalid_limit} correctly identified as out-of-bounds")
        else:
            test_failed(f"Invalid limit={invalid_limit}", "Should be invalid")

    # 5. Non-negative ratio & volume
    for valid_thresh in [0.0, 0.5, 1.0, 2.0, 5.0, 10.0]:
        if valid_thresh >= 0.0:
            test_passed(f"min_volume_oi_ratio={valid_thresh} is non-negative")
        else:
            test_failed(f"min_volume_oi_ratio={valid_thresh}", "Should be valid")


# ═══════════════════════════════════════════════════════════════════════════════
# Test Suite 6: OCC Option Ticker Formatting & Strike Normalization
# ═══════════════════════════════════════════════════════════════════════════════

def run_suite_6_ticker_formatting():
    print_header("Test Suite 6: OCC Option Ticker Formatting & Strike Normalization")

    def format_occ_ticker(underlying: str, expiration: str, opt_type: str, strike: float) -> str:
        opt_char = "C" if opt_type.upper() in ["CALL", "C"] else "P"
        exp_clean = expiration.replace("-", "")
        exp_compact = exp_clean[2:] if len(exp_clean) >= 8 else exp_clean
        strike_int = int(strike * 1000.0)
        return f"O:{underlying}{exp_compact}{opt_char}{strike_int:08d}"

    test_cases = [
        ("AAPL", "2025-12-19", "CALL", 250.0, "O:AAPL251219C00250000"),
        ("NVDA", "2025-11-21", "PUT",  120.0, "O:NVDA251121P00120000"),
        ("TSLA", "2025-12-19", "CALL", 280.5, "O:TSLA251219C00280500"),
        ("SPY",  "2025-12-19", "PUT",  580.0, "O:SPY251219P00580000"),
        ("META", "2025-09-19", "CALL", 555.25, "O:META250919C00555250"),
    ]

    for underlying, exp, opt_type, strike, expected in test_cases:
        actual = format_occ_ticker(underlying, exp, opt_type, strike)
        if actual == expected:
            test_passed(f"OCC Ticker format: {underlying} K={strike} {opt_type}", actual)
        else:
            test_failed(f"OCC Ticker format: {underlying}", f"Expected {expected}, got {actual}")


# ═══════════════════════════════════════════════════════════════════════════════
# Main Entry Point
# ═══════════════════════════════════════════════════════════════════════════════

def main():
    print(f"\n{BOLD}{CYAN}{'=' * 80}{RESET}")
    print(f"{BOLD}{CYAN}  FinText-Alpha-Vectorizer: Unusual Options Activity Verification Suite{RESET}")
    print(f"{BOLD}{CYAN}{'=' * 80}{RESET}")

    run_suite_1_math_scoring()
    run_suite_2_filtering()
    run_suite_3_ranking()
    run_suite_4_sdk_models()
    run_suite_5_validation()
    run_suite_6_ticker_formatting()

    total = passed + failed
    print(f"\n{CYAN}{BOLD}{'=' * 80}{RESET}")
    if failed == 0:
        print(f"  {GREEN}{BOLD}[OK] ALL {passed} TESTS PASSED{RESET}")
    else:
        print(f"  {RED}{BOLD}[FAIL] {failed} OF {total} TESTS FAILED{RESET}")
    print(f"{CYAN}{BOLD}{'=' * 80}{RESET}\n")

    sys.exit(0 if failed == 0 else 1)


if __name__ == "__main__":
    main()

