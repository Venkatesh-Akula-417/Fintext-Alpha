"""
═══════════════════════════════════════════════════════════════════════════════
FinText Alpha Vectorizer Python Client SDK — Model Validation & Calibration Tests
═══════════════════════════════════════════════════════════════════════════════
"""

import httpx
import pytest
from fintext import FinTextClient, FinTextAsyncClient, FinTextAPIError
from fintext.models import (
    CalibrationPoint,
    ClassConfusion,
    ClassificationMetrics,
    ConfusionMatrix,
    ModelValidationResponse,
    ModelValidation,
    PerClassMetrics,
)

SAMPLE_VALIDATION_REPORT = {
    "model_id": "fintext-sentiment-finbert",
    "model_version": "3.0.0",
    "dataset_version": "1.0.0",
    "dataset_size": 105,
    "evaluated_at": "2026-09-05T13:30:00Z",
    "metrics": {
        "accuracy": 0.876,
        "macro_precision": 0.868,
        "macro_recall": 0.872,
        "macro_f1": 0.870,
        "positive": {"precision": 0.889, "recall": 0.889, "f1_score": 0.889, "support": 45},
        "negative": {"precision": 0.875, "recall": 0.875, "f1_score": 0.875, "support": 40},
        "neutral": {"precision": 0.842, "recall": 0.800, "f1_score": 0.821, "support": 20},
    },
    "confusion_matrix": {
        "true_positive": {"predicted_positive": 40, "predicted_negative": 1, "predicted_neutral": 4},
        "true_negative": {"predicted_positive": 1, "predicted_negative": 35, "predicted_neutral": 4},
        "true_neutral": {"predicted_positive": 2, "predicted_negative": 2, "predicted_neutral": 16},
    },
    "calibration_curve": [
        {"bin_index": 0, "confidence_min": 0.0, "confidence_max": 0.1, "predicted_confidence_mean": 0.05, "accuracy": 0.05, "sample_count": 0},
        {"bin_index": 1, "confidence_min": 0.1, "confidence_max": 0.2, "predicted_confidence_mean": 0.15, "accuracy": 0.15, "sample_count": 0},
        {"bin_index": 2, "confidence_min": 0.2, "confidence_max": 0.3, "predicted_confidence_mean": 0.25, "accuracy": 0.25, "sample_count": 0},
        {"bin_index": 3, "confidence_min": 0.3, "confidence_max": 0.4, "predicted_confidence_mean": 0.35, "accuracy": 0.35, "sample_count": 0},
        {"bin_index": 4, "confidence_min": 0.4, "confidence_max": 0.5, "predicted_confidence_mean": 0.45, "accuracy": 0.45, "sample_count": 0},
        {"bin_index": 5, "confidence_min": 0.5, "confidence_max": 0.6, "predicted_confidence_mean": 0.55, "accuracy": 0.55, "sample_count": 0},
        {"bin_index": 6, "confidence_min": 0.6, "confidence_max": 0.7, "predicted_confidence_mean": 0.665, "accuracy": 0.700, "sample_count": 10},
        {"bin_index": 7, "confidence_min": 0.7, "confidence_max": 0.8, "predicted_confidence_mean": 0.755, "accuracy": 0.818, "sample_count": 33},
        {"bin_index": 8, "confidence_min": 0.8, "confidence_max": 0.9, "predicted_confidence_mean": 0.852, "accuracy": 0.884, "sample_count": 43},
        {"bin_index": 9, "confidence_min": 0.9, "confidence_max": 1.0, "predicted_confidence_mean": 0.941, "accuracy": 0.947, "sample_count": 19},
    ],
    "brier_score": 0.182,
    "expected_calibration_error": 0.048,
    "notes": [
        "FinBERT INT8 model demonstrated 87.6% multi-class accuracy on 105 labeled financial headlines.",
        "Macro F1 score of 0.870 surpasses the institutional production threshold (>=0.80).",
        "Expected Calibration Error (ECE) is 0.048 (within the <=0.10 threshold), confirming reliable probability estimates."
    ],
}


def test_model_validation_models_parsing():
    """Verify ModelValidationResponse and nested DTO parsing."""
    resp = ModelValidationResponse.model_validate(SAMPLE_VALIDATION_REPORT)
    assert resp.model_id == "fintext-sentiment-finbert"
    assert resp.model_version == "3.0.0"
    assert resp.dataset_size == 105
    assert resp.metrics.accuracy == 0.876
    assert resp.metrics.macro_f1 == 0.870
    assert resp.metrics.positive.support == 45
    assert resp.metrics.negative.support == 40
    assert resp.metrics.neutral.support == 20
    assert resp.confusion_matrix.true_positive.predicted_positive == 40
    assert len(resp.calibration_curve) == 10
    assert resp.expected_calibration_error == 0.048
    assert ModelValidation is ModelValidationResponse


def test_model_validation_sync_client():
    """Verify synchronous client.get_model_validation() and alias."""
    def mock_handler(request: httpx.Request):
        assert request.url.path == "/model-validation"
        assert request.method == "GET"
        assert "Bearer test_jwt_token" in request.headers.get("Authorization", "")
        return httpx.Response(200, json=SAMPLE_VALIDATION_REPORT)

    transport = httpx.MockTransport(mock_handler)
    client = FinTextClient(base_url="http://testserver", api_token="test_jwt_token")
    client._client = httpx.Client(transport=transport, base_url="http://testserver")

    resp = client.get_model_validation()
    assert isinstance(resp, ModelValidationResponse)
    assert resp.metrics.accuracy >= 0.80
    assert resp.expected_calibration_error <= 0.10

    resp_alias = client.model_validation(recalibrate=True, dataset_version="1.0.0")
    assert resp_alias.model_id == "fintext-sentiment-finbert"


@pytest.mark.asyncio
async def test_model_validation_async_client():
    """Verify asynchronous client.get_model_validation() and alias."""
    def mock_handler(request: httpx.Request):
        assert request.url.path == "/model-validation"
        assert request.method == "GET"
        assert "Bearer test_async_token" in request.headers.get("Authorization", "")
        return httpx.Response(200, json=SAMPLE_VALIDATION_REPORT)

    transport = httpx.MockTransport(mock_handler)
    client = FinTextAsyncClient(base_url="http://testserver", api_token="test_async_token")
    client._client = httpx.AsyncClient(transport=transport, base_url="http://testserver")

    resp = await client.get_model_validation()
    assert isinstance(resp, ModelValidationResponse)
    assert resp.metrics.accuracy == 0.876

    resp_alias = await client.model_validation()
    assert resp_alias.metrics.macro_f1 == 0.870
