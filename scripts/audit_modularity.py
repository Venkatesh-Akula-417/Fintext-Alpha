#!/usr/bin/env python3
"""
FinText Alpha Vectorizer — Rust Workspace Modularity & Dependency Graph Auditor
=============================================================================
Analyzes the 10-crate Cargo workspace dependency graph to detect:
1. Cyclic dependencies (via 3-color DFS)
2. Excessive coupling & single-responsibility violations
3. Dependency bloat across external crates
4. Orphan / unreferenced modules

Outputs:
- ASCII / Mermaid dependency graph
- Per-crate coupling metrics (In-degree, Out-degree, Instability I, External deps)
- Overall Modularity Health Score (0-100)
- Detailed JSON report: `scripts/modularity_report.json`
"""

import sys
import json
import tomllib
from pathlib import Path
from dataclasses import dataclass, field, asdict
from typing import Dict, List, Set, Tuple, Optional
from datetime import datetime, timezone

if sys.platform == "win32":
    try:
        sys.stdout.reconfigure(encoding="utf-8")
        sys.stderr.reconfigure(encoding="utf-8")
    except Exception:
        pass


# ─────────────────────────────────────────────────────────────────────────────
# Data Structures & Models
# ─────────────────────────────────────────────────────────────────────────────

@dataclass
class CrateInfo:
    directory: str
    package_name: str
    version: str
    description: str
    is_binary: bool
    is_library: bool
    workspace_deps: List[str] = field(default_factory=list)
    external_deps: List[str] = field(default_factory=list)
    dev_deps: List[str] = field(default_factory=list)
    in_degree: int = 0
    out_degree: int = 0
    instability: float = 0.0
    warnings: List[str] = field(default_factory=list)


@dataclass
class ModularityReport:
    timestamp: str
    workspace_root: str
    total_crates: int
    total_workspace_edges: int
    total_external_dependencies: int
    has_cycles: bool
    cycles: List[List[str]]
    modularity_score: int
    health_grade: str
    crates: Dict[str, dict]
    recommendations: List[str]
    mermaid_diagram: str


# ─────────────────────────────────────────────────────────────────────────────
# Workspace Parser
# ─────────────────────────────────────────────────────────────────────────────

