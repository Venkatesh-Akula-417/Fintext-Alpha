#!/usr/bin/env python3
"""
===============================================================================
FinText-Alpha-Vectorizer — Verification: ClickHouse Elimination (Suite #252)
===============================================================================
Verifies:
  1.  Zero active ClickHouse dependencies or imports across all Rust crates & Cargo.toml files
  2.  Complete deletion of ClickHouse Rust modules and SQL initializers
  3.  Rust API server state, routing, and background tasks clean of ClickHouse registry/workers
  4.  OpenAPI specification cleanliness (zero /clickhouse/* endpoints, schemas, or tags)
  5.  Configuration hygiene (config/config.yaml, config/feature_flags.yaml)
  6.  Docker Compose container simplification (zero ClickHouse services or volumes)
  7.  Kubernetes manifests & Velero/Prometheus configuration cleanliness
  8.  Python SDK models and test scripts clean of ClickHouse artifacts
  9.  QuestDB consolidated hot/historical time-series storage architecture integrity
  10. System documentation & architecture runbooks synchronization
===============================================================================
"""

import json
import os
from pathlib import Path
import re
import sys
import yaml

if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
if hasattr(sys.stderr, "reconfigure"):
    sys.stderr.reconfigure(encoding="utf-8", errors="replace")

PROJECT_ROOT = Path(__file__).resolve().parent.parent

passed = 0
failed = 0
total = 10


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


