#!/usr/bin/env python3
"""
===============================================================================
FinText-Alpha-Vectorizer — Suite #216: Streaming Kafka Topic Access Certification
===============================================================================
Verifies:
  1.  Unauthenticated GET /stream/kafka/topics returns 401 Unauthorized
  2.  List available Kafka streaming topics (GET /stream/kafka/topics -> 200 OK)
  3.  Topic metadata schema verification (sentiment-events, news-events, options-events)
  4.  Free tier plan authorization check (403 Forbidden on free role/plan)
  5.  Input validation error handling (missing/invalid topic, out-of-bounds TTL, bad consumer group -> 400)
  6.  Generate Kafka consumer credentials (GET /stream/kafka/credentials -> 200 OK)
  7.  Custom consumer group and TTL configuration
  8.  Multi-user credential isolation & security
  9.  Early credential revocation (DELETE /stream/kafka/credentials/{id} -> 200 OK)
  10. Idempotent / non-existent credential revocation error handling (404 Not Found)
  11. Python SDK sync client integration (list_kafka_topics, get_kafka_credentials, revoke_kafka_credentials)
  12. Python SDK async client integration (list_kafka_topics, get_kafka_credentials, revoke_kafka_credentials)
===============================================================================
"""

import asyncio
import os
from pathlib import Path
import subprocess
import sys
import time
import uuid

import httpx

if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
if hasattr(sys.stderr, "reconfigure"):
    sys.stderr.reconfigure(encoding="utf-8", errors="replace")

PROJECT_ROOT = Path(__file__).resolve().parent.parent
SDK_PATH = PROJECT_ROOT / "python_sdk" / "src"
if str(SDK_PATH) not in sys.path:
    sys.path.insert(0, str(SDK_PATH))

PORT = 8116
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


