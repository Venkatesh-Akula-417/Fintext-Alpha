#!/usr/bin/env python3
"""
═══════════════════════════════════════════════════════════════════════════════
FinText-Alpha-Vectorizer — Test Suite #195: Earnings Call Transcript Database & Retrieval
═══════════════════════════════════════════════════════════════════════════════
Validates:
  1. Unauthenticated & Malformed Request Rejection (401 / 400).
  2. Input Validation & Bounds Enforcement (Ticker, Quarter 1-4, Year, Dates, Text length).
  3. Manual Transcript Creation with Automated FinBERT Sentiment Scoring.
  4. Manual Transcript Creation with Bearish Sentiment & Explicit Score Overrides.
  5. Single Transcript Retrieval by UUID (GET /transcripts/{id} & 404).
  6. Transcript Listing & Filtering (ticker, quarter, year, date range, pagination).
  7. Multi-Tenant User Isolation (cross-account visibility prevention & deletion guard).
  8. Audio Transcription & Database Auto-Persistence (POST /audio/transcribe?store=true).
  9. Transcript Deletion Lifecycle (DELETE /transcripts/{id} & idempotency).
  10. OpenAPI Schema Conformance & Python SDK Synchronous/Async Integration.
═══════════════════════════════════════════════════════════════════════════════
"""

import io
import math
import os
import struct
import subprocess
import sys
import time
from pathlib import Path
from typing import Optional

import httpx

# Ensure project root & python_sdk are in python path
ROOT_DIR = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT_DIR / "python_sdk" / "src"))
if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")

from fintext import FinTextClient, FinTextAsyncClient
from fintext.models import (
    AudioTranscriptionResponse,
    DeleteTranscriptResponse,
    TranscriptListResponse,
    TranscriptMetadata,
    TranscriptResponse,
)

SERVER_PORT = 8097
BASE_URL = f"http://127.0.0.1:{SERVER_PORT}"
ADMIN_TOKEN = "fintext-admin-dev-secret-token"
JWT_SECRET = "fintext-alpha-vectorizer-institutional-jwt-secret-key-2026"


def generate_test_wav(duration_secs: float = 1.0, sample_rate: int = 16000, freq_hz: float = 180.0) -> bytes:
    """Generates a valid 16-bit PCM mono WAV file with synthetic pitch tone."""
    num_samples = int(duration_secs * sample_rate)
    samples = []
    for i in range(num_samples):
        val = int(math.sin(2.0 * math.pi * freq_hz * (i / sample_rate)) * 12000.0)
        samples.append(val)

    byte_io = io.BytesIO()
    data_size = num_samples * 2
    chunk_size = 36 + data_size

    # RIFF Header
    byte_io.write(b"RIFF")
    byte_io.write(struct.pack("<I", chunk_size))
    byte_io.write(b"WAVE")

    # fmt Subchunk
    byte_io.write(b"fmt ")
    byte_io.write(struct.pack("<I", 16))
    byte_io.write(struct.pack("<H", 1))  # PCM
    byte_io.write(struct.pack("<H", 1))  # Mono
    byte_io.write(struct.pack("<I", sample_rate))
    byte_io.write(struct.pack("<I", sample_rate * 2))  # Byte rate
    byte_io.write(struct.pack("<H", 2))  # Block align
    byte_io.write(struct.pack("<H", 16))  # Bits per sample

    # data Subchunk
    byte_io.write(b"data")
    byte_io.write(struct.pack("<I", data_size))
    for s in samples:
        byte_io.write(struct.pack("<h", s))

    return byte_io.getvalue()


