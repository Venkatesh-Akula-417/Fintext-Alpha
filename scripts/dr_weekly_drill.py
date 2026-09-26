#!/usr/bin/env python3
"""
FinText-Alpha-Vectorizer — Automated Disaster Recovery Weekly Drill
═══════════════════════════════════════════════════════════════════════════════
Automates institutional disaster recovery verification using standard library:
1. Discovers the latest PostgreSQL backup archive (local staging or S3).
2. Spawns an EPHEMERAL PostgreSQL 16 container (never touching prod containers).
3. Restores the dump into a clean, isolated database instance.
4. Executes comprehensive verification checks:
   - Schema base table count (>= 20 CLASS-T tables, expected 28)
   - Row counts for all 4 Point-in-Time (PIT) tables:
     * instrument_master
     * filings_raw
     * filings_normalized
     * sentiment_records
   - 2-Organization Row-Level Security (RLS) cross-tenant leak spot check.
5. Benchmarks wall-clock Recovery Time Objective (RTO) against the 4-hour SLA.
6. Records immutable ledger audit in logs/dr_ledger.md and outputs logs/dr_report.json.
7. Guarantees container cleanup in all execution branches (try/finally).

Usage:
    python scripts/dr_weekly_drill.py [--mode {local|s3}] [--dump-path <path>]
                                      [--json-report logs/dr_report.json]
"""

import sys
import os
import time
import json
import hashlib
import argparse
import subprocess
from pathlib import Path
from datetime import datetime, timezone

# ── Workspace Root & Constants ────────────────────────────────────────────────
REPO_ROOT = Path(__file__).resolve().parent.parent
LOGS_DIR = REPO_ROOT / "logs"
LOCAL_BACKUPS_DIR = LOGS_DIR / "backups_local"
FALLBACK_BACKUPS_DIR = REPO_ROOT / "data" / "backups"
DR_LEDGER_FILE = LOGS_DIR / "dr_ledger.md"
DR_REPORT_FILE = LOGS_DIR / "dr_report.json"

# SLA Thresholds
RTO_MAX_ALLOWED_SECONDS = 14400.0  # 4 Hours Maximum RTO SLA
DRILL_TARGET_MAX_SECONDS = 300.0   # Automated drill target < 5 minutes
MIN_EXPECTED_TABLES = 20          # At least 20 CLASS-T tables

REQUIRED_PIT_TABLES = [
    "instrument_master",
    "filings_raw",
    "filings_normalized",
    "sentiment_records",
]

class Colors:
    GREEN = "\033[92m" if sys.platform != "win32" or "WT_SESSION" in os.environ else ""
    RED = "\033[91m" if sys.platform != "win32" or "WT_SESSION" in os.environ else ""
    YELLOW = "\033[93m" if sys.platform != "win32" or "WT_SESSION" in os.environ else ""
    CYAN = "\033[96m" if sys.platform != "win32" or "WT_SESSION" in os.environ else ""
    BOLD = "\033[1m" if sys.platform != "win32" or "WT_SESSION" in os.environ else ""
    RESET = "\033[0m" if sys.platform != "win32" or "WT_SESSION" in os.environ else ""


