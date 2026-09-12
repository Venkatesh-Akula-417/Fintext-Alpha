#!/usr/bin/env python3
"""
===============================================================================
FinText-Alpha-Vectorizer — Model Quality & Calibration Validation Harness
Suite: FinBERT INT8 ONNX Institutional Performance & ECE Certification
===============================================================================

Evaluates:
  - Multi-class classification Accuracy, Precision, Recall, Macro-F1, Weighted-F1
  - 3x3 Confusion Matrix across POSITIVE, NEGATIVE, NEUTRAL
  - Expected Calibration Error (ECE) across 10 decile confidence bins
  - Single-sample inference latency percentiles (P50, P95, P99)

Exit Codes:
  0 = All quality, calibration, and latency thresholds PASS
  1 = Threshold failure (one or more metrics out of bounds)
  2 = Environment error (missing dataset, invalid JSON, missing dependencies)
  3 = SKIP (model artifacts not present)
===============================================================================
"""

import argparse
from datetime import datetime, timezone
import json
import os
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


def parse_args():
    parser = argparse.ArgumentParser(
        description="FinText Alpha Vectorizer Model Quality & Calibration Validation Harness"
    )
    parser.add_argument(
        "--dataset",
        default="config/model_validation_dataset.json",
        help="Path to labeled evaluation dataset JSON.",
    )
    parser.add_argument(
        "--model-dir",
        default="models/finbert-finetuned",
        help="Directory containing ONNX model and tokenizer assets.",
    )
    parser.add_argument(
        "--output-dir",
        default="data/model-validation",
        help="Directory to save timestamped JSON validation reports.",
    )
    parser.add_argument(
        "--report-path",
        default="docs/MODEL_VALIDATION_REPORT.md",
        help="Path to generate human-readable Markdown certification report.",
    )
    parser.add_argument(
        "--min-f1-macro",
        type=float,
        default=0.75,
        help="Minimum acceptable Macro F1 score threshold (default: 0.75).",
    )
    parser.add_argument(
        "--max-ece",
        type=float,
        default=0.15,
        help="Maximum acceptable Expected Calibration Error threshold (default: 0.15).",
    )
    parser.add_argument(
        "--environment",
        choices=["cpu-local", "cpu-ci", "gpu-optimized"],
        default="cpu-local",
        help="Deployment environment profile for the latency threshold context.",
    )
    parser.add_argument(
        "--max-latency-p95-ms",
        type=float,
        default=250.0,
        help="Maximum acceptable P95 latency in milliseconds (default: 250.0 ms).",
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
    output_dir = Path(args.output_dir)
    report_path = Path(args.report_path)

    thresholds = {
        "f1_macro_min": args.min_f1_macro,
        "ece_max": args.max_ece,
        "latency_p95_ms_max": args.max_latency_p95_ms,
    }

    reproduction_cmd = (
        f"python scripts/validate_model_quality.py "
        f"--dataset {args.dataset} "
        f"--model-dir {args.model_dir} "
        f"--output-dir {args.output_dir} "
        f"--report-path {args.report_path} "
        f"--min-f1-macro {args.min_f1_macro} "
        f"--max-ece {args.max_ece} "
        f"--max-latency-p95-ms {args.max_latency_p95_ms} "
        f"--environment {args.environment}"
    )

    print("=" * 82)
    print(" FinText-Alpha-Vectorizer — Model Quality & Calibration Validation Harness")
    print("=" * 82)
    print(f" Dataset Target : {dataset_path}")
    print(f" Model Target   : {model_dir}")
    print(f" Environment    : {args.environment}")
    print(f" Run Started    : {run_started_utc}")
    print("-" * 82)

    # 1. Check Model Existence
    model_file = model_dir / "model.onnx"
    fallback_file = model_dir / "finbert.onnx"
    if not model_dir.is_dir() or (not model_file.is_file() and not fallback_file.is_file()):
        skip_msg = f"SKIP: model artifacts not present; harness requires {model_dir}/model.onnx"
        print(f"⚠️  {skip_msg}")

        from scripts.model_validation.report import (
            generate_json_report,
            generate_markdown_report,
            get_git_commit_sha,
        )

        run_finished_utc = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
        json_p, r_data = generate_json_report(
            output_dir=output_dir,
            run_started_utc=run_started_utc,
            run_finished_utc=run_finished_utc,
            git_commit_sha=get_git_commit_sha(),
            model_dir=str(model_dir),
            dataset_path=str(dataset_path),
            dataset_size=0,
            metrics={},
            thresholds=thresholds,
            status="SKIPPED",
            failures=[skip_msg],
            calibration_bins=[],
            environment=args.environment,
        )
        md_p = generate_markdown_report(
            report_path=report_path,
            report_data=r_data,
            reproduction_cmd=reproduction_cmd,
            environment=args.environment,
        )
        print(f"  JSON Audit Report     : {json_p}")
        print(f"  Markdown Audit Report : {md_p}")
        print("=" * 82)
        sys.exit(3)

    # 2. Check and Load Dataset
    samples = load_dataset(dataset_path)
    print(f"  Loaded {len(samples)} samples from dataset.")

    # 3. Initialize Model Inference Engine
    try:
        from scripts.model_validation.inference import (
            ModelInferenceEngine,
            ModelNotFoundError,
        )
        from scripts.model_validation.metrics import (
            compute_calibration_error,
            compute_classification_metrics,
            compute_latency_percentiles,
            evaluate_metrics_against_thresholds,
        )
        from scripts.model_validation.report import (
            generate_json_report,
            generate_markdown_report,
            get_git_commit_sha,
        )

        engine = ModelInferenceEngine(model_dir=model_dir)
    except ModelNotFoundError as exc:
        print(f"⚠️  {exc}")
        sys.exit(3)
    except Exception as exc:
        print(f"❌ Environment Error initializing inference engine: {exc}", file=sys.stderr)
        sys.exit(2)

    # 4. Execute Inference
    print("  Executing deterministic model inference...")
    try:
        y_true, y_pred, confidences, probabilities, latencies_ms = engine.evaluate_dataset(samples)
    except Exception as exc:
        print(f"❌ Execution Error during inference: {exc}", file=sys.stderr)
        sys.exit(2)

    # 5. Compute Metrics
    clf_metrics = compute_classification_metrics(y_true, y_pred)
    ece_val, cal_bins = compute_calibration_error(y_true, y_pred, confidences)
    lat_metrics = compute_latency_percentiles(latencies_ms)

    combined_metrics = {
        **clf_metrics,
        "ece_10bins": ece_val,
        **lat_metrics,
    }

    # 6. Evaluate Against Thresholds
    passed, failures = evaluate_metrics_against_thresholds(
        combined_metrics,
        min_f1_macro=args.min_f1_macro,
        max_ece=args.max_ece,
        max_latency_p95_ms=args.max_latency_p95_ms,
    )
    status = "PASS" if passed else "FAIL"

    run_finished_utc = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")

    # 7. Generate Reports
    json_path, report_data = generate_json_report(
        output_dir=output_dir,
        run_started_utc=run_started_utc,
        run_finished_utc=run_finished_utc,
        git_commit_sha=get_git_commit_sha(),
        model_dir=str(model_dir),
        dataset_path=str(dataset_path),
        dataset_size=len(samples),
        metrics=combined_metrics,
        thresholds=thresholds,
        status=status,
        failures=failures,
        calibration_bins=cal_bins,
        environment=args.environment,
    )
    md_path = generate_markdown_report(
        report_path=report_path,
        report_data=report_data,
        reproduction_cmd=reproduction_cmd,
        environment=args.environment,
    )

    # 8. Print Summary Results
    print("\n── Summary Results ────────────────────────────────────────────────────────────")
    print(f"  Environment Profile       : {args.environment}")
    print(f"  Accuracy                  : {combined_metrics['accuracy']:.4f}")
    print(f"  Macro F1 Score            : {combined_metrics['f1_macro']:.4f} (Min SLA: {args.min_f1_macro:.2f})")
    print(f"  Weighted F1 Score         : {combined_metrics['f1_weighted']:.4f}")
    print(f"  Expected Calibration Error: {combined_metrics['ece_10bins']:.4f} (Max SLA: {args.max_ece:.2f})")
    print(f"  Latency P95               : {combined_metrics['latency_p95_ms']:.2f} ms (Max SLA: {args.max_latency_p95_ms:.1f} ms)")
    print(f"  JSON Audit Report         : {json_path}")
    print(f"  Markdown Audit Report     : {md_path}")
    print("=" * 82)

    if passed:
        print("[SUCCESS] All institutional model quality and calibration criteria PASSED. (Exit Code 0)")
        return 0
    else:
        print(f"[FAILURE] Model validation failed {len(failures)} threshold(s):", file=sys.stderr)
        for fail in failures:
            print(f"  - {fail}", file=sys.stderr)
        print("[STATUS] Quality threshold breach detected. (Exit Code 1)", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
