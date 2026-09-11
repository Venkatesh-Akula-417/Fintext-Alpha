"""
FinText Point-in-Time (PIT) Correctness Scenarios & Database Adapter

Executes:
  - Schema isolation in dedicated 'fintext_pit_validation_test'
  - Deterministic fixture seeding
  - Canonical AS-OF scenarios S1–S8
  - Negative control N1 (intentional look-ahead sensitivity proof)
"""

from datetime import datetime, timezone
import json
import sqlite3
from typing import Any, Dict, List, Optional, Tuple
from urllib.parse import urlparse

from .fixtures import (
    CORPORATE_ACTIONS_FIXTURES,
    DELISTED_SECURITIES_FIXTURES,
    INDEX_MEMBERSHIP_FIXTURES,
    SENTIMENT_RECORDS_FIXTURES,
    TICKER_HISTORY_FIXTURES,
)

SCHEMA_NAME = "fintext_pit_validation_test"

# ═══════════════════════════════════════════════════════════════════════════════
# Database Abstraction Layer
# ═══════════════════════════════════════════════════════════════════════════════

class DatabaseAdapter:
    """Unified database interface supporting PostgreSQL (via psycopg) and SQLite."""

    def __init__(self, database_url: str):
        self.raw_url = database_url
        self.parsed = urlparse(database_url) if "://" in database_url else None
        self.is_postgres = bool(
            self.parsed and self.parsed.scheme in ("postgres", "postgresql")
        )
        self.conn = None
        self._pg_engine_str = ""

    @property
    def is_production_stack(self) -> bool:
        return self.is_postgres

    @property
    def engine_name(self) -> str:
        if self.is_postgres:
            return self._pg_engine_str or "PostgreSQL 16"
        return f"SQLite in-memory (FALLBACK — not production stack, sqlite {sqlite3.sqlite_version})"

    @property
    def sanitized_host(self) -> str:
        """Returns host/port with zero credential leakage."""
        if self.is_postgres and self.parsed:
            host = self.parsed.hostname or "localhost"
            port = self.parsed.port or 5432
            dbname = self.parsed.path.lstrip("/") or "postgres"
            return f"{host}:{port}/{dbname}"
        return "in-memory-sqlite"

    def connect(self):
        if self.is_postgres:
            try:
                import psycopg
                from psycopg.rows import dict_row
            except ImportError as e:
                raise RuntimeError(
                    f"psycopg>=3.1.0 is required for PostgreSQL connections: {e}"
                )
            self.conn = psycopg.connect(
                self.raw_url,
                row_factory=dict_row,
                autocommit=True,
                connect_timeout=3,
            )
            try:
                with self.conn.cursor() as cur:
                    cur.execute("SELECT version();")
                    row = cur.fetchone()
                    pg_ver = row["version"].split()[1] if row and "version" in row else "16.x"
                    try:
                        cur.execute("SELECT extversion FROM pg_extension WHERE extname = 'timescaledb';")
                        ts_row = cur.fetchone()
                        if ts_row:
                            self._pg_engine_str = f"PostgreSQL {pg_ver} + TimescaleDB {ts_row['extversion']}"
                        else:
                            self._pg_engine_str = f"PostgreSQL {pg_ver} (TimescaleDB extension NOT available in this environment)"
                    except Exception:
                        self._pg_engine_str = f"PostgreSQL {pg_ver}"
            except Exception:
                self._pg_engine_str = "PostgreSQL 16"
        else:
            # SQLite in-memory or file
            db_path = ":memory:"
            if self.raw_url.startswith("sqlite:///"):
                db_path = self.raw_url.replace("sqlite:///", "")
            elif self.raw_url.startswith("sqlite://"):
                db_path = self.raw_url.replace("sqlite://", "")
            
            self.conn = sqlite3.connect(db_path)
            self.conn.row_factory = sqlite3.Row

    def close(self):
        if self.conn:
            try:
                self.conn.close()
            except Exception:
                pass
            self.conn = None

    def execute(self, sql: str, params: Optional[Dict[str, Any]] = None):
        if self.is_postgres:
            with self.conn.cursor() as cur:
                if params:
                    # psycopg supports %(name)s syntax
                    cur.execute(sql, params)
                else:
                    cur.execute(sql)
        else:
            # SQLite supports :name syntax directly
            cur = self.conn.cursor()
            if params:
                cur.execute(sql, params)
            else:
                cur.execute(sql)
            self.conn.commit()

    def fetchall(self, sql: str, params: Optional[Dict[str, Any]] = None) -> List[Dict[str, Any]]:
        if self.is_postgres:
            with self.conn.cursor() as cur:
                if params:
                    cur.execute(sql, params)
                else:
                    cur.execute(sql)
                rows = cur.fetchall()
                return [dict(r) for r in rows]
        else:
            cur = self.conn.cursor()
            if params:
                cur.execute(sql, params)
            else:
                cur.execute(sql)
            rows = cur.fetchall()
            return [dict(r) for r in rows]

    def fetchone(self, sql: str, params: Optional[Dict[str, Any]] = None) -> Optional[Dict[str, Any]]:
        rows = self.fetchall(sql, params)
        return rows[0] if rows else None