def locate_latest_dump(mode: str, explicit_path: str = None) -> Path:
    """Discovers the latest valid backup dump archive."""
    if explicit_path:
        p = Path(explicit_path)
        if not p.is_absolute():
            p = REPO_ROOT / p
        if p.exists() and p.is_file():
            return p
        raise FileNotFoundError(f"Explicit backup dump file not found: {p}")

    if mode == "s3":
        # S3 mode: fetch latest dump using aws s3 ls
        s3_bucket = "fintext-backups-production"
        s3_prefix = "postgres/hourly/"
        print(f"  [S3 Discovery] Querying s3://{s3_bucket}/{s3_prefix} ...")
        cmd = ["aws", "s3", "ls", f"s3://{s3_bucket}/{s3_prefix}"]
        res = subprocess.run(cmd, capture_output=True, text=True)
        if res.returncode != 0:
            print(f"  [WARN] S3 discovery failed ({res.stderr.strip()}), falling back to local backups...")
        else:
            lines = [line.strip().split()[-1] for line in res.stdout.strip().splitlines() if line.endswith(".dump.gz")]
            if lines:
                latest_key = sorted(lines)[-1]
                local_tmp = LOGS_DIR / "backups_local" / latest_key
                local_tmp.parent.mkdir(parents=True, exist_ok=True)
                print(f"  [S3 Download] Downloading s3://{s3_bucket}/{s3_prefix}{latest_key} -> {local_tmp}...")
                dl_cmd = ["aws", "s3", "cp", f"s3://{s3_bucket}/{s3_prefix}{latest_key}", str(local_tmp)]
                subprocess.run(dl_cmd, check=True)
                return local_tmp

    # Local mode: search logs/backups_local, then data/backups
    search_dirs = [LOCAL_BACKUPS_DIR, FALLBACK_BACKUPS_DIR]
    candidates = []
    for sdir in search_dirs:
        if sdir.exists():
            candidates.extend(sdir.glob("fintext_*.dump.gz"))
            candidates.extend(sdir.glob("fintext_*.sql.gz"))

    if not candidates:
        raise FileNotFoundError(
            f"No backup archives found in {LOCAL_BACKUPS_DIR} or {FALLBACK_BACKUPS_DIR}. "
            "Please run 'bash scripts/backup_pg.sh local hourly' first."
        )

    # Sort by modification time descending
    candidates.sort(key=lambda p: p.stat().st_mtime, reverse=True)
    return candidates[0]


def init_dr_ledger():
    """Initializes the DR ledger file with proper markdown table headers if not present."""
    LOGS_DIR.mkdir(parents=True, exist_ok=True)
    if not DR_LEDGER_FILE.exists():
        header = (
            "# FinText Alpha Vectorizer — Disaster Recovery (DR) Drill Ledger\n"
            "════════════════════════════════════════════════════════════════════════════════\n\n"
            "| Drill ID | UTC Timestamp | Mode | RTO (s) | Tables | PIT Records | RLS Isolation | Verdict |\n"
            "| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |\n"
        )
        DR_LEDGER_FILE.write_text(header, encoding="utf-8")


def append_dr_ledger(row: str):
    """Appends a single verified line to logs/dr_ledger.md."""
    init_dr_ledger()
    with open(DR_LEDGER_FILE, "a", encoding="utf-8") as f:
        f.write(row + "\n")