def parse_workspace(rust_dir: Path) -> Dict[str, CrateInfo]:
    """Parse all crates defined in rust/Cargo.toml workspace."""
    workspace_cargo = rust_dir / "Cargo.toml"
    if not workspace_cargo.exists():
        raise FileNotFoundError(f"Root workspace manifest not found at: {workspace_cargo}")

    with open(workspace_cargo, "rb") as f:
        ws_data = tomllib.load(f)

    members = ws_data.get("workspace", {}).get("members", [])
    if not members:
        raise ValueError(f"No workspace members found in {workspace_cargo}")

    crates: Dict[str, CrateInfo] = {}
    dir_to_pkg: Dict[str, str] = {}
    pkg_to_dir: Dict[str, str] = {}

    # First pass: collect package names and directory mappings
    for member in members:
        crate_dir = rust_dir / member
        cargo_file = crate_dir / "Cargo.toml"
        if not cargo_file.exists():
            continue

        with open(cargo_file, "rb") as f:
            cdata = tomllib.load(f)

        pkg_name = cdata.get("package", {}).get("name", member)
        dir_to_pkg[member] = pkg_name
        pkg_to_dir[pkg_name] = member

    # Second pass: extract dependencies and metadata
    for member in members:
        crate_dir = rust_dir / member
        cargo_file = crate_dir / "Cargo.toml"
        if not cargo_file.exists():
            continue

        with open(cargo_file, "rb") as f:
            cdata = tomllib.load(f)

        pkg_info = cdata.get("package", {})
        pkg_name = pkg_info.get("name", member)
        version = pkg_info.get("version", "0.1.0")
        description = pkg_info.get("description", "")

        is_binary = "bin" in cdata or (crate_dir / "src" / "main.rs").exists()
        is_library = "lib" in cdata or (crate_dir / "src" / "lib.rs").exists()

        workspace_deps: List[str] = []
        external_deps: List[str] = []
        dev_deps: List[str] = []

        # Parse [dependencies]
        deps = cdata.get("dependencies", {})
        for dep_name, dep_spec in deps.items():
            if isinstance(dep_spec, dict) and "path" in dep_spec:
                # Path dependency -> internal workspace crate
                path_val = dep_spec["path"]
                # Resolve target crate name
                target_dir = (crate_dir / path_val).resolve().name
                target_pkg = dir_to_pkg.get(target_dir, dep_name)
                workspace_deps.append(target_pkg)
            elif dep_name in pkg_to_dir:
                workspace_deps.append(dep_name)
            else:
                external_deps.append(dep_name)

        # Parse [dev-dependencies]
        d_deps = cdata.get("dev-dependencies", {})
        for dname in d_deps:
            dev_deps.append(dname)

        crates[pkg_name] = CrateInfo(
            directory=member,
            package_name=pkg_name,
            version=version,
            description=description,
            is_binary=is_binary,
            is_library=is_library,
            workspace_deps=sorted(list(set(workspace_deps))),
            external_deps=sorted(list(set(external_deps))),
            dev_deps=sorted(list(set(dev_deps))),
        )

    # Third pass: compute in-degree and out-degree
    for pkg_name, cinfo in crates.items():
        cinfo.out_degree = len(cinfo.workspace_deps)

    for pkg_name, cinfo in crates.items():
        for target_pkg in cinfo.workspace_deps:
            if target_pkg in crates:
                crates[target_pkg].in_degree += 1

    # Fourth pass: calculate instability I = Out / (In + Out)
    for pkg_name, cinfo in crates.items():
        total_deg = cinfo.in_degree + cinfo.out_degree
        cinfo.instability = round(cinfo.out_degree / total_deg, 3) if total_deg > 0 else 0.0

    return crates


# ─────────────────────────────────────────────────────────────────────────────
# Cycle Detection (3-Color DFS)
# ─────────────────────────────────────────────────────────────────────────────

def detect_cycles(crates: Dict[str, CrateInfo]) -> List[List[str]]:
    """Detect cycles in the directed dependency graph using 3-color DFS."""
    # 0 = UNVISITED, 1 = VISITING (in recursion stack), 2 = VISITED
    state: Dict[str, int] = {pkg: 0 for pkg in crates}
    parent: Dict[str, Optional[str]] = {pkg: None for pkg in crates}
    cycles: List[List[str]] = []
    path_stack: List[str] = []

    def dfs(u: str):
        state[u] = 1
        path_stack.append(u)

        for v in crates[u].workspace_deps:
            if v not in crates:
                continue
            if state[v] == 1:
                # Cycle found! Extract cycle path from path_stack
                cycle_start_idx = path_stack.index(v)
                cycle_path = path_stack[cycle_start_idx:] + [v]
                cycles.append(cycle_path)
            elif state[v] == 0:
                parent[v] = u
                dfs(v)

        path_stack.pop()
        state[u] = 2

    for pkg in crates:
        if state[pkg] == 0:
            dfs(pkg)

    return cycles


# ─────────────────────────────────────────────────────────────────────────────
# Modularity Evaluation & Warnings
# ─────────────────────────────────────────────────────────────────────────────

