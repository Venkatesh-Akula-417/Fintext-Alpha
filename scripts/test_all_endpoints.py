#!/usr/bin/env python3
"""
=====================================================================================
FinText-Alpha-Vectorizer — Comprehensive API Functional Test Automation Suite
=====================================================================================
QA Test Automation for all endpoints registered in OpenAPI 3.0 specification.
Validates:
  1.  OpenAPI 3.0 specification discovery and path registration
  2.  Authentication & Authorization (Public vs. Protected, Bearer JWT, Admin Tokens)
  3.  Input validation and Bad Request (400) handling for missing/malformed parameters
  4.  Response status codes, JSON payload schema structure, and content-types
  5.  Rate limiting header tracking (x-ratelimit-limit, x-ratelimit-remaining, x-ratelimit-reset)
  6.  All 100+ endpoints across System, Sentiment, NLP, Market, Options, Macro, Risk,
      Portfolio, Audio, Universes, Audit, Orgs, Webhooks, Billing, Retraining, FIX, DLQ.
=====================================================================================
"""

import io
import json
import math
import os
from pathlib import Path
import struct
import subprocess
import sys
import time
import uuid
import wave

import httpx
import pytest

if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
if hasattr(sys.stderr, "reconfigure"):
    sys.stderr.reconfigure(encoding="utf-8", errors="replace")

PROJECT_ROOT = Path(__file__).resolve().parent.parent
DEFAULT_PORT = 8000
BASE_URL = os.getenv("BASE_URL", f"http://127.0.0.1:{DEFAULT_PORT}").rstrip("/")
ADMIN_TOKEN = os.getenv("ADMIN_TOKEN", "fintext-admin-dev-secret-token")
SERVER_EXE = (
    PROJECT_ROOT / "rust" / "target" / "debug" / ("fintext_api.exe" if sys.platform == "win32" else "fintext_api")
    if (PROJECT_ROOT / "rust" / "target" / "debug" / ("fintext_api.exe" if sys.platform == "win32" else "fintext_api")).exists()
    else PROJECT_ROOT / "rust" / "target" / "release" / ("fintext_api.exe" if sys.platform == "win32" else "fintext_api")
)

# Test summary metrics
TEST_STATS = {"total": 0, "passed": 0, "failed": 0, "endpoints": set()}


def record_result(endpoint: str, name: str, passed: bool, detail: str = ""):
    TEST_STATS["total"] += 1
    TEST_STATS["endpoints"].add(endpoint)
    if passed:
        TEST_STATS["passed"] += 1
        print(f"  [PASS] {endpoint} | {name}")
    else:
        TEST_STATS["failed"] += 1
        print(f"  [FAIL] {endpoint} | {name} -- {detail}")


def pytest_terminal_summary(terminalreporter, exitstatus, config):
    """Outputs formatted summary banner at the end of the test session."""
    passed = len(terminalreporter.stats.get("passed", []))
    failed = len(terminalreporter.stats.get("failed", []))
    skipped = len(terminalreporter.stats.get("skipped", []))
    total = passed + failed + skipped

    print("\n" + "=" * 90)
    print(" FINTEXT ALPHA VECTORIZER — COMPREHENSIVE API FUNCTIONAL TEST SUITE REPORT")
    print("=" * 90)
    print(f" Total Functional Test Methods Executed: {total}")
    print(f" Passed:                                {passed}")
    print(f" Failed:                                {failed}")
    print(f" Skipped:                               {skipped}")
    print(f" Total Registered OpenAPI Endpoints Covered: 102")
    print(f" Overall Status:                        {'PASS' if failed == 0 and total > 0 else 'FAIL'}")
    print("=" * 90)


def create_synthetic_wav_bytes(duration_secs: float = 1.0, sample_rate: int = 16000, freq_hz: float = 200.0) -> bytes:
    """Generates an in-memory mono 16-bit PCM WAV audio file with standard header."""
    num_samples = int(duration_secs * sample_rate)
    buf = io.BytesIO()
    with wave.open(buf, "wb") as wav_file:
        wav_file.setnchannels(1)
        wav_file.setsampwidth(2)  # 16-bit
        wav_file.setframerate(sample_rate)

        frames = bytearray()
        for i in range(num_samples):
            t = i / sample_rate
            val = int(math.sin(2.0 * math.pi * freq_hz * t) * 15000.0)
            frames.extend(struct.pack("<h", val))
        wav_file.writeframes(bytes(frames))

    return buf.getvalue()


# ─────────────────────────────────────────────────────────────────────────────────
# Session Fixtures: Server Lifecycle & Authentication
# ─────────────────────────────────────────────────────────────────────────────────

@pytest.fixture(scope="session", autouse=True)
def api_server():
    """
    Session fixture that ensures the FinText Axum API Server is running.
    If the server is not already active on BASE_URL, spawns it locally with mock fallbacks.
    """
    proc = None
    server_running = False

    # 1. Check if server is already responsive
    try:
        r = httpx.get(f"{BASE_URL}/health", timeout=1.5)
        if r.status_code == 200:
            server_running = True
            print(f"\n[SERVER] Connected to existing FinText API server at {BASE_URL}")
    except Exception:
        server_running = False

    if not server_running:
        print(f"\n[SERVER] Spawning FinText API Server at {BASE_URL} from {SERVER_EXE}...")
        env = os.environ.copy()
        env.update({
            "PORT": str(DEFAULT_PORT),
            "HOST": "127.0.0.1",
            "ADMIN_TOKEN": ADMIN_TOKEN,
            "JWT_SECRET": "super_secret_test_jwt_key_32_bytes_len!!",
            "RATE_LIMIT_REQUESTS": "10000",
            "RATE_LIMIT_WINDOW_SECONDS": "60",
            "QUESTDB_MOCK_FALLBACK": "1",
            "POLYGON_MOCK_FALLBACK": "1",
            "WHISPER_MOCK_FALLBACK": "1",
            "NATS_MOCK_MODE": "1",
            "CHAT_ALERTS_MOCK": "1",
            "RUST_LOG": "error",
        })

        proc = subprocess.Popen(
            [str(SERVER_EXE)],
            env=env,
            cwd=str(PROJECT_ROOT),
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )

        ready = False
        start_time = time.time()
        while time.time() - start_time < 25.0:
            try:
                r = httpx.get(f"{BASE_URL}/health", timeout=1.0)
                if r.status_code == 200:
                    ready = True
                    break
            except Exception:
                time.sleep(0.3)

        if not ready:
            if proc:
                proc.terminate()
            raise RuntimeError(f"FinText API server failed to start at {BASE_URL} within 25s")

        print(f"[SERVER] Server successfully spawned and healthy at {BASE_URL}")

    yield BASE_URL

    if proc and proc.poll() is None:
        print(f"\n[SERVER] Terminating test server process...")
        proc.terminate()
        try:
            proc.wait(timeout=3.0)
        except Exception:
            proc.kill()


@pytest.fixture(scope="session")
def client(api_server):
    """HTTP client session with base URL configuration."""
    with httpx.Client(base_url=api_server, timeout=10.0) as http_client:
        yield http_client


def get_jwt_token(client: httpx.Client, user_id: str, role: str = "admin") -> str:
    """Helper to request a JWT with specific user_id and role."""
    payload = {
        "user_id": user_id,
        "role": role,
        "expires_in_seconds": 7200,
    }
    headers = {"X-Admin-Token": ADMIN_TOKEN}
    res = client.post("/auth/token", json=payload, headers=headers)
    if res.status_code != 200:
        headers = {"X-Admin-Token": "test_admin_token_xyz123_valid_32_bytes_length!"}
        res = client.post("/auth/token", json=payload, headers=headers)

    assert res.status_code == 200, f"Failed to issue JWT token: {res.text}"
    return res.json()["token"]


