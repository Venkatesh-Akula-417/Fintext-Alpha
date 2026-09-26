"""
═══════════════════════════════════════════════════════════════════════════════
FinText Alpha Vectorizer Python Client SDK — Tenant Usage & Audit Tests
═══════════════════════════════════════════════════════════════════════════════
"""

import httpx
import pytest
from fintext import FinTextClient, FinTextAsyncClient, FinTextAuthError

SAMPLE_USAGE_RESPONSE = {
    "org_id": "fund_sigma_capital",
    "period_utc": "2026-09",
    "plan": "enterprise_monthly",
    "plan_limit": 2000000,
    "requests_total": 412500,
    "headroom_pct": 79.38,
    "daily": [
        {"date": "2026-09-01", "requests": 14200},
        {"date": "2026-09-02", "requests": 15800},
    ],
    "by_endpoint_group": [
        {"group": "sentiment", "requests": 250000},
        {"group": "alpha", "requests": 100000},
        {"group": "pit", "requests": 50000},
        {"group": "analytics", "requests": 10000},
        {"group": "other", "requests": 2500},
    ],
    "keys": [
        {
            "prefix": "fta_live_sigm",
            "name": "Production Trade Execution",
            "created_utc": "2026-08-01T00:00:00Z",
            "last_seen_utc": "2026-09-25T14:30:00Z",
            "active": True,
        }
    ],
    "recent_audit": [
        {
            "ts": "2026-09-25T14:20:00Z",
            "event_type": "api_key.created",
            "actor": "user_quant_admin",
        }
    ],
    "ip_whitelist": ["198.51.100.0/24"],
    "generated_utc": "2026-09-25T15:00:00Z",
}


def test_client_usage_returns_expected_payload():
    def mock_handler(request: httpx.Request) -> httpx.Response:
        assert request.url.path in ("/v1/account/usage", "/account/usage")
        assert "Authorization" in request.headers
        assert request.headers["Authorization"].startswith("Bearer ")
        return httpx.Response(200, json=SAMPLE_USAGE_RESPONSE)

    transport = httpx.MockTransport(mock_handler)
    client = FinTextClient(
        base_url="http://testserver",
        api_token="valid_test_token_123",
        transport=transport,
    )

    usage_data = client.usage()
    assert usage_data["org_id"] == "fund_sigma_capital"
    assert usage_data["plan"] == "enterprise_monthly"
    assert usage_data["plan_limit"] == 2000000
    assert usage_data["requests_total"] == 412500
    assert usage_data["headroom_pct"] == 79.38
    assert len(usage_data["daily"]) == 2
    assert len(usage_data["by_endpoint_group"]) == 5
    assert len(usage_data["keys"]) == 1
    assert usage_data["keys"][0]["prefix"] == "fta_live_sigm"
    # S-1 check: payload does not contain secret or key_hash
    assert "key_hash" not in str(usage_data)
    assert "secret" not in str(usage_data)


def test_client_usage_unauthenticated_raises_error():
    client = FinTextClient(
        base_url="http://testserver",
        api_token=None,
        admin_token=None,
    )
    with pytest.raises(FinTextAuthError):
        client.usage()


@pytest.mark.asyncio
async def test_async_client_usage_returns_expected_payload():
    def mock_handler(request: httpx.Request) -> httpx.Response:
        assert request.url.path in ("/v1/account/usage", "/account/usage")
        assert "Authorization" in request.headers
        return httpx.Response(200, json=SAMPLE_USAGE_RESPONSE)

    transport = httpx.MockTransport(mock_handler)
    async_client = FinTextAsyncClient(
        base_url="http://testserver",
        api_token="valid_test_token_456",
        transport=transport,
    )

    usage_data = await async_client.usage()
    assert usage_data["org_id"] == "fund_sigma_capital"
    assert usage_data["headroom_pct"] == 79.38
    await async_client.close()
