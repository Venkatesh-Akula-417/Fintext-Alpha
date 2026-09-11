"""
═══════════════════════════════════════════════════════════════════════════════
FinText-Alpha-Vectorizer — Native Rust Axum API Gateway Certification Suite
═══════════════════════════════════════════════════════════════════════════════

Suite #180: Native Rust Axum HTTP & WebSocket API Gateway (QuestDB SQL, Kafka, Backtest & JWT Auth)
═══════════════════════════════════════════════════════════════════════════════
"""

import json
import os
from pathlib import Path
import socket
import subprocess
import sys
import time
import urllib.error
import urllib.request
import unittest

PROJECT_ROOT = Path(__file__).resolve().parent.parent
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))


class TestRustAxumApi(unittest.TestCase):
    """Verifies the native Rust Axum HTTP API server, QuestDB SQL querying, WebSocket push, spillovers, backtest, and JWT authentication."""

    def test_01_cargo_test_passes(self):
        """Verify that all Rust unit tests in fintext_api_server pass cleanly."""
        res = subprocess.run(
            ["cargo", "test", "--manifest-path", "rust/Cargo.toml", "--package", "fintext_api_server"],
            cwd=str(PROJECT_ROOT),
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
        )
        self.assertEqual(res.returncode, 0, f"Cargo test failed:\n{res.stderr}\n{res.stdout}")
        self.assertIn("0 failed", res.stdout)
        self.assertIn("passed", res.stdout)

    def test_02_release_binary_built(self):
        """Verify that the optimized release binary is present and executable."""
        bin_path = PROJECT_ROOT / "rust" / "target" / "release" / ("fintext_api.exe" if sys.platform == "win32" else "fintext_api")
        self.assertTrue(bin_path.exists(), f"Release binary not found at {bin_path}")

    def test_03_live_http_server_endpoints(self):
        """Verify live HTTP endpoints (/health, /auth/token, /sentiment, /spillovers, and /backtest) and WebSocket (/ws) on the native Axum server."""
        bin_path = PROJECT_ROOT / "rust" / "target" / "release" / ("fintext_api.exe" if sys.platform == "win32" else "fintext_api")

        env = os.environ.copy()
        env["QUESTDB_MOCK_FALLBACK"] = "1"
        env["NATS_MOCK_MODE"] = "1"
        env["JWT_SECRET"] = "suite180-institutional-jwt-secret-key-2026"
        env["ADMIN_TOKEN"] = "suite180-admin-token"

        proc = subprocess.Popen(
            [str(bin_path)],
            cwd=str(PROJECT_ROOT),
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            env=env,
        )

        # Wait for server to bind and listen
        time.sleep(1.5)

        base_url = "http://127.0.0.1:8000"

        try:
            # 1. Test GET /health (Public)
            req = urllib.request.Request(f"{base_url}/health")
            with urllib.request.urlopen(req, timeout=3) as resp:
                self.assertEqual(resp.status, 200)
                data = json.loads(resp.read().decode("utf-8"))
                self.assertEqual(data["status"], "ok")
                self.assertEqual(data["version"], "2.0.0-institutional")
                self.assertIn("timestamp_us", data)

            # 2. Test GET /sentiment WITHOUT token -> 401 Unauthorized
            req = urllib.request.Request(f"{base_url}/sentiment?ticker=AAPL")
            try:
                urllib.request.urlopen(req, timeout=3)
                self.fail("Expected HTTPError 401 Unauthorized for unauthenticated request")
            except urllib.error.HTTPError as err:
                self.assertEqual(err.code, 401)
                err.close()

            # 3. Test POST /auth/token (Obtain JWT with Admin Token)
            token_req_payload = json.dumps({
                "user_id": "suite180_quant_user",
                "expires_in_seconds": 3600,
                "role": "institutional",
            }).encode("utf-8")

            token_req = urllib.request.Request(
                f"{base_url}/auth/token",
                data=token_req_payload,
                headers={
                    "Content-Type": "application/json",
                    "X-Admin-Token": "suite180-admin-token",
                },
                method="POST",
            )
            with urllib.request.urlopen(token_req, timeout=3) as resp:
                self.assertEqual(resp.status, 200)
                token_data = json.loads(resp.read().decode("utf-8"))
                self.assertEqual(token_data["user_id"], "suite180_quant_user")
                self.assertEqual(token_data["token_type"], "Bearer")
                token = token_data["token"]
                self.assertTrue(len(token) > 20)

            auth_headers = {
                "Authorization": f"Bearer {token}",
                "Content-Type": "application/json",
            }

            # 4. Test GET /sentiment (with valid JWT token)
            req = urllib.request.Request(
                f"{base_url}/sentiment?ticker=AAPL&date=2026-08-25",
                headers={"Authorization": f"Bearer {token}"},
            )
            with urllib.request.urlopen(req, timeout=3) as resp:
                self.assertEqual(resp.status, 200)
                data = json.loads(resp.read().decode("utf-8"))
                self.assertEqual(data["ticker"], "AAPL")
                self.assertEqual(data["date"], "2026-08-25")
                self.assertIn("sentiment_score", data)
                self.assertIn("sentiment_label", data)
                self.assertIn("signal_available_ts_us", data)
                self.assertGreater(data["signal_available_ts_us"], 0)

            # 5. Test GET /sentiment (missing ticker -> 400 Bad Request)
            req = urllib.request.Request(
                f"{base_url}/sentiment?ticker=",
                headers={"Authorization": f"Bearer {token}"},
            )
            try:
                urllib.request.urlopen(req, timeout=3)
                self.fail("Expected HTTPError 400")
            except urllib.error.HTTPError as err:
                self.assertEqual(err.code, 400)
                err.close()

            # 6. Test GET /sentiment (invalid ticker length -> 400 Bad Request)
            req = urllib.request.Request(
                f"{base_url}/sentiment?ticker=WAYTOOLONGTICKERNAME",
                headers={"Authorization": f"Bearer {token}"},
            )
            try:
                urllib.request.urlopen(req, timeout=3)
                self.fail("Expected HTTPError 400")
            except urllib.error.HTTPError as err:
                self.assertEqual(err.code, 400)
                err.close()

            # 7. Test GET /spillovers (valid parameters)
            req = urllib.request.Request(
                f"{base_url}/spillovers?ticker=AAPL&limit=10",
                headers={"Authorization": f"Bearer {token}"},
            )
            with urllib.request.urlopen(req, timeout=3) as resp:
                self.assertEqual(resp.status, 200)
                data = json.loads(resp.read().decode("utf-8"))
                self.assertEqual(data["ticker"], "AAPL")
                self.assertEqual(data["status"], "ok")
                self.assertGreaterEqual(data["count"], 2)
                self.assertTrue(len(data["spillovers"]) >= 2)
                self.assertEqual(data["spillovers"][0]["related_ticker"], "MSFT")
                self.assertEqual(data["spillovers"][0]["lag_hours"], 1)
                self.assertIn("AAPL LEADS MSFT", data["spillovers"][0]["relationship"])

            # 8. Test GET /spillovers (missing ticker -> 400 Bad Request)
            req = urllib.request.Request(
                f"{base_url}/spillovers?ticker=",
                headers={"Authorization": f"Bearer {token}"},
            )
            try:
                urllib.request.urlopen(req, timeout=3)
                self.fail("Expected HTTPError 400")
            except urllib.error.HTTPError as err:
                self.assertEqual(err.code, 400)
                err.close()

            # 9. Test POST /backtest (valid strategy parameters)
            payload = json.dumps({
                "ticker": "AAPL",
                "start_date": "2025-01-01",
                "end_date": "2025-03-31",
                "long_threshold": 0.2,
                "short_threshold": -0.2,
                "holding_days": 5,
                "initial_capital": 1000000.0,
            }).encode("utf-8")

            req = urllib.request.Request(
                f"{base_url}/backtest",
                data=payload,
                headers=auth_headers,
                method="POST",
            )
            with urllib.request.urlopen(req, timeout=3) as resp:
                self.assertEqual(resp.status, 200)
                data = json.loads(resp.read().decode("utf-8"))
                self.assertEqual(data["ticker"], "AAPL")
                self.assertEqual(data["start_date"], "2025-01-01")
                self.assertEqual(data["end_date"], "2025-03-31")
                self.assertIn("total_return", data)
                self.assertIn("annualized_return", data)
                self.assertIn("sharpe_ratio", data)
                self.assertIn("max_drawdown", data)
                self.assertIn("num_trades", data)
                self.assertIn("win_rate", data)
                self.assertIn("equity_curve", data)
                self.assertGreaterEqual(len(data["equity_curve"]), 30)

            # 10. Test POST /backtest (invalid date range -> 400 Bad Request)
            bad_payload = json.dumps({
                "ticker": "AAPL",
                "start_date": "2025-12-31",
                "end_date": "2025-01-01",
            }).encode("utf-8")

            bad_req = urllib.request.Request(
                f"{base_url}/backtest",
                data=bad_payload,
                headers=auth_headers,
                method="POST",
            )
            try:
                urllib.request.urlopen(bad_req, timeout=3)
                self.fail("Expected HTTPError 400 for inverted date range")
            except urllib.error.HTTPError as err:
                self.assertEqual(err.code, 400)
                err.close()

            # 11. Test WebSocket /ws Upgrade Handshake with Query Param Token
            s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
            s.settimeout(3.0)
            s.connect(("127.0.0.1", 8000))
            upgrade_req = (
                f"GET /ws?token={token} HTTP/1.1\r\n"
                f"Host: 127.0.0.1:8000\r\n"
                f"Upgrade: websocket\r\n"
                f"Connection: Upgrade\r\n"
                f"Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n"
                f"Sec-WebSocket-Version: 13\r\n\r\n"
            ).encode("utf-8")
            s.sendall(upgrade_req)
            resp_header = s.recv(1024).lower()
            s.close()

            self.assertIn(b"101 switching protocols", resp_header, "WebSocket upgrade handshake failed")
            self.assertIn(b"upgrade: websocket", resp_header)

            # 12. Test Public GET /api-docs/openapi.json (OpenAPI 3.0 Spec)
            req = urllib.request.Request(f"{base_url}/api-docs/openapi.json")
            with urllib.request.urlopen(req, timeout=3) as resp:
                self.assertEqual(resp.status, 200)
                spec = json.loads(resp.read().decode("utf-8"))
                self.assertEqual(spec["info"]["title"], "FinText-Alpha-Vectorizer API")
                self.assertIn("/health", spec["paths"])
                self.assertIn("/auth/token", spec["paths"])
                self.assertIn("/sentiment", spec["paths"])
                self.assertIn("/spillovers", spec["paths"])
                self.assertIn("/backtest", spec["paths"])
                self.assertIn("bearerAuth", spec["components"]["securitySchemes"])

            # 13. Test Public GET /swagger-ui/ (Swagger UI HTML)
            req = urllib.request.Request(f"{base_url}/swagger-ui/")
            with urllib.request.urlopen(req, timeout=3) as resp:
                self.assertEqual(resp.status, 200)
                html_body = resp.read().decode("utf-8")
                self.assertTrue("swagger-ui" in html_body.lower() or "<!doctype html>" in html_body.lower())

        finally:
            proc.terminate()
            try:
                proc.wait(timeout=3)
            except subprocess.TimeoutExpired:
                proc.kill()


if __name__ == "__main__":
    unittest.main()