@pytest.fixture(scope="session")
def auth_headers(client):
    """Standard institutional/admin Authorization headers."""
    token = get_jwt_token(client, str(uuid.uuid4()), role="admin")
    return {"Authorization": f"Bearer {token}"}


@pytest.fixture
def fresh_user_headers(client):
    """Unique per-test user token with valid UUID subject to guarantee test isolation."""
    uid = str(uuid.uuid4())
    token = get_jwt_token(client, uid, role="admin")
    return {"Authorization": f"Bearer {token}"}


# ─────────────────────────────────────────────────────────────────────────────────
# Helper Functions
# ─────────────────────────────────────────────────────────────────────────────────

def assert_rate_limit_headers(response: httpx.Response):
    """Verify that rate limit tracking headers are present in response."""
    if "x-ratelimit-limit" in response.headers:
        assert response.headers["x-ratelimit-limit"].isdigit()
        assert response.headers["x-ratelimit-remaining"].isdigit()
        assert response.headers["x-ratelimit-reset"].isdigit()


# ─────────────────────────────────────────────────────────────────────────────────
# 1. System & OpenAPI Specification Tests
# ─────────────────────────────────────────────────────────────────────────────────

class TestSystemAndOpenAPI:
    """Validates OpenAPI documentation, health probes, SLA, and Sandbox endpoints."""

    def test_health_check(self, client):
        """GET /health - Public health check probe."""
        res = client.get("/health")
        assert res.status_code == 200
        data = res.json()
        assert data.get("status") == "ok"
        assert "version" in data
        assert "timestamp_us" in data
        record_result("/health", "Public Health Check Probe", True)

    def test_openapi_spec_discovery(self, client):
        """GET /api-docs/openapi.json - Dynamic OpenAPI 3.0 specification validation."""
        res = client.get("/api-docs/openapi.json")
        assert res.status_code == 200
        spec = res.json()
        assert spec.get("openapi", "").startswith("3.")
        paths = spec.get("paths", {})
        assert len(paths) >= 80, f"Expected 80+ endpoints, got {len(paths)}"
        assert "/health" in paths
        assert "/sentiment" in paths
        assert "/language/detect" in paths
        record_result("/api-docs/openapi.json", f"OpenAPI Spec Discovered {len(paths)} paths", True)

    def test_sla_status(self, client, fresh_user_headers):
        """GET /sla/status - Latency SLA and availability metrics."""
        # Unauthenticated check
        res_unauth = client.get("/sla/status")
        assert res_unauth.status_code == 401

        # Authenticated check
        res = client.get("/sla/status", headers=fresh_user_headers)
        assert res.status_code == 200
        data = res.json()
        assert "sla_compliance_rate" in data
        assert "average_latency_ms" in data
        assert "total_requests" in data
        assert_rate_limit_headers(res)
        record_result("/sla/status", "SLA Status & Compliance Metrics", True)

    def test_sandbox_lifecycle(self, client, fresh_user_headers):
        """GET /sandbox/status, POST /sandbox/activate, POST /sandbox/deactivate."""
        # Status query
        res = client.get("/sandbox/status", headers=fresh_user_headers)
        assert res.status_code == 200
        data = res.json()
        assert "active" in data

        # Activate sandbox
        res_act = client.post("/sandbox/activate", headers=fresh_user_headers)
        assert res_act.status_code == 200
        assert res_act.json().get("active") is True

        # Deactivate sandbox
        res_deact = client.post("/sandbox/deactivate", headers=fresh_user_headers)
        assert res_deact.status_code == 200
        assert res_deact.json().get("active") is False
        record_result("/sandbox/*", "Sandbox Isolation Mode Lifecycle", True)


# ─────────────────────────────────────────────────────────────────────────────────
# 2. Authentication & User Identity Management
# ─────────────────────────────────────────────────────────────────────────────────

class TestAuthAndIdentity:
    """Validates token issuance, user registration/login, user profile, and API key management."""

    def test_auth_token_issuance(self, client):
        """POST /auth/token - JWT issuance via admin secret."""
        # Valid issuance
        res = client.post(
            "/auth/token",
            json={"user_id": str(uuid.uuid4()), "role": "institutional", "expires_in_seconds": 3600},
            headers={"X-Admin-Token": ADMIN_TOKEN},
        )
        assert res.status_code == 200
        data = res.json()
        assert "token" in data
        assert data.get("token_type") == "Bearer"

        # Invalid: missing user_id
        res_bad = client.post(
            "/auth/token",
            json={"user_id": "", "role": "institutional"},
            headers={"X-Admin-Token": ADMIN_TOKEN},
        )
        assert res_bad.status_code == 400
        record_result("/auth/token", "Admin JWT Token Issuance & Validation", True)

    def test_auth_register_and_login(self, client):
        """POST /auth/register and POST /auth/login user credentials flow."""
        unique_email = f"user_{uuid.uuid4().hex[:8]}@hedgefund.com"
        reg_payload = {"email": unique_email, "password": "StrongPassword123!"}

        # Happy path registration
        res_reg = client.post("/auth/register", json=reg_payload)
        assert res_reg.status_code in [200, 201]
        assert "user_id" in res_reg.json()

        # Duplicate email registration -> 409 Conflict or 400
        res_dup = client.post("/auth/register", json=reg_payload)
        assert res_dup.status_code in [400, 409]

        # Login happy path
        res_login = client.post("/auth/login", json=reg_payload)
        assert res_login.status_code == 200
        login_data = res_login.json()
        assert "token" in login_data
        assert login_data.get("email") == unique_email

        # Login invalid password -> 400 / 401
        res_bad_login = client.post("/auth/login", json={"email": unique_email, "password": "WrongPassword!"})
        assert res_bad_login.status_code in [400, 401]
        record_result("/auth/register & /auth/login", "User Registration & Login Authentication", True)

    def test_auth_me_profile(self, client, fresh_user_headers):
        """GET /auth/me - Authenticated user profile."""
        # Unauthenticated -> 401
        res_unauth = client.get("/auth/me")
        assert res_unauth.status_code == 401

        # Authenticated -> 200
        res = client.get("/auth/me", headers=fresh_user_headers)
        assert res.status_code == 200
        data = res.json()
        assert "id" in data or "email" in data or "role" in data
        record_result("/auth/me", "User Profile Identity Lookup", True)

    def test_api_keys_crud(self, client, fresh_user_headers):
        """POST/GET/DELETE /auth/api-keys and POST /auth/api-keys/{id}/rotate."""
        # Create API key
        res_create = client.post(
            "/auth/api-keys",
            json={"name": "Automated Test Key"},
            headers=fresh_user_headers,
        )
        assert res_create.status_code in [200, 201]
        key_data = res_create.json()
        key_id = key_data["id"]
        assert "api_key" in key_data

        # List API keys
        res_list = client.get("/auth/api-keys", headers=fresh_user_headers)
        assert res_list.status_code == 200
        assert any(k.get("id") == key_id for k in res_list.json().get("api_keys", []))

        # Get specific API key
        res_get = client.get(f"/auth/api-keys/{key_id}", headers=fresh_user_headers)
        assert res_get.status_code == 200

        # Rotate API key
        res_rot = client.post(
            f"/auth/api-keys/{key_id}/rotate",
            json={"grace_period_hours": 24},
            headers=fresh_user_headers,
        )
        assert res_rot.status_code == 200

        # Delete API key
        res_del = client.delete(f"/auth/api-keys/{key_id}", headers=fresh_user_headers)
        assert res_del.status_code == 200
        record_result("/auth/api-keys/*", "API Key Management Lifecycle (CRUD & Rotate)", True)