# ═══════════════════════════════════════════════════════════════════════════════
# Schema Management & Data Seeding
# ═══════════════════════════════════════════════════════════════════════════════

def setup_validation_schema(db: DatabaseAdapter):
    """
    Creates isolated test schema 'fintext_pit_validation_test' and all required tables.
    Strictly mirrors config/timescale/init.sql and config/pit_reference_schema.sql.
    """
    if db.is_postgres:
        # Schema recreation in PostgreSQL
        db.execute(f"DROP SCHEMA IF EXISTS {SCHEMA_NAME} CASCADE;")
        db.execute(f"CREATE SCHEMA {SCHEMA_NAME};")
        db.execute(f"SET search_path TO {SCHEMA_NAME};")

        # 1. sentiment_records
        db.execute(
            """
            CREATE TABLE IF NOT EXISTS sentiment_records (
                id BIGINT,
                ticker TEXT NOT NULL,
                published_utc TIMESTAMPTZ NOT NULL,
                ingested_utc TIMESTAMPTZ NOT NULL,
                db_commit_utc TIMESTAMPTZ NOT NULL,
                source TEXT,
                title TEXT,
                sentiment_score DOUBLE PRECISION,
                sentiment_label TEXT,
                confidence DOUBLE PRECISION,
                data_quality_score DOUBLE PRECISION,
                vpin DOUBLE PRECISION,
                gamma_exposure DOUBLE PRECISION,
                valid_from TIMESTAMPTZ NOT NULL,
                valid_to TIMESTAMPTZ,
                revision_number INTEGER DEFAULT 1,
                is_current BOOLEAN DEFAULT TRUE,
                PRIMARY KEY (id, published_utc)
            );
            """
        )

        # 2. pit_ticker_history
        db.execute(
            """
            CREATE TABLE IF NOT EXISTS pit_ticker_history (
                id BIGINT PRIMARY KEY,
                entity_id TEXT NOT NULL,
                entity_name TEXT,
                cik TEXT,
                figi TEXT,
                isin TEXT,
                ticker TEXT NOT NULL,
                start_date DATE NOT NULL,
                end_date DATE NOT NULL,
                valid_from TIMESTAMPTZ NOT NULL,
                valid_to TIMESTAMPTZ,
                is_current BOOLEAN NOT NULL DEFAULT TRUE,
                source TEXT DEFAULT 'SEC',
                created_at TIMESTAMPTZ NOT NULL
            );
            """
        )

        # 3. pit_delisted_securities
        db.execute(
            """
            CREATE TABLE IF NOT EXISTS pit_delisted_securities (
                id BIGINT PRIMARY KEY,
                ticker TEXT NOT NULL,
                name TEXT,
                delisting_date DATE NOT NULL,
                reason TEXT,
                final_price_usd DOUBLE PRECISION,
                last_close_usd DOUBLE PRECISION,
                delisting_return DOUBLE PRECISION,
                sec_form25_date TIMESTAMPTZ,
                announcement_date TIMESTAMPTZ,
                valid_from TIMESTAMPTZ NOT NULL,
                valid_to TIMESTAMPTZ,
                is_current BOOLEAN NOT NULL DEFAULT TRUE,
                source TEXT DEFAULT 'SEC',
                created_at TIMESTAMPTZ NOT NULL
            );
            """
        )

        # 4. pit_corporate_actions
        db.execute(
            """
            CREATE TABLE IF NOT EXISTS pit_corporate_actions (
                id BIGINT PRIMARY KEY,
                ticker TEXT NOT NULL,
                action_date DATE NOT NULL,
                action_type TEXT NOT NULL,
                split_ratio DOUBLE PRECISION,
                dividend_per_share DOUBLE PRECISION,
                description TEXT,
                valid_from TIMESTAMPTZ NOT NULL,
                valid_to TIMESTAMPTZ,
                is_current BOOLEAN NOT NULL DEFAULT TRUE,
                source TEXT DEFAULT 'MANUAL',
                created_at TIMESTAMPTZ NOT NULL
            );
            """
        )

        # 5. pit_index_membership
        db.execute(
            """
            CREATE TABLE IF NOT EXISTS pit_index_membership (
                id BIGINT PRIMARY KEY,
                ticker TEXT NOT NULL,
                index_name TEXT NOT NULL,
                company_name TEXT,
                join_date DATE NOT NULL,
                leave_date DATE,
                reason TEXT,
                valid_from TIMESTAMPTZ NOT NULL,
                valid_to TIMESTAMPTZ,
                is_current BOOLEAN NOT NULL DEFAULT TRUE,
                source TEXT DEFAULT 'MANUAL',
                created_at TIMESTAMPTZ NOT NULL
            );
            """
        )
    else:
        # SQLite dialect
        db.execute("DROP TABLE IF EXISTS sentiment_records;")
        db.execute(
            """
            CREATE TABLE sentiment_records (
                id INTEGER,
                ticker TEXT NOT NULL,
                published_utc TEXT NOT NULL,
                ingested_utc TEXT NOT NULL,
                db_commit_utc TEXT NOT NULL,
                source TEXT,
                title TEXT,
                sentiment_score REAL,
                sentiment_label TEXT,
                confidence REAL,
                data_quality_score REAL,
                vpin REAL,
                gamma_exposure REAL,
                valid_from TEXT NOT NULL,
                valid_to TEXT,
                revision_number INTEGER DEFAULT 1,
                is_current INTEGER DEFAULT 1,
                PRIMARY KEY (id, published_utc)
            );
            """
        )

        db.execute("DROP TABLE IF EXISTS pit_ticker_history;")
        db.execute(
            """
            CREATE TABLE pit_ticker_history (
                id INTEGER PRIMARY KEY,
                entity_id TEXT NOT NULL,
                entity_name TEXT,
                cik TEXT,
                figi TEXT,
                isin TEXT,
                ticker TEXT NOT NULL,
                start_date TEXT NOT NULL,
                end_date TEXT NOT NULL,
                valid_from TEXT NOT NULL,
                valid_to TEXT,
                is_current INTEGER NOT NULL DEFAULT 1,
                source TEXT DEFAULT 'SEC',
                created_at TEXT NOT NULL
            );
            """
        )

        db.execute("DROP TABLE IF EXISTS pit_delisted_securities;")
        db.execute(
            """
            CREATE TABLE pit_delisted_securities (
                id INTEGER PRIMARY KEY,
                ticker TEXT NOT NULL,
                name TEXT,
                delisting_date TEXT NOT NULL,
                reason TEXT,
                final_price_usd REAL,
                last_close_usd REAL,
                delisting_return REAL,
                sec_form25_date TEXT,
                announcement_date TEXT,
                valid_from TEXT NOT NULL,
                valid_to TEXT,
                is_current INTEGER NOT NULL DEFAULT 1,
                source TEXT DEFAULT 'SEC',
                created_at TEXT NOT NULL
            );
            """
        )

        db.execute("DROP TABLE IF EXISTS pit_corporate_actions;")
        db.execute(
            """
            CREATE TABLE pit_corporate_actions (
                id INTEGER PRIMARY KEY,
                ticker TEXT NOT NULL,
                action_date TEXT NOT NULL,
                action_type TEXT NOT NULL,
                split_ratio REAL,
                dividend_per_share REAL,
                description TEXT,
                valid_from TEXT NOT NULL,
                valid_to TEXT,
                is_current INTEGER NOT NULL DEFAULT 1,
                source TEXT DEFAULT 'MANUAL',
                created_at TEXT NOT NULL
            );
            """
        )

        db.execute("DROP TABLE IF EXISTS pit_index_membership;")
        db.execute(
            """
            CREATE TABLE pit_index_membership (
                id INTEGER PRIMARY KEY,
                ticker TEXT NOT NULL,
                index_name TEXT NOT NULL,
                company_name TEXT,
                join_date TEXT NOT NULL,
                leave_date TEXT,
                reason TEXT,
                valid_from TEXT NOT NULL,
                valid_to TEXT,
                is_current INTEGER NOT NULL DEFAULT 1,
                source TEXT DEFAULT 'MANUAL',
                created_at TEXT NOT NULL
            );
            """
        )

    # ── Seed Fixtures ───────────────────────────────────────────────────────────
    prefix = f"{SCHEMA_NAME}." if db.is_postgres else ""
    param_style = "%(" if db.is_postgres else ":"
    
    # 1. sentiment_records
    for rec in SENTIMENT_RECORDS_FIXTURES:
        cols = list(rec.keys())
        placeholders = [f"{param_style}{c})s" if db.is_postgres else f":{c}" for c in cols]
        sql = f"INSERT INTO {prefix}sentiment_records ({', '.join(cols)}) VALUES ({', '.join(placeholders)});"
        db.execute(sql, rec)

    # 2. pit_ticker_history
    for rec in TICKER_HISTORY_FIXTURES:
        cols = list(rec.keys())
        placeholders = [f"{param_style}{c})s" if db.is_postgres else f":{c}" for c in cols]
        sql = f"INSERT INTO {prefix}pit_ticker_history ({', '.join(cols)}) VALUES ({', '.join(placeholders)});"
        db.execute(sql, rec)

    # 3. pit_delisted_securities
    for rec in DELISTED_SECURITIES_FIXTURES:
        cols = list(rec.keys())
        placeholders = [f"{param_style}{c})s" if db.is_postgres else f":{c}" for c in cols]
        sql = f"INSERT INTO {prefix}pit_delisted_securities ({', '.join(cols)}) VALUES ({', '.join(placeholders)});"
        db.execute(sql, rec)

    # 4. pit_corporate_actions
    for rec in CORPORATE_ACTIONS_FIXTURES:
        cols = list(rec.keys())
        placeholders = [f"{param_style}{c})s" if db.is_postgres else f":{c}" for c in cols]
        sql = f"INSERT INTO {prefix}pit_corporate_actions ({', '.join(cols)}) VALUES ({', '.join(placeholders)});"
        db.execute(sql, rec)

    # 5. pit_index_membership
    for rec in INDEX_MEMBERSHIP_FIXTURES:
        cols = list(rec.keys())
        placeholders = [f"{param_style}{c})s" if db.is_postgres else f":{c}" for c in cols]
        sql = f"INSERT INTO {prefix}pit_index_membership ({', '.join(cols)}) VALUES ({', '.join(placeholders)});"
        db.execute(sql, rec)


