#!/usr/bin/env python3
"""
===============================================================================
FinText-Alpha-Vectorizer — Point-in-Time (PIT) Correctness Validation Harness
Suite: Zero Look-Ahead Bias & SCD Type 2 Temporal Invariant Certification
===============================================================================

Verifies:
  S1. Basic as-of before revision (T1 < rev2.valid_from -> rev1 only)
  S2. As-of after revision (T2 >= rev2.valid_from -> rev2 as current)
  S3. Late-arriving event isolation (Tp < Ta < Ti -> event must not appear)
  S4. Restated earnings revision historical transition (T1 -> rev1, T3 -> rev2)
  S5. Ticker change & symbol lineage replay (FB -> META)
  S6. Delisting resolution & delisting return retrieval (FRC FDIC receivership)
  S7. Corporate action split adjustment factor (TSLA 3:1 forward split)
  S8. Index membership point-in-time constituent set (S&P 500 survivorship test)
  N1. Negative control sensitivity proof (naive query detects look-ahead defect)

Exit Codes:
  0 = All 8 positive scenarios PASS AND negative control N1 fails as expected.
  1 = Any positive scenario FAILS OR negative control N1 does not fail as expected.
  2 = Environment error (database connection failure, missing drivers, etc.).
===============================================================================
"""

import argparse
from datetime import datetime, timezone
import os
from pathlib import Path
import sys

# Ensure UTF-8 output encoding
if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
if hasattr(sys.stderr, "reconfigure"):
    sys.stderr.reconfigure(encoding="utf-8", errors="replace")

PROJECT_ROOT = Path(__file__).resolve().parent.parent
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))

from scripts.pit_validation.report import (
    generate_json_report,
    generate_markdown_report,
    get_git_commit_sha,
)
from scripts.pit_validation.scenarios import (
    DatabaseAdapter,
    cleanup_validation_schema,
    execute_all_scenarios,
    setup_validation_schema,
)


def parse_args():
    parser = argparse.ArgumentParser(
        description="FinText Alpha Vectorizer Point-in-Time (PIT) Correctness Test Harness"
    )
    parser.add_argument(
        "--database-url",
        default=os.environ.get("DATABASE_URL"),
        help="Target database URL (PostgreSQL or SQLite). Default: $DATABASE_URL env var.",
    )
    parser.add_argument(
        "--output-dir",
        default="data/pit-validation",
        help="Directory to save timestamped JSON validation reports.",
    )
    parser.add_argument(
        "--report-path",
        default="docs/PIT_VALIDATION_REPORT.md",
        help="Path to generate human-readable Markdown audit report.",
    )
    return parser.parse_args()


def main():
    args = parse_args()
    run_started_utc = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    
    print("=" * 82)
    print(" FinText Alpha Vectorizer — Point-in-Time (PIT) Correctness Validation")
    print(" Zero Look-Ahead Bias & Temporal Interval Verification Harness")
    print("=" * 82)

    if not args.database_url:
        print(
            "\n[!] ENVIRONMENT ERROR (Code 2): --database-url was not specified and $DATABASE_URL is unset.\n"
            "    Production certification requires an explicit PostgreSQL URL (e.g. postgres://fintext:fintext@localhost:5432/fintext_metadata).\n"
            "    To run local logic validation only, explicitly pass: --database-url \"sqlite://:memory:\".",
            file=sys.stderr,
        )
        return 2

    db = DatabaseAdapter(args.database_url)
    print(f"[*] Target Database : {db.sanitized_host} ({db.engine_name})")
    print(f"[*] Output Dir      : {args.output_dir}")
    print(f"[*] Report Path     : {args.report_path}")

    # 1. Connect to Database (Environment Check: Exit Code 2 on failure)
    try:
        db.connect()
    except Exception as e:
        print(f"\n[!] ENVIRONMENT ERROR (Code 2): Unable to connect to database: {e}", file=sys.stderr)
        return 2

    scenarios = []
    try:
        # 2. Setup Test Schema & Seed Fixtures
        print("[*] Initializing isolated test schema and seeding fixtures...")
        setup_validation_schema(db)
        print("[+] Test schema ready.\n")

        # 3. Execute Scenarios S1–S8 and N1
        print("── Executing Point-in-Time Scenarios ──────────────────────────────────────────")
        scenarios = execute_all_scenarios(db)

        for s in scenarios:
            s_id = s["id"]
            name = s["name"]
            status = s["status"]
            if status == "PASS":
                badge = "[PASS]"
            elif status == "EXPECTED_FAILURE":
                badge = "[EXPECTED_FAILURE]"
            else:
                badge = "[FAIL]"
            
            print(f"  {badge:<18} │ {s_id}: {name}")

    except Exception as e:
        print(f"\n[!] TEST EXECUTION ERROR (Code 1): {e}", file=sys.stderr)
        import traceback
        traceback.print_exc()
        return 1
    finally:
        # 4. Clean up isolated test schema
        cleanup_validation_schema(db)
        db.close()

    run_finished_utc = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    git_commit_sha = get_git_commit_sha()

    # 5. Evaluate Results
    positive_scenarios = [s for s in scenarios if s["id"].startswith("S")]
    negative_scenarios = [s for s in scenarios if s["id"].startswith("N")]

    all_positive_passed = all(s["passed"] for s in positive_scenarios)
    negative_control_passed = all(s["passed"] for s in negative_scenarios)

    # 6. Generate JSON and Markdown Reports
    reproduction_cmd = (
        f"python scripts/validate_pit_correctness.py "
        f"--database-url \"{args.database_url}\" "
        f"--output-dir {args.output_dir} "
        f"--report-path {args.report_path}"
    )

    json_path, report_data = generate_json_report(
        output_dir=Path(args.output_dir),
        run_started_utc=run_started_utc,
        run_finished_utc=run_finished_utc,
        git_commit_sha=git_commit_sha,
        database_url_host=db.sanitized_host,
        database_engine=db.engine_name,
        is_production_stack=db.is_production_stack,
        scenarios=scenarios,
    )
    md_path = generate_markdown_report(
        report_path=Path(args.report_path),
        report_data=report_data,
        reproduction_cmd=reproduction_cmd,
    )

    print("\n── Summary Results ────────────────────────────────────────────────────────────")
    print(f"  Positive Scenarios Passed : {sum(1 for s in positive_scenarios if s['passed'])} / {len(positive_scenarios)}")
    print(f"  Negative Control Verified : {sum(1 for s in negative_scenarios if s['passed'])} / {len(negative_scenarios)}")
    print(f"  JSON Audit Report         : {json_path}")
    print(f"  Markdown Audit Report     : {md_path}")
    print("=" * 82)

    if all_positive_passed and negative_control_passed:
        print("[SUCCESS] All 8 positive scenarios passed and negative control N1 failed as expected.")
        print("[STATUS] Point-in-Time Correctness Certified. (Exit Code 0)")
        return 0
    else:
        print("[FAILURE] Validation criteria not met. (Exit Code 1)", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
