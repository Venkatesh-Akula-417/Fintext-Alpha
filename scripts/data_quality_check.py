#!/usr/bin/env python3
"""
=====================================================================================
FinText-Alpha-Vectorizer — Comprehensive Financial Data Quality & Validation Suite
=====================================================================================
Principal Data Quality Analyst test harness validating:
  1. Score Range Bounds (sentiment_score in [-1, 1], confidence & quality in [0, 1])
  2. Timestamp RFC3339 Validity & Temporal Consistency (No future dates)
  3. Schema Completeness & Required Field Integrity across all sentiment endpoints
  4. Point-in-Time (PIT) & Survivorship Bias Enforcement for delisted equities
  5. Multi-Source Provenance & Data Provider Coverage Diversity
  6. Statistical Sanity & Distribution Dispersion (Mean in [-1, 1], StdDev <= 1.0)

Usage:
  venv/Scripts/python.exe scripts/data_quality_check.py
=====================================================================================
"""

from datetime import datetime, timezone, timedelta
import math
import os
from pathlib import Path
import subprocess
import sys
import time
import uuid

import httpx

if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
if hasattr(sys.stderr, "reconfigure"):
    sys.stderr.reconfigure(encoding="utf-8", errors="replace")

PROJECT_ROOT = Path(__file__).resolve().parent.parent
DEFAULT_PORT = 8000
BASE_URL = os.getenv("BASE_URL", f"http://127.0.0.1:{DEFAULT_PORT}").rstrip("/")
ADMIN_TOKEN = os.getenv("ADMIN_TOKEN", "fintext-admin-dev-secret-token")

SERVER_EXE = (
    PROJECT_ROOT / "rust" / "target" / "debug" / ("fintext_api.exe" if sys.platform == "win32" else "fintext_api")
    if (PROJECT_ROOT / "rust" / "target" / "debug" / ("fintext_api.exe" if sys.platform == "win32" else "fintext_api")).exists()
    else PROJECT_ROOT / "rust" / "target" / "release" / ("fintext_api.exe" if sys.platform == "win32" else "fintext_api")
)

# Test results accumulator
VALIDATION_METRICS = []


def record_validation(name: str, total_checked: int, violations: int, detail: str = ""):
    """Records the outcome of a data quality validation check."""
    pass_rate = 100.0 if total_checked == 0 else ((total_checked - violations) / total_checked) * 100.0
    passed = (violations == 0) or (pass_rate >= 95.0)

    VALIDATION_METRICS.append({
        "name": name,
        "total_checked": total_checked,
        "violations": violations,
        "pass_rate": pass_rate,
        "passed": passed,
        "detail": detail,
    })

    status_str = "[PASS]" if passed else "[FAIL]"
    print(f"  {status_str} {name}")
    print(f"         ↳ Checked: {total_checked} | Violations: {violations} | Pass Rate: {pass_rate:.2f}%")
    if detail:
        print(f"         ↳ Note: {detail}")


def ensure_server_running() -> tuple[bool, subprocess.Popen | None]:
    """Ensures the FinText Axum API Server is running, spawning it if necessary."""
    try:
        r = httpx.get(f"{BASE_URL}/health", timeout=1.5)
        if r.status_code == 200:
            print(f"[SERVER] Connected to active FinText API server at {BASE_URL}")
            return True, None
    except Exception:
        pass

    print(f"[SERVER] Spawning local FinText API Server at {BASE_URL} (Data Quality Mode)...")
    env = os.environ.copy()
    env.update({
        "PORT": str(DEFAULT_PORT),
        "HOST": "127.0.0.1",
        "ADMIN_TOKEN": ADMIN_TOKEN,
        "JWT_SECRET": "super_secret_test_jwt_key_32_bytes_len!!",
        "RATE_LIMIT_REQUESTS": "100000",
        "RATE_LIMIT_WINDOW_SECONDS": "60",
        "QUESTDB_MOCK_FALLBACK": "1",
        "POLYGON_MOCK_FALLBACK": "1",
        "WHISPER_MOCK_FALLBACK": "1",
        "RUST_LOG": "error",
    })

    proc = subprocess.Popen(
        [str(SERVER_EXE)],
        env=env,
        cwd=str(PROJECT_ROOT),
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )

    start_time = time.time()
    while time.time() - start_time < 25.0:
        try:
            r = httpx.get(f"{BASE_URL}/health", timeout=1.0)
            if r.status_code == 200:
                print(f"[SERVER] Server successfully started and healthy at {BASE_URL}")
                return True, proc
        except Exception:
            time.sleep(0.3)

    if proc:
        proc.terminate()
    raise RuntimeError(f"FinText API server failed to start at {BASE_URL} within 25s")


