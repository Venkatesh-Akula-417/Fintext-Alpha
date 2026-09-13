"""Model asset fetcher for FinText CI. Stdlib only. Idempotent."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import sys
import tarfile
import tempfile
from typing import Any, Dict, Optional, Tuple
from urllib import error as urlerror
from urllib import parse as urlparse
from urllib import request as urlrequest
import zipfile


def load_manifest(path: Path) -> Dict[str, Any]:
    """Load and validate models manifest JSON. Exits with code 2 on missing or invalid manifest."""
    manifest_path = Path(path)
    if not manifest_path.is_file():
        print(f"ERROR: Manifest file not found at '{manifest_path}'", file=sys.stderr)
        sys.exit(2)

    try:
        with open(manifest_path, "r", encoding="utf-8") as f:
            data = json.load(f)
    except Exception as exc:
        print(f"ERROR: Failed to parse manifest JSON at '{manifest_path}': {exc}", file=sys.stderr)
        sys.exit(2)

    if not isinstance(data, dict) or "assets" not in data or not isinstance(data["assets"], list):
        print(f"ERROR: Invalid manifest format in '{manifest_path}': expected object with 'assets' list", file=sys.stderr)
        sys.exit(2)

    return data


def sha256_file(path: Path) -> str:
    """Compute lowercase hexadecimal SHA256 digest of a file in streaming chunks."""
    h = hashlib.sha256()
    with open(path, "rb") as f:
        while chunk := f.read(65536):
            h.update(chunk)
    return h.hexdigest().lower()


def all_required_files_present(asset: Dict[str, Any]) -> bool:
    """Check if all required files for an asset exist and are non-empty."""
    target_dir = Path(asset.get("target_dir", "."))
    if not target_dir.is_dir():
        return False

    required_files = asset.get("required_files", [])
    if not required_files:
        return False

    for filename in required_files:
        filepath = target_dir / filename
        if not filepath.is_file() or filepath.stat().st_size == 0:
            return False
    return True


def verify_asset(asset: Dict[str, Any]) -> Tuple[bool, str]:
    """Verify that target directory exists and all required files are present and non-empty."""
    target_dir = Path(asset.get("target_dir", "."))
    if not target_dir.is_dir():
        return False, f"Target directory '{target_dir}' does not exist"

    required_files = asset.get("required_files", [])
    if not required_files:
        return False, f"Asset '{asset.get('id')}' has no required_files declared"

    for filename in required_files:
        filepath = target_dir / filename
        if not filepath.is_file():
            return False, f"Required file '{filename}' missing in '{target_dir}'"
        if filepath.stat().st_size == 0:
            return False, f"Required file '{filename}' in '{target_dir}' is 0 bytes"

    return True, "All required files verified"


def download_asset(asset: Dict[str, Any], workdir: Path) -> Tuple[bool, str]:
    """
    Download asset URL to a temporary file in workdir and verify SHA256 if configured.
    Returns (True, downloaded_file_path) on success.
    Returns (False, error_reason) on failure. Does not raise.
    """
    url = asset.get("url")
    if not url:
        return False, "URL is not configured"

    try:
        workdir.mkdir(parents=True, exist_ok=True)
        url_path = urlparse.urlparse(url).path
        filename = os.path.basename(url_path) or f"{asset.get('id', 'asset')}.download"
        dest_path = workdir / filename

        req = urlrequest.Request(
            url,
            headers={"User-Agent": "FinText-ModelFetcher/1.0"},
        )
        with urlrequest.urlopen(req, timeout=120) as response:
            with open(dest_path, "wb") as f:
                shutil.copyfileobj(response, f, length=65536)

        expected_hash = asset.get("sha256")
        if expected_hash:
            actual_hash = sha256_file(dest_path)
            if actual_hash.lower() != expected_hash.strip().lower():
                return False, f"SHA256 mismatch: expected {expected_hash}, got {actual_hash}"

        return True, str(dest_path)
    except urlerror.HTTPError as exc:
        return False, f"HTTP error {exc.code}: {exc.reason}"
    except urlerror.URLError as exc:
        return False, f"Network error: {exc.reason}"
    except Exception as exc:
        return False, f"Download error: {exc}"


def extract_archive(archive_path: Path, target_dir: Path, fmt: str) -> bool:
    """
    Extract archive or place single file into target_dir.
    Supported formats: 'none', 'zip', 'tar.gz' (also 'tgz', 'tar').
    """
    try:
        target_dir.mkdir(parents=True, exist_ok=True)
        archive_path = Path(archive_path)

        if fmt == "none":
            shutil.copy2(archive_path, target_dir / archive_path.name)
            return True
        elif fmt == "zip":
            with zipfile.ZipFile(archive_path, "r") as zf:
                zf.extractall(target_dir)
            return True
        elif fmt in ("tar.gz", "tgz", "tar"):
            mode = "r:gz" if fmt in ("tar.gz", "tgz") else "r:*"
            with tarfile.open(archive_path, mode) as tf:
                tf.extractall(target_dir)
            return True
        else:
            print(f"FAIL: Unsupported archive_format '{fmt}'", file=sys.stderr)
            return False
    except Exception as exc:
        print(f"FAIL: Archive extraction failed: {exc}", file=sys.stderr)
        return False


def process_asset(asset: Dict[str, Any], workdir: Path, force: bool) -> str:
    """
    Process a single model asset.
    Returns: 'skipped' | 'already_present' | 'fetched' | 'error'
    """
    asset_id = asset.get("id", "unknown")
    url = asset.get("url")

    # 1. If url is null -> skip
    if not url:
        print(f"SKIP: URL not configured for '{asset_id}'")
        return "skipped"

    # 2. If all required files present and not --force -> already present
    if all_required_files_present(asset) and not force:
        print(f"OK: '{asset_id}' already present")
        return "already_present"

    # 3. Download asset
    print(f"DOWNLOADING: '{asset_id}' from {url}...")
    ok, result = download_asset(asset, workdir)
    if not ok:
        print(f"FAIL: '{asset_id}' download/verification failed: {result}")
        return "error"

    downloaded_file = Path(result)
    target_dir = Path(asset.get("target_dir", "."))
    fmt = asset.get("archive_format", "none")

    # 4. Extract or place files
    extracted = extract_archive(downloaded_file, target_dir, fmt)
    if not extracted:
        print(f"FAIL: '{asset_id}' extraction failed for format '{fmt}'")
        return "error"

    # 5. Verify placed files
    verified, reason = verify_asset(asset)
    if not verified:
        print(f"FAIL: '{asset_id}' verification failed: {reason}")
        return "error"

    print(f"FETCHED: '{asset_id}' successfully installed to '{target_dir}'")
    return "fetched"


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Model asset fetcher for FinText CI. Stdlib only. Idempotent."
    )
    parser.add_argument(
        "--manifest",
        default="config/models_manifest.json",
        help="Path to models manifest JSON (default: config/models_manifest.json)",
    )
    parser.add_argument(
        "--force",
        action="store_true",
        help="Force re-download even if all required files are present",
    )
    parser.add_argument(
        "--only",
        default=None,
        help="Download only a single asset by id",
    )
    args = parser.parse_args()

    manifest_path = Path(args.manifest)
    manifest = load_manifest(manifest_path)

    assets = manifest.get("assets", [])
    if not isinstance(assets, list):
        print(f"ERROR: Invalid manifest format: 'assets' must be a list.", file=sys.stderr)
        return 2

    if args.only:
        filtered = [a for a in assets if a.get("id") == args.only]
        if not filtered:
            print(f"ERROR: Asset '{args.only}' not found in manifest '{manifest_path}'.", file=sys.stderr)
            return 2
        assets = filtered

    has_error = False
    with tempfile.TemporaryDirectory(prefix="fintext_models_") as tmpdir:
        workdir = Path(tmpdir)
        for asset in assets:
            status = process_asset(asset, workdir, force=args.force)
            if status == "error":
                has_error = True

    return 1 if has_error else 0


if __name__ == "__main__":
    sys.exit(main())
