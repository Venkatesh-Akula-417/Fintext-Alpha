"""
═══════════════════════════════════════════════════════════════════════════════
FinText Alpha Vectorizer Python Client SDK — Point-in-Time (PIT) Replay Tests
═══════════════════════════════════════════════════════════════════════════════
"""

import httpx
import pytest
from fintext import FinTextClient, FinTextAsyncClient
from fintext.exceptions import FinTextValidationError
from fintext.models import (
    PITReplayNewsItem,
    PITReplayFilingItem,
    PITReplayEventItem,
    PITReplaySentimentItem,
    PITReplaySummary,
    PITReplayConsistency,
    PITReplayResponse,
    PITReplay,
)

SAMPLE_PIT_REPLAY = {
    "ticker": "AAPL",
    "as_of_utc": "2025-06-15T14:30:00+00:00",
    "news_articles": [
        {
            "id": "news-aapl-0001",
            "title": "AAPL Expands Cloud Infrastructure and Neural Vector Capacity",
            "source": "Institutional Wire",
            "published_utc": "2025-06-15T13:55:00+00:00",
            "ingested_utc": "2025-06-15T13:55:00.045000+00:00",
            "db_commit_utc": "2025-06-15T13:55:00.090000+00:00",
            "sentiment_score": 0.6500,
        },
        {
            "id": "news-aapl-0002",
            "title": "AAPL Institutional Earnings & Order Flow Volume Update",
            "source": "SEC EDGAR",
            "published_utc": "2025-06-15T13:20:00+00:00",
            "ingested_utc": "2025-06-15T13:20:00.045000+00:00",
            "db_commit_utc": "2025-06-15T13:20:00.090000+00:00",
            "sentiment_score": 0.4200,
        },
    ],
    "filings": [
        {
            "id": "filing-aapl-8k-0001",
            "form_type": "8-K",
            "filing_date": "2025-06-14",
            "accession_number": "0000320193-25-000050",
            "event_category": "Item 2.02 Results of Operations and Financial Condition",
            "published_utc": "2025-06-14T20:30:00+00:00",
            "ingested_utc": "2025-06-14T20:30:00.120000+00:00",
        }
    ],
    "events": [
        {
            "id": "evt-aapl-0001",
            "event_type": "EARNINGS_ANNOUNCEMENT",
            "ticker": "AAPL",
            "event_date": "2025-06-13",
            "published_utc": "2025-06-13T14:30:00+00:00",
            "ingested_utc": "2025-06-13T14:30:00.080000+00:00",
        }
    ],
    "sentiment_records": [
        {
            "id": "sent-aapl-0001",
            "ticker": "AAPL",
            "published_utc": "2025-06-15T14:10:00+00:00",
            "ingested_utc": "2025-06-15T14:10:00.030000+00:00",
            "db_commit_utc": "2025-06-15T14:10:00.075000+00:00",
            "sentiment_score": 0.7500,
            "sentiment_label": "POSITIVE",
            "confidence": 0.9400,
        }
    ],
    "summary": {
        "news_count": 2,
        "filings_count": 1,
        "events_count": 1,
        "sentiment_count": 1,
        "total_records": 5,
    },
    "replay_consistency": {
        "all_records_consistent": True,
        "violations_count": 0,
        "invariant": "published_utc <= ingested_utc <= db_commit_utc <= as_of_utc",
        "message": "All records strictly verified point-in-time consistent with zero look-ahead bias.",
    },
    "model_generated_at": "2025-06-15T14:10:00.075000+00:00",
    "message": "Point-in-time historical state successfully reconstructed for AAPL as of 2025-06-15T14:30:00+00:00",
}


def test_pit_replay_sync_client():
    """Verify synchronous client.pit_replay() returns structured PITReplayResponse."""
    captured_params = {}

    def mock_handler(request: httpx.Request):
        assert request.url.path == "/pit/replay"
        assert request.method == "GET"
        assert "Bearer test_jwt_token" in request.headers.get("Authorization", "")
        captured_params.update(dict(request.url.params))
        return httpx.Response(200, json=SAMPLE_PIT_REPLAY)

    transport = httpx.MockTransport(mock_handler)
    client = FinTextClient(base_url="http://testserver", api_token="test_jwt_token")
    client._client = httpx.Client(transport=transport, base_url="http://testserver")

    replay = client.pit_replay(
        ticker="AAPL",
        as_of_utc="2025-06-15T14:30:00Z",
        include_news=True,
        include_filings=True,
        include_events=True,
        include_sentiment=True,
        limit=50,
    )

    assert isinstance(replay, PITReplayResponse)
    assert replay.ticker == "AAPL"
    assert replay.as_of_utc == "2025-06-15T14:30:00+00:00"
    assert replay.replay_consistency.all_records_consistent is True
    assert replay.replay_consistency.violations_count == 0
    assert replay.summary.total_records == 5
    assert len(replay.news_articles) == 2
    assert len(replay.filings) == 1
    assert len(replay.events) == 1
    assert len(replay.sentiment_records) == 1
    assert replay.news_articles[0].sentiment_score == 0.65
    assert replay.filings[0].form_type == "8-K"
    assert replay.events[0].event_type == "EARNINGS_ANNOUNCEMENT"
    assert replay.sentiment_records[0].sentiment_label == "POSITIVE"

    assert captured_params.get("ticker") == "AAPL"
    assert captured_params.get("as_of_utc") == "2025-06-15T14:30:00Z"
    assert captured_params.get("limit") == "50"


