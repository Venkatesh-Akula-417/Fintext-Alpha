#!/usr/bin/env python3
"""
===============================================================================
FinText-Alpha-Vectorizer — Verification: GraphQL Elimination (Suite #231)
===============================================================================
Verifies:
  1.  Zero active async-graphql dependencies across all Cargo.toml files in workspace
  2.  Complete deletion of rust/api_server/src/graphql_api.rs
  3.  Rust API server routing & module hygiene in lib.rs (zero /graphql routes or handlers)
  4.  AppState in state.rs clean of GraphQL schemas or contexts
  5.  Python SDK (FinTextClient & FinTextAsyncClient) clean of graphql() methods
  6.  Python SDK test suite clean of /graphql mock endpoints and GraphQL test cases
  7.  Complete deletion of legacy scripts/verify_graphql.py test suite
  8.  System documentation & READMEs clean of GraphQL references and playground links
  9.  OpenAPI 3.0 specification and REST API routing surface integrity
  10. Codebase wide audit confirming zero active GraphQL imports or active code paths
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
    print(" FinText-Alpha-Vectorizer — GraphQL Elimination Verification (Suite #231)")
    print("=" * 80)
    print(f" Project Root: {PROJECT_ROOT}\n")

    # ─────────────────────────────────────────────────────────────────────────
    # Phase 1: Zero active async-graphql dependencies in Cargo.toml files
    # ─────────────────────────────────────────────────────────────────────────
    p1_ok = True
    p1_details = []
    rust_dir = PROJECT_ROOT / "rust"
    for cargo_file in rust_dir.rglob("Cargo.toml"):
        if "target" in cargo_file.parts:
            continue
        try:
            content = cargo_file.read_text(encoding="utf-8", errors="replace")
            if "async-graphql" in content.lower():
                p1_ok = False
                p1_details.append(f"Found async-graphql in {cargo_file.relative_to(PROJECT_ROOT)}")
        except Exception as e:
            p1_ok = False
            p1_details.append(f"Error reading {cargo_file}: {e}")

    report(
        1,
        "Zero active async-graphql dependencies in Rust workspace Cargo.toml files",
        p1_ok,
        "; ".join(p1_details) if p1_details else "",
    )

    # ─────────────────────────────────────────────────────────────────────────
    # Phase 2: Complete deletion of rust/api_server/src/graphql_api.rs
    # ─────────────────────────────────────────────────────────────────────────
    gql_rs = rust_dir / "api_server" / "src" / "graphql_api.rs"
    p2_ok = not gql_rs.exists()
    report(
        2,
        "graphql_api.rs source module completely deleted",
        p2_ok,
        f"{gql_rs.relative_to(PROJECT_ROOT)} still exists" if not p2_ok else "",
    )

    # ─────────────────────────────────────────────────────────────────────────
    # Phase 3: Rust API server routing & module hygiene in lib.rs
    # ─────────────────────────────────────────────────────────────────────────
    lib_rs = rust_dir / "api_server" / "src" / "lib.rs"
    p3_ok = True
    p3_details = []
    if lib_rs.exists():
        content = lib_rs.read_text(encoding="utf-8", errors="replace")
        if "graphql_api" in content.lower():
            p3_ok = False
            p3_details.append("graphql_api reference found in lib.rs")
        if "graphql_handler" in content.lower():
            p3_ok = False
            p3_details.append("graphql_handler reference found in lib.rs")
        if "graphql_playground" in content.lower():
            p3_ok = False
            p3_details.append("graphql_playground reference found in lib.rs")
        if '"/graphql"' in content.lower():
            p3_ok = False
            p3_details.append('"/graphql" route found in lib.rs')
    else:
        p3_ok = False
        p3_details.append("lib.rs not found")

    report(
        3,
        "Rust API server lib.rs clean of GraphQL routes, modules & handlers",
        p3_ok,
        "; ".join(p3_details) if p3_details else "",
    )

    # ─────────────────────────────────────────────────────────────────────────
    # Phase 4: AppState in state.rs clean of GraphQL schemas
    # ─────────────────────────────────────────────────────────────────────────
    state_rs = rust_dir / "api_server" / "src" / "state.rs"
    p4_ok = True
    p4_detail = ""
    if state_rs.exists():
        content = state_rs.read_text(encoding="utf-8", errors="replace")
        if "graphql" in content.lower():
            p4_ok = False
            p4_detail = "GraphQL schema or reference found in state.rs"
    else:
        p4_ok = False
        p4_detail = "state.rs not found"

    report(
        4,
        "AppState in state.rs clean of GraphQL schemas or contexts",
        p4_ok,
        p4_detail,
    )

    # ─────────────────────────────────────────────────────────────────────────
    # Phase 5: Python SDK clean of graphql() methods
    # ─────────────────────────────────────────────────────────────────────────
    client_py = PROJECT_ROOT / "python_sdk" / "src" / "fintext" / "client.py"
    async_client_py = PROJECT_ROOT / "python_sdk" / "src" / "fintext" / "async_client.py"
    p5_ok = True
    p5_details = []

    if client_py.exists():
        content = client_py.read_text(encoding="utf-8", errors="replace")
        if "def graphql(" in content:
            p5_ok = False
            p5_details.append("def graphql() method found in client.py")
    else:
        p5_ok = False
        p5_details.append("client.py not found")

    if async_client_py.exists():
        content = async_client_py.read_text(encoding="utf-8", errors="replace")
        if "def graphql(" in content:
            p5_ok = False
            p5_details.append("def graphql() method found in async_client.py")
    else:
        p5_ok = False
        p5_details.append("async_client.py not found")

    report(
        5,
        "Python SDK (FinTextClient & FinTextAsyncClient) clean of graphql() methods",
        p5_ok,
        "; ".join(p5_details) if p5_details else "",
    )

    # ─────────────────────────────────────────────────────────────────────────
    # Phase 6: Python SDK test suite clean of GraphQL tests
    # ─────────────────────────────────────────────────────────────────────────
    test_client_py = PROJECT_ROOT / "python_sdk" / "tests" / "test_client.py"
    test_async_client_py = PROJECT_ROOT / "python_sdk" / "tests" / "test_async_client.py"
    p6_ok = True
    p6_details = []

    if test_client_py.exists():
        content = test_client_py.read_text(encoding="utf-8", errors="replace")
        if "graphql" in content.lower():
            p6_ok = False
            p6_details.append("GraphQL test reference in test_client.py")
    else:
        p6_ok = False
        p6_details.append("test_client.py not found")

    if test_async_client_py.exists():
        content = test_async_client_py.read_text(encoding="utf-8", errors="replace")
        if "graphql" in content.lower():
            p6_ok = False
            p6_details.append("GraphQL test reference in test_async_client.py")
    else:
        p6_ok = False
        p6_details.append("test_async_client.py not found")

    report(
        6,
        "Python SDK test suite clean of /graphql mocks and test cases",
        p6_ok,
        "; ".join(p6_details) if p6_details else "",
    )

    # ─────────────────────────────────────────────────────────────────────────
    # Phase 7: Complete deletion of legacy scripts/verify_graphql.py
    # ─────────────────────────────────────────────────────────────────────────
    legacy_verify = PROJECT_ROOT / "scripts" / "verify_graphql.py"
    p7_ok = not legacy_verify.exists()
    report(
        7,
        "Legacy scripts/verify_graphql.py test suite completely deleted",
        p7_ok,
        f"{legacy_verify.relative_to(PROJECT_ROOT)} still exists" if not p7_ok else "",
    )

    # ─────────────────────────────────────────────────────────────────────────
    # Phase 8: System documentation & READMEs cleanliness
    # ─────────────────────────────────────────────────────────────────────────
    p8_ok = True
    p8_details = []
    docs_to_check = [
        PROJECT_ROOT / "README.md",
        PROJECT_ROOT / "rust" / "api_server" / "README.md",
    ]
    for doc in docs_to_check:
        if doc.exists():
            content = doc.read_text(encoding="utf-8", errors="replace")
            matches = [line.strip() for line in content.splitlines() if "graphql" in line.lower()]
            if matches:
                p8_ok = False
                p8_details.append(f"{doc.relative_to(PROJECT_ROOT)} has active references: {matches[:2]}")
        else:
            p8_ok = False
            p8_details.append(f"Missing doc file: {doc.relative_to(PROJECT_ROOT)}")

    report(
        8,
        "System documentation & READMEs clean of GraphQL references",
        p8_ok,
        "; ".join(p8_details) if p8_details else "",
    )

    # ─────────────────────────────────────────────────────────────────────────
    # Phase 9: OpenAPI 3.0 specification & REST API routing surface integrity
    # ─────────────────────────────────────────────────────────────────────────
    p9_ok = True
    p9_details = []
    openapi_rs = rust_dir / "api_server" / "src" / "openapi.rs"
    if openapi_rs.exists():
        content = openapi_rs.read_text(encoding="utf-8", errors="replace")
        if "graphql" in content.lower():
            p9_ok = False
            p9_details.append("GraphQL reference in openapi.rs")
        if "ApiDoc" not in content:
            p9_ok = False
            p9_details.append("ApiDoc missing from openapi.rs")
    else:
        p9_ok = False
        p9_details.append("openapi.rs not found")

    report(
        9,
        "OpenAPI 3.0 specification and REST routing surface integrity intact",
        p9_ok,
        "; ".join(p9_details) if p9_details else "",
    )

    # ─────────────────────────────────────────────────────────────────────────
    # Phase 10: Codebase wide audit confirming zero active GraphQL imports/code
    # ─────────────────────────────────────────────────────────────────────────
    p10_ok = True
    p10_details = []
    for ext in ["*.rs", "*.toml", "*.yaml"]:
        for f in PROJECT_ROOT.rglob(ext):
            if any(ignored in f.parts for ignored in ["target", "venv", ".git", ".system_generated"]):
                continue
            try:
                content = f.read_text(encoding="utf-8", errors="replace")
                for line_no, line in enumerate(content.splitlines(), start=1):
                    clean = line.strip()
                    if clean.startswith("//") or clean.startswith("#") or clean.startswith("/*"):
                        continue
                    if "graphql" in clean.lower():
                        p10_ok = False
                        p10_details.append(f"{f.relative_to(PROJECT_ROOT)}:{line_no} -> {clean}")
            except Exception as e:
                p10_ok = False
                p10_details.append(f"Error scanning {f}: {e}")

    report(
        10,
        "Codebase wide scan confirms zero active GraphQL code tokens or imports",
        p10_ok,
        "; ".join(p10_details[:3]) if p10_details else "",
    )

    # ─────────────────────────────────────────────────────────────────────────
    # Summary
    # ─────────────────────────────────────────────────────────────────────────
    print("\n" + "=" * 80)
    print(f" Suite #231 Results: {passed}/{total} Phases Passed (100% Target)")
    print("=" * 80)
    if passed == total:
        print("  ALL 10 VERIFICATION PHASES PASSED CLEANLY! [OK]\n")
        return 0
    else:
        print(f"  FAILED: {failed} phase(s) encountered issues [X]\n")
        return 1


if __name__ == "__main__":
    sys.exit(run_all_phases())