def cleanup_validation_schema(db: DatabaseAdapter):
    """Tear down the test schema in PostgreSQL."""
    if db.is_postgres:
        try:
            db.execute(f"DROP SCHEMA IF EXISTS {SCHEMA_NAME} CASCADE;")
        except Exception:
            pass


# ═══════════════════════════════════════════════════════════════════════════════
# Scenario Implementations (S1–S8, N1)
# ═══════════════════════════════════════════════════════════════════════════════

def _format_sql(db: DatabaseAdapter, sql_template: str) -> str:
    """Replaces :param with %(param)s when using PostgreSQL."""
    if db.is_postgres:
        import re
        return re.sub(r':([a-zA-Z0-9_]+)', r'%(\1)s', sql_template)
    return sql_template


def run_scenario_s1(db: DatabaseAdapter) -> Dict[str, Any]:
    """S1. Basic as-of before revision: query at T1 < rev2.valid_from -> returns rev1 only."""
    as_of = "2025-06-15T12:00:00Z"
    sql = _format_sql(
        db,
        """
        SELECT id, ticker, revision_number, sentiment_score, sentiment_label, valid_from, valid_to, is_current
        FROM sentiment_records
        WHERE ticker = :ticker
          AND valid_from <= :as_of
          AND (valid_to > :as_of OR valid_to IS NULL)
        ORDER BY revision_number ASC;
        """
    )
    rows = db.fetchall(sql, {"ticker": "AAPL", "as_of": as_of})
    
    passed = (
        len(rows) == 1
        and rows[0]["revision_number"] == 1
        and abs(float(rows[0]["sentiment_score"]) - 0.85) < 1e-4
    )
    
    return {
        "id": "S1",
        "name": "Basic as-of before revision (T1 < rev2.valid_from)",
        "as_of_utc": as_of,
        "expected_row_count": 1,
        "actual_row_count": len(rows),
        "passed": passed,
        "status": "PASS" if passed else "FAIL",
        "evidence": {
            "query_timestamp": as_of,
            "returned_revisions": [r["revision_number"] for r in rows],
            "returned_scores": [float(r["sentiment_score"]) for r in rows],
            "target_record": rows[0] if rows else None,
            "invariant": "At T1=12:00, only rev1 (valid [10:00, 14:00)) is returned. rev2 (valid >= 14:00) is excluded.",
        },
    }


