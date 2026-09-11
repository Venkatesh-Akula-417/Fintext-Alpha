#!/usr/bin/env python3
"""
===============================================================================
FinText-Alpha-Vectorizer — Suite #253: Single-Region Kubernetes Infrastructure
===============================================================================
Verifies:
  1.  Zero Multi-Region Overlay Folders (No k8s/overlays/, k8s/regions/, etc.)
  2.  Zero Cross-Zone Topology Spread Constraints (No multi-zone scheduling constraints)
  3.  Zero Regional Node Affinities or Multi-Region Selectors
  4.  Single-Region Cloud Storage & Backup Alignment (us-east-1 for Velero & S3)
  5.  Kubernetes YAML Syntax & Document Validity across all k8s manifests
  6.  Kustomize Hierarchy & Sub-Package Reference Integrity
  7.  High Availability & Single-Cluster Pod Disruption Budgets (PDB)
  8.  Autoscaling (HPA/VPA) & Progressive Delivery (Flagger Canary) Preservation
  9.  Full Observability Stack Preservation (Prometheus, Alertmanager, Grafana, Loki, OTel)
  10. System Documentation Consistency (Single-region bootstrap architecture documented)
===============================================================================
"""

import os
from pathlib import Path
import sys
import yaml

if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
if hasattr(sys.stderr, "reconfigure"):
    sys.stderr.reconfigure(encoding="utf-8", errors="replace")

