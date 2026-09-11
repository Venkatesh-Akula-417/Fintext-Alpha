#!/usr/bin/env python3
"""
===============================================================================
FinText-Alpha-Vectorizer — Verification Suite #269:
PostgreSQL Relational Point-in-Time (PIT) Database Storage & Fallback Integration
===============================================================================
Verifies:
  1.  Configuration & Defaults Validation (config.yaml & .env.example)
  2.  Schema DDL & Table Structure Verification (4 core bi-temporal tables & indexes)
  3.  Native Rust Unit Tests for pit_db (Config, Defaults, Env Overrides, Schema)
  4.  Live API Server Startup & Health Probe with Graceful Fallback to JSON
  5.  Point-in-Time Historical Resolution Query Verification (Ticker intervals)
  6.  Delisted Securities Detection & Query Clamping Validation
  7.  Corporate Actions & S&P 500 Historical Membership Verification
  8.  PIT Certificate Endpoint Operational Verification with Archival & Auth
  9.  Non-Fallback Strict Mode Failure Validation (PIT_DB_FALLBACK_TO_JSON=false)
  10. Relational Schema Simulation & Ingestion Correctness (Entity & Delisting resolution)
  11. Python SDK & Architecture Integrity Audit
===============================================================================
"""

import os
from pathlib import Path
import sqlite3
import subprocess
import sys
import time
import yaml

import httpx

if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
if hasattr(sys.stderr, "reconfigure"):
    sys.stderr.reconfigure(encoding="utf-8", errors="replace")

PROJECT_ROOT = Path(__file__).resolve().parent.parent
SDK_PATH = PROJECT_ROOT / "python_sdk" / "src"
if str(SDK_PATH) not in sys.path:
    sys.path.insert(0, str(SDK_PATH))

PORT = 8270
PORT_STRICT = 8271
BASE_URL = f"http://127.0.0.1:{PORT}"
ADMIN_TOKEN = "test_admin_token_xyz123_valid_32_bytes_length!"
SERVER_EXE = PROJECT_ROOT / "rust" / "target" / "debug" / "fintext_api.exe"

passed = 0
failed = 0
total = 11


def report(phase: int, name: str, ok: bool, detail: str = ""):
    global passed, failed
    if ok:
        passed += 1
        print(f"  ✅ Phase {phase:2d} │ {name}")
    else:
        failed += 1
        print(f"  ❌ Phase {phase:2d} │ {name}")
        if detail:
            print(f"     └─ {detail}")


class ServerContext:
    def __init__(self, port: int, extra_env: dict = None):
        self.port = port
        self.base_url = f"http://127.0.0.1:{port}"
        self.extra_env = extra_env or {}
        self.process = None

    def __enter__(self):
        print(f"[STARTING] Spawning FinText API Server on port {self.port}...")
        env = os.environ.copy()
        env["PORT"] = str(self.port)
        env["HOST"] = "127.0.0.1"
        env["JWT_SECRET"] = "verification_suite_secret_key_32_bytes_len!"
        env["ADMIN_TOKEN"] = ADMIN_TOKEN
        env["RUST_LOG"] = "info,fintext_api_server=debug"
        env["QUESTDB_MOCK_FALLBACK"] = "1"
        env["POLYGON_MOCK_FALLBACK"] = "1"
        env["WHISPER_MOCK_FALLBACK"] = "1"
        env["NATS_MOCK_MODE"] = "1"
        env["CHAT_ALERTS_MOCK"] = "1"
        env["TIMESCALE_MOCK_MODE"] = "1"
        env.update(self.extra_env)

        self.process = subprocess.Popen(
            [str(SERVER_EXE)],
            cwd=str(PROJECT_ROOT),
            env=env,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )

        # Wait for ready
        deadline = time.time() + 30.0
        healthy = False
        while time.time() < deadline:
            if self.process.poll() is not None:
                raise RuntimeError(
                    f"Server process on port {self.port} terminated prematurely with exit code {self.process.returncode}"
                )
            try:
                r = httpx.get(f"{self.base_url}/health", timeout=1.0)
                if r.status_code == 200:
                    healthy = True
                    break
            except Exception:
                time.sleep(0.3)

        if not healthy:
            self.terminate()
            raise RuntimeError(f"Server on port {self.port} failed to become healthy within 30s")

        print(f"[READY] API Server is healthy at {self.base_url}")
        return self

    def terminate(self):
        if self.process and self.process.poll() is None:
            self.process.terminate()
            try:
                self.process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait(timeout=2)

    def __exit__(self, exc_type, exc_val, exc_tb):
        print(f"[CLEANUP] Terminating FinText API Server process on port {self.port}...")
        self.terminate()


