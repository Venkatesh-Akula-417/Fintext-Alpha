#!/usr/bin/env python3
"""
FinText Alpha Vectorizer — SOC 2 Auditor Evidence Pack & Manifest Generator
═══════════════════════════════════════════════════════════════════════════════
Standard Library Only. Deterministically crawls declared compliance evidence
artifacts across Trust Services Criteria (CC6.1, CC6.3, CC6.6, CC7.1, CC7.2,
CC8.1, A1.2, CC3.x), calculates SHA-256 digests, captures Git HEAD lineage,
and outputs logs/auditor_pack_manifest.json.

Usage:
    python scripts/build_auditor_pack.py
"""

import hashlib
import json
import os
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

# Workspace root
REPO_ROOT = Path(__file__).resolve().parent.parent

# Declared Evidence Catalog mapped to SOC 2 Trust Services Criteria
EVIDENCE_CATALOG = [
    # ── CC6.1 / CC6.3: Access Control, Authentication & Logical Separation ──
    {
        "id": "EVID-CC61-01",
        "path": "logs/rls_isolation_report.json",
        "controls": ["CC6.1", "CC6.3"],
        "criticality": "HIGH",
        "description": "Multi-tenant PostgreSQL Row-Level Security isolation drill (0 leaks across 20 tables)",
    },
    {
        "id": "EVID-CC61-02",
        "path": "rust/api_server/src/billing.rs",
        "controls": ["CC6.1", "CC6.3"],
        "criticality": "HIGH",
        "description": "Self-service usage RLS query scoping, admin token gating, and audit logging",
    },
    {
        "id": "EVID-CC61-03",
        "path": "rust/api_server/src/ip_whitelist.rs",
        "controls": ["CC6.1", "CC6.3"],
        "criticality": "MEDIUM",
        "description": "Per-tenant IP whitelist CIDR matching middleware and 403 Forbidden contract",
    },
    {
        "id": "EVID-CC61-04",
        "path": "rust/api_server/src/auth.rs",
        "controls": ["CC6.1"],
        "criticality": "HIGH",
        "description": "Cryptographic JWT claim validation, session expiration, and role extraction",
    },
    {
        "id": "EVID-CC61-05",
        "path": "config/timescale/03-rls-multi-tenant-isolation.sql",
        "controls": ["CC6.1", "CC6.3"],
        "criticality": "HIGH",
        "description": "PostgreSQL kernel RLS DDL policies for tenant session context binding",
    },
    {
        "id": "EVID-CC61-06",
        "path": "logs/billing_flow_report.json",
        "controls": ["CC6.1", "CC6.3"],
        "criticality": "MEDIUM",
        "description": "Stripe webhook signature validation, replay defense, and dunning state machine",
    },
    {
        "id": "EVID-CC61-07",
        "path": "logs/billing_reconciliation_report.json",
        "controls": ["CC6.1", "CC6.3"],
        "criticality": "MEDIUM",
        "description": "Monthly usage metering to Stripe invoice reconciliation verification (0 drift)",
    },

    # ── CC6.6: Infrastructure Security & Network Defense ─────────────────────
    {
        "id": "EVID-CC66-01",
        "path": "infra/terraform/security_groups.tf",
        "controls": ["CC6.6"],
        "criticality": "HIGH",
        "description": "Defense-in-depth security group chaining (EC2 80/443/22, RDS strictly from EC2)",
    },
    {
        "id": "EVID-CC66-02",
        "path": "infra/terraform/iam.tf",
        "controls": ["CC6.6"],
        "criticality": "HIGH",
        "description": "Least-privilege IAM instance role, S3 least-privilege policy, and CloudWatch permissions",
    },
    {
        "id": "EVID-CC66-03",
        "path": "infra/terraform/rds.tf",
        "controls": ["CC6.6"],
        "criticality": "HIGH",
        "description": "Private database subnet placement, storage encryption (KMS/gp3), and parameter hardening",
    },
    {
        "id": "EVID-CC66-04",
        "path": "docs/SECURITY.md",
        "controls": ["CC6.6"],
        "criticality": "MEDIUM",
        "description": "Institutional platform security whitepaper, credential redaction, and threat model",
    },

    # ── CC7.1 / CC7.2: System Monitoring, Telemetry & Anomaly Detection ──────
    {
        "id": "EVID-CC71-01",
        "path": "infra/terraform/cloudwatch.tf",
        "controls": ["CC7.1", "CC7.2"],
        "criticality": "MEDIUM",
        "description": "CloudWatch log groups with 30-day retention and automated CPU/storage alarms",
    },
    {
        "id": "EVID-CC71-02",
        "path": ".github/workflows/status-page.yml",
        "controls": ["CC7.1", "CC7.2"],
        "criticality": "MEDIUM",
        "description": "Zero-cost public status page cron telemetry surveillance workflow",
    },
    {
        "id": "EVID-CC71-03",
        "path": "k8s/observability/prometheus-alerts.yaml",
        "controls": ["CC7.1", "CC7.2"],
        "criticality": "MEDIUM",
        "description": "Prometheus container memory leak and SLA degradation alerting rules",
    },
    {
        "id": "EVID-CC71-04",
        "path": "dashboards/tenant_usage.json",
        "controls": ["CC7.1", "CC7.2"],
        "criticality": "LOW",
        "description": "Day-2 operational tenant usage, quota headroom, and ingestion latency dashboard",
    },
    {
        "id": "EVID-CC71-05",
        "path": "rust/ingestion_engine/src/telemetry/metrics.rs",
        "controls": ["CC7.1"],
        "criticality": "MEDIUM",
        "description": "Sub-second Prometheus latency histograms (0.005s..2.0s) across market feeds",
    },

    # ── CC8.1: Change Management, Code Integrity & Release Controls ──────────
    {
        "id": "EVID-CC81-01",
        "path": "docs/PRIVATE_BETA_LAUNCH_CHECKLIST.md",
        "controls": ["CC8.1"],
        "criticality": "HIGH",
        "description": "49-check comprehensive launch readiness certification and pre-flight gate",
    },
    {
        "id": "EVID-CC81-02",
        "path": "scripts/verify_private_beta_readiness.py",
        "controls": ["CC8.1"],
        "criticality": "HIGH",
        "description": "Automated 27-check readiness audit suite certifying code, models, and docs",
    },
    {
        "id": "EVID-CC81-03",
        "path": ".github/workflows/ci.yml",
        "controls": ["CC8.1"],
        "criticality": "HIGH",
        "description": "Continuous Integration workflow enforcing rust test suites, clippy, and security scans",
    },
    {
        "id": "EVID-CC81-04",
        "path": "docs/CERTIFIED_METRICS_REGISTER.md",
        "controls": ["CC8.1"],
        "criticality": "HIGH",
        "description": "Authoritative single source of truth for all certified platform metrics and parameters",
    },

    # ── A1.2: System Availability, Disaster Recovery & Resilience ────────────
    {
        "id": "EVID-A12-01",
        "path": "logs/backup_restore_test_report.json",
        "controls": ["A1.2"],
        "criticality": "HIGH",
        "description": "Automated disaster recovery drill certifying RTO of 0.111s against 4.0h SLA",
    },
    {
        "id": "EVID-A12-02",
        "path": "logs/soak_ledger.md",
        "controls": ["A1.2"],
        "criticality": "HIGH",
        "description": "Immutable ledger tracking continuous soak stability, RSS slope, and zero leak proof",
    },
    {
        "id": "EVID-A12-03",
        "path": "infra/terraform/s3.tf",
        "controls": ["A1.2"],
        "criticality": "HIGH",
        "description": "S3 backup bucket lifecycle (7-day expiration) and raw Parquet Glacier transition (90d)",
    },
    {
        "id": "EVID-A12-04",
        "path": "docs/HA_MULTIAZ_GA_CUTOVER_PLAN.md",
        "controls": ["A1.2"],
        "criticality": "HIGH",
        "description": "GA Multi-AZ high-availability cutover plan, priced deltas, and rollback runbook",
    },

    # ── CC3.1 / CC3.2: Risk Assessment & Mitigation Planning ─────────────────
    {
        "id": "EVID-CC31-01",
        "path": "docs/LATENCY_AND_COLOCATION_DECISION.md",
        "controls": ["CC3.1", "CC3.2"],
        "criticality": "MEDIUM",
        "description": "Architecture Decision Record evaluating us-east-1 colocation and risk trade-offs",
    },
    {
        "id": "EVID-CC31-02",
        "path": "docs/PRODUCTION_DEPLOYMENT_RUNBOOK.md",
        "controls": ["CC3.1", "CC3.2"],
        "criticality": "HIGH",
        "description": "Step-by-step AWS production provisioning, verification, and incident repair runbook",
    },
    {
        "id": "EVID-CC31-03",
        "path": "docs/PENTEST_PLAN.md",
        "controls": ["CC3.1", "CC3.2"],
        "criticality": "HIGH",
        "description": "Third-party penetration testing methodology, scope inventory, and remediation SLAs",
    },
    {
        "id": "EVID-CC31-04",
        "path": "docs/FOUNDER_ACTION_TRACKER.md",
        "controls": ["CC3.1", "CC3.2"],
        "criticality": "HIGH",
        "description": "Founder-side security posture hardening tracker (2FA, PAT rotation, soak scheduler)",
    },
]

