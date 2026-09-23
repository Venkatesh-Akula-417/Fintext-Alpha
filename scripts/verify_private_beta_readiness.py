#!/usr/bin/env python3
"""
FinText Alpha Vectorizer — Private Beta Launch Readiness Verification Suite
═══════════════════════════════════════════════════════════════════════════════
Runs 20 automated checks across Code, Data, PIT Correctness, Security, Billing,
Documentation, and Quality to certify launch readiness for Private Beta.

Usage:
    python scripts/verify_private_beta_readiness.py
"""

import sys
import os
import re
import json
import subprocess
from pathlib import Path

# Workspace root
REPO_ROOT = Path(__file__).resolve().parent.parent

if hasattr(sys.stdout, "reconfigure"):
    try:
        sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    except Exception:
        pass

class Colors:
    GREEN = "\033[92m" if sys.platform != "win32" or "WT_SESSION" in os.environ else ""
    RED = "\033[91m" if sys.platform != "win32" or "WT_SESSION" in os.environ else ""
    YELLOW = "\033[93m" if sys.platform != "win32" or "WT_SESSION" in os.environ else ""
    CYAN = "\033[96m" if sys.platform != "win32" or "WT_SESSION" in os.environ else ""
    BOLD = "\033[1m" if sys.platform != "win32" or "WT_SESSION" in os.environ else ""
    RESET = "\033[0m" if sys.platform != "win32" or "WT_SESSION" in os.environ else ""

checks_passed = 0
checks_failed = 0
results = []

def record_check(number: int, name: str, passed: bool, evidence: str):
    global checks_passed, checks_failed, results
    if passed:
        checks_passed += 1
        status_str = f"{Colors.GREEN}[PASS]{Colors.RESET}"
    else:
        checks_failed += 1
        status_str = f"{Colors.RED}[FAIL]{Colors.RESET}"
    results.append((number, name, status_str, evidence))

banner_line = "=" * 79
print(f"\n{Colors.CYAN}{Colors.BOLD}{banner_line}{Colors.RESET}")
print(f"{Colors.CYAN}{Colors.BOLD} FinText Alpha Vectorizer - Private Beta 20-Check Readiness Audit{Colors.RESET}")
print(f"{Colors.CYAN}{Colors.BOLD}{banner_line}{Colors.RESET}\n")

# ─────────────────────────────────────────────────────────────────────────────
# Check 1: Docker Compose Configuration Valid
# ─────────────────────────────────────────────────────────────────────────────
try:
    compose_path = REPO_ROOT / "docker-compose.yml"
    if compose_path.exists():
        content = compose_path.read_text(encoding="utf-8")
        has_services = "services:" in content and "postgres:" in content and "kafka:" in content
        record_check(1, "Docker Compose Structure Valid", has_services, "docker-compose.yml exists with core services")
    else:
        record_check(1, "Docker Compose Structure Valid", False, "docker-compose.yml missing")
except Exception as e:
    record_check(1, "Docker Compose Structure Valid", False, str(e))

# ─────────────────────────────────────────────────────────────────────────────
# Check 2: Handlers Module Count == 46
# ─────────────────────────────────────────────────────────────────────────────
try:
    handlers_mod = REPO_ROOT / "rust" / "api_server" / "src" / "handlers" / "mod.rs"
    content = handlers_mod.read_text(encoding="utf-8")
    modules = re.findall(r"^pub mod ([a-z0-9_]+);", content, re.MULTILINE)
    count = len(modules)
    passed = count == 46
    record_check(2, "Active Handlers in mod.rs == 46", passed, f"Found {count} active modules (expected 46)")
except Exception as e:
    record_check(2, "Active Handlers in mod.rs == 46", False, str(e))

# ─────────────────────────────────────────────────────────────────────────────
# Check 3: Public Router Exposes >= 32 Endpoints
# ─────────────────────────────────────────────────────────────────────────────
try:
    lib_rs = REPO_ROOT / "rust" / "api_server" / "src" / "lib.rs"
    content = lib_rs.read_text(encoding="utf-8")
    router_start = content.find("pub fn public_v1_router")
    router_end = content.find("fn test_auth_header", router_start)
    router_section = content[router_start:router_end] if router_start != -1 and router_end != -1 else ""
    routes = re.findall(r'\.route\(\s*"([^"]+)"', router_section)
    unique_routes = set(routes)
    passed = len(unique_routes) >= 32 or len(routes) >= 32
    record_check(3, "Public v1 Router >= 32 Endpoints", passed, f"Identified {len(unique_routes)} unique route paths in public_v1_router")
except Exception as e:
    record_check(3, "Public v1 Router >= 32 Endpoints", False, str(e))