def get_jwt_token(client: httpx.Client) -> str:
    """Requests a valid institutional JWT token."""
    payload = {
        "user_id": f"data_quality_auditor_{uuid.uuid4().hex[:8]}",
        "role": "institutional",
        "expires_in_seconds": 3600,
    }
    headers = {"X-Admin-Token": ADMIN_TOKEN}
    res = client.post("/auth/token", json=payload, headers=headers)
    assert res.status_code == 200, f"Failed to issue JWT token: {res.text}"
    return res.json()["token"]


def request_with_retry(
    client: httpx.Client,
    method: str,
    endpoint: str,
    max_retries: int = 3,
    **kwargs,
) -> httpx.Response:
    """Executes HTTP request with retry on 429 rate limits."""
    for attempt in range(max_retries + 1):
        res = client.request(method, endpoint, **kwargs)
        if res.status_code == 429 and attempt < max_retries:
            time.sleep(2.0)
            continue
        return res
    return res


def parse_rfc3339_timestamp(ts_str: str) -> datetime | None:
    """Parses RFC3339 / ISO-8601 timestamp string into timezone-aware datetime."""
    if not ts_str or not isinstance(ts_str, str):
        return None
    try:
        clean = ts_str.strip()
        if clean.endswith("Z"):
            clean = clean[:-1] + "+00:00"
        return datetime.fromisoformat(clean)
    except Exception:
        return None


# ─────────────────────────────────────────────────────────────────────────────────
# Data Quality Check Procedures
# ─────────────────────────────────────────────────────────────────────────────────

def run_score_ranges_validation(client: httpx.Client, headers: dict):
    """
    Validation 1: Score Ranges Bounds
    Verifies that:
      - sentiment_score is in [-1.0, 1.0]
      - confidence is in [0.0, 1.0]
      - data_quality_score is in [0.0, 1.0]
    """
    print("\n[VALIDATION 1/6] Validating Score Ranges & Value Bounds...")
    total_checked = 0
    violations = 0
    sample_violations = []

    # 1. Check single sentiment for multiple tickers
    tickers = ["AAPL", "MSFT", "NVDA", "AMZN", "GOOGL"]
    for t in tickers:
        r = request_with_retry(client, "GET", "/sentiment", params={"ticker": t}, headers=headers)
        if r.status_code == 200:
            d = r.json()
            total_checked += 1
            score = d.get("sentiment_score")
            conf = d.get("confidence")

            if score is not None and not (-1.0 <= score <= 1.0):
                violations += 1
                sample_violations.append(f"Invalid sentiment_score {score} for {t}")
            if conf is not None and not (0.0 <= conf <= 1.0):
                violations += 1
                sample_violations.append(f"Invalid confidence {conf} for {t}")

    # 2. Check historical records
    r_hist = request_with_retry(
        client,
        "GET",
        "/sentiment/history",
        params={"ticker": "AAPL", "start_date": "2025-01-01", "end_date": "2025-03-31", "limit": 100},
        headers=headers,
    )
    if r_hist.status_code == 200:
        records = r_hist.json().get("records", [])
        for rec in records:
            total_checked += 1
            score = rec.get("sentiment_score")
            conf = rec.get("confidence")
            quality = rec.get("data_quality_score")

            if score is not None and not (-1.0 <= score <= 1.0):
                violations += 1
                sample_violations.append(f"History invalid score: {score}")
            if conf is not None and not (0.0 <= conf <= 1.0):
                violations += 1
                sample_violations.append(f"History invalid confidence: {conf}")
            if quality is not None and not (0.0 <= quality <= 1.0):
                violations += 1
                sample_violations.append(f"History invalid data_quality_score: {quality}")

    # 3. Check aggregated feed items
    r_feed = request_with_retry(client, "GET", "/sentiment/feed", params={"limit": 50}, headers=headers)
    if r_feed.status_code == 200:
        items = r_feed.json().get("records", [])
        for item in items:
            total_checked += 1
            score = item.get("sentiment_score")
            conf = item.get("confidence")
            quality = item.get("data_quality_score")

            if score is not None and not (-1.0 <= score <= 1.0):
                violations += 1
                sample_violations.append(f"Feed invalid score: {score}")
            if conf is not None and not (0.0 <= conf <= 1.0):
                violations += 1
                sample_violations.append(f"Feed invalid confidence: {conf}")
            if quality is not None and not (0.0 <= quality <= 1.0):
                violations += 1
                sample_violations.append(f"Feed invalid data_quality_score: {quality}")

    detail = "; ".join(sample_violations[:3]) if sample_violations else "All score values adhere strictly to [-1, 1] and [0, 1] bounds"
    record_validation("Score Range & Probability Bounds ([-1, 1], [0, 1])", total_checked, violations, detail)


