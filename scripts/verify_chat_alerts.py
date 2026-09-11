#!/usr/bin/env python3
"""
===============================================================================
FinText-Alpha-Vectorizer — Suite #227: Telegram & Discord Alert Bot Subscription Certification
===============================================================================
Verifies:
  1.  Unauthenticated GET and POST /chat-alerts return 401 Unauthorized
  2.  Invalid channel_type returns 400 Bad Request
  3.  Empty channel_target returns 400 Bad Request
  4.  Invalid Telegram channel_target format returns 400 Bad Request
  5.  Invalid Discord webhook URL (non-HTTPS) returns 400 Bad Request
  6.  Empty or unknown event_types return 400 Bad Request
  7.  Create Telegram alert subscription returns 201 Created with valid fields
  8.  Create Discord alert subscription returns 201 Created with valid fields
  9.  List chat alert subscriptions returns 200 OK with active subscriptions
  10. Deleting non-existent subscription ID returns 404 Not Found
  11. Deleting active subscription returns 200 OK confirmation
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

PORT = 8127
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
        env["CHAT_ALERTS_MOCK"] = "1"

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

        print(f"[READY] FinText API Server is responding on {BASE_URL}.\n")
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        if self.process:
            print(f"[STOPPING] Terminating FinText API Server (PID: {self.process.pid})...")
            self.process.terminate()
            try:
                self.process.wait(timeout=5.0)
            except subprocess.TimeoutExpired:
                self.process.kill()


def get_jwt_token(client: httpx.Client, user_id: str = "alert_quant") -> str:
    r = client.post(
        f"{BASE_URL}/auth/token",
        headers={"X-Admin-Token": ADMIN_TOKEN},
        json={"user_id": user_id},
    )
    assert r.status_code == 200, f"Failed to acquire JWT: {r.text}"
    return r.json()["token"]


def main():
    global passed, failed

    print("═══════════════════════════════════════════════════════════════════════════════")
    print(" FinText-Alpha-Vectorizer — Suite #227: Telegram & Discord Alert Bot")
    print("═══════════════════════════════════════════════════════════════════════════════\n")

    with ServerContext():
        client = httpx.Client(base_url=BASE_URL, timeout=10.0)
        token = get_jwt_token(client)
        auth_headers = {"Authorization": f"Bearer {token}"}

        # ── Phase 1: Unauthenticated requests return 401 ─────────────────────
        r1_get = client.get("/chat-alerts")
        r1_post = client.post("/chat-alerts", json={"channel_type": "telegram", "channel_target": "123", "event_types": ["sentiment_anomaly"]})
        report(1, "Unauthenticated requests to /chat-alerts return 401", r1_get.status_code == 401 and r1_post.status_code == 401)

        # ── Phase 2: Invalid channel_type returns 400 ────────────────────────
        r2 = client.post(
            "/chat-alerts",
            headers=auth_headers,
            json={"channel_type": "slack", "channel_target": "123456", "event_types": ["sentiment_anomaly"]},
        )
        report(2, "Invalid channel_type ('slack') returns 400 Bad Request", r2.status_code == 400)

        # ── Phase 3: Empty channel_target returns 400 ────────────────────────
        r3 = client.post(
            "/chat-alerts",
            headers=auth_headers,
            json={"channel_type": "telegram", "channel_target": "   ", "event_types": ["sentiment_anomaly"]},
        )
        report(3, "Empty channel_target returns 400 Bad Request", r3.status_code == 400)

        # ── Phase 4: Invalid Telegram channel_target format returns 400 ──────
        r4 = client.post(
            "/chat-alerts",
            headers=auth_headers,
            json={"channel_type": "telegram", "channel_target": "invalid target with spaces!!!", "event_types": ["sentiment_anomaly"]},
        )
        report(4, "Invalid Telegram channel_target format returns 400 Bad Request", r4.status_code == 400)

        # ── Phase 5: Invalid Discord webhook URL returns 400 ─────────────────
        r5 = client.post(
            "/chat-alerts",
            headers=auth_headers,
            json={"channel_type": "discord", "channel_target": "ftp://discord.com/hook", "event_types": ["sentiment_anomaly"]},
        )
        report(5, "Invalid Discord webhook URL (non-HTTPS) returns 400 Bad Request", r5.status_code == 400)

        # ── Phase 6: Empty or unknown event_types return 400 ─────────────────
        r6_empty = client.post(
            "/chat-alerts",
            headers=auth_headers,
            json={"channel_type": "telegram", "channel_target": "123456789", "event_types": []},
        )
        r6_unknown = client.post(
            "/chat-alerts",
            headers=auth_headers,
            json={"channel_type": "telegram", "channel_target": "123456789", "event_types": ["invalid_event_type"]},
        )
        report(6, "Empty or unknown event_types return 400 Bad Request", r6_empty.status_code == 400 and r6_unknown.status_code == 400)

        # ── Phase 7: Create Telegram alert subscription returns 201 ──────────
        r7 = client.post(
            "/chat-alerts",
            headers=auth_headers,
            json={
                "channel_type": "telegram",
                "channel_target": "987654321",
                "event_types": ["sentiment_anomaly", "8k_filing"],
            },
        )
        data7 = r7.json()
        tg_sub_id = data7.get("id", "")
        p7_ok = (
            r7.status_code == 201
            and data7.get("channel_type") == "telegram"
            and data7.get("channel_target") == "987654321"
            and data7.get("event_types") == ["sentiment_anomaly", "8k_filing"]
            and data7.get("is_active") is True
            and bool(tg_sub_id)
        )
        report(7, "Create Telegram alert subscription returns 201 Created", p7_ok)

        # ── Phase 8: Create Discord alert subscription returns 201 ───────────
        discord_url = "https://discord.com/api/webhooks/1234567890/abcdefghijklmnopqrstuvwxyz"
        r8 = client.post(
            "/chat-alerts",
            headers=auth_headers,
            json={
                "channel_type": "discord",
                "channel_target": discord_url,
                "event_types": ["unusual_options"],
            },
        )
        data8 = r8.json()
        dc_sub_id = data8.get("id", "")
        p8_ok = (
            r8.status_code == 201
            and data8.get("channel_type") == "discord"
            and data8.get("channel_target") == discord_url
            and data8.get("event_types") == ["unusual_options"]
            and data8.get("is_active") is True
            and bool(dc_sub_id)
        )
        report(8, "Create Discord alert subscription returns 201 Created", p8_ok)

        # ── Phase 9: List chat alert subscriptions returns 200 OK ────────────
        r9 = client.get("/chat-alerts", headers=auth_headers)
        data9 = r9.json()
        subs = data9.get("subscriptions", [])
        p9_ok = (
            r9.status_code == 200
            and data9.get("total", 0) >= 2
            and any(s["id"] == tg_sub_id for s in subs)
            and any(s["id"] == dc_sub_id for s in subs)
        )
        report(9, "List chat alert subscriptions returns 200 OK with active subscriptions", p9_ok)

        # ── Phase 10: Deleting non-existent subscription returns 404 ─────────
        r10 = client.delete(
            "/chat-alerts/00000000-0000-0000-0000-000000000000",
            headers=auth_headers,
        )
        report(10, "Deleting non-existent subscription ID returns 404 Not Found", r10.status_code == 404)

        # ── Phase 11: Deleting active subscription returns 200 OK ────────────
        r11 = client.delete(f"/chat-alerts/{tg_sub_id}", headers=auth_headers)
        data11 = r11.json()
        p11_ok = (
            r11.status_code == 200
            and data11.get("success") is True
            and data11.get("id") == tg_sub_id
        )
        report(11, "Deleting active subscription returns 200 OK confirmation", p11_ok)

        # ── Phase 12: Python SDK sync & async client parity ──────────────────
        from fintext import FinTextClient, FinTextAsyncClient, ChatAlertSubscription, ChatAlertsResponse, DeleteChatAlertResponse

        sync_sdk = FinTextClient(base_url=BASE_URL, api_token=token)
        sync_sub = sync_sdk.create_chat_alert(
            channel_type="telegram",
            channel_target="-100123456789",
            event_types=["sentiment_anomaly", "8k_filing", "unusual_options"],
        )
        sync_list = sync_sdk.list_chat_alerts()
        sync_del = sync_sdk.delete_chat_alert(sync_sub.id)

        async def run_async_sdk():
            async_sdk = FinTextAsyncClient(base_url=BASE_URL, api_token=token)
            sub = await async_sdk.create_chat_alert(
                channel_type="discord",
                channel_target="https://discord.com/api/webhooks/999/async_test",
                event_types=["sentiment_anomaly"],
            )
            listed = await async_sdk.list_chat_alerts()
            deleted = await async_sdk.delete_chat_alert(sub.id)
            await async_sdk.close()
            return sub, listed, deleted

        async_sub, async_list, async_del = asyncio.run(run_async_sdk())

        p12_ok = (
            isinstance(sync_sub, ChatAlertSubscription)
            and isinstance(sync_list, ChatAlertsResponse)
            and isinstance(sync_del, DeleteChatAlertResponse)
            and sync_del.success is True
            and isinstance(async_sub, ChatAlertSubscription)
            and isinstance(async_list, ChatAlertsResponse)
            and isinstance(async_del, DeleteChatAlertResponse)
            and async_del.success is True
        )
        report(12, "Python SDK sync and async client parity verified", p12_ok)

    print("\n═══════════════════════════════════════════════════════════════════════════════")
    print(f" Certification Results: {passed}/{total} Passed ({(passed/total)*100:.1f}%)")
    print("═══════════════════════════════════════════════════════════════════════════════\n")

    if failed > 0:
        sys.exit(1)


if __name__ == "__main__":
    main()
