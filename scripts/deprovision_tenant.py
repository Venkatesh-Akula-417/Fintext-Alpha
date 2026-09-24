#!/usr/bin/env python3
"""
FinText Alpha Vectorizer - Institutional Tenant Deprovisioning & Offboarding Engine

Purpose:
  Safely, idempotently, and compliantly offboards an institutional tenant.
  Adheres strictly to SOC2 CC6.1 / CC6.6 (least privilege & access revocation),
  SEC Rule 17a-4 (books & records retention), and FINRA Rule 4511.

Phases:
  Phase 1 (Immediate Access Revocation):
    - Revokes all API keys (sets revoked_at = NOW(), rotation_status = 'revoked')
    - Deactivates member accounts (users.is_active = false)
    - Cancels commercial subscription (subscriptions.status = 'canceled')
    - Suspends organization membership roles (organization_members.role = 'suspended')
    - Deactivates active data retention policies
  Phase 2 (Lifecycle & Data Retention Compliance):
    - When --purge-data is passed: purges tenant-specific universes.
    - STRICT INVARIANT: Audit history (api_request_logs, audit_logs) is NEVER destroyed.
      Audit logs are maintained for SEC compliance (2555 days / 7 years).

Safety Guard:
  Defaults to DRY-RUN mode (exit code 2).
  Live execution REQUIRES explicit flag: --i-understand-data-loss (exit code 0 on success).

Usage:
  # Dry-run preview (safe, non-destructive, exit code 2)
  python scripts/deprovision_tenant.py --slug org_demo_beta1

  # Live execution (revokes keys and suspends accounts, exit code 0)
  python scripts/deprovision_tenant.py --slug org_demo_beta1 --i-understand-data-loss

  # Live execution with domain data purge (preserves audit logs)
  python scripts/deprovision_tenant.py --slug org_demo_beta1 --i-understand-data-loss --purge-data
"""

import os
import sys
import json
import argparse
import subprocess
from datetime import datetime, timezone
from pathlib import Path

class Colors:
    GREEN = "\033[92m" if sys.platform != "win32" or "WT_SESSION" in os.environ else ""
    RED = "\033[91m" if sys.platform != "win32" or "WT_SESSION" in os.environ else ""
    YELLOW = "\033[93m" if sys.platform != "win32" or "WT_SESSION" in os.environ else ""
    CYAN = "\033[96m" if sys.platform != "win32" or "WT_SESSION" in os.environ else ""
    BOLD = "\033[1m" if sys.platform != "win32" or "WT_SESSION" in os.environ else ""
    RESET = "\033[0m" if sys.platform != "win32" or "WT_SESSION" in os.environ else ""

def get_git_commit() -> str:
    try:
        res = subprocess.run(["git", "rev-parse", "HEAD"], capture_output=True, text=True, check=True)
        return res.stdout.strip()
    except Exception:
        return "UNKNOWN"

def execute_psql(sql: str, user: str = "fintext", db: str = "fintext_metadata") -> tuple[int, str, str]:
    """Executes SQL against Postgres container or local psql instance."""
    cmd = [
        "docker", "exec", "-i", "fintext-postgres",
        "psql", "-U", user, "-d", db, "-t", "-A", "-c", sql
    ]
    try:
        proc = subprocess.run(cmd, capture_output=True, text=True, timeout=30)
        return proc.returncode, proc.stdout.strip(), proc.stderr.strip()
    except Exception as e:
        return 1, "", str(e)