def test_pit_replay_sync_alias():
    """Verify ergonomic alias client.replay() behaves identically to pit_replay()."""
    def mock_handler(request: httpx.Request):
        assert request.url.path == "/pit/replay"
        return httpx.Response(200, json=SAMPLE_PIT_REPLAY)

    transport = httpx.MockTransport(mock_handler)
    client = FinTextClient(base_url="http://testserver", api_token="test_jwt_token")
    client._client = httpx.Client(transport=transport, base_url="http://testserver")

    replay = client.replay(ticker="AAPL", as_of_utc="2025-06-15T14:30:00Z")
    assert isinstance(replay, PITReplay)
    assert replay.ticker == "AAPL"


@pytest.mark.asyncio
async def test_pit_replay_async_client():
    """Verify asynchronous client.pit_replay() returns structured PITReplayResponse."""
    def mock_handler(request: httpx.Request):
        assert request.url.path == "/pit/replay"
        assert request.method == "GET"
        assert "Bearer async_test_token" in request.headers.get("Authorization", "")
        return httpx.Response(200, json=SAMPLE_PIT_REPLAY)

    transport = httpx.MockTransport(mock_handler)
    async_client = FinTextAsyncClient(base_url="http://testserver", api_token="async_test_token")
    async_client._client = httpx.AsyncClient(transport=transport, base_url="http://testserver")

    replay = await async_client.pit_replay(ticker="AAPL", as_of_utc="2025-06-15T14:30:00Z")

    assert isinstance(replay, PITReplayResponse)
    assert replay.ticker == "AAPL"
    assert replay.replay_consistency.all_records_consistent is True
    assert replay.summary.total_records == 5


@pytest.mark.asyncio
async def test_pit_replay_async_alias():
    """Verify asynchronous alias async_client.replay() works as expected."""
    def mock_handler(request: httpx.Request):
        assert request.url.path == "/pit/replay"
        return httpx.Response(200, json=SAMPLE_PIT_REPLAY)

    transport = httpx.MockTransport(mock_handler)
    async_client = FinTextAsyncClient(base_url="http://testserver", api_token="async_test_token")
    async_client._client = httpx.AsyncClient(transport=transport, base_url="http://testserver")

    replay = await async_client.replay(ticker="AAPL", as_of_utc="2025-06-15T14:30:00Z")
    assert isinstance(replay, PITReplay)
    assert replay.ticker == "AAPL"


def test_pit_replay_validation_empty_ticker():
    """Verify client rejects empty ticker with FinTextValidationError."""
    client = FinTextClient(base_url="http://testserver", api_token="test_jwt_token")
    with pytest.raises(FinTextValidationError) as exc_info:
        client.pit_replay(ticker="", as_of_utc="2025-06-15T14:30:00Z")
    assert "ticker" in str(exc_info.value).lower()


def test_pit_replay_validation_empty_timestamp():
    """Verify client rejects empty as_of_utc timestamp with FinTextValidationError."""
    client = FinTextClient(base_url="http://testserver", api_token="test_jwt_token")
    with pytest.raises(FinTextValidationError) as exc_info:
        client.pit_replay(ticker="AAPL", as_of_utc="   ")
    assert "as_of_utc" in str(exc_info.value).lower()


def test_pit_replay_validation_invalid_limit():
    """Verify client rejects limit outside [1, 200] with FinTextValidationError."""
    client = FinTextClient(base_url="http://testserver", api_token="test_jwt_token")
    with pytest.raises(FinTextValidationError):
        client.pit_replay(ticker="AAPL", as_of_utc="2025-06-15T14:30:00Z", limit=0)

    with pytest.raises(FinTextValidationError):
        client.pit_replay(ticker="AAPL", as_of_utc="2025-06-15T14:30:00Z", limit=250)


def test_pit_replay_query_params_forwarding():
    """Verify optional filter parameters are correctly serialised into query string."""
    captured_params = {}

    def mock_handler(request: httpx.Request):
        captured_params.update(dict(request.url.params))
        return httpx.Response(200, json=SAMPLE_PIT_REPLAY)

    transport = httpx.MockTransport(mock_handler)
    client = FinTextClient(base_url="http://testserver", api_token="test_jwt_token")
    client._client = httpx.Client(transport=transport, base_url="http://testserver")

    client.pit_replay(
        ticker="NVDA",
        as_of_utc="2025-07-01T12:00:00Z",
        include_news=False,
        include_filings=True,
        include_events=False,
        include_sentiment=True,
        limit=25,
    )

    assert captured_params.get("ticker") == "NVDA"
    assert captured_params.get("as_of_utc") == "2025-07-01T12:00:00Z"
    assert captured_params.get("include_news") == "false"
    assert captured_params.get("include_filings") == "true"
    assert captured_params.get("include_events") == "false"
    assert captured_params.get("include_sentiment") == "true"
    assert captured_params.get("limit") == "25"