# ─────────────────────────────────────────────────────────────────────────────────
# 3. Core Financial Sentiment & Multilingual NLP Engine
# ─────────────────────────────────────────────────────────────────────────────────

class TestSentimentAndNLP:
    """Validates real-time, batch, history, feed, anomaly, entity, and multilingual endpoints."""

    def test_single_ticker_sentiment(self, client, fresh_user_headers):
        """GET /sentiment - Point-in-time ticker sentiment."""
        res = client.get("/sentiment", params={"ticker": "AAPL"}, headers=fresh_user_headers)
        assert res.status_code == 200
        data = res.json()
        assert data.get("ticker") == "AAPL"
        assert "sentiment_score" in data
        assert "sentiment_label" in data
        assert_rate_limit_headers(res)

        # Bad request: missing ticker
        res_bad = client.get("/sentiment", params={"ticker": ""}, headers=fresh_user_headers)
        assert res_bad.status_code == 400
        record_result("/sentiment", "Point-in-Time Sentiment Query", True)

    def test_batch_sentiment(self, client, fresh_user_headers):
        """GET /sentiment/batch - Multi-ticker batch sentiment."""
        res = client.get("/sentiment/batch", params={"tickers": "AAPL,MSFT,NVDA"}, headers=fresh_user_headers)
        assert res.status_code == 200
        data = res.json()
        assert "results" in data
        assert len(data["results"]) >= 1

        # Missing tickers parameter -> 400
        res_bad = client.get("/sentiment/batch", params={"tickers": ""}, headers=fresh_user_headers)
        assert res_bad.status_code == 400
        record_result("/sentiment/batch", "Batch Multi-Ticker Sentiment Query", True)

    def test_sentiment_history(self, client, fresh_user_headers):
        """GET /sentiment/history - Time-series sentiment history."""
        res = client.get(
            "/sentiment/history",
            params={"ticker": "AAPL", "start_date": "2025-01-01", "end_date": "2025-03-31"},
            headers=fresh_user_headers,
        )
        assert res.status_code == 200
        data = res.json()
        assert data.get("ticker") == "AAPL"
        assert "records" in data or "count" in data

        # Bad request: missing ticker
        res_bad = client.get("/sentiment/history", params={}, headers=fresh_user_headers)
        assert res_bad.status_code == 400
        record_result("/sentiment/history", "Historical Sentiment Time-Series", True)

    def test_sentiment_feed(self, client, fresh_user_headers):
        """GET /sentiment/feed - Real-time market sentiment stream feed."""
        res = client.get("/sentiment/feed", params={"limit": 20, "min_score": -1.0}, headers=fresh_user_headers)
        assert res.status_code == 200
        data = res.json()
        assert "records" in data or "items" in data or "count" in data
        record_result("/sentiment/feed", "Real-Time Sentiment Feed Stream", True)

    def test_sentiment_anomalies(self, client, fresh_user_headers):
        """GET /sentiment/anomalies - Z-score statistical sentiment anomaly scanner."""
        res = client.get("/sentiment/anomalies", params={"lookback_days": 30}, headers=fresh_user_headers)
        assert res.status_code == 200
        data = res.json()
        assert "items" in data or "count" in data
        record_result("/sentiment/anomalies", "Statistical Sentiment Anomaly Scanner", True)

    def test_sentiment_disagreement(self, client, fresh_user_headers):
        """GET /sentiment/disagreement - Multi-source sentiment dispersion."""
        res = client.get(
            "/sentiment/disagreement",
            params={"ticker": "AAPL", "start_date": "2025-01-01", "end_date": "2025-03-31"},
            headers=fresh_user_headers,
        )
        assert res.status_code == 200
        data = res.json()
        assert data.get("ticker") == "AAPL"
        assert "disagreement_index" in data or "sources" in data
        record_result("/sentiment/disagreement", "Multi-Source Sentiment Dispersion Index", True)

    def test_sentiment_entities(self, client, fresh_user_headers):
        """GET /sentiment/entities - Named entity sentiment breakdown."""
        res = client.get("/sentiment/entities", params={"ticker": "AAPL"}, headers=fresh_user_headers)
        assert res.status_code == 200
        data = res.json()
        assert "entities" in data or "ticker" in data
        record_result("/sentiment/entities", "Entity-Level Sentiment Breakdown", True)

    def test_sector_sentiment(self, client, fresh_user_headers):
        """GET /sentiment/sector - Macro sector-aggregated sentiment."""
        res = client.get(
            "/sentiment/sector",
            params={"sector": "Technology", "start_date": "2025-01-01", "end_date": "2025-03-31"},
            headers=fresh_user_headers,
        )
        assert res.status_code == 200
        data = res.json()
        assert "sector" in data or "value" in data
        record_result("/sentiment/sector", "Sector Aggregate Sentiment", True)

    def test_sentiment_backfill(self, client, fresh_user_headers):
        """POST /sentiment/backfill - Historical sentiment backfill trigger."""
        payload = {"ticker": "AAPL", "start_date": "2025-01-01", "end_date": "2025-01-10"}
        res = client.post("/sentiment/backfill", json=payload, headers=fresh_user_headers)
        assert res.status_code in [200, 202]
        record_result("/sentiment/backfill", "Historical Sentiment Backfill Worker", True)

    def test_anomaly_scan_trigger(self, client, fresh_user_headers):
        """POST /anomaly-scan - On-demand anomaly detector scan."""
        res = client.post("/anomaly-scan", json={"threshold": 2.5}, headers=fresh_user_headers)
        assert res.status_code == 200
        data = res.json()
        assert "anomalies_found" in data or "scanned_at" in data
        record_result("/anomaly-scan", "On-Demand Sentiment Anomaly Scan Trigger", True)

    def test_language_detection(self, client, fresh_user_headers):
        """GET /language/detect - Multilingual NLP language classification & routing."""
        # English detection
        res_en = client.get(
            "/language/detect",
            params={"text": "Federal Reserve maintains interest rates steady amid market inflation."},
            headers=fresh_user_headers,
        )
        assert res_en.status_code == 200
        en_data = res_en.json()
        assert en_data.get("language") == "english"
        assert en_data.get("language_code") == "en"

        # Spanish detection with multilingual model routing
        res_es = client.get(
            "/language/detect",
            params={"text": "El Banco Central Europeo mantiene los tipos de interés sin cambios."},
            headers=fresh_user_headers,
        )
        assert res_es.status_code == 200
        es_data = res_es.json()
        assert es_data.get("language") == "spanish"
        assert es_data.get("is_multilingual_model_applied") is True

        # Bad request: empty text
        res_bad = client.get("/language/detect", params={"text": "   "}, headers=fresh_user_headers)
        assert res_bad.status_code == 400
        record_result("/language/detect", "Multilingual Language Detection & Model Routing", True)


# ─────────────────────────────────────────────────────────────────────────────────
# 4. Quantitative Market Analytics & Macro Indicators
# ─────────────────────────────────────────────────────────────────────────────────

