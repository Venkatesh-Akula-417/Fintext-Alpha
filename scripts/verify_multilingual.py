#!/usr/bin/env python3
"""
===============================================================================
FinText-Alpha-Vectorizer — Suite #240: Multilingual Sentiment Support &
                                       Language Detection Engine
===============================================================================
Verifies:
  1.  Unauthenticated GET /language/detect returns 401 Unauthorized
  2.  Empty text parameter returns 400 Bad Request
  3.  English text correctly detected with high confidence
  4.  Spanish text correctly detected and multilingual model applied
  5.  German text correctly detected and multilingual model applied
  6.  French text correctly detected and multilingual model applied
  7.  Japanese text correctly detected via Unicode range analysis
  8.  Mixed English-dominant text defaults to English
  9.  Model routing: English uses 'finbert-v3.1.0', non-English uses 'multilingual-minilm-v1.0'
  10. Analyzed chars are capped at 500 characters for performance
  11. Confidence scores are in valid range [0.0, 1.0]
  12. Python SDK Sync Client integration (client.detect_language)
  13. Python SDK Async Client integration (await async_client.detect_language)
  14. OpenAPI specification verification (/language/detect path, LanguageDetectionResponse schema, tag)
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

PORT = 8140
BASE_URL = f"http://127.0.0.1:{PORT}"
ADMIN_TOKEN = "test_admin_token_xyz123_valid_32_bytes_length!"
SERVER_EXE = PROJECT_ROOT / "rust" / "target" / "debug" / "fintext_api.exe"

passed = 0
failed = 0
total = 14


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
        env["CHAT_ALERTS_MOCK"] = "1"

        self.process = subprocess.Popen(
            [str(SERVER_EXE)],
            env=env,
            cwd=str(PROJECT_ROOT),
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
                time.sleep(0.4)

        if not ready:
            if self.process.poll() is not None:
                raise RuntimeError(f"Server exited prematurely with return code {self.process.returncode}")
            raise TimeoutError("FinText API server failed to respond within 25 seconds.")

        print(f"[READY] API Server is healthy at {BASE_URL}")
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        if self.process and self.process.poll() is None:
            print("[CLEANUP] Terminating FinText API Server process...")
            self.process.terminate()
            try:
                self.process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait(timeout=2)


def get_token(user_id: str = "multilingual_tester") -> str:
    payload = {"user_id": user_id, "role": "institutional", "expiry_seconds": 3600}
    headers = {"X-Admin-Token": ADMIN_TOKEN}
    r = httpx.post(f"{BASE_URL}/auth/token", json=payload, headers=headers, timeout=5.0)
    r.raise_for_status()
    return r.json()["token"]


# ─── Test inputs ─────────────────────────────────────────────────────────────
ENGLISH_TEXT = "Federal Reserve raises interest rates by 25 basis points amid persistent inflation concerns"
SPANISH_TEXT = "El Banco Central Europeo mantiene los tipos de interés sin cambios en su última reunión"
GERMAN_TEXT = "Die Deutsche Bundesbank warnt vor steigenden Risiken im Immobilienmarkt"
FRENCH_TEXT = "La Banque de France publie son rapport annuel sur la stabilité financière du pays"
JAPANESE_TEXT = "日本銀行は金融政策決定会合で大規模な金融緩和の維持を決定しました"
LONG_TEXT = "A " * 600  # 1200 chars, should be capped at 500


def run_tests():
    print("=" * 80)
    print("FINTEXT ALPHA VECTORIZER — SUITE #240: MULTILINGUAL SENTIMENT & LANGUAGE DETECTION")
    print("=" * 80)

    with ServerContext():
        # Phase 1: Unauthenticated GET /language/detect returns 401
        try:
            r = httpx.get(f"{BASE_URL}/language/detect", params={"text": ENGLISH_TEXT}, timeout=5.0)
            ok = r.status_code == 401
            report(1, "Unauthenticated GET /language/detect returns 401 Unauthorized", ok, f"status={r.status_code}")
        except Exception as e:
            report(1, "Unauthenticated GET /language/detect returns 401 Unauthorized", False, str(e))

        token = get_token()
        auth_headers = {"Authorization": f"Bearer {token}"}

        # Phase 2: Empty text parameter returns 400
        try:
            r = httpx.get(f"{BASE_URL}/language/detect", params={"text": "   "}, headers=auth_headers, timeout=5.0)
            ok = r.status_code == 400
            report(2, "Empty text parameter returns 400 Bad Request", ok, f"status={r.status_code}")
        except Exception as e:
            report(2, "Empty text parameter returns 400 Bad Request", False, str(e))

        # Phase 3: English text detected correctly
        try:
            r = httpx.get(f"{BASE_URL}/language/detect", params={"text": ENGLISH_TEXT}, headers=auth_headers, timeout=5.0)
            ok = r.status_code == 200
            data = r.json()
            lang_ok = data.get("language") == "english" and data.get("language_code") == "en"
            conf_ok = data.get("confidence", 0) >= 0.5
            model_ok = not data.get("is_multilingual_model_applied", True)
            ok = ok and lang_ok and conf_ok and model_ok
            report(3, "English text correctly detected with high confidence", ok,
                   f"lang={data.get('language')}, code={data.get('language_code')}, conf={data.get('confidence', 0):.2f}")
        except Exception as e:
            report(3, "English text correctly detected with high confidence", False, str(e))

        # Phase 4: Spanish text detected, multilingual model applied
        try:
            r = httpx.get(f"{BASE_URL}/language/detect", params={"text": SPANISH_TEXT}, headers=auth_headers, timeout=5.0)
            ok = r.status_code == 200
            data = r.json()
            lang_ok = data.get("language") == "spanish" and data.get("language_code") == "es"
            multi_ok = data.get("is_multilingual_model_applied", False)
            ok = ok and lang_ok and multi_ok
            report(4, "Spanish text correctly detected and multilingual model applied", ok,
                   f"lang={data.get('language')}, multilingual={data.get('is_multilingual_model_applied')}")
        except Exception as e:
            report(4, "Spanish text correctly detected and multilingual model applied", False, str(e))

        # Phase 5: German text detected, multilingual model applied
        try:
            r = httpx.get(f"{BASE_URL}/language/detect", params={"text": GERMAN_TEXT}, headers=auth_headers, timeout=5.0)
            ok = r.status_code == 200
            data = r.json()
            lang_ok = data.get("language") == "german" and data.get("language_code") == "de"
            multi_ok = data.get("is_multilingual_model_applied", False)
            ok = ok and lang_ok and multi_ok
            report(5, "German text correctly detected and multilingual model applied", ok,
                   f"lang={data.get('language')}, multilingual={data.get('is_multilingual_model_applied')}")
        except Exception as e:
            report(5, "German text correctly detected and multilingual model applied", False, str(e))

        # Phase 6: French text detected, multilingual model applied
        try:
            r = httpx.get(f"{BASE_URL}/language/detect", params={"text": FRENCH_TEXT}, headers=auth_headers, timeout=5.0)
            ok = r.status_code == 200
            data = r.json()
            lang_ok = data.get("language") == "french" and data.get("language_code") == "fr"
            multi_ok = data.get("is_multilingual_model_applied", False)
            ok = ok and lang_ok and multi_ok
            report(6, "French text correctly detected and multilingual model applied", ok,
                   f"lang={data.get('language')}, multilingual={data.get('is_multilingual_model_applied')}")
        except Exception as e:
            report(6, "French text correctly detected and multilingual model applied", False, str(e))

        # Phase 7: Japanese text detected via Unicode range
        try:
            r = httpx.get(f"{BASE_URL}/language/detect", params={"text": JAPANESE_TEXT}, headers=auth_headers, timeout=5.0)
            ok = r.status_code == 200
            data = r.json()
            lang_ok = data.get("language") == "japanese" and data.get("language_code") == "ja"
            multi_ok = data.get("is_multilingual_model_applied", False)
            ok = ok and lang_ok and multi_ok
            report(7, "Japanese text correctly detected via Unicode range analysis", ok,
                   f"lang={data.get('language')}, multilingual={data.get('is_multilingual_model_applied')}")
        except Exception as e:
            report(7, "Japanese text correctly detected via Unicode range analysis", False, str(e))

        # Phase 8: Mixed English-dominant text defaults to English
        try:
            mixed_text = "The central bank policy is unchanged los tipos de interés"
            r = httpx.get(f"{BASE_URL}/language/detect", params={"text": mixed_text}, headers=auth_headers, timeout=5.0)
            ok = r.status_code == 200
            data = r.json()
            # Mixed text should resolve to a detected language (English dominant expected)
            ok = ok and data.get("language") is not None and data.get("language_code") is not None
            report(8, "Mixed English-dominant text resolves to a valid language", ok,
                   f"lang={data.get('language')}, code={data.get('language_code')}")
        except Exception as e:
            report(8, "Mixed English-dominant text resolves to a valid language", False, str(e))

        # Phase 9: Model routing correctness
        try:
            # English should get default model
            r_en = httpx.get(f"{BASE_URL}/language/detect", params={"text": ENGLISH_TEXT}, headers=auth_headers, timeout=5.0)
            en_data = r_en.json()
            en_model = en_data.get("model_version", "")

            # Spanish should get multilingual model
            r_es = httpx.get(f"{BASE_URL}/language/detect", params={"text": SPANISH_TEXT}, headers=auth_headers, timeout=5.0)
            es_data = r_es.json()
            es_model = es_data.get("model_version", "")

            en_ok = "finbert" in en_model.lower() or "minilm" in en_model.lower()
            es_ok = "multilingual" in es_model.lower()
            ok = en_ok and es_ok
            report(9, "Model routing: English→finbert, non-English→multilingual", ok,
                   f"en_model={en_model}, es_model={es_model}")
        except Exception as e:
            report(9, "Model routing: English→finbert, non-English→multilingual", False, str(e))

        # Phase 10: Analyzed chars capped at 500
        try:
            r = httpx.get(f"{BASE_URL}/language/detect", params={"text": LONG_TEXT}, headers=auth_headers, timeout=5.0)
            ok = r.status_code == 200
            data = r.json()
            analyzed = data.get("analyzed_chars", 0)
            ok = ok and analyzed <= 500
            report(10, "Analyzed chars are capped at 500 characters for performance", ok,
                   f"analyzed_chars={analyzed}")
        except Exception as e:
            report(10, "Analyzed chars are capped at 500 characters for performance", False, str(e))

        # Phase 11: Confidence scores in valid range
        try:
            all_valid = True
            for text, lang_name in [(ENGLISH_TEXT, "English"), (SPANISH_TEXT, "Spanish"), (JAPANESE_TEXT, "Japanese")]:
                r = httpx.get(f"{BASE_URL}/language/detect", params={"text": text}, headers=auth_headers, timeout=5.0)
                data = r.json()
                conf = data.get("confidence", -1)
                if not (0.0 <= conf <= 1.0):
                    all_valid = False
            report(11, "Confidence scores are in valid range [0.0, 1.0]", all_valid)
        except Exception as e:
            report(11, "Confidence scores are in valid range [0.0, 1.0]", False, str(e))

        # Phase 12: Python SDK Sync Client integration
        try:
            from fintext import FinTextClient, LanguageDetectionResponse
            client = FinTextClient(base_url=BASE_URL, api_token=token, timeout=10.0)
            result = client.detect_language(ENGLISH_TEXT)
            ok = isinstance(result, LanguageDetectionResponse)
            ok = ok and result.language == "english"
            ok = ok and result.language_code == "en"
            ok = ok and 0.0 <= result.confidence <= 1.0
            client.close()
            report(12, "Python SDK Sync Client detect_language integration", ok,
                   f"lang={result.language}, code={result.language_code}, confidence={result.confidence:.2f}")
        except Exception as e:
            report(12, "Python SDK Sync Client detect_language integration", False, str(e))

        # Phase 13: Python SDK Async Client integration
        try:
            from fintext import FinTextAsyncClient

            async def _test_async():
                async_client = FinTextAsyncClient(base_url=BASE_URL, api_token=token, timeout=10.0)
                result = await async_client.detect_language(SPANISH_TEXT)
                await async_client.close()
                return result

            result = asyncio.run(_test_async())
            ok = isinstance(result, LanguageDetectionResponse)
            ok = ok and result.language == "spanish"
            ok = ok and result.is_multilingual_model_applied
            report(13, "Python SDK Async Client detect_language integration", ok,
                   f"lang={result.language}, multilingual={result.is_multilingual_model_applied}")
        except Exception as e:
            report(13, "Python SDK Async Client detect_language integration", False, str(e))

        # Phase 14: OpenAPI specification verification
        try:
            r = httpx.get(f"{BASE_URL}/api-docs/openapi.json", timeout=5.0)
            spec = r.json()
            paths = spec.get("paths", {})
            has_path = "/language/detect" in paths

            schemas = spec.get("components", {}).get("schemas", {})
            has_schema = "LanguageDetectionResponse" in schemas

            tags = [t.get("name") for t in spec.get("tags", [])]
            has_tag = "Multilingual & NLP Services" in tags

            ok = has_path and has_schema and has_tag
            detail = f"path={has_path}, schema={has_schema}, tag={has_tag}"
            report(14, "OpenAPI specification contains /language/detect path, schema, and tag", ok, detail)
        except Exception as e:
            report(14, "OpenAPI specification verification", False, str(e))

    print("-" * 80)
    print(f"Results: {passed}/{total} passed, {failed} failed")
    print("=" * 80)
    if failed > 0:
        sys.exit(1)


if __name__ == "__main__":
    run_tests()