def run_scenario_s2(db: DatabaseAdapter) -> Dict[str, Any]:
    """S2. As-of after revision: query at T2 >= rev2.valid_from -> returns rev2 as current."""
    as_of = "2025-06-15T15:00:00Z"
    sql = _format_sql(
        db,
        """
        SELECT id, ticker, revision_number, sentiment_score, sentiment_label, valid_from, valid_to, is_current
        FROM sentiment_records
        WHERE ticker = :ticker
          AND valid_from <= :as_of
          AND (valid_to > :as_of OR valid_to IS NULL)
        ORDER BY revision_number ASC;
        """
    )
    rows = db.fetchall(sql, {"ticker": "AAPL", "as_of": as_of})
    
    passed = (
        len(rows) == 1
        and rows[0]["revision_number"] == 2
        and abs(float(rows[0]["sentiment_score"]) - 0.45) < 1e-4
        and bool(rows[0]["is_current"]) is True
    )
    
    return {
        "id": "S2",
        "name": "As-of after revision (T2 >= rev2.valid_from)",
        "as_of_utc": as_of,
        "expected_row_count": 1,
        "actual_row_count": len(rows),
        "passed": passed,
        "status": "PASS" if passed else "FAIL",
        "evidence": {
            "query_timestamp": as_of,
            "returned_revisions": [r["revision_number"] for r in rows],
            "returned_scores": [float(r["sentiment_score"]) for r in rows],
            "target_record": rows[0] if rows else None,
            "invariant": "At T2=15:00, rev2 is active (is_current=True, valid_to=NULL). rev1 expired at 14:00.",
        },
    }


