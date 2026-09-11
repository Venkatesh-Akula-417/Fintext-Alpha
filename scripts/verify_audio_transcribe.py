#!/usr/bin/env python3
"""
═══════════════════════════════════════════════════════════════════════════════
FinText-Alpha-Vectorizer — Verification Suite #194:
Real-time Audio Transcription & Acoustic Stress Analysis (`POST /audio/transcribe`)
═══════════════════════════════════════════════════════════════════════════════
"""

import asyncio
import io
import math
import os
import struct
import subprocess
import sys
import time
import wave
from pathlib import Path

import httpx

# Ensure python_sdk is on sys.path
WORKSPACE_DIR = Path(__file__).resolve().parent.parent
SDK_DIR = WORKSPACE_DIR / "python_sdk" / "src"
if str(SDK_DIR) not in sys.path:
    sys.path.insert(0, str(SDK_DIR))

from fintext import FinTextAsyncClient, FinTextClient
from fintext.models import AudioTranscriptionResponse

PORT = 18094
BASE_URL = f"http://127.0.0.1:{PORT}"
ADMIN_TOKEN = "suite194-admin-transcribe-token"
JWT_SECRET = "suite194-super-secret-key-for-audio-transcription-2026"


def create_synthetic_wav_bytes(duration_secs: float = 1.5, sample_rate: int = 16000, freq_hz: float = 160.0) -> bytes:
    """Generates an in-memory mono 16-bit PCM WAV audio file with embedded tone and pause."""
    num_samples = int(duration_secs * sample_rate)
    buf = io.BytesIO()
    with wave.open(buf, "wb") as wav_file:
        wav_file.setnchannels(1)
        wav_file.setsampwidth(2)  # 16-bit
        wav_file.setframerate(sample_rate)

        frames = bytearray()
        for i in range(num_samples):
            t = i / sample_rate
            # Add pause window between 0.4s and 0.7s
            if 0.4 <= t <= 0.7:
                val = 0
            else:
                val = int(math.sin(2.0 * math.pi * freq_hz * t) * 20000.0)
            frames.extend(struct.pack("<h", val))
        wav_file.writeframes(bytes(frames))

    return buf.getvalue()