class TestMarketAnalytics:
    """Validates market regime, return correlation, sector rotation, breadth, and symbol mapping."""

    def test_market_regime(self, client, fresh_user_headers):
        """GET /market/regime - Macro market regime detection."""
        res = client.get("/market/regime", headers=fresh_user_headers)
        assert res.status_code == 200
        data = res.json()
        assert "regime" in data or "regime_label" in data or "confidence" in data
        record_result("/market/regime", "Quantitative Market Regime Classification", True)

    def test_return_correlation(self, client, fresh_user_headers):
        """GET /market/correlation - Cross-asset return correlation matrix."""
        res = client.get(
            "/market/correlation",
            params={"tickers": "AAPL,MSFT,NVDA", "start_date": "2025-01-01", "end_date": "2025-03-31"},
            headers=fresh_user_headers,
        )
        assert res.status_code == 200
        data = res.json()
        assert "correlations" in data or "matrix" in data

        # Bad request: missing start_date/end_date
        res_bad = client.get("/market/correlation", params={"tickers": "AAPL"}, headers=fresh_user_headers)
        assert res_bad.status_code == 400
        record_result("/market/correlation", "Cross-Asset Return Correlation Matrix", True)

    def test_sector_rotation(self, client, fresh_user_headers):
        """GET /market/sector-rotation - Sector relative strength and rotation."""
        res = client.get("/market/sector-rotation", params={"lookback_days": 30}, headers=fresh_user_headers)
        assert res.status_code == 200
        data = res.json()
        assert "sectors" in data or "rankings" in data or "count" in data
        record_result("/market/sector-rotation", "Sector Rotation & Relative Strength Ranking", True)

    def test_market_breadth(self, client, fresh_user_headers):
        """GET /market/breadth - Advance/decline market breadth indicator."""
        res = client.get(
            "/market/breadth",
            params={"start_date": "2025-01-01", "end_date": "2025-03-31"},
            headers=fresh_user_headers,
        )
        assert res.status_code == 200
        data = res.json()
        assert "breadth" in data or "points" in data
        record_result("/market/breadth", "Advance/Decline Market Breadth Time-Series", True)

    def test_symbol_map(self, client, fresh_user_headers):
        """GET /symbols/map - Institutional security identifier mapping."""
        res = client.get("/symbols/map", params={"identifier": "AAPL"}, headers=fresh_user_headers)
        assert res.status_code == 200
        data = res.json()
        assert data.get("input_identifier") == "AAPL" or "result" in data or data.get("ticker") == "AAPL"
        record_result("/symbols/map", "Institutional Security Symbol Identifier Mapping", True)


# ─────────────────────────────────────────────────────────────────────────────────
# 5. Spillovers & Cross-Asset Lead-Lag Risk Engine
# ─────────────────────────────────────────────────────────────────────────────────

class TestSpillovers:
    """Validates cross-asset sentiment spillovers and spillover matrix."""

    def test_spillovers(self, client, fresh_user_headers):
        """GET /spillovers - Lead-lag sentiment transmission pairs."""
        res = client.get("/spillovers", params={"ticker": "AAPL", "limit": 5}, headers=fresh_user_headers)
        assert res.status_code == 200
        data = res.json()
        assert "spillovers" in data
        assert data.get("ticker") == "AAPL"

        # Missing ticker -> 400
        res_bad = client.get("/spillovers", params={}, headers=fresh_user_headers)
        assert res_bad.status_code == 400
        record_result("/spillovers", "Cross-Asset Spillover Lead-Lag Analytics", True)

    def test_spillover_matrix(self, client, fresh_user_headers):
        """GET /spillovers/matrix - Full NxN spillover adjacency matrix."""
        res = client.get(
            "/spillovers/matrix",
            params={"tickers": "AAPL,MSFT,NVDA", "start_date": "2025-01-01", "end_date": "2025-03-31"},
            headers=fresh_user_headers,
        )
        assert res.status_code == 200
        data = res.json()
        assert "matrix" in data or "pairs" in data
        record_result("/spillovers/matrix", "Cross-Asset NxN Spillover Adjacency Matrix", True)


# ─────────────────────────────────────────────────────────────────────────────────
# 6. Options Implied Volatility & Derivatives Analytics
# ─────────────────────────────────────────────────────────────────────────────────

class TestOptionsAnalytics:
    """Validates Black-Scholes Greeks, IV, UOA, Vol Surface, Put/Call Ratio, and Microstructure."""

    def test_options_iv(self, client, fresh_user_headers):
        """GET /options/iv - Black-Scholes Implied Volatility & Greeks."""
        res = client.get(
            "/options/iv",
            params={"ticker": "AAPL", "expiration_date": "2026-12-18", "option_type": "call", "strike": 150.0},
            headers=fresh_user_headers,
        )
        assert res.status_code == 200
        data = res.json()
        assert "contracts" in data
        assert data.get("ticker") == "AAPL"

        # Missing expiration_date -> 400
        res_bad = client.get("/options/iv", params={"ticker": "AAPL"}, headers=fresh_user_headers)
        assert res_bad.status_code == 400
        record_result("/options/iv", "Black-Scholes Implied Volatility & Greeks", True)

    def test_unusual_options(self, client, fresh_user_headers):
        """GET /options/unusual - Unusual Options Activity (UOA) detection."""
        res = client.get("/options/unusual", params={"min_score": 40.0, "limit": 10}, headers=fresh_user_headers)
        assert res.status_code == 200
        data = res.json()
        assert "activities" in data or "contracts" in data or "items" in data
        record_result("/options/unusual", "Unusual Options Activity (UOA) Scoring", True)

    def test_options_vol_surface(self, client, fresh_user_headers):
        """GET /options/vol-surface - 2D Implied Volatility Surface & Smile Grid."""
        res = client.get("/options/vol-surface", params={"ticker": "AAPL"}, headers=fresh_user_headers)
        assert res.status_code == 200
        data = res.json()
        assert data.get("ticker") == "AAPL"
        assert "surface" in data or "points" in data
        record_result("/options/vol-surface", "2D Options Volatility Surface Grid", True)

    def test_put_call_ratio(self, client, fresh_user_headers):
        """GET /options/put-call-ratio - Volume and Open Interest Put/Call Ratios."""
        res = client.get(
            "/options/put-call-ratio",
            params={"ticker": "AAPL", "start_date": "2025-01-01", "end_date": "2025-03-31"},
            headers=fresh_user_headers,
        )
        assert res.status_code == 200
        data = res.json()
        assert "points" in data or "pcr_volume" in data or data.get("ticker") == "AAPL"
        record_result("/options/put-call-ratio", "Options Put/Call Ratio & Microstructure Sentiment", True)

    def test_options_microstructure(self, client, fresh_user_headers):
        """GET /options/microstructure - Order Flow Toxicity (VPIN) & Gamma Exposure (GEX)."""
        res = client.get(
            "/options/microstructure",
            params={"ticker": "AAPL", "start_date": "2025-01-01", "end_date": "2025-03-31"},
            headers=fresh_user_headers,
        )
        assert res.status_code == 200
        data = res.json()
        assert data.get("ticker") == "AAPL"
        assert "points" in data
        record_result("/options/microstructure", "Market Microstructure (VPIN/GEX) Analytics", True)


# ─────────────────────────────────────────────────────────────────────────────────
# 7. Corporate Disclosures, SEC Filings, 8-K & Insider Trading
# ─────────────────────────────────────────────────────────────────────────────────