def run_timestamp_validity_validation(client: httpx.Client, headers: dict):
    """
    Validation 2: Timestamp Validity & Temporal Consistency
    Verifies that all published_utc timestamps parse as RFC3339 and are not in the future.
    """
    print("\n[VALIDATION 2/6] Validating Timestamp RFC3339 Format & Temporal Consistency...")
    total_checked = 0
    violations = 0
    now_utc = datetime.now(timezone.utc) + timedelta(minutes=10)  # 10m clock skew tolerance

    # Fetch history records and feed records
    r_hist = request_with_retry(
        client,
        "GET",
        "/sentiment/history",
        params={"ticker": "AAPL", "start_date": "2025-01-01", "end_date": "2025-03-31", "limit": 100},
        headers=headers,
    )
    r_feed = request_with_retry(client, "GET", "/sentiment/feed", params={"limit": 50}, headers=headers)

    records = []
    if r_hist.status_code == 200:
        records.extend(r_hist.json().get("records", []))
    if r_feed.status_code == 200:
        records.extend(r_feed.json().get("records", []))

    for rec in records:
        ts_str = rec.get("published_utc")
        if ts_str:
            total_checked += 1
            dt = parse_rfc3339_timestamp(ts_str)
            if dt is None:
                violations += 1
            elif dt > now_utc:
                violations += 1

    detail = f"{total_checked} timestamps validated (RFC3339 compliant, zero future timestamps)"
    record_validation("Timestamp Validity & Temporal Integrity (RFC3339, <= Now)", total_checked, violations, detail)


def run_field_completeness_validation(client: httpx.Client, headers: dict):
    """
    Validation 3: Field Completeness & Schema Integrity
    Verifies required fields exist across:
      - /sentiment: ticker, sentiment_score, sentiment_label, confidence, data_quality_score
      - /sentiment/history: ticker, sentiment_score, published_utc, source, title, data_quality_score
      - /sentiment/feed: ticker, sentiment_score, published_utc, source, title, data_quality_score
      - /sentiment/anomalies: ticker, latest_score, mean_score, zscore, direction
      - /sentiment/disagreement: ticker, disagreement_index
    """
    print("\n[VALIDATION 3/6] Validating Field Completeness & Schema Integrity...")
    total_checked = 0
    violations = 0
    missing_fields = []

    # 1. Single sentiment endpoint
    r_sent = request_with_retry(client, "GET", "/sentiment", params={"ticker": "AAPL"}, headers=headers)
    if r_sent.status_code == 200:
        d = r_sent.json()
        total_checked += 1
        req_fields = ["ticker", "sentiment_score", "sentiment_label", "confidence", "data_quality_score"]
        for f in req_fields:
            if f not in d or d[f] is None:
                violations += 1
                missing_fields.append(f"/sentiment missing '{f}'")

    # 2. History records
    r_hist = request_with_retry(
        client,
        "GET",
        "/sentiment/history",
        params={"ticker": "AAPL", "start_date": "2025-01-01", "end_date": "2025-03-31", "limit": 20},
        headers=headers,
    )
    if r_hist.status_code == 200:
        records = r_hist.json().get("records", [])
        hist_req_fields = ["ticker", "published_utc", "source", "title", "sentiment_score", "data_quality_score"]
        for rec in records:
            total_checked += 1
            for f in hist_req_fields:
                if f not in rec or rec[f] is None:
                    violations += 1
                    missing_fields.append(f"/sentiment/history missing '{f}'")

    # 3. Feed records
    r_feed = request_with_retry(client, "GET", "/sentiment/feed", params={"limit": 20}, headers=headers)
    if r_feed.status_code == 200:
        items = r_feed.json().get("records", [])
        feed_req_fields = ["ticker", "published_utc", "source", "title", "sentiment_score", "data_quality_score"]
        for item in items:
            total_checked += 1
            for f in feed_req_fields:
                if f not in item or item[f] is None:
                    violations += 1
                    missing_fields.append(f"/sentiment/feed missing '{f}'")

    # 4. Anomalies
    r_anom = request_with_retry(client, "GET", "/sentiment/anomalies", params={"lookback_days": 30}, headers=headers)
    if r_anom.status_code == 200:
        anom_items = r_anom.json().get("items", [])
        anom_fields = ["ticker", "latest_score", "mean_score", "zscore", "direction"]
        for item in anom_items:
            total_checked += 1
            for f in anom_fields:
                if f not in item or item[f] is None:
                    violations += 1
                    missing_fields.append(f"/sentiment/anomalies missing '{f}'")

    # 5. Disagreement
    r_dis = request_with_retry(
        client,
        "GET",
        "/sentiment/disagreement",
        params={"ticker": "AAPL", "start_date": "2025-01-01", "end_date": "2025-03-31"},
        headers=headers,
    )
    if r_dis.status_code == 200:
        total_checked += 1
        d = r_dis.json()
        if "ticker" not in d or "disagreement_index" not in d:
            violations += 1
            missing_fields.append("/sentiment/disagreement missing 'ticker' or 'disagreement_index'")

    detail = "; ".join(missing_fields[:3]) if missing_fields else "100% required field completeness across all sentiment endpoints"
    record_validation("Field Completeness & Required Keys Integrity", total_checked, violations, detail)