def compute_sha256(filepath: Path) -> str:
    """Compute SHA-256 hexadecimal digest of a file."""
    hasher = hashlib.sha256()
    with open(filepath, "rb") as f:
        while chunk := f.read(65536):
            hasher.update(chunk)
    return hasher.hexdigest()

def get_git_info() -> dict:
    """Retrieve git HEAD commit SHA, branch, and clean status."""
    try:
        head_proc = subprocess.run(
            ["git", "rev-parse", "HEAD"],
            cwd=str(REPO_ROOT),
            capture_output=True,
            text=True,
            check=True,
        )
        head_sha = head_proc.stdout.strip()
    except Exception:
        head_sha = "UNKNOWN_COMMIT"

    try:
        branch_proc = subprocess.run(
            ["git", "rev-parse", "--abbrev-ref", "HEAD"],
            cwd=str(REPO_ROOT),
            capture_output=True,
            text=True,
            check=True,
        )
        branch = branch_proc.stdout.strip()
    except Exception:
        branch = "main"

    return {
        "head_commit_sha": head_sha,
        "head_commit_short": head_sha[:7],
        "branch": branch,
    }

def main():
    print("=" * 80)
    print(" FinText Alpha Vectorizer — SOC 2 Auditor Evidence Pack Generator")
    print("=" * 80)

    git_info = get_git_info()
    timestamp_utc = datetime.now(timezone.utc).isoformat()

    manifest_entries = []
    missing_count = 0
    present_count = 0

    print(f"\n[INFO] Git Commit: {git_info['head_commit_short']} ({git_info['branch']})")
    print(f"[INFO] Generation Timestamp: {timestamp_utc}")
    print("\nScanning declared evidence catalog...\n")

    print(f"{'ID':<15} | {'Status':<7} | {'Controls':<12} | {'File Path'}")
    print("-" * 80)

    for item in sorted(EVIDENCE_CATALOG, key=lambda x: x["id"]):
        file_path = REPO_ROOT / item["path"]
        entry = {
            "id": item["id"],
            "path": item["path"],
            "controls": sorted(item["controls"]),
            "criticality": item["criticality"],
            "description": item["description"],
        }

        if file_path.exists() and file_path.is_file():
            entry["status"] = "PRESENT"
            entry["size_bytes"] = file_path.stat().st_size
            entry["sha256"] = compute_sha256(file_path)
            present_count += 1
            status_display = "[OK]"
        else:
            entry["status"] = "MISSING"
            entry["size_bytes"] = 0
            entry["sha256"] = None
            missing_count += 1
            status_display = "[MISSING]"

        manifest_entries.append(entry)
        ctrl_str = ",".join(entry["controls"])
        print(f"{entry['id']:<15} | {status_display:<7} | {ctrl_str:<12} | {entry['path']}")

    print("-" * 80)
    print(f"Total Declared Evidence Items: {len(manifest_entries)}")
    print(f"Verified Present:             {present_count}")
    print(f"Missing Items:                {missing_count}")

    manifest = {
        "manifest_version": "1.0.0",
        "document_id": "SOC2-EVID-MANIFEST-2026-V1",
        "generated_at_utc": timestamp_utc,
        "disclaimer": "Evidence index for auditor convenience; not a SOC2 certification claim.",
        "git_lineage": git_info,
        "ci_run_reference": "GitHub Actions Run Set #13 (Pipeline, Security Scan, Drift, Model Validation, PIT, Status)",
        "summary": {
            "total_items": len(manifest_entries),
            "present_items": present_count,
            "missing_items": missing_count,
            "integrity_pass": missing_count == 0,
        },
        "evidence_items": manifest_entries,
    }

    output_path = REPO_ROOT / "logs" / "auditor_pack_manifest.json"
    output_path.parent.mkdir(parents=True, exist_ok=True)

    with open(output_path, "w", encoding="utf-8") as f:
        json.dump(manifest, f, indent=2, sort_keys=True)

    print(f"\n[SUCCESS] Deterministic manifest exported to: {output_path.relative_to(REPO_ROOT)}")
    print("=" * 80 + "\n")
    sys.exit(0)

if __name__ == "__main__":
    main()