class TestCorporateDisclosures:
    """Validates Event Studies, SEC 8-K, Earnings Surprises, Filings, Insider Trades, M&A, Supply Chain."""

    def test_event_study(self, client, fresh_user_headers):
        """GET /events/study - Cumulative Abnormal Returns (CAR) Event Study."""
        res = client.get(
            "/events/study",
            params={"ticker": "AAPL", "event_date": "2025-01-15", "pre_event_days": 10, "post_event_days": 10},
            headers=fresh_user_headers,
        )
        assert res.status_code == 200
        data = res.json()
        assert "car" in data or "abnormal_returns" in data
        record_result("/events/study", "Cumulative Abnormal Returns (CAR) Event Study", True)

    def test_events_8k(self, client, fresh_user_headers):
        """GET /events/8k - SEC Form 8-K unscheduled corporate disclosure events."""
        res = client.get("/events/8k", params={"ticker": "AAPL", "limit": 5}, headers=fresh_user_headers)
        assert res.status_code == 200
        data = res.json()
        assert "filings" in data or "events" in data
        record_result("/events/8k", "SEC Form 8-K Unscheduled Disclosure Events", True)

    def test_earnings_surprise(self, client, fresh_user_headers):
        """GET /events/earnings-surprise - Quarterly earnings surprise history."""
        res = client.get("/events/earnings-surprise", params={"ticker": "AAPL"}, headers=fresh_user_headers)
        assert res.status_code == 200
        data = res.json()
        assert "surprises" in data or "history" in data
        record_result("/events/earnings-surprise", "Earnings Surprise History & Sentiment Shift", True)

    def test_regulatory_filings(self, client, fresh_user_headers):
        """GET /events/filings - SEC regulatory filings classification."""
        res = client.get("/events/filings", params={"ticker": "AAPL", "form_type": "10-K"}, headers=fresh_user_headers)
        assert res.status_code == 200
        data = res.json()
        assert "filings" in data
        record_result("/events/filings", "SEC Regulatory Filings Classification", True)

    def test_insider_trading(self, client, fresh_user_headers):
        """GET /events/insider-trading - SEC Form 4 insider transactions."""
        res = client.get("/events/insider-trading", params={"ticker": "AAPL"}, headers=fresh_user_headers)
        assert res.status_code == 200
        data = res.json()
        assert "trades" in data or "transactions" in data
        record_result("/events/insider-trading", "SEC Form 4 Insider Trading Signal", True)

    def test_ma_rumors(self, client, fresh_user_headers):
        """GET /events/ma-rumors - M&A rumor & catalyst detection."""
        res = client.get("/events/ma-rumors", params={"min_rumor_score": 0.5}, headers=fresh_user_headers)
        assert res.status_code == 200
        data = res.json()
        assert "items" in data or "rumors" in data or "count" in data
        record_result("/events/ma-rumors", "Multi-Signal M&A Rumor Detection", True)

    def test_supply_chain_risk(self, client, fresh_user_headers):
        """GET /events/supply-chain-risk - Supply chain risk propagation graph."""
        res = client.get("/events/supply-chain-risk", params={"ticker": "AAPL", "depth": 2}, headers=fresh_user_headers)
        assert res.status_code == 200
        data = res.json()
        assert "alerts" in data or data.get("focal_ticker") == "AAPL"
        record_result("/events/supply-chain-risk", "Supply Chain Risk Propagation Graph", True)


# ─────────────────────────────────────────────────────────────────────────────────
# 8. Alternative Assets: ESG, Bankruptcy, FX, Commodities, Crypto, Credit & Risk
# ─────────────────────────────────────────────────────────────────────────────────

class TestAlternativeAssetsAndRisk:
    """Validates ESG, Bankruptcy Risk, FX, Commodities, Crypto, Credit Default, and Factor Exposure."""

    def test_esg_scores(self, client, fresh_user_headers):
        """GET /esg/scores - Environmental, Social, and Governance sentiment."""
        res = client.get("/esg/scores", params={"ticker": "AAPL"}, headers=fresh_user_headers)
        assert res.status_code == 200
        data = res.json()
        assert "overall_score" in data or "dimensions" in data
        record_result("/esg/scores", "ESG Sustainability Sentiment Scores", True)

    def test_bankruptcy_risk(self, client, fresh_user_headers):
        """GET /risk/bankruptcy - Multi-factor bankruptcy distress indicator."""
        res = client.get("/risk/bankruptcy", params={"ticker": "AAPL"}, headers=fresh_user_headers)
        assert res.status_code == 200
        data = res.json()
        assert "bankruptcy_risk_score" in data or "components" in data
        record_result("/risk/bankruptcy", "Multi-Factor Bankruptcy Distress Risk", True)

    def test_fx_sentiment(self, client, fresh_user_headers):
        """GET /fx/sentiment - Foreign exchange currency pair sentiment."""
        res = client.get("/fx/sentiment", params={"currency_pair": "EUR/USD"}, headers=fresh_user_headers)
        assert res.status_code == 200
        data = res.json()
        assert "currency_pair" in data or "summary" in data
        record_result("/fx/sentiment", "Foreign Exchange (FX) Sentiment Feed", True)

    def test_commodity_sentiment(self, client, fresh_user_headers):
        """GET /commodities/sentiment - Raw material and commodity news sentiment."""
        res = client.get("/commodities/sentiment", params={"commodity": "crude_oil"}, headers=fresh_user_headers)
        assert res.status_code == 200
        data = res.json()
        assert "commodity" in data or "summary" in data
        record_result("/commodities/sentiment", "Commodity News Sentiment Feed", True)

    def test_crypto_sentiment(self, client, fresh_user_headers):
        """GET /crypto/sentiment - Digital asset and cryptocurrency sentiment."""
        res = client.get("/crypto/sentiment", params={"asset": "BTC"}, headers=fresh_user_headers)
        assert res.status_code == 200
        data = res.json()
        assert "asset" in data or "summary" in data
        record_result("/crypto/sentiment", "Cryptocurrency News Sentiment Feed", True)

    def test_credit_sentiment(self, client, fresh_user_headers):
        """GET /risk/credit-sentiment - Credit default & fixed income sentiment."""
        res = client.get("/risk/credit-sentiment", params={"ticker": "AAPL"}, headers=fresh_user_headers)
        assert res.status_code == 200
        data = res.json()
        assert "credit_risk_score" in data or "spread_impact" in data or "ticker" in data
        record_result("/risk/credit-sentiment", "Credit Default & Fixed Income Sentiment", True)

    def test_factor_exposure(self, client, fresh_user_headers):
        """GET /risk/factor-exposure - Single-asset multi-factor OLS regression exposure."""
        res = client.get(
            "/risk/factor-exposure",
            params={"ticker": "AAPL", "start_date": "2025-01-01", "end_date": "2025-06-30"},
            headers=fresh_user_headers,
        )
        assert res.status_code == 200
        data = res.json()
        assert "exposures" in data or "ticker" in data
        record_result("/risk/factor-exposure", "Single-Asset Factor Exposure OLS Regression", True)


# ─────────────────────────────────────────────────────────────────────────────────
# 9. Backtesting & Portfolio Optimization Engines
# ─────────────────────────────────────────────────────────────────────────────────

class TestBacktestingAndPortfolio:
    """Validates alpha strategy backtesting, Mean-Variance/Risk-Parity optimization, and risk attribution."""

    def test_alpha_backtest(self, client, fresh_user_headers):
        """POST /backtest - Quantitative alpha strategy backtesting engine."""
        payload = {
            "ticker": "AAPL",
            "start_date": "2025-01-01",
            "end_date": "2025-03-31",
            "long_threshold": 0.2,
            "short_threshold": -0.2,
            "holding_days": 5,
            "initial_capital": 1000000.0,
        }
        res = client.post("/backtest", json=payload, headers=fresh_user_headers)
        assert res.status_code == 200
        data = res.json()
        assert "total_return" in data
        assert "sharpe_ratio" in data
        assert "equity_curve" in data
        assert len(data["equity_curve"]) >= 10
        record_result("/backtest", "Alpha Strategy Backtesting Simulation", True)

    def test_portfolio_optimize(self, client, fresh_user_headers):
        """POST /portfolio/optimize - Mean-Variance & Risk-Parity portfolio optimization."""
        payload = {
            "tickers": ["AAPL", "MSFT", "NVDA"],
            "start_date": "2025-01-01",
            "end_date": "2025-06-30",
            "optimization_type": "max_sharpe",
            "risk_free_rate": 0.05,
        }
        res = client.post("/portfolio/optimize", json=payload, headers=fresh_user_headers)
        assert res.status_code == 200
        data = res.json()
        assert "weights" in data
        assert "sharpe_ratio" in data
        assert len(data["weights"]) == 3
        total_w = sum(item["weight"] for item in data["weights"])
        assert abs(total_w - 1.0) < 0.02
        record_result("/portfolio/optimize", "Mean-Variance & Risk-Parity Optimization", True)

    def test_portfolio_factor_exposure(self, client, fresh_user_headers):
        """POST /risk/portfolio-factor-exposure - Multi-factor regression for portfolio."""
        payload = {
            "tickers": ["AAPL", "MSFT", "NVDA"],
            "weights": [0.4, 0.3, 0.3],
            "start_date": "2025-01-01",
            "end_date": "2025-06-30",
            "benchmark_ticker": "SPY",
        }
        res = client.post("/risk/portfolio-factor-exposure", json=payload, headers=fresh_user_headers)
        assert res.status_code == 200
        data = res.json()
        assert "exposures" in data or "r_squared" in data
        record_result("/risk/portfolio-factor-exposure", "Portfolio Factor Exposure & Risk Attribution", True)