def run_scenario_s3(db: DatabaseAdapter) -> Dict[str, Any]:
    """S3. Late-arriving event: published Tp=09:00, ingested Ti=11:00. As-of Ta=10:00 -> MUST NOT appear."""
    as_of = "2025-06-16T10:00:00Z"
    sql = _format_sql(
        db,
        """
        SELECT id, ticker, published_utc, ingested_utc, valid_from, valid_to
        FROM sentiment_records
        WHERE ticker = :ticker
          AND valid_from <= :as_of
          AND (valid_to > :as_of OR valid_to IS NULL);
        """
    )
    rows = db.fetchall(sql, {"ticker": "MSFT", "as_of": as_of})
    
    passed = (len(rows) == 0)
    
    return {
        "id": "S3",
        "name": "Late-arriving event zero look-ahead isolation (Tp < Ta < Ti)",
        "as_of_utc": as_of,
        "expected_row_count": 0,
        "actual_row_count": len(rows),
        "passed": passed,
        "status": "PASS" if passed else "FAIL",
        "evidence": {
            "query_timestamp": as_of,
            "event_published_utc": "2025-06-16T09:00:00Z",
            "event_ingested_utc": "2025-06-16T11:00:00Z",
            "matched_rows": rows,
            "invariant": "News published at 09:00 but ingested at 11:00 was unobservable at 10:00. 0 rows returned proves zero look-ahead.",
        },
    }