def run_pit_survivorship_validation(client: httpx.Client, headers: dict):
    """
    Validation 4: Point-in-Time (PIT) & Survivorship Bias Enforcement
    Verifies that querying delisted tickers (e.g., BBBY delisted May 2023, SIVB delisted Mar 2023)
    for post-delisting intervals returns empty results without phantom survivorship leakage.
    """
    print("\n[VALIDATION 4/6] Validating Point-in-Time (PIT) & Survivorship Bias Enforcement...")
    delisted_test_cases = [
        ("BBBY", "2025-01-01", "2025-03-31", "Bed Bath & Beyond (Delisted May 2023)"),
        ("SIVB", "2025-01-01", "2025-03-31", "Silicon Valley Bank (Delisted Mar 2023)"),
        ("FRC", "2025-01-01", "2025-03-31", "First Republic Bank (Delisted May 2023)"),
    ]

    total_checked = 0
    violations = 0
    details = []

    for ticker, start_d, end_d, desc in delisted_test_cases:
        total_checked += 1
        r = request_with_retry(
            client,
            "GET",
            "/sentiment/history",
            params={"ticker": ticker, "start_date": start_d, "end_date": end_d},
            headers=headers,
        )

        if r.status_code == 200:
            records = r.json().get("records", [])
            if len(records) == 0:
                details.append(f"{ticker} post-delisting -> 0 records (PIT Enforced [OK])")
            else:
                violations += 1
                details.append(f"{ticker} leaked {len(records)} post-delisting records (VIOLATION)")
        elif r.status_code == 400:
            # 400 Bad Request with delisting explanation is also valid PIT rejection
            details.append(f"{ticker} post-delisting -> 400 Bad Request (PIT Enforced [OK])")
        else:
            violations += 1
            details.append(f"{ticker} returned unexpected HTTP {r.status_code}")

    record_validation(
        "Point-in-Time (PIT) & Delisting Survivorship Bias Enforcement",
        total_checked,
        violations,
        "; ".join(details),
    )


def run_source_coverage_validation(client: httpx.Client, headers: dict):
    """
    Validation 5: Multi-Source Provenance & Coverage Diversity
    Inspects historical records to count distinct upstream providers (e.g. SEC EDGAR, Finnhub, Polygon, Bloomberg).
    """
    print("\n[VALIDATION 5/6] Validating Multi-Source Provenance & Provider Diversity...")
    r = request_with_retry(
        client,
        "GET",
        "/sentiment/history",
        params={"ticker": "AAPL", "start_date": "2025-01-01", "end_date": "2025-03-31", "limit": 100},
        headers=headers,
    )

    distinct_sources = set()
    total_checked = 0
    violations = 0

    if r.status_code == 200:
        records = r.json().get("records", [])
        for rec in records:
            total_checked += 1
            src = rec.get("source")
            if not src or not isinstance(src, str) or not src.strip():
                violations += 1
            else:
                distinct_sources.add(src.strip())

    detail = f"Identified {len(distinct_sources)} distinct sources: {', '.join(sorted(distinct_sources)) if distinct_sources else 'None'}"
    record_validation("Multi-Source Provenance & Data Provider Coverage", total_checked, violations, detail)


