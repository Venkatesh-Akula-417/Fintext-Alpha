#!/usr/bin/env python3
"""
=====================================================================================
FinText-Alpha-Vectorizer — In-Memory Cache TTL & Capacity Governance Verification
Suite #272: Validates In-Memory Caching, TTL Expiration, Capacity Bounding & Cleanup
=====================================================================================
Validates:
 1. Configuration non-breaking defaults in config/config.yaml (cache section).
 2. Environment template documentation in .env.example (all 11 CACHE_* vars).
 3. Native Rust unit tests for api_server (cache module: 9 unit tests).
 4. TTL Expiration & Dynamic Update Verification (expired keys, get_or_insert_with).
 5. Maximum Capacity Bounding & Eviction Verification (bounded capacity, oldest evicted).
 6. Monthly Quota Cache Integration (MonthlyQuotaCache backed by TtlCache).
 7. Per-User Rate Limiter Bounded Buckets Integration (PerUserRateLimiter bounded).
 8. Provider Health Store & SCD2 Revision Registry Cache Integration.
 9. Live API Server startup, Cache Governance banner emission & graceful shutdown.
=====================================================================================
"""

import os
import sys
import time
import signal
import subprocess
from pathlib import Path
import yaml

# Ensure UTF-8 output on Windows
if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
if hasattr(sys.stderr, "reconfigure"):
    sys.stderr.reconfigure(encoding="utf-8", errors="replace")

PROJECT_ROOT = Path(__file__).resolve().parent.parent
CONFIG_FILE = PROJECT_ROOT / "config" / "config.yaml"
ENV_EXAMPLE = PROJECT_ROOT / ".env.example"
LIBCLANG_PATH = PROJECT_ROOT / "venv" / "Lib" / "site-packages" / "clang" / "native"
API_SERVER_EXE = PROJECT_ROOT / "rust" / "target" / "debug" / "fintext_api.exe"

passed = 0
failed = 0
total = 9


def report(phase: int, name: str, ok: bool, detail: str = ""):
    global passed, failed
    if ok:
        passed += 1
        status = "[PASS] OK"
    else:
        failed += 1
        status = "[FAIL] X "
    print(f"  Phase {phase:02d} [{status}] {name}", flush=True)
    if detail:
        for line in detail.strip().splitlines():
            print(f"            {line}", flush=True)


def test_01_config_defaults():
    name = "Configuration Non-Breaking Defaults (config.yaml)"
    try:
        assert CONFIG_FILE.exists(), f"Missing config file: {CONFIG_FILE}"
        with open(CONFIG_FILE, "r", encoding="utf-8") as f:
            cfg = yaml.safe_load(f)

        cache = cfg.get("cache", {})
        assert cache, "Missing 'cache' section in config.yaml"

        default_ttl = cache.get("default_ttl_seconds")
        default_max_cap = cache.get("default_max_capacity")
        cleanup_interval = cache.get("cleanup_interval_seconds")
        quota_ttl = cache.get("quota_cache_ttl_seconds")
        quota_cap = cache.get("quota_cache_max_capacity")
        rate_limit_ttl = cache.get("rate_limit_bucket_ttl_seconds")
        rate_limit_cap = cache.get("rate_limit_max_buckets")
        ph_ttl = cache.get("provider_health_ttl_seconds")
        ph_cap = cache.get("provider_health_max_capacity")
        scd2_ttl = cache.get("scd2_cache_ttl_seconds")
        scd2_cap = cache.get("scd2_cache_max_capacity")

        assert default_ttl == 300, f"default_ttl_seconds expected 300, got {default_ttl}"
        assert default_max_cap == 10000, f"default_max_capacity expected 10000, got {default_max_cap}"
        assert cleanup_interval == 60, f"cleanup_interval_seconds expected 60, got {cleanup_interval}"
        assert quota_ttl == 300, f"quota_cache_ttl_seconds expected 300, got {quota_ttl}"
        assert quota_cap == 10000, f"quota_cache_max_capacity expected 10000, got {quota_cap}"
        assert rate_limit_ttl == 300, f"rate_limit_bucket_ttl_seconds expected 300, got {rate_limit_ttl}"
        assert rate_limit_cap == 10000, f"rate_limit_max_buckets expected 10000, got {rate_limit_cap}"
        assert ph_ttl == 60, f"provider_health_ttl_seconds expected 60, got {ph_ttl}"
        assert ph_cap == 1000, f"provider_health_max_capacity expected 1000, got {ph_cap}"
        assert scd2_ttl == 300, f"scd2_cache_ttl_seconds expected 300, got {scd2_ttl}"
        assert scd2_cap == 5000, f"scd2_cache_max_capacity expected 5000, got {scd2_cap}"

        report(1, name, True, f"All 11 cache config defaults verified: default_ttl={default_ttl}s, max_cap={default_max_cap}")
    except Exception as e:
        report(1, name, False, str(e))


