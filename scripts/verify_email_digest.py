"""
===============================================================================
FinText-Alpha-Vectorizer — Suite #215: Email Digest Service & Background Worker
===============================================================================

12-phase certification covering:
  1. Unauthenticated rejection (401) on digest endpoints
  2. Initial GET /digest/subscription returns 404 Not Found
  3. Create daily subscription (POST /digest/subscription -> 200 OK)
  4. Retrieve subscription (GET /digest/subscription) & verify fields
  5. Update subscription to weekly frequency with customized tickers/sectors
  6. Validation errors (invalid frequency, invalid event types -> 400 Bad Request)
  7. On-demand digest compilation and delivery (POST /digest/trigger)
  8. Content verification (sentiment signals, catalyst events, full-text news)
  9. Multi-user subscription isolation & security
 10. Delete subscription (DELETE /digest/subscription -> 200 OK, verify 404)
 11. Python SDK sync client integration
 12. Python SDK async client integration
"""

import asyncio
import os
import subprocess
import sys
import time
import uuid
from pathlib import Path

import httpx

# ── Project bootstrap ────────────────────────────────────────────────────────
PROJECT_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(PROJECT_ROOT / "python_sdk" / "src"))

if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")

SERVER_EXE = PROJECT_ROOT / "rust" / "target" / "debug" / "fintext_api.exe"
PORT = 8115
BASE_URL = f"http://127.0.0.1:{PORT}"
ADMIN_TOKEN = "dev_admin_secret_token_215"

passed = 0
failed = 0
total  = 0


def report(phase: int, title: str, ok: bool, detail: str = ""):
    global passed, failed, total
    total += 1
    if ok:
        passed += 1
        print(f"  ✅ Phase {phase:>2d} │ {title}")
    else:
        failed += 1
        msg = f" — {detail}" if detail else ""
        print(f"  ❌ Phase {phase:>2d} │ {title}{msg}")


class ServerContext:
    def __init__(self):
        self.process = None

    def __enter__(self):
        print(f"[STARTING] Spawning FinText API Server on port {PORT}...")
        env = os.environ.copy()
        env["PORT"] = str(PORT)
        env["HOST"] = "127.0.0.1"
        env["ADMIN_TOKEN"] = ADMIN_TOKEN
        env["JWT_SECRET"] = "dev_jwt_secret_key_suite_215_testing_digest_service"
        env["QUESTDB_MOCK_FALLBACK"] = "1"
        env["DIGEST_WORKER_INTERVAL_SECS"] = "3600"

        self.process = subprocess.Popen(
            [str(SERVER_EXE)],
            env=env,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )

        # Wait for server readiness
        deadline = time.time() + 20
        while time.time() < deadline:
            try:
                r = httpx.get(f"{BASE_URL}/health", timeout=1.0)
                if r.status_code == 200:
                    print("[READY] FinText API Server is responding to health checks.\n")
                    return self
            except Exception:
                time.sleep(0.3)

        raise RuntimeError("FinText API Server failed to start within 20s timeout.")

    def __exit__(self, exc_type, exc_val, exc_tb):
        if self.process:
            print("\n[STOPPING] Terminating FinText API Server process...")
            self.process.terminate()
            try:
                self.process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.process.kill()


def get_auth_token(user_id: str = "digest_trader_01") -> str:
    r = httpx.post(
        f"{BASE_URL}/auth/token",
        json={"user_id": user_id, "expires_in_seconds": 3600},
        headers={"X-Admin-Token": ADMIN_TOKEN},
        timeout=5.0,
    )
    if r.status_code != 200:
        raise RuntimeError(f"Failed to obtain auth token: {r.status_code} - {r.text}")
    return r.json()["token"]