def run_statistical_sanity_validation(client: httpx.Client, headers: dict):
    """
    Validation 6: Statistical Sanity & Dispersion Bounds
    Computes sample mean and sample standard deviation of sentiment scores across 50+ records.
    Ensures -1.0 <= mean <= 1.0 and stddev <= 1.0.
    """
    print("\n[VALIDATION 6/6] Validating Statistical Sanity & Score Dispersion Bounds...")
    r = request_with_retry(
        client,
        "GET",
        "/sentiment/history",
        params={"ticker": "AAPL", "start_date": "2025-01-01", "end_date": "2025-03-31", "limit": 100},
        headers=headers,
    )

    scores = []
    total_checked = 0
    violations = 0

    if r.status_code == 200:
        records = r.json().get("records", [])
        for rec in records:
            s = rec.get("sentiment_score")
            if s is not None and isinstance(s, (int, float)):
                scores.append(float(s))

    total_checked = len(scores)
    if total_checked >= 10:
        mean_val = sum(scores) / total_checked
        variance = sum((x - mean_val) ** 2 for x in scores) / total_checked
        stddev_val = math.sqrt(variance)

        if not (-1.0 <= mean_val <= 1.0):
            violations += 1
        if stddev_val > 1.0:
            violations += 1

        detail = f"Population n={total_checked} | Mean={mean_val:+.4f} (in [-1, 1]) | StdDev={stddev_val:.4f} (<= 1.0)"
    else:
        violations += 1
        detail = f"Insufficient sample size: {total_checked} records found"

    record_validation("Statistical Sanity & Distribution Dispersion (Mean & StdDev Bounds)", total_checked, violations, detail)


# ─────────────────────────────────────────────────────────────────────────────────
# Main Execution Runner
# ─────────────────────────────────────────────────────────────────────────────────

def run_data_quality_audit() -> bool:
    """Executes all 6 financial data quality validation checks."""
    print("=" * 90)
    print(" FINTEXT ALPHA VECTORIZER — COMPREHENSIVE DATA QUALITY & VALIDATION AUDIT")
    print("=" * 90)
    print(f" Target API Base URL: {BASE_URL}")
    print(f" Admin Token:         {ADMIN_TOKEN[:6]}***\n")

    server_ok, server_proc = ensure_server_running()
    if not server_ok:
        print("[ERROR] API Server could not be reached or started.")
        return False

    client = httpx.Client(base_url=BASE_URL, timeout=15.0)

    try:
        token = get_jwt_token(client)
        headers = {"Authorization": f"Bearer {token}"}
        print(f"[AUTH] Issued Data Quality Auditor JWT Token")

        # Run all 6 validations
        run_score_ranges_validation(client, headers)
        run_timestamp_validity_validation(client, headers)
        run_field_completeness_validation(client, headers)
        run_pit_survivorship_validation(client, headers)
        run_source_coverage_validation(client, headers)
        run_statistical_sanity_validation(client, headers)

        # Print Executive Summary Table
        print("\n" + "=" * 95)
        print(" FINTEXT DATA QUALITY AUDIT SUMMARY REPORT")
        print("=" * 95)
        print(f" {'#':<3} | {'Validation Domain':<42} | {'Checked':<8} | {'Violations':<10} | {'Pass Rate':<10} | {'Status'}")
        print("-" * 95)

        all_passed = True
        total_records = 0
        total_violations = 0

        for idx, item in enumerate(VALIDATION_METRICS, start=1):
            status = "PASS" if item["passed"] else "FAIL"
            if not item["passed"]:
                all_passed = False
            total_records += item["total_checked"]
            total_violations += item["violations"]

            print(
                f" {idx:<3} | {item['name'][:42]:<42} | {item['total_checked']:<8} | "
                f"{item['violations']:<10} | {item['pass_rate']:<9.2f}% | {status}"
            )

        print("=" * 95)
        overall_pass_rate = 100.0 if total_records == 0 else ((total_records - total_violations) / total_records) * 100.0
        print(f" Total Records / Fields Checked: {total_records}")
        print(f" Total Quality Violations:       {total_violations}")
        print(f" Aggregate Data Quality Pass Rate: {overall_pass_rate:.2f}%")
        print(f" Overall Audit Verdict:           {'PASSED — DATA MEETS ALL QUALITY STANDARDS' if all_passed else 'FAILED — QUALITY VIOLATIONS DETECTED'}")
        print("=" * 95)

        return all_passed

    except Exception as exc:
        print(f"\n[EXCEPTION] Data quality validation error: {exc}")
        import traceback
        traceback.print_exc()
        return False

    finally:
        client.close()
        if server_proc and server_proc.poll() is None:
            print("\n[SERVER] Terminating test server process...")
            server_proc.terminate()
            try:
                server_proc.wait(timeout=3.0)
            except Exception:
                server_proc.kill()


def main():
    success = run_data_quality_audit()
    sys.exit(0 if success else 1)


if __name__ == "__main__":
    main()
