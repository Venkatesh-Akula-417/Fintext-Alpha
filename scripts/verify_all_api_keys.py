#!/usr/bin/env python3
"""
═══════════════════════════════════════════════════════════════════════════════
FinText-Alpha-Vectorizer — Active Ingestion Source & API Key Verification Suite
═══════════════════════════════════════════════════════════════════════════════
Verifies active data sources in accordance with Data Licensing & Redistribution Compliance:
1. SEC EDGAR (Public Domain Regulatory Filings — Safe for Redistribution)
2. Polygon.io (Options Microstructure & OHLCV Daily Bars — Active)
3. Finnhub (Real-Time Market News — Active)

Masks keys (first 4 & last 4 chars), measures HTTP round-trip latency (ms),
and validates live payload schemas.
═══════════════════════════════════════════════════════════════════════════════
"""

import os
import sys
import time
import requests
from dotenv import load_dotenv

# Ensure stdout handles unicode/formatting gracefully
if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")

USER_AGENT = "FinText Institutional Research/2.0 (contact@fintext-alpha.com)"
TIMEOUT_SECS = 10.0


def mask_key(key: str) -> str:
    """Masks an API key showing only first 4 and last 4 characters."""
    if not key:
        return "<NOT_SET>"
    clean = key.strip()
    if clean.upper() in ("PUBLIC", "KEYLESS", "<PUBLIC>"):
        return "<PUBLIC>"
    if len(clean) <= 8:
        return "********"
    return f"{clean[:4]}...{clean[-4:]}"


def is_placeholder(key: str) -> bool:
    """Checks if the key is empty or default placeholder."""
    if not key:
        return True
    clean = key.strip().lower()
    placeholders = [
        "",
        "your_polygon_api_key_here",
        "your_finnhub_api_key_here",
        "placeholder",
        "none",
    ]
    return clean in placeholders


def test_sec_edgar(key: str = "") -> dict:
    """Tests public SEC EDGAR company submission endpoint."""
    cik = "0000320193"  # AAPL
    url = f"https://data.sec.gov/submissions/CIK{cik}.json"
    headers = {"User-Agent": USER_AGENT}
    t0 = time.perf_counter()
    resp = requests.get(url, headers=headers, timeout=TIMEOUT_SECS)
    latency_ms = (time.perf_counter() - t0) * 1000.0

    status_code = resp.status_code
    if status_code == 200:
        data = resp.json()
        entity_name = data.get("name", "Apple Inc.")
        filings = data.get("filings", {}).get("recent", {}).get("form", [])
        return {
            "status": status_code,
            "latency_ms": latency_ms,
            "result": "PASS",
            "message": f"Valid public data (entity='{entity_name}', recent_filings={len(filings)})",
        }
    elif status_code == 429:
        return {
            "status": status_code,
            "latency_ms": latency_ms,
            "result": "FAIL",
            "message": "SEC EDGAR rate limit exceeded (HTTP 429)",
        }
    else:
        return {
            "status": status_code,
            "latency_ms": latency_ms,
            "result": "FAIL",
            "message": f"HTTP {status_code}: {resp.text[:80]}",
        }


def test_polygon(key: str) -> dict:
    """Tests Polygon.io options / aggregate bar endpoint."""
    url = f"https://api.polygon.io/v2/aggs/ticker/AAPL/prev?adjusted=true&apiKey={key}"
    headers = {"User-Agent": USER_AGENT}
    t0 = time.perf_counter()
    resp = requests.get(url, headers=headers, timeout=TIMEOUT_SECS)
    latency_ms = (time.perf_counter() - t0) * 1000.0

    status_code = resp.status_code
    if status_code == 200:
        data = resp.json()
        status_field = data.get("status")
        results_count = data.get("resultsCount", 0)
        results = data.get("results", [])
        if status_field == "OK" and (results_count > 0 or len(results) > 0):
            return {
                "status": status_code,
                "latency_ms": latency_ms,
                "result": "PASS",
                "message": f"Valid (status=OK, results={len(results)})",
            }
        else:
            return {
                "status": status_code,
                "latency_ms": latency_ms,
                "result": "FAIL",
                "message": f"Unexpected payload: status={status_field}",
            }
    elif status_code in (401, 403):
        return {
            "status": status_code,
            "latency_ms": latency_ms,
            "result": "FAIL",
            "message": "Authentication failed (invalid key)",
        }
    elif status_code == 429:
        return {
            "status": status_code,
            "latency_ms": latency_ms,
            "result": "FAIL",
            "message": "Rate limit exceeded (HTTP 429)",
        }
    else:
        return {
            "status": status_code,
            "latency_ms": latency_ms,
            "result": "FAIL",
            "message": f"HTTP {status_code}: {resp.text[:80]}",
        }


