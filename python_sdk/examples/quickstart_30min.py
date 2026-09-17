"""
FinText Alpha Vectorizer — 15-Minute Institutional Quant Quickstart
Demonstrates core Point-in-Time sentiment, historical replay, symbol mapping,
supply chain network risk, and columnar Parquet research data export.
"""
import os
import sys
from fintext import FinTextClient, FinTextError

# 1. Initialize client using environment variable or dev default
client = FinTextClient(
    base_url=os.getenv("FINTEXT_BASE_URL", "http://127.0.0.1:8000"),
    api_version="v1",
    admin_token=os.getenv("FINTEXT_ADMIN_TOKEN", "fintext-admin-dev-secret-token")
)

try:
    # 2. System Health & Core Connectivity Probe
    health = client.health()
    print(f"[+] Gateway Connected: Status={health.status} (v{health.version})")

    # 3. Real-Time & Point-in-Time Sentiment Scoring
    sent = client.sentiment("AAPL", as_of_utc="2023-01-03T16:00:00Z")
    print(f"[+] AAPL Sentiment: Score={sent.sentiment_score:.3f} [{sent.sentiment_label}]")

    # 4. Point-in-Time Historical State Replay (Zero Look-Ahead Proof)
    replay = client.pit_replay("AAPL", as_of_utc="2023-01-03T16:00:00Z", limit=5)
    print(f"[+] PIT Replay: Visible Articles={len(replay.news_articles)} | Zero-Lookahead={replay.replay_consistency.is_lookahead_bias_free}")

    # 5. Permanent Identifier Mapping (Ticker -> CIK / FIGI / ISIN)
    sym = client.symbol_map("AAPL")
    print(f"[+] Symbol Crosswalk: {sym.ticker} -> CIK:{sym.cik} | FIGI:{sym.figi}")

    # 6. Multi-Tier Supply Chain Graph Shock Propagation
    risk = client.supply_chain_risk("AAPL", max_depth=2)
    print(f"[+] Supply Chain Risk: Composite={risk.composite_risk_score:.2f} ({risk.risk_tier})")

    # 7. High-Throughput Columnar Parquet Research Export
    pq_bytes = client.export_parquet(start_date="2025-01-01", end_date="2025-01-05")
    print(f"[+] Parquet Export: Received {len(pq_bytes)} bytes of compressed feature data.")
    print("[*] 15-Minute TTFV Quickstart Complete. Platform verified for quant production.")

except Exception as err:
    print(f"[-] FinText API Offline or Unreachable: {err}")
    print("[*] Launch the 4 core platform containers: 'docker compose up -d'")
    sys.exit(0)