def run_all_phases():
    print("=" * 80)
    print(" FinText-Alpha-Vectorizer — ClickHouse Elimination Verification (Suite #252)")
    print("=" * 80)
    print(f" Project Root: {PROJECT_ROOT}\n")

    # ─────────────────────────────────────────────────────────────────────────
    # Phase 1: Zero active ClickHouse dependencies/imports across all Rust crates
    # ─────────────────────────────────────────────────────────────────────────
    p1_ok = True
    p1_details = []
    rust_dir = PROJECT_ROOT / "rust"
    for path in rust_dir.rglob("*.rs"):
        if "target" in path.parts:
            continue
        try:
            content = path.read_text(encoding="utf-8", errors="replace")
            # Search for active clickhouse imports or uses
            for line_no, line in enumerate(content.splitlines(), start=1):
                clean_line = line.strip()
                if clean_line.startswith("//") or clean_line.startswith("/*"):
                    continue
                if "clickhouse" in clean_line.lower() and not "questdb" in clean_line.lower():
                    p1_ok = False
                    p1_details.append(f"{path.relative_to(PROJECT_ROOT)}:{line_no} -> {clean_line}")
        except Exception as e:
            p1_ok = False
            p1_details.append(f"Error reading {path}: {e}")

    for cargo_file in rust_dir.rglob("Cargo.toml"):
        if "target" in cargo_file.parts:
            continue
        try:
            content = cargo_file.read_text(encoding="utf-8", errors="replace")
            if "clickhouse" in content.lower():
                p1_ok = False
                p1_details.append(f"Found ClickHouse dependency in {cargo_file.relative_to(PROJECT_ROOT)}")
        except Exception as e:
            p1_ok = False
            p1_details.append(f"Error reading {cargo_file}: {e}")

    report(
        1,
        "Zero active ClickHouse dependencies/imports in Rust workspace",
        p1_ok,
        "; ".join(p1_details) if p1_details else "",
    )

    # ─────────────────────────────────────────────────────────────────────────
    # Phase 2: Complete deletion of ClickHouse Rust modules and SQL initializers
    # ─────────────────────────────────────────────────────────────────────────
    deleted_paths = [
        rust_dir / "api_server" / "src" / "clickhouse_access.rs",
        rust_dir / "api_server" / "src" / "handlers" / "clickhouse_access.rs",
        rust_dir / "api_server" / "src" / "models" / "clickhouse.rs",
        PROJECT_ROOT / "config" / "clickhouse" / "init.sql",
        PROJECT_ROOT / "config" / "clickhouse",
    ]
    p2_ok = True
    p2_details = []
    for dp in deleted_paths:
        if dp.exists():
            p2_ok = False
            p2_details.append(f"File/directory still exists: {dp.relative_to(PROJECT_ROOT)}")

    report(
        2,
        "ClickHouse source files and SQL initializers completely deleted",
        p2_ok,
        "; ".join(p2_details) if p2_details else "",
    )

    # ─────────────────────────────────────────────────────────────────────────
    # Phase 3: Rust API server state, routing, and background tasks clean
    # ─────────────────────────────────────────────────────────────────────────
    p3_ok = True
    p3_details = []
    lib_rs = rust_dir / "api_server" / "src" / "lib.rs"
    state_rs = rust_dir / "api_server" / "src" / "state.rs"
    main_rs = rust_dir / "api_server" / "src" / "main.rs"
    handlers_mod = rust_dir / "api_server" / "src" / "handlers" / "mod.rs"
    models_mod = rust_dir / "api_server" / "src" / "models" / "mod.rs"

    for f in [lib_rs, state_rs, main_rs, handlers_mod, models_mod]:
        if f.exists():
            content = f.read_text(encoding="utf-8", errors="replace")
            if "clickhouse" in content.lower():
                p3_ok = False
                p3_details.append(f"ClickHouse reference found in {f.relative_to(PROJECT_ROOT)}")
        else:
            p3_ok = False
            p3_details.append(f"Missing expected file: {f.relative_to(PROJECT_ROOT)}")

    report(
        3,
        "Rust API server state, routing, and background tasks clean",
        p3_ok,
        "; ".join(p3_details) if p3_details else "",
    )

    # ─────────────────────────────────────────────────────────────────────────
    # Phase 4: OpenAPI specification cleanliness
    # ─────────────────────────────────────────────────────────────────────────
    openapi_rs = rust_dir / "api_server" / "src" / "openapi.rs"
    p4_ok = True
    p4_detail = ""
    if openapi_rs.exists():
        content = openapi_rs.read_text(encoding="utf-8", errors="replace")
        if "clickhouse" in content.lower():
            p4_ok = False
            p4_detail = "ClickHouse schema/path reference still present in openapi.rs"
    else:
        p4_ok = False
        p4_detail = "openapi.rs not found"

    report(
        4,
        "OpenAPI specification clean of /clickhouse/* endpoints & schemas",
        p4_ok,
        p4_detail,
    )

    # ─────────────────────────────────────────────────────────────────────────
    # Phase 5: Configuration hygiene (config.yaml, feature_flags.yaml)
    # ─────────────────────────────────────────────────────────────────────────
    config_yaml = PROJECT_ROOT / "config" / "config.yaml"
    ff_yaml = PROJECT_ROOT / "config" / "feature_flags.yaml"
    p5_ok = True
    p5_details = []

    if config_yaml.exists():
        with open(config_yaml, "r", encoding="utf-8") as f:
            cfg = yaml.safe_load(f)
            if "clickhouse" in cfg:
                p5_ok = False
                p5_details.append("clickhouse block present in config.yaml")
    else:
        p5_ok = False
        p5_details.append("config.yaml not found")

    if ff_yaml.exists():
        with open(ff_yaml, "r", encoding="utf-8") as f:
            flags = yaml.safe_load(f)
            if "clickhouse_cold_storage" in flags:
                p5_ok = False
                p5_details.append("clickhouse_cold_storage present in feature_flags.yaml")
    else:
        p5_ok = False
        p5_details.append("feature_flags.yaml not found")

    report(
        5,
        "Configuration files clean of ClickHouse configs & feature flags",
        p5_ok,
        "; ".join(p5_details) if p5_details else "",
    )

    # ─────────────────────────────────────────────────────────────────────────
    # Phase 6: Docker Compose container simplification
    # ─────────────────────────────────────────────────────────────────────────
    compose_path = PROJECT_ROOT / "docker-compose.yml"
    p6_ok = True
    p6_detail = ""
    if compose_path.exists():
        with open(compose_path, "r", encoding="utf-8") as f:
            compose = yaml.safe_load(f)
            services = compose.get("services", {})
            volumes = compose.get("volumes", {})
            if "clickhouse" in services:
                p6_ok = False
                p6_detail = "ClickHouse service found in docker-compose.yml"
            if "clickhouse_data" in volumes:
                p6_ok = False
                p6_detail = "clickhouse_data volume found in docker-compose.yml"
    else:
        p6_ok = False
        p6_detail = "docker-compose.yml not found"

    report(
        6,
        "Docker Compose clean of ClickHouse services and volumes",
        p6_ok,
        p6_detail,
    )

    # ─────────────────────────────────────────────────────────────────────────
    # Phase 7: Kubernetes & Infrastructure manifests cleanliness
    # ─────────────────────────────────────────────────────────────────────────
    p7_ok = True
    p7_details = []
    k8s_dir = PROJECT_ROOT / "k8s"

    deleted_k8s = [
        k8s_dir / "clickhouse-keeper.yaml",
        k8s_dir / "clickhouse-keeper-config.yaml",
        k8s_dir / "pdb-clickhouse.yaml",
    ]
    for kf in deleted_k8s:
        if kf.exists():
            p7_ok = False
            p7_details.append(f"Deleted k8s manifest still exists: {kf.relative_to(PROJECT_ROOT)}")

    kustomize = k8s_dir / "kustomization.yaml"
    if kustomize.exists():
        content = kustomize.read_text(encoding="utf-8", errors="replace")
        if "clickhouse" in content.lower():
            p7_ok = False
            p7_details.append("ClickHouse reference in k8s/kustomization.yaml")

    velero = k8s_dir / "velero-schedules.yaml"
    if velero.exists():
        content = velero.read_text(encoding="utf-8", errors="replace")
        if "clickhouse" in content.lower():
            p7_ok = False
            p7_details.append("ClickHouse reference in k8s/velero-schedules.yaml")

    prom = k8s_dir / "observability" / "prometheus.yaml"
    if prom.exists():
        content = prom.read_text(encoding="utf-8", errors="replace")
        if "clickhouse" in content.lower():
            p7_ok = False
            p7_details.append("ClickHouse reference in k8s/observability/prometheus.yaml")

    report(
        7,
        "Kubernetes manifests & observability configs clean",
        p7_ok,
        "; ".join(p7_details) if p7_details else "",
    )

    # ─────────────────────────────────────────────────────────────────────────
    # Phase 8: Python SDK models and test scripts synchronization
    # ─────────────────────────────────────────────────────────────────────────
    p8_ok = True
    p8_details = []
    sdk_models = PROJECT_ROOT / "python_sdk" / "src" / "fintext" / "models.py"
    if sdk_models.exists():
        content = sdk_models.read_text(encoding="utf-8", errors="replace")
        if "clickhouse" in content.lower():
            p8_ok = False
            p8_details.append("ClickHouse model found in python_sdk/models.py")
    else:
        p8_ok = False
        p8_details.append("python_sdk models.py not found")

    test_endpoints = PROJECT_ROOT / "scripts" / "test_all_endpoints.py"
    if test_endpoints.exists():
        content = test_endpoints.read_text(encoding="utf-8", errors="replace")
        if "clickhouse" in content.lower():
            p8_ok = False
            p8_details.append("ClickHouse test/reference found in scripts/test_all_endpoints.py")

    report(
        8,
        "Python SDK models & active verification scripts clean",
        p8_ok,
        "; ".join(p8_details) if p8_details else "",
    )

    # ─────────────────────────────────────────────────────────────────────────
    # Phase 9: QuestDB consolidated storage architecture integrity
    # ─────────────────────────────────────────────────────────────────────────
    p9_ok = True
    p9_details = []
    # Verify QuestDB client and ILP sink exist and are operational in Rust
    questdb_rs = rust_dir / "api_server" / "src" / "storage" / "questdb_client.rs"
    if not questdb_rs.exists():
        p9_ok = False
        p9_details.append("rust/api_server/src/storage/questdb_client.rs missing")
    else:
        q_content = questdb_rs.read_text(encoding="utf-8", errors="replace")
        if "sentiment_news" not in q_content:
            p9_ok = False
            p9_details.append("sentiment_news table missing from questdb_client.rs")

    # Verify QuestDB handles export endpoints
    export_rs = rust_dir / "api_server" / "src" / "handlers" / "export.rs"
    if not export_rs.exists():
        p9_ok = False
        p9_details.append("rust/api_server/src/handlers/export.rs missing")

    report(
        9,
        "QuestDB hot/historical time-series storage architecture intact",
        p9_ok,
        "; ".join(p9_details) if p9_details else "",
    )

    # ─────────────────────────────────────────────────────────────────────────
    # Phase 10: System documentation & architecture synchronization
    # ─────────────────────────────────────────────────────────────────────────
    p10_ok = True
    p10_details = []
    docs_to_check = [
        PROJECT_ROOT / "README.md",
        PROJECT_ROOT / "docs" / "current_architecture.md",
        PROJECT_ROOT / "docs" / "OPERATIONS.md",
        PROJECT_ROOT / "docs" / "ai_audit_ready_summary.md",
    ]
    for doc in docs_to_check:
        if doc.exists():
            content = doc.read_text(encoding="utf-8", errors="replace")
            # In docs, check for active ClickHouse service references (excluding historical changelogs/snapshots if any)
            # Find any mention of ClickHouse
            matches = [line.strip() for line in content.splitlines() if "clickhouse" in line.lower()]
            if matches:
                p10_ok = False
                p10_details.append(f"{doc.relative_to(PROJECT_ROOT)} has references: {matches[:2]}")
        else:
            p10_ok = False
            p10_details.append(f"Missing doc file: {doc.relative_to(PROJECT_ROOT)}")

    report(
        10,
        "Documentation & operational runbooks fully synchronized",
        p10_ok,
        "; ".join(p10_details) if p10_details else "",
    )

    # ─────────────────────────────────────────────────────────────────────────
    # Summary
    # ─────────────────────────────────────────────────────────────────────────
    print("\n" + "=" * 80)
    print(f" Suite #252 Results: {passed}/{total} Phases Passed (100% Target)")
    print("=" * 80)
    if passed == total:
        print("  ALL 10 VERIFICATION PHASES PASSED CLEANLY! [OK]\n")
        return 0
    else:
        print(f"  FAILED: {failed} phase(s) encountered issues [X]\n")
        return 1


if __name__ == "__main__":
    sys.exit(run_all_phases())