def evaluate_modularity(crates: Dict[str, CrateInfo], cycles: List[List[str]]) -> Tuple[int, str, List[str]]:
    """Evaluates modularity score (0-100), health grade, and issues."""
    score = 100
    recommendations: List[str] = []

    # 1. Cycle penalties (Critical)
    if cycles:
        penalty = len(cycles) * 40
        score -= penalty
        for cycle in cycles:
            cycle_str = " -> ".join(cycle)
            recommendations.append(f"CRITICAL: Break circular dependency loop: {cycle_str}")

    # 2. Check per-crate coupling & bloat
    for pkg, cinfo in crates.items():
        # High out-degree (> 4 workspace dependencies)
        if cinfo.out_degree > 4:
            cinfo.warnings.append(f"High workspace coupling ({cinfo.out_degree} deps). May violate Single Responsibility Principle.")
            score -= 6
            recommendations.append(f"Review `{pkg}`: High out-degree ({cinfo.out_degree}). Consider abstracting shared traits or sub-crates.")

        # High external dependencies (> 30 external crates)
        if len(cinfo.external_deps) > 30:
            cinfo.warnings.append(f"High external dependency count ({len(cinfo.external_deps)} crates). Potential dependency bloat.")
            score -= 4
            recommendations.append(f"Audit `{pkg}`: Contains {len(cinfo.external_deps)} external dependencies. Prune unused features.")

        # Orphan library crate (in-degree == 0, out-degree == 0, not a binary entrypoint)
        if cinfo.in_degree == 0 and cinfo.out_degree == 0 and not cinfo.is_binary:
            cinfo.warnings.append("Orphan library crate (0 incoming and 0 outgoing workspace dependencies).")
            score -= 5
            recommendations.append(f"Verify `{pkg}`: Standalone library with 0 workspace references. Ensure it is actively utilized or expose CLI/PyO3 bindings.")

    score = max(0, min(100, score))

    if score >= 90:
        grade = "A+ (Excellent Modularity & Clean DAG)"
    elif score >= 80:
        grade = "A (Strong Separation of Concerns)"
    elif score >= 70:
        grade = "B (Good Modularity with Minor Coupling)"
    elif score >= 50:
        grade = "C (Moderate Coupling / Refactoring Recommended)"
    else:
        grade = "F (High Cyclic Complexity / Refactoring Required)"

    if not recommendations:
        recommendations.append("Architecture is cleanly modularized. All workspace dependencies form a strict Directed Acyclic Graph (DAG).")

    return score, grade, recommendations


# ─────────────────────────────────────────────────────────────────────────────
# Mermaid & ASCII Visualization
# ─────────────────────────────────────────────────────────────────────────────

def generate_mermaid_diagram(crates: Dict[str, CrateInfo]) -> str:
    """Generate Mermaid flowchart diagram for the workspace dependency graph."""
    lines = ["flowchart TD"]
    
    # Define subgraphs for architectural tiers
    lines.append("    subgraph LeafUtilities [Tier 1: High-Speed Pure Leaf Utilities]")
    for pkg, cinfo in sorted(crates.items()):
        if cinfo.out_degree == 0 and not cinfo.is_binary:
            lines.append(f'        {pkg}["{pkg}<br/>(In: {cinfo.in_degree}, Ext: {len(cinfo.external_deps)})"]')
    lines.append("    end")

    lines.append("    subgraph CoreEngines [Tier 2: Core Processing & Quantitative Engines]")
    for pkg, cinfo in sorted(crates.items()):
        if 0 < cinfo.out_degree <= 4 and pkg != "fintext_api_server":
            lines.append(f'        {pkg}["{pkg}<br/>(In: {cinfo.in_degree}, Out: {cinfo.out_degree})"]')
    lines.append("    end")

    lines.append("    subgraph GatewaysAndDaemons [Tier 3: Gateway, Workers & Daemons]")
    for pkg, cinfo in sorted(crates.items()):
        if cinfo.out_degree > 4 or (cinfo.is_binary and cinfo.out_degree == 0) or pkg == "fintext_api_server":
            if pkg not in [p for p, c in crates.items() if c.out_degree == 0 and not c.is_binary]:
                lines.append(f'        {pkg}["{pkg}<br/>(In: {cinfo.in_degree}, Out: {cinfo.out_degree})"]')
    lines.append("    end")

    # Add edges
    lines.append("")
    lines.append("    %% Workspace Dependency Edges")
    for pkg, cinfo in sorted(crates.items()):
        for dep in sorted(cinfo.workspace_deps):
            lines.append(f"    {pkg} -->|depends on| {dep}")

    return "\n".join(lines)


# ─────────────────────────────────────────────────────────────────────────────
# Main CLI & Report Generation
# ─────────────────────────────────────────────────────────────────────────────

