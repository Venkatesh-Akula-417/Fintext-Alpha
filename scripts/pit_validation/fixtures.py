"""
Deterministic Fixtures for FinText Point-in-Time (PIT) Validation Suite

All timestamps, identifiers, and financial parameters are 100% hard-coded and reproducible.
No dynamic timestamps (NOW(), CURRENT_TIMESTAMP) or random generators are used.
Column schemas strictly mirror config/timescale/init.sql and config/pit_reference_schema.sql.
"""

from typing import Any, Dict, List

# ═══════════════════════════════════════════════════════════════════════════════
# 1. sentiment_records (TimescaleDB / PostgreSQL Hypertable SCD2)
# ═══════════════════════════════════════════════════════════════════════════════
SENTIMENT_RECORDS_FIXTURES: List[Dict[str, Any]] = [
    # ── S1, S2, N1: AAPL Revision Lineage (Rev 1 superseded by Rev 2) ───────────
    {
        "id": 1001,
        "ticker": "AAPL",
        "published_utc": "2025-06-15T09:30:00Z",
        "ingested_utc": "2025-06-15T09:30:05Z",
        "db_commit_utc": "2025-06-15T10:00:00Z",
        "source": "SEC_8K",
        "title": "Apple Inc. Form 8-K Item 2.02 Results of Operations",
        "sentiment_score": 0.85,
        "sentiment_label": "POSITIVE",
        "confidence": 0.94,
        "data_quality_score": 0.98,
        "vpin": 0.22,
        "gamma_exposure": 450000.0,
        "valid_from": "2025-06-15T10:00:00Z",
        "valid_to": "2025-06-15T14:00:00Z",
        "revision_number": 1,
        "is_current": False,
    },
    {
        "id": 1002,
        "ticker": "AAPL",
        "published_utc": "2025-06-15T09:30:00Z",
        "ingested_utc": "2025-06-15T13:59:50Z",
        "db_commit_utc": "2025-06-15T14:00:00Z",
        "source": "SEC_8K",
        "title": "Apple Inc. Form 8-K Item 2.02 Revision - Normalized Margin",
        "sentiment_score": 0.45,
        "sentiment_label": "NEUTRAL",
        "confidence": 0.89,
        "data_quality_score": 0.99,
        "vpin": 0.28,
        "gamma_exposure": 510000.0,
        "valid_from": "2025-06-15T14:00:00Z",
        "valid_to": None,
        "revision_number": 2,
        "is_current": True,
    },
    # ── S3: Late-Arriving Event (MSFT: Published Tp=09:00, Ingested Ti=11:00) ──
    {
        "id": 1003,
        "ticker": "MSFT",
        "published_utc": "2025-06-16T09:00:00Z",
        "ingested_utc": "2025-06-16T11:00:00Z",
        "db_commit_utc": "2025-06-16T11:00:02Z",
        "source": "Reuters",
        "title": "Microsoft Announces Strategic AI Cloud Expansion",
        "sentiment_score": 0.78,
        "sentiment_label": "POSITIVE",
        "confidence": 0.92,
        "data_quality_score": 0.95,
        "vpin": 0.19,
        "gamma_exposure": 320000.0,
        "valid_from": "2025-06-16T11:00:00Z",
        "valid_to": None,
        "revision_number": 1,
        "is_current": True,
    },
    # ── S4: Restated Earnings Revision (NVDA: Rev 1 at 05-01, Rev 2 at 05-10) ───
    {
        "id": 1004,
        "ticker": "NVDA",
        "published_utc": "2025-05-01T08:00:00Z",
        "ingested_utc": "2025-05-01T08:00:05Z",
        "db_commit_utc": "2025-05-01T08:00:10Z",
        "source": "SEC_10Q",
        "title": "NVIDIA Corp Form 10-Q Initial Filing Q1 Revenue",
        "sentiment_score": 0.90,
        "sentiment_label": "POSITIVE",
        "confidence": 0.96,
        "data_quality_score": 0.97,
        "vpin": 0.15,
        "gamma_exposure": 780000.0,
        "valid_from": "2025-05-01T08:00:00Z",
        "valid_to": "2025-05-10T16:00:00Z",
        "revision_number": 1,
        "is_current": False,
    },
    {
        "id": 1005,
        "ticker": "NVDA",
        "published_utc": "2025-05-10T15:55:00Z",
        "ingested_utc": "2025-05-10T15:59:50Z",
        "db_commit_utc": "2025-05-10T16:00:00Z",
        "source": "SEC_10Q_A",
        "title": "NVIDIA Corp Form 10-Q/A Amendment & Inventory Restatement",
        "sentiment_score": 0.30,
        "sentiment_label": "NEUTRAL",
        "confidence": 0.91,
        "data_quality_score": 0.99,
        "vpin": 0.35,
        "gamma_exposure": 890000.0,
        "valid_from": "2025-05-10T16:00:00Z",
        "valid_to": None,
        "revision_number": 2,
        "is_current": True,
    },
]