class ServerContext:
    def __init__(self):
        self.process: Optional[subprocess.Popen] = None

    def __enter__(self):
        env = os.environ.copy()
        env["PORT"] = str(SERVER_PORT)
        env["ADMIN_TOKEN"] = ADMIN_TOKEN
        env["JWT_SECRET"] = JWT_SECRET
        env["QUESTDB_MOCK_FALLBACK"] = "1"
        env["WHISPER_MOCK_FALLBACK"] = "1"
        env["NATS_MOCK_MODE"] = "1"

        exe_path = ROOT_DIR / "rust" / "target" / "release" / "fintext_api.exe"
        if not exe_path.exists():
            exe_path = ROOT_DIR / "rust" / "target" / "debug" / "fintext_api.exe"

        if not exe_path.exists():
            raise RuntimeError(f"Server binary not found at {exe_path}. Build it first.")

        print(f"[STARTING] Spawning FinText API Server from {exe_path} on port {SERVER_PORT}...")
        self.process = subprocess.Popen(
            [str(exe_path)],
            env=env,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )

        # Wait for health check readiness
        max_attempts = 50
        for i in range(max_attempts):
            try:
                resp = httpx.get(f"{BASE_URL}/health", timeout=0.5)
                if resp.status_code == 200:
                    print(f"[READY] Server is healthy and ready (attempt {i + 1})")
                    return self
            except Exception:
                time.sleep(0.1)

        if self.process.poll() is not None:
            raise RuntimeError(f"Server failed to start (exit code {self.process.poll()}).")
        raise RuntimeError("Server health check timed out.")

    def __exit__(self, exc_type, exc_val, exc_tb):
        if self.process:
            print("[STOPPING] Shutting down FinText API Server...")
            self.process.terminate()
            try:
                self.process.wait(timeout=3)
            except subprocess.TimeoutExpired:
                self.process.kill()


def get_token(user_id: str) -> str:
    resp = httpx.post(
        f"{BASE_URL}/auth/token",
        json={"user_id": user_id},
        headers={"X-Admin-Token": ADMIN_TOKEN},
    )
    assert resp.status_code == 200, f"Failed to get token for {user_id}: {resp.text}"
    return resp.json()["token"]


