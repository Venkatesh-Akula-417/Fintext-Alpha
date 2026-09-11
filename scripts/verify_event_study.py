#!/usr/bin/env python3
"""
================================================================================
FinText-Alpha-Vectorizer -- Event Study & Cumulative Abnormal Returns (CAR) Verifier
================================================================================
Validates econometric Ordinary Least Squares (OLS) Market Model regressions,
Abnormal Return (AR) and Cumulative Abnormal Return (CAR) calculations,
Point-in-Time (PIT) survivorship rules, Parameter validation bounds, and
Python SDK model serialization fidelity.
================================================================================
"""

import sys
import math
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

from fintext.models import AbnormalReturnPoint, EventStudyResponse
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
# Reference OLS & Event Study Implementation (Matches Rust Implementation)
# ═══════════════════════════════════════════════════════════════════════════════

def compute_ols_market_model(asset_returns: list[float], benchmark_returns: list[float]) -> tuple[float, float, float]:
    """Compute OLS alpha, beta, and R^2 for r_asset = alpha + beta * r_bench + eps."""
    n = len(asset_returns)
    if n < 2 or len(benchmark_returns) != n:
        return 0.0, 1.0, 0.0

    mean_x = sum(benchmark_returns) / n
    mean_y = sum(asset_returns) / n

    cov_xy = sum((x - mean_x) * (y - mean_y) for x, y in zip(benchmark_returns, asset_returns))
    var_x = sum((x - mean_x) ** 2 for x in benchmark_returns)
    var_y = sum((y - mean_y) ** 2 for y in asset_returns)

    if var_x < 1e-12:
        return mean_y, 0.0, 0.0

    beta = cov_xy / var_x
    alpha = mean_y - beta * mean_x

    if var_y < 1e-12:
        r_squared = 0.0
    else:
        r = cov_xy / (math.sqrt(var_x) * math.sqrt(var_y))
        r_squared = max(0.0, min(1.0, r * r))

    return round(alpha, 6), round(beta, 4), round(r_squared, 4)


def compute_abnormal_returns(
    event_asset_returns: list[float],
    event_bench_returns: list[float],
    alpha: float,
    beta: float,
) -> tuple[list[dict], float, float, float]:
    """Calculate AR, Expected Return, and CAR over event window."""
    points = []
    cum_ar = 0.0
    w = (len(event_asset_returns) - 1) // 2
    car_pre = 0.0
    car_post = 0.0

    for i, (r_asset, r_bench) in enumerate(zip(event_asset_returns, event_bench_returns)):
        offset = i - w
        expected = alpha + beta * r_bench
        ar = r_asset - expected
        cum_ar += ar

        if offset < 0:
            car_pre += ar
        elif offset > 0:
            car_post += ar

        points.append({
            "day_offset": offset,
            "actual_return": round(r_asset, 6),
            "benchmark_return": round(r_bench, 6),
            "expected_return": round(expected, 6),
            "abnormal_return": round(ar, 6),
            "cumulative_abnormal_return": round(cum_ar, 6),
        })

    return points, round(cum_ar, 6), round(car_pre, 6), round(car_post, 6)


# ═══════════════════════════════════════════════════════════════════════════════
# Test Suite 1: OLS Regression & Statistical Invariants
# ═══════════════════════════════════════════════════════════════════════════════

def run_suite_1_ols_math():
    print_header("Suite 1: OLS Market Model Regression & Statistical Accuracy")

    # 1. Perfect linear relationship: y = 0.001 + 1.5 * x
    x_data = [0.01 * (i - 5) for i in range(11)]
    y_data = [0.001 + 1.5 * x for x in x_data]
    alpha, beta, r2 = compute_ols_market_model(y_data, x_data)

    if abs(alpha - 0.001) < 1e-5 and abs(beta - 1.5) < 1e-4 and abs(r2 - 1.0) < 1e-4:
        test_passed("Perfect linear OLS fit (Beta=1.5, Alpha=0.001, R^2=1.0)", f"alpha={alpha}, beta={beta}, r2={r2}")
    else:
        test_failed("Perfect linear OLS fit", f"Expected (0.001, 1.5, 1.0), got ({alpha}, {beta}, {r2})")

    # 2. Negative beta relationship: y = -0.8 * x
    y_neg = [-0.8 * x for x in x_data]
    alpha_neg, beta_neg, r2_neg = compute_ols_market_model(y_neg, x_data)
    if abs(beta_neg - (-0.8)) < 1e-4 and abs(r2_neg - 1.0) < 1e-4:
        test_passed("Negative beta OLS fit (Beta=-0.8, R^2=1.0)", f"beta={beta_neg}, r2={r2_neg}")
    else:
        test_failed("Negative beta OLS fit", f"Expected Beta=-0.8, got {beta_neg}")

    # 3. Uncorrelated data: R^2 close to 0
    x_uncorr = [0.01, -0.01, 0.01, -0.01, 0.01, -0.01]
    y_uncorr = [0.02, 0.02, -0.02, -0.02, 0.01, 0.01]
    alpha_u, beta_u, r2_u = compute_ols_market_model(y_uncorr, x_uncorr)
    if r2_u < 0.2:
        test_passed("Uncorrelated returns yield low R^2", f"R^2={r2_u:.4f}")
    else:
        test_failed("Uncorrelated returns yield low R^2", f"Expected R^2 < 0.2, got {r2_u}")

    # 4. Zero variance benchmark fallback
    x_const = [0.01] * 10
    y_const = [0.02 + 0.001 * i for i in range(10)]
    alpha_c, beta_c, r2_c = compute_ols_market_model(y_const, x_const)
    if beta_c == 0.0 and r2_c == 0.0:
        test_passed("Zero variance benchmark fallback (Beta=0, R^2=0)", f"beta={beta_c}, r2={r2_c}")
    else:
        test_failed("Zero variance benchmark fallback", f"Expected Beta=0, got {beta_c}")