def run_scenario_s4(db: DatabaseAdapter) -> Dict[str, Any]:
    """S4. Restated earnings revision: valid_from rev2 = T2. As-of T1 < T2 -> rev1. As-of T3 > T2 -> rev2."""
    t1 = "2025-05-05T12:00:00Z"
    t3 = "2025-05-15T12:00:00Z"
    sql = _format_sql(
        db,
        """
        SELECT id, ticker, revision_number, sentiment_score, title, valid_from, valid_to, is_current
        FROM sentiment_records
        WHERE ticker = :ticker
          AND valid_from <= :as_of
          AND (valid_to > :as_of OR valid_to IS NULL);
        """
    )
    rows_t1 = db.fetchall(sql, {"ticker": "NVDA", "as_of": t1})
    rows_t3 = db.fetchall(sql, {"ticker": "NVDA", "as_of": t3})
    
    passed_t1 = len(rows_t1) == 1 and rows_t1[0]["revision_number"] == 1 and abs(float(rows_t1[0]["sentiment_score"]) - 0.90) < 1e-4
    passed_t3 = len(rows_t3) == 1 and rows_t3[0]["revision_number"] == 2 and abs(float(rows_t3[0]["sentiment_score"]) - 0.30) < 1e-4
    passed = passed_t1 and passed_t3
    
    return {
        "id": "S4",
        "name": "Restated earnings revision historical transition",
        "as_of_utc": [t1, t3],
        "expected_row_count": 2,
        "actual_row_count": len(rows_t1) + len(rows_t3),
        "passed": passed,
        "status": "PASS" if passed else "FAIL",
        "evidence": {
            "t1_query": {"as_of": t1, "rev": rows_t1[0]["revision_number"] if rows_t1 else None, "score": float(rows_t1[0]["sentiment_score"]) if rows_t1 else None},
            "t3_query": {"as_of": t3, "rev": rows_t3[0]["revision_number"] if rows_t3 else None, "score": float(rows_t3[0]["sentiment_score"]) if rows_t3 else None},
            "invariant": "Before restatement (T1=05-05), initial filing (score=0.90) is returned. After restatement (T3=05-15), amended filing (score=0.30) is returned.",
        },
    }


def run_scenario_s5(db: DatabaseAdapter) -> Dict[str, Any]:
    """S5. Ticker change: entity with ticker AAA until T1, BBB after T2. As-of T0 -> AAA. As-of T3 -> BBB. Lineage returns both."""
    t0 = "2020-01-15T00:00:00Z"
    t3 = "2023-01-15T00:00:00Z"
    sql_as_of = _format_sql(
        db,
        """
        SELECT ticker, entity_id, start_date, end_date, valid_from, valid_to
        FROM pit_ticker_history
        WHERE entity_id = :entity_id
          AND valid_from <= :as_of
          AND (valid_to > :as_of OR valid_to IS NULL);
        """
    )
    sql_lineage = _format_sql(
        db,
        """
        SELECT ticker, start_date, end_date
        FROM pit_ticker_history
        WHERE entity_id = :entity_id
        ORDER BY start_date ASC;
        """
    )
    
    rows_t0 = db.fetchall(sql_as_of, {"entity_id": "ENT_META", "as_of": t0})
    rows_t3 = db.fetchall(sql_as_of, {"entity_id": "ENT_META", "as_of": t3})
    rows_lineage = db.fetchall(sql_lineage, {"entity_id": "ENT_META"})
    
    passed_t0 = len(rows_t0) == 1 and rows_t0[0]["ticker"] == "FB"
    passed_t3 = len(rows_t3) == 1 and rows_t3[0]["ticker"] == "META"
    passed_lineage = len(rows_lineage) == 2 and [r["ticker"] for r in rows_lineage] == ["FB", "META"]
    passed = passed_t0 and passed_t3 and passed_lineage
    
    return {
        "id": "S5",
        "name": "Ticker change and bi-temporal symbol lineage replay",
        "as_of_utc": [t0, t3],
        "expected_row_count": 4,  # 1 + 1 + 2
        "actual_row_count": len(rows_t0) + len(rows_t3) + len(rows_lineage),
        "passed": passed,
        "status": "PASS" if passed else "FAIL",
        "evidence": {
            "t0_ticker": rows_t0[0]["ticker"] if rows_t0 else None,
            "t3_ticker": rows_t3[0]["ticker"] if rows_t3 else None,
            "complete_lineage": [r["ticker"] for r in rows_lineage],
            "invariant": "In 2020, entity was 'FB'. In 2023, entity was 'META'. Full lineage correctly links both.",
        },
    }


