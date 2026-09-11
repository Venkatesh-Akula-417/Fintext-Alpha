"""
═══════════════════════════════════════════════════════════════════════════════
FinText Alpha Vectorizer Python Client SDK — Signal Quality Report Tests
═══════════════════════════════════════════════════════════════════════════════
"""

import httpx
import pytest
from fintext import FinTextClient, FinTextAsyncClient
from fintext.exceptions import FinTextValidationError
from fintext.models import (
    SignalQualityReportRequest,
    SignalQualityReportResponse,
    SignalQualityReport,
    ICSummary,
    DecayCurvePoint,
    MarketCapBias,
)

SAMPLE_QUALITY_REPORT = {
    "signal_type": "sentiment",
    "tickers": ["AAPL", "MSFT", "NVDA"],
    "start_date": "2025-01-01",
    "end_date": "2025-06-30",
    "horizon_days": 5,
    "coverage_pct": 88.5,
    "freshness_avg_ms": 410.2,
    "freshness_p95_ms": 980.0,
    "ic_summary": {
        "spearman_ic": 0.0420,
        "rank_ic": 0.0420,
        "observations": 4500,
        "icir": 0.6500,
    },
    "decay_curve": [
        {"horizon_days": 1, "ic": 0.0550},
        {"horizon_days": 2, "ic": 0.0480},
        {"horizon_days": 3, "ic": 0.0450},
        {"horizon_days": 5, "ic": 0.0420},
        {"horizon_days": 10, "ic": 0.0310},
        {"horizon_days": 20, "ic": 0.0220},
    ],
    "half_life_days": 14.0,
    "hit_rate_pct": 52.3,
    "false_positive_rate_pct": 18.2,
    "sector_bias": {
        "Technology": 0.0150,
        "Financials": -0.0080,
        "Healthcare": 0.0030,
    },
    "market_cap_bias": {
        "top_quantile_avg": 0.0120,
        "bottom_quantile_avg": 0.0250,
        "spread": 0.0130,
    },
    "generated_at": "2025-07-01T12:00:00Z",
    "message": "Signal quality report generated for sentiment stream across 3 tickers",
}


def test_signal_quality_sync_client():
    """Verify synchronous client.signal_quality_report() returns structured SignalQualityReportResponse."""
    captured_payload = {}

    def mock_handler(request: httpx.Request):
        assert request.url.path == "/signals/quality-report"
        assert request.method == "POST"
        assert "Bearer test_jwt_token" in request.headers.get("Authorization", "")
        import json
        captured_payload.update(json.loads(request.content))
        return httpx.Response(200, json=SAMPLE_QUALITY_REPORT)

    transport = httpx.MockTransport(mock_handler)
    client = FinTextClient(base_url="http://testserver", api_token="test_jwt_token")
    client._client = httpx.Client(transport=transport, base_url="http://testserver")

    report = client.signal_quality_report(
        signal_type="sentiment",
        tickers=["AAPL", "MSFT", "NVDA"],
        start_date="2025-01-01",
        end_date="2025-06-30",
        horizon_days=5,
        benchmark_ticker="SPY",
    )

    assert isinstance(report, SignalQualityReportResponse)
    assert report.signal_type == "sentiment"
    assert report.tickers == ["AAPL", "MSFT", "NVDA"]
    assert report.coverage_pct == 88.5
    assert report.freshness_avg_ms == 410.2
    assert report.ic_summary.spearman_ic == 0.042
    assert report.ic_summary.icir == 0.65
    assert len(report.decay_curve) == 6
    assert report.half_life_days == 14.0
    assert report.hit_rate_pct == 52.3
    assert report.market_cap_bias.spread == 0.013

    assert captured_payload.get("signal_type") == "sentiment"
    assert captured_payload.get("tickers") == ["AAPL", "MSFT", "NVDA"]
    assert captured_payload.get("horizon_days") == 5


def test_signal_quality_sync_alias():
    """Verify ergonomic alias client.quality_report() behaves identically."""
    def mock_handler(request: httpx.Request):
        assert request.url.path == "/signals/quality-report"
        return httpx.Response(200, json=SAMPLE_QUALITY_REPORT)

    transport = httpx.MockTransport(mock_handler)
    client = FinTextClient(base_url="http://testserver", api_token="test_jwt_token")
    client._client = httpx.Client(transport=transport, base_url="http://testserver")

    report = client.quality_report(
        signal_type="sentiment",
        tickers=["AAPL"],
        start_date="2025-01-01",
        end_date="2025-06-30",
    )

    assert isinstance(report, SignalQualityReport)
    assert report.signal_type == "sentiment"


