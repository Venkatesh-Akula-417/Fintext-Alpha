"""
═══════════════════════════════════════════════════════════════════════════════
FinText Alpha Vectorizer Python Client SDK — SLA Latency Telemetry Tests
═══════════════════════════════════════════════════════════════════════════════
"""

import httpx
import pytest
from fintext import FinTextClient, FinTextAsyncClient
from fintext.models import SLALatencyResponse, StageBreakdown


def test_sla_latency_sync_client():
    """Verify synchronous client.sla_latency() returns structured SLALatencyResponse."""
    mock_payload = {
        "ticker": "AAPL",
        "start_date": "2025-08-01",
        "end_date": "2025-08-31",
        "total_signals": 7500,
        "average_latency_ms": 42.5,
        "percentiles": {
            "p50": 34.2,
            "p95": 86.8,
            "p99": 142.1,
        },
        "max_latency_ms": 210.5,
        "sla_target_ms": 500,
        "sla_compliant_signals": 7490,
        "sla_compliance_rate": 99.87,
        "sla_status": "met",
        "stage_breakdown": {
            "fetch_latency_ms": 18.2,
            "normalization_latency_ms": 3.4,
            "inference_latency_ms": 15.1,
            "write_latency_ms": 5.8,
        },
        "generated_at": "2026-09-01T12:00:00Z",
    }

    def mock_handler(request: httpx.Request):
        assert request.url.path == "/sla/latency"
        assert request.url.params["ticker"] == "AAPL"
        assert request.url.params["start_date"] == "2025-08-01"
        assert request.url.params["end_date"] == "2025-08-31"
        assert request.url.params["percentiles"] == "50,95,99"
        assert request.url.params["sla_target_ms"] == "500"
        return httpx.Response(200, json=mock_payload)

    transport = httpx.MockTransport(mock_handler)
    client = FinTextClient(base_url="http://testserver", api_token="test_jwt_token")
    client._client = httpx.Client(transport=transport, base_url="http://testserver")

    resp = client.sla_latency(
        ticker="AAPL",
        start_date="2025-08-01",
        end_date="2025-08-31",
        percentiles="50,95,99",
        sla_target_ms=500,
    )

    assert isinstance(resp, SLALatencyResponse)
    assert resp.ticker == "AAPL"
    assert resp.total_signals == 7500
    assert resp.average_latency_ms == pytest.approx(42.5)
    assert resp.percentiles["p50"] == pytest.approx(34.2)
    assert resp.percentiles["p95"] == pytest.approx(86.8)
    assert resp.percentiles["p99"] == pytest.approx(142.1)
    assert resp.sla_target_ms == 500
    assert resp.sla_compliance_rate == pytest.approx(99.87)
    assert resp.sla_status == "met"
    assert isinstance(resp.stage_breakdown, StageBreakdown)
    assert resp.stage_breakdown.fetch_latency_ms == pytest.approx(18.2)
    assert resp.stage_breakdown.normalization_latency_ms == pytest.approx(3.4)
    assert resp.stage_breakdown.inference_latency_ms == pytest.approx(15.1)
    assert resp.stage_breakdown.write_latency_ms == pytest.approx(5.8)


@pytest.mark.asyncio
async def test_sla_latency_async_client():
    """Verify asynchronous async_client.sla_latency() returns structured SLALatencyResponse."""
    mock_payload = {
        "ticker": None,
        "start_date": "2025-08-01",
        "end_date": "2025-08-31",
        "total_signals": 75000,
        "average_latency_ms": 38.4,
        "percentiles": {
            "p50": 31.0,
            "p95": 78.5,
            "p99": 125.0,
        },
        "max_latency_ms": 195.0,
        "sla_target_ms": 300,
        "sla_compliant_signals": 74850,
        "sla_compliance_rate": 99.80,
        "sla_status": "met",
        "stage_breakdown": {
            "fetch_latency_ms": 17.5,
            "normalization_latency_ms": 3.0,
            "inference_latency_ms": 13.2,
            "write_latency_ms": 4.7,
        },
        "generated_at": "2026-09-01T12:00:00Z",
    }

    def mock_handler(request: httpx.Request):
        assert request.url.path == "/sla/latency"
        assert "ticker" not in request.url.params
        assert request.url.params["sla_target_ms"] == "300"
        return httpx.Response(200, json=mock_payload)

    transport = httpx.MockTransport(mock_handler)
    client = FinTextAsyncClient(base_url="http://testserver", api_token="test_jwt_token")
    client._client = httpx.AsyncClient(transport=transport, base_url="http://testserver")

    resp = await client.sla_latency(sla_target_ms=300)

    assert isinstance(resp, SLALatencyResponse)
    assert resp.ticker is None
    assert resp.total_signals == 75000
    assert resp.average_latency_ms == pytest.approx(38.4)
    assert resp.percentiles["p50"] == pytest.approx(31.0)
    assert resp.percentiles["p95"] == pytest.approx(78.5)
    assert resp.percentiles["p99"] == pytest.approx(125.0)
    assert resp.sla_target_ms == 300
    assert resp.sla_status == "met"
    assert resp.stage_breakdown.fetch_latency_ms == pytest.approx(17.5)
    assert resp.stage_breakdown.write_latency_ms == pytest.approx(4.7)