PROJECT_ROOT = Path(__file__).resolve().parent.parent
K8S_DIR = PROJECT_ROOT / "k8s"

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
    print("  FinText-Alpha-Vectorizer — Single-Region K8s Infrastructure Suite (#253)")
    print("=" * 80)

    # -----------------------------------------------------------------
    # Phase 1: Zero Multi-Region Overlay Folders
    # -----------------------------------------------------------------
    disallowed_dirs = ["overlays", "regions", "clusters", "us-east", "us-west", "eu-west", "ap-south"]
    found_disallowed = []
    for d in disallowed_dirs:
        if (K8S_DIR / d).exists():
            found_disallowed.append(d)
    ok1 = len(found_disallowed) == 0
    report(1, "Zero multi-region overlay directories in k8s/", ok1, f"Found: {found_disallowed}")

    # -----------------------------------------------------------------
    # Phase 2: Zero Cross-Zone Topology Spread Constraints
    # -----------------------------------------------------------------
    all_k8s_files = list(K8S_DIR.rglob("*.yaml")) + list(K8S_DIR.rglob("*.yml"))
    files_with_spread = []
    for f in all_k8s_files:
        content = f.read_text(encoding="utf-8", errors="replace")
        if "topologySpreadConstraints" in content or "topology.kubernetes.io/zone" in content:
            files_with_spread.append(str(f.relative_to(PROJECT_ROOT)))
    ok2 = len(files_with_spread) == 0
    report(2, "Zero cross-zone topology spread constraints in manifests", ok2, f"Files: {files_with_spread}")

    # -----------------------------------------------------------------
    # Phase 3: Zero Regional Node Affinities or Multi-Region Selectors
    # -----------------------------------------------------------------
    files_with_reg_affinity = []
    for f in all_k8s_files:
        content = f.read_text(encoding="utf-8", errors="replace")
        if "topology.kubernetes.io/region" in content or "failure-domain.beta.kubernetes.io/region" in content:
            files_with_reg_affinity.append(str(f.relative_to(PROJECT_ROOT)))
    ok3 = len(files_with_reg_affinity) == 0
    report(3, "Zero regional node affinities or multi-region selectors", ok3, f"Files: {files_with_reg_affinity}")

    # -----------------------------------------------------------------
    # Phase 4: Single-Region Cloud Storage & Backup Alignment
    # -----------------------------------------------------------------
    velero_loc = K8S_DIR / "velero-storage-location.yaml"
    ok4 = False
    regions = set()
    if velero_loc.exists():
        content = velero_loc.read_text(encoding="utf-8", errors="replace")
        docs = list(yaml.safe_load_all(content))
        for doc in docs:
            if isinstance(doc, dict):
                cfg = doc.get("spec", {}).get("config", {})
                if "region" in cfg:
                    regions.add(cfg["region"])
        ok4 = regions == {"us-east-1"}
    report(4, "Single-region cloud storage alignment (us-east-1)", ok4, f"Regions: {regions if velero_loc.exists() else 'N/A'}")

    # -----------------------------------------------------------------
    # Phase 5: Kubernetes YAML Syntax & Document Validity
    # -----------------------------------------------------------------
    yaml_errors = []
    total_docs = 0
    for f in all_k8s_files:
        try:
            content = f.read_text(encoding="utf-8", errors="replace")
            docs = list(yaml.safe_load_all(content))
            total_docs += len(docs)
        except Exception as e:
            yaml_errors.append(f"{f.name}: {e}")
    ok5 = len(yaml_errors) == 0 and total_docs > 20
    report(5, f"All {len(all_k8s_files)} YAML files valid ({total_docs} Kubernetes documents)", ok5, f"Errors: {yaml_errors}")

    # -----------------------------------------------------------------
    # Phase 6: Kustomize Hierarchy & Sub-Package Reference Integrity
    # -----------------------------------------------------------------
    root_kustomization = K8S_DIR / "kustomization.yaml"
    ok6 = False
    missing_refs = []
    if root_kustomization.exists():
        kust_data = yaml.safe_load(root_kustomization.read_text(encoding="utf-8", errors="replace"))
        resources = kust_data.get("resources", [])
        for r in resources:
            target_path = K8S_DIR / r
            if not target_path.exists():
                missing_refs.append(str(r))
            elif target_path.is_dir():
                sub_kust = target_path / "kustomization.yaml"
                if not sub_kust.exists():
                    missing_refs.append(f"{r}/kustomization.yaml")
                else:
                    sub_data = yaml.safe_load(sub_kust.read_text(encoding="utf-8", errors="replace"))
                    for sub_r in sub_data.get("resources", []):
                        if not (target_path / sub_r).exists():
                            missing_refs.append(f"{r}/{sub_r}")
        ok6 = len(missing_refs) == 0 and len(resources) >= 10
    report(6, "Kustomize root & sub-package reference integrity", ok6, f"Missing: {missing_refs}")

    # -----------------------------------------------------------------
    # Phase 7: High Availability & Single-Cluster Pod Disruption Budgets
    # -----------------------------------------------------------------
    expected_pdbs = [
        "pdb-api.yaml",
        "pdb-ingestion.yaml",
        "pdb-questdb.yaml",
        "pdb-kafka.yaml",
        "postgres-statefulset.yaml",
        "dead-letter-worker.yaml",
    ]
    pdb_found = 0
    for pdb_file in expected_pdbs:
        p = K8S_DIR / pdb_file
        if p.exists() and "PodDisruptionBudget" in p.read_text(encoding="utf-8", errors="replace"):
            pdb_found += 1
    ok7 = pdb_found == len(expected_pdbs)
    report(7, f"Single-cluster Pod Disruption Budgets verified ({pdb_found}/{len(expected_pdbs)})", ok7)

    # -----------------------------------------------------------------
    # Phase 8: Autoscaling & Progressive Delivery Preservation
    # -----------------------------------------------------------------
    has_hpa_api = (K8S_DIR / "hpa-api.yaml").exists()
    has_hpa_ingestion = (K8S_DIR / "hpa-ingestion.yaml").exists()
    has_vpa = (K8S_DIR / "vpa-api.yaml").exists()
    has_canary = (K8S_DIR / "deployments" / "flagger-canary-api.yaml").exists()
    ok8 = has_hpa_api and has_hpa_ingestion and has_vpa and has_canary
    report(8, "Autoscaling (HPA/VPA) & Progressive Delivery (Flagger) preserved", ok8)

    # -----------------------------------------------------------------
    # Phase 9: Full Observability Stack Preservation
    # -----------------------------------------------------------------
    obs_dir = K8S_DIR / "observability"
    expected_obs = [
        "prometheus.yaml",
        "prometheus-alerts.yaml",
        "alertmanager.yaml",
        "grafana.yaml",
        "loki.yaml",
        "opentelemetry-collector.yaml",
        "anomaly-detector.yaml",
    ]
    obs_found = sum(1 for o in expected_obs if (obs_dir / o).exists())
    ok9 = obs_found == len(expected_obs)
    report(9, f"Observability stack components verified ({obs_found}/{len(expected_obs)})", ok9)

    # -----------------------------------------------------------------
    # Phase 10: System Documentation Consistency
    # -----------------------------------------------------------------
    arch_doc = PROJECT_ROOT / "docs" / "current_architecture.md"
    arch_content = arch_doc.read_text(encoding="utf-8", errors="replace") if arch_doc.exists() else ""
    ok10 = "single-region, single-cluster deployment" in arch_content.lower() and "us-east-1" in arch_content
    report(10, "Documentation consistency (Single-region bootstrap documented)", ok10)

    print("\n" + "=" * 80)
    print(f"  Summary: {passed}/{total} Passed | {failed} Failed")
    print("=" * 80)

    if passed == total:
        print("  ✅ ALL SINGLE-REGION KUBERNETES CHECKS PASSED CLEANLY!\n")
        return 0
    else:
        print("  ❌ SOME SINGLE-REGION KUBERNETES CHECKS FAILED!\n")
        return 1


if __name__ == "__main__":
    sys.exit(main())