def test_finnhub(key: str) -> dict:
    """Tests Finnhub financial news feed endpoint."""
    url = f"https://finnhub.io/api/v1/news?category=general&token={key}"
    headers = {"User-Agent": USER_AGENT}
    t0 = time.perf_counter()
    resp = requests.get(url, headers=headers, timeout=TIMEOUT_SECS)
    latency_ms = (time.perf_counter() - t0) * 1000.0

    status_code = resp.status_code
    if status_code == 200:
        data = resp.json()
        if isinstance(data, list):
            if len(data) > 0:
                first_title = data[0].get("headline", "")[:35]
                return {
                    "status": status_code,
                    "latency_ms": latency_ms,
                    "result": "PASS",
                    "message": f"Valid ({len(data)} articles; sample: '{first_title}...')",
                }
            else:
                return {
                    "status": status_code,
                    "latency_ms": latency_ms,
                    "result": "PASS",
                    "message": "Valid (0 articles returned)",
                }
        elif isinstance(data, dict) and "error" in data:
            return {
                "status": status_code,
                "latency_ms": latency_ms,
                "result": "FAIL",
                "message": f"API Error: {data.get('error')}",
            }
        else:
            return {
                "status": status_code,
                "latency_ms": latency_ms,
                "result": "FAIL",
                "message": "Unexpected JSON response format",
            }
    elif status_code in (401, 403):
        return {
            "status": status_code,
            "latency_ms": latency_ms,
            "result": "FAIL",
            "message": "Authentication failed (invalid token)",
        }
    elif status_code == 429:
        return {
            "status": status_code,
            "latency_ms": latency_ms,
            "result": "FAIL",
            "message": "Rate limit exceeded (HTTP 429)",
        }
    else:
        return {
            "status": status_code,
            "latency_ms": latency_ms,
            "result": "FAIL",
            "message": f"HTTP {status_code}: {resp.text[:80]}",
        }


def main():
    print("=" * 95)
    print(" FinText-Alpha-Vectorizer — Active Ingestion Source & API Key Verification")
    print("=" * 95)
    print(" [Compliance] Active Sources: SEC EDGAR (Public), Polygon.io (Options/Bars), Finnhub (Market News)")
    print("=" * 95)

    # Load environment variables
    dotenv_path = os.path.abspath(".env")
    if os.path.exists(dotenv_path):
        load_dotenv(dotenv_path, override=True)
        print(f" Loaded environment from: {dotenv_path}")
    else:
        print(" [WARNING] .env file not found in project root; checking system environment.")

    active_providers = [
        ("SEC EDGAR", "ENABLE_SEC_EDGAR", test_sec_edgar, "PUBLIC"),
        ("Polygon.io", "POLYGON_API_KEY", test_polygon, None),
        ("Finnhub", "FINNHUB_API_KEY", test_finnhub, None),
    ]

    results = []

    # Test Active Providers
    print("\n[ACTIVE DATA SOURCES]")
    for name, env_var, test_fn, default_key in active_providers:
        key = default_key if default_key == "PUBLIC" else os.environ.get(env_var, "").strip()
        masked = mask_key(key)

        print(f"\n[+] Testing {name} ({env_var})...")
        print(f"    Key: {masked}")

        if key != "PUBLIC" and is_placeholder(key):
            print(f"    Result: SKIPPED (Key is missing or default placeholder)")
            results.append({
                "provider": name,
                "env_var": env_var,
                "masked_key": masked,
                "status": "N/A",
                "latency_ms": 0.0,
                "result": "SKIPPED",
                "message": "Key is missing or default placeholder in .env",
            })
            continue

        try:
            res = test_fn(key)
            print(f"    HTTP Status : {res['status']}")
            print(f"    Latency     : {res['latency_ms']:.2f} ms")
            print(f"    Status      : {res['result']}")
            print(f"    Message     : {res['message']}")

            results.append({
                "provider": name,
                "env_var": env_var,
                "masked_key": masked,
                "status": str(res["status"]),
                "latency_ms": res["latency_ms"],
                "result": res["result"],
                "message": res["message"],
            })
        except requests.exceptions.Timeout:
            print(f"    [!] Timeout connecting to {name} after {TIMEOUT_SECS}s")
            results.append({
                "provider": name,
                "env_var": env_var,
                "masked_key": masked,
                "status": "TIMEOUT",
                "latency_ms": TIMEOUT_SECS * 1000.0,
                "result": "FAIL",
                "message": f"Connection timed out after {TIMEOUT_SECS}s",
            })
        except requests.exceptions.ConnectionError as e:
            print(f"    [!] Connection error for {name}: {e}")
            results.append({
                "provider": name,
                "env_var": env_var,
                "masked_key": masked,
                "status": "CONN_ERR",
                "latency_ms": 0.0,
                "result": "FAIL",
                "message": "Network connection error",
            })
        except Exception as e:
            print(f"    [!] Unexpected exception for {name}: {e}")
            results.append({
                "provider": name,
                "env_var": env_var,
                "masked_key": masked,
                "status": "ERROR",
                "latency_ms": 0.0,
                "result": "FAIL",
                "message": str(e)[:60],
            })

    # Print summary table
    print("\n" + "=" * 95)
    print(" INGESTION SOURCE & API KEY VERIFICATION SUMMARY")
    print("=" * 95)
    header = f"{'Provider':<15} | {'Key':<12} | {'HTTP':<8} | {'Latency (ms)':<13} | {'Result':<10} | {'Details'}"
    print(header)
    print("-" * 95)

    passed_count = 0
    failed_count = 0
    skipped_count = 0

    for r in results:
        lat_str = f"{r['latency_ms']:.1f}" if r["latency_ms"] > 0 else "N/A"
        res_str = r["result"]
        if res_str == "PASS":
            passed_count += 1
        elif res_str == "FAIL":
            failed_count += 1
        else:
            skipped_count += 1

        print(
            f"{r['provider']:<15} | {r['masked_key']:<12} | {r['status']:<8} | {lat_str:<13} | {res_str:<10} | {r['message']}"
        )

    print("=" * 95)
    print(f" Total: {len(results)} | Passed: {passed_count} | Failed: {failed_count} | Skipped: {skipped_count}")
    print("=" * 95)

    if failed_count > 0:
        sys.exit(1)
    sys.exit(0)


if __name__ == "__main__":
    main()
