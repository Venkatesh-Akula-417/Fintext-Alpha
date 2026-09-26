"""
═══════════════════════════════════════════════════════════════════════════════
FinText Alpha Vectorizer Python Client SDK — Tenant API Key Lifecycle Tests
═══════════════════════════════════════════════════════════════════════════════
"""

import httpx
import pytest
from fintext import (
    FinTextClient,
    FinTextAsyncClient,
    FinTextAuthError,
    TenantCreateKeyResponse,
    TenantRevokeKeyResponse,
    TenantRotateKeyResponse,
    TenantKeyItem,
)

SAMPLE_CREATE_RESPONSE = {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "name": "Alpha-Algo-Prod",
    "prefix": "ft_live_AbC123Xy",
    "created_at": "2026-09-26T12:00:00Z",
    "expires_at": "2026-10-26T12:00:00Z",
    "plaintext_once": "ft_live_AbC123Xy9876543210ZYXWVUTSRQPONMLKJIHGFEDCBA",
}

SAMPLE_LIST_RESPONSE = {
    "keys": [
        {
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "name": "Alpha-Algo-Prod",
            "prefix": "ft_live_AbC123Xy",
            "created_at": "2026-09-26T12:00:00Z",
            "expires_at": "2026-10-26T12:00:00Z",
            "revoked_at": None,
            "status": "active",
            "last_seen_utc": "2026-09-26T12:05:00Z",
            "rotated_from": None,
        }
    ]
}

SAMPLE_ROTATE_RESPONSE = {
    "id": "660e8400-e29b-41d4-a716-446655440111",
    "name": "Alpha-Algo-Prod",
    "prefix": "ft_live_Rot987Zz",
    "created_at": "2026-09-26T12:10:00Z",
    "expires_at": "2026-10-26T12:00:00Z",
    "plaintext_once": "ft_live_Rot987Zz1234567890abcdefghijklmnopqrstuvwxyz",
    "rotated_from": "550e8400-e29b-41d4-a716-446655440000",
}

SAMPLE_REVOKE_RESPONSE = {
    "id": "660e8400-e29b-41d4-a716-446655440111",
    "status": "revoked",
    "revoked_at": "2026-09-26T12:15:00Z",
}

RATE_LIMIT_HEADERS = {
    "X-RateLimit-Limit": "100",
    "X-RateLimit-Remaining": "99",
    "X-RateLimit-Reset": "1790424000",
}


def test_client_create_key():
    def mock_handler(request: httpx.Request) -> httpx.Response:
        assert request.url.path in ("/v1/account/keys", "/account/keys")
        assert request.method == "POST"
        assert "Authorization" in request.headers
        return httpx.Response(201, json=SAMPLE_CREATE_RESPONSE, headers=RATE_LIMIT_HEADERS)

    transport = httpx.MockTransport(mock_handler)
    client = FinTextClient(
        base_url="http://testserver",
        api_token="valid_token",
        transport=transport,
    )

    resp = client.create_key(name="Alpha-Algo-Prod", expires_in_days=30)
    assert isinstance(resp, TenantCreateKeyResponse)
    assert resp.name == "Alpha-Algo-Prod"
    assert resp.prefix == "ft_live_AbC123Xy"
    assert resp.plaintext_once.startswith("ft_live_")
    # Crucial security assertion: key_hash is never present
    assert not hasattr(resp, "key_hash")


def test_client_list_keys():
    def mock_handler(request: httpx.Request) -> httpx.Response:
        assert request.url.path in ("/v1/account/keys", "/account/keys")
        assert request.method == "GET"
        return httpx.Response(200, json=SAMPLE_LIST_RESPONSE, headers=RATE_LIMIT_HEADERS)

    transport = httpx.MockTransport(mock_handler)
    client = FinTextClient(
        base_url="http://testserver",
        api_token="valid_token",
        transport=transport,
    )

    keys = client.list_keys()
    assert len(keys) == 1
    assert isinstance(keys[0], TenantKeyItem)
    assert keys[0].status == "active"
    assert keys[0].prefix == "ft_live_AbC123Xy"
    assert not hasattr(keys[0], "key_hash")
    assert not hasattr(keys[0], "plaintext_once")


def test_client_rotate_key():
    def mock_handler(request: httpx.Request) -> httpx.Response:
        assert request.url.path in (
            "/v1/account/keys/550e8400-e29b-41d4-a716-446655440000/rotate",
            "/account/keys/550e8400-e29b-41d4-a716-446655440000/rotate",
        )
        assert request.method == "POST"
        return httpx.Response(200, json=SAMPLE_ROTATE_RESPONSE, headers=RATE_LIMIT_HEADERS)

    transport = httpx.MockTransport(mock_handler)
    client = FinTextClient(
        base_url="http://testserver",
        api_token="valid_token",
        transport=transport,
    )

    resp = client.rotate_key("550e8400-e29b-41d4-a716-446655440000")
    assert isinstance(resp, TenantRotateKeyResponse)
    assert resp.rotated_from == "550e8400-e29b-41d4-a716-446655440000"
    assert resp.plaintext_once.startswith("ft_live_")


def test_client_revoke_key():
    def mock_handler(request: httpx.Request) -> httpx.Response:
        assert request.url.path in (
            "/v1/account/keys/660e8400-e29b-41d4-a716-446655440111",
            "/account/keys/660e8400-e29b-41d4-a716-446655440111",
        )
        assert request.method == "DELETE"
        return httpx.Response(200, json=SAMPLE_REVOKE_RESPONSE, headers=RATE_LIMIT_HEADERS)

    transport = httpx.MockTransport(mock_handler)
    client = FinTextClient(
        base_url="http://testserver",
        api_token="valid_token",
        transport=transport,
    )

    resp = client.revoke_key("660e8400-e29b-41d4-a716-446655440111")
    assert isinstance(resp, TenantRevokeKeyResponse)
    assert resp.status == "revoked"


@pytest.mark.asyncio
async def test_async_client_lifecycle():
    def mock_handler(request: httpx.Request) -> httpx.Response:
        if request.method == "POST" and "rotate" in request.url.path:
            return httpx.Response(200, json=SAMPLE_ROTATE_RESPONSE, headers=RATE_LIMIT_HEADERS)
        elif request.method == "POST":
            return httpx.Response(201, json=SAMPLE_CREATE_RESPONSE, headers=RATE_LIMIT_HEADERS)
        elif request.method == "GET":
            return httpx.Response(200, json=SAMPLE_LIST_RESPONSE, headers=RATE_LIMIT_HEADERS)
        elif request.method == "DELETE":
            return httpx.Response(200, json=SAMPLE_REVOKE_RESPONSE, headers=RATE_LIMIT_HEADERS)
        return httpx.Response(404)

    transport = httpx.MockTransport(mock_handler)
    async_client = FinTextAsyncClient(
        base_url="http://testserver",
        api_token="valid_token",
        transport=transport,
    )

    created = await async_client.create_key(name="Async-Bot")
    assert created.id == "550e8400-e29b-41d4-a716-446655440000"

    keys = await async_client.list_keys()
    assert len(keys) == 1

    rotated = await async_client.rotate_key("550e8400-e29b-41d4-a716-446655440000")
    assert rotated.rotated_from == "550e8400-e29b-41d4-a716-446655440000"

    revoked = await async_client.revoke_key("660e8400-e29b-41d4-a716-446655440111")
    assert revoked.status == "revoked"
