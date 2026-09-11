"""
═══════════════════════════════════════════════════════════════════════════════
FinText Alpha Vectorizer Python Client SDK — PIT Certificate Tests
═══════════════════════════════════════════════════════════════════════════════
"""

import httpx
import pytest
from fintext import FinTextClient, FinTextAsyncClient
from fintext.models import (
    PITCertificateParams,
    PITCertificateResponse,
    PITCertificate,
    PITCertificateTests,
    PITCertificatePolicies,
    PITTestResult,
    PITDuplicateTestResult,
    PITBackfillTestResult,
)

SAMPLE_CERTIFICATE = {
    "certificate_id": "PIT-CERT-20250902-105",
    "dataset_version": "2.1.0",
    "universe": "all",
    "audit_start_date": "2025-06-01",
    "audit_end_date": "2025-08-31",
    "issued_at": "2025-09-02T10:00:00Z",
    "overall_result": "pass",
    "tests": {
        "signal_availability_ordering": {"violations": 0, "total_checked": 15000, "status": "pass"},
        "ticker_rename": {"violations": 0, "total_checked": 100, "status": "pass"},
        "delisted_security": {"violations": 0, "total_checked": 50, "status": "pass"},
        "corporate_action": {"violations": 0, "total_checked": 30, "status": "pass"},
        "duplicate_event": {"duplicate_rate_pct": 0.2, "total_checked": 15000, "status": "pass"},
        "out_of_order_event": {"violations": 0, "total_checked": 15000, "status": "pass"},
        "timestamp_precision": {"violations": 0, "total_checked": 15000, "status": "pass"},
        "backfill_consistency": {"backfill_count": 12, "policy": "within_7_days", "status": "pass"},
    },
    "policies": {
        "timestamp_policy": "triple_timestamp_utc",
        "correction_policy": "append_only_with_new_record",
        "backfill_policy": "allowed_within_7_days_with_audit_log",
        "universe_policy": "pit_aware_with_delisting",
    },
    "signature": "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    "status": "active",
}


def test_pit_certificate_sync_client():
    """Verify synchronous client.pit_certificate() returns structured PITCertificateResponse."""
    def mock_handler(request: httpx.Request):
        assert request.url.path == "/pit/certificate"
        assert request.method == "GET"
        assert "Bearer test_jwt_token" in request.headers.get("Authorization", "")
        return httpx.Response(200, json=SAMPLE_CERTIFICATE)

    transport = httpx.MockTransport(mock_handler)
    client = FinTextClient(base_url="http://testserver", api_token="test_jwt_token")
    client._client = httpx.Client(transport=transport, base_url="http://testserver")

    cert = client.pit_certificate(
        dataset_version="2.1.0",
        universe="all",
        start_date="2025-06-01",
        end_date="2025-08-31",
    )

    assert isinstance(cert, PITCertificateResponse)
    assert cert.certificate_id == "PIT-CERT-20250902-105"
    assert cert.dataset_version == "2.1.0"
    assert cert.universe == "all"
    assert cert.overall_result == "pass"
    assert cert.status == "active"
    assert cert.signature.startswith("sha256:")
    assert cert.tests.signal_availability_ordering.violations == 0


def test_pit_certificate_sync_alias():
    """Verify ergonomic alias client.certify_pit() returns identical structure."""
    def mock_handler(request: httpx.Request):
        assert request.url.path == "/pit/certificate"
        return httpx.Response(200, json=SAMPLE_CERTIFICATE)

    transport = httpx.MockTransport(mock_handler)
    client = FinTextClient(base_url="http://testserver", api_token="test_jwt_token")
    client._client = httpx.Client(transport=transport, base_url="http://testserver")

    cert = client.certify_pit()
    assert isinstance(cert, PITCertificate)
    assert cert.overall_result == "pass"


@pytest.mark.asyncio
async def test_pit_certificate_async_client():
    """Verify asynchronous client.pit_certificate() returns structured response."""
    def mock_handler(request: httpx.Request):
        assert request.url.path == "/pit/certificate"
        assert request.method == "GET"
        assert "Bearer async_token_123" in request.headers.get("Authorization", "")
        return httpx.Response(200, json=SAMPLE_CERTIFICATE)

    transport = httpx.MockTransport(mock_handler)
    async_client = FinTextAsyncClient(base_url="http://testserver", api_token="async_token_123")
    async_client._client = httpx.AsyncClient(transport=transport, base_url="http://testserver")

    cert = await async_client.pit_certificate(dataset_version="2.1.0", universe="sp500")
    assert isinstance(cert, PITCertificateResponse)
    assert cert.overall_result == "pass"
    assert cert.tests.ticker_rename.violations == 0