def run_scenario_s6(db: DatabaseAdapter) -> Dict[str, Any]:
    """S6. Delisting: as-of T0 < delisting_date -> no delisting record. As-of T1 > delisting_date -> delisting return retrievable."""
    t0 = "2023-04-15T00:00:00Z"
    t1 = "2023-05-02T00:00:00Z"
    sql = _format_sql(
        db,
        """
        SELECT ticker, name, delisting_date, delisting_return, reason, valid_from
        FROM pit_delisted_securities
        WHERE ticker = :ticker
          AND valid_from <= :as_of
          AND (valid_to > :as_of OR valid_to IS NULL);
        """
    )
    rows_t0 = db.fetchall(sql, {"ticker": "FRC", "as_of": t0})
    rows_t1 = db.fetchall(sql, {"ticker": "FRC", "as_of": t1})
    
    passed_t0 = len(rows_t0) == 0
    passed_t1 = (
        len(rows_t1) == 1
        and abs(float(rows_t1[0]["delisting_return"]) - (-0.954)) < 1e-3
    )
    passed = passed_t0 and passed_t1
    
    return {
        "id": "S6",
        "name": "Delisting resolution and terminal return retrieval",
        "as_of_utc": [t0, t1],
        "expected_row_count": 1,
        "actual_row_count": len(rows_t0) + len(rows_t1),
        "passed": passed,
        "status": "PASS" if passed else "FAIL",
        "evidence": {
            "t0_delisted_records": len(rows_t0),
            "t1_delisted_records": len(rows_t1),
            "t1_delisting_return": float(rows_t1[0]["delisting_return"]) if rows_t1 else None,
            "t1_reason": rows_t1[0]["reason"] if rows_t1 else None,
            "invariant": "Prior to 2023-05-01, FRC was not delisted. After 2023-05-01, delisting return (-95.4%) is captured.",
        },
    }


def run_scenario_s7(db: DatabaseAdapter) -> Dict[str, Any]:
    """S7. Corporate action split: price before split. As-of query after split returns adjusted factor."""
    t0 = "2022-08-20T00:00:00Z"
    t1 = "2022-08-26T00:00:00Z"
    raw_price = 900.0
    
    sql = _format_sql(
        db,
        """
        SELECT ticker, action_date, split_ratio, valid_from
        FROM pit_corporate_actions
        WHERE ticker = :ticker
          AND action_type = 'SPLIT'
          AND valid_from <= :as_of
          AND (valid_to > :as_of OR valid_to IS NULL);
        """
    )
    rows_t0 = db.fetchall(sql, {"ticker": "TSLA", "as_of": t0})
    rows_t1 = db.fetchall(sql, {"ticker": "TSLA", "as_of": t1})
    
    ratio_t0 = float(rows_t0[0]["split_ratio"]) if rows_t0 else 1.0
    ratio_t1 = float(rows_t1[0]["split_ratio"]) if rows_t1 else 1.0
    
    adj_price_t0 = raw_price / ratio_t0
    adj_price_t1 = raw_price / ratio_t1
    
    passed = (
        len(rows_t0) == 0
        and len(rows_t1) == 1
        and abs(adj_price_t0 - 900.0) < 1e-4
        and abs(adj_price_t1 - 300.0) < 1e-4
    )
    
    return {
        "id": "S7",
        "name": "Corporate action split adjustment factor",
        "as_of_utc": [t0, t1],
        "expected_row_count": 1,
        "actual_row_count": len(rows_t0) + len(rows_t1),
        "passed": passed,
        "status": "PASS" if passed else "FAIL",
        "evidence": {
            "unadjusted_price": raw_price,
            "t0_split_factor": ratio_t0,
            "t0_adjusted_price": adj_price_t0,
            "t1_split_factor": ratio_t1,
            "t1_adjusted_price": adj_price_t1,
            "invariant": "Before 3:1 split (T0=08-20), adj price is $900. After split (T1=08-26), adj price is $300.",
        },
    }