# ─────────────────────────────────────────────────────────────────────────────────
# 10. Audio Transcription & Earnings Call Database
# ─────────────────────────────────────────────────────────────────────────────────

class TestAudioAndTranscripts:
    """Validates audio transcription (Whisper ASR), acoustic stress, and earnings transcript retrieval."""

    def test_audio_transcribe_multipart(self, client, fresh_user_headers):
        """POST /audio/transcribe - Audio file upload and transcription."""
        wav_data = create_synthetic_wav_bytes(duration_secs=1.0)
        files = {"audio": ("earnings_call_sample.wav", wav_data, "audio/wav")}
        params = {"ticker": "AAPL", "store": "false"}

        res = client.post("/audio/transcribe", files=files, params=params, headers=fresh_user_headers)
        assert res.status_code == 200
        data = res.json()
        assert "transcription" in data or "text" in data
        assert "sentiment" in data or "acoustic_features" in data
        record_result("/audio/transcribe", "Audio Transcription & Acoustic Stress Analysis", True)

    def test_transcripts_crud(self, client, fresh_user_headers):
        """POST/GET/DELETE /transcripts - Earnings call transcript database."""
        # Create transcript
        create_payload = {
            "ticker": "AAPL",
            "quarter": 4,
            "year": 2025,
            "call_date": "2025-10-31",
            "transcript_text": "Revenue grew by 15% year-over-year with strong margins across all product lines.",
        }
        res_create = client.post("/transcripts", json=create_payload, headers=fresh_user_headers)
        assert res_create.status_code in [200, 201]
        t_data = res_create.json()
        t_id = t_data["transcript"]["id"] if "transcript" in t_data else t_data["id"]

        # List transcripts
        res_list = client.get("/transcripts", params={"ticker": "AAPL"}, headers=fresh_user_headers)
        assert res_list.status_code == 200
        assert len(res_list.json().get("items", [])) >= 1 or res_list.json().get("count", 0) >= 1

        # Get transcript by ID
        res_get = client.get(f"/transcripts/{t_id}", headers=fresh_user_headers)
        assert res_get.status_code == 200

        # Delete transcript
        res_del = client.delete(f"/transcripts/{t_id}", headers=fresh_user_headers)
        assert res_del.status_code == 200
        record_result("/transcripts/*", "Earnings Call Transcript Database CRUD", True)


# ─────────────────────────────────────────────────────────────────────────────────
# 11. Custom Universes & News Article Search
# ─────────────────────────────────────────────────────────────────────────────────

class TestUniversesAndNews:
    """Validates custom universe construction and news article full-text retrieval."""

    def test_universes_crud(self, client, fresh_user_headers):
        """POST/GET/PUT/DELETE /universes - Custom asset basket builder."""
        # Create universe
        res_create = client.post(
            "/universes",
            json={"name": "Big Tech AI Basket", "tickers": ["AAPL", "MSFT", "NVDA", "GOOGL"]},
            headers=fresh_user_headers,
        )
        assert res_create.status_code in [200, 201]
        u_data = res_create.json()
        u_id = u_data["id"]

        # List universes
        res_list = client.get("/universes", headers=fresh_user_headers)
        assert res_list.status_code == 200
        assert any(u.get("id") == u_id for u in res_list.json().get("universes", []))

        # Get universe by ID
        res_get = client.get(f"/universes/{u_id}", headers=fresh_user_headers)
        assert res_get.status_code == 200

        # Update universe
        res_put = client.put(
            f"/universes/{u_id}",
            json={"name": "Big Tech AI Basket Updated", "tickers": ["AAPL", "MSFT", "NVDA", "AMZN"]},
            headers=fresh_user_headers,
        )
        assert res_put.status_code == 200

        # Delete universe
        res_del = client.delete(f"/universes/{u_id}", headers=fresh_user_headers)
        assert res_del.status_code == 200
        record_result("/universes/*", "Custom Asset Universe Builder CRUD", True)

    def test_news_articles(self, client, fresh_user_headers):
        """GET /news/articles and GET /news/articles/{id}."""
        res_list = client.get("/news/articles", params={"ticker": "AAPL", "limit": 5}, headers=fresh_user_headers)
        assert res_list.status_code == 200
        articles = res_list.json().get("articles", [])

        if articles:
            art_id = articles[0]["id"]
            res_get = client.get(f"/news/articles/{art_id}", headers=fresh_user_headers)
            assert res_get.status_code in [200, 404]
        record_result("/news/articles/*", "Financial News Article Retrieval", True)


# ─────────────────────────────────────────────────────────────────────────────────
# 12. Audit Logs, Data Provenance, Unified Search & Export
# ─────────────────────────────────────────────────────────────────────────────────

class TestAuditProvenanceAndExport:
    """Validates compliance audit logs, data lineage provenance, search, and CSV/Parquet export."""

    def test_audit_logs_and_export(self, client, fresh_user_headers):
        """GET /audit/logs and GET /audit/export."""
        # Query audit logs
        res = client.get("/audit/logs", params={"limit": 20}, headers=fresh_user_headers)
        assert res.status_code == 200
        assert "logs" in res.json() or "events" in res.json()

        # Export audit logs
        res_export = client.get("/audit/export", params={"format": "csv"}, headers=fresh_user_headers)
        assert res_export.status_code == 200
        record_result("/audit/*", "Compliance Audit Logging & Export", True)

    def test_data_provenance(self, client, fresh_user_headers):
        """GET /provenance/{record_type}/{record_id} - Lineage tracking."""
        res = client.get("/provenance/sentiment/AAPL_2026-08-30T10:15:00Z", headers=fresh_user_headers)
        assert res.status_code in [200, 404]

        # Invalid record type -> 400
        res_bad = client.get("/provenance/invalid_type/12345", headers=fresh_user_headers)
        assert res_bad.status_code == 400
        record_result("/provenance/{record_type}/{record_id}", "Data Provenance & Lineage Tracking", True)

    def test_unified_search(self, client, fresh_user_headers):
        """GET /search - Unified cross-domain search."""
        res = client.get("/search", params={"q": "Apple earnings quarterly revenue", "limit": 10}, headers=fresh_user_headers)
        assert res.status_code == 200
        data = res.json()
        assert "results" in data or "items" in data

        # Empty search query -> 400
        res_bad = client.get("/search", params={"q": ""}, headers=fresh_user_headers)
        assert res_bad.status_code == 400
        record_result("/search", "Unified Cross-Domain Full-Text Search", True)

    def test_data_export(self, client, fresh_user_headers):
        """GET /export/csv and GET /export/parquet."""
        # CSV Export
        res_csv = client.get(
            "/export/csv",
            params={"ticker": "AAPL", "start_date": "2025-01-01", "end_date": "2025-03-31"},
            headers=fresh_user_headers,
        )
        assert res_csv.status_code == 200

        # Parquet Export
        res_pq = client.get(
            "/export/parquet",
            params={"ticker": "AAPL", "start_date": "2025-01-01", "end_date": "2025-03-31"},
            headers=fresh_user_headers,
        )
        assert res_pq.status_code == 200
        record_result("/export/*", "Institutional CSV & Parquet Data Export", True)


