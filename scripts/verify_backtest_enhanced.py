#!/usr/bin/env python3
"""
═══════════════════════════════════════════════════════════════════════════════
FinText-Alpha-Vectorizer — Enhanced Multi-Asset Portfolio Backtest Verifier
═══════════════════════════════════════════════════════════════════════════════
Verifies the enhanced /backtest endpoint supporting multi-asset portfolios,
custom/equal weights, turnover transaction costs (bps), Sortino ratio,
profit factor, benchmark comparison, and equity points.
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

def run_tests():
    from fintext.client import FinTextClient
    from fintext.models import BacktestRequest, BacktestResponse

    print_header("FinText Enhanced Multi-Asset Backtest Verification Suite")

    passed_count = 0
    failed_count = 0

    # 1. Test Single Ticker Legacy Compatibility Model
    try:
        req = BacktestRequest(
            ticker="AAPL",
            start_date="2025-01-01",
            end_date="2025-03-31",
            long_threshold=0.2,
            short_threshold=-0.2,
            holding_days=5,
            initial_capital=1_000_000.0,
        )
        assert req.ticker == "AAPL"
        assert req.benchmark_ticker == "SPY"
        assert req.transaction_cost_bps == 5.0
        test_passed("Model: Single Ticker Legacy BacktestRequest serialization")
        passed_count += 1
    except Exception as e:
        test_failed("Model: Single Ticker Legacy BacktestRequest", str(e))
        failed_count += 1

    # 2. Test Multi-Asset Portfolio Model with Custom Weights
    try:
        req = BacktestRequest(
            tickers=["AAPL", "NVDA", "MSFT"],
            weights=[0.5, 0.3, 0.2],
            benchmark_ticker="SPY",
            transaction_cost_bps=10.0,
            start_date="2025-01-01",
            end_date="2025-03-31",
            long_threshold=0.2,
            short_threshold=-0.2,
            holding_days=5,
            initial_capital=1_000_000.0,
        )
        d = req.model_dump(exclude_none=True)
        assert d["tickers"] == ["AAPL", "NVDA", "MSFT"]
        assert d["weights"] == [0.5, 0.3, 0.2]
        assert d["benchmark_ticker"] == "SPY"
        assert d["transaction_cost_bps"] == 10.0
        test_passed("Model: Multi-Asset BacktestRequest serialization with weights and costs")
        passed_count += 1
    except Exception as e:
        test_failed("Model: Multi-Asset BacktestRequest", str(e))
        failed_count += 1

    # 3. Test Response Model Deserialization & Metric Fields
    try:
        sample_resp_data = {
            "ticker": "AAPL, NVDA, MSFT",
            "tickers": ["AAPL", "NVDA", "MSFT"],
            "weights": [0.5, 0.3, 0.2],
            "benchmark_ticker": "SPY",
            "start_date": "2025-01-01",
            "end_date": "2025-03-31",
            "total_return": 0.1852,
            "annualized_return": 0.6214,
            "sharpe_ratio": 2.45,
            "sortino_ratio": 3.82,
            "max_drawdown": -0.0385,
            "num_trades": 15,
            "win_rate": 73.3,
            "profit_factor": 2.45,
            "transaction_cost_bps": 10.0,
            "equity_curve": [1000000.0, 1050000.0, 1185200.0],
            "equity_points": [
                {"date": "2025-01-01", "portfolio_value": 1000000.0, "daily_return": 0.0},
                {"date": "2025-01-02", "portfolio_value": 1050000.0, "daily_return": 0.05},
                {"date": "2025-01-03", "portfolio_value": 1185200.0, "daily_return": 0.1287},
            ],
            "benchmark_curve": [1000000.0, 1010000.0, 1025000.0],
            "benchmark_total_return": 0.025,
            "alpha": 0.1602,
            "message": "Point-in-time multi-asset backtest simulated successfully",
        }
        resp = BacktestResponse.model_validate(sample_resp_data)
        assert resp.tickers == ["AAPL", "NVDA", "MSFT"]
        assert resp.weights == [0.5, 0.3, 0.2]
        assert resp.sortino_ratio == 3.82
        assert resp.profit_factor == 2.45
        assert resp.transaction_cost_bps == 10.0
        assert len(resp.equity_points) == 3
        assert resp.equity_points[1].daily_return == 0.05
        assert resp.benchmark_total_return == 0.025
        assert resp.alpha == 0.1602
        test_passed("Model: BacktestResponse parsing with Sortino, Profit Factor, Alpha, and Equity Points")
        passed_count += 1
    except Exception as e:
        test_failed("Model: BacktestResponse parsing", str(e))
        failed_count += 1

    # 4. Test Client Integration with Mock Transport
    try:
        import httpx
        def mock_handler(request: httpx.Request) -> httpx.Response:
            if request.url.path == "/backtest":
                body = json.loads(request.content)
                tickers = body.get("tickers") or ([body["ticker"]] if "ticker" in body else ["AAPL"])
                weights = body.get("weights") or [1.0 / len(tickers)] * len(tickers)
                return httpx.Response(
                    200,
                    json={
                        "ticker": ", ".join(tickers),
                        "tickers": tickers,
                        "weights": weights,
                        "benchmark_ticker": body.get("benchmark_ticker", "SPY"),
                        "start_date": body["start_date"],
                        "end_date": body["end_date"],
                        "total_return": 0.142,
                        "annualized_return": 0.568,
                        "sharpe_ratio": 2.15,
                        "sortino_ratio": 3.42,
                        "max_drawdown": -0.048,
                        "num_trades": 12,
                        "win_rate": 69.2,
                        "profit_factor": 2.15,
                        "transaction_cost_bps": body.get("transaction_cost_bps", 5.0),
                        "equity_curve": [1000000.0, 1050000.0, 1142000.0],
                        "equity_points": [
                            {"date": "2025-01-01", "portfolio_value": 1000000.0, "daily_return": 0.0},
                            {"date": "2025-01-02", "portfolio_value": 1050000.0, "daily_return": 0.05},
                            {"date": "2025-01-03", "portfolio_value": 1142000.0, "daily_return": 0.0876},
                        ],
                        "benchmark_curve": [1000000.0, 1010000.0, 1030000.0],
                        "benchmark_total_return": 0.030,
                        "alpha": 0.112,
                        "message": "Mock portfolio backtest executed successfully",
                    }
                )
            return httpx.Response(404, json={"error": "Not Found"})

        client = FinTextClient(
            base_url="http://mock.fintext",
            api_token="mock_token",
            transport=httpx.MockTransport(mock_handler),
        )

        res = client.backtest({
            "tickers": ["AAPL", "NVDA", "MSFT", "GOOGL"],
            "weights": [0.3, 0.3, 0.2, 0.2],
            "transaction_cost_bps": 7.5,
            "benchmark_ticker": "SPY",
            "start_date": "2025-01-01",
            "end_date": "2025-03-31",
        })

        assert res.tickers == ["AAPL", "NVDA", "MSFT", "GOOGL"]
        assert len(res.weights) == 4
        assert res.transaction_cost_bps == 7.5
        assert res.sortino_ratio == 3.42
        assert res.profit_factor == 2.15
        assert len(res.equity_points) == 3
        test_passed("SDK Client: Multi-asset dictionary request execution via FinTextClient")
        passed_count += 1
    except Exception as e:
        test_failed("SDK Client: Multi-asset request", str(e))
        failed_count += 1

    # Summary
    print_header("Enhanced Multi-Asset Backtest Verification Summary")
    print(f" Total Tests Run: {passed_count + failed_count}")
    print(f" {GREEN}Passed: {passed_count}{RESET}")
    print(f" {RED}Failed: {failed_count}{RESET}")

    if failed_count > 0:
        sys.exit(1)
    else:
        print(f"\n {GREEN}{BOLD}ALL ENHANCED BACKTEST VERIFICATIONS PASSED SUCCESSFULLY!{RESET}\n")

if __name__ == "__main__":
    run_tests()