# ═══════════════════════════════════════════════════════════════════════════════
# 2. pit_ticker_history (S5: Symbol Lineage ENT_META: FB -> META)
# ═══════════════════════════════════════════════════════════════════════════════
TICKER_HISTORY_FIXTURES: List[Dict[str, Any]] = [
    {
        "id": 2001,
        "entity_id": "ENT_META",
        "entity_name": "Meta Platforms, Inc.",
        "cik": "0001326801",
        "figi": "BBG000MM2P62",
        "isin": "US30303M1027",
        "ticker": "FB",
        "start_date": "2012-05-18",
        "end_date": "2022-06-08",
        "valid_from": "2012-05-18T00:00:00Z",
        "valid_to": "2022-06-09T00:00:00Z",
        "is_current": False,
        "source": "SEC",
        "created_at": "2012-05-18T00:00:00Z",
    },
    {
        "id": 2002,
        "entity_id": "ENT_META",
        "entity_name": "Meta Platforms, Inc.",
        "cik": "0001326801",
        "figi": "BBG000MM2P62",
        "isin": "US30303M1027",
        "ticker": "META",
        "start_date": "2022-06-09",
        "end_date": "9999-12-31",
        "valid_from": "2022-06-09T00:00:00Z",
        "valid_to": None,
        "is_current": True,
        "source": "SEC",
        "created_at": "2022-06-09T00:00:00Z",
    },
]

# ═══════════════════════════════════════════════════════════════════════════════
# 3. pit_delisted_securities (S6: FRC FDIC Delisting & Delisting Return)
# ═══════════════════════════════════════════════════════════════════════════════
DELISTED_SECURITIES_FIXTURES: List[Dict[str, Any]] = [
    {
        "id": 3001,
        "ticker": "FRC",
        "name": "First Republic Bank",
        "delisting_date": "2023-05-01",
        "reason": "FDIC Receivership & NYSE Delisting",
        "final_price_usd": 3.51,
        "last_close_usd": 3.51,
        "delisting_return": -0.954,
        "sec_form25_date": "2023-05-01T08:00:00Z",
        "announcement_date": "2023-05-01T07:00:00Z",
        "valid_from": "2023-05-01T07:00:00Z",
        "valid_to": None,
        "is_current": True,
        "source": "SEC",
        "created_at": "2023-05-01T07:00:00Z",
    }
]

# ═══════════════════════════════════════════════════════════════════════════════
# 4. pit_corporate_actions (S7: TSLA 3-for-1 Stock Split on 2022-08-25)
# ═══════════════════════════════════════════════════════════════════════════════
CORPORATE_ACTIONS_FIXTURES: List[Dict[str, Any]] = [
    {
        "id": 4001,
        "ticker": "TSLA",
        "action_date": "2022-08-25",
        "action_type": "SPLIT",
        "split_ratio": 3.0,
        "dividend_per_share": 0.0,
        "description": "Tesla 3-for-1 Forward Stock Split",
        "valid_from": "2022-08-25T00:00:00Z",
        "valid_to": None,
        "is_current": True,
        "source": "SEC",
        "created_at": "2022-08-25T00:00:00Z",
    }
]

# ═══════════════════════════════════════════════════════════════════════════════
# 5. pit_index_membership (S8: S&P 500 Constituent Rebalance TSLA adds / OXY drops)
# ═══════════════════════════════════════════════════════════════════════════════
INDEX_MEMBERSHIP_FIXTURES: List[Dict[str, Any]] = [
    {
        "id": 5001,
        "ticker": "AAPL",
        "index_name": "SP500",
        "company_name": "Apple Inc.",
        "join_date": "1982-11-30",
        "leave_date": None,
        "reason": "Initial Constituent Addition",
        "valid_from": "1982-11-30T00:00:00Z",
        "valid_to": None,
        "is_current": True,
        "source": "S&P",
        "created_at": "1982-11-30T00:00:00Z",
    },
    {
        "id": 5002,
        "ticker": "OXY",
        "index_name": "SP500",
        "company_name": "Occidental Petroleum Corp.",
        "join_date": "1982-12-31",
        "leave_date": "2020-12-21",
        "reason": "Replaced by TSLA in quarterly rebalance",
        "valid_from": "1982-12-31T00:00:00Z",
        "valid_to": "2020-12-21T00:00:00Z",
        "is_current": False,
        "source": "S&P",
        "created_at": "1982-12-31T00:00:00Z",
    },
    {
        "id": 5003,
        "ticker": "TSLA",
        "index_name": "SP500",
        "company_name": "Tesla, Inc.",
        "join_date": "2020-12-21",
        "leave_date": None,
        "reason": "Added to S&P 500 in quarterly rebalance",
        "valid_from": "2020-12-21T00:00:00Z",
        "valid_to": None,
        "is_current": True,
        "source": "S&P",
        "created_at": "2020-12-21T00:00:00Z",
    },
]
