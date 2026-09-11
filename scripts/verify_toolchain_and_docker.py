#!/usr/bin/env python3
"""
===============================================================================
FinText-Alpha-Vectorizer — Suite #254: Toolchain & Docker Base Image Modernization
===============================================================================
Verifies:
  1.  Dockerfile Base Images: Debian Bookworm builder & runtime-base (bookworm-slim)
  2.  Zero Legacy 'bullseye' occurrences across repository files
  3.  Rust Toolchain Configuration: rust-toolchain.toml present with components
  4.  Cargo Workspace MSRV: rust/Cargo.toml defines rust-version = "1.80" & edition = "2021"
  5.  CI/CD Workflow Alignment: .github/workflows/ci.yml pins Rust toolchain
  6.  Native Rust Workspace Compilation Check: cargo check --workspace succeeds
  7.  Native Unit Test Verification: Core parser crates pass unit tests
  8.  Dockerfile Multi-Stage Targets: All 5 production stages defined and valid
  9.  Container Security: Non-root 'appuser' (UID 1000) and strict permissions
  10. System Documentation: README.md and docs/current_architecture.md updated
===============================================================================
"""

import os
from pathlib import Path
import subprocess
import sys

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
        msg = f"  ❌ Phase {phase:2d} │ {name}"
        if detail:
            msg += f" — {detail}"
        print(msg)


