#!/usr/bin/env python3
"""
===============================================================================
FinText-Alpha-Vectorizer — Model Drift Monitoring Harness
Suite: FinBERT Continuous Performance Drift & Calibration Degradation Watchdog
===============================================================================

Monitors:
  - Macro F1 drop against committed baseline
  - Expected Calibration Error (ECE) increase against committed baseline
  - P95 single-sample inference latency degradation
  - Multi-class PRF and accuracy stability

Exit Codes:
  0 = No drift OR warn-level drift detected (nominal operation)
  1 = Critical drift threshold breach detected (alert triggered)
  2 = Environment error (missing baseline, missing dataset, invalid JSON)
  3 = SKIP (model artifacts not present)
===============================================================================
"""

import argparse
from datetime import datetime, timezone
import json
from pathlib import Path
import sys

# Ensure UTF-8 stdout/stderr encoding
if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
if hasattr(sys.stderr, "reconfigure"):
    sys.stderr.reconfigure(encoding="utf-8", errors="replace")

PROJECT_ROOT = Path(__file__).resolve().parent.parent
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))

from scripts.model_drift.baseline import load_baseline, save_baseline
from scripts.model_drift.drift import DriftThresholds, compute_drift
from scripts.model_drift.report import (
    generate_json_report,
    generate_markdown_report,
)
from scripts.model_validation.report import get_git_commit_sha


def parse_args():
    parser = argparse.ArgumentParser(
        description="FinText Alpha Vectorizer Model Drift Monitoring Harness"
    )
    parser.add_argument(
        "--baseline",
        default="data/model-drift/baseline.json",
        help="Path to baseline JSON snapshot (default: data/model-drift/baseline.json).",
    )
    parser.add_argument(
        "--dataset",
        default="config/model_validation_dataset.json",
        help="Path to evaluation dataset JSON (default: config/model_validation_dataset.json).",
    )
    parser.add_argument(
        "--model-dir",
        default="models/finbert-finetuned",
        help="Directory containing ONNX model and tokenizer assets.",
    )
    parser.add_argument(
        "--output-dir",
        default="data/model-drift",
        help="Directory to save timestamped JSON drift reports.",
    )
    parser.add_argument(
        "--report-path",
        default="docs/MODEL_DRIFT_REPORT.md",
        help="Path to generate human-readable Markdown drift report.",
    )
    parser.add_argument(
        "--init-baseline",
        action="store_true",
        help="Execute inference and initialize or update the baseline JSON snapshot, then exit 0.",
    )
    parser.add_argument(
        "--f1-drop-warn",
        type=float,
        default=0.03,
        help="Warning threshold for Macro F1 drop vs baseline (default: 0.03).",
    )
    parser.add_argument(
        "--f1-drop-critical",
        type=float,
        default=0.07,
        help="Critical threshold for Macro F1 drop vs baseline (default: 0.07).",
    )
    parser.add_argument(
        "--ece-increase-warn",
        type=float,
        default=0.02,
        help="Warning threshold for ECE increase vs baseline (default: 0.02).",
    )
    parser.add_argument(
        "--ece-increase-critical",
        type=float,
        default=0.05,
        help="Critical threshold for ECE increase vs baseline (default: 0.05).",
    )
    parser.add_argument(
        "--latency-p95-pct-warn",
        type=float,
        default=30.0,
        help="Warning threshold for P95 latency percentage increase vs baseline (default: 30.0%%).",
    )
    parser.add_argument(
        "--latency-p95-pct-critical",
        type=float,
        default=60.0,
        help="Critical threshold for P95 latency percentage increase vs baseline (default: 60.0%%).",
    )
    return parser.parse_args()


def load_dataset(dataset_path: Path):
    """Load and validate the evaluation dataset."""
    if not dataset_path.is_file():
        print(
            f"❌ Environment Error: Dataset file not found at '{dataset_path}'",
            file=sys.stderr,
        )
        sys.exit(2)

    try:
        with open(dataset_path, "r", encoding="utf-8") as f:
            data = json.load(f)
    except Exception as exc:
        print(
            f"❌ Environment Error: Failed to parse JSON dataset at '{dataset_path}': {exc}",
            file=sys.stderr,
        )
        sys.exit(2)

    if isinstance(data, list):
        samples = data
    elif isinstance(data, dict) and "samples" in data:
        samples = data["samples"]
    else:
        print(
            f"❌ Environment Error: Unexpected dataset structure in '{dataset_path}'. Expected list or object with 'samples' array.",
            file=sys.stderr,
        )
        sys.exit(2)

    if not samples:
        print(
            f"❌ Environment Error: Dataset in '{dataset_path}' contains zero samples.",
            file=sys.stderr,
        )
        sys.exit(2)

    return samples


