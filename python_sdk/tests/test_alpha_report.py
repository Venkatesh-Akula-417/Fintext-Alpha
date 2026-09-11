"""
═══════════════════════════════════════════════════════════════════════════════
FinText Alpha Vectorizer Python Client SDK — Alpha Validation Report Tests
═══════════════════════════════════════════════════════════════════════════════
"""

import httpx
import pytest
from fintext import FinTextClient, FinTextAsyncClient
from fintext.exceptions import FinTextValidationError
from fintext.models import (
    AlphaSignalConfig,
    AlphaReportRequest,
    AlphaReportResponse,
    PerformanceMetrics,
    EquityCurvePoint,
    AlphaReport,
)


SAMPLE_ALPHA_REPORT = {
    "tickers": ["AAPL", "MSFT", "NVDA"],
    "start_date": "2024-01-01",
    "end_date": "2024-12-31",
    "signal_config": {
        "signal_type": "sentiment",
        "threshold_long": 0.2,
        "threshold_short": -0.2,
        "holding_days": 5,
        "smoothing_window_days": 3,
    },
    "initial_capital": 1000000.0,
    "metrics": {
        "total_return": 0.2450,
        "annualized_return": 0.2475,
        "annualized_volatility": 0.1420,
        "sharpe_ratio": 1.74,
        "sortino_ratio": 2.45,
        "max_drawdown": 0.0680,
        "win_rate": 64.5,
        "profit_factor": 1.85,
        "total_trades": 28,
        "avg_holding_period_days": 4.8,
        "benchmark_ticker": "SPY",
        "benchmark_total_return": 0.1250,
        "benchmark_annualized_return": 0.1265,
        "benchmark_annualized_volatility": 0.1580,
        "alpha": 0.1200,
        "beta": 0.82,
        "information_ratio": 1.15,
        "tracking_error": 0.0980,
    },
    "equity_curve": [
        {
            "date": "2024-01-02",
            "portfolio_value": 1002500.0,
            "benchmark_value": 1001200.0,
            "strategy_daily_return": 0.0025,
            "benchmark_daily_return": 0.0012,
            "position": 1,
        },
        {
            "date": "2024-01-03",
            "portfolio_value": 1005100.0,
            "benchmark_value": 1002400.0,
            "strategy_daily_return": 0.0026,
            "benchmark_daily_return": 0.0012,
            "position": 1,
        },
        {
            "date": "2024-01-04",
            "portfolio_value": 1003050.0,
            "benchmark_value": 1001800.0,
            "strategy_daily_return": -0.0020,
            "benchmark_daily_return": -0.0006,
            "position": 0,
        },
    ],
    "generated_at": "2024-12-31T23:59:59Z",
    "message": "Alpha report generated from QuestDB sentiment and market price stores. 3 tickers evaluated over 252 trading days.",
}


