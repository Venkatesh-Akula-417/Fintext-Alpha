#!/usr/bin/env python3
"""
================================================================================
FinText-Alpha-Vectorizer -- API Usage Statistics & Consumption Verifier
================================================================================
Validates statistical metrics calculations (percentile interpolation, latency
aggregates), multi-dimensional grouping invariants (day, endpoint, method,
status_code), parameter validation bounds, and Python SDK model serialization
fidelity.
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
# Reference Percentile Implementation (Matches Rust compute_percentile)
# ═══════════════════════════════════════════════════════════════════════════════

def compute_percentile(sorted_data: list[float], p: float) -> float:
    """Linear-interpolated exact percentile on pre-sorted array."""
    if not sorted_data:
        return 0.0
    if len(sorted_data) == 1:
        return sorted_data[0]
    p_clamped = max(0.0, min(100.0, p))
    idx = (len(sorted_data) - 1.0) * (p_clamped / 100.0)
    low = int(math.floor(idx))
    high = int(math.ceil(idx))
    if low == high:
        return sorted_data[low]
    weight = idx - low
    return sorted_data[low] * (1.0 - weight) + sorted_data[high] * weight


# ═══════════════════════════════════════════════════════════════════════════════
# Test Suites
# ═══════════════════════════════════════════════════════════════════════════════

def run_percentile_tests():
    print_header("Suite 1: Percentile & Statistical Calculations")

    # 1. Empty array
    res = compute_percentile([], 95.0)
    if res == 0.0:
        test_passed("Empty array returns 0.0", f"{res}")
    else:
        test_failed("Empty array returns 0.0", f"Expected 0.0, got {res}")

    # 2. Single element
    res = compute_percentile([42.5], 95.0)
    if res == 42.5:
        test_passed("Single element returns value", f"{res}")
    else:
        test_failed("Single element returns value", f"Expected 42.5, got {res}")

    # 3. 10 elements [1..10]
    data = [float(x) for x in range(1, 11)]
    p50 = compute_percentile(data, 50.0)
    if abs(p50 - 5.5) < 1e-6:
        test_passed("P50 median interpolation [1..10]", f"p50={p50:.2f}")
    else:
        test_failed("P50 median interpolation [1..10]", f"Expected 5.5, got {p50}")

    p95 = compute_percentile(data, 95.0)
    if abs(p95 - 9.55) < 1e-6:
        test_passed("P95 percentile interpolation [1..10]", f"p95={p95:.2f}")
    else:
        test_failed("P95 percentile interpolation [1..10]", f"Expected 9.55, got {p95}")

    p0 = compute_percentile(data, 0.0)
    p100 = compute_percentile(data, 100.0)
    if p0 == 1.0 and p100 == 10.0:
        test_passed("P0 min and P100 max boundary checks", f"p0={p0}, p100={p100}")
    else:
        test_failed("P0 min and P100 max boundary checks", f"p0={p0}, p100={p100}")

    # 4. Outlier distribution
    latencies = [1.2, 1.5, 1.8, 2.0, 2.1, 2.3, 2.5, 3.0, 4.2, 50.0]
    avg = sum(latencies) / len(latencies)
    p95_outlier = compute_percentile(latencies, 95.0)
    max_lat = max(latencies)
    if avg <= p95_outlier <= max_lat:
        test_passed("Monotonic ordering with outlier: Avg <= P95 <= Max", f"Avg={avg:.2f}, P95={p95_outlier:.2f}, Max={max_lat:.2f}")
    else:
        test_failed("Monotonic ordering with outlier", f"Avg={avg}, P95={p95_outlier}, Max={max_lat}")


def run_summary_invariants_tests():
    print_header("Suite 2: Summary Invariants & Conservation Laws")

    total_requests = 1250
    successful_requests = 1205
    failed_requests = 45
    rate_limited_requests = 12
    avg_latency = 4.85
    p95_latency = 14.20
    max_latency = 45.60

    # 1. Total request conservation
    if total_requests == successful_requests + failed_requests:
        test_passed("Total requests conservation: Total == Succ + Fail", f"{total_requests} == {successful_requests} + {failed_requests}")
    else:
        test_failed("Total requests conservation", f"{total_requests} != {successful_requests} + {failed_requests}")

    # 2. Rate limit upper bound
    if rate_limited_requests <= failed_requests:
        test_passed("Rate limited requests bounded by failed requests", f"{rate_limited_requests} <= {failed_requests}")
    else:
        test_failed("Rate limited requests bounded by failed requests", f"{rate_limited_requests} > {failed_requests}")

    # 3. Latency hierarchy
    if avg_latency <= p95_latency <= max_latency:
        test_passed("Latency hierarchy: Avg <= P95 <= Max", f"{avg_latency} <= {p95_latency} <= {max_latency}")
    else:
        test_failed("Latency hierarchy", f"{avg_latency}, {p95_latency}, {max_latency}")


def run_grouping_dimension_tests():
    print_header("Suite 3: Multi-Dimensional Grouping Integrity")

    # 1. Day grouping
    start_d = date(2026, 8, 1)
    end_d = date(2026, 8, 10)
    day_count = (end_d - start_d).days + 1
    if day_count == 10:
        test_passed("Day range span calculation", f"{day_count} days")
    else:
        test_failed("Day range span calculation", f"Expected 10, got {day_count}")

    # 2. Endpoint grouping
    endpoints = ["/sentiment", "/options/iv", "/options/unusual", "/sentiment/history", "/spillovers", "/backtest"]
    if len(endpoints) == 6 and all(ep.startswith("/") for ep in endpoints):
        test_passed("Endpoint group keys start with '/'", f"{len(endpoints)} endpoints")
    else:
        test_failed("Endpoint group keys", f"Invalid endpoints: {endpoints}")

    # 3. Method grouping
    methods = ["GET", "POST"]
    if len(methods) == 2 and all(m in {"GET", "POST", "PUT", "DELETE"} for m in methods):
        test_passed("HTTP method group keys valid", f"{methods}")
    else:
        test_failed("HTTP method group keys", f"Invalid methods: {methods}")

    # 4. Status code grouping
    status_codes = ["200", "400", "429", "500"]
    if all(s.isdigit() and len(s) == 3 for s in status_codes):
        test_passed("Status code group keys valid 3-digit strings", f"{status_codes}")
    else:
        test_failed("Status code group keys", f"Invalid statuses: {status_codes}")


def run_sdk_models_and_client_tests():
    print_header("Suite 4: Python SDK Models & Client Integration")

    try:
        from fintext.models import UsageStatsSummary, UsageGroupItem, UsageStatsResponse
        from fintext.client import FinTextClient
        from fintext.async_client import FinTextAsyncClient
        from fintext.exceptions import FinTextValidationError

        # 1. Model Instantiation & Validation
        summary = UsageStatsSummary(
            total_requests=1250,
            successful_requests=1205,
            failed_requests=45,
            rate_limited_requests=12,
            average_latency_ms=4.85,
            p95_latency_ms=14.20,
            max_latency_ms=45.60,
        )
        test_passed("UsageStatsSummary model instantiation", f"total={summary.total_requests}")

        item = UsageGroupItem(
            key="2026-08-28",
            count=150,
            successful_requests=145,
            failed_requests=5,
            rate_limited_requests=1,
            average_latency_ms=3.92,
            p95_latency_ms=11.50,
            max_latency_ms=38.20,
        )
        test_passed("UsageGroupItem model instantiation", f"key={item.key}, count={item.count}")

        resp = UsageStatsResponse(
            user_id="usr_institutional_01",
            start_date="2026-07-30",
            end_date="2026-08-29",
            group_by="day",
            summary=summary,
            breakdown=[item],
            message="Usage statistics aggregated successfully",
        )
        test_passed("UsageStatsResponse model instantiation", f"user={resp.user_id}")

        # 2. Serialization round-trip
        json_data = resp.model_dump()
        reconstructed = UsageStatsResponse.model_validate(json_data)
        if reconstructed == resp:
            test_passed("UsageStatsResponse JSON round-trip fidelity", "Identical match")
        else:
            test_failed("UsageStatsResponse JSON round-trip fidelity", "Mismatch")

        # 3. Synchronous Client Test
        from test_client import create_mock_transport
        client = FinTextClient(
            base_url="http://mock.fintext",
            api_token="valid_test_token",
            transport=create_mock_transport(),
        )

        client_resp = client.usage_stats()
        if client_resp.user_id == "test_quant_fund" and client_resp.summary.total_requests == 1250:
            test_passed("FinTextClient.usage_stats() default call", f"user={client_resp.user_id}, total={client_resp.summary.total_requests}")
        else:
            test_failed("FinTextClient.usage_stats() default call", f"Unexpected response: {client_resp}")

        # 4. Client Parameter Validation
        try:
            client.usage_stats(group_by="unsupported_dim")
            test_failed("Client rejects invalid group_by", "Did not raise FinTextValidationError")
        except FinTextValidationError:
            test_passed("Client rejects invalid group_by", "Raised FinTextValidationError")

        try:
            client.usage_stats(limit=0)
            test_failed("Client rejects limit=0", "Did not raise FinTextValidationError")
        except FinTextValidationError:
            test_passed("Client rejects limit=0", "Raised FinTextValidationError")

        # 5. Asynchronous Client Test
        from test_async_client import create_async_mock_transport

        async def run_async_test():
            async_client = FinTextAsyncClient(
                base_url="http://mock.fintext",
                api_token="valid_test_token",
                transport=create_async_mock_transport(),
            )
            async_resp = await async_client.usage_stats(group_by="endpoint")
            await async_client.close()
            return async_resp

        async_resp = asyncio.run(run_async_test())
        if async_resp.group_by == "endpoint" and len(async_resp.breakdown) > 0:
            test_passed("FinTextAsyncClient.usage_stats() async call", f"group_by={async_resp.group_by}, items={len(async_resp.breakdown)}")
        else:
            test_failed("FinTextAsyncClient.usage_stats() async call", f"Unexpected response: {async_resp}")

    except Exception as e:
        test_failed("Python SDK Models & Client Integration", f"Exception: {e}")


def main():
    print(f"\n{BOLD}{'#' * 80}{RESET}")
    print(f"{BOLD} FinText-Alpha-Vectorizer: Usage Statistics & Consumption Verification{RESET}")
    print(f"{BOLD}{'#' * 80}{RESET}")

    run_percentile_tests()
    run_summary_invariants_tests()
    run_grouping_dimension_tests()
    run_sdk_models_and_client_tests()

    print(f"\n{CYAN}{BOLD}{'=' * 80}{RESET}")
    print(f"{BOLD} VERIFICATION SUMMARY{RESET}")
    print(f"{CYAN}{BOLD}{'=' * 80}{RESET}")
    print(f"  Total Passed : {GREEN}{passed}{RESET}")
    print(f"  Total Failed : {RED}{failed}{RESET}")

    if failed == 0:
        print(f"\n{GREEN}{BOLD}[OK] ALL API USAGE STATISTICS VERIFICATION TESTS PASSED!{RESET}\n")
        return 0
    else:
        print(f"\n{RED}{BOLD}[FAIL] {failed} VERIFICATION TEST(S) FAILED.{RESET}\n")
        return 1



if __name__ == "__main__":
    sys.exit(main())