def main():
    global passed, failed
    print("=" * 80)
    print("  FinText-Alpha-Vectorizer — Toolchain & Docker Modernization Suite (#254)")
    print("=" * 80)

    # -----------------------------------------------------------------
    # Phase 1: Dockerfile Base Images (Debian Bookworm)
    # -----------------------------------------------------------------
    dockerfile = PROJECT_ROOT / "Dockerfile"
    df_content = dockerfile.read_text(encoding="utf-8", errors="replace") if dockerfile.exists() else ""
    ok1 = (
        "bookworm" in df_content
        and "debian:bookworm-slim" in df_content
        and "FROM rust:" in df_content
    )
    report(1, "Dockerfile builder & runtime base images use Debian Bookworm", ok1)

    # -----------------------------------------------------------------
    # Phase 2: Zero Legacy 'bullseye' occurrences
    # -----------------------------------------------------------------
    bullseye_found = []
    for ext in ["*.rs", "*.toml", "*.yaml", "*.yml", "Dockerfile", "*.md"]:
        for f in PROJECT_ROOT.glob(f"**/{ext}"):
            if "venv" in str(f) or "target" in str(f) or ".git" in str(f) or "_archive" in str(f):
                continue
            try:
                txt = f.read_text(encoding="utf-8", errors="replace")
                if "bullseye" in txt.lower():
                    bullseye_found.append(str(f.relative_to(PROJECT_ROOT)))
            except Exception:
                pass
    ok2 = len(bullseye_found) == 0
    report(2, "Zero legacy 'bullseye' occurrences across codebase", ok2, f"Found in: {bullseye_found}")

    # -----------------------------------------------------------------
    # Phase 3: Rust Toolchain Configuration (rust-toolchain.toml)
    # -----------------------------------------------------------------
    toolchain_toml = PROJECT_ROOT / "rust-toolchain.toml"
    tc_content = toolchain_toml.read_text(encoding="utf-8", errors="replace") if toolchain_toml.exists() else ""
    ok3 = (
        toolchain_toml.exists()
        and "[toolchain]" in tc_content
        and "channel" in tc_content
        and "rustfmt" in tc_content
        and "clippy" in tc_content
    )
    report(3, "rust-toolchain.toml configured with components (rustfmt, clippy)", ok3)

    # -----------------------------------------------------------------
    # Phase 4: Cargo Workspace MSRV (rust-version = "1.80")
    # -----------------------------------------------------------------
    cargo_toml = PROJECT_ROOT / "rust" / "Cargo.toml"
    cg_content = cargo_toml.read_text(encoding="utf-8", errors="replace") if cargo_toml.exists() else ""
    ok4 = (
        cargo_toml.exists()
        and "[workspace.package]" in cg_content
        and 'rust-version = "1.80"' in cg_content
        and 'edition = "2021"' in cg_content
    )
    report(4, "Cargo.toml defines workspace MSRV (rust-version = 1.80)", ok4)

    # -----------------------------------------------------------------
    # Phase 5: CI/CD Workflow Alignment (.github/workflows/ci.yml)
    # -----------------------------------------------------------------
    ci_yml = PROJECT_ROOT / ".github" / "workflows" / "ci.yml"
    ci_content = ci_yml.read_text(encoding="utf-8", errors="replace") if ci_yml.exists() else ""
    ok5 = (
        ci_yml.exists()
        and "dtolnay/rust-toolchain" in ci_content
        and "rustfmt, clippy" in ci_content
    )
    report(5, "GitHub Actions CI pipeline configured with matching toolchain", ok5)

    # -----------------------------------------------------------------
    # Phase 6: Native Rust Workspace Compilation Check
    # -----------------------------------------------------------------
    env = os.environ.copy()
    env["LIBCLANG_PATH"] = str(PROJECT_ROOT / "venv" / "Lib" / "site-packages" / "clang" / "native")
    env["CMAKE"] = r"C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin\cmake.exe"
    
    res_check = subprocess.run(
        ["cargo", "check", "--workspace", "--manifest-path", "rust/Cargo.toml"],
        cwd=str(PROJECT_ROOT),
        capture_output=True,
        text=True,
        env=env,
        shell=True,
    )
    ok6 = res_check.returncode == 0
    report(6, "Native Rust workspace compilation check succeeds (0 errors)", ok6, f"Stderr: {res_check.stderr[-200:] if not ok6 else ''}")

    # -----------------------------------------------------------------
    # Phase 7: Native Unit Test Verification
    # -----------------------------------------------------------------
    res_test = subprocess.run(
        ["cargo", "test", "-p", "fintext_html_sanitizer", "-p", "fintext_ticker_extractor", "--manifest-path", "rust/Cargo.toml"],
        cwd=str(PROJECT_ROOT),
        capture_output=True,
        text=True,
        env=env,
        shell=True,
    )
    ok7 = res_test.returncode == 0
    report(7, "Core Rust parsing crates pass unit tests", ok7)

    # -----------------------------------------------------------------
    # Phase 8: Dockerfile Multi-Stage Target Integrity
    # -----------------------------------------------------------------
    required_targets = ["builder", "runtime-base", "ingestion", "api", "spillover", "dead_letter_worker", "anomaly_detector"]
    found_targets = [t for t in required_targets if f"AS {t}" in df_content]
    ok8 = len(found_targets) == len(required_targets)
    report(8, f"Dockerfile multi-stage targets verified ({len(found_targets)}/{len(required_targets)})", ok8)

    # -----------------------------------------------------------------
    # Phase 9: Container Security & Non-Root Execution
    # -----------------------------------------------------------------
    ok9 = (
        "useradd -u 1000" in df_content
        and "USER appuser" in df_content
        and "chown -R appuser:appuser /app" in df_content
    )
    report(9, "Container security: non-root 'appuser' (UID 1000) enforced", ok9)

    # -----------------------------------------------------------------
    # Phase 10: System Documentation Consistency
    # -----------------------------------------------------------------
    readme = PROJECT_ROOT / "README.md"
    readme_content = readme.read_text(encoding="utf-8", errors="replace") if readme.exists() else ""
    arch = PROJECT_ROOT / "docs" / "current_architecture.md"
    arch_content = arch.read_text(encoding="utf-8", errors="replace") if arch.exists() else ""
    
    ok10 = (
        "rust-1.80+" in readme_content.lower()
        and "bookworm" in arch_content.lower()
        and "1.80" in arch_content
    )
    report(10, "Documentation updated with Rust 1.80+ and Debian Bookworm specs", ok10)

    print("\n" + "=" * 80)
    print(f"  Summary: {passed}/{total} Passed | {failed} Failed")
    print("=" * 80)

    if passed == total:
        print("  ✅ ALL TOOLCHAIN & DOCKER MODERNIZATION CHECKS PASSED CLEANLY!\n")
        return 0
    else:
        print("  ❌ SOME TOOLCHAIN & DOCKER MODERNIZATION CHECKS FAILED!\n")
        return 1


if __name__ == "__main__":
    sys.exit(main())