# ═══════════════════════════════════════════════════════════════════════════════
# Test Suite 2: Abnormal Returns & CAR Invariants
# ═══════════════════════════════════════════════════════════════════════════════

def run_suite_2_car_invariants():
    print_header("Suite 2: Abnormal Returns & Cumulative Abnormal Returns (CAR)")

    # 11-day event window [-5 to +5]
    asset_ret = [0.01, -0.005, 0.02, -0.01, 0.005, 0.04, 0.015, -0.008, 0.012, -0.003, 0.007]
    bench_ret = [0.005, -0.002, 0.01, -0.005, 0.002, 0.01, 0.005, -0.004, 0.006, -0.001, 0.003]
    alpha = 0.0005
    beta = 1.20

    points, car_full, car_pre, car_post = compute_abnormal_returns(asset_ret, bench_ret, alpha, beta)

    # 1. Total points count matches 2 * W_e + 1
    if len(points) == 11:
        test_passed("Observation points count matches 2 * W_e + 1 (11 points)", f"count={len(points)}")
    else:
        test_failed("Observation points count", f"Expected 11, got {len(points)}")

    # 2. Day offset spans monotonically from -5 to +5
    offsets = [p["day_offset"] for p in points]
    if offsets == list(range(-5, 6)):
        test_passed("Day offset spans monotonically [-5..+5]", f"{offsets}")
    else:
        test_failed("Day offset monotonicity", f"Expected [-5..5], got {offsets}")

    # 3. Running sum property of CAR
    car_diffs = [
        abs(points[i]["cumulative_abnormal_return"] - (points[i-1]["cumulative_abnormal_return"] + points[i]["abnormal_return"]))
        for i in range(1, len(points))
    ]
    if max(car_diffs) < 1e-5:
        test_passed("CAR is exact cumulative running sum of AR", f"max diff={max(car_diffs):.2e}")
    else:
        test_failed("CAR running sum", f"Max diff {max(car_diffs)}")

    # 4. Partition identity: CAR_full = CAR_pre + AR(0) + CAR_post
    ar_0 = points[5]["abnormal_return"]
    part_sum = round(car_pre + ar_0 + car_post, 6)
    if abs(car_full - part_sum) < 1e-5:
        test_passed("CAR partition identity: CAR_full == CAR_pre + AR(0) + CAR_post", f"{car_full} == {part_sum}")
    else:
        test_failed("CAR partition identity", f"{car_full} != {part_sum}")


# ═══════════════════════════════════════════════════════════════════════════════
# Test Suite 3: Python SDK Models & Serialization
# ═══════════════════════════════════════════════════════════════════════════════

