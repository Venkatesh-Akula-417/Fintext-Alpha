"""
===============================================================================
FinText-Alpha-Vectorizer — Master Test Certification Runner
Native Rust Institutional Platform (PostgreSQL 16 + TimescaleDB Primary, QuestDB Hot Dual-Storage, Kafka Stream, 96 Master Suites)
===============================================================================
"""

import os
from pathlib import Path
import subprocess
import sys
import time

if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
if hasattr(sys.stderr, "reconfigure"):
    sys.stderr.reconfigure(encoding="utf-8", errors="replace")

PROJECT_ROOT = Path(__file__).resolve().parent.parent

TEST_SUITES = [
    ("Native Rust Workspace Unit Tests (10 Crates)", ["cargo", "test", "--workspace", "--jobs", "2", "--manifest-path", "rust/Cargo.toml"]),
    ("Suite #179: Rust Ingestion Engine, Whisper ASR & QuestDB ILP Sink", [sys.executable, str(PROJECT_ROOT / "scripts" / "test_rust_ingestion_engine.py")]),
    ("Suite #180: Native Rust Axum HTTP Gateway & QuestDB SQL", [sys.executable, str(PROJECT_ROOT / "scripts" / "test_rust_axum_api.py")]),
    ("Suite #181: Cross-Asset Spillover Engine & Lead-Lag Analytics", [sys.executable, str(PROJECT_ROOT / "scripts" / "test_spillover_engine.py")]),
    ("Suite #182: Security Symbol Mapping Service & Institutional Identifiers", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_symbol_map.py")]),
    ("Suite #183: Custom Universe Builder & Batch Sentiment Integration", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_universes.py")]),
    ("Suite #184: Real Stock Price (OHLCV) Ingestion & Actual Price Backtesting", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_price_backtest.py")]),
    ("Suite #185: Quantitative Data Quality Scoring & Filtering Suite", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_data_quality.py")]),
    ("Suite #186: Options Implied Volatility & Black-Scholes Greeks Engine", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_options_iv.py")]),
    ("Suite #187: Unusual Options Activity (UOA) Detection & Scoring Engine", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_unusual_options.py")]),
    ("Suite #188: User API Usage Statistics & Consumption Analytics Engine", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_usage_stats.py")]),
    ("Suite #189: Event Study & Cumulative Abnormal Returns (CAR) Analytics Engine", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_event_study.py")]),
    ("Suite #190: SEC Form 8-K Unscheduled Corporate Disclosure Event Classification Engine", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_8k_events.py")]),
    ("Suite #191: Supply Chain Risk Propagation & Graph Intelligence Engine", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_supply_chain_risk.py")]),
    ("Suite #192: News Sentiment Aggregated Feed & Market Stream Engine", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_sentiment_feed.py")]),
    ("Suite #193: Sentiment Anomaly Detection Engine & Z-Score Deviation Scanner", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_sentiment_anomalies.py")]),
    ("Suite #194: Real-time Audio Transcription & Acoustic Stress Analysis Engine", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_audio_transcribe.py")]),
    ("Suite #195: Earnings Call Transcript Database & Retrieval Engine", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_transcripts.py")]),
    ("Suite #196: Quantitative Macro Market Regime Detection & Sentiment Breadth Engine", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_market_regime.py")]),
    ("Suite #197: Return Correlation Matrix & Cross-Asset Portfolio Risk Engine", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_return_correlation.py")]),
    ("Suite #198: Options Put/Call Ratio & Microstructure Sentiment Engine", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_put_call_ratio.py")]),
    ("Suite #199: Earnings Surprise Tracker & Event-Driven Sentiment Shift Engine", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_earnings_surprise.py")]),
    ("Suite #200: SEC Form 4 Insider Trading Signal & Executive Conviction Engine", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_insider_trading.py")]),
    ("Suite #201: Multi-Source Sentiment Disagreement & Dispersion Index Engine", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_sentiment_disagreement.py")]),
    ("Suite #202: 2D Options Volatility Surface & Smile/Skew Grid Engine", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_options_vol_surface.py")]),
    ("Suite #203: Multi-Signal M&A Rumor Detection & Catalyst Engine", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_ma_rumors.py")]),
    ("Suite #204: SEC Regulatory Filing Classifier & Discovery Engine", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_regulatory_filings.py")]),
    ("Suite #205: Stripe Subscription & Billing Integration Engine", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_billing.py")]),
    ("Suite #206: Organizations & Multi-User Team Access (RBAC) Engine", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_orgs.py")]),
    ("Suite #207: IP Whitelisting & CIDR Access Control Engine", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_ip_whitelist.py")]),
    ("Suite #208: News Article Full Text Retrieval Engine", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_news_articles.py")]),
    ("Suite #209: Entity Sentiment Breakdown & Analytics Engine", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_sentiment_entities.py")]),
    ("Suite #210: Compliance Audit Log Export Engine", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_audit_logs.py")]),
    ("Suite #211: API Key Rotation Automation Engine", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_api_key_rotation.py")]),
    ("Suite #212: Unified Cross-Domain Search API Engine", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_search.py")]),
    ("Suite #213: Model Versioning & Data Provenance Lineage Engine", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_model_metadata.py")]),
    ("Suite #214: Sector Rotation Signals & Relative Strength Ranking Engine", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_sector_rotation.py")]),
    ("Suite #215: Email Digest Service & Background Worker", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_email_digest.py")]),
    ("Suite #216: Streaming Kafka Topic Access & Consumer Credentials Engine", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_kafka_stream.py")]),
    ("Suite #217: Data Retention Policy Tool & Automated Cleanup Engine", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_retention.py")]),
    ("Suite #218: Factor Exposure Report & Multi-Factor OLS Regression Engine", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_factor_exposure.py")]),
    ("Suite #219: ESG Sentiment Scores & Sustainability Analytics Engine", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_esg_scores.py")]),
    ("Suite #220: Bankruptcy Risk Signals & Multi-Factor Distress Engine", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_bankruptcy_risk.py")]),
    ("Suite #221: FX Sentiment Feed & Currency Pair Analytics Engine", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_fx_sentiment.py")]),
    ("Suite #222: Commodity News Sentiment & Raw Material Analytics Engine", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_commodity_sentiment.py")]),
    ("Suite #223: Custom Polling Webhooks & Pull-Based Data Delivery Engine", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_polling_webhooks.py")]),
    ("Suite #224: Crypto News Sentiment & Digital Asset Analytics Engine", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_crypto_sentiment.py")]),
    ("Suite #225: Options Market Microstructure (VPIN/GEX) Time Series Engine", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_microstructure.py")]),
    ("Suite #226: Market Breadth & Advance/Decline Time Series Engine", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_market_breadth.py")]),
    ("Suite #227: Telegram & Discord Alert Bot Subscription Engine", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_chat_alerts.py")]),
    ("Suite #228: Credit Default Sentiment & Fixed Income Analytics Engine", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_credit_sentiment.py")]),
    ("Suite #229: News Sentiment Backfill Engine", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_backfill.py")]),
    ("Suite #230: Portfolio Optimization (Mean-Variance & Risk Parity) Engine", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_portfolio_optimize.py")]),
    ("Suite #231: Complete GraphQL Elimination & REST/WebSocket Consolidation", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_graphql_elimination.py")]),
    ("Suite #232: Portfolio Factor Exposure & Risk Attribution Engine", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_portfolio_factor_exposure.py")]),
    ("Suite #233: Model Retraining Automation Engine", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_retraining.py")]),
    ("Suite #234: FIX Protocol Bridge & Simulated Broker Execution Engine", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_fix_orders.py")]),
    ("Suite #235: Dead Letter Queue (DLQ) Monitoring & Auto-Reprocessing Engine", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_dlq.py")]),
    ("Suite #236: Latency SLA Reporting & Compliance Analytics Engine", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_sla_status.py")]),
    ("Suite #237: API Sandbox Environment & Isolation Engine", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_sandbox.py")]),
    ("Suite #238: Data Lineage & Provenance Tracking Engine", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_provenance.py")]),
    ("Suite #239: Real-time Sentiment Anomaly WebSocket Push Feature", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_anomaly_websocket.py")]),
    ("Suite #240: Multilingual Sentiment Support & Language Detection Engine", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_multilingual.py")]),
    ("Suite #241: End-to-End User Journey Integration Workflow (Signup -> Billing -> API Key -> Sentiment -> Backtest -> CSV)", [sys.executable, str(PROJECT_ROOT / "scripts" / "test_integration_workflow.py")]),
    ("Suite #242: API Security & Penetration Audit (Auth Bypass, JWT, SQLi, XSS, Rate Limit, IP Whitelist, API Keys)", [sys.executable, str(PROJECT_ROOT / "scripts" / "security_test.py")]),
    ("Suite #243: Financial Data Quality & Validation Audit (Score Ranges, Timestamps, PIT, Completeness, Stats)", [sys.executable, str(PROJECT_ROOT / "scripts" / "data_quality_check.py")]),
    ("Suite #244: OpenAPI Specification & Documentation Audit (Route Reachability, Parameter Constraints, DTO Schemas, Auth)", [sys.executable, str(PROJECT_ROOT / "scripts" / "audit_openapi_documentation.py")]),
    ("Suite #245: Model Card & Model Lineage Governance API", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_model_card.py")]),
    ("Suite #246: Alpha Signal Validation Report & Strategy Performance Attribution Engine", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_alpha_report.py")]),
    ("Suite #247: Point-in-Time Data Replay & Look-Ahead Bias Validation Engine", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_pit_replay.py")]),
    ("Suite #248: Signal Quality Report & Alpha Validation Engine", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_signal_quality.py")]),
    ("Suite #249: Point-in-Time (PIT) Certification & Look-Ahead Bias Audit Engine", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_pit_certificate.py")]),
    ("Suite #250: Zero-Touch Operations & Alerting Automation", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_operations_alerting.py")]),
    ("Suite #251: Kafka Messaging Simplification & Infrastructure Streamlining", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_kafka_messaging_simplification.py")]),
    ("Suite #252: Complete ClickHouse Removal & Storage Consolidation to QuestDB", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_clickhouse_elimination.py")]),
    ("Suite #253: Single-Region Kubernetes Infrastructure & Manifest Consolidation", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_single_region_k8s.py")]),
    ("Suite #254: Toolchain & Docker Base Image Modernization", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_toolchain_and_docker.py")]),
    ("Suite #255: Production Mode Guard (No Synthetic Data in Production)", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_production_mode.py")]),
    ("Suite #256: Provider Health Status & Operational Transparency API", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_provider_health.py")]),
    ("Suite #257: FinBERT Model Validation & Calibration Suite", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_model_validation.py")]),
    ("Suite #258: FinBERT Domain Fine-Tuning, INT8 Quantization & Fallback Engine", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_finetuned_model.py")]),
    ("Suite #259: Slowly Changing Dimension Type 2 (SCD2) Point-in-Time Revision History", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_scd2_revision_history.py")]),
    ("Suite #260: Versioned Public API Gateway Layer & Surface Simplification", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_public_api_surface.py")]),
    ("Suite #261: Automated Corporate Actions & Ticker History Updater", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_corporate_actions_updater.py")]),
    ("Suite #262: Data Source Quality Scoring & Validation Gates (ACCEPT/QUARANTINE/REJECT)", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_source_quality.py")]),
    ("Suite #263: TimescaleDB Time-Series Migration & Storage Adapter (Phase 1)", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_timescale_migration.py")]),
    ("Suite #264: TimescaleDB Historical Backfill & Primary Switchover (Phase 2)", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_timescale_switchover.py")]),
    ("Suite #265: Raw Data Archive (Apache Parquet & S3/MinIO Storage)", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_raw_archive.py")]),
    ("Suite #266: Secrets Management Abstraction Layer (Local Environment vs AWS Secrets Manager)", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_secrets_manager.py")]),
    ("Suite #267: Stripe Subscription Billing Completion & Revenue Automation", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_stripe_completion.py")]),
    ("Suite #268: PIT Certificate Cryptographic Proof Archival & Independent Hash Verifiability", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_pit_certificate_archival.py")]),
    ("Suite #269: PostgreSQL Relational Point-in-Time (PIT) Database Storage & Fallback Integration", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_pit_database.py")]),
    ("Suite #270: Ingestion Bounded Concurrency & Backpressure Limiter Engine", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_backpressure.py")]),
    ("Suite #271: Database Circuit Breaker & Retry Policy", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_db_circuit_breaker.py")]),
    ("Suite #272: In-Memory Cache TTL & Capacity Governance", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_cache_governance.py")]),
    ("Suite #273: Raw Data Archive S3/MinIO Lifecycle Policies & Upload Verification", [sys.executable, str(PROJECT_ROOT / "scripts" / "verify_raw_archive_lifecycle.py")]),
]






def get_runner_env():
    env = os.environ.copy()
    env["CMAKE"] = r"C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin\cmake.exe"
    env["CMAKE_GENERATOR"] = "Visual Studio 17 2022"
    env["LIBCLANG_PATH"] = str(PROJECT_ROOT / "venv" / "Lib" / "site-packages" / "clang" / "native")
    env["WHISPER_MOCK_FALLBACK"] = "1"
    env["QUESTDB_MOCK_FALLBACK"] = "1"
    env["TIMESCALE_MOCK_FALLBACK"] = "1"
    env["TIMESCALE_MOCK_MODE"] = "1"
    env["POLYGON_MOCK_FALLBACK"] = "1"
    env["KAFKA_MOCK_FALLBACK"] = "1"
    env["KAFKA_MOCK_MODE"] = "1"
    env["ENABLE_FIX_BRIDGE"] = "1"
    env["PYTHONUNBUFFERED"] = "1"
    return env


def main():
    print("=" * 80, flush=True)
    print(" FinText-Alpha-Vectorizer -- Master Test Certification (PostgreSQL 16 + TimescaleDB & QuestDB)", flush=True)
    print("=" * 80, flush=True)
    print(f" Working Directory: {PROJECT_ROOT}\n", flush=True)

    passed_count = 0
    start_total = time.time()
    runner_env = get_runner_env()

    for idx, (title, cmd) in enumerate(TEST_SUITES, start=1):
        print(f"[{idx}/{len(TEST_SUITES)}] Running {title}...", flush=True)
        t0 = time.time()
        res = subprocess.run(
            cmd,
            cwd=str(PROJECT_ROOT),
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            env=runner_env,
            shell=True if cmd[0] == "cargo" else False,
        )
        elapsed = time.time() - t0

        if res.returncode == 0:
            print(f"    STATUS: PASSED [OK] ({elapsed:.2f}s)\n", flush=True)
            passed_count += 1
        else:
            print(f"    STATUS: FAILED [X] ({elapsed:.2f}s)", flush=True)
            print(f"    STDERR:\n{res.stderr}", flush=True)
            print(f"    STDOUT:\n{res.stdout}\n", flush=True)

        time.sleep(0.5)

    total_time = time.time() - start_total
    print("=" * 80, flush=True)
    print(f" Total Suites: {len(TEST_SUITES)} | Passed: {passed_count} | Failed: {len(TEST_SUITES) - passed_count}", flush=True)
    print(f" Total Execution Time: {total_time:.2f}s", flush=True)
    print("=" * 80, flush=True)

    if passed_count == len(TEST_SUITES):
        print(" ALL TEST SUITES PASSED CLEANLY! [OK]\n", flush=True)
        return 0
    else:
        print(" SOME TEST SUITES FAILED [X]\n", flush=True)
        return 1


if __name__ == "__main__":
    sys.exit(main())