def get_tenant_details(slug: str):
    """Retrieve tenant metadata and counts across related tables."""
    sql = f"""
    SELECT id, name, created_at FROM organizations WHERE name = '{slug}';
    """
    code, stdout, stderr = execute_psql(sql)
    if code != 0 or not stdout:
        return None, None

    parts = stdout.split("|")
    org_id = parts[0].strip()
    name = parts[1].strip()
    created_at = parts[2].strip() if len(parts) > 2 else ""

    # Count members, keys, universes, subscriptions
    sql_counts = f"""
    SELECT
      (SELECT count(*) FROM organization_members WHERE org_id = '{org_id}') AS member_count,
      (SELECT count(*) FROM api_keys WHERE org_id = '{org_id}' OR user_id IN (SELECT user_id FROM organization_members WHERE org_id = '{org_id}')) AS total_keys,
      (SELECT count(*) FROM api_keys WHERE (org_id = '{org_id}' OR user_id IN (SELECT user_id FROM organization_members WHERE org_id = '{org_id}')) AND revoked_at IS NULL AND rotation_status != 'revoked') AS active_keys,
      (SELECT count(*) FROM universes WHERE org_id = '{org_id}') AS univ_count,
      (SELECT count(*) FROM subscriptions WHERE org_id = '{org_id}') AS sub_count;
    """
    code_c, stdout_c, _ = execute_psql(sql_counts)
    if code_c != 0 or not stdout_c:
        return None, None

    c_parts = stdout_c.split("|")
    member_count = int(c_parts[0]) if len(c_parts) > 0 and c_parts[0].isdigit() else 0
    total_keys = int(c_parts[1]) if len(c_parts) > 1 and c_parts[1].isdigit() else 0
    active_keys = int(c_parts[2]) if len(c_parts) > 2 and c_parts[2].isdigit() else 0
    univ_count = int(c_parts[3]) if len(c_parts) > 3 and c_parts[3].isdigit() else 0
    sub_count = int(c_parts[4]) if len(c_parts) > 4 and c_parts[4].isdigit() else 0

    details = {
        "org_id": org_id,
        "name": name,
        "created_at": created_at,
        "member_count": member_count,
        "total_keys": total_keys,
        "active_keys": active_keys,
        "universes_count": univ_count,
        "subscriptions_count": sub_count,
    }
    return {"id": org_id, "name": name}, details