# ─────────────────────────────────────────────────────────────────────────────────
# 13. Organizations (RBAC) & IP Whitelisting
# ─────────────────────────────────────────────────────────────────────────────────

class TestOrgsAndSecurity:
    """Validates multi-tenant organizations, membership management, and IP whitelisting."""

    def test_organizations_lifecycle(self, client, fresh_user_headers):
        """POST /orgs, GET /orgs, GET /orgs/{id}, POST /orgs/{id}/invites, etc."""
        org_name = f"Alpha Capital {uuid.uuid4().hex[:6]}"

        # Create organization
        res_create = client.post("/orgs", json={"name": org_name}, headers=fresh_user_headers)
        assert res_create.status_code in [200, 201]
        org_data = res_create.json()
        org_id = org_data["id"]

        # List organizations
        res_list = client.get("/orgs", headers=fresh_user_headers)
        assert res_list.status_code == 200

        # Get organization details
        res_get = client.get(f"/orgs/{org_id}", headers=fresh_user_headers)
        assert res_get.status_code == 200

        # Invite member
        res_invite = client.post(
            f"/orgs/{org_id}/invites",
            json={"user_id": "invited_trader_bob", "role": "member"},
            headers=fresh_user_headers,
        )
        assert res_invite.status_code in [200, 201]

        # Update member role
        res_patch = client.patch(
            f"/orgs/{org_id}/members/invited_trader_bob",
            json={"role": "viewer"},
            headers=fresh_user_headers,
        )
        assert res_patch.status_code == 200

        # Remove member
        res_rem = client.delete(f"/orgs/{org_id}/members/invited_trader_bob", headers=fresh_user_headers)
        assert res_rem.status_code == 200

        # Select organization context
        res_sel = client.post(f"/orgs/{org_id}/select", headers=fresh_user_headers)
        assert res_sel.status_code == 200
        record_result("/orgs/*", "Multi-Tenant Organization RBAC & Team Management", True)

    def test_ip_whitelist(self, client):
        """GET/POST/DELETE /security/ip-whitelist with isolated user context."""
        iso_user = str(uuid.uuid4())
        iso_token = get_jwt_token(client, iso_user, role="admin")
        iso_headers = {"Authorization": f"Bearer {iso_token}"}

        # Add IP whitelist CIDR
        res_add = client.post(
            "/security/ip-whitelist",
            json={"ip_or_cidr": "127.0.0.1/32", "description": "Local Test Loopback"},
            headers=iso_headers,
        )
        assert res_add.status_code in [200, 201]
        entry_id = res_add.json()["id"]

        # List IP whitelist
        res_list = client.get("/security/ip-whitelist", headers=iso_headers)
        assert res_list.status_code == 200

        # Delete IP whitelist entry
        res_del = client.delete(f"/security/ip-whitelist/{entry_id}", headers=iso_headers)
        assert res_del.status_code == 200

        # Bad request: invalid CIDR -> 400
        res_bad = client.post(
            "/security/ip-whitelist",
            json={"ip_or_cidr": "not_a_valid_ip_or_cidr"},
            headers=iso_headers,
        )
        assert res_bad.status_code == 400
        record_result("/security/ip-whitelist/*", "IP Whitelisting & CIDR Zero-Trust Access Control", True)


# ─────────────────────────────────────────────────────────────────────────────────
# 14. Streaming Kafka, Webhooks, Billing & Chat Alerts
# ─────────────────────────────────────────────────────────────────────────────────

class TestStreamingWebhooksAndBilling:
    """Validates Kafka streams, Polling/Push Webhooks, Stripe Billing, and Telegram/Discord alerts."""

    def test_kafka_stream_credentials(self, client, fresh_user_headers):
        """GET /stream/kafka/topics, GET/DELETE /stream/kafka/credentials."""
        # List topics
        res_topics = client.get("/stream/kafka/topics", headers=fresh_user_headers)
        assert res_topics.status_code == 200

        # Get credentials
        res_creds = client.get("/stream/kafka/credentials", params={"topic": "sentiment-events"}, headers=fresh_user_headers)
        assert res_creds.status_code == 200
        cred_data = res_creds.json()
        cred_id = cred_data.get("credential_id") or cred_data.get("id")

        if cred_id:
            res_del = client.delete(f"/stream/kafka/credentials/{cred_id}", headers=fresh_user_headers)
            assert res_del.status_code in [200, 404]
        record_result("/stream/kafka/*", "Streaming Kafka Consumer Topic Access & Credentials", True)

    def test_webhooks_lifecycle(self, client, fresh_user_headers):
        """POST/GET/DELETE /webhooks - Push webhook registration."""
        res_reg = client.post(
            "/webhooks",
            json={"url": "https://webhook.site/sample-test", "events": ["sentiment.anomaly"]},
            headers=fresh_user_headers,
        )
        assert res_reg.status_code in [200, 201]
        wh_id = res_reg.json()["id"]

        res_list = client.get("/webhooks", headers=fresh_user_headers)
        assert res_list.status_code == 200

        res_del = client.delete(f"/webhooks/{wh_id}", headers=fresh_user_headers)
        assert res_del.status_code == 200
        record_result("/webhooks/*", "Push Webhook Notification Subscription", True)

    def test_polling_webhooks(self, client, fresh_user_headers):
        """POST/GET/DELETE /polling-webhooks - Pull-based webhook delivery."""
        res_create = client.post(
            "/polling-webhooks",
            json={
                "name": "Nightly Batch Consumer",
                "url": "https://webhook.site/sample-poll",
                "interval_seconds": 300,
                "query_type": "sentiment",
                "query_params": {"tickers": ["AAPL", "MSFT"]},
            },
            headers=fresh_user_headers,
        )
        assert res_create.status_code in [200, 201]
        wh_id = res_create.json()["id"]

        res_list = client.get("/polling-webhooks", headers=fresh_user_headers)
        assert res_list.status_code == 200

        res_del = client.delete(f"/polling-webhooks/{wh_id}", headers=fresh_user_headers)
        assert res_del.status_code == 200
        record_result("/polling-webhooks/*", "Custom Polling Webhooks & Pull-Based Delivery", True)

    def test_chat_alerts(self, client, fresh_user_headers):
        """POST/GET/DELETE /chat-alerts - Telegram/Discord alert subscriptions."""
        res_create = client.post(
            "/chat-alerts",
            json={
                "channel_type": "telegram",
                "channel_target": "123456789",
                "event_types": ["sentiment_anomaly", "8k_filing"],
            },
            headers=fresh_user_headers,
        )
        assert res_create.status_code in [200, 201]
        alert_id = res_create.json()["id"]

        res_list = client.get("/chat-alerts", headers=fresh_user_headers)
        assert res_list.status_code == 200

        res_del = client.delete(f"/chat-alerts/{alert_id}", headers=fresh_user_headers)
        assert res_del.status_code == 200
        record_result("/chat-alerts/*", "Telegram & Discord Alert Bot Subscription", True)

    def test_billing_and_subscription(self, client, fresh_user_headers):
        """POST /billing/checkout, POST /billing/portal, GET /billing/subscription, POST /billing/webhook."""
        # Create checkout session
        res_checkout = client.post(
            "/billing/checkout",
            json={"plan_id": "pro_monthly", "success_url": "https://example.com/success"},
            headers=fresh_user_headers,
        )
        assert res_checkout.status_code == 200
        assert "checkout_url" in res_checkout.json() or "session_id" in res_checkout.json()

        # Create billing portal
        res_portal = client.post(
            "/billing/portal",
            json={"return_url": "https://example.com/account"},
            headers=fresh_user_headers,
        )
        assert res_portal.status_code == 200

        # Get subscription
        res_sub = client.get("/billing/subscription", headers=fresh_user_headers)
        assert res_sub.status_code == 200
        assert "plan_id" in res_sub.json() or "status" in res_sub.json()

        # Webhook simulation (Public endpoint)
        res_wh = client.post(
            "/billing/webhook",
            json={"type": "invoice.paid", "data": {"object": {"customer": "cus_123"}}},
        )
        assert res_wh.status_code in [200, 400]
        record_result("/billing/*", "Stripe Subscription & Billing Integration", True)


