#!/usr/bin/env python3
"""
===============================================================================
FinText-Alpha-Vectorizer — Suite #224: Crypto News Sentiment Certification
===============================================================================
Verifies:
  1.  Unauthenticated GET /crypto/sentiment returns 401 Unauthorized
  2.  Invalid crypto asset returns 400 Bad Request
  3.  Invalid start_date/end_date format returns 400 Bad Request
  4.  Inverted date range (start_date > end_date) returns 400 Bad Request
  5.  Out-of-bounds min_confidence (< 0.0 or > 1.0) returns 400 Bad Request
  6.  Out-of-bounds limit (< 1 or > 50) returns 400 Bad Request
  7.  Default Bitcoin (BTC) sentiment analysis returns 200 OK with valid schema
  8.  Ethereum (ETH) sentiment analysis returns 200 OK with valid metrics
  9.  Solana (SOL) sentiment analysis returns 200 OK with valid metrics
  10. Multi-asset evaluation (BNB, XRP, ADA) returns 200 OK
  11. Model confidence threshold filtering works correctly
  12. Python SDK sync and async client parity verification
===============================================================================
"""

import asyncio
import os
from pathlib import Path
import subprocess
import sys
import time

import httpx

if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
if hasattr(sys.stderr, "reconfigure"):
    sys.stderr.reconfigure(encoding="utf-8", errors="replace")

PROJECT_ROOT = Path(__file__).resolve().parent.parent
SDK_PATH = PROJECT_ROOT / "python_sdk" / "src"
if str(SDK_PATH) not in sys.path:
    sys.path.insert(0, str(SDK_PATH))

PORT = 8124
BASE_URL = f"http://127.0.0.1:{PORT}"
ADMIN_TOKEN = "test_admin_token_xyz123_valid_32_bytes_length!"
SERVER_EXE = PROJECT_ROOT / "rust" / "target" / "debug" / "fintext_api.exe"

passed = 0
failed = 0
total = 12


def report(phase: int, name: str, ok: bool, detail: str = ""):
    global passed, failed
    if ok:
        passed += 1
        print(f"  ✅ Phase {phase:2d} │ {name}")
    else:
        failed += 1
        msg = f"  ❌ Phase {phase:2d} │ {name}"
        if detail:
            msg += f" — {detail}"
        print(msg)


class ServerContext:
    def __init__(self):
        self.process = None

    def __enter__(self):
        print(f"[STARTING] Spawning FinText API Server on port {PORT}...")
        env = os.environ.copy()
        env["PORT"] = str(PORT)
        env["HOST"] = "127.0.0.1"
        env["ADMIN_TOKEN"] = ADMIN_TOKEN
        env["JWT_SECRET"] = "super_secret_test_jwt_key_32_bytes_len!!"
        env["QUESTDB_MOCK_FALLBACK"] = "1"
        env["POLYGON_MOCK_FALLBACK"] = "1"
        env["WHISPER_MOCK_FALLBACK"] = "1"
        env["NATS_MOCK_MODE"] = "1"

        self.process = subprocess.Popen(
            [str(SERVER_EXE)],
            env=env,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )

        start_t = time.time()
        ready = False
        while time.time() - start_t < 25:
            try:
                r = httpx.get(f"{BASE_URL}/health", timeout=1.0)
                if r.status_code == 200:
                    ready = True
                    break
            except Exception:
                time.sleep(0.3)

        if not ready:
            if self.process.poll() is not None:
                raise RuntimeError("FinText API server process exited prematurely")
            raise RuntimeError("FinText API server failed to start within 25 seconds")

        print("[READY] FinText API Server is responding to health checks.\n")
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        if self.process:
            print("\n[STOPPING] Terminating FinText API Server process...")
            self.process.terminate()
            try:
                self.process.wait(timeout=5.0)
            except subprocess.TimeoutExpired:
                self.process.kill()


def get_auth_token(user_id: str = "crypto_quant_01", role: str = "institutional") -> str:
    r = httpx.post(
        f"{BASE_URL}/auth/token",
        json={"user_id": user_id, "role": role, "expires_in_seconds": 3600},
        headers={"X-Admin-Token": ADMIN_TOKEN},
        timeout=5.0,
    )
    if r.status_code != 200:
        raise RuntimeError(f"Failed to obtain auth token: {r.status_code} - {r.text}")
    return r.json()["token"]