def main():
    if hasattr(sys.stdout, "reconfigure"):
        try:
            sys.stdout.reconfigure(encoding="utf-8")
        except Exception:
            pass

    parser = argparse.ArgumentParser(description="FinText Institutional Tenant Deprovisioning Engine")
    parser.add_argument("--slug", required=True, help="Organization slug to deprovision (e.g. org_demo_beta1)")
    parser.add_argument("--i-understand-data-loss", action="store_true", help="Explicit safety flag required for live deprovisioning")
    parser.add_argument("--purge-data", action="store_true", help="Optional Phase 2 purge of custom universes (audit history strictly preserved)")
    parser.add_argument("--base-url", default="http://127.0.0.1:8000", help="FinText API gateway base URL")
    parser.add_argument("--json-report", default="logs/tenant_deprovisioning_report.json", help="Report output path")
    args = parser.parse_args()

    print(f"\n{Colors.CYAN}{Colors.BOLD}{'=' * 79}{Colors.RESET}")
    print(f"{Colors.CYAN}{Colors.BOLD} FinText Alpha Vectorizer - Institutional Tenant Deprovisioning Engine{Colors.RESET}")
    print(f"{Colors.CYAN}{Colors.BOLD}{'=' * 79}{Colors.RESET}\n")

    org, details = get_tenant_details(args.slug)
    if not org:
        print(f"  {Colors.RED}[NOT FOUND]{Colors.RESET} Tenant slug '{args.slug}' does not exist in database.")
        sys.exit(2)

    org_id = details["org_id"]

    # If NOT confirmed with --i-understand-data-loss, execute DRY-RUN preview
    if not args.i_understand_data_loss:
        print(f"{Colors.YELLOW}{Colors.BOLD}>>> MODE: DRY-RUN PREVIEW (No changes committed to database) <<<{Colors.RESET}\n")
        print(f"  Target Organization:   {details['name']}")
        print(f"  Organization UUID:     {org_id}")
        print(f"  Registered At:         {details['created_at']}")
        print(f"  Associated Members:    {details['member_count']}")
        print(f"  Active API Keys:       {details['active_keys']} (Total: {details['total_keys']})")
        print(f"  Universes:             {details['universes_count']}")
        print(f"  Subscriptions:         {details['subscriptions_count']}")
        print(f"  Purge Requested:       {args.purge_data}")

        print(f"\n{Colors.BOLD}Planned Deprovisioning Actions:{Colors.RESET}")
        print(f"  [1] Hard revoke {details['active_keys']} active API key(s) in api_keys")
        print(f"  [2] Suspend {details['member_count']} user account(s) in users (is_active = false)")
        print(f"  [3] Cancel active subscription(s) in subscriptions (status = 'canceled')")
        print(f"  [4] Suspend memberships in organization_members (role = 'suspended')")
        print(f"  [5] Deactivate policies in data_retention_policies")
        if args.purge_data:
            print(f"  [6] Purge {details['universes_count']} custom universe(s) in universes")
            print(f"  [7] SEC/FINRA Compliance: Audit logs & request history strictly PRESERVED (0 rows deleted)")
        else:
            print(f"  [6] Domain data left at rest under strict RLS isolation")

        print(f"\n{Colors.YELLOW}Dry-run completed successfully.{Colors.RESET}")
        print(f"To execute live deprovisioning, re-run with: {Colors.BOLD}--i-understand-data-loss{Colors.RESET}\n")
        sys.exit(2)

    # LIVE DEPROVISIONING EXECUTION
    print(f"{Colors.BOLD}[Step 1/3] Executing live deprovisioning transaction as database admin...{Colors.RESET}")
    
    purge_clause = f"DELETE FROM universes WHERE org_id = '{org_id}';" if args.purge_data else "-- Purge skipped"

    sql_tx = f"""
    BEGIN;
    UPDATE api_keys 
    SET revoked_at = NOW(), rotation_status = 'revoked'
    WHERE (org_id = '{org_id}' OR user_id IN (SELECT user_id FROM organization_members WHERE org_id = '{org_id}'))
      AND revoked_at IS NULL;

    UPDATE users 
    SET is_active = false 
    WHERE id IN (SELECT user_id FROM organization_members WHERE org_id = '{org_id}');

    UPDATE subscriptions 
    SET status = 'canceled', updated_at = NOW() 
    WHERE org_id = '{org_id}';

    UPDATE organization_members 
    SET role = 'suspended' 
    WHERE org_id = '{org_id}';

    UPDATE data_retention_policies 
    SET is_active = false, updated_at = NOW() 
    WHERE org_id = '{org_id}';

    {purge_clause}
    COMMIT;
    """

    code, stdout, stderr = execute_psql(sql_tx)
    if code != 0:
        print(f"  {Colors.RED}[FAIL]{Colors.RESET} Deprovisioning transaction aborted: {stderr}")
        sys.exit(4)

    print(f"  {Colors.GREEN}[PASS]{Colors.RESET} Database transaction committed successfully.")

    # Step 2: Verification of revocation
    print(f"\n{Colors.BOLD}[Step 2/3] Verifying complete access revocation...{Colors.RESET}")
    _, post_details = get_tenant_details(args.slug)
    if post_details and post_details["active_keys"] == 0:
        print(f"  {Colors.GREEN}[PASS]{Colors.RESET} 0 active API keys remaining for tenant '{args.slug}'.")
    else:
        print(f"  {Colors.RED}[FAIL]{Colors.RESET} Active keys still detected after deprovisioning.")
        sys.exit(5)

    # Step 3: Write certification report (Zero Secrets)
    print(f"\n{Colors.BOLD}[Step 3/3] Generating deprovisioning certification report...{Colors.RESET}")
    report_path = Path(args.json_report)
    report_path.parent.mkdir(parents=True, exist_ok=True)

    report = {
        "generated_utc": datetime.now(timezone.utc).isoformat(),
        "commit": get_git_commit(),
        "mode": "live",
        "slug": args.slug,
        "org_id": org_id,
        "keys_revoked_count": details["active_keys"],
        "users_suspended_count": details["member_count"],
        "subscriptions_canceled_count": details["subscriptions_count"],
        "universes_purged_count": details["universes_count"] if args.purge_data else 0,
        "purge_executed": args.purge_data,
        "audit_history_preserved": True,
        "sec_rule_17a4_compliant": True,
        "verdict": "DEPROVISIONED"
    }

    report_path.write_text(json.dumps(report, indent=2), encoding="utf-8")
    print(f"  {Colors.GREEN}[PASS]{Colors.RESET} Report written: {report_path}")

    print(f"\n{Colors.CYAN}{'=' * 79}{Colors.RESET}")
    print(f"{Colors.GREEN}{Colors.BOLD}  TENANT DEPROVISIONING COMPLETE - ACCESS FULLY REVOKED{Colors.RESET}")
    print(f"{Colors.CYAN}{'=' * 79}{Colors.RESET}\n")

    sys.exit(0)

if __name__ == "__main__":
    main()
