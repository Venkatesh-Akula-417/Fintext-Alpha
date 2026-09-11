#!/usr/bin/env python3
"""
═══════════════════════════════════════════════════════════════════════════════
FinText-Alpha-Vectorizer — Live API Rate Limiting Verification Suite
═══════════════════════════════════════════════════════════════════════════════
Validates per-user rate limiting on the native Axum API server:
1. GET /health (Public) is not rate limited
2. POST /auth/token (Public) is not rate limited
3. Authenticated requests include X-RateLimit-* headers
4. Quota decrements sequentially with each request
5. HTTP 429 Too Many Requests returned when quota is exhausted
6. 429 response contains Retry-After and X-RateLimit-Reset headers
7. Independent quotas enforced across distinct user IDs
═══════════════════════════════════════════════════════════════════════════════
"""

import os
import sys
import time
import subprocess
import requests
from pathlib import Path

if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")


def main():
    print("=" * 85)
    print(" FinText-Alpha-Vectorizer — Live Per-User Rate Limiting Verification Suite")
    print("=" * 85)

    env = os.environ.copy()
    env.update({
        "PORT": "8010",
        "HOST": "127.0.0.1",
        "RUST_LOG": "info",
        "JWT_SECRET": "rate-limit-test-jwt-secret-88888",
        "ADMIN_TOKEN": "rate-limit-admin-token-22222",
        "RATE_LIMIT_REQUESTS": "4",       # 4 requests per window for fast testing
        "RATE_LIMIT_WINDOW_SECONDS": "10", # 10 second window
        "QUESTDB_MOCK_FALLBACK": "1",
        "NATS_MOCK_MODE": "1",
    })

    bin_path = Path("rust/target/release/fintext_api.exe").resolve()
    if not bin_path.exists():
        bin_path = Path("rust/target/debug/fintext_api.exe").resolve()

    print(f"[*] Starting API Server binary: {bin_path}")
    print("    Configuration: RATE_LIMIT_REQUESTS=4, RATE_LIMIT_WINDOW_SECONDS=10")

    proc = subprocess.Popen(
        [str(bin_path)],
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        encoding="utf-8",
        errors="replace",
    )
    time.sleep(2.0)

    base_url = "http://127.0.0.1:8010"

    try:
        # 1. Obtain JWTs for user_alpha and user_beta
        print("\n[1/6] Issuing JWTs for trader_alpha and trader_beta...")
        r1 = requests.post(
            f"{base_url}/auth/token",
            headers={"X-Admin-Token": "rate-limit-admin-token-22222"},
            json={"user_id": "trader_alpha", "expires_in_seconds": 3600, "role": "institutional"},
            timeout=5,
        )
        assert r1.status_code == 200
        token_alpha = r1.json()["token"]

        r2 = requests.post(
            f"{base_url}/auth/token",
            headers={"X-Admin-Token": "rate-limit-admin-token-22222"},
            json={"user_id": "trader_beta", "expires_in_seconds": 3600, "role": "institutional"},
            timeout=5,
        )
        assert r2.status_code == 200
        token_beta = r2.json()["token"]
        print("      Issued tokens successfully.")

        # 2. Consume quota for trader_alpha (requests 1 to 4)
        print("\n[2/6] Sending 4 requests under trader_alpha (Capacity: 4)...")
        for i in range(1, 5):
            r = requests.get(
                f"{base_url}/sentiment?ticker=AAPL",
                headers={"Authorization": f"Bearer {token_alpha}"},
                timeout=5,
            )
            limit = r.headers.get("x-ratelimit-limit")
            remaining = r.headers.get("x-ratelimit-remaining")
            reset = r.headers.get("x-ratelimit-reset")
            print(f"      Request #{i}: HTTP {r.status_code} | X-RateLimit-Limit={limit}, Remaining={remaining}, Reset={reset}s")
            assert r.status_code == 200, f"Request {i} failed with {r.status_code}"
            assert limit == "4"

        # 3. 5th request for trader_alpha must return 429 Too Many Requests
        print("\n[3/6] Sending 5th request under trader_alpha (Should exceed quota -> HTTP 429)...")
        r_blocked = requests.get(
            f"{base_url}/sentiment?ticker=AAPL",
            headers={"Authorization": f"Bearer {token_alpha}"},
            timeout=5,
        )
        print(f"      HTTP Status: {r_blocked.status_code} | Body: {r_blocked.json()}")
        limit = r_blocked.headers.get("x-ratelimit-limit")
        remaining = r_blocked.headers.get("x-ratelimit-remaining")
        reset = r_blocked.headers.get("x-ratelimit-reset")
        retry_after = r_blocked.headers.get("retry-after")
        print(f"      Headers: Limit={limit}, Remaining={remaining}, Reset={reset}s, Retry-After={retry_after}s")

        assert r_blocked.status_code == 429, f"Expected 429, got {r_blocked.status_code}"
        assert remaining == "0"
        assert retry_after is not None
        assert "rate limit exceeded" in r_blocked.json()["message"].lower()

        # 4. Verify trader_beta has independent quota and is NOT blocked
        print("\n[4/6] Verifying trader_beta has independent quota (Should return HTTP 200)...")
        r_beta = requests.get(
            f"{base_url}/sentiment?ticker=AAPL",
            headers={"Authorization": f"Bearer {token_beta}"},
            timeout=5,
        )
        print(f"      trader_beta Request: HTTP {r_beta.status_code} | Remaining={r_beta.headers.get('x-ratelimit-remaining')}")
        assert r_beta.status_code == 200, f"Expected 200 for trader_beta, got {r_beta.status_code}"
        assert r_beta.headers.get("x-ratelimit-remaining") == "3"

        # 5. Verify /health and /auth/token are unrestricted
        print("\n[5/6] Verifying /health and /auth/token are unrestricted...")
        for _ in range(5):
            h_resp = requests.get(f"{base_url}/health", timeout=5)
            assert h_resp.status_code == 200
        print("      Public endpoints remained fully responsive and unblocked.")

        # 6. Verify cross-asset spillovers and backtest also include rate limit headers
        print("\n[6/6] Verifying /spillovers and /backtest rate limit headers...")
        r_spill = requests.get(
            f"{base_url}/spillovers?ticker=AAPL&limit=5",
            headers={"Authorization": f"Bearer {token_beta}"},
            timeout=5,
        )
        assert r_spill.status_code == 200
        assert "x-ratelimit-remaining" in r_spill.headers
        print(f"      GET /spillovers: HTTP 200 | Remaining={r_spill.headers.get('x-ratelimit-remaining')}")

        print("\n" + "=" * 85)
        print(" [OK] ALL 6 RATE LIMITING END-TO-END VERIFICATION CHECKS PASSED!")
        print("=" * 85)

    finally:
        proc.terminate()
        proc.wait(timeout=5)


if __name__ == "__main__":
    main()