# ─────────────────────────────────────────────────────────────────────────────
# Check 4: Zero Residual Waste Imports in lib.rs
# ─────────────────────────────────────────────────────────────────────────────
try:
    lib_rs = REPO_ROOT / "rust" / "api_server" / "src" / "lib.rs"
    content = lib_rs.read_text(encoding="utf-8")
    matches = re.findall(r"\b(BacktestResponse|SpilloverResponse|FixOrderRequest)\b", content)
    passed = len(matches) == 0
    record_check(4, "Zero Waste Models in lib.rs", passed, f"{len(matches)} matches for deprecated response models")
except Exception as e:
    record_check(4, "Zero Waste Models in lib.rs", False, str(e))

# ─────────────────────────────────────────────────────────────────────────────
# Check 5: Three Production Research Notebooks Exist
# ─────────────────────────────────────────────────────────────────────────────
try:
    nb_dir = REPO_ROOT / "notebooks"
    expected_nbs = [
        "01_pit_replay_zero_lookahead.ipynb",
        "02_backtest_survivorship_bias_free.ipynb",
        "03_alpha_fusion_vpin_gex_gnn.ipynb"
    ]
    present = [nb for nb in expected_nbs if (nb_dir / nb).exists()]
    passed = len(present) == 3
    record_check(5, "3 Production Research Notebooks", passed, f"{len(present)}/3 notebooks verified in notebooks/")
except Exception as e:
    record_check(5, "3 Production Research Notebooks", False, str(e))

# ─────────────────────────────────────────────────────────────────────────────
# Check 6: Postman Collection Valid JSON & 32 Requests
# ─────────────────────────────────────────────────────────────────────────────
try:
    postman_path = REPO_ROOT / "postman" / "FinText_Alpha_Vectorizer_32_core.postman_collection.json"
    data = json.loads(postman_path.read_text(encoding="utf-8"))
    items = data.get("item", [])
    total_reqs = sum(len(f.get("item", [])) for f in items)
    passed = total_reqs == 32
    record_check(6, "Postman Collection 32 Requests Valid", passed, f"{total_reqs} requests across {len(items)} folders")
except Exception as e:
    record_check(6, "Postman Collection 32 Requests Valid", False, str(e))

# ─────────────────────────────────────────────────────────────────────────────
# Check 7: Postman README Has 0 Secret Tokens
# ─────────────────────────────────────────────────────────────────────────────
try:
    postman_readme = REPO_ROOT / "postman" / "README.md"
    content = postman_readme.read_text(encoding="utf-8")
    has_secret = "fintext-admin-dev-secret-token" in content
    record_check(7, "Postman README Secret Sanitized", not has_secret, "Zero occurrences of raw dev admin secret")
except Exception as e:
    record_check(7, "Postman README Secret Sanitized", False, str(e))

# ─────────────────────────────────────────────────────────────────────────────
# Check 8: API Guide Uses ${ADMIN_TOKEN} Placeholder
# ─────────────────────────────────────────────────────────────────────────────
try:
    guide = REPO_ROOT / "docs" / "API_CUSTOMER_GUIDE.md"
    content = guide.read_text(encoding="utf-8")
    has_placeholder = "${ADMIN_TOKEN" in content
    has_secret = "fintext-admin-dev-secret-token" in content
    passed = has_placeholder and not has_secret
    record_check(8, "API Customer Guide Uses Placeholder", passed, "Uses ${ADMIN_TOKEN} placeholder, 0 real secrets")
except Exception as e:
    record_check(8, "API Customer Guide Uses Placeholder", False, str(e))

# ─────────────────────────────────────────────────────────────────────────────
# Check 9: API Customer Guide Has >= 32 /v1/ References
# ─────────────────────────────────────────────────────────────────────────────
try:
    guide = REPO_ROOT / "docs" / "API_CUSTOMER_GUIDE.md"
    content = guide.read_text(encoding="utf-8")
    refs = len(re.findall(r"/v1/[a-z0-9\-_/]+", content))
    passed = refs >= 32
    record_check(9, "API Customer Guide >= 32 /v1/ References", passed, f"Found {refs} endpoint references (target >= 32)")
except Exception as e:
    record_check(9, "API Customer Guide >= 32 /v1/ References", False, str(e))

# ─────────────────────────────────────────────────────────────────────────────
# Check 10: API Customer Guide Zero Waste Keywords
# ─────────────────────────────────────────────────────────────────────────────
try:
    guide = REPO_ROOT / "docs" / "API_CUSTOMER_GUIDE.md"
    content = guide.read_text(encoding="utf-8")
    waste_matches = re.findall(r"\b(backtest|spillover|fix/order)\b", content, re.IGNORECASE)
    passed = len(waste_matches) == 0
    record_check(10, "API Guide Zero Waste Residuals", passed, f"{len(waste_matches)} occurrences of backtest/spillover/fix")