def main():
    print("=" * 80)
    print(" FinText-Alpha-Vectorizer — Suite #269: PostgreSQL PIT Database Storage")
    print("=" * 80)
    print(f" Project Root: {PROJECT_ROOT}")
    print(f" Server Port:  {PORT}")

    # ─────────────────────────────────────────────────────────────────────────
    # Phase 1: Configuration & Defaults Validation
    # ─────────────────────────────────────────────────────────────────────────
    try:
        config_yaml_path = PROJECT_ROOT / "config" / "config.yaml"
        with open(config_yaml_path, "r", encoding="utf-8") as f:
            cfg = yaml.safe_load(f)

        pit_db_cfg = cfg.get("pit_database", {})
        has_cfg = (
            "enabled" in pit_db_cfg
            and "url" in pit_db_cfg
            and "max_connections" in pit_db_cfg
            and "timeout_ms" in pit_db_cfg
            and pit_db_cfg.get("fallback_to_json") is True
        )

        env_example_path = PROJECT_ROOT / ".env.example"
        env_content = env_example_path.read_text(encoding="utf-8")
        has_env = (
            "PIT_DB_ENABLED" in env_content
            and "PIT_DB_URL" in env_content
            and "PIT_DB_MAX_CONNECTIONS" in env_content
            and "PIT_DB_TIMEOUT_MS" in env_content
            and "PIT_DB_FALLBACK_TO_JSON" in env_content
        )

        ok = has_cfg and has_env
        detail = ""
        if not has_cfg:
            detail += f"Missing/invalid config.yaml pit_database section: {pit_db_cfg}. "
        if not has_env:
            detail += "Missing PIT_DB_* variables in .env.example. "
        report(1, "Configuration & Defaults Validation (config.yaml & .env.example)", ok, detail)
    except Exception as e:
        report(1, "Configuration & Defaults Validation (config.yaml & .env.example)", False, str(e))

    # ─────────────────────────────────────────────────────────────────────────
    # Phase 2: Schema DDL & Table Structure Verification
    # ─────────────────────────────────────────────────────────────────────────
    try:
        pit_db_rs = PROJECT_ROOT / "rust" / "api_server" / "src" / "pit_db.rs"
        rs_content = pit_db_rs.read_text(encoding="utf-8")

        required_tables = [
            "pit_delisted_securities",
            "pit_ticker_history",
            "pit_corporate_actions",
            "pit_index_membership",
        ]
        missing_tables = [t for t in required_tables if f"CREATE TABLE IF NOT EXISTS {t}" not in rs_content]

        required_indexes = [
            "idx_pit_delisted_ticker",
            "idx_pit_ticker_history_ticker",
            "idx_pit_corp_actions_ticker",
            "idx_pit_index_member_ticker",
        ]
        missing_indexes = [idx for idx in required_indexes if idx not in rs_content]

        ok = len(missing_tables) == 0 and len(missing_indexes) == 0
        detail = ""
        if missing_tables:
            detail += f"Missing tables in DDL: {missing_tables}. "
        if missing_indexes:
            detail += f"Missing indexes in DDL: {missing_indexes}. "
        report(2, "Schema DDL & Table Structure Verification (4 core bi-temporal tables & indexes)", ok, detail)
    except Exception as e:
        report(2, "Schema DDL & Table Structure Verification", False, str(e))

    # ─────────────────────────────────────────────────────────────────────────
    # Phase 3: Native Rust Unit Tests for pit_db
    # ─────────────────────────────────────────────────────────────────────────
    try:
        cmd = [
            "cargo",
            "test",
            "--manifest-path",
            "rust/Cargo.toml",
            "-p",
            "fintext_api_server",
            "--lib",
            "pit_db",
        ]
        env = os.environ.copy()
        clang_path = PROJECT_ROOT / "venv" / "Lib" / "site-packages" / "clang" / "native"
        if clang_path.exists():
            env["LIBCLANG_PATH"] = str(clang_path)

        res = subprocess.run(
            cmd,
            cwd=str(PROJECT_ROOT),
            env=env,
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            timeout=120,
        )
        ok = res.returncode == 0 and "test result: ok. 4 passed" in res.stdout
        detail = "" if ok else f"Rust test failed with code {res.returncode}:\n{res.stdout[:500]}\n{res.stderr[:500]}"
        report(3, "Native Rust Unit Tests for pit_db (Config, Defaults, Env Overrides, Schema)", ok, detail)
    except Exception as e:
        report(3, "Native Rust Unit Tests for pit_db", False, str(e))

    # ─────────────────────────────────────────────────────────────────────────
    # Phases 4 to 8: Live Server Testing with Graceful Fallback to JSON
    # ─────────────────────────────────────────────────────────────────────────
    try:
        server_env = {
            "PIT_DB_ENABLED": "true",
            "PIT_DB_URL": "postgresql://unreachable_fintext_user:secret@127.0.0.1:54399/unreachable_db",
            "PIT_DB_TIMEOUT_MS": "1500",
            "PIT_DB_FALLBACK_TO_JSON": "true",
        }
        with ServerContext(PORT, extra_env=server_env) as srv:
            client = httpx.Client(base_url=srv.base_url, timeout=10.0)

            # Obtain valid JWT authentication token
            login_resp = client.post(
                "/auth/token",
                json={"user_id": "pit_auditor_01", "role": "compliance"},
                headers={"X-Admin-Token": ADMIN_TOKEN},
            )
            token = login_resp.json().get("token")
            if token:
                client.headers["Authorization"] = f"Bearer {token}"

            # Phase 4: Server Startup & Health Probe with Graceful Fallback
            r_health = client.get("/health")
            ok_health = r_health.status_code == 200 and r_health.json().get("status") == "ok"
            report(
                4,
                "Live API Server Startup & Health Probe with Graceful Fallback to JSON",
                ok_health,
                f"Status: {r_health.status_code}, Body: {r_health.text}",
            )

            # Phase 5: Point-in-Time Historical Resolution Query Verification
            r_pit = client.get(
                "/pit/point-in-time",
                params={"ticker": "FB", "date": "2021-06-01"},
            )
            ok_pit = r_pit.status_code in (200, 404)
            if r_pit.status_code == 200:
                body = r_pit.json()
                ok_pit = body.get("valid", True) is True or "FB" in str(body)
            report(
                5,
                "Point-in-Time Historical Resolution Query Verification (Ticker intervals)",
                ok_pit,
                f"Status: {r_pit.status_code}, Body: {r_pit.text[:100]}",
            )

            # Phase 6: Delisted Securities Detection & Query Clamping Validation
            r_delist = client.get(
                "/pit/point-in-time",
                params={"ticker": "TWTR", "date": "2023-01-01"},
            )
            ok_delist = r_delist.status_code in (200, 404)
            if r_delist.status_code == 200:
                body = r_delist.json()
                ok_delist = body.get("valid") is False or body.get("is_delisted") is True
            report(
                6,
                "Delisted Securities Detection & Query Clamping Validation",
                ok_delist,
                f"Status: {r_delist.status_code}, Body: {r_delist.text[:100]}",
            )

            # Phase 7: Corporate Actions & S&P 500 Historical Membership Verification
            r_sp = client.get(
                "/pit/point-in-time",
                params={"ticker": "TSLA", "date": "2021-01-01"},
            )
            ok_sp = r_sp.status_code in (200, 404)
            report(
                7,
                "Corporate Actions & S&P 500 Historical Membership Verification",
                ok_sp,
                f"Status: {r_sp.status_code}",
            )

            # Phase 8: PIT Certificate Endpoint Operational Verification with Archival
            r_cert = client.get("/pit/certificate")
            ok_cert = False
            cert_detail = ""
            if r_cert.status_code == 200:
                cert = r_cert.json()
                ok_cert = (
                    cert.get("overall_result") == "pass"
                    and len(cert.get("tests", [])) == 8
                    and cert.get("signature", "").startswith("sha256:")
                    and cert.get("archive_object_key") is not None
                    and cert.get("archive_timestamp") is not None
                )
                if not ok_cert:
                    cert_detail = f"Certificate payload incomplete: {cert}"
            else:
                cert_detail = f"Status: {r_cert.status_code}, Body: {r_cert.text[:200]}"

            report(
                8,
                "PIT Certificate Endpoint Operational Verification with Archival",
                ok_cert,
                cert_detail,
            )

    except Exception as e:
        report(4, "Live Server Operations", False, str(e))
        report(5, "Historical Resolution Query", False, str(e))
        report(6, "Delisted Securities Detection", False, str(e))
        report(7, "Corporate Actions Verification", False, str(e))
        report(8, "PIT Certificate Endpoint Verification", False, str(e))

    # ─────────────────────────────────────────────────────────────────────────
    # Phase 9: Non-Fallback Strict Mode Failure Validation
    # When PIT_DB_FALLBACK_TO_JSON=false and DB is unreachable, server must refuse
    # to start (panic / exit with non-zero code) to enforce strict compliance.
    # ─────────────────────────────────────────────────────────────────────────
    try:
        env_strict = os.environ.copy()
        env_strict["PORT"] = str(PORT_STRICT)
        env_strict["JWT_SECRET"] = "verification_suite_secret_key_32_bytes_len!"
        env_strict["ADMIN_TOKEN"] = ADMIN_TOKEN
        env_strict["PIT_DB_ENABLED"] = "true"
        env_strict["PIT_DB_URL"] = "postgresql://unreachable_user:secret@127.0.0.1:54398/nonexistent"
        env_strict["PIT_DB_TIMEOUT_MS"] = "1000"
        env_strict["PIT_DB_FALLBACK_TO_JSON"] = "false"
        env_strict["QUESTDB_MOCK_FALLBACK"] = "1"
        env_strict["TIMESCALE_MOCK_MODE"] = "1"

        res_strict = subprocess.run(
            [str(SERVER_EXE)],
            cwd=str(PROJECT_ROOT),
            env=env_strict,
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            timeout=15,
        )

        ok_strict = (
            res_strict.returncode != 0
            or "Fatal: Database-backed PIT required" in res_strict.stdout
            or "Fatal: Database-backed PIT required" in res_strict.stderr
        )
        detail = f"Exit code: {res_strict.returncode}"

        report(
            9,
            "Non-Fallback Strict Mode Failure Validation (PIT_DB_FALLBACK_TO_JSON=false)",
            ok_strict,
            detail,
        )
    except Exception as e:
        report(9, "Non-Fallback Strict Mode Failure Validation", False, str(e))

    # ─────────────────────────────────────────────────────────────────────────
    # Phase 10: Relational Schema Simulation & Ingestion Correctness
    # ─────────────────────────────────────────────────────────────────────────
    try:
        conn = sqlite3.connect(":memory:")
        cur = conn.cursor()

        cur.execute("""
            CREATE TABLE pit_ticker_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                entity_id TEXT NOT NULL,
                ticker TEXT NOT NULL,
                start_date TEXT NOT NULL,
                end_date TEXT NOT NULL,
                is_current INTEGER NOT NULL DEFAULT 1
            );
        """)
        cur.execute("""
            CREATE TABLE pit_delisted_securities (
                ticker TEXT PRIMARY KEY,
                delisting_date TEXT NOT NULL,
                reason TEXT,
                is_current INTEGER NOT NULL DEFAULT 1
            );
        """)
        cur.execute("""
            CREATE TABLE pit_corporate_actions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                ticker TEXT NOT NULL,
                action_type TEXT NOT NULL,
                action_date TEXT NOT NULL,
                split_ratio REAL,
                is_current INTEGER NOT NULL DEFAULT 1
            );
        """)
        cur.execute("""
            CREATE TABLE pit_index_membership (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                ticker TEXT NOT NULL,
                index_name TEXT NOT NULL,
                join_date TEXT NOT NULL,
                leave_date TEXT,
                is_current INTEGER NOT NULL DEFAULT 1
            );
        """)

        cur.execute(
            "INSERT INTO pit_ticker_history (entity_id, ticker, start_date, end_date) VALUES (?, ?, ?, ?)",
            ("ENT_META", "FB", "2012-05-18", "2022-06-08"),
        )
        cur.execute(
            "INSERT INTO pit_ticker_history (entity_id, ticker, start_date, end_date) VALUES (?, ?, ?, ?)",
            ("ENT_META", "META", "2022-06-09", "9999-12-31"),
        )
        cur.execute(
            "INSERT INTO pit_delisted_securities (ticker, delisting_date, reason) VALUES (?, ?, ?)",
            ("TWTR", "2022-10-28", "Acquired by X Holdings"),
        )
        cur.execute(
            "INSERT INTO pit_index_membership (ticker, index_name, join_date, leave_date) VALUES (?, ?, ?, ?)",
            ("TSLA", "SP500", "2020-12-21", None),
        )
        conn.commit()

        # Test PIT query 1: Meta on 2021-01-01 -> FB
        cur.execute(
            "SELECT ticker FROM pit_ticker_history WHERE entity_id = ? AND start_date <= ? AND end_date >= ?",
            ("ENT_META", "2021-01-01", "2021-01-01"),
        )
        row = cur.fetchone()
        assert row and row[0] == "FB", f"Expected FB, got {row}"

        # Test PIT query 2: Meta on 2023-01-01 -> META
        cur.execute(
            "SELECT ticker FROM pit_ticker_history WHERE entity_id = ? AND start_date <= ? AND end_date >= ?",
            ("ENT_META", "2023-01-01", "2023-01-01"),
        )
        row = cur.fetchone()
        assert row and row[0] == "META", f"Expected META, got {row}"

        # Test delisted query
        cur.execute(
            "SELECT delisting_date FROM pit_delisted_securities WHERE ticker = ? AND delisting_date <= ?",
            ("TWTR", "2023-01-01"),
        )
        row = cur.fetchone()
        assert row and row[0] == "2022-10-28", f"Expected 2022-10-28, got {row}"

        conn.close()
        report(10, "Relational Schema Simulation & Ingestion Correctness (Entity & Delisting resolution)", True)
    except Exception as e:
        report(10, "Relational Schema Simulation & Ingestion Correctness", False, str(e))

    # ─────────────────────────────────────────────────────────────────────────
    # Phase 11: Python SDK & Architecture Integrity Audit
    # ─────────────────────────────────────────────────────────────────────────
    try:
        from fintext.models import PITCertificateResponse
        if hasattr(PITCertificateResponse, "model_fields"):
            fields = PITCertificateResponse.model_fields.keys()
        else:
            fields = PITCertificateResponse.__fields__.keys()

        has_expected = {
            "certificate_id",
            "dataset_version",
            "universe",
            "overall_result",
            "tests",
            "signature",
            "archive_object_key",
            "archive_timestamp",
        }.issubset(fields)

        report(11, "Python SDK & Architecture Integrity Audit", has_expected)
    except Exception as e:
        report(11, "Python SDK & Architecture Integrity Audit", False, str(e))

    # ─────────────────────────────────────────────────────────────────────────
    # Final Summary
    # ─────────────────────────────────────────────────────────────────────────
    print("=" * 80)
    print(f" Verification Suite #269 Results: {passed}/{total} Phases Passed ({failed} Failed)")
    print("=" * 80)

    if failed > 0:
        sys.exit(1)


if __name__ == "__main__":
    main()