def main():
    args = parse_args()
    run_started_utc = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")

    dataset_path = Path(args.dataset)
    model_dir = Path(args.model_dir)
    baseline_path = Path(args.baseline)
    output_dir = Path(args.output_dir)
    report_path = Path(args.report_path)

    thresholds_config = {
        "f1_macro_drop_warn": args.f1_drop_warn,
        "f1_macro_drop_critical": args.f1_drop_critical,
        "ece_increase_warn": args.ece_increase_warn,
        "ece_increase_critical": args.ece_increase_critical,
        "latency_p95_increase_pct_warn": args.latency_p95_pct_warn,
        "latency_p95_increase_pct_critical": args.latency_p95_pct_critical,
    }

    reproduction_cmd = (
        f"python scripts/model_drift_monitor.py "
        f"--dataset {args.dataset} "
        f"--model-dir {args.model_dir} "
        f"--baseline {args.baseline} "
        f"--output-dir {args.output_dir} "
        f"--report-path {args.report_path} "
        f"--f1-drop-warn {args.f1_drop_warn} "
        f"--f1-drop-critical {args.f1_drop_critical} "
        f"--ece-increase-warn {args.ece_increase_warn} "
        f"--ece-increase-critical {args.ece_increase_critical} "
        f"--latency-p95-pct-warn {args.latency_p95_pct_warn} "
        f"--latency-p95-pct-critical {args.latency_p95_pct_critical}"
    )

    print("=" * 82)
    print(" FinText-Alpha-Vectorizer — Model Drift Monitoring Harness")
    print("=" * 82)
    print(f" Dataset Target  : {dataset_path}")
    print(f" Model Target    : {model_dir}")
    print(f" Baseline Target : {baseline_path}")
    print(f" Mode            : {'INIT-BASELINE' if args.init_baseline else 'DRIFT-COMPARISON'}")
    print(f" Run Started     : {run_started_utc}")
    print("-" * 82)

    # 1. Check Model Artifacts Presence
    model_file = model_dir / "model.onnx"
    fallback_file = model_dir / "finbert.onnx"
    if not model_dir.is_dir() or (not model_file.is_file() and not fallback_file.is_file()):
        skip_msg = f"SKIP: model artifacts not present; harness requires {model_dir}/model.onnx"
        print(f"⚠️  {skip_msg}")

        if args.init_baseline:
            print("❌ Environment Error: Cannot initialize baseline without model artifacts.", file=sys.stderr)
            sys.exit(2)

        run_finished_utc = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
        run_metadata = {
            "git_commit_sha": get_git_commit_sha(),
            "run_started_utc": run_started_utc,
            "run_finished_utc": run_finished_utc,
            "model_dir": str(model_dir),
            "dataset_path": str(dataset_path),
            "dataset_size": 0,
        }
        json_p, r_data = generate_json_report(
            output_dir=output_dir,
            run_metadata=run_metadata,
            baseline={},
            current={},
            drift={"overall_status": "SKIPPED", "breaches": [skip_msg]},
            thresholds=thresholds_config,
            reproduction_cmd=reproduction_cmd,
            status="SKIPPED",
        )
        md_p = generate_markdown_report(
            report_path=report_path,
            report_data=r_data,
            reproduction_cmd=reproduction_cmd,
        )
        print(f"  JSON Audit Report     : {json_p}")
        print(f"  Markdown Audit Report : {md_p}")
        print("=" * 82)
        sys.exit(3)

    # 2. Check and Load Dataset
    samples = load_dataset(dataset_path)
    print(f"  Loaded {len(samples)} samples from dataset.")

    # 3. Load Baseline if in Comparison Mode
    baseline = {}
    if not args.init_baseline:
        try:
            baseline = load_baseline(baseline_path)
            print(f"  Loaded baseline established at: {baseline.get('created_utc', 'UNKNOWN')}")
        except EnvironmentError as exc:
            print(f"❌ Environment Error loading baseline: {exc}", file=sys.stderr)
            sys.exit(2)

    # 4. Initialize Model Inference Engine
    try:
        from scripts.model_validation.inference import (
            ModelInferenceEngine,
            ModelNotFoundError,
        )
        from scripts.model_validation.metrics import (
            compute_calibration_error,
            compute_classification_metrics,
            compute_latency_percentiles,
        )

        engine = ModelInferenceEngine(model_dir=model_dir)
    except ModelNotFoundError as exc:
        print(f"⚠️  {exc}")
        sys.exit(3)
    except Exception as exc:
        print(f"❌ Environment Error initializing inference engine: {exc}", file=sys.stderr)
        sys.exit(2)

    # 5. Execute Inference
    print("  Executing deterministic model inference...")
    try:
        y_true, y_pred, confidences, probabilities, latencies_ms = engine.evaluate_dataset(samples)
    except Exception as exc:
        print(f"❌ Execution Error during inference: {exc}", file=sys.stderr)
        sys.exit(2)

    # 6. Compute Metrics
    clf_metrics = compute_classification_metrics(y_true, y_pred)
    ece_val, _ = compute_calibration_error(y_true, y_pred, confidences)
    lat_metrics = compute_latency_percentiles(latencies_ms)

    current_metrics = {
        **clf_metrics,
        "ece_10bins": ece_val,
        **lat_metrics,
    }

    run_finished_utc = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    run_metadata = {
        "git_commit_sha": get_git_commit_sha(),
        "run_started_utc": run_started_utc,
        "run_finished_utc": run_finished_utc,
        "model_dir": str(model_dir),
        "dataset_path": str(dataset_path),
        "dataset_size": len(samples),
    }

    # 7. Handle Baseline Initialization
    if args.init_baseline:
        save_baseline(baseline_path, current_metrics, run_metadata)
        print(f"\n[SUCCESS] Established new baseline at '{baseline_path}'. (Exit Code 0)")
        print(f"  Macro F1                  : {current_metrics['f1_macro']:.4f}")
        print(f"  Expected Calibration Error: {current_metrics['ece_10bins']:.4f}")
        print(f"  Latency P95               : {current_metrics['latency_p95_ms']:.2f} ms")
        print("=" * 82)
        return 0

    # 8. Compute Drift vs Baseline
    current_data = {
        "metrics": current_metrics,
        "dataset_size": len(samples),
        "model_dir": str(model_dir),
    }

    thresholds_obj = DriftThresholds(
        f1_macro_drop_warn=args.f1_drop_warn,
        f1_macro_drop_critical=args.f1_drop_critical,
        ece_increase_warn=args.ece_increase_warn,
        ece_increase_critical=args.ece_increase_critical,
        latency_p95_increase_pct_warn=args.latency_p95_pct_warn,
        latency_p95_increase_pct_critical=args.latency_p95_pct_critical,
    )

    drift_results = compute_drift(baseline, current_data, thresholds_obj)
    status = drift_results["overall_status"]

    # 9. Generate Reports
    json_path, report_data = generate_json_report(
        output_dir=output_dir,
        run_metadata=run_metadata,
        baseline=baseline,
        current=current_data,
        drift=drift_results,
        thresholds=thresholds_config,
        reproduction_cmd=reproduction_cmd,
        status="PASS" if status in ("OK", "WARN") else "FAIL",
    )
    md_path = generate_markdown_report(
        report_path=report_path,
        report_data=report_data,
        reproduction_cmd=reproduction_cmd,
    )

    # 10. Print Summary Results
    base_m = baseline.get("metrics", {})
    print("\n── Drift Summary Results ──────────────────────────────────────────────────────")
    print(
        f"  Macro F1 Score            : {current_metrics['f1_macro']:.4f} "
        f"(Baseline: {base_m.get('f1_macro', 0.0):.4f} | Delta: {drift_results['f1_macro_delta']:+.4f} | {drift_results['f1_macro_status']})"
    )
    print(
        f"  Expected Calibration Error: {current_metrics['ece_10bins']:.4f} "
        f"(Baseline: {base_m.get('ece_10bins', 0.0):.4f} | Delta: {drift_results['ece_delta']:+.4f} | {drift_results['ece_status']})"
    )
    print(
        f"  Latency P95               : {current_metrics['latency_p95_ms']:.2f} ms "
        f"(Baseline: {base_m.get('latency_p95_ms', 0.0):.2f} ms | Change: {drift_results['latency_p95_rel_change_pct']:+.2f}% | {drift_results['latency_p95_status']})"
    )
    print(f"  Overall Drift Verdict     : {status}")
    print(f"  JSON Audit Report         : {json_path}")
    print(f"  Markdown Audit Report     : {md_path}")
    print("=" * 82)

    if status == "CRITICAL":
        print(f"[FAILURE] Critical model drift detected ({len(drift_results['breaches'])} breach(es)):", file=sys.stderr)
        for b in drift_results["breaches"]:
            print(f"  - {b}", file=sys.stderr)
        print("[STATUS] Critical drift threshold exceeded. (Exit Code 1)", file=sys.stderr)
        return 1
    elif status == "WARN":
        print(f"[WARNING] Moderate model drift detected ({len(drift_results['breaches'])} warning(s)):")
        for b in drift_results["breaches"]:
            print(f"  - {b}")
        print("[STATUS] Drift within warning bounds. (Exit Code 0)")
        return 0
    else:
        print("[SUCCESS] All model drift metrics within nominal tolerance. (Exit Code 0)")
        return 0


if __name__ == "__main__":
    sys.exit(main())