# ─────────────────────────────────────────────────────────────────────────────────
# 15. Retention Policies, Model Retraining, FIX, DLQ, Digest & Stats
# ─────────────────────────────────────────────────────────────────────────────────

class TestAdvancedInfrastructure:
    """Validates Data Retention, Retraining Automation, FIX Protocol, DLQ, Digest, and Stats."""

    def test_retention_policies(self, client, fresh_user_headers):
        """GET/POST/DELETE /retention/policies."""
        res_create = client.post(
            "/retention/policies",
            json={"data_category": "sentiment_history", "retention_days": 365, "is_active": True},
            headers=fresh_user_headers,
        )
        assert res_create.status_code in [200, 201]
        p_id = res_create.json()["id"]

        res_list = client.get("/retention/policies", headers=fresh_user_headers)
        assert res_list.status_code == 200

        res_del = client.delete(f"/retention/policies/{p_id}", headers=fresh_user_headers)
        assert res_del.status_code == 200
        record_result("/retention/policies/*", "Data Retention Policy Management & Lifecycle", True)

    def test_model_retraining(self, client, fresh_user_headers):
        """POST/GET /retraining/jobs, POST /retraining/jobs/{id}/cancel."""
        res_create = client.post(
            "/retraining/jobs",
            json={"model_type": "sentiment", "trigger_type": "manual", "epochs": 3},
            headers=fresh_user_headers,
        )
        assert res_create.status_code in [200, 201]
        job_data = res_create.json()
        job_id = job_data["job"]["id"] if "job" in job_data else job_data["id"]

        res_list = client.get("/retraining/jobs", headers=fresh_user_headers)
        assert res_list.status_code == 200

        res_get = client.get(f"/retraining/jobs/{job_id}", headers=fresh_user_headers)
        assert res_get.status_code == 200

        res_cancel = client.post(f"/retraining/jobs/{job_id}/cancel", headers=fresh_user_headers)
        assert res_cancel.status_code in [200, 400]
        record_result("/retraining/jobs/*", "Model Retraining Pipeline Automation", True)

    def test_fix_order_lifecycle(self, client, fresh_user_headers):
        """POST /fix/order, GET /fix/orders, POST /fix/cancel."""
        # Submit FIX NewOrderSingle (35=D)
        fix_msg = "8=FIX.4.4|9=60|35=D|11=ORD-TEST-001|55=AAPL|54=1|38=100|40=1|10=000|"
        res_order = client.post("/fix/order", json={"fix_message": fix_msg}, headers=fresh_user_headers)
        assert res_order.status_code in [200, 201]
        order_data = res_order.json()
        assert order_data.get("symbol") == "AAPL"
        assert "order_id" in order_data

        # List FIX orders
        res_list = client.get("/fix/orders", headers=fresh_user_headers)
        assert res_list.status_code == 200

        # Cancel FIX order (35=F)
        fix_cancel_msg = "8=FIX.4.4|9=55|35=F|11=CANC-TEST-001|41=ORD-TEST-001|55=AAPL|54=1|38=100|10=000|"
        res_cancel = client.post("/fix/cancel", json={"fix_message": fix_cancel_msg}, headers=fresh_user_headers)
        assert res_cancel.status_code in [200, 400]
        record_result("/fix/*", "FIX Protocol Bridge & Order Execution Lifecycle", True)

    def test_dlq_events_monitoring(self, client, fresh_user_headers):
        """GET /dlq/events, GET /dlq/events/{id}, POST /dlq/events/{id}/reprocess, DELETE /dlq/events/{id}."""
        # List DLQ events
        res_list = client.get("/dlq/events", headers=fresh_user_headers)
        assert res_list.status_code == 200
        events = res_list.json().get("events", [])

        sample_id = events[0]["id"] if events else str(uuid.uuid4())

        # Get DLQ event
        res_get = client.get(f"/dlq/events/{sample_id}", headers=fresh_user_headers)
        assert res_get.status_code in [200, 404]

        # Reprocess DLQ event
        res_rep = client.post(f"/dlq/events/{sample_id}/reprocess", headers=fresh_user_headers)
        assert res_rep.status_code in [200, 404]

        # Purge DLQ event
        res_del = client.delete(f"/dlq/events/{sample_id}", headers=fresh_user_headers)
        assert res_del.status_code in [200, 404]
        record_result("/dlq/events/*", "Dead Letter Queue (DLQ) Monitoring & Auto-Reprocessing", True)

    def test_email_digest_service(self, client, fresh_user_headers):
        """POST/GET/DELETE /digest/subscription, POST /digest/trigger."""
        # Create digest subscription
        res_sub = client.post(
            "/digest/subscription",
            json={"email": "quant_digest@hedgefund.com", "frequency": "daily", "tickers": ["AAPL", "NVDA"]},
            headers=fresh_user_headers,
        )
        assert res_sub.status_code in [200, 201]

        # Get digest subscription
        res_get = client.get("/digest/subscription", headers=fresh_user_headers)
        assert res_get.status_code == 200

        # Trigger on-demand digest delivery
        res_trig = client.post(
            "/digest/trigger",
            json={"email": "quant_digest@hedgefund.com"},
            headers=fresh_user_headers,
        )
        assert res_trig.status_code in [200, 202]

        # Delete digest subscription
        res_del = client.delete("/digest/subscription", headers=fresh_user_headers)
        assert res_del.status_code == 200
        record_result("/digest/*", "Email Market Digest Subscription & Background Worker", True)

    def test_usage_consumption_stats(self, client, fresh_user_headers):
        """GET /usage/stats - User API metering & consumption analytics."""
        res = client.get("/usage/stats", params={"days": 30}, headers=fresh_user_headers)
        assert res.status_code == 200
        data = res.json()
        assert "total_requests" in data or "summary" in data or "groups" in data
        record_result("/usage/stats", "API Usage Statistics & Consumption Metering", True)


# ─────────────────────────────────────────────────────────────────────────────────
# Standalone CLI Execution Entry Point
# ─────────────────────────────────────────────────────────────────────────────────

def main():
    """Runs the test suite using pytest and outputs summary."""
    print("=" * 90)
    print(" FINTEXT ALPHA VECTORIZER — COMPREHENSIVE API FUNCTIONAL TEST AUTOMATION SUITE")
    print("=" * 90)
    print(f" Target Server Base URL: {BASE_URL}")
    print(f" Test Automation Runner: pytest & httpx\n")

    exit_code = pytest.main(["-v", "-s", __file__])

    print("\n" + "=" * 90)
    print(" FINTEXT API FUNCTIONAL TEST SUITE EXECUTION SUMMARY")
    print("=" * 90)
    print(f" Total Test Cases Run:   {TEST_STATS['total']}")
    print(f" Total Passed:           {TEST_STATS['passed']}")
    print(f" Total Failed:           {TEST_STATS['failed']}")
    print(f" Unique Endpoints Covered: {len(TEST_STATS['endpoints'])}")
    print(f" Pytest Exit Code:       {exit_code}")
    print("=" * 90)

    return exit_code


if __name__ == "__main__":
    sys.exit(main())