def test_alpha_report_sync_client():
    """Verify synchronous client.generate_alpha_report() returns structured AlphaReportResponse."""
    captured_payloads = []

    def mock_handler(request: httpx.Request):
        assert request.url.path == "/signals/alpha-report"
        assert request.method == "POST"
        assert "Bearer test_jwt_token" in request.headers.get("Authorization", "")
        import json
        captured_payloads.append(json.loads(request.content))
        return httpx.Response(200, json=SAMPLE_ALPHA_REPORT)

    transport = httpx.MockTransport(mock_handler)
    client = FinTextClient(base_url="http://testserver", api_token="test_jwt_token")
    client._client = httpx.Client(transport=transport, base_url="http://testserver")

    report = client.generate_alpha_report(
        tickers=["AAPL", "MSFT", "NVDA"],
        start_date="2024-01-01",
        end_date="2024-12-31",
        signal_config=AlphaSignalConfig(
            signal_type="sentiment",
            threshold_long=0.2,
            threshold_short=-0.2,
            holding_days=5,
            smoothing_window_days=3,
        ),
    )

    # ── Verify request payload ──────────────────────────────────────────────
    assert len(captured_payloads) == 1
    payload = captured_payloads[0]
    assert payload["tickers"] == ["AAPL", "MSFT", "NVDA"]
    assert payload["start_date"] == "2024-01-01"
    assert payload["end_date"] == "2024-12-31"
    assert payload["signal_config"]["signal_type"] == "sentiment"
    assert payload["signal_config"]["threshold_long"] == 0.2
    assert payload["signal_config"]["holding_days"] == 5
    assert payload["benchmark_ticker"] == "SPY"
    assert payload["initial_capital"] == 1_000_000.0

    # ── Verify response type and top-level fields ───────────────────────────
    assert isinstance(report, AlphaReportResponse)
    assert report.tickers == ["AAPL", "MSFT", "NVDA"]
    assert report.start_date == "2024-01-01"
    assert report.end_date == "2024-12-31"
    assert report.initial_capital == pytest.approx(1_000_000.0)
    assert report.generated_at == "2024-12-31T23:59:59Z"

    # ── Verify signal config round-trip ─────────────────────────────────────
    assert isinstance(report.signal_config, AlphaSignalConfig)
    assert report.signal_config.signal_type == "sentiment"
    assert report.signal_config.threshold_long == pytest.approx(0.2)
    assert report.signal_config.threshold_short == pytest.approx(-0.2)
    assert report.signal_config.holding_days == 5
    assert report.signal_config.smoothing_window_days == 3

    # ── Verify performance metrics ──────────────────────────────────────────
    m = report.metrics
    assert isinstance(m, PerformanceMetrics)
    assert m.total_return == pytest.approx(0.2450)
    assert m.annualized_return == pytest.approx(0.2475)
    assert m.annualized_volatility == pytest.approx(0.1420)
    assert m.sharpe_ratio == pytest.approx(1.74)
    assert m.sortino_ratio == pytest.approx(2.45)
    assert m.max_drawdown == pytest.approx(0.0680)
    assert m.win_rate == pytest.approx(64.5)
    assert m.profit_factor == pytest.approx(1.85)
    assert m.total_trades == 28
    assert m.avg_holding_period_days == pytest.approx(4.8)
    assert m.benchmark_ticker == "SPY"
    assert m.benchmark_total_return == pytest.approx(0.1250)
    assert m.benchmark_annualized_return == pytest.approx(0.1265)
    assert m.benchmark_annualized_volatility == pytest.approx(0.1580)
    assert m.alpha == pytest.approx(0.1200)
    assert m.beta == pytest.approx(0.82)
    assert m.information_ratio == pytest.approx(1.15)
    assert m.tracking_error == pytest.approx(0.0980)

    # ── Verify equity curve ─────────────────────────────────────────────────
    assert len(report.equity_curve) == 3
    pt = report.equity_curve[0]
    assert isinstance(pt, EquityCurvePoint)
    assert pt.date == "2024-01-02"
    assert pt.portfolio_value == pytest.approx(1002500.0)
    assert pt.benchmark_value == pytest.approx(1001200.0)
    assert pt.strategy_daily_return == pytest.approx(0.0025)
    assert pt.benchmark_daily_return == pytest.approx(0.0012)
    assert pt.position == 1

    # Verify drawdown day
    pt3 = report.equity_curve[2]
    assert pt3.strategy_daily_return == pytest.approx(-0.0020)
    assert pt3.position == 0  # flat

    # ── Verify ergonomic alias ──────────────────────────────────────────────
    report_alias = client.alpha_report(
        tickers=["AAPL"],
        start_date="2024-01-01",
        end_date="2024-12-31",
    )
    assert isinstance(report_alias, AlphaReportResponse)


def test_alpha_report_default_signal_config():
    """Verify that omitting signal_config uses sensible defaults."""
    captured_payloads = []

    def mock_handler(request: httpx.Request):
        import json
        captured_payloads.append(json.loads(request.content))
        return httpx.Response(200, json=SAMPLE_ALPHA_REPORT)

    transport = httpx.MockTransport(mock_handler)
    client = FinTextClient(base_url="http://testserver", api_token="test_jwt_token")
    client._client = httpx.Client(transport=transport, base_url="http://testserver")

    client.generate_alpha_report(
        tickers=["TSLA"],
        start_date="2024-06-01",
        end_date="2024-12-31",
    )

    payload = captured_payloads[0]
    sc = payload["signal_config"]
    assert sc["signal_type"] == "sentiment"
    assert sc["threshold_long"] == 0.2
    assert sc["threshold_short"] == -0.2
    assert sc["holding_days"] == 5
    # smoothing_window_days should be excluded (None → exclude_none)
    assert "smoothing_window_days" not in sc


