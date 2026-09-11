"""
═══════════════════════════════════════════════════════════════════════════════
FinText Alpha Vectorizer Python Client SDK — Provider Health Status Tests
═══════════════════════════════════════════════════════════════════════════════
"""

import httpx
import pytest
from fintext import FinTextClient, FinTextAsyncClient, FinTextAPIError
from fintext.models import (
    ProviderHealthItem,
    ProviderHealthResponse,
    ProviderHealth,
)

SAMPLE_PROVIDER_HEALTH = {
    "providers": [
        {
            "provider": "sec_edgar",
            "status": "healthy",
            "requests_total": 120,
            "requests_success": 118,
            "success_rate_pct": 98.33,
            "avg_latency_ms": 250.5,
            "p95_latency_ms": 480.0,
            "error_count_last_hour": 2,
            "last_success_timestamp": "2026-09-05T10:30:00Z",
            "last_error_message": "Timeout after 5000ms",
        },
        {
            "provider": "finnhub",
            "status": "healthy",
            "requests_total": 450,
            "requests_success": 448,
            "success_rate_pct": 99.56,
            "avg_latency_ms": 45.2,
            "p95_latency_ms": 110.0,
            "error_count_last_hour": 2,
            "last_success_timestamp": "2026-09-05T10:31:00Z",
            "last_error_message": None,
        },
        {
            "provider": "polygon",
            "status": "degraded",
            "requests_total": 800,
            "requests_success": 720,
            "success_rate_pct": 90.0,
            "avg_latency_ms": 85.0,
            "p95_latency_ms": 220.0,
            "error_count_last_hour": 80,
            "last_success_timestamp": "2026-09-05T10:29:00Z",
            "last_error_message": "Rate limit 429 exceeded",
        },
    ],
    "generated_at": "2026-09-05T11:00:00Z",
}


def test_provider_health_models():
    """Verify ProviderHealthResponse and ProviderHealthItem Pydantic parsing."""
    resp = ProviderHealthResponse.model_validate(SAMPLE_PROVIDER_HEALTH)
    assert len(resp.providers) == 3
    assert resp.generated_at == "2026-09-05T11:00:00Z"

    sec = resp.providers[0]
    assert sec.provider == "sec_edgar"
    assert sec.status == "healthy"
    assert sec.requests_total == 120
    assert sec.requests_success == 118
    assert sec.success_rate_pct == 98.33
    assert sec.avg_latency_ms == 250.5
    assert sec.p95_latency_ms == 480.0
    assert sec.error_count_last_hour == 2
    assert sec.last_success_timestamp == "2026-09-05T10:30:00Z"
    assert sec.last_error_message == "Timeout after 5000ms"

    # Verify alias
    assert ProviderHealth is ProviderHealthResponse


def test_provider_health_sync_client():
    """Verify synchronous client get_provider_health() and alias provider_health()."""
    def mock_handler(request: httpx.Request):
        assert request.url.path == "/providers/health"
        assert request.method == "GET"
        assert "Bearer test_jwt_token" in request.headers.get("Authorization", "")

        provider = request.url.params.get("provider", "all")
        window = int(request.url.params.get("window_minutes", "60"))

        if provider not in ("sec_edgar", "finnhub", "polygon", "all"):
            return httpx.Response(400, json={"error": "Bad Request", "message": "Invalid provider"})
        if not (1 <= window <= 1440):
            return httpx.Response(400, json={"error": "Bad Request", "message": "Invalid window_minutes"})

        if provider == "all":
            filtered = SAMPLE_PROVIDER_HEALTH["providers"]
        else:
            filtered = [p for p in SAMPLE_PROVIDER_HEALTH["providers"] if p["provider"] == provider]

        return httpx.Response(200, json={"providers": filtered, "generated_at": SAMPLE_PROVIDER_HEALTH["generated_at"]})

    transport = httpx.MockTransport(mock_handler)
    client = FinTextClient(base_url="http://testserver", api_token="test_jwt_token")
    client._client = httpx.Client(transport=transport, base_url="http://testserver")

    # All providers
    res = client.get_provider_health()
    assert isinstance(res, ProviderHealthResponse)
    assert len(res.providers) == 3
    assert res.providers[0].provider == "sec_edgar"
    assert res.providers[1].provider == "finnhub"
    assert res.providers[2].provider == "polygon"

    # Filtered by provider using alias
    res_single = client.provider_health(provider="sec_edgar", window_minutes=120)
    assert len(res_single.providers) == 1
    assert res_single.providers[0].provider == "sec_edgar"

    # Invalid provider
    with pytest.raises(FinTextAPIError) as exc_info:
        client.get_provider_health(provider="invalid")
    assert exc_info.value.status_code == 400

    # Invalid window_minutes
    with pytest.raises(FinTextAPIError) as exc_info:
        client.get_provider_health(window_minutes=2000)
    assert exc_info.value.status_code == 400


@pytest.mark.asyncio
async def test_provider_health_async_client():
    """Verify asynchronous client get_provider_health() and alias provider_health()."""
    def mock_handler(request: httpx.Request):
        assert request.url.path == "/providers/health"
        assert request.method == "GET"
        assert "Bearer test_async_token" in request.headers.get("Authorization", "")

        provider = request.url.params.get("provider", "all")
        if provider == "finnhub":
            filtered = [p for p in SAMPLE_PROVIDER_HEALTH["providers"] if p["provider"] == "finnhub"]
        else:
            filtered = SAMPLE_PROVIDER_HEALTH["providers"]

        return httpx.Response(200, json={"providers": filtered, "generated_at": SAMPLE_PROVIDER_HEALTH["generated_at"]})

    transport = httpx.MockTransport(mock_handler)
    client = FinTextAsyncClient(base_url="http://testserver", api_token="test_async_token")
    client._client = httpx.AsyncClient(transport=transport, base_url="http://testserver")

    res = await client.get_provider_health()
    assert isinstance(res, ProviderHealthResponse)
    assert len(res.providers) == 3

    res_alias = await client.provider_health(provider="finnhub")
    assert len(res_alias.providers) == 1
    assert res_alias.providers[0].provider == "finnhub"
    assert res_alias.providers[0].status == "healthy"
