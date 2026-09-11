#!/usr/bin/env python3
"""
===============================================================================
FinText-Alpha-Vectorizer — Suite #239: Real-time Sentiment Anomaly WebSocket Push
===============================================================================
Verifies:
  1.  Unauthenticated POST /anomaly-scan returns 401 Unauthorized
  2.  Authenticated POST /anomaly-scan triggers immediate anomaly computation
  3.  Anomaly response schema validation (anomalies_found, alerts_broadcasted, scanned_at, message)
  4.  SentimentAnomalyAlert frame fields validation (type='sentiment_anomaly', ticker, latest_score, mean_score, stddev, zscore, direction, timestamp)
  5.  Statistical validity and direction consistency (|zscore| >= 2.0, bullish/bearish alignment)
  6.  Unauthenticated WebSocket connection (/ws) without token returns 401 Unauthorized
  7.  Authenticated WebSocket connection (/ws?token=...) receives handshake welcome frame
  8.  WebSocket query parameter 'streams=anomalies' configures subscription
  9.  Real-time push: Triggering POST /anomaly-scan pushes sentiment_anomaly alert frames to WebSocket receiver
  10. Dynamic subscription: Sending control frame {"action": "subscribe", "stream": "anomalies"} receives ack
  11. Dynamic unsubscription: Sending control frame {"action": "unsubscribe", "stream": "anomalies"} receives ack
  12. Multi-stream client: 'streams=all' receives both connection welcome and anomaly alerts
  13. Python SDK Sync Client integration (client.trigger_anomaly_scan & client.get_anomaly_websocket_url)
  14. Python SDK Async Client integration (async_client.trigger_anomaly_scan & async_client.get_anomaly_websocket_url)
===============================================================================
"""

import asyncio
import json
import os
from pathlib import Path
import socket
import struct
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

PORT = 8138
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
        print(f"  \u2705 Phase {phase:2d} \u2502 {name}")
    else:
        failed += 1
        msg = f"  \u274c Phase {phase:2d} \u2502 {name}"
        if detail:
            msg += f" \u2014 {detail}"
        print(msg)


class RawWebSocketClient:
    def __init__(self, host: str, port: int, path: str):
        self.sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        self.sock.settimeout(4.0)
        self.sock.connect((host, port))

        req = (
            f"GET {path} HTTP/1.1\r\n"
            f"Host: {host}:{port}\r\n"
            f"Upgrade: websocket\r\n"
            f"Connection: Upgrade\r\n"
            f"Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n"
            f"Sec-WebSocket-Version: 13\r\n\r\n"
        ).encode("utf-8")
        self.sock.sendall(req)

        header_data = b""
        while b"\r\n\r\n" not in header_data:
            chunk = self.sock.recv(1024)
            if not chunk:
                break
            header_data += chunk

        self.handshake_header = header_data.decode("utf-8", errors="replace")
        self.is_connected = "101 Switching Protocols" in self.handshake_header

    def send_text(self, text: str):
        payload = text.encode("utf-8")
        length = len(payload)
        mask = b"\x12\x34\x56\x78"
        masked_payload = bytes([b ^ mask[i % 4] for i, b in enumerate(payload)])

        if length <= 125:
            header = struct.pack("!BB", 0x81, 0x80 | length) + mask
        elif length <= 65535:
            header = struct.pack("!BBH", 0x81, 0x80 | 126, length) + mask
        else:
            header = struct.pack("!BBQ", 0x81, 0x80 | 127, length) + mask

        self.sock.sendall(header + masked_payload)

    def recv_frame(self, timeout=4.0):
        self.sock.settimeout(timeout)
        try:
            head = self.sock.recv(2)
            if len(head) < 2:
                return None
            b1, b2 = head
            fin = (b1 & 0x80) != 0
            opcode = b1 & 0x0F
            is_masked = (b2 & 0x80) != 0
            length = b2 & 0x7F

            if length == 126:
                ext = self.sock.recv(2)
                if len(ext) < 2:
                    return None
                length = struct.unpack("!H", ext)[0]
            elif length == 127:
                ext = self.sock.recv(8)
                if len(ext) < 8:
                    return None
                length = struct.unpack("!Q", ext)[0]

            mask = b""
            if is_masked:
                mask = self.sock.recv(4)

            payload = b""
            while len(payload) < length:
                chunk = self.sock.recv(min(4096, length - len(payload)))
                if not chunk:
                    break
                payload += chunk

            if is_masked:
                payload = bytes([b ^ mask[i % 4] for i, b in enumerate(payload)])

            if opcode == 0x01:  # Text frame
                return payload.decode("utf-8", errors="replace")
            elif opcode == 0x09:  # Ping -> reply Pong
                self.sock.sendall(b"\x8a\x00")
                return self.recv_frame(timeout)
            elif opcode == 0x08:  # Close
                return None
            return payload
        except socket.timeout:
            return None

    def close(self):
        try:
            self.sock.close()
        except Exception:
            pass


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
        env["CHAT_ALERTS_MOCK"] = "1"
        env["ANOMALY_SCAN_INTERVAL_SECS"] = "60"

        if not SERVER_EXE.exists():
            raise RuntimeError(f"Server binary not found at {SERVER_EXE}. Run `cargo build -p fintext_api_server` first.")

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
                raise RuntimeError(f"Server process terminated early with return code {self.process.returncode}")
            raise RuntimeError("Timed out waiting for FinText API server to become ready.")

        print(f"[READY] FinText API Server online at {BASE_URL}")
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        if self.process:
            print("[STOPPING] Terminating FinText API Server...")
            self.process.terminate()
            try:
                self.process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.process.kill()
            print("[STOPPED] FinText API Server stopped.")


