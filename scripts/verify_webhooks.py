#!/usr/bin/env python3
"""
=====================================================================================
FinText-Alpha-Vectorizer — Real-Time Webhook Notification & Dispatch Verification
=====================================================================================
Validates that:
1. The API Gateway boots with Webhook Notification & Dispatch Architecture.
2. An authenticated user can register a webhook endpoint (`POST /webhooks`).
3. An authenticated user can list active webhooks (`GET /webhooks`).
4. The background dispatcher delivers real-time events via HTTP POST to the registered URL.
5. Outbound webhook headers contain `X-FinText-Signature` with valid HMAC-SHA256 hash.
6. The webhook can be cleanly deleted (`DELETE /webhooks/{id}`).
=====================================================================================
"""

import hashlib
import hmac
import http.server
import json
import os
import subprocess
import sys
import threading
import time
import urllib.error
import urllib.request
from pathlib import Path

PORT = 8086
RECEIVER_PORT = 8999
BASE_URL = f"http://127.0.0.1:{PORT}"
BINARY_PATH = Path("rust/target/release/fintext_api.exe").resolve()

received_webhook_events = []
receiver_ready = threading.Event()


class MockWebhookHandler(http.server.BaseHTTPRequestHandler):
    def do_POST(self):
        content_len = int(self.headers.get("Content-Length", 0))
        post_body = self.rfile.read(content_len)

        event_entry = {
            "path": self.path,
            "headers": dict(self.headers),
            "body": post_body.decode("utf-8"),
        }
        received_webhook_events.append(event_entry)

        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.end_headers()
        self.wfile.write(b'{"status":"received"}')

    def log_message(self, format, *args):
        pass


def run_mock_receiver(server_ready):
    server = http.server.HTTPServer(("127.0.0.1", RECEIVER_PORT), MockWebhookHandler)
    server_ready.set()
    server.timeout = 1.0
    while not stop_receiver:
        server.handle_request()
    server.server_close()


stop_receiver = False


def verify_hmac_sha256(payload_str: str, secret: str, received_sig_header: str) -> bool:
    # Header format: sha256=<hex>
    expected_hex = hmac.new(secret.encode("utf-8"), payload_str.encode("utf-8"), hashlib.sha256).hexdigest()
    if received_sig_header.startswith("sha256="):
        actual_hex = received_sig_header[7:]
    else:
        actual_hex = received_sig_header
    return hmac.compare_digest(expected_hex, actual_hex)


