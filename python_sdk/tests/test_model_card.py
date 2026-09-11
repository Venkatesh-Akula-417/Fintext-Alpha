"""
═══════════════════════════════════════════════════════════════════════════════
FinText Alpha Vectorizer Python Client SDK — Model Card & Lineage Tests
═══════════════════════════════════════════════════════════════════════════════
"""

import httpx
import pytest
from fintext import FinTextClient, FinTextAsyncClient
from fintext.models import (
    ModelCardResponse,
    HardwareRequirements,
    VersionHistoryItem,
    LicensingInfo,
)


SAMPLE_MODEL_CARD = {
    "model_id": "fintext-sentiment-minilm-l6-v2",
    "model_name": "MiniLM-L6-v2 Fine-Tuned for Financial Sentiment",
    "architecture": "transformer_encoder",
    "base_model": "sentence-transformers/all-MiniLM-L6-v2",
    "fine_tuning_dataset": "proprietary_financial_news_and_filings_corpus",
    "task": "financial_sentiment_classification",
    "precision": "FP16",
    "quantization": "INT8_dynamic",
    "sequence_length": 32,
    "chunking_strategy": "sliding_window_overlap_8",
    "mean_latency_ms": 0.85,
    "p95_latency_ms": 1.2,
    "p99_latency_ms": 1.5,
    "hardware_requirements": {
        "cpu": "8 vCPU",
        "memory_gb": 4,
        "gpu": "optional_cuda_tensorrt",
    },
    "version_history": [
        {
            "version": "2.1.0",
            "release_date": "2025-08-01",
            "changes": "Sliding window chunking added",
        },
        {
            "version": "2.0.0",
            "release_date": "2025-06-15",
            "changes": "Base MiniLM-L6-v2 fine-tuned for sentiment",
        },
    ],
    "licensing": {
        "model_license": "internal_proprietary",
        "training_data_rights": "verified_internal_use",
    },
}


def test_model_card_sync_client():
    """Verify synchronous client.get_model_card() returns structured ModelCardResponse."""
    def mock_handler(request: httpx.Request):
        assert request.url.path == "/model-card"
        assert request.method == "GET"
        return httpx.Response(200, json=SAMPLE_MODEL_CARD)

    transport = httpx.MockTransport(mock_handler)
    client = FinTextClient(base_url="http://testserver", api_token="test_jwt_token")
    client._client = httpx.Client(transport=transport, base_url="http://testserver")

    card = client.get_model_card()

    assert isinstance(card, ModelCardResponse)
    assert card.model_id == "fintext-sentiment-minilm-l6-v2"
    assert card.model_name == "MiniLM-L6-v2 Fine-Tuned for Financial Sentiment"
    assert card.architecture == "transformer_encoder"
    assert card.base_model == "sentence-transformers/all-MiniLM-L6-v2"
    assert card.precision == "FP16"
    assert card.quantization == "INT8_dynamic"
    assert card.sequence_length == 32
    assert card.chunking_strategy == "sliding_window_overlap_8"
    assert card.mean_latency_ms == pytest.approx(0.85)
    assert card.p95_latency_ms == pytest.approx(1.2)
    assert card.p99_latency_ms == pytest.approx(1.5)

    assert isinstance(card.hardware_requirements, HardwareRequirements)
    assert card.hardware_requirements.cpu == "8 vCPU"
    assert card.hardware_requirements.memory_gb == 4
    assert card.hardware_requirements.gpu == "optional_cuda_tensorrt"

    assert len(card.version_history) == 2
    assert isinstance(card.version_history[0], VersionHistoryItem)
    assert card.version_history[0].version == "2.1.0"
    assert card.version_history[0].release_date == "2025-08-01"

    assert isinstance(card.licensing, LicensingInfo)
    assert card.licensing.model_license == "internal_proprietary"
    assert card.licensing.training_data_rights == "verified_internal_use"

    # Test alias
    card_alias = client.model_card()
    assert card_alias.model_id == card.model_id


@pytest.mark.asyncio
async def test_model_card_async_client():
    """Verify asynchronous async_client.get_model_card() returns structured ModelCardResponse."""
    def mock_handler(request: httpx.Request):
        assert request.url.path == "/model-card"
        assert request.method == "GET"
        return httpx.Response(200, json=SAMPLE_MODEL_CARD)

    transport = httpx.MockTransport(mock_handler)
    client = FinTextAsyncClient(base_url="http://testserver", api_token="test_jwt_token")
    client._client = httpx.AsyncClient(transport=transport, base_url="http://testserver")

    card = await client.get_model_card()

    assert isinstance(card, ModelCardResponse)
    assert card.model_id == "fintext-sentiment-minilm-l6-v2"
    assert card.model_name == "MiniLM-L6-v2 Fine-Tuned for Financial Sentiment"
    assert card.precision == "FP16"
    assert card.hardware_requirements.memory_gb == 4
    assert len(card.version_history) == 2

    # Test alias
    card_alias = await client.model_card()
    assert card_alias.model_id == card.model_id