def get_auth_token(user_id: str = "kafka_trader_01", role: str = "institutional") -> str:
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
    print("  FinText-Alpha-Vectorizer — Suite #216: Streaming Kafka Topic Access Certification")
    print("=" * 79)

    with ServerContext():
        token_inst1 = get_auth_token("quant_fund_alpha", role="institutional")
        token_inst2 = get_auth_token("quant_fund_beta", role="institutional")
        token_free = get_auth_token("retail_user_gamma", role="free")

        headers_inst1 = {"Authorization": f"Bearer {token_inst1}"}
        headers_inst2 = {"Authorization": f"Bearer {token_inst2}"}
        headers_free = {"Authorization": f"Bearer {token_free}"}

        # ── Phase 1: Unauthenticated rejection ────────────────────────────────
        try:
            r = httpx.get(f"{BASE_URL}/stream/kafka/topics", timeout=5.0)
            ok = r.status_code == 401
            report(1, "Unauthenticated GET /stream/kafka/topics returns 401 Unauthorized", ok, f"status={r.status_code}")
        except Exception as e:
            report(1, "Unauthenticated GET /stream/kafka/topics returns 401 Unauthorized", False, str(e))

        # ── Phase 2: List available Kafka streaming topics ────────────────────
        try:
            r = httpx.get(f"{BASE_URL}/stream/kafka/topics", headers=headers_inst1, timeout=5.0)
            ok = r.status_code == 200
            data = r.json()
            topics = [t["topic"] for t in data.get("topics", [])]
            ok = ok and data.get("total_topics") == 3
            ok = ok and "sentiment-events" in topics
            ok = ok and "news-events" in topics
            ok = ok and "options-events" in topics
            report(2, "List available Kafka streaming topics (GET /stream/kafka/topics -> 200 OK)", ok, f"topics={topics}")
        except Exception as e:
            report(2, "List available Kafka streaming topics (GET /stream/kafka/topics -> 200 OK)", False, str(e))

        # ── Phase 3: Topic metadata schema verification ───────────────────────
        try:
            ok = True
            for t in data.get("topics", []):
                ok = ok and bool(t.get("topic"))
                ok = ok and bool(t.get("description"))
                ok = ok and bool(t.get("schema_description"))
                ok = ok and isinstance(t.get("example_payload"), dict)
                ok = ok and t.get("partitions", 0) > 0
                ok = ok and t.get("retention_hours", 0) > 0
            report(3, "Topic metadata schema verification (partitions, retention, examples)", ok)
        except Exception as e:
            report(3, "Topic metadata schema verification (partitions, retention, examples)", False, str(e))

        # ── Phase 4: Free tier plan authorization check ───────────────────────
        try:
            r_free = httpx.get(
                f"{BASE_URL}/stream/kafka/credentials",
                params={"topic": "sentiment-events"},
                headers=headers_free,
                timeout=5.0,
            )
            ok = r_free.status_code == 403
            err_json = r_free.json()
            ok = ok and "Streaming access requires Pro or Enterprise plan" in err_json.get("message", "")
            report(4, "Free tier plan authorization check (403 Forbidden)", ok, f"status={r_free.status_code}, msg={err_json.get('message')}")
        except Exception as e:
            report(4, "Free tier plan authorization check (403 Forbidden)", False, str(e))

        # ── Phase 5: Input validation error handling ──────────────────────────
        try:
            # 1. Missing topic
            r_no_topic = httpx.get(f"{BASE_URL}/stream/kafka/credentials", headers=headers_inst1, timeout=5.0)
            # 2. Invalid topic
            r_bad_topic = httpx.get(f"{BASE_URL}/stream/kafka/credentials", params={"topic": "crypto_pump_events"}, headers=headers_inst1, timeout=5.0)
            # 3. Invalid TTL (< 1)
            r_ttl_low = httpx.get(f"{BASE_URL}/stream/kafka/credentials", params={"topic": "sentiment-events", "ttl_minutes": 0}, headers=headers_inst1, timeout=5.0)
            # 4. Invalid TTL (> 1440)
            r_ttl_high = httpx.get(f"{BASE_URL}/stream/kafka/credentials", params={"topic": "sentiment-events", "ttl_minutes": 5000}, headers=headers_inst1, timeout=5.0)
            # 5. Invalid consumer group characters
            r_bad_cg = httpx.get(f"{BASE_URL}/stream/kafka/credentials", params={"topic": "sentiment-events", "consumer_group": "bad/consumer$group!!"}, headers=headers_inst1, timeout=5.0)

            ok = (
                r_no_topic.status_code == 400
                and r_bad_topic.status_code == 400
                and r_ttl_low.status_code == 400
                and r_ttl_high.status_code == 400
                and r_bad_cg.status_code == 400
            )
            report(5, "Input validation error handling (missing/invalid topic, TTL bounds, invalid CG -> 400)", ok,
                   f"no_topic={r_no_topic.status_code}, bad_topic={r_bad_topic.status_code}, ttl_low={r_ttl_low.status_code}, ttl_high={r_ttl_high.status_code}, bad_cg={r_bad_cg.status_code}")
        except Exception as e:
            report(5, "Input validation error handling (missing/invalid topic, TTL bounds, invalid CG -> 400)", False, str(e))

        # ── Phase 6: Generate Kafka consumer credentials ──────────────────────
        try:
            r_creds = httpx.get(
                f"{BASE_URL}/stream/kafka/credentials",
                params={"topic": "sentiment-events", "ttl_minutes": 60},
                headers=headers_inst1,
                timeout=5.0,
            )
            ok = r_creds.status_code == 200
            cred_data1 = r_creds.json()
            cred_id1 = cred_data1.get("id")
            ok = ok and bool(cred_id1)
            ok = ok and cred_data1.get("topic") == "sentiment-events"
            ok = ok and cred_data1.get("user_id") == "quant_fund_alpha"
            ok = ok and cred_data1.get("username", "").startswith("user_")
            ok = ok and cred_data1.get("password", "").startswith("sec_")
            ok = ok and bool(cred_data1.get("broker_address"))
            ok = ok and bool(cred_data1.get("consumer_group"))
            ok = ok and bool(cred_data1.get("expires_at"))
            report(6, "Generate Kafka consumer credentials (GET /stream/kafka/credentials -> 200 OK)", ok, f"creds={cred_data1.get('username')}")
        except Exception as e:
            report(6, "Generate Kafka consumer credentials (GET /stream/kafka/credentials -> 200 OK)", False, str(e))

        # ── Phase 7: Custom consumer group and TTL configuration ──────────────
        try:
            custom_cg = "hedge_fund_alpha_options_cg"
            r_custom = httpx.get(
                f"{BASE_URL}/stream/kafka/credentials",
                params={"topic": "options-events", "ttl_minutes": 180, "consumer_group": custom_cg},
                headers=headers_inst1,
                timeout=5.0,
            )
            ok = r_custom.status_code == 200
            cred_custom = r_custom.json()
            ok = ok and cred_custom.get("topic") == "options-events"
            ok = ok and cred_custom.get("consumer_group") == custom_cg
            report(7, "Custom consumer group and TTL configuration", ok, f"cg={cred_custom.get('consumer_group')}")
        except Exception as e:
            report(7, "Custom consumer group and TTL configuration", False, str(e))

        # ── Phase 8: Multi-user credential isolation ──────────────────────────
        try:
            # User 2 attempts to revoke User 1's credentials -> 404
            r_u2_del = httpx.delete(
                f"{BASE_URL}/stream/kafka/credentials/{cred_id1}",
                headers=headers_inst2,
                timeout=5.0,
            )
            ok = r_u2_del.status_code == 404
            report(8, "Multi-user credential isolation & security (cross-tenant delete rejected -> 404)", ok, f"status={r_u2_del.status_code}")
        except Exception as e:
            report(8, "Multi-user credential isolation & security (cross-tenant delete rejected -> 404)", False, str(e))

        # ── Phase 9: Early credential revocation ──────────────────────────────
        try:
            r_del = httpx.delete(
                f"{BASE_URL}/stream/kafka/credentials/{cred_id1}",
                headers=headers_inst1,
                timeout=5.0,
            )
            ok = r_del.status_code == 200
            del_data = r_del.json()
            ok = ok and del_data.get("status") == "revoked"
            ok = ok and del_data.get("id") == cred_id1
            report(9, "Early credential revocation (DELETE /stream/kafka/credentials/{id} -> 200 OK)", ok, f"resp={del_data}")
        except Exception as e:
            report(9, "Early credential revocation (DELETE /stream/kafka/credentials/{id} -> 200 OK)", False, str(e))

        # ── Phase 10: Idempotent / non-existent revocation handling ───────────
        try:
            # Delete non-existent UUID -> 404
            random_uuid = str(uuid.uuid4())
            r_404 = httpx.delete(
                f"{BASE_URL}/stream/kafka/credentials/{random_uuid}",
                headers=headers_inst1,
                timeout=5.0,
            )
            ok = r_404.status_code == 404
            report(10, "Non-existent credential revocation error handling (404 Not Found)", ok, f"status={r_404.status_code}")
        except Exception as e:
            report(10, "Non-existent credential revocation error handling (404 Not Found)", False, str(e))

        # ── Phase 11: Python SDK sync client integration ──────────────────────
        try:
            from fintext import FinTextClient

            token_sdk_sync = get_auth_token("sdk_sync_trader", role="institutional")
            with FinTextClient(base_url=BASE_URL, api_token=token_sdk_sync) as client:
                # 1. List topics
                topics_resp = client.list_kafka_topics()
                ok = topics_resp.total_topics == 3
                ok = ok and len(topics_resp.topics) == 3

                # 2. Get credentials
                creds = client.get_kafka_credentials(
                    topic="news-events",
                    ttl_minutes=45,
                    consumer_group="sdk_sync_news_group",
                )
                ok = ok and creds.topic == "news-events"
                ok = ok and creds.consumer_group == "sdk_sync_news_group"
                ok = ok and creds.password.startswith("sec_")

                # 3. Revoke credentials
                rev = client.revoke_kafka_credentials(creds.id)
                ok = ok and rev.status == "revoked"
                ok = ok and rev.id == creds.id

            report(11, "Python SDK sync client integration (list, get_credentials, revoke)", ok)
        except Exception as e:
            report(11, "Python SDK sync client integration (list, get_credentials, revoke)", False, str(e))

        # ── Phase 12: Python SDK async client integration ─────────────────────
        async def verify_async():
            from fintext import FinTextAsyncClient

            token_sdk_async = get_auth_token("sdk_async_trader", role="institutional")
            async with FinTextAsyncClient(base_url=BASE_URL, api_token=token_sdk_async) as client:
                # 1. List topics
                topics_resp = await client.list_kafka_topics()
                ok = topics_resp.total_topics == 3

                # 2. Get credentials
                creds = await client.get_kafka_credentials(
                    topic="sentiment-events",
                    ttl_minutes=90,
                    consumer_group="sdk_async_sentiment_group",
                )
                ok = ok and creds.topic == "sentiment-events"
                ok = ok and creds.consumer_group == "sdk_async_sentiment_group"

                # 3. Revoke credentials
                rev = await client.revoke_kafka_credentials(creds.id)
                ok = ok and rev.status == "revoked"
                return ok

        try:
            ok_async = asyncio.run(verify_async())
            report(12, "Python SDK async client integration (list, get_credentials, revoke)", ok_async)
        except Exception as e:
            report(12, "Python SDK async client integration (list, get_credentials, revoke)", False, str(e))

    # ── Summary ──────────────────────────────────────────────────────────────
    print("\n" + "=" * 79)
    pct = (passed / total) * 100.0
    print(f"  Suite #216 Results: {passed}/{total} Passed ({pct:.1f}%)")
    print("=" * 79)
    if failed == 0:
        print("\n🎉 ALL 12 PHASES PASSED CLEANLY!\n")
    else:
        print(f"\n❌ {failed} PHASES FAILED!\n")
        sys.exit(1)


if __name__ == "__main__":
    main()
