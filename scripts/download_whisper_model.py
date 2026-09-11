"""
═══════════════════════════════════════════════════════════════════════════════
FinText-Alpha-Vectorizer — Whisper.cpp Model Downloader Utility
═══════════════════════════════════════════════════════════════════════════════
"""

import os
from pathlib import Path
import sys
import urllib.request

PROJECT_ROOT = Path(__file__).resolve().parent.parent
WHISPER_DIR = PROJECT_ROOT / "models" / "whisper"
WHISPER_DIR.mkdir(parents=True, exist_ok=True)

MODEL_URLS = {
    "tiny.en": "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.en.bin",
    "base.en": "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en.bin",
    "small.en": "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.en.bin",
}


def download_model(model_name="base.en"):
    if model_name not in MODEL_URLS:
        print(f"[Error] Unknown model name '{model_name}'. Available: {list(MODEL_URLS.keys())}")
        return False

    target_file = WHISPER_DIR / f"ggml-{model_name}.bin"
    if target_file.exists() and target_file.stat().st_size > 10_000_000:
        print(f"[Info] Model already exists at {target_file} ({target_file.stat().st_size / 1024 / 1024:.1f} MB)")
        return True

    url = MODEL_URLS[model_name]
    print(f"[Info] Downloading Whisper '{model_name}' model from {url}...")
    print(f"[Info] Destination: {target_file}")

    try:
        urllib.request.urlretrieve(url, target_file)
        print(f"[Success] Downloaded {target_file} ({target_file.stat().st_size / 1024 / 1024:.1f} MB)")
        return True
    except Exception as e:
        print(f"[Error] Failed to download model: {e}")
        return False


if __name__ == "__main__":
    model = sys.argv[1] if len(sys.argv) > 1 else "base.en"
    download_model(model)