def test_02_env_example():
    name = "Environment Template Documentation (.env.example)"
    try:
        assert ENV_EXAMPLE.exists(), f"Missing .env.example: {ENV_EXAMPLE}"
        content = ENV_EXAMPLE.read_text(encoding="utf-8")

        required_vars = [
            "CACHE_DEFAULT_TTL_SECONDS=300",
            "CACHE_DEFAULT_MAX_CAPACITY=10000",
            "CACHE_CLEANUP_INTERVAL_SECONDS=60",
            "QUOTA_CACHE_TTL_SECONDS=300",
            "QUOTA_CACHE_MAX_CAPACITY=10000",
            "RATE_LIMIT_BUCKET_TTL_SECONDS=300",
            "RATE_LIMIT_MAX_BUCKETS=10000",
            "PROVIDER_HEALTH_CACHE_TTL_SECONDS=60",
            "PROVIDER_HEALTH_CACHE_MAX_CAPACITY=1000",
            "SCD2_CACHE_TTL_SECONDS=300",
            "SCD2_CACHE_MAX_CAPACITY=5000",
        ]

        missing = [v for v in required_vars if v not in content]
        assert not missing, f"Missing cache environment variables in .env.example: {missing}"

        report(2, name, True, f"All {len(required_vars)} cache environment variables documented in .env.example")
    except Exception as e:
        report(2, name, False, str(e))


def test_03_rust_unit_tests():
    name = "Native Rust Unit Tests for Cache Governance Module"
    try:
        cmd = [
            "cargo", "test",
            "--manifest-path", str(PROJECT_ROOT / "rust" / "Cargo.toml"),
            "-p", "fintext_api_server",
            "--lib", "cache::tests",
        ]
        res = subprocess.run(cmd, capture_output=True, text=True, timeout=120)
        out = res.stdout + "\n" + res.stderr
        assert res.returncode == 0, f"cargo test failed with code {res.returncode}:\n{out[:500]}"
        assert "9 passed" in out or "test result: ok" in out, f"Unexpected test output:\n{out[:500]}"

        report(3, name, True, "All 9 native Rust cache unit tests passed")
    except Exception as e:
        report(3, name, False, str(e))


def test_04_ttl_expiration():
    name = "TTL Expiration & Dynamic Value Update Verification"
    try:
        cmd = [
            "cargo", "test",
            "--manifest-path", str(PROJECT_ROOT / "rust" / "Cargo.toml"),
            "-p", "fintext_api_server",
            "--lib", "test_ttl_cache_expiration",
        ]
        res = subprocess.run(cmd, capture_output=True, text=True, timeout=60)
        assert res.returncode == 0, f"TTL expiration test failed:\n{res.stdout}\n{res.stderr}"

        cmd2 = [
            "cargo", "test",
            "--manifest-path", str(PROJECT_ROOT / "rust" / "Cargo.toml"),
            "-p", "fintext_api_server",
            "--lib", "test_ttl_cache_update",
        ]
        res2 = subprocess.run(cmd2, capture_output=True, text=True, timeout=60)
        assert res2.returncode == 0, f"TTL cache update test failed:\n{res2.stdout}\n{res2.stderr}"

        report(4, name, True, "TTL expiration correctly evicts expired items and update refreshes timestamp")
    except Exception as e:
        report(4, name, False, str(e))


def test_05_capacity_bounding():
    name = "Maximum Capacity Bounding & FIFO/Oldest Eviction Verification"
    try:
        cmd = [
            "cargo", "test",
            "--manifest-path", str(PROJECT_ROOT / "rust" / "Cargo.toml"),
            "-p", "fintext_api_server",
            "--lib", "test_ttl_cache_capacity_eviction",
        ]
        res = subprocess.run(cmd, capture_output=True, text=True, timeout=60)
        assert res.returncode == 0, f"Capacity eviction test failed:\n{res.stdout}\n{res.stderr}"

        report(5, name, True, "Capacity ceiling strictly enforced; oldest items evicted on capacity overflow")
    except Exception as e:
        report(5, name, False, str(e))


def test_06_monthly_quota_cache():
    name = "Monthly Quota Cache Integration (TtlCache-backed)"
    try:
        cmd = [
            "cargo", "test",
            "--manifest-path", str(PROJECT_ROOT / "rust" / "Cargo.toml"),
            "-p", "fintext_api_server",
            "--lib", "billing::tests::test_monthly_quota_cache",
        ]
        res = subprocess.run(cmd, capture_output=True, text=True, timeout=60)
        assert res.returncode == 0, f"Monthly quota cache test failed:\n{res.stdout}\n{res.stderr}"

        report(6, name, True, "MonthlyQuotaCache passes basic, increment, expiry, remove, and clear tests")
    except Exception as e:
        report(6, name, False, str(e))