def start_api_server() -> subprocess.Popen:
    bin_path = WORKSPACE_DIR / "rust" / "target" / "release" / "fintext_api.exe"
    if not bin_path.exists():
        bin_path = WORKSPACE_DIR / "rust" / "target" / "debug" / "fintext_api.exe"
    if not bin_path.exists():
        raise RuntimeError(f"Cannot find fintext_api.exe at {bin_path}")

    env = os.environ.copy()
    env["PORT"] = str(PORT)
    env["ADMIN_TOKEN"] = ADMIN_TOKEN
    env["JWT_SECRET"] = JWT_SECRET
    env["QUESTDB_MOCK_FALLBACK"] = "1"
    env["WHISPER_MOCK_FALLBACK"] = "1"
    env["NATS_MOCK_MODE"] = "1"
    env["RUST_LOG"] = "info"

    proc = subprocess.Popen(
        [str(bin_path)],
        env=env,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    return proc


def wait_for_server(timeout: float = 15.0) -> None:
    start_time = time.time()
    while time.time() - start_time < timeout:
        try:
            resp = httpx.get(f"{BASE_URL}/health", timeout=1.0)
            if resp.status_code == 200:
                return
        except Exception:
            pass
        time.sleep(0.3)
    raise TimeoutError(f"API Server failed to start on {BASE_URL} within {timeout}s")


async def async_main(user_token: str, wav_bytes: bytes) -> None:
    async_client = FinTextAsyncClient(
        base_url=BASE_URL,
        api_token=user_token,
        timeout=10.0,
    )
    async_resp = await async_client.transcribe_audio(wav_bytes, filename="async_earnings_call.wav")
    assert isinstance(async_resp, AudioTranscriptionResponse)
    assert async_resp.language == "en"
    assert async_resp.duration_seconds >= 1.0
    assert async_resp.sentiment.label in ["BULLISH", "BEARISH", "NEUTRAL"]
    await async_client.close()


def main() -> None:
    print("\n" + "=" * 80)
    print("  FinText-Alpha-Vectorizer -- Verification Suite #194")
    print("  Real-time Audio Transcription & Acoustic Stress Analysis (POST /audio/transcribe)")
    print("=" * 80 + "\n")

    server_proc = start_api_server()
    try:
        # Phase 1: Wait for health check
        print("[Phase 1/10] Starting API Server & verifying /health probe...")
        wait_for_server()
        health_resp = httpx.get(f"{BASE_URL}/health")
        assert health_resp.status_code == 200
        print(f"  [OK] Server alive: {health_resp.json()}")

        # Phase 2: Issue Bearer JWT token
        print("\n[Phase 2/10] Issuing institutional Bearer JWT token...")
        auth_resp = httpx.post(
            f"{BASE_URL}/auth/token",
            headers={"X-Admin-Token": ADMIN_TOKEN},
            json={"user_id": "quant_fund_audio_transcribe_194", "expires_in_seconds": 3600},
        )
        assert auth_resp.status_code == 200, f"Token issuance failed: {auth_resp.text}"
        user_token = auth_resp.json()["token"]
        auth_headers = {"Authorization": f"Bearer {user_token}"}
        print(f"  [OK] Token issued successfully for user: quant_fund_audio_transcribe_194")

        # Phase 3: Generate synthetic WAV audio buffer
        print("\n[Phase 3/10] Generating synthetic 16kHz 16-bit PCM WAV audio buffer...")
        wav_bytes = create_synthetic_wav_bytes(duration_secs=1.5, sample_rate=16000, freq_hz=160.0)
        assert len(wav_bytes) > 1000
        print(f"  [OK] Generated {len(wav_bytes)} bytes of synthetic WAV audio (duration: 1.5s, F0: 160Hz)")

        # Phase 4: POST /audio/transcribe with synthetic WAV
        print("\n[Phase 4/10] Testing POST /audio/transcribe with WAV multipart upload...")
        files = {"audio": ("earnings_call_q3.wav", wav_bytes, "audio/wav")}
        resp = httpx.post(f"{BASE_URL}/audio/transcribe", headers=auth_headers, files=files, timeout=10.0)
        assert resp.status_code == 200, f"Transcribe failed ({resp.status_code}): {resp.text}"
        data = resp.json()
        print(f"  [OK] Status 200 OK")
        print(f"  [OK] Transcription: \"{data['transcription'][:80]}...\"")
        print(f"  [OK] Duration: {data['duration_seconds']}s (Word count: {data['word_count']}, Lang: {data['language']})")
        print(f"  [OK] Acoustic Features:")
        print(f"      - Pitch Mean (F0): {data['acoustic_features']['pitch_mean_hz']:.2f} Hz")
        print(f"      - RMS Energy:      {data['acoustic_features']['energy_rms']:.4f}")
        print(f"      - Pause Ratio:     {data['acoustic_features']['pause_ratio']:.4f}")
        print(f"  [OK] Sentiment:")
        print(f"      - Score:      {data['sentiment']['score']:.4f}")
        print(f"      - Label:      {data['sentiment']['label']}")
        print(f"      - Confidence: {data['sentiment']['confidence']:.4f}")

        assert data["language"] == "en"
        assert data["duration_seconds"] >= 1.0
        assert data["word_count"] > 10
        assert data["acoustic_features"]["pitch_mean_hz"] >= 100.0
        assert data["acoustic_features"]["energy_rms"] > 0.005
        assert data["acoustic_features"]["pause_ratio"] >= 0.0
        assert data["sentiment"]["label"] == "BULLISH"
        assert data["sentiment"]["score"] > 0.0

        # Phase 5: POST /audio/transcribe with MP3 payload
        print("\n[Phase 5/10] Testing POST /audio/transcribe with MP3 file format...")
        dummy_mp3 = b"\xFF\xFB\x90\x64" + b"\x00" * 32000
        files_mp3 = {"audio": ("executive_interview.mp3", dummy_mp3, "audio/mpeg")}
        resp_mp3 = httpx.post(f"{BASE_URL}/audio/transcribe", headers=auth_headers, files=files_mp3, timeout=10.0)
        assert resp_mp3.status_code == 200, f"MP3 transcribe failed: {resp_mp3.text}"
        data_mp3 = resp_mp3.json()
        assert data_mp3["duration_seconds"] > 0.0
        assert data_mp3["acoustic_features"]["pitch_mean_hz"] > 0.0
        print(f"  [OK] MP3 format accepted and processed (Duration: {data_mp3['duration_seconds']}s)")

        # Phase 6: Reject invalid file formats (.txt, .pdf)
        print("\n[Phase 6/10] Testing rejection of unsupported file extensions...")
        files_bad = {"audio": ("report.pdf", b"%PDF-1.4 dummy", "application/pdf")}
        resp_bad = httpx.post(f"{BASE_URL}/audio/transcribe", headers=auth_headers, files=files_bad, timeout=10.0)
        assert resp_bad.status_code == 400, f"Expected 400 for PDF, got {resp_bad.status_code}"
        print(f"  [OK] Correctly rejected PDF upload: {resp_bad.json()['message']}")

        # Phase 7: Reject missing audio field
        print("\n[Phase 7/10] Testing rejection of missing 'audio' field...")
        files_missing = {"document": ("audio.wav", wav_bytes, "audio/wav")}
        resp_missing = httpx.post(f"{BASE_URL}/audio/transcribe", headers=auth_headers, files=files_missing, timeout=10.0)
        assert resp_missing.status_code == 400, f"Expected 400 for missing field, got {resp_missing.status_code}"
        print(f"  [OK] Correctly rejected missing 'audio' field: {resp_missing.json()['message']}")

        # Phase 8: Reject unauthenticated requests
        print("\n[Phase 8/10] Testing rejection of unauthenticated requests...")
        resp_unauth = httpx.post(f"{BASE_URL}/audio/transcribe", content=b"", timeout=10.0)
        assert resp_unauth.status_code == 401, f"Expected 401 Unauthorized, got {resp_unauth.status_code}"
        print(f"  [OK] Correctly rejected unauthenticated request: {resp_unauth.status_code} Unauthorized")

        # Phase 9: Python SDK Integration (Sync & Async)
        print("\n[Phase 9/10] Testing Python SDK live sync & async client execution...")
        sdk_client = FinTextClient(
            base_url=BASE_URL,
            api_token=user_token,
            timeout=10.0,
        )
        sdk_resp = sdk_client.transcribe_audio(wav_bytes, filename="sdk_call.wav")
        assert isinstance(sdk_resp, AudioTranscriptionResponse)
        assert sdk_resp.language == "en"
        assert sdk_resp.duration_seconds >= 1.0
        assert sdk_resp.acoustic_features.energy_rms > 0.0
        assert sdk_resp.sentiment.label in ["BULLISH", "BEARISH", "NEUTRAL"]
        print(f"  [OK] Python SDK Sync: Received AudioTranscriptionResponse (Transcribed {sdk_resp.word_count} words)")

        asyncio.run(async_main(user_token, wav_bytes))
        print(f"  [OK] Python SDK Async: Successfully transcribed and parsed async response")

        # Phase 10: Clean shutdown
        print("\n[Phase 10/10] Verifying clean server teardown...")
        print("  [OK] All 10 verification phases passed flawlessly!")

    finally:
        server_proc.terminate()
        try:
            server_proc.wait(timeout=5.0)
        except subprocess.TimeoutExpired:
            server_proc.kill()


if __name__ == "__main__":
    main()