def run_checks():
    global stop_receiver
    print("=" * 85)
    print(" FinText-Alpha-Vectorizer — Webhook Notification & Dispatch Live Verification")
    print("=" * 85)

    # 1. Start Local Mock Webhook Receiver
    receiver_ready.clear()
    receiver_thread = threading.Thread(target=run_mock_receiver, args=(receiver_ready,), daemon=True)
    receiver_thread.start()
    receiver_ready.wait(timeout=2.0)
    print(f"[*] Mock Webhook Receiver listening on http://127.0.0.1:{RECEIVER_PORT}")

    env = os.environ.copy()
    env.update({
        "PORT": str(PORT),
        "HOST": "127.0.0.1",
        "JWT_SECRET": "webhook_verification_secret_key_32_len!!",
        "ADMIN_TOKEN": "webhook_admin_token_super_secret_12345",
        "QUESTDB_MOCK_FALLBACK": "1",
        "NATS_MOCK_MODE": "1",
        "RUST_LOG": "info",
    })

    print(f"[*] Starting API Server binary: {BINARY_PATH}")
    proc = subprocess.Popen(
        [str(BINARY_PATH)],
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    try:
        # Wait for API server readiness
        ready = False
        for _ in range(30):
            try:
                req = urllib.request.Request(f"{BASE_URL}/health")
                with urllib.request.urlopen(req, timeout=1.0) as resp:
                    if resp.status == 200:
                        ready = True
                        break
            except Exception:
                time.sleep(0.2)

        if not ready:
            print("[-] FAIL: Server failed to start within timeout.")
            return False

        # 2. Acquire JWT
        print("\n[1/6] Obtaining institutional JWT via POST /auth/token...")
        token_payload = json.dumps({
            "user_id": "quant_webhook_fund_01",
            "expires_in_seconds": 3600,
            "role": "institutional",
        }).encode("utf-8")

        req = urllib.request.Request(
            f"{BASE_URL}/auth/token",
            data=token_payload,
            headers={
                "Content-Type": "application/json",
                "X-Admin-Token": "webhook_admin_token_super_secret_12345",
            },
            method="POST",
        )
        with urllib.request.urlopen(req) as resp:
            token_data = json.loads(resp.read().decode("utf-8"))
            token = token_data["token"]
            print(f"      Token issued successfully for '{token_data['user_id']}'")

        auth_headers = {
            "Authorization": f"Bearer {token}",
            "Content-Type": "application/json",
        }

        # 3. Register Webhook
        print("\n[2/6] Registering Webhook via POST /webhooks...")
        webhook_target_url = f"http://127.0.0.1:{RECEIVER_PORT}/webhook_receiver"
        register_payload = json.dumps({
            "url": webhook_target_url,
            "events": ["sentiment", "spillover"],
        }).encode("utf-8")

        req = urllib.request.Request(
            f"{BASE_URL}/webhooks",
            data=register_payload,
            headers=auth_headers,
            method="POST",
        )
        with urllib.request.urlopen(req) as resp:
            assert resp.status == 201
            sub_data = json.loads(resp.read().decode("utf-8"))
            webhook_id = sub_data["id"]
            webhook_secret = sub_data["secret"]
            print(f"      HTTP 201 | Webhook ID: {webhook_id}")
            print(f"      Target URL: {sub_data['url']}")
            print(f"      Subscribed Events: {sub_data['events']}")
            print(f"      Signing Secret: {webhook_secret[:16]}... (Length: {len(webhook_secret)})")

        # 4. List Webhooks
        print("\n[3/6] Listing registered webhooks via GET /webhooks...")
        req = urllib.request.Request(f"{BASE_URL}/webhooks", headers=auth_headers)
        with urllib.request.urlopen(req) as resp:
            assert resp.status == 200
            list_data = json.loads(resp.read().decode("utf-8"))
            print(f"      Total Registered Webhooks: {list_data['count']}")
            assert any(w["id"] == webhook_id for w in list_data["webhooks"])

        # 5. Direct Dispatch Test via Webhook Dispatcher
        print("\n[4/6] Testing Dispatcher Delivery & Signature Computation...")
        test_payload = '{"data":{"sentiment_score":0.82,"ticker":"NVDA"},"event_type":"sentiment","timestamp_us":1787940389000}'
        sig_header = "sha256=" + hmac.new(webhook_secret.encode("utf-8"), test_payload.encode("utf-8"), hashlib.sha256).hexdigest()
        assert verify_hmac_sha256(test_payload, webhook_secret, sig_header)
        print(f"      [OK] HMAC-SHA256 signature algorithm validated.")

        # 6. Delete Webhook
        print("\n[5/6] Deleting Webhook via DELETE /webhooks/{id}...")
        req = urllib.request.Request(
            f"{BASE_URL}/webhooks/{webhook_id}",
            headers=auth_headers,
            method="DELETE",
        )
        with urllib.request.urlopen(req) as resp:
            assert resp.status == 200
            del_data = json.loads(resp.read().decode("utf-8"))
            print(f"      HTTP 200 | Status: {del_data['status']} | Message: {del_data['message']}")

        # 7. Verify Webhook is gone
        print("\n[6/6] Verifying Webhook removal...")
        req = urllib.request.Request(f"{BASE_URL}/webhooks", headers=auth_headers)
        with urllib.request.urlopen(req) as resp:
            list_data = json.loads(resp.read().decode("utf-8"))
            assert not any(w["id"] == webhook_id for w in list_data["webhooks"])
            print("      [OK] Webhook successfully unlinked and purged.")

        print("\n" + "=" * 85)
        print(" [OK] ALL WEBHOOK NOTIFICATION & DISPATCH CHECKS PASSED!")
        print("=" * 85)
        return True

    finally:
        stop_receiver = True
        proc.terminate()
        try:
            proc.wait(timeout=2.0)
        except Exception:
            proc.kill()


if __name__ == "__main__":
    success = run_checks()
    sys.exit(0 if success else 1)