@pytest.mark.asyncio
async def test_signal_quality_async_client():
    """Verify asynchronous client.signal_quality_report() returns structured response."""
    def mock_handler(request: httpx.Request):
        assert request.url.path == "/signals/quality-report"
        assert request.method == "POST"
        assert "Bearer async_test_token" in request.headers.get("Authorization", "")
        return httpx.Response(200, json=SAMPLE_QUALITY_REPORT)

    transport = httpx.MockTransport(mock_handler)
    async_client = FinTextAsyncClient(base_url="http://testserver", api_token="async_test_token")
    async_client._client = httpx.AsyncClient(transport=transport, base_url="http://testserver")

    report = await async_client.signal_quality_report(
        signal_type="sentiment",
        tickers=["AAPL", "MSFT"],
        start_date="2025-01-01",
        end_date="2025-06-30",
    )

    assert isinstance(report, SignalQualityReportResponse)
    assert report.signal_type == "sentiment"
    assert report.ic_summary.observations == 4500


@pytest.mark.asyncio
async def test_signal_quality_async_alias():
    """Verify asynchronous alias async_client.quality_report() works."""
    def mock_handler(request: httpx.Request):
        assert request.url.path == "/signals/quality-report"
        return httpx.Response(200, json=SAMPLE_QUALITY_REPORT)

    transport = httpx.MockTransport(mock_handler)
    async_client = FinTextAsyncClient(base_url="http://testserver", api_token="async_test_token")
    async_client._client = httpx.AsyncClient(transport=transport, base_url="http://testserver")

    report = await async_client.quality_report(
        signal_type="gex",
        tickers=["NVDA"],
        start_date="2025-01-01",
        end_date="2025-06-30",
    )

    assert isinstance(report, SignalQualityReport)
    assert report.signal_type == "sentiment"


def test_signal_quality_validation_empty_signal_type():
    """Verify client rejects empty signal_type."""
    client = FinTextClient(base_url="http://testserver", api_token="test_jwt_token")
    with pytest.raises(FinTextValidationError) as exc_info:
        client.signal_quality_report(
            signal_type="",
            tickers=["AAPL"],
            start_date="2025-01-01",
            end_date="2025-06-30",
        )
    assert "signal_type" in str(exc_info.value).lower()


def test_signal_quality_validation_empty_tickers():
    """Verify client rejects empty tickers list."""
    client = FinTextClient(base_url="http://testserver", api_token="test_jwt_token")
    with pytest.raises(FinTextValidationError) as exc_info:
        client.signal_quality_report(
            signal_type="sentiment",
            tickers=[],
            start_date="2025-01-01",
            end_date="2025-06-30",
        )
    assert "tickers" in str(exc_info.value).lower()


def test_signal_quality_validation_too_many_tickers():
    """Verify client rejects universe size > 20 tickers."""
    client = FinTextClient(base_url="http://testserver", api_token="test_jwt_token")
    tickers = [f"T{i}" for i in range(25)]
    with pytest.raises(FinTextValidationError) as exc_info:
        client.signal_quality_report(
            signal_type="sentiment",
            tickers=tickers,
            start_date="2025-01-01",
            end_date="2025-06-30",
        )
    assert "tickers" in str(exc_info.value).lower()


def test_signal_quality_validation_invalid_horizon():
    """Verify client rejects horizon outside [1, 20]."""
    client = FinTextClient(base_url="http://testserver", api_token="test_jwt_token")
    with pytest.raises(FinTextValidationError):
        client.signal_quality_report(
            signal_type="sentiment",
            tickers=["AAPL"],
            start_date="2025-01-01",
            end_date="2025-06-30",
            horizon_days=0,
        )

    with pytest.raises(FinTextValidationError):
        client.signal_quality_report(
            signal_type="sentiment",
            tickers=["AAPL"],
            start_date="2025-01-01",
            end_date="2025-06-30",
            horizon_days=25,
        )


def test_ic_summary_backward_compatibility_without_icir():
    """Verify ICSummary parses correctly when icir is omitted from response payload."""
    payload = {
        "spearman_ic": 0.042,
        "rank_ic": 0.042,
        "observations": 4500,
    }
    summary = ICSummary.model_validate(payload)
    assert summary.spearman_ic == 0.042
    assert summary.rank_ic == 0.042
    assert summary.observations == 4500
    assert summary.icir is None


def test_ic_summary_with_icir_parsing():
    """Verify ICSummary parses correctly when icir is present in response payload."""
    payload = {
        "spearman_ic": 0.042,
        "rank_ic": 0.042,
        "observations": 4500,
        "icir": 0.7250,
    }
    summary = ICSummary.model_validate(payload)
    assert summary.spearman_ic == 0.042
    assert summary.rank_ic == 0.042
    assert summary.observations == 4500
    assert summary.icir == 0.7250