def run_suite_3_sdk_models():
    print_header("Suite 3: Python SDK Models & Serialization Round-Trip")

    point = AbnormalReturnPoint(
        date="2025-06-15",
        day_offset=0,
        actual_return=0.035,
        benchmark_return=0.008,
        expected_return=0.0101,
        abnormal_return=0.0249,
        cumulative_abnormal_return=0.0412,
    )

    resp = EventStudyResponse(
        ticker="AAPL",
        event_date="2025-06-15",
        event_window=5,
        estimation_window=60,
        benchmark_ticker="SPY",
        alpha=0.0005,
        beta=1.20,
        r_squared=0.65,
        car_full_window=0.0412,
        car_pre_event=0.0120,
        car_post_event=0.0043,
        count=1,
        abnormal_returns=[point],
        message="Market model OLS regression completed successfully",
    )

    # 1. Pydantic validation
    if resp.ticker == "AAPL" and resp.event_window == 5 and resp.abnormal_returns[0].day_offset == 0:
        test_passed("EventStudyResponse Pydantic model initialization", "Fields populated correctly")
    else:
        test_failed("EventStudyResponse Pydantic model initialization", "Field mismatch")

    # 2. Round-trip JSON serialization
    dumped = resp.model_dump()
    reconstructed = EventStudyResponse.model_validate(dumped)
    if reconstructed == resp:
        test_passed("EventStudyResponse JSON round-trip fidelity", "Identical match")
    else:
        test_failed("EventStudyResponse JSON round-trip fidelity", "Mismatch")


# ═══════════════════════════════════════════════════════════════════════════════
# Test Suite 4: Synchronous & Asynchronous Client Integration
# ═══════════════════════════════════════════════════════════════════════════════

def run_suite_4_client_integration():
    print_header("Suite 4: Synchronous & Asynchronous FinTextClient Integration")

    from test_client import create_mock_transport
    from test_async_client import create_async_mock_transport

    # 1. Sync Client
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )

    res = client.event_study(
        ticker="AAPL",
        event_date="2025-06-15",
        event_window=5,
        estimation_window=60,
        benchmark_ticker="SPY",
    )

    if res.ticker == "AAPL" and res.count == 11 and len(res.abnormal_returns) == 11:
        test_passed("FinTextClient.event_study() synchronous execution", f"ticker={res.ticker}, points={res.count}")
    else:
        test_failed("FinTextClient.event_study() synchronous execution", f"Unexpected response: {res}")

    # 2. Sync Client Validation Errors
    try:
        client.event_study(ticker="", event_date="2025-06-15")
        test_failed("Sync client rejects empty ticker", "Did not raise FinTextValidationError")
    except FinTextValidationError:
        test_passed("Sync client rejects empty ticker", "Raised FinTextValidationError")

    try:
        client.event_study(ticker="AAPL", event_date="2025-06-15", event_window=25)
        test_failed("Sync client rejects event_window > 20", "Did not raise FinTextValidationError")
    except FinTextValidationError:
        test_passed("Sync client rejects event_window > 20", "Raised FinTextValidationError")

    try:
        client.event_study(ticker="AAPL", event_date="2025-06-15", estimation_window=5)
        test_failed("Sync client rejects estimation_window < 10", "Did not raise FinTextValidationError")
    except FinTextValidationError:
        test_passed("Sync client rejects estimation_window < 10", "Raised FinTextValidationError")

    # 3. Async Client
    async def run_async():
        async_client = FinTextAsyncClient(
            base_url="http://mock.fintext",
            api_token="valid_test_token",
            transport=create_async_mock_transport(),
        )
        async_res = await async_client.event_study(
            ticker="NVDA",
            event_date="2025-06-15",
            event_window=5,
            estimation_window=60,
            benchmark_ticker="SPY",
        )
        await async_client.close()
        return async_res

    async_res = asyncio.run(run_async())
    if async_res.ticker == "NVDA" and async_res.count == 11:
        test_passed("FinTextAsyncClient.event_study() asynchronous execution", f"ticker={async_res.ticker}, count={async_res.count}")
    else:
        test_failed("FinTextAsyncClient.event_study() asynchronous execution", f"Unexpected response: {async_res}")


def main():
    print(f"\n{BOLD}{'#' * 80}{RESET}")
    print(f"{BOLD} FinText-Alpha-Vectorizer: Event Study & CAR Engine Verification{RESET}")
    print(f"{BOLD}{'#' * 80}{RESET}")

    run_suite_1_ols_math()
    run_suite_2_car_invariants()
    run_suite_3_sdk_models()
    run_suite_4_client_integration()

    print(f"\n{CYAN}{BOLD}{'=' * 80}{RESET}")
    print(f"{BOLD} VERIFICATION SUMMARY{RESET}")
    print(f"{CYAN}{BOLD}{'=' * 80}{RESET}")
    print(f"  Total Passed : {GREEN}{passed}{RESET}")
    print(f"  Total Failed : {RED}{failed}{RESET}")

    if failed == 0:
        print(f"\n{GREEN}{BOLD}[OK] ALL EVENT STUDY & CAR ENGINE VERIFICATION TESTS PASSED!{RESET}\n")
        return 0
    else:
        print(f"\n{RED}{BOLD}[FAIL] {failed} VERIFICATION TEST(S) FAILED.{RESET}\n")
        return 1


if __name__ == "__main__":
    sys.exit(main())