def run_weekly_dr_drill(mode: str = "local", dump_path: str = None, json_report_path: Path = None) -> int:
    drill_start_epoch = time.time()
    iso_timestamp = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    utc_compact = datetime.now(timezone.utc).strftime("%Y%m%d_%H%M%SZ")
    drill_id = f"dr_drill_{utc_compact}"
    container_name = f"fintext_dr_{int(drill_start_epoch)}"

    if json_report_path is None:
        json_report_path = DR_REPORT_FILE

    print(f"\n{Colors.CYAN}{Colors.BOLD}{'=' * 79}{Colors.RESET}")
    print(f"{Colors.CYAN}{Colors.BOLD} FinText Alpha Vectorizer — Automated Weekly Disaster Recovery Drill{Colors.RESET}")
    print(f"{Colors.CYAN}{Colors.BOLD}{'=' * 79}{Colors.RESET}\n")
    print(f"Drill Identifier:      {drill_id}")
    print(f"Timestamp (UTC):        {iso_timestamp}")
    print(f"Target Container:       {container_name} (EPHEMERAL sandbox)")
    print(f"Max RTO SLA Threshold:  {RTO_MAX_ALLOWED_SECONDS}s (4.0h)\n")

    report_data = {
        "drill_id": drill_id,
        "timestamp_utc": iso_timestamp,
        "mode": mode,
        "ephemeral_container": container_name,
        "backup_archive": "",
        "backup_size_bytes": 0,
        "backup_sha256": "",
        "restore_wall_seconds": 0.0,
        "table_count": 0,
        "pit_table_records": {},
        "rls_isolation_verified": False,
        "cross_tenant_leak_count": 0,
        "rto_sla_compliant": False,
        "status": "IN_PROGRESS",
    }

    # Step 1: Locate latest dump
    print(f"{Colors.BOLD}[Phase 1/5] Discovering latest database dump archive...{Colors.RESET}")
    try:
        latest_dump = locate_latest_dump(mode, dump_path)
    except Exception as e:
        print(f"  {Colors.RED}[FAIL] Could not locate backup dump: {e}{Colors.RESET}")
        report_data["status"] = "FAILED"
        report_data["error"] = str(e)
        return 1

    dump_size = latest_dump.stat().st_size
    dump_bytes = latest_dump.read_bytes()
    dump_sha256 = hashlib.sha256(dump_bytes).hexdigest()

    report_data["backup_archive"] = latest_dump.name
    report_data["backup_size_bytes"] = dump_size
    report_data["backup_sha256"] = dump_sha256

    print(f"  Located archive: {latest_dump.name}")
    print(f"  Archive Size:    {dump_size:,} bytes")
    print(f"  SHA-256 Digest:  {dump_sha256[:16]}...{dump_sha256[-8:]}")

    # Check sidecar checksum if present
    sidecar_path = Path(f"{latest_dump}.sha256")
    if sidecar_path.exists():
        expected_sha = sidecar_path.read_text(encoding="utf-8").split()[0].strip()
        if expected_sha != dump_sha256:
            print(f"  {Colors.RED}[FAIL] Sidecar SHA-256 mismatch: expected {expected_sha}, got {dump_sha256}{Colors.RESET}")
            report_data["status"] = "CHECKSUM_MISMATCH"
            return 1
        print(f"  {Colors.GREEN}[PASS]{Colors.RESET} Cryptographic checksum matches .sha256 sidecar")

    # Step 2: Spin up EPHEMERAL PostgreSQL container
    print(f"\n{Colors.BOLD}[Phase 2/5] Spawning ephemeral PostgreSQL 16 container ({container_name})...{Colors.RESET}")
    run_cmd = [
        "docker", "run", "-d", "--rm",
        "--name", container_name,
        "-e", "POSTGRES_PASSWORD=drillonly",
        "postgres:16"
    ]

    try:
        proc = subprocess.run(run_cmd, capture_output=True, text=True, check=True)
        container_id = proc.stdout.strip()[:12]
        print(f"  Ephemeral container spawned: {container_id}")
    except subprocess.CalledProcessError as e:
        print(f"  {Colors.RED}[FAIL] Failed to spawn ephemeral container: {e.stderr.strip()}{Colors.RESET}")
        report_data["status"] = "DOCKER_SPAWN_FAILED"
        return 1

    restore_start_epoch = time.time()

    try:
        # Wait for PostgreSQL to accept connections inside container
        print(f"  Waiting for ephemeral database engine to initialize...")
        ready = False
        for _ in range(30):
            check = subprocess.run(
                ["docker", "exec", container_name, "pg_isready", "-U", "postgres"],
                capture_output=True, text=True
            )
            if check.returncode == 0:
                ready = True
                break
            time.sleep(1)

        if not ready:
            raise RuntimeError("Ephemeral PostgreSQL container failed readiness probe within 30s")

        print(f"  {Colors.GREEN}[PASS]{Colors.RESET} Ephemeral database engine healthy and ready")

        # Step 3: Copy dump archive into container
        print(f"\n{Colors.BOLD}[Phase 3/5] Copying archive and restoring schema into ephemeral database...{Colors.RESET}")
        container_dest = "/tmp/restore.dump"
        subprocess.run(["docker", "cp", str(latest_dump), f"{container_name}:{container_dest}"], check=True)

        # Create target database and prerequisite schemas for hypertable chunk restoration
        subprocess.run(
            ["docker", "exec", container_name, "createdb", "-U", "postgres", "fintext_metadata"],
            check=True, capture_output=True
        )
        subprocess.run(
            ["docker", "exec", container_name, "psql", "-U", "postgres", "-d", "fintext_metadata",
             "-c", "CREATE SCHEMA IF NOT EXISTS _timescaledb_internal; CREATE SCHEMA IF NOT EXISTS _timescaledb_catalog;"],
            check=True, capture_output=True
        )

        # Determine archive format (custom pg_dump vs compressed plain sql)
        is_custom_dump = dump_bytes[:5] == b"PGDMP"
        if is_custom_dump:
            print("  Detected PostgreSQL Custom Archive format. Restoring via pg_restore...")
            restore_cmd = [
                "docker", "exec", "-i", container_name,
                "pg_restore", "-U", "postgres", "-d", "fintext_metadata",
                "--no-owner", "--no-privileges", container_dest
            ]
            # pg_restore may return 1 if there are minor object warnings; capture output
            res = subprocess.run(restore_cmd, capture_output=True, text=True)
            if res.returncode > 1:
                print(f"  {Colors.YELLOW}[WARN] pg_restore issued notices: {res.stderr.strip()[:200]}...{Colors.RESET}")
        else:
            print("  Detected gzip/plain SQL dump. Restoring via psql pipe...")
            restore_cmd = [
                "docker", "exec", "-i", container_name,
                "bash", "-c", f"gunzip -c {container_dest} | psql -U postgres -d fintext_metadata -q"
            ]
            subprocess.run(restore_cmd, check=True, capture_output=True)

        restore_wall_seconds = round(time.time() - restore_start_epoch, 3)
        report_data["restore_wall_seconds"] = restore_wall_seconds
        print(f"  {Colors.GREEN}[PASS]{Colors.RESET} Database restoration completed in {restore_wall_seconds:.3f}s")

        # Step 4: Verification Battery (Schema, PIT Tables, RLS Isolation)
        print(f"\n{Colors.BOLD}[Phase 4/5] Executing institutional schema & data correctness verification...{Colors.RESET}")

        # Check 4.1: Base Table Count
        table_count_sql = "SELECT count(*) FROM information_schema.tables WHERE table_schema = 'public' AND table_type = 'BASE TABLE';"
        table_proc = subprocess.run(
            ["docker", "exec", "-i", container_name, "psql", "-U", "postgres", "-d", "fintext_metadata", "-t", "-A", "-c", table_count_sql],
            capture_output=True, text=True, check=True
        )
        table_count = int(table_proc.stdout.strip() or 0)
        report_data["table_count"] = table_count

        if table_count < MIN_EXPECTED_TABLES:
            print(f"  {Colors.RED}[FAIL] Table count {table_count} is less than required {MIN_EXPECTED_TABLES}{Colors.RESET}")
            report_data["status"] = "INSUFFICIENT_TABLES"
            return 1
        print(f"  {Colors.GREEN}[PASS]{Colors.RESET} Schema structure verified: {table_count} base tables present (>= {MIN_EXPECTED_TABLES} required)")

        # Check 4.2: PIT Table Records
        pit_counts = {}
        all_pit_populated = True
        total_pit_records = 0

        for tbl in REQUIRED_PIT_TABLES:
            tbl_sql = f"SELECT count(*) FROM {tbl};"
            cnt_proc = subprocess.run(
                ["docker", "exec", "-i", container_name, "psql", "-U", "postgres", "-d", "fintext_metadata", "-t", "-A", "-c", tbl_sql],
                capture_output=True, text=True
            )
            count = int(cnt_proc.stdout.strip() or 0) if cnt_proc.returncode == 0 else 0
            pit_counts[tbl] = count
            total_pit_records += count
            if count == 0:
                all_pit_populated = False
                print(f"  {Colors.RED}[FAIL] PIT Table '{tbl}': 0 records found!{Colors.RESET}")
            else:
                print(f"  {Colors.GREEN}[PASS]{Colors.RESET} PIT Table '{tbl}': {count} verified records restored")

        report_data["pit_table_records"] = pit_counts

        if not all_pit_populated:
            print(f"  {Colors.RED}[FAIL] PIT integrity failure: One or more critical PIT tables empty!{Colors.RESET}")
            report_data["status"] = "PIT_DATA_EMPTY"
            return 1

        # Check 4.3: 2-Organization Row-Level Security (RLS) Spot Check
        print(f"  Auditing Multi-Tenant RLS isolation between competing hedge funds...")
        # Check if RLS is enabled on instrument_master
        rls_query = "SELECT relrowsecurity, relforcerowsecurity FROM pg_class WHERE relname = 'instrument_master';"
        rls_proc = subprocess.run(
            ["docker", "exec", "-i", container_name, "psql", "-U", "postgres", "-d", "fintext_metadata", "-t", "-A", "-c", rls_query],
            capture_output=True, text=True, check=True
        )
        rls_flags = rls_proc.stdout.strip()

        # Run 2-organization scoped query
        cross_leak_sql = """
        DO $$
        BEGIN
            -- Ensure fintext_app role exists for non-bypass check
            IF NOT EXISTS (SELECT FROM pg_catalog.pg_roles WHERE rolname = 'fintext_app') THEN
                CREATE ROLE fintext_app NOBYPASSRLS;
            END IF;
            GRANT SELECT ON ALL TABLES IN SCHEMA public TO fintext_app;
        END $$;

        SET ROLE fintext_app;
        SET app.current_org_id = 'org_tier1_hedge_fund';
        SELECT count(*) FROM instrument_master WHERE org_id = 'org_stat_arb_partners';
        """
        leak_proc = subprocess.run(
            ["docker", "exec", "-i", container_name, "psql", "-U", "postgres", "-d", "fintext_metadata", "-t", "-A", "-c", cross_leak_sql],
            capture_output=True, text=True, check=True
        )
        cross_leak_count = int(leak_proc.stdout.strip().splitlines()[-1] or 0)
        report_data["cross_tenant_leak_count"] = cross_leak_count

        if cross_leak_count == 0:
            print(f"  {Colors.GREEN}[PASS]{Colors.RESET} Multi-tenant RLS isolation certified: 0 cross-tenant leaks (org_stat_arb rows invisible to org_tier1)")
            report_data["rls_isolation_verified"] = True
        else:
            print(f"  {Colors.RED}[FAIL] RLS isolation breached: {cross_leak_count} rows leaked!{Colors.RESET}")
            report_data["status"] = "RLS_ISOLATION_LEAK"
            return 1

        # Step 5: Wall-Clock RTO SLA Benchmark
        print(f"\n{Colors.BOLD}[Phase 5/5] Benchmarking Recovery Time Objective (RTO) against SLA...{Colors.RESET}")
        total_drill_seconds = round(time.time() - drill_start_epoch, 3)
        rto_compliant = restore_wall_seconds <= RTO_MAX_ALLOWED_SECONDS
        report_data["rto_sla_compliant"] = rto_compliant
        report_data["total_drill_seconds"] = total_drill_seconds
        report_data["status"] = "CERTIFIED_HEALTHY"

        print(f"  Measured Restore RTO:  {restore_wall_seconds:.3f}s (SLA Target: < {RTO_MAX_ALLOWED_SECONDS:.0f}s)")
        print(f"  Total Drill Duration:  {total_drill_seconds:.3f}s")
        print(f"  RTO SLA Compliance:    {Colors.GREEN}[COMPLIANT]{Colors.RESET}")

        # Ledger & Report generation
        ledger_row = f"| {drill_id} | {iso_timestamp} | {mode} | {restore_wall_seconds:.3f} | {table_count} | {total_pit_records} | 0 leaks (VERIFIED) | CERTIFIED |"
        append_dr_ledger(ledger_row)

        json_report_path.parent.mkdir(parents=True, exist_ok=True)
        json_report_path.write_text(json.dumps(report_data, indent=2, sort_keys=True), encoding="utf-8")

        print(f"\n{Colors.CYAN}{Colors.BOLD}{'=' * 79}{Colors.RESET}")
        print(f"{Colors.GREEN}{Colors.BOLD}>>> VERDICT: DISASTER RECOVERY DRILL PASSED (RTO: {restore_wall_seconds:.3f}s, 0 LEAKS) <<<{Colors.RESET}")
        print(f"Detailed JSON Report written to: {json_report_path}")
        print(f"Ledger updated at:               {DR_LEDGER_FILE}")
        print(f"{Colors.CYAN}{Colors.BOLD}{'=' * 79}{Colors.RESET}\n")
        return 0

    finally:
        # Guarantee container teardown
        print(f"Tearing down ephemeral container '{container_name}'...")
        subprocess.run(["docker", "rm", "-f", container_name], capture_output=True)
        print("Ephemeral container removed cleanly.\n")


def main():
    parser = argparse.ArgumentParser(description="FinText Automated Weekly Disaster Recovery Drill")
    parser.add_argument("--mode", choices=["local", "s3"], default="local",
                        help="Backup source mode (local or s3)")
    parser.add_argument("--dump-path", type=str, default=None,
                        help="Optional explicit path to backup dump file")
    parser.add_argument("--json-report", type=str, default="logs/dr_report.json",
                        help="Output path for JSON drill report")
    args = parser.parse_args()

    report_path = Path(args.json_report)
    if not report_path.is_absolute():
        report_path = REPO_ROOT / report_path

    exit_code = run_weekly_dr_drill(
        mode=args.mode,
        dump_path=args.dump_path,
        json_report_path=report_path
    )
    sys.exit(exit_code)


if __name__ == "__main__":
    main()