def main() -> int:
    workspace_root = Path(__file__).resolve().parent.parent
    rust_dir = workspace_root / "rust"

    print("=" * 80)
    print(" FinText Alpha Vectorizer — Rust Workspace Modularity & Dependency Auditor")
    print("=" * 80)
    print(f" Workspace Root : {rust_dir}")
    print(f" Timestamp      : {datetime.now(timezone.utc).isoformat()}")
    print("-" * 80)

    try:
        crates = parse_workspace(rust_dir)
    except Exception as e:
        print(f"❌ Error parsing workspace: {e}", file=sys.stderr)
        return 1

    cycles = detect_cycles(crates)
    score, grade, recommendations = evaluate_modularity(crates, cycles)

    total_edges = sum(c.out_degree for c in crates.values())
    total_ext = sum(len(c.external_deps) for c in crates.values())

    # 1. Print Summary Header
    print(f"\n📊 Summary Metrics:")
    print(f" • Total Workspace Crates    : {len(crates)}")
    print(f" • Total Workspace Edges     : {total_edges}")
    print(f" • Total External Deps       : {total_ext}")
    print(f" • Cyclic Dependencies       : {'❌ DETECTED (' + str(len(cycles)) + ')' if cycles else '✅ None (Strict DAG)'}")
    print(f" • Modularity Health Score   : {score} / 100 [{grade}]")
    print("-" * 80)

    # 2. Print Per-Crate Metrics Table
    print("\n📦 Crate Metrics Table:")
    header = f"{'Crate Package':<30} | {'Type':<10} | {'In-Deg':<6} | {'Out-Deg':<7} | {'Ext Deps':<8} | {'Instability':<11} | {'Status'}"
    print(header)
    print("-" * len(header))

    for pkg, cinfo in sorted(crates.items(), key=lambda x: (x[1].out_degree, x[1].in_degree)):
        ctype = []
        if cinfo.is_library:
            ctype.append("lib")
        if cinfo.is_binary:
            ctype.append("bin")
        type_str = "/".join(ctype) if ctype else "other"

        status_str = "⚠️ Warnings" if cinfo.warnings else "✅ Clean"
        print(f"{pkg:<30} | {type_str:<10} | {cinfo.in_degree:<6} | {cinfo.out_degree:<7} | {len(cinfo.external_deps):<8} | {cinfo.instability:<11.2f} | {status_str}")

    # 3. Print Warnings & Recommendations
    print("\n💡 Architectural Analysis & Recommendations:")
    for rec in recommendations:
        print(f" • {rec}")

    if any(c.warnings for c in crates.values()):
        print("\n⚠️ Detailed Crate Warnings:")
        for pkg, c in sorted(crates.items()):
            for w in c.warnings:
                print(f" • [{pkg}]: {w}")

    # 4. Generate Mermaid Diagram
    mermaid = generate_mermaid_diagram(crates)
    print("\n🗺️ Mermaid Dependency Graph:")
    print("```mermaid")
    print(mermaid)
    print("```")

    # 5. Save JSON Report
    report_dict = {
        "timestamp": datetime.now(timezone.utc).isoformat(),
        "workspace_root": str(rust_dir),
        "total_crates": len(crates),
        "total_workspace_edges": total_edges,
        "total_external_dependencies": total_ext,
        "has_cycles": len(cycles) > 0,
        "cycles": cycles,
        "modularity_score": score,
        "health_grade": grade,
        "crates": {pkg: asdict(c) for pkg, c in crates.items()},
        "recommendations": recommendations,
        "mermaid_diagram": mermaid,
    }

    report_path = workspace_root / "scripts" / "modularity_report.json"
    with open(report_path, "w", encoding="utf-8") as f:
        json.dump(report_dict, f, indent=2)

    print(f"\n📄 Saved JSON report to: {report_path}")
    print("=" * 80)
    print(" Modularity Audit Completed Successfully [OK]")
    print("=" * 80)

    return 0 if not cycles else 2


if __name__ == "__main__":
    sys.exit(main())