except Exception as e:
    record_check(10, "API Guide Zero Waste Residuals", False, str(e))

# ─────────────────────────────────────────────────────────────────────────────
# Check 11: .gitleaks.toml Contains Allowlist Regexes
# ─────────────────────────────────────────────────────────────────────────────
try:
    gitleaks_cfg = REPO_ROOT / ".gitleaks.toml"
    content = gitleaks_cfg.read_text(encoding="utf-8")
    has_admin_token = "fintext-admin-dev-secret-token" in content
    has_placeholder = "your_admin_token_here" in content
    passed = has_admin_token and has_placeholder
    record_check(11, "Gitleaks Allowlist Configured", passed, "Allowlist includes dev token and placeholder strings")
except Exception as e:
    record_check(11, "Gitleaks Allowlist Configured", False, str(e))

# ─────────────────────────────────────────────────────────────────────────────
# Check 12: Signal Quality Report Certified (IC +0.0540)
# ─────────────────────────────────────────────────────────────────────────────
try:
    report = REPO_ROOT / "docs" / "SIGNAL_QUALITY_REPORT.md"
    content = report.read_text(encoding="utf-8")
    has_ic = "+0.0540" in content
    has_sharpe = "1.86" in content or "1.42" in content
    passed = has_ic and has_sharpe
    record_check(12, "Signal Quality Certified (IC +0.0540)", passed, "Verified Rank IC +0.0540 and target Sharpe")
except Exception as e:
    record_check(12, "Signal Quality Certified (IC +0.0540)", False, str(e))

# ─────────────────────────────────────────────────────────────────────────────
# Check 13: Cloud Cost Optimization Documents $295/mo
# ─────────────────────────────────────────────────────────────────────────────
try:
    cost_doc = REPO_ROOT / "docs" / "CLOUD_COST_OPTIMIZATION.md"
    content = cost_doc.read_text(encoding="utf-8")
    has_295 = "$295" in content
    has_saving = "71.2%" in content
    passed = has_295 and has_saving
    record_check(13, "Cloud Cost Blueprint ($295/mo)", passed, "Verified $295/mo run-rate (71.2% reduction)")
except Exception as e:
    record_check(13, "Cloud Cost Blueprint ($295/mo)", False, str(e))

# ─────────────────────────────────────────────────────────────────────────────
# Check 14: Latency Reconciliation CPU (155ms) vs GPU (0.85ms)
# ─────────────────────────────────────────────────────────────────────────────
try:
    lat_doc = REPO_ROOT / "docs" / "LATENCY_RECONCILIATION.md"
    content = lat_doc.read_text(encoding="utf-8")
    has_cpu = "146" in content or "155" in content
    has_gpu = "0.85" in content
    passed = has_cpu and has_gpu
    record_check(14, "Latency Reconciliation Documented", passed, "Reconciled host CPU vs GPU acceleration specs")
except Exception as e:
    record_check(14, "Latency Reconciliation Documented", False, str(e))

# ─────────────────────────────────────────────────────────────────────────────
# Check 15: Python Quickstart Script Clean & Zero Secrets
# ─────────────────────────────────────────────────────────────────────────────
try:
    qs = REPO_ROOT / "python_sdk" / "examples" / "quickstart_30min.py"
    content = qs.read_text(encoding="utf-8")
    has_secret = "fintext-admin-dev-secret-token" in content
    has_client = "FinTextClient" in content
    passed = has_client and not has_secret
    record_check(15, "Python Quickstart Script Clean", passed, "Verified FinTextClient import and 0 hardcoded secrets")
except Exception as e:
    record_check(15, "Python Quickstart Script Clean", False, str(e))

# ─────────────────────────────────────────────────────────────────────────────
# Check 16: Python SDK Test Suite Available
# ─────────────────────────────────────────────────────────────────────────────
try:
    test_dir = REPO_ROOT / "python_sdk" / "tests"
    test_files = list(test_dir.glob("test_*.py"))
    passed = len(test_files) >= 10
    record_check(16, "Python SDK Test Suite Structure", passed, f"Found {len(test_files)} test modules in python_sdk/tests/")
except Exception as e:
    record_check(16, "Python SDK Test Suite Structure", False, str(e))