def main():
    print("=" * 79)
    print("  FinText-Alpha-Vectorizer — Suite #224: Crypto News Sentiment Certification")
    print("=" * 79)

    with ServerContext():
        token = get_auth_token("crypto_fund_alpha", role="institutional")
        headers = {"Authorization": f"Bearer {token}"}

        # ── Phase 1: Unauthenticated request rejection ─────────────────────────
        try:
            r = httpx.get(f"{BASE_URL}/crypto/sentiment?asset=BTC", timeout=5.0)
            ok = r.status_code == 401
            report(1, "Unauthenticated GET /crypto/sentiment returns 401 Unauthorized", ok, f"status={r.status_code}")
        except Exception as e:
            report(1, "Unauthenticated GET /crypto/sentiment returns 401 Unauthorized", False, str(e))

        # ── Phase 2: Invalid cryptocurrency asset ──────────────────────────────
        try:
            r = httpx.get(f"{BASE_URL}/crypto/sentiment?asset=DOGECOIN_INVALID", headers=headers, timeout=5.0)
            ok = r.status_code == 400
            report(2, "Invalid crypto asset (DOGECOIN_INVALID) returns 400 Bad Request", ok, f"status={r.status_code}")
        except Exception as e:
            report(2, "Invalid crypto asset returns 400 Bad Request", False, str(e))

        # ── Phase 3: Invalid date format ───────────────────────────────────────
        try:
            r = httpx.get(f"{BASE_URL}/crypto/sentiment?asset=BTC&start_date=2025/08/01", headers=headers, timeout=5.0)
            ok = r.status_code == 400
            report(3, "Invalid start_date format returns 400 Bad Request", ok, f"status={r.status_code}")
        except Exception as e:
            report(3, "Invalid start_date format returns 400 Bad Request", False, str(e))

        # ── Phase 4: Inverted date range ───────────────────────────────────────
        try:
            r = httpx.get(
                f"{BASE_URL}/crypto/sentiment?asset=BTC&start_date=2025-08-30&end_date=2025-08-01",
                headers=headers,
                timeout=5.0,
            )
            ok = r.status_code == 400
            report(4, "Inverted date range (start > end) returns 400 Bad Request", ok, f"status={r.status_code}")
        except Exception as e:
            report(4, "Inverted date range returns 400 Bad Request", False, str(e))

        # ── Phase 5: Out-of-bounds min_confidence ──────────────────────────────
        try:
            r1 = httpx.get(f"{BASE_URL}/crypto/sentiment?asset=BTC&min_confidence=1.5", headers=headers, timeout=5.0)
            r2 = httpx.get(f"{BASE_URL}/crypto/sentiment?asset=BTC&min_confidence=-0.2", headers=headers, timeout=5.0)
            ok = r1.status_code == 400 and r2.status_code == 400
            report(5, "Out-of-bounds min_confidence (< 0.0 or > 1.0) returns 400 Bad Request", ok)
        except Exception as e:
            report(5, "Out-of-bounds min_confidence returns 400 Bad Request", False, str(e))

        # ── Phase 6: Out-of-bounds limit ───────────────────────────────────────
        try:
            r1 = httpx.get(f"{BASE_URL}/crypto/sentiment?asset=BTC&limit=0", headers=headers, timeout=5.0)
            r2 = httpx.get(f"{BASE_URL}/crypto/sentiment?asset=BTC&limit=100", headers=headers, timeout=5.0)
            ok = r1.status_code == 400 and r2.status_code == 400
            report(6, "Out-of-bounds limit (< 1 or > 50) returns 400 Bad Request", ok)
        except Exception as e:
            report(6, "Out-of-bounds limit returns 400 Bad Request", False, str(e))

        # ── Phase 7: Default Bitcoin (BTC) sentiment analysis ─────────────────
        try:
            r = httpx.get(f"{BASE_URL}/crypto/sentiment?asset=BTC", headers=headers, timeout=5.0)
            ok = r.status_code == 200
            data = r.json()
            ok = ok and data.get("asset") == "BTC"
            ok = ok and data.get("summary", {}).get("mention_count", 0) > 0
            ok = ok and -1.0 <= data.get("summary", {}).get("avg_sentiment", 99.0) <= 1.0
            ok = ok and isinstance(data.get("top_articles"), list) and len(data["top_articles"]) > 0
            report(7, f"Default Bitcoin (BTC) sentiment analysis (200 OK, mentions={data.get('summary', {}).get('mention_count')})", ok)
        except Exception as e:
            report(7, "Default Bitcoin (BTC) sentiment analysis", False, str(e))

        # ── Phase 8: Ethereum (ETH) sentiment analysis ─────────────────────────
        try:
            r = httpx.get(f"{BASE_URL}/crypto/sentiment?asset=ETH&min_confidence=0.5", headers=headers, timeout=5.0)
            ok = r.status_code == 200
            data = r.json()
            ok = ok and data.get("asset") == "ETH"
            ok = ok and data.get("summary", {}).get("mention_count", 0) > 0
            report(8, f"Ethereum (ETH) sentiment analysis (200 OK, avg_sentiment={data.get('summary', {}).get('avg_sentiment')})", ok)
        except Exception as e:
            report(8, "Ethereum (ETH) sentiment analysis", False, str(e))

        # ── Phase 9: Solana (SOL) sentiment analysis ───────────────────────────
        try:
            r = httpx.get(f"{BASE_URL}/crypto/sentiment?asset=SOL", headers=headers, timeout=5.0)
            ok = r.status_code == 200
            data = r.json()
            ok = ok and data.get("asset") == "SOL"
            ok = ok and data.get("summary", {}).get("mention_count", 0) > 0
            report(9, f"Solana (SOL) sentiment analysis (200 OK, mentions={data.get('summary', {}).get('mention_count')})", ok)
        except Exception as e:
            report(9, "Solana (SOL) sentiment analysis", False, str(e))

        # ── Phase 10: Multi-asset evaluation (BNB, XRP, ADA) ───────────────────
        try:
            r_bnb = httpx.get(f"{BASE_URL}/crypto/sentiment?asset=BNB", headers=headers, timeout=5.0)
            r_xrp = httpx.get(f"{BASE_URL}/crypto/sentiment?asset=XRP", headers=headers, timeout=5.0)
            r_ada = httpx.get(f"{BASE_URL}/crypto/sentiment?asset=ADA", headers=headers, timeout=5.0)

            ok_bnb = r_bnb.status_code == 200 and r_bnb.json().get("asset") == "BNB"
            ok_xrp = r_xrp.status_code == 200 and r_xrp.json().get("asset") == "XRP"
            ok_ada = r_ada.status_code == 200 and r_ada.json().get("asset") == "ADA"
            report(10, "Multi-asset evaluation (BNB, XRP, ADA -> 200 OK)", ok_bnb and ok_xrp and ok_ada)
        except Exception as e:
            report(10, "Multi-asset evaluation (BNB, XRP, ADA)", False, str(e))

        # ── Phase 11: Confidence filter validation ─────────────────────────────
        try:
            r_filtered = httpx.get(f"{BASE_URL}/crypto/sentiment?asset=BTC&min_confidence=0.88", headers=headers, timeout=5.0)
            ok = r_filtered.status_code == 200
            data = r_filtered.json()
            for art in data.get("top_articles", []):
                if art.get("confidence", 0.0) < 0.88:
                    ok = False
                    break
            report(11, f"Model confidence threshold filtering (min_confidence=0.88 -> 200 OK, articles={len(data.get('top_articles', []))})", ok)
        except Exception as e:
            report(11, "Model confidence threshold filtering", False, str(e))

        # ── Phase 12: Python SDK sync & async client parity ────────────────────
        try:
            from fintext import FinTextClient, FinTextAsyncClient

            token_sdk = get_auth_token("sdk_crypto_user", role="institutional")
            with FinTextClient(base_url=BASE_URL, api_token=token_sdk) as client:
                res_sync = client.crypto_sentiment(asset="BTC", min_confidence=0.5, limit=5)
                ok_sync = res_sync.asset == "BTC" and res_sync.summary.mention_count > 0 and len(res_sync.top_articles) > 0

            async def verify_async():
                token_async = get_auth_token("sdk_async_crypto_user", role="institutional")
                async with FinTextAsyncClient(base_url=BASE_URL, api_token=token_async) as aclient:
                    res_async = await aclient.crypto_sentiment(asset="ETH", min_confidence=0.5, limit=5)
                    return res_async.asset == "ETH" and res_async.summary.mention_count > 0

            ok_async = asyncio.run(verify_async())
            report(12, "Python SDK sync & async client parity verification (200 OK)", ok_sync and ok_async)
        except Exception as e:
            report(12, "Python SDK sync & async client parity verification", False, str(e))

    # ── Summary ──────────────────────────────────────────────────────────────
    print("\n" + "=" * 79)
    pct = (passed / total) * 100.0
    print(f"  Suite #224 Results: {passed}/{total} Passed ({pct:.1f}%)")
    print("=" * 79)
    if failed == 0:
        print("\n🎉 ALL 12 PHASES PASSED CLEANLY!\n")
    else:
        print(f"\n❌ {failed} PHASES FAILED!\n")
        sys.exit(1)


if __name__ == "__main__":
    main()