@pytest.mark.asyncio
async def test_pit_certificate_async_alias():
    """Verify asynchronous alias async_client.certify_pit() works."""
    def mock_handler(request: httpx.Request):
        assert request.url.path == "/pit/certificate"
        return httpx.Response(200, json=SAMPLE_CERTIFICATE)

    transport = httpx.MockTransport(mock_handler)
    async_client = FinTextAsyncClient(base_url="http://testserver", api_token="async_token_123")
    async_client._client = httpx.AsyncClient(transport=transport, base_url="http://testserver")

    cert = await async_client.certify_pit()
    assert isinstance(cert, PITCertificate)
    assert cert.certificate_id.startswith("PIT-CERT-")


def test_pit_certificate_query_params():
    """Verify query parameters are forwarded properly in GET request."""
    captured_params = {}

    def mock_handler(request: httpx.Request):
        assert request.url.path == "/pit/certificate"
        for k, v in request.url.params.items():
            captured_params[k] = v
        return httpx.Response(200, json=SAMPLE_CERTIFICATE)

    transport = httpx.MockTransport(mock_handler)
    client = FinTextClient(base_url="http://testserver", api_token="test_jwt_token")
    client._client = httpx.Client(transport=transport, base_url="http://testserver")

    client.pit_certificate(
        dataset_version="2.0.0",
        universe="AAPL,MSFT,NVDA",
        start_date="2025-01-01",
        end_date="2025-06-30",
    )

    assert captured_params.get("dataset_version") == "2.0.0"
    assert captured_params.get("universe") == "AAPL,MSFT,NVDA"
    assert captured_params.get("start_date") == "2025-01-01"
    assert captured_params.get("end_date") == "2025-06-30"


def test_pit_certificate_signature_verification():
    """Verify cryptographic SHA-256 signature is present and formatted."""
    cert = PITCertificateResponse.model_validate(SAMPLE_CERTIFICATE)
    assert cert.signature.startswith("sha256:")
    sig_hex = cert.signature.split("sha256:")[1]
    assert len(sig_hex) == 64
    assert all(c in "0123456789abcdefABCDEF" for c in sig_hex)


def test_pit_certificate_all_tests_presence():
    """Verify all 8 automated look-ahead bias tests are present in certificate response."""
    cert = PITCertificateResponse.model_validate(SAMPLE_CERTIFICATE)
    tests = cert.tests

    assert isinstance(tests.signal_availability_ordering, PITTestResult)
    assert isinstance(tests.ticker_rename, PITTestResult)
    assert isinstance(tests.delisted_security, PITTestResult)
    assert isinstance(tests.corporate_action, PITTestResult)
    assert isinstance(tests.duplicate_event, PITDuplicateTestResult)
    assert isinstance(tests.out_of_order_event, PITTestResult)
    assert isinstance(tests.timestamp_precision, PITTestResult)
    assert isinstance(tests.backfill_consistency, PITBackfillTestResult)

    assert tests.signal_availability_ordering.violations == 0
    assert tests.duplicate_event.duplicate_rate_pct == 0.2
    assert tests.backfill_consistency.policy == "within_7_days"


def test_pit_certificate_policies_structure():
    """Verify platform policies are properly certified."""
    cert = PITCertificateResponse.model_validate(SAMPLE_CERTIFICATE)
    policies = cert.policies

    assert policies.timestamp_policy == "triple_timestamp_utc"
    assert policies.correction_policy == "append_only_with_new_record"
    assert policies.backfill_policy == "allowed_within_7_days_with_audit_log"
    assert policies.universe_policy == "pit_aware_with_delisting"


def test_pit_certificate_archival_fields():
    """Verify archive_object_key and archive_timestamp are parsed when present and optional when omitted."""
    # When omitted
    cert_none = PITCertificateResponse.model_validate(SAMPLE_CERTIFICATE)
    assert cert_none.archive_object_key is None
    assert cert_none.archive_timestamp is None

    # When present
    sample_with_archive = dict(SAMPLE_CERTIFICATE)
    sample_with_archive["archive_object_key"] = "pit-cert/pit-cert-2.1.0-all-2025-06-01-2025-08-31-abc123.json"
    sample_with_archive["archive_timestamp"] = "2025-09-02T10:00:01Z"

    cert_archived = PITCertificateResponse.model_validate(sample_with_archive)
    assert cert_archived.archive_object_key == "pit-cert/pit-cert-2.1.0-all-2025-06-01-2025-08-31-abc123.json"
    assert cert_archived.archive_timestamp == "2025-09-02T10:00:01Z"

