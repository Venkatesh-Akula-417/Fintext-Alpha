"""
═══════════════════════════════════════════════════════════════════════════════
FinText Alpha Vectorizer Python Client SDK — MessagePack WebSocket Streaming Tests
═══════════════════════════════════════════════════════════════════════════════
"""

import json
import pytest
import msgpack
from fintext import FinTextClient, FinTextAsyncClient, decode_websocket_frame


def test_decode_websocket_frame_json_text():
    """Verify decoding standard JSON text frames."""
    sample_payload = {
        "type": "connected",
        "server": "fintext-api",
        "version": "2.0.0-institutional",
        "encoding": "json",
        "active_connections": 1,
    }
    json_text = json.dumps(sample_payload)
    decoded = decode_websocket_frame(json_text)
    assert decoded["type"] == "connected"
    assert decoded["server"] == "fintext-api"
    assert decoded["encoding"] == "json"
    assert decoded["active_connections"] == 1


def test_decode_websocket_frame_msgpack_binary():
    """Verify decoding binary MessagePack frames with named fields."""
    sample_payload = {
        "type": "anomaly_alert",
        "ticker": "AAPL",
        "sentiment_score": 0.88,
        "z_score": 3.75,
        "severity": "CRITICAL",
        "is_anomaly": True,
        "timestamp_utc": "2026-09-01T12:00:00Z",
    }
    msgpack_bytes = msgpack.packb(sample_payload)
    assert isinstance(msgpack_bytes, bytes)

    decoded = decode_websocket_frame(msgpack_bytes)
    assert decoded["type"] == "anomaly_alert"
    assert decoded["ticker"] == "AAPL"
    assert decoded["sentiment_score"] == pytest.approx(0.88)
    assert decoded["z_score"] == pytest.approx(3.75)
    assert decoded["severity"] == "CRITICAL"
    assert decoded["is_anomaly"] is True


def test_decode_websocket_frame_invalid_type():
    """Verify TypeError when non-str and non-bytes are passed."""
    with pytest.raises(TypeError, match="Expected str or bytes"):
        decode_websocket_frame(12345)  # type: ignore


def test_client_ws_url_format_negotiation():
    """Verify ws_url generation with format='msgpack' vs format='json'."""
    client = FinTextClient(
        base_url="http://127.0.0.1:8000",
        api_token="test-jwt-token-xyz",
    )

    # 1. Default JSON format
    default_url = client.ws_url(ticker="NVDA")
    assert "ws://127.0.0.1:8000/ws" in default_url
    assert "token=test-jwt-token-xyz" in default_url
    assert "ticker=NVDA" in default_url
    assert "format=" not in default_url

    # 2. Explicit MessagePack format
    msgpack_url = client.ws_url(ticker="NVDA", format="msgpack")
    assert "format=msgpack" in msgpack_url
    assert "ticker=NVDA" in msgpack_url
    assert "token=test-jwt-token-xyz" in msgpack_url


def test_client_anomaly_websocket_url_format_negotiation():
    """Verify get_anomaly_websocket_url with format='msgpack' vs format='json'."""
    client = FinTextClient(
        base_url="https://api.fintext.alpha",
        api_token="prod-token-123",
    )

    # 1. Default JSON format
    default_url = client.get_anomaly_websocket_url()
    assert default_url.startswith("wss://api.fintext.alpha/ws?")
    assert "streams=anomalies" in default_url
    assert "token=prod-token-123" in default_url
    assert "format=" not in default_url

    # 2. Explicit MessagePack format
    msgpack_url = client.get_anomaly_websocket_url(format="msgpack")
    assert "format=msgpack" in msgpack_url
    assert "streams=anomalies" in msgpack_url
    assert "token=prod-token-123" in msgpack_url


@pytest.mark.asyncio
async def test_async_client_ws_url_format_negotiation():
    """Verify async client ws_url generation with format='msgpack'."""
    client = FinTextAsyncClient(
        base_url="http://127.0.0.1:8000",
        api_token="async-token-abc",
    )

    # 1. Default JSON format
    default_url = await client.ws_url(ticker="MSFT")
    assert "ticker=MSFT" in default_url
    assert "format=" not in default_url

    # 2. MessagePack format
    msgpack_url = await client.ws_url(ticker="MSFT", format="msgpack")
    assert "format=msgpack" in msgpack_url
    assert "ticker=MSFT" in msgpack_url
    assert "token=async-token-abc" in msgpack_url

    # 3. Anomaly WebSocket URL
    anomaly_url = client.get_anomaly_websocket_url(format="msgpack")
    assert "format=msgpack" in anomaly_url
    assert "streams=anomalies" in anomaly_url