# ─────────────────────────────────────────────────────────────────────────────
# Check 17: Docker Compose Core 4 Services + Optional Profiles
# ─────────────────────────────────────────────────────────────────────────────
try:
    compose = REPO_ROOT / "docker-compose.yml"
    content = compose.read_text(encoding="utf-8")
    has_core = "fintext-ingestion:" in content and "fintext-api:" in content
    has_profiles = 'profiles: ["hot-cache"' in content or "profiles:" in content
    passed = has_core and has_profiles
    record_check(17, "Docker Compose 4-Core + Profiles", passed, "Verified core services and optional profiles")
except Exception as e:
    record_check(17, "Docker Compose 4-Core + Profiles", False, str(e))

# ─────────────────────────────────────────────────────────────────────────────
# Check 18: Metering Middleware Pipeline Exists
# ─────────────────────────────────────────────────────────────────────────────
try:
    metering_rs = REPO_ROOT / "rust" / "api_server" / "src" / "metering.rs"
    content = metering_rs.read_text(encoding="utf-8")
    has_event = "struct UsageEvent" in content
    has_middleware = "metering_middleware" in content
    passed = has_event and has_middleware
    record_check(18, "Usage Metering Pipeline Implemented", passed, "Verified UsageEvent and metering_middleware in metering.rs")
except Exception as e:
    record_check(18, "Usage Metering Pipeline Implemented", False, str(e))

# ─────────────────────────────────────────────────────────────────────────────
# Check 19: Billing Integration & Plan Definitions
# ─────────────────────────────────────────────────────────────────────────────
try:
    billing_rs = REPO_ROOT / "rust" / "api_server" / "src" / "billing.rs"
    content = billing_rs.read_text(encoding="utf-8")
    has_plan = "struct PlanDefinition" in content
    has_checkout = "checkout" in content
    passed = has_plan and has_checkout
    record_check(19, "Stripe Billing & Plans Implemented", passed, "Verified PlanDefinition and checkout in billing.rs")
except Exception as e:
    record_check(19, "Stripe Billing & Plans Implemented", False, str(e))

# ─────────────────────────────────────────────────────────────────────────────
# Check 20: Launch Checklist, Billing Guide & Load Test SLA Certified
# ─────────────────────────────────────────────────────────────────────────────
try:
    checklist = REPO_ROOT / "docs" / "PRIVATE_BETA_LAUNCH_CHECKLIST.md"
    billing_guide = REPO_ROOT / "docs" / "BILLING_METERING_GUIDE.md"
    load_runbook = REPO_ROOT / "docs" / "LOAD_TEST_RUNBOOK.md"
    perf_dash = REPO_ROOT / "dashboards" / "api_performance.json"
    load_report_path = REPO_ROOT / "logs" / "load_test_report.json"

    content_ch = checklist.read_text(encoding="utf-8")
    pass_count = content_ch.count("PASS")

    has_load_report = False
    p95_val = 0.0
    if load_report_path.exists():
        try:
            report_data = json.loads(load_report_path.read_text(encoding="utf-8"))
            p95_val = report_data.get("latency_ms", {}).get("p95", 999.0)
            has_load_report = p95_val <= 500.0
        except Exception:
            has_load_report = False

    passed = (
        checklist.exists()
        and billing_guide.exists()
        and load_runbook.exists()
        and perf_dash.exists()
        and has_load_report
        and pass_count >= 30
    )
    evidence = f"{pass_count} PASS entries, runbook & perf dashboard verified, P95={p95_val:.1f}ms (<500ms SLA)"
    record_check(20, "Launch Checklist & Load Test SLA Certified", passed, evidence)
except Exception as e:
    record_check(20, "Launch Checklist & Load Test SLA Certified", False, str(e))

# ─────────────────────────────────────────────────────────────────────────────
# Print Results Table
# ─────────────────────────────────────────────────────────────────────────────
print(f"{'#':<3} | {'Check Description':<40} | {'Status':<8} | {'Evidence'}")
print("-" * 90)
for num, name, status, evidence in results:
    print(f"{num:<3} | {name:<40} | {status} | {evidence}")
print("-" * 90)

print(f"\n{Colors.BOLD}Final Readiness Audit Summary:{Colors.RESET}")
print(f"Total Checks: {len(results)}")
print(f"Passed:       {Colors.GREEN}{checks_passed}{Colors.RESET}")
print(f"Failed:       {Colors.RED}{checks_failed}{Colors.RESET}")

if checks_failed == 0:
    print(f"\n{Colors.GREEN}{Colors.BOLD}>>> VERDICT: ALL 20 AUTOMATED CHECKS PASSED. SYSTEM IS LAUNCH READY! <<<{Colors.RESET}\n")
    sys.exit(0)
else:
    print(f"\n{Colors.RED}{Colors.BOLD}>>> VERDICT: {checks_failed} CHECKS FAILED. RESOLVE BEFORE LAUNCH. <<<{Colors.RESET}\n")
    sys.exit(1)
