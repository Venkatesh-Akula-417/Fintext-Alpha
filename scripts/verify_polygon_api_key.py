import os
import sys
import time
import json
import urllib.request
import urllib.error
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parent.parent
sys.stdout.reconfigure(encoding='utf-8')

def load_polygon_key():
    env_path = PROJECT_ROOT / ".env"
    if not env_path.exists():
        return None
    for line in env_path.read_text(encoding='utf-8-sig', errors='ignore').splitlines():
        line = line.strip()
        if line and not line.startswith('#') and '=' in line:
            k, v = line.split('=', 1)
            if k.strip() == 'POLYGON_API_KEY':
                return v.strip().strip("'").strip('"')
    return None

def mask_key(key):
    if not key:
        return "<MISSING>"
    if len(key) <= 8:
        return key[:2] + "****" + key[-2:]
    return key[:4] + "*" * (len(key) - 8) + key[-4:]

def test_endpoint(url, description):
    print(f"Testing {description}...")
    t0 = time.perf_counter()
    req = urllib.request.Request(
        url,
        headers={"User-Agent": "FinText-Alpha-Vectorizer/2.0.0"}
    )
    try:
        with urllib.request.urlopen(req, timeout=10) as resp:
            latency_ms = (time.perf_counter() - t0) * 1000.0
            status_code = resp.status
            raw_body = resp.read().decode('utf-8')
            data = json.loads(raw_body)
            return {
                "status_code": status_code,
                "latency_ms": latency_ms,
                "data": data,
                "error": None
            }
    except urllib.error.HTTPError as e:
        latency_ms = (time.perf_counter() - t0) * 1000.0
        err_body = e.read().decode('utf-8', errors='ignore')
        try:
            err_data = json.loads(err_body)
        except Exception:
            err_data = {"raw_error": err_body}
        return {
            "status_code": e.code,
            "latency_ms": latency_ms,
            "data": err_data,
            "error": str(e)
        }
    except Exception as e:
        latency_ms = (time.perf_counter() - t0) * 1000.0
        return {
            "status_code": 0,
            "latency_ms": latency_ms,
            "data": None,
            "error": str(e)
        }

def main():
    print("=" * 80)
    print(" Polygon.io API Key & Market Data Access Verification")
    print("=" * 80)
    
    key = load_polygon_key()
    if not key:
        print("ERROR: POLYGON_API_KEY not found in .env!")
        sys.exit(1)
        
    masked = mask_key(key)
    print(f"Loaded API Key: {masked} (Length: {len(key)} chars)\n")
    
    # 1. Stocks Previous Close Aggregate
    stocks_url = f"https://api.polygon.io/v2/aggs/ticker/AAPL/prev?adjusted=true&apiKey={key}"
    stocks_res = test_endpoint(stocks_url, "Stocks Aggregate (AAPL Prev Day)")
    print(f"  Status Code: {stocks_res['status_code']}")
    print(f"  Latency:     {stocks_res['latency_ms']:.2f} ms")
    if stocks_res['data']:
        status_val = stocks_res['data'].get('status')
        results_count = stocks_res['data'].get('resultsCount', len(stocks_res['data'].get('results', [])))
        print(f"  API Status:  {status_val}")
        print(f"  Results:     {results_count} items")
        if 'results' in stocks_res['data'] and stocks_res['data']['results']:
            print(f"  Sample Data: {stocks_res['data']['results'][0]}")
    else:
        print(f"  Error:       {stocks_res['error']}")
    print("-" * 60)
    
    # 2. Options Reference Contracts
    options_url = f"https://api.polygon.io/v3/reference/options/contracts?underlying_ticker=AAPL&limit=1&apiKey={key}"
    options_res = test_endpoint(options_url, "Options Reference Contracts (AAPL)")
    print(f"  Status Code: {options_res['status_code']}")
    print(f"  Latency:     {options_res['latency_ms']:.2f} ms")
    if options_res['data']:
        status_val = options_res['data'].get('status')
        results_count = len(options_res['data'].get('results', []))
        print(f"  API Status:  {status_val}")
        print(f"  Results:     {results_count} items")
        if 'results' in options_res['data'] and options_res['data']['results']:
            print(f"  Sample Contract: {stocks_res['data'].get('results', [{}])[0] if 'results' in options_res['data'] else ''}")
            sample = options_res['data']['results'][0]
            print(f"  Contract Ticker: {sample.get('ticker')}, Strike: ${sample.get('strike_price')}, Expiry: {sample.get('expiration_date')}")
    else:
        print(f"  Error:       {options_res['error']}")
        
    print("=" * 80)
    
    # Verdict calculation
    stocks_ok = stocks_res['status_code'] == 200 and stocks_res['data'].get('status') == 'OK'
    options_ok = options_res['status_code'] == 200 and options_res['data'].get('status') == 'OK'
    
    if stocks_ok and options_ok:
        verdict = "VALID (FULL ACCESS: STOCKS & OPTIONS)"
    elif stocks_ok and not options_ok:
        verdict = "PARTIAL (STOCKS VALID, OPTIONS RESTRICTED)"
    elif stocks_res['status_code'] == 429 or options_res['status_code'] == 429:
        verdict = "RATE-LIMITED (HTTP 429)"
    elif stocks_res['status_code'] in (401, 403):
        verdict = "UNAUTHORIZED / INVALID KEY"
    else:
        verdict = f"FAILED (HTTP {stocks_res['status_code']})"
        
    print(f"OVERALL VERDICT: {verdict}")

if __name__ == '__main__':
    main()