def main():
    print("=" * 79)
    print("  FinText-Alpha-Vectorizer — Suite #215: Email Digest Service Certification")
    print("=" * 79)

    with ServerContext():
        token_u1 = get_auth_token("digest_user_alpha")
        token_u2 = get_auth_token("digest_user_beta")
        headers_u1 = {"Authorization": f"Bearer {token_u1}"}
        headers_u2 = {"Authorization": f"Bearer {token_u2}"}

        # ── Phase 1: Unauthenticated rejection ────────────────────────────────
        try:
            r = httpx.get(f"{BASE_URL}/digest/subscription", timeout=5.0)
            ok = r.status_code == 401
            report(1, "Unauthenticated GET /digest/subscription returns 401", ok, f"status={r.status_code}")
        except Exception as e:
            report(1, "Unauthenticated GET /digest/subscription returns 401", False, str(e))

        # ── Phase 2: Initial GET returns 404 ──────────────────────────────────
        try:
            r = httpx.get(f"{BASE_URL}/digest/subscription", headers=headers_u1, timeout=5.0)
            ok = r.status_code == 404
            report(2, "Initial GET /digest/subscription returns 404 Not Found", ok, f"status={r.status_code}")
        except Exception as e:
            report(2, "Initial GET /digest/subscription returns 404 Not Found", False, str(e))

        # ── Phase 3: Create daily subscription ────────────────────────────────
        try:
            create_payload = {
                "frequency": "daily",
                "tickers": ["AAPL", "NVDA", "MSFT"],
                "sectors": ["Technology"],
                "event_types": ["earnings", "insider", "8k", "news"],
                "is_active": True,
            }
            r = httpx.post(f"{BASE_URL}/digest/subscription", json=create_payload, headers=headers_u1, timeout=5.0)
            ok = r.status_code == 200
            data = r.json()
            ok = ok and data.get("status") == "ok"
            ok = ok and data["subscription"]["frequency"] == "daily"
            ok = ok and len(data["subscription"]["tickers"]) == 3
            report(3, "Create daily subscription (POST /digest/subscription -> 200 OK)", ok, f"resp={data}")
        except Exception as e:
            report(3, "Create daily subscription (POST /digest/subscription -> 200 OK)", False, str(e))

        # ── Phase 4: Retrieve created subscription ────────────────────────────
        try:
            r = httpx.get(f"{BASE_URL}/digest/subscription", headers=headers_u1, timeout=5.0)
            ok = r.status_code == 200
            data = r.json()
            sub = data.get("subscription", {})
            ok = ok and sub.get("user_id") == "digest_user_alpha"
            ok = ok and sub.get("frequency") == "daily"
            ok = ok and "AAPL" in sub.get("tickers", [])
            ok = ok and sub.get("is_active") is True
            report(4, "Retrieve subscription (GET /digest/subscription) matches config", ok, f"sub={sub}")
        except Exception as e:
            report(4, "Retrieve subscription (GET /digest/subscription) matches config", False, str(e))

        # ── Phase 5: Update to weekly frequency ───────────────────────────────
        try:
            update_payload = {
                "frequency": "weekly",
                "tickers": ["AAPL", "GOOGL", "AMZN", "META"],
                "sectors": ["Technology", "Communication Services"],
                "event_types": ["earnings", "8k", "ma"],
                "is_active": True,
            }
            r = httpx.post(f"{BASE_URL}/digest/subscription", json=update_payload, headers=headers_u1, timeout=5.0)
            ok = r.status_code == 200
            data = r.json()
            sub = data.get("subscription", {})
            ok = ok and sub.get("frequency") == "weekly"
            ok = ok and len(sub.get("tickers", [])) == 4
            ok = ok and "Communication Services" in sub.get("sectors", [])
            report(5, "Update subscription to weekly frequency with custom tickers/sectors", ok, f"sub={sub}")
        except Exception as e:
            report(5, "Update subscription to weekly frequency with custom tickers/sectors", False, str(e))

        # ── Phase 6: Input validation error handling ──────────────────────────
        try:
            bad_freq = httpx.post(
                f"{BASE_URL}/digest/subscription",
                json={"frequency": "hourly", "tickers": ["AAPL"]},
                headers=headers_u1,
                timeout=5.0,
            )
            bad_event = httpx.post(
                f"{BASE_URL}/digest/subscription",
                json={"frequency": "daily", "event_types": ["illegal_cryptopump_event"]},
                headers=headers_u1,
                timeout=5.0,
            )
            bad_ticker = httpx.post(
                f"{BASE_URL}/digest/subscription",
                json={"frequency": "daily", "tickers": ["$$$BADTICKER$$$"]},
                headers=headers_u1,
                timeout=5.0,
            )
            ok = (
                bad_freq.status_code == 400
                and bad_event.status_code == 400
                and bad_ticker.status_code == 400
            )
            report(6, "Validation errors (invalid frequency, event type, ticker -> 400)", ok, f"freq={bad_freq.status_code}, ev={bad_event.status_code}, tkr={bad_ticker.status_code}")
        except Exception as e:
            report(6, "Validation errors (invalid frequency, event type, ticker -> 400)", False, str(e))

        # ── Phase 7: On-demand digest compilation and delivery ────────────────
        try:
            trigger_payload = {
                "recipient_email": "alpha.trader@hedgefund.com",
                "format": "html",
            }
            r = httpx.post(f"{BASE_URL}/digest/trigger", json=trigger_payload, headers=headers_u1, timeout=5.0)
            ok = r.status_code == 200
            data = r.json()
            ok = ok and data.get("status") == "sent"
            ok = ok and data.get("recipient") == "alpha.trader@hedgefund.com"
            ok = ok and "Weekly Market Digest" in data.get("subject", "")
            report(7, "On-demand digest compilation and delivery (POST /digest/trigger)", ok, f"resp={data.get('status')}")
        except Exception as e:
            report(7, "On-demand digest compilation and delivery (POST /digest/trigger)", False, str(e))

        # ── Phase 8: Content verification ─────────────────────────────────────
        try:
            html = data.get("preview_html", "")
            text = data.get("preview_text", "")
            counts = data.get("item_counts", {})
            ok = True
            ok = ok and "FinText Alpha Weekly Digest" in html
            ok = ok and "Sentiment Signals" in html
            ok = ok and "SENTIMENT SIGNALS" in text
            ok = ok and counts.get("sentiment_count", 0) > 0
            ok = ok and counts.get("events_count", 0) > 0
            report(8, "Content verification (HTML & text bodies contain signals & events)", ok, f"counts={counts}")
        except Exception as e:
            report(8, "Content verification (HTML & text bodies contain signals & events)", False, str(e))

        # ── Phase 9: Multi-user subscription isolation ────────────────────────
        try:
            # User 2 should NOT see User 1's subscription
            r_u2_get = httpx.get(f"{BASE_URL}/digest/subscription", headers=headers_u2, timeout=5.0)
            ok = r_u2_get.status_code == 404

            # Create User 2 subscription
            r_u2_post = httpx.post(
                f"{BASE_URL}/digest/subscription",
                json={"frequency": "daily", "tickers": ["TSLA"]},
                headers=headers_u2,
                timeout=5.0,
            )
            ok = ok and r_u2_post.status_code == 200

            # Verify User 1's subscription is unaffected
            r_u1_check = httpx.get(f"{BASE_URL}/digest/subscription", headers=headers_u1, timeout=5.0)
            ok = ok and r_u1_check.status_code == 200
            ok = ok and r_u1_check.json()["subscription"]["frequency"] == "weekly"

            report(9, "Multi-user subscription isolation & tenant security", ok)
        except Exception as e:
            report(9, "Multi-user subscription isolation & tenant security", False, str(e))

        # ── Phase 10: Delete subscription ─────────────────────────────────────
        try:
            del_resp = httpx.delete(f"{BASE_URL}/digest/subscription", headers=headers_u1, timeout=5.0)
            ok = del_resp.status_code == 200
            del_data = del_resp.json()
            ok = ok and del_data.get("status") == "deleted"

            # GET should now return 404
            get_after = httpx.get(f"{BASE_URL}/digest/subscription", headers=headers_u1, timeout=5.0)
            ok = ok and get_after.status_code == 404

            report(10, "Delete subscription (DELETE /digest/subscription -> 200, then 404)", ok)
        except Exception as e:
            report(10, "Delete subscription (DELETE /digest/subscription -> 200, then 404)", False, str(e))

        # ── Phase 11: Python SDK sync client integration ──────────────────────
        try:
            from fintext import FinTextClient

            token_sdk_sync = get_auth_token("sdk_sync_user")
            with FinTextClient(base_url=BASE_URL, api_token=token_sdk_sync) as client:
                # Create subscription
                created = client.create_digest_subscription(
                    frequency="daily",
                    tickers=["AAPL", "NVDA"],
                    sectors=["Technology"],
                    event_types=["earnings", "8k", "news"],
                )
                ok = created.status == "ok"
                ok = ok and created.subscription.frequency == "daily"
                ok = ok and len(created.subscription.tickers) == 2

                # Get subscription
                fetched = client.get_digest_subscription()
                ok = ok and fetched.subscription.user_id == "sdk_sync_user"

                # Trigger on-demand digest
                triggered = client.trigger_digest(
                    recipient_email="sdk.sync@hedgefund.com",
                    format="html",
                )
                ok = ok and triggered.status == "sent"
                ok = ok and triggered.recipient == "sdk.sync@hedgefund.com"

                # Delete subscription
                deleted = client.delete_digest_subscription()
                ok = ok and deleted.status == "deleted"

            report(11, "Python SDK sync client integration (CRUD + trigger)", ok)
        except Exception as e:
            report(11, "Python SDK sync client integration (CRUD + trigger)", False, str(e))

        # ── Phase 12: Python SDK async client integration ─────────────────────
        async def verify_async():
            from fintext import FinTextAsyncClient

            token_sdk_async = get_auth_token("sdk_async_user")
            async with FinTextAsyncClient(base_url=BASE_URL, api_token=token_sdk_async) as client:
                # Create subscription
                created = await client.create_digest_subscription(
                    frequency="weekly",
                    tickers=["MSFT", "AMZN"],
                    sectors=["Technology"],
                    event_types=["earnings", "insider", "8k", "news"],
                )
                ok = created.status == "ok"
                ok = ok and created.subscription.frequency == "weekly"

                # Get subscription
                fetched = await client.get_digest_subscription()
                ok = ok and fetched.subscription.user_id == "sdk_async_user"

                # Trigger on-demand digest
                triggered = await client.trigger_digest(
                    recipient_email="sdk.async@hedgefund.com",
                    format="html",
                )
                ok = ok and triggered.status == "sent"
                ok = ok and triggered.recipient == "sdk.async@hedgefund.com"

                # Delete subscription
                deleted = await client.delete_digest_subscription()
                ok = ok and deleted.status == "deleted"
                return ok

        try:
            ok_async = asyncio.run(verify_async())
            report(12, "Python SDK async client integration (CRUD + trigger)", ok_async)
        except Exception as e:
            report(12, "Python SDK async client integration (CRUD + trigger)", False, str(e))

    # ── Summary ───────────────────────────────────────────────────────────────
    print("\n" + "=" * 79)
    print(f"  Suite #215 Results: {passed}/{total} Passed ({(passed / total) * 100:.1f}%)")
    print("=" * 79)

    if failed > 0:
        print(f"\n❌ FAILED: {failed} test phases failed.")
        sys.exit(1)
    else:
        print("\n🎉 ALL 12 PHASES PASSED CLEANLY!")
        sys.exit(0)


if __name__ == "__main__":
    main()
