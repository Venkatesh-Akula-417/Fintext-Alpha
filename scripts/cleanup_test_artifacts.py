#!/usr/bin/env python3
"""Post-test cleanup of ephemeral artifacts to prevent audit clutter."""
import shutil
import sys
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parent.parent

CLEANUP_PATHS = [
    PROJECT_ROOT / "rust" / "ingestion_engine" / "data" / "test_quarantine",
    PROJECT_ROOT / "rust" / "api_server" / "data" / "pit-cert-archive",
    PROJECT_ROOT / "data" / "test_quarantine",
    PROJECT_ROOT / "data" / "test_archive",
]

TMP_PATTERNS = ["*.tmp", "*.bak", "*.old"]

def cleanup():
    total_removed = 0
    for path in CLEANUP_PATHS:
        if path.exists():
            shutil.rmtree(path)
            print(f"[OK] Removed directory: {path}")
            total_removed += 1
    for pattern in TMP_PATTERNS:
        for tmp in PROJECT_ROOT.rglob(pattern):
            tmp.unlink()
            print(f"[OK] Removed file: {tmp}")
            total_removed += 1
    print(f"\nCleanup complete. Total items removed: {total_removed}")

if __name__ == "__main__":
    cleanup()
    sys.exit(0)
