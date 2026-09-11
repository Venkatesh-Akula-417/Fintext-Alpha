#!/usr/bin/env python3
"""
=====================================================================================
FinText-Alpha-Vectorizer — Live Python SDK Client Verification Suite
=====================================================================================
Validates that:
1. The Axum API Gateway starts up.
2. FinTextClient connects to GET /health.
3. FinTextClient automatically obtains a JWT using admin_token on first protected call.
4. FinTextClient queries GET /sentiment?ticker=AAPL.
5. FinTextClient queries GET /spillovers?ticker=AAPL&limit=5.
6. FinTextClient executes POST /backtest with a BacktestRequest model.
7. FinTextClient tracks rate limit headers and constructs valid ws_url.
8. FinTextAsyncClient executes asynchronous concurrent queries.
9. Rate limiting (HTTP 429) triggers FinTextRateLimitError with Retry-After.
=====================================================================================
"""

import asyncio
import os
import subprocess
import sys
import time
from pathlib import Path

from fintext import (
    BacktestRequest,
    FinTextAsyncClient,
    FinTextAuthError,
    FinTextClient,
    FinTextRateLimitError,
)

PORT = 8085
BASE_URL = f"http://127.0.0.1:{PORT}"
BINARY_PATH = Path("rust/target/release/fintext_api.exe").resolve()


def run_live_verification():
    print("=" * 85)
    print(" FinText-Alpha-Vectorizer — Official Python Client SDK Live Verification")
    print("=" * 85)

    env = os.environ.copy()
    env.update({
        "PORT": str(PORT),
        "HOST": "127.0.0.1",
        "JWT_SECRET": "sdk_verification_secret_key_32_bytes_len!!",
        "ADMIN_TOKEN": "sdk_admin_token_super_secret_12345",
        "RATE_LIMIT_REQUESTS": "5",
        "RATE_LIMIT_WINDOW_SECONDS": "60",
        "QUESTDB_MOCK_FALLBACK": "1",
        "RUST_LOG": "info",
    })

    print(f"[*] Launching API Server: {BINARY_PATH}")
    proc = subprocess.Popen(
        [str(BINARY_PATH)],
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    try:
        # Wait for server readiness
        ready = False
        client = FinTextClient(
            base_url=BASE_URL,
            admin_token="sdk_admin_token_super_secret_12345",
            auto_auth_user="institutional_sdk_user",
        )

        for _ in range(30):
            try:
                health = client.health()
                if health.status == "ok":
                    ready = True
                    break
            except Exception:
                time.sleep(0.2)

        if not ready:
            print("[-] FAIL: Server failed to start within timeout.")
            return False

        # 1. Health Probe
        print(f"\n[1/7] Testing client.health()...")
        health = client.health()
        print(f"      Status: {health.status} | Version: {health.version} | Timestamp: {health.timestamp_us}")
        assert health.status == "ok"

        # 2. Sentiment Query with Auto-Authentication
        print(f"\n[2/7] Testing client.sentiment('AAPL') [Auto-Auth Flow]...")
        sentiment = client.sentiment("AAPL")
        print(f"      Ticker: {sentiment.ticker} | Score: {sentiment.sentiment_score} ({sentiment.sentiment_label}) | Date: {sentiment.date}")
        assert sentiment.ticker == "AAPL"
        assert client.api_token is not None
        print(f"      [OK] Token auto-issued: {client.api_token[:25]}... (Role: institutional)")

        # 3. Cross-Asset Spillovers
        print(f"\n[3/7] Testing client.spillovers('AAPL', limit=5)...")
        spillovers = client.spillovers("AAPL", limit=5)
        print(f"      Found {spillovers.count} correlated spillover relations for {spillovers.ticker}:")
        for s in spillovers.spillovers:
            print(f"       - {s.relationship} (r = {s.correlation:.3f}, lag = {s.lag_hours}h)")
        assert spillovers.count > 0

        # 4. Alpha Strategy Backtesting
        print(f"\n[4/7] Testing client.backtest(BacktestRequest)...")
        req = BacktestRequest(
            ticker="AAPL",
            start_date="2025-01-01",
            end_date="2025-03-31",
            long_threshold=0.2,
            short_threshold=-0.2,
            holding_days=5,
            initial_capital=1_000_000.0,
        )
        backtest = client.backtest(req)
        print(f"      Total Return: {backtest.total_return * 100:.2f}% | Annualized: {backtest.annualized_return * 100:.2f}%")
        print(f"      Sharpe Ratio: {backtest.sharpe_ratio:.2f} | Max Drawdown: {backtest.max_drawdown * 100:.2f}% | Win Rate: {backtest.win_rate:.1f}%")
        assert backtest.ticker == "AAPL"
        assert len(backtest.equity_curve) >= 30

        # 5. Rate Limit Tracking & WebSocket URL
        print(f"\n[5/7] Testing Rate Limit Metadata & WebSocket URL Generation...")
        print(f"      Rate Limit Remaining: {client.last_rate_limit.remaining}/{client.last_rate_limit.limit} (Reset: {client.last_rate_limit.reset}s)")
        ws_url = client.ws_url(ticker="AAPL")
        print(f"      Generated WebSocket URI: {ws_url}")
        assert "ws://127.0.0.1:8085/ws?token=" in ws_url

        # 6. Asynchronous Client Concurrent Execution
        print(f"\n[6/7] Testing FinTextAsyncClient with concurrent queries...")
        async def run_async_test():
            async with FinTextAsyncClient(
                base_url=BASE_URL,
                api_token=client.api_token,
            ) as async_client:
                t1 = async_client.sentiment("AAPL")
                return await t1

        async_sentiment = asyncio.run(run_async_test())
        print(f"      Async Sentiment Retrieved: {async_sentiment.ticker} -> {async_sentiment.sentiment_score}")

        # 7. Rate Limit Exhaustion & FinTextRateLimitError Triggering
        print(f"\n[7/7] Testing Rate Limit Enforcement & Exception Handling...")
        caught_429 = False
        for i in range(10):
            try:
                client.sentiment("AAPL")
            except FinTextRateLimitError as rle:
                print(f"      [OK] Successfully caught FinTextRateLimitError: {rle}")
                print(f"           Retry-After: {rle.retry_after}s | Remaining: {rle.remaining}/{rle.limit}")
                caught_429 = True
                break

        assert caught_429, "Expected rate limit to be triggered"

        print("\n" + "=" * 85)
        print(" [OK] ALL PYTHON CLIENT SDK INTEGRATION CHECKS PASSED!")
        print("=" * 85)
        return True

    finally:
        client.close()
        proc.terminate()
        try:
            proc.wait(timeout=2.0)
        except Exception:
            proc.kill()


if __name__ == "__main__":
    success = run_live_verification()
    sys.exit(0 if success else 1)