def obtain_token(user_id="anomaly_trader", role="institutional"):
    resp = httpx.post(
        f"{BASE_URL}/auth/token",
        json={"user_id": user_id, "role": role, "expires_in_seconds": 3600},
        headers={"X-Admin-Token": ADMIN_TOKEN},
        timeout=5.0,
    )
    resp.raise_for_status()
    return resp.json()["token"]


def run_tests():
    global passed, failed

    print("=" * 80)
    print("Suite #239: Real-time Sentiment Anomaly WebSocket Push Verification")
    print("=" * 80)

    token = obtain_token()
    auth_headers = {"Authorization": f"Bearer {token}"}

    # ─────────────────────────────────────────────────────────────────────────────
    # Phase 1: Unauthenticated POST /anomaly-scan returns 401
    # ─────────────────────────────────────────────────────────────────────────────
    try:
        r = httpx.post(f"{BASE_URL}/anomaly-scan", timeout=5.0)
        report(1, "Unauthenticated POST /anomaly-scan returns 401 Unauthorized", r.status_code == 401, f"Status: {r.status_code}")
    except Exception as e:
        report(1, "Unauthenticated POST /anomaly-scan returns 401 Unauthorized", False, str(e))

    # ─────────────────────────────────────────────────────────────────────────────
    # Phase 2: Authenticated POST /anomaly-scan returns 200 OK with AnomalyScanResponse
    # ─────────────────────────────────────────────────────────────────────────────
    scan_resp_data = None
    try:
        r = httpx.post(f"{BASE_URL}/anomaly-scan", headers=auth_headers, timeout=5.0)
        ok = (r.status_code == 200)
        if ok:
            scan_resp_data = r.json()
            ok = ("anomalies_found" in scan_resp_data and "anomalies" in scan_resp_data)
        report(2, "Authenticated POST /anomaly-scan triggers immediate computation", ok, f"Status: {r.status_code}")
    except Exception as e:
        report(2, "Authenticated POST /anomaly-scan triggers immediate computation", False, str(e))

    # ─────────────────────────────────────────────────────────────────────────────
    # Phase 3: Anomaly response envelope and metadata validation
    # ─────────────────────────────────────────────────────────────────────────────
    try:
        ok = False
        if scan_resp_data:
            has_fields = all(k in scan_resp_data for k in ["anomalies_found", "alerts_broadcasted", "anomalies", "scanned_at", "message"])
            ok = has_fields and scan_resp_data["anomalies_found"] > 0 and len(scan_resp_data["anomalies"]) > 0
        report(3, "Anomaly response envelope and metadata validation", ok, f"Found: {scan_resp_data.get('anomalies_found') if scan_resp_data else 0}")
    except Exception as e:
        report(3, "Anomaly response envelope and metadata validation", False, str(e))

    # ─────────────────────────────────────────────────────────────────────────────
    # Phase 4: SentimentAnomalyAlert frame schema and fields validation
    # ─────────────────────────────────────────────────────────────────────────────
    try:
        ok = False
        if scan_resp_data and scan_resp_data.get("anomalies"):
            item = scan_resp_data["anomalies"][0]
            req_fields = ["type", "ticker", "latest_score", "mean_score", "stddev", "zscore", "direction", "timestamp"]
            has_all = all(f in item for f in req_fields)
            is_type = item.get("type") == "sentiment_anomaly"
            ok = has_all and is_type
        report(4, "SentimentAnomalyAlert frame schema and field validation", ok, f"Sample: {scan_resp_data['anomalies'][0] if scan_resp_data and scan_resp_data.get('anomalies') else None}")
    except Exception as e:
        report(4, "SentimentAnomalyAlert frame schema and field validation", False, str(e))

    # ─────────────────────────────────────────────────────────────────────────────
    # Phase 5: Statistical validity and direction consistency (|z| >= 2.0, bullish/bearish)
    # ─────────────────────────────────────────────────────────────────────────────
    try:
        ok = False
        if scan_resp_data and scan_resp_data.get("anomalies"):
            all_valid = True
            for alert in scan_resp_data["anomalies"]:
                z = alert["zscore"]
                dir_str = alert["direction"]
                if abs(z) < 2.0:
                    all_valid = False
                    break
                if z > 0 and dir_str != "bullish":
                    all_valid = False
                    break
                if z < 0 and dir_str != "bearish":
                    all_valid = False
                    break
            ok = all_valid
        report(5, "Statistical z-score magnitude (|z| >= 2.0) and direction consistency", ok)
    except Exception as e:
        report(5, "Statistical z-score magnitude (|z| >= 2.0) and direction consistency", False, str(e))

    # ─────────────────────────────────────────────────────────────────────────────
    # Phase 6: Unauthenticated WebSocket connection (/ws) returns 401 Unauthorized
    # ─────────────────────────────────────────────────────────────────────────────
    try:
        ws_unauth = RawWebSocketClient("127.0.0.1", PORT, "/ws")
        ok = "401 Unauthorized" in ws_unauth.handshake_header
        ws_unauth.close()
        report(6, "Unauthenticated WebSocket connection (/ws) returns 401", ok)
    except Exception as e:
        report(6, "Unauthenticated WebSocket connection (/ws) returns 401", False, str(e))

    # ─────────────────────────────────────────────────────────────────────────────
    # Phase 7: Authenticated WebSocket connection (/ws?token=...) receives handshake
    # ─────────────────────────────────────────────────────────────────────────────
    try:
        ws_auth = RawWebSocketClient("127.0.0.1", PORT, f"/ws?token={token}")
        ok = ws_auth.is_connected
        welcome_frame = ws_auth.recv_frame(timeout=3.0)
        if ok and welcome_frame:
            welcome_data = json.loads(welcome_frame)
            ok = (welcome_data.get("type") == "connected" and "subscribed_streams" in welcome_data)
        ws_auth.close()
        report(7, "Authenticated WebSocket connection receives handshake welcome frame", ok)
    except Exception as e:
        report(7, "Authenticated WebSocket connection receives handshake welcome frame", False, str(e))

    # ─────────────────────────────────────────────────────────────────────────────
    # Phase 8: WebSocket query parameter 'streams=anomalies' configures subscription
    # ─────────────────────────────────────────────────────────────────────────────
    try:
        ws_anom = RawWebSocketClient("127.0.0.1", PORT, f"/ws?token={token}&streams=anomalies")
        ok = ws_anom.is_connected
        welcome_frame = ws_anom.recv_frame(timeout=3.0)
        if ok and welcome_frame:
            welcome_data = json.loads(welcome_frame)
            sub_streams = welcome_data.get("subscribed_streams", {})
            ok = (sub_streams.get("anomalies") is True and sub_streams.get("sentiment") is False)
        ws_anom.close()
        report(8, "WebSocket query param 'streams=anomalies' sets anomaly subscription", ok)
    except Exception as e:
        report(8, "WebSocket query param 'streams=anomalies' sets anomaly subscription", False, str(e))

    # ─────────────────────────────────────────────────────────────────────────────
    # Phase 9: Real-time push: Triggering POST /anomaly-scan pushes alerts to WS client
    # ─────────────────────────────────────────────────────────────────────────────
    try:
        ws_listener = RawWebSocketClient("127.0.0.1", PORT, f"/ws?token={token}&streams=anomalies")
        ok_conn = ws_listener.is_connected
        welcome_frame = ws_listener.recv_frame(timeout=3.0)

        # Trigger scan
        r = httpx.post(f"{BASE_URL}/anomaly-scan", headers=auth_headers, timeout=5.0)
        r.raise_for_status()

        # Receive anomaly alert over WebSocket
        pushed_frame = ws_listener.recv_frame(timeout=4.0)
        ok = False
        if pushed_frame:
            pushed_data = json.loads(pushed_frame)
            ok = (pushed_data.get("type") == "sentiment_anomaly" and "ticker" in pushed_data and "zscore" in pushed_data)
        ws_listener.close()
        report(9, "Real-time push: Triggering POST /anomaly-scan pushes alerts to WS client", ok, f"Pushed alert: {pushed_frame[:80] if pushed_frame else 'None'}")
    except Exception as e:
        report(9, "Real-time push: Triggering POST /anomaly-scan pushes alerts to WS client", False, str(e))

    # ─────────────────────────────────────────────────────────────────────────────
    # Phase 10: Dynamic subscription: Sending control frame {"action": "subscribe", "stream": "anomalies"} receives ack
    # ─────────────────────────────────────────────────────────────────────────────
    try:
        ws_dyn = RawWebSocketClient("127.0.0.1", PORT, f"/ws?token={token}&streams=sentiment")
        welcome = ws_dyn.recv_frame(timeout=3.0)

        # Send dynamic subscribe frame
        ws_dyn.send_text(json.dumps({"action": "subscribe", "stream": "anomalies"}))
        ack_frame = ws_dyn.recv_frame(timeout=3.0)
        ok = False
        if ack_frame:
            ack_data = json.loads(ack_frame)
            ok = (ack_data.get("type") == "subscribed" and ack_data.get("stream") == "anomalies" and ack_data.get("status") == "active")
        ws_dyn.close()
        report(10, "Dynamic subscription control frame receives subscription confirmation", ok, f"Ack: {ack_frame}")
    except Exception as e:
        report(10, "Dynamic subscription control frame receives subscription confirmation", False, str(e))

    # ─────────────────────────────────────────────────────────────────────────────
    # Phase 11: Dynamic unsubscription: Sending control frame {"action": "unsubscribe", "stream": "anomalies"} receives ack
    # ─────────────────────────────────────────────────────────────────────────────
    try:
        ws_unsub = RawWebSocketClient("127.0.0.1", PORT, f"/ws?token={token}&streams=anomalies")
        welcome = ws_unsub.recv_frame(timeout=3.0)

        # Send dynamic unsubscribe frame
        ws_unsub.send_text(json.dumps({"action": "unsubscribe", "stream": "anomalies"}))
        ack_frame = ws_unsub.recv_frame(timeout=3.0)
        ok = False
        if ack_frame:
            ack_data = json.loads(ack_frame)
            ok = (ack_data.get("type") == "unsubscribed" and ack_data.get("stream") == "anomalies" and ack_data.get("status") == "inactive")
        ws_unsub.close()
        report(11, "Dynamic unsubscription control frame receives unsubscription confirmation", ok, f"Ack: {ack_frame}")
    except Exception as e:
        report(11, "Dynamic unsubscription control frame receives unsubscription confirmation", False, str(e))

    # ─────────────────────────────────────────────────────────────────────────────
    # Phase 12: Multi-stream client: 'streams=all' subscribes to both sentiment and anomalies
    # ─────────────────────────────────────────────────────────────────────────────
    try:
        ws_all = RawWebSocketClient("127.0.0.1", PORT, f"/ws?token={token}&streams=all")
        ok = ws_all.is_connected
        welcome_frame = ws_all.recv_frame(timeout=3.0)
        if ok and welcome_frame:
            welcome_data = json.loads(welcome_frame)
            sub_streams = welcome_data.get("subscribed_streams", {})
            ok = (sub_streams.get("anomalies") is True and sub_streams.get("sentiment") is True)
        ws_all.close()
        report(12, "Multi-stream client ('streams=all') subscribes to both sentiment and anomalies", ok)
    except Exception as e:
        report(12, "Multi-stream client ('streams=all') subscribes to both sentiment and anomalies", False, str(e))

    # ─────────────────────────────────────────────────────────────────────────────
    # Phase 13: Python SDK Sync Client integration
    # ─────────────────────────────────────────────────────────────────────────────
    try:
        from fintext import FinTextClient, AnomalyScanResponse, SentimentAnomalyAlert

        with FinTextClient(base_url=BASE_URL, api_token=token) as client:
            scan_res = client.trigger_anomaly_scan()
            ok = isinstance(scan_res, AnomalyScanResponse) and scan_res.anomalies_found > 0
            if ok and scan_res.anomalies:
                ok = isinstance(scan_res.anomalies[0], SentimentAnomalyAlert)

            ws_url = client.get_anomaly_websocket_url()
            ok = ok and ("ws://" in ws_url and "streams=anomalies" in ws_url and f"token={token}" in ws_url)
        report(13, "Python SDK Sync Client integration (trigger_anomaly_scan & get_anomaly_websocket_url)", ok)
    except Exception as e:
        report(13, "Python SDK Sync Client integration (trigger_anomaly_scan & get_anomaly_websocket_url)", False, str(e))

    # ─────────────────────────────────────────────────────────────────────────────
    # Phase 14: Python SDK Async Client integration
    # ─────────────────────────────────────────────────────────────────────────────
    async def test_async_sdk():
        from fintext import FinTextAsyncClient, AnomalyScanResponse

        async with FinTextAsyncClient(base_url=BASE_URL, api_token=token) as async_client:
            scan_res = await async_client.trigger_anomaly_scan()
            ok = isinstance(scan_res, AnomalyScanResponse) and scan_res.anomalies_found > 0
            ws_url = async_client.get_anomaly_websocket_url()
            ok = ok and ("ws://" in ws_url and "streams=anomalies" in ws_url)
            return ok

    try:
        ok = asyncio.run(test_async_sdk())
        report(14, "Python SDK Async Client integration (async trigger_anomaly_scan)", ok)
    except Exception as e:
        report(14, "Python SDK Async Client integration (async trigger_anomaly_scan)", False, str(e))

    print("=" * 80)
    print(f"Results: {passed}/{total} Passed ({(passed/total)*100:.1f}%) | Failed: {failed}")
    print("=" * 80)

    return failed == 0


if __name__ == "__main__":
    with ServerContext():
        success = run_tests()
        sys.exit(0 if success else 1)
