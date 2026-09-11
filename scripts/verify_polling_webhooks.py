#!/usr/bin/env python3
"""
===============================================================================
FinText-Alpha-Vectorizer — Suite #223: Custom Polling Webhooks & Pull-Based Delivery Certification
===============================================================================
Verifies:
  1.  Unauthenticated POST /polling-webhooks returns 401 Unauthorized
  2.  Unauthenticated GET /polling-webhooks returns 401 Unauthorized
  3.  Invalid payload (empty name) returns 400 Bad Request
  4.  Invalid payload (insecure public HTTP URL) returns 400 Bad Request
  5.  Invalid interval (< 60s or > 86400s) returns 400 Bad Request
  6.  Invalid query_type (unsupported_type) returns 400 Bad Request
  7.  Successful creation of Sentiment Polling Webhook (201 Created, 64-char secret)
  8.  Successful creation of News Polling Webhook (201 Created)
  9.  Listing polling webhooks (GET /polling-webhooks -> 200 OK, total >= 2)
  10. Successful deletion of a polling webhook (DELETE /polling-webhooks/{id} -> 200 OK)
  11. Deleting non-existent webhook returns 404 Not Found
  12. Python SDK sync and async client integration (200 OK)
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

PORT = 8123
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


def get_auth_token(user_id: str = "polling_trader_01", role: str = "institutional") -> str:
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
    print("  FinText-Alpha-Vectorizer — Suite #223: Custom Polling Webhooks Certification")
    print("=" * 79)

    with ServerContext():
        token = get_auth_token("polling_manager_01", role="institutional")
        headers = {"Authorization": f"Bearer {token}"}
        created_ids = []

        # ── Phase 1: Unauthenticated POST rejection ───────────────────────────
        try:
            r = httpx.post(
                f"{BASE_URL}/polling-webhooks",
                json={"name": "Poll", "url": "https://quant.fund.com/poll", "interval_seconds": 300, "query_type": "sentiment"},
                timeout=5.0,
            )
            ok = r.status_code == 401
            report(1, "Unauthenticated POST /polling-webhooks returns 401 Unauthorized", ok, f"status={r.status_code}")
        except Exception as e:
            report(1, "Unauthenticated POST /polling-webhooks returns 401 Unauthorized", False, str(e))

        # ── Phase 2: Unauthenticated GET rejection ────────────────────────────
        try:
            r = httpx.get(f"{BASE_URL}/polling-webhooks", timeout=5.0)
            ok = r.status_code == 401
            report(2, "Unauthenticated GET /polling-webhooks returns 401 Unauthorized", ok, f"status={r.status_code}")
        except Exception as e:
            report(2, "Unauthenticated GET /polling-webhooks returns 401 Unauthorized", False, str(e))

        # ── Phase 3: Invalid payload (empty name) ─────────────────────────────
        try:
            r = httpx.post(
                f"{BASE_URL}/polling-webhooks",
                headers=headers,
                json={"name": "   ", "url": "https://quant.fund.com/poll", "interval_seconds": 300, "query_type": "sentiment"},
                timeout=5.0,
            )
            ok = r.status_code == 400
            report(3, "Invalid payload (empty name) returns 400 Bad Request", ok, f"status={r.status_code}")
        except Exception as e:
            report(3, "Invalid payload (empty name) returns 400 Bad Request", False, str(e))

        # ── Phase 4: Invalid payload (insecure public HTTP URL) ───────────────
        try:
            r = httpx.post(
                f"{BASE_URL}/polling-webhooks",
                headers=headers,
                json={"name": "Poll", "url": "http://insecure.fund.com/poll", "interval_seconds": 300, "query_type": "sentiment"},
                timeout=5.0,
            )
            ok = r.status_code == 400
            report(4, "Invalid payload (insecure public HTTP URL) returns 400 Bad Request", ok, f"status={r.status_code}")
        except Exception as e:
            report(4, "Invalid payload (insecure HTTP) returns 400 Bad Request", False, str(e))

        # ── Phase 5: Invalid interval bounds (< 60s or > 86400s) ──────────────
        try:
            r_low = httpx.post(
                f"{BASE_URL}/polling-webhooks",
                headers=headers,
                json={"name": "Poll", "url": "https://quant.fund.com/poll", "interval_seconds": 10, "query_type": "sentiment"},
                timeout=5.0,
            )
            r_high = httpx.post(
                f"{BASE_URL}/polling-webhooks",
                headers=headers,
                json={"name": "Poll", "url": "https://quant.fund.com/poll", "interval_seconds": 100000, "query_type": "sentiment"},
                timeout=5.0,
            )
            ok = r_low.status_code == 400 and r_high.status_code == 400
            report(5, "Invalid interval (< 60s or > 86400s) returns 400 Bad Request", ok)
        except Exception as e:
            report(5, "Invalid interval bounds returns 400 Bad Request", False, str(e))

        # ── Phase 6: Invalid query_type ───────────────────────────────────────
        try:
            r = httpx.post(
                f"{BASE_URL}/polling-webhooks",
                headers=headers,
                json={"name": "Poll", "url": "https://quant.fund.com/poll", "interval_seconds": 300, "query_type": "invalid_type"},
                timeout=5.0,
            )
            ok = r.status_code == 400
            report(6, "Invalid query_type (invalid_type) returns 400 Bad Request", ok, f"status={r.status_code}")
        except Exception as e:
            report(6, "Invalid query_type returns 400 Bad Request", False, str(e))

        # ── Phase 7: Successful creation of Sentiment Polling Webhook ─────────
        try:
            r = httpx.post(
                f"{BASE_URL}/polling-webhooks",
                headers=headers,
                json={
                    "name": "Tech Sentiment 5m Poll",
                    "url": "https://quant.fund.com/api/v1/sentiment-poll",
                    "interval_seconds": 300,
                    "query_type": "sentiment",
                    "query_params": {"tickers": ["AAPL", "MSFT", "NVDA"], "min_confidence": 0.6},
                },
                timeout=5.0,
            )
            ok = r.status_code == 201
            data = r.json()
            ok = ok and data.get("name") == "Tech Sentiment 5m Poll"
            ok = ok and len(data.get("secret", "")) == 64
            ok = ok and data.get("interval_seconds") == 300
            created_ids.append(data.get("id"))
            report(7, f"Successful creation of Sentiment Polling Webhook (201 Created, ID={data.get('id')})", ok)
        except Exception as e:
            report(7, "Successful creation of Sentiment Polling Webhook", False, str(e))

        # ── Phase 8: Successful creation of News Polling Webhook ──────────────
        try:
            r = httpx.post(
                f"{BASE_URL}/polling-webhooks",
                headers=headers,
                json={
                    "name": "Hourly Macro News Poll",
                    "url": "http://127.0.0.1:9000/api/news-poll",
                    "interval_seconds": 3600,
                    "query_type": "news",
                    "query_params": {"limit": 10},
                },
                timeout=5.0,
            )
            ok = r.status_code == 201
            data = r.json()
            ok = ok and data.get("name") == "Hourly Macro News Poll"
            ok = ok and len(data.get("secret", "")) == 64
            created_ids.append(data.get("id"))
            report(8, f"Successful creation of News Polling Webhook (201 Created, ID={data.get('id')})", ok)
        except Exception as e:
            report(8, "Successful creation of News Polling Webhook", False, str(e))

        # ── Phase 9: Listing polling webhooks ─────────────────────────────────
        try:
            r = httpx.get(f"{BASE_URL}/polling-webhooks", headers=headers, timeout=5.0)
            ok = r.status_code == 200
            data = r.json()
            ok = ok and data.get("total", 0) >= 2
            ok = ok and len(data.get("webhooks", [])) >= 2
            report(9, f"Listing polling webhooks (GET /polling-webhooks -> 200 OK, Total={data.get('total')})", ok)
        except Exception as e:
            report(9, "Listing polling webhooks", False, str(e))

        # ── Phase 10: Successful deletion of a polling webhook ────────────────
        target_id = created_ids[0] if created_ids else "00000000-0000-0000-0000-000000000000"
        try:
            r = httpx.delete(f"{BASE_URL}/polling-webhooks/{target_id}", headers=headers, timeout=5.0)
            ok = r.status_code == 200
            data = r.json()
            ok = ok and data.get("success") is True
            ok = ok and data.get("id") == target_id
            report(10, f"Successful deletion of polling webhook (DELETE /polling-webhooks/{target_id} -> 200 OK)", ok)
        except Exception as e:
            report(10, "Successful deletion of polling webhook", False, str(e))

        # ── Phase 11: Deleting non-existent webhook ───────────────────────────
        try:
            r = httpx.delete(f"{BASE_URL}/polling-webhooks/{target_id}", headers=headers, timeout=5.0)
            ok = r.status_code == 404
            report(11, f"Deleting already-deleted webhook returns 404 Not Found (status={r.status_code})", ok)
        except Exception as e:
            report(11, "Deleting already-deleted webhook returns 404 Not Found", False, str(e))

        # ── Phase 12: Python SDK sync & async client integration ──────────────
        try:
            from fintext import FinTextClient, FinTextAsyncClient

            token_sdk = get_auth_token("sdk_polling_user", role="institutional")
            with FinTextClient(base_url=BASE_URL, api_token=token_sdk) as client:
                wh = client.create_polling_webhook(
                    name="SDK Sync Options Poll",
                    url="https://quant.fund.com/sdk-poll",
                    interval_seconds=600,
                    query_type="options",
                    query_params={"tickers": ["NVDA"]},
                )
                ok_sync = wh.name == "SDK Sync Options Poll" and len(wh.secret) == 64
                listing = client.list_polling_webhooks()
                ok_sync = ok_sync and listing.total >= 1
                del_res = client.delete_polling_webhook(wh.id)
                ok_sync = ok_sync and del_res.success is True

            async def verify_async():
                token_async = get_auth_token("sdk_async_polling_user", role="institutional")
                async with FinTextAsyncClient(base_url=BASE_URL, api_token=token_async) as aclient:
                    wh_a = await aclient.create_polling_webhook(
                        name="SDK Async Events Poll",
                        url="https://quant.fund.com/async-poll",
                        interval_seconds=1200,
                        query_type="events",
                    )
                    ok_a = wh_a.name == "SDK Async Events Poll" and len(wh_a.secret) == 64
                    listing_a = await aclient.list_polling_webhooks()
                    ok_a = ok_a and listing_a.total >= 1
                    del_a = await aclient.delete_polling_webhook(wh_a.id)
                    ok_a = ok_a and del_a.success is True
                    return ok_a

            ok_async = asyncio.run(verify_async())
            report(12, "Python SDK sync & async client integration (create/list/delete -> 200 OK)", ok_sync and ok_async)
        except Exception as e:
            report(12, "Python SDK sync & async client integration", False, str(e))

    # ── Summary ──────────────────────────────────────────────────────────────
    print("\n" + "=" * 79)
    pct = (passed / total) * 100.0
    print(f"  Suite #223 Results: {passed}/{total} Passed ({pct:.1f}%)")
    print("=" * 79)
    if failed == 0:
        print("\n🎉 ALL 12 PHASES PASSED CLEANLY!\n")
    else:
        print(f"\n❌ {failed} PHASES FAILED!\n")
        sys.exit(1)


if __name__ == "__main__":
    main()