def test_07_rate_limiter_buckets():
    name = "Per-User Rate Limiter Bounded Buckets Integration"
    try:
        cmd = [
            "cargo", "test",
            "--manifest-path", str(PROJECT_ROOT / "rust" / "Cargo.toml"),
            "-p", "fintext_api_server",
            "--lib", "rate_limit::tests",
        ]
        res = subprocess.run(cmd, capture_output=True, text=True, timeout=60)
        assert res.returncode == 0, f"Rate limiter test failed:\n{res.stdout}\n{res.stderr}"

        report(7, name, True, "PerUserRateLimiter operates correctly with TtlCache bounded storage")
    except Exception as e:
        report(7, name, False, str(e))


def test_08_provider_health_and_scd2():
    name = "Provider Health Store & SCD2 Revision Registry Cache Integration"
    try:
        cmd = [
            "cargo", "test",
            "--manifest-path", str(PROJECT_ROOT / "rust" / "Cargo.toml"),
            "-p", "fintext_api_server",
            "--lib", "test_provider_health",
        ]
        res = subprocess.run(cmd, capture_output=True, text=True, timeout=60)
        assert res.returncode == 0, f"Provider health tests failed:\n{res.stdout}\n{res.stderr}"

        cmd2 = [
            "cargo", "test",
            "--manifest-path", str(PROJECT_ROOT / "rust" / "Cargo.toml"),
            "-p", "fintext_api_server",
            "--lib", "scd2::tests",
        ]
        res2 = subprocess.run(cmd2, capture_output=True, text=True, timeout=60)
        assert res2.returncode == 0, f"SCD2 tests failed:\n{res2.stdout}\n{res2.stderr}"

        report(8, name, True, "ProviderHealthStore and SCD2 active cache operate seamlessly with TTL caching")
    except Exception as e:
        report(8, name, False, str(e))


def test_09_api_server_startup_and_banner():
    name = "Live API Server Startup & Cache Governance Banner Emission"
    try:
        assert API_SERVER_EXE.exists(), f"API server binary not found: {API_SERVER_EXE}"

        env = os.environ.copy()
        env["PORT"] = "8098"
        env["HOST"] = "127.0.0.1"
        env["JWT_SECRET"] = "verification_suite_secret_key_32_bytes_len!"
        env["ADMIN_TOKEN"] = "test_admin_token_xyz123_valid_32_bytes_length!"
        env["RUST_LOG"] = "info"
        env["QUESTDB_MOCK_FALLBACK"] = "1"
        env["POLYGON_MOCK_FALLBACK"] = "1"
        env["WHISPER_MOCK_FALLBACK"] = "1"
        env["TIMESCALE_MOCK_MODE"] = "1"
        env["KAFKA_MOCK_MODE"] = "1"
        env["KAFKA_MOCK_FALLBACK"] = "1"
        env["CACHE_DEFAULT_TTL_SECONDS"] = "120"
        env["CACHE_DEFAULT_MAX_CAPACITY"] = "5000"
        env["CACHE_CLEANUP_INTERVAL_SECONDS"] = "30"

        proc = subprocess.Popen(
            [str(API_SERVER_EXE)],
            cwd=str(PROJECT_ROOT),
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            encoding="utf-8",
            errors="replace",
            env=env,
        )

        banner_found = False
        captured = []
        start_time = time.time()

        while time.time() - start_time < 20:
            line = proc.stdout.readline()
            if line:
                captured.append(line)
                if "[Cache Governance]" in line and "Initialized:" in line:
                    banner_found = True
                    break
            elif proc.poll() is not None:
                break
            else:
                time.sleep(0.05)

        proc.terminate()
        try:
            rem_stdout, _ = proc.communicate(timeout=5)
            if rem_stdout:
                captured.append(rem_stdout)
        except subprocess.TimeoutExpired:
            proc.kill()
            rem_stdout, _ = proc.communicate()
            if rem_stdout:
                captured.append(rem_stdout)

        stdout = "".join(captured)
        assert banner_found, f"Cache Governance banner not found in output:\n{stdout[:1500]}"
        report(9, name, True, "API server successfully emitted [Cache Governance] startup banner")
    except Exception as e:
        report(9, name, False, str(e))


def main():
    print("=" * 85)
    print(" FinText-Alpha-Vectorizer — In-Memory Cache TTL & Capacity Governance Verification")
    print(" Suite #272: Validates TTL Expiration, Capacity Bounding, & System-Wide Cache Policies")
    print("=" * 85)

    test_01_config_defaults()
    test_02_env_example()
    test_03_rust_unit_tests()
    test_04_ttl_expiration()
    test_05_capacity_bounding()
    test_06_monthly_quota_cache()
    test_07_rate_limiter_buckets()
    test_08_provider_health_and_scd2()
    test_09_api_server_startup_and_banner()

    print("-" * 85)
    print(f"Results: {passed}/{total} passed, {failed} failed")
    if failed == 0:
        print("[SUCCESS] Suite #272: In-Memory Cache TTL & Capacity Governance VERIFIED.")
        return 0
    else:
        print("[FAILURE] Suite #272: Verification FAILED.")
        return 1


if __name__ == "__main__":
    sys.exit(main())