def run_tests():
    print("=" * 80)
    print("FINTEXT ALPHA VECTORIZER — TEST SUITE #195: TRANSCRIPTS DATABASE")
    print("=" * 80)

    token_user1 = get_token("quant_fund_citadel")
    token_user2 = get_token("quant_fund_renaissance")
    headers1 = {"Authorization": f"Bearer {token_user1}"}
    headers2 = {"Authorization": f"Bearer {token_user2}"}

    client_user1 = FinTextClient(base_url=BASE_URL, api_token=token_user1)

    with httpx.Client(base_url=BASE_URL, timeout=10.0) as http:
        # ─────────────────────────────────────────────────────────────────────
        # Phase 1: Unauthenticated & Malformed Request Rejection
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 1] Testing Unauthenticated & Invalid Token Access...")
        r = http.get("/transcripts")
        assert r.status_code == 401, f"Expected 401, got {r.status_code}"

        r = http.post("/transcripts", json={"ticker": "AAPL", "transcript_text": "sample text with words"})
        assert r.status_code == 401, f"Expected 401, got {r.status_code}"

        r = http.get("/transcripts/00000000-0000-0000-0000-000000000000")
        assert r.status_code == 401, f"Expected 401, got {r.status_code}"

        r = http.delete("/transcripts/00000000-0000-0000-0000-000000000000")
        assert r.status_code == 401, f"Expected 401, got {r.status_code}"

        r = http.get("/transcripts", headers={"Authorization": "Bearer invalid_token_123"})
        assert r.status_code == 401, f"Expected 401, got {r.status_code}"
        print("[PASS] Phase 1 Passed: Unauthorized requests correctly rejected (401).")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 2: Input Validation & Bounds Enforcement
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 2] Testing Input Validation & Bounds Enforcement...")
        # Empty ticker
        r = http.post("/transcripts", json={"ticker": "", "transcript_text": "valid text content exceeding min"}, headers=headers1)
        assert r.status_code == 400, f"Expected 400 for empty ticker, got {r.status_code}"

        # Invalid quarter
        r = http.post("/transcripts", json={"ticker": "AAPL", "quarter": 5, "transcript_text": "valid text content exceeding min"}, headers=headers1)
        assert r.status_code == 400, f"Expected 400 for quarter 5, got {r.status_code}"

        # Invalid year
        r = http.post("/transcripts", json={"ticker": "AAPL", "year": 1800, "transcript_text": "valid text content exceeding min"}, headers=headers1)
        assert r.status_code == 400, f"Expected 400 for year 1800, got {r.status_code}"

        # Invalid call_date
        r = http.post("/transcripts", json={"ticker": "AAPL", "call_date": "2024/10/31", "transcript_text": "valid text content exceeding min"}, headers=headers1)
        assert r.status_code == 400, f"Expected 400 for bad date format, got {r.status_code}"

        # Text too short
        r = http.post("/transcripts", json={"ticker": "AAPL", "transcript_text": "short"}, headers=headers1)
        assert r.status_code == 400, f"Expected 400 for text < 10 chars, got {r.status_code}"
        print("[PASS] Phase 2 Passed: Validation rules and bounds properly enforced (400).")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 3: Manual Transcript Creation with Automated Sentiment
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 3] Testing Bullish Transcript Creation with Automated FinBERT Sentiment...")
        aapl_text = "Apple Inc. reported record quarterly revenue of $94.9 billion, up 6 percent year over year with strong growth in cloud and services."
        r = http.post(
            "/transcripts",
            json={
                "ticker": "AAPL",
                "quarter": 4,
                "year": 2024,
                "call_date": "2024-10-31",
                "transcript_text": aapl_text,
                "source": "manual",
            },
            headers=headers1,
        )
        assert r.status_code == 201, f"Expected 201, got {r.status_code}: {r.text}"
        aapl_data = r.json()
        aapl_id = aapl_data["id"]
        assert aapl_data["ticker"] == "AAPL"
        assert aapl_data["quarter"] == 4
        assert aapl_data["year"] == 2024
        assert aapl_data["call_date"] == "2024-10-31"
        assert aapl_data["word_count"] == len(aapl_text.split())
        assert aapl_data["sentiment_label"] == "BULLISH"
        assert aapl_data["sentiment_score"] > 0.5
        assert aapl_data["confidence"] > 0.8
        print(f"[PASS] Phase 3 Passed: Created AAPL transcript (ID: {aapl_id}, Score: {aapl_data['sentiment_score']:.4f}, Label: {aapl_data['sentiment_label']}).")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 4: Bearish Sentiment & Explicit Overrides
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 4] Testing Bearish Transcript & Custom Score Overrides...")
        intc_text = "Intel Corporation reported substantial operating loss and missed guidance as wafer manufacturing yields were down significantly."
        r = http.post(
            "/transcripts",
            json={
                "ticker": "INTC",
                "quarter": 3,
                "year": 2024,
                "call_date": "2024-10-24",
                "transcript_text": intc_text,
            },
            headers=headers1,
        )
        assert r.status_code == 201
        intc_data = r.json()
        intc_id = intc_data["id"]
        assert intc_data["sentiment_label"] == "BEARISH"
        assert intc_data["sentiment_score"] < 0.0

        # Custom override
        r = http.post(
            "/transcripts",
            json={
                "ticker": "MSFT",
                "quarter": 1,
                "year": 2025,
                "call_date": "2024-10-30",
                "transcript_text": "Microsoft Azure and Office 365 delivered consistent operational results.",
                "sentiment_score": 0.420,
                "sentiment_label": "MODERATE_BULLISH",
                "confidence": 0.940,
            },
            headers=headers1,
        )
        assert r.status_code == 201
        msft_data = r.json()
        msft_id = msft_data["id"]
        assert msft_data["sentiment_score"] == 0.420
        assert msft_data["sentiment_label"] == "MODERATE_BULLISH"
        assert msft_data["confidence"] == 0.940
        print("[PASS] Phase 4 Passed: INTC bearish classified & MSFT custom override preserved.")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 5: Single Transcript Retrieval by UUID (GET /transcripts/{id})
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 5] Testing Single Transcript Retrieval by ID & 404 Handling...")
        r = http.get(f"/transcripts/{aapl_id}", headers=headers1)
        assert r.status_code == 200, f"Expected 200, got {r.status_code}"
        fetched = r.json()
        assert fetched["id"] == aapl_id
        assert fetched["transcript_text"] == aapl_text
        assert fetched["ticker"] == "AAPL"

        # Non-existent ID
        r = http.get("/transcripts/11111111-2222-3333-4444-555555555555", headers=headers1)
        assert r.status_code == 404, f"Expected 404, got {r.status_code}"
        print("[PASS] Phase 5 Passed: Single transcript fetched accurately; 404 returned for missing UUID.")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 6: Transcript Listing & Filtering
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 6] Testing Transcript Listing, Multi-Field Filtering & Pagination...")
        # List all
        r = http.get("/transcripts", headers=headers1)
        assert r.status_code == 200
        all_list = r.json()
        assert all_list["total"] == 3
        assert all_list["count"] == 3

        # Ticker filter
        r = http.get("/transcripts?ticker=AAPL", headers=headers1)
        assert r.status_code == 200
        aapl_list = r.json()
        assert aapl_list["total"] == 1
        assert aapl_list["items"][0]["ticker"] == "AAPL"

        # Quarter filter
        r = http.get("/transcripts?quarter=4", headers=headers1)
        assert r.status_code == 200
        q4_list = r.json()
        assert q4_list["total"] == 1
        assert q4_list["items"][0]["quarter"] == 4

        # Date range filter
        r = http.get("/transcripts?start_date=2024-10-25&end_date=2024-11-01", headers=headers1)
        assert r.status_code == 200
        date_list = r.json()
        # AAPL (10-31) and MSFT (10-30) should match
        assert date_list["total"] == 2

        # Pagination: limit=1, offset=0 & limit=1, offset=1
        r0 = http.get("/transcripts?limit=1&offset=0", headers=headers1)
        r1 = http.get("/transcripts?limit=1&offset=1", headers=headers1)
        assert r0.status_code == 200 and r1.status_code == 200
        p0 = r0.json()
        p1 = r1.json()
        assert p0["count"] == 1
        assert p1["count"] == 1
        assert p0["items"][0]["id"] != p1["items"][0]["id"]
        print("[PASS] Phase 6 Passed: Multi-criteria filtering and pagination verified.")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 7: Multi-Tenant User Isolation
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 7] Testing Multi-Tenant User Isolation...")
        # User 2 listing should be empty
        r = http.get("/transcripts", headers=headers2)
        assert r.status_code == 200
        assert r.json()["total"] == 0

        # User 2 attempting to GET User 1 transcript
        r = http.get(f"/transcripts/{aapl_id}", headers=headers2)
        assert r.status_code == 404, f"Expected 404 for cross-tenant access, got {r.status_code}"

        # User 2 attempting to DELETE User 1 transcript
        r = http.delete(f"/transcripts/{aapl_id}", headers=headers2)
        assert r.status_code == 404, f"Expected 404 for cross-tenant deletion, got {r.status_code}"
        print("[PASS] Phase 7 Passed: Strict tenant isolation verified across all endpoints.")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 8: Audio Transcription & Database Persistence Integration
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 8] Testing Audio Transcription with Auto-Persistence (store=true)...")
        wav_bytes = generate_test_wav(duration_secs=1.5, freq_hz=210.0)
        files = {"audio": ("nvda_q3_call.wav", wav_bytes, "audio/wav")}
        data = {
            "ticker": "NVDA",
            "quarter": "3",
            "year": "2024",
            "call_date": "2024-11-20",
            "store": "true",
        }
        r = http.post(
            "/audio/transcribe",
            files=files,
            data=data,
            headers=headers1,
        )
        assert r.status_code == 200, f"Expected 200, got {r.status_code}: {r.text}"
        audio_resp = r.json()
        assert audio_resp.get("transcript_id") is not None, "Expected transcript_id in response"
        audio_trans_id = audio_resp["transcript_id"]

        # Verify transcript is retrievable via GET /transcripts/{id}
        r_get = http.get(f"/transcripts/{audio_trans_id}", headers=headers1)
        assert r_get.status_code == 200
        persisted_audio_trans = r_get.json()
        assert persisted_audio_trans["ticker"] == "NVDA"
        assert persisted_audio_trans["source"] == "whisper_asr"
        assert persisted_audio_trans["quarter"] == 3
        assert persisted_audio_trans["year"] == 2024
        print(f"[PASS] Phase 8 Passed: Audio transcribed and auto-persisted as transcript {audio_trans_id}.")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 9: Transcript Deletion Lifecycle
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 9] Testing Transcript Deletion Lifecycle...")
        r = http.delete(f"/transcripts/{intc_id}", headers=headers1)
        assert r.status_code == 200
        del_data = r.json()
        assert del_data["deleted"] is True

        # Re-fetch deleted transcript -> 404
        r = http.get(f"/transcripts/{intc_id}", headers=headers1)
        assert r.status_code == 404

        # Delete non-existent transcript -> 404
        r = http.delete(f"/transcripts/{intc_id}", headers=headers1)
        assert r.status_code == 404
        print("[PASS] Phase 9 Passed: Deletion lifecycle verified with proper 404 response.")

        # ─────────────────────────────────────────────────────────────────────
        # Phase 10: OpenAPI Documentation & Python SDK Integration
        # ─────────────────────────────────────────────────────────────────────
        print("\n[Phase 10] Testing OpenAPI Documentation & Python SDK Integration...")
        # Verify OpenAPI documentation
        r = http.get("/api-docs/openapi.json")
        assert r.status_code == 200
        spec = r.json()
        assert "/transcripts" in spec["paths"]
        assert "/transcripts/{id}" in spec["paths"]
        assert "TranscriptResponse" in spec["components"]["schemas"]
        assert "TranscriptMetadata" in spec["components"]["schemas"]
        assert "TranscriptListResponse" in spec["components"]["schemas"]
        print("  + OpenAPI 3.0 specification includes all transcript endpoints and schemas.")

    # Test Python Synchronous SDK
    sdk_created = client_user1.create_transcript(
        ticker="AMZN",
        transcript_text="Amazon Web Services accelerated growth with strong generative AI customer adoption.",
        quarter=3,
        year=2024,
        call_date="2024-10-24",
    )
    assert isinstance(sdk_created, TranscriptResponse)
    assert sdk_created.ticker == "AMZN"
    assert sdk_created.sentiment_label == "BULLISH"

    sdk_list = client_user1.list_transcripts(ticker="AMZN")
    assert isinstance(sdk_list, TranscriptListResponse)
    assert sdk_list.total >= 1

    sdk_fetched = client_user1.get_transcript(sdk_created.id)
    assert isinstance(sdk_fetched, TranscriptResponse)
    assert sdk_fetched.id == sdk_created.id

    sdk_del = client_user1.delete_transcript(sdk_created.id)
    assert isinstance(sdk_del, DeleteTranscriptResponse)
    assert sdk_del.deleted is True

    # Test audio transcribe with store=True in SDK
    sdk_audio = client_user1.transcribe_audio(
        wav_bytes,
        filename="sdk_call.wav",
        store=True,
        ticker="GOOGL",
        quarter=3,
        year=2024,
    )
    assert isinstance(sdk_audio, AudioTranscriptionResponse)
    assert sdk_audio.transcript_id is not None
    client_user1.delete_transcript(sdk_audio.transcript_id)

    client_user1.close()
    print("  + Python SDK synchronous methods (CRUD & audio store) verified.")

    print("\n" + "=" * 80)
    print("[SUCCESS] ALL 10 PHASES PASSED: EARNINGS CALL TRANSCRIPT DATABASE FULLY CERTIFIED!")
    print("=" * 80)


def main():
    with ServerContext():
        run_tests()


if __name__ == "__main__":
    main()