def run_scenario_s8(db: DatabaseAdapter) -> Dict[str, Any]:
    """S8. Index membership: as-of T0 returns constituent set C0; as-of T1 > change returns set C1. No survivorship leak."""
    t0 = "2020-11-01T00:00:00Z"
    t1 = "2021-01-15T00:00:00Z"
    
    sql = _format_sql(
        db,
        """
        SELECT ticker
        FROM pit_index_membership
        WHERE index_name = :index_name
          AND valid_from <= :as_of
          AND (valid_to > :as_of OR valid_to IS NULL)
        ORDER BY ticker ASC;
        """
    )
    rows_t0 = db.fetchall(sql, {"index_name": "SP500", "as_of": t0})
    rows_t1 = db.fetchall(sql, {"index_name": "SP500", "as_of": t1})
    
    constituents_t0 = sorted([r["ticker"] for r in rows_t0])
    constituents_t1 = sorted([r["ticker"] for r in rows_t1])
    
    passed_t0 = constituents_t0 == ["AAPL", "OXY"]
    passed_t1 = constituents_t1 == ["AAPL", "TSLA"]
    passed = passed_t0 and passed_t1
    
    return {
        "id": "S8",
        "name": "Index membership point-in-time constituent set (survivorship bias prevention)",
        "as_of_utc": [t0, t1],
        "expected_row_count": 4,
        "actual_row_count": len(rows_t0) + len(rows_t1),
        "passed": passed,
        "status": "PASS" if passed else "FAIL",
        "evidence": {
            "t0_constituents": constituents_t0,
            "t1_constituents": constituents_t1,
            "survivorship_check": "TSLA not in S&P500 at T0=2020-11-01; OXY present at T0 and departed by T1.",
        },
    }


def run_scenario_n1(db: DatabaseAdapter) -> Dict[str, Any]:
    """
    N1. NEGATIVE CONTROL: run an intentionally incorrect as-of query that ignores valid_from/valid_to.
    The harness MUST detect that it returns a wrong row and mark this as EXPECTED_FAILURE.
    If the negative control does NOT fail, the harness exits non-zero (test sensitivity proof).
    """
    t1 = "2025-06-15T12:00:00Z"
    
    # Intentionally incorrect query that ignores valid_from/valid_to and only checks is_current = TRUE
    sql_naive = _format_sql(
        db,
        """
        SELECT id, ticker, revision_number, sentiment_score, valid_from, valid_to, is_current
        FROM sentiment_records
        WHERE ticker = :ticker
          AND is_current = TRUE;
        """
    )
    rows = db.fetchall(sql_naive, {"ticker": "AAPL"})
    
    # At T1=12:00, the correct historical revision is 1.
    # The naive query returns revision 2 (which is current *today*, but was not valid until 14:00).
    # Sensitivity proof passes if the naive query returned revision 2 (i.e. Look-ahead bias detected).
    detected_lookahead_bias = False
    defect_description = ""
    
    if rows:
        returned_rev = rows[0]["revision_number"]
        if returned_rev != 1:
            detected_lookahead_bias = True
            defect_description = (
                f"Look-ahead defect caught: Naive query at T1=12:00:00Z returned revision {returned_rev} "
                f"(valid_from={rows[0]['valid_from']}) instead of historical revision 1."
            )
        else:
            defect_description = "Defect undetected: Naive query unexpectedly returned rev 1."
    else:
        defect_description = "Naive query returned 0 rows."

    # Per specification: The harness MUST detect that it returns a wrong row and mark this as EXPECTED_FAILURE.
    # If detected_lookahead_bias is True, passed=True (the negative control succeeded in catching look-ahead).
    passed = detected_lookahead_bias
    status = "EXPECTED_FAILURE" if passed else "FAIL"

    return {
        "id": "N1",
        "name": "Negative Control: Naive query look-ahead sensitivity detector",
        "as_of_utc": t1,
        "expected_row_count": 1,
        "actual_row_count": len(rows),
        "passed": passed,
        "status": status,
        "evidence": {
            "query_timestamp": t1,
            "naive_query": "SELECT * FROM sentiment_records WHERE ticker = 'AAPL' AND is_current = TRUE;",
            "expected_revision": 1,
            "actual_revision": rows[0]["revision_number"] if rows else None,
            "lookahead_bias_detected": detected_lookahead_bias,
            "defect_description": defect_description,
            "sensitivity_proof": "Proves that omitting bi-temporal intervals leaks future revisions.",
        },
    }


def execute_all_scenarios(db: DatabaseAdapter) -> List[Dict[str, Any]]:
    """Runs S1 through S8, followed by N1."""
    scenarios = [
        run_scenario_s1(db),
        run_scenario_s2(db),
        run_scenario_s3(db),
        run_scenario_s4(db),
        run_scenario_s5(db),
        run_scenario_s6(db),
        run_scenario_s7(db),
        run_scenario_s8(db),
        run_scenario_n1(db),
    ]
    return scenarios