def test_alpha_report_validation_empty_tickers():
    """Verify that empty tickers list raises FinTextValidationError."""
    client = FinTextClient(base_url="http://testserver", api_token="test_jwt_token")
    with pytest.raises(FinTextValidationError, match="at least one ticker"):
        client.generate_alpha_report(
            tickers=[],
            start_date="2024-01-01",
            end_date="2024-12-31",
        )


def test_alpha_report_validation_too_many_tickers():
    """Verify that >10 tickers raises FinTextValidationError."""
    client = FinTextClient(base_url="http://testserver", api_token="test_jwt_token")
    tickers = [f"T{i}" for i in range(11)]
    with pytest.raises(FinTextValidationError, match="at most 10"):
        client.generate_alpha_report(
            tickers=tickers,
            start_date="2024-01-01",
            end_date="2024-12-31",
        )


def test_alpha_report_model_alias():
    """Verify AlphaReport is an alias for AlphaReportResponse."""
    assert AlphaReport is AlphaReportResponse


def test_alpha_report_request_model():
    """Verify AlphaReportRequest Pydantic model round-trips correctly."""
    req = AlphaReportRequest(
        tickers=["AAPL", "MSFT"],
        start_date="2024-01-01",
        end_date="2024-12-31",
        signal_config=AlphaSignalConfig(
            signal_type="esg",
            threshold_long=0.3,
            threshold_short=-0.3,
            holding_days=10,
            smoothing_window_days=5,
        ),
        benchmark_ticker="QQQ",
        initial_capital=500_000.0,
    )
    data = req.model_dump()
    assert data["tickers"] == ["AAPL", "MSFT"]
    assert data["signal_config"]["signal_type"] == "esg"
    assert data["signal_config"]["holding_days"] == 10
    assert data["benchmark_ticker"] == "QQQ"
    assert data["initial_capital"] == 500_000.0

    # Round-trip
    req2 = AlphaReportRequest.model_validate(data)
    assert req2.tickers == req.tickers
    assert req2.signal_config.signal_type == req.signal_config.signal_type


@pytest.mark.asyncio
async def test_alpha_report_async_client():
    """Verify asynchronous async_client.generate_alpha_report() returns structured AlphaReportResponse."""
    def mock_handler(request: httpx.Request):
        assert request.url.path == "/signals/alpha-report"
        assert request.method == "POST"
        assert "Bearer test_jwt_token" in request.headers.get("Authorization", "")
        return httpx.Response(200, json=SAMPLE_ALPHA_REPORT)

    transport = httpx.MockTransport(mock_handler)
    client = FinTextAsyncClient(base_url="http://testserver", api_token="test_jwt_token")
    client._client = httpx.AsyncClient(transport=transport, base_url="http://testserver")

    report = await client.generate_alpha_report(
        tickers=["AAPL", "MSFT", "NVDA"],
        start_date="2024-01-01",
        end_date="2024-12-31",
        signal_config=AlphaSignalConfig(
            signal_type="sentiment",
            threshold_long=0.2,
            threshold_short=-0.2,
            holding_days=5,
        ),
    )

    assert isinstance(report, AlphaReportResponse)
    assert report.tickers == ["AAPL", "MSFT", "NVDA"]
    assert report.metrics.sharpe_ratio == pytest.approx(1.74)
    assert report.metrics.alpha == pytest.approx(0.1200)
    assert report.metrics.beta == pytest.approx(0.82)
    assert len(report.equity_curve) == 3

    # Test alias
    report_alias = await client.alpha_report(
        tickers=["AAPL"],
        start_date="2024-01-01",
        end_date="2024-12-31",
    )
    assert isinstance(report_alias, AlphaReportResponse)


@pytest.mark.asyncio
async def test_alpha_report_async_validation():
    """Verify async client validation matches sync client."""
    client = FinTextAsyncClient(base_url="http://testserver", api_token="test_jwt_token")
    with pytest.raises(FinTextValidationError, match="at least one ticker"):
        await client.generate_alpha_report(
            tickers=[],
            start_date="2024-01-01",
            end_date="2024-12-31",
        )
