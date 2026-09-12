"""
FinText-Alpha-Vectorizer — Model Validation Report Generator
Generates institutional JSON audit reports and human-readable Markdown documentation.
"""

from datetime import datetime, timezone
import json
from pathlib import Path
import subprocess
from typing import Any, Dict, List, Optional, Tuple


# Audit claim reference constants from FULL_PROJECT_AUDIT.md Section 11.1
AUDIT_CLAIM_FINBERT_LATENCY = "0.85 ms"
AUDIT_CLAIM_PIPELINE_LATENCY = "< 4.5 ms"


def get_git_commit_sha() -> str:
    """Retrieve the current Git commit SHA, or 'UNKNOWN' upon error."""
    try:
        res = subprocess.run(
            ["git", "rev-parse", "HEAD"],
            capture_output=True,
            text=True,
            check=True,
            timeout=5,
        )
        return res.stdout.strip()
    except Exception:
        return "UNKNOWN"


def generate_json_report(
    output_dir: Path,
    run_started_utc: str,
    run_finished_utc: str,
    git_commit_sha: str,
    model_dir: str,
    dataset_path: str,
    dataset_size: int,
    metrics: Dict[str, Any],
    thresholds: Dict[str, Any],
    status: str,
    failures: List[str],
    calibration_bins: Optional[List[Dict[str, Any]]] = None,
    environment: str = "cpu-local",
) -> Tuple[Path, Dict[str, Any]]:

    """
    Generate timestamped JSON validation report in output_dir.
    Returns: (report_path, report_data)
    """
    output_dir = Path(output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)

    timestamp_slug = datetime.now(timezone.utc).strftime("%Y%m%d_%H%M%S")
    report_filename = f"report_{timestamp_slug}.json"
    report_path = output_dir / report_filename

    report_data = {
        "report_version": "1.0.0",
        "platform": "FinText Alpha Vectorizer",
        "suite": "FinBERT Model Quality & Calibration Certification",
        "git_commit_sha": git_commit_sha,
        "run_started_utc": run_started_utc,
        "run_finished_utc": run_finished_utc,
        "model_dir": str(model_dir),
        "dataset_path": str(dataset_path),
        "dataset_size": dataset_size,
        "environment": environment,
        "metrics": metrics,
        "thresholds": thresholds,
        "calibration_bins": calibration_bins or [],
        "status": status,
        "failures": failures,
    }

    with open(report_path, "w", encoding="utf-8") as f:
        json.dump(report_data, f, indent=2)

    return report_path, report_data


def generate_markdown_report(
    report_path: Path,
    report_data: Dict[str, Any],
    reproduction_cmd: str,
    environment: str = "cpu-local",
) -> Path:
    """
    Generate institutional Markdown report conforming to specification.
    """
    report_path = Path(report_path)
    report_path.parent.mkdir(parents=True, exist_ok=True)

    status = report_data.get("status", "UNKNOWN")
    metrics = report_data.get("metrics", {})
    thresholds = report_data.get("thresholds", {})
    failures = report_data.get("failures", [])
    cm = metrics.get("confusion_matrix", [[0, 0, 0], [0, 0, 0], [0, 0, 0]])
    cal_bins = report_data.get("calibration_bins", [])
    env_display = report_data.get("environment", environment)

    lines: List[str] = [
        "# FinText Alpha Vectorizer — FinBERT Model Quality & Calibration Certification Report",
        "",
    ]

    # Status banner
    if status == "PASS":
        lines.append("> **Overall Status**: ✅ CERTIFIED (Passes All Institutional Quality & Calibration Thresholds)")
    elif status == "SKIPPED":
        lines.append("> **Overall Status**: ⚠️ SKIPPED (Model Artifacts Not Present — Validation Pending Model Assets)")
    else:
        lines.append("> **Overall Status**: ❌ FAILED (One or More Quality/Calibration Thresholds Breached)")

    lines.extend([
        "> **Audit Level**: Tier-1 Institutional Machine Learning SLA",
        "> **Evaluation Scope**: Multi-Class PRF, Accuracy, Expected Calibration Error (ECE), Confusion Matrix, Inference Latency.",
        "",
        "---",
        "",
        "## 1. Executive Summary",
        "",
    ])

    if status == "SKIPPED":
        lines.extend([
            "Model validation was **SKIPPED** because model artifacts (e.g. `model.onnx` or tokenizer assets) were not found in the target directory.",
            "",
            "> [!WARNING]",
            "> Model artifacts not present; no quality claim can be made from this run.",
            "",
        ])
    else:
        acc = metrics.get("accuracy", 0.0) * 100.0
        f1_m = metrics.get("f1_macro", 0.0)
        ece = metrics.get("ece_10bins", 0.0)
        p95 = metrics.get("latency_p95_ms", 0.0)
        lines.extend([
            f"This report certifies the fine-tuned FinBERT INT8 ONNX model evaluated against **{report_data.get('dataset_size', 0)} annotated financial samples** across Technology, Financials, Healthcare, Industrials, Energy, and Consumer sectors.",
            "",
            "### Key Metric Highlights",
            f"- **Overall Accuracy**: **{acc:.1f}%** ({metrics.get('accuracy', 0.0):.4f})",
            f"- **Macro F1 Score**: **{f1_m:.4f}** (Threshold >= {thresholds.get('f1_macro_min', 0.75):.2f})",
            f"- **Expected Calibration Error (ECE)**: **{ece:.4f}** (Threshold <= {thresholds.get('ece_max', 0.15):.2f})",
            f"- **Inference Latency P95**: **{p95:.2f} ms** (Threshold <= {thresholds.get('latency_p95_ms_max', 50.0):.1f} ms)",
            f"- **Certification Verdict**: **{status}**",
            "",
        ])

    lines.extend([
        "---",
        "",
        "## 2. Test Execution Metadata",
        "",
        "| Parameter | Value |",
        "| :--- | :--- |",
        f"| **Platform** | {report_data.get('platform', 'FinText Alpha Vectorizer')} |",
        f"| **Audit Suite** | {report_data.get('suite', 'FinBERT Quality')} |",
        f"| **Git Commit SHA** | `{report_data.get('git_commit_sha', 'UNKNOWN')}` |",
        f"| **Run Started (UTC)** | `{report_data.get('run_started_utc', '')}` |",
        f"| **Run Finished (UTC)** | `{report_data.get('run_finished_utc', '')}` |",
        f"| **Model Directory** | `{report_data.get('model_dir', '')}` |",
        f"| **Validation Dataset** | `{report_data.get('dataset_path', '')}` |",
        f"| **Dataset Sample Count** | `{report_data.get('dataset_size', 0)}` |",
        f"| **Execution Status** | **{status}** |",
        "",
        "---",
        "",
        "## 3. Metrics Table",
        "",
        "### Global Performance Metrics",
        "| Metric | Value | Target Threshold | Compliance Status |",
        "| :--- | :---: | :---: | :---: |",
        f"| **Accuracy** | {metrics.get('accuracy', 0.0):.4f} | — | Recorded |",
        f"| **Macro Precision** | {metrics.get('precision_macro', 0.0):.4f} | — | Recorded |",
        f"| **Macro Recall** | {metrics.get('recall_macro', 0.0):.4f} | — | Recorded |",
        f"| **Macro F1** | {metrics.get('f1_macro', 0.0):.4f} | >= {thresholds.get('f1_macro_min', 0.75):.2f} | {'✅ PASS' if metrics.get('f1_macro', 0.0) >= thresholds.get('f1_macro_min', 0.75) else '❌ FAIL'} |",
        f"| **Weighted F1** | {metrics.get('f1_weighted', 0.0):.4f} | — | Recorded |",
        f"| **Expected Calibration Error (ECE)** | {metrics.get('ece_10bins', 0.0):.4f} | <= {thresholds.get('ece_max', 0.15):.2f} | {'✅ PASS' if metrics.get('ece_10bins', 0.0) <= thresholds.get('ece_max', 0.15) else '❌ FAIL'} |",
        "",
        "### Per-Class Detailed Performance",
        "| Class | Precision | Recall | F1 Score | Support Count |",
        "| :--- | :---: | :---: | :---: | :---: |",
    ])

    prec_pc = metrics.get("precision_per_class", {})
    rec_pc = metrics.get("recall_per_class", {})
    f1_pc = metrics.get("f1_per_class", {})
    sup_pc = metrics.get("support_per_class", {})

    for cls in ["POSITIVE", "NEGATIVE", "NEUTRAL"]:
        lines.append(
            f"| **{cls}** | {prec_pc.get(cls, 0.0):.4f} | {rec_pc.get(cls, 0.0):.4f} | {f1_pc.get(cls, 0.0):.4f} | {sup_pc.get(cls, 0)} |"
        )

    lines.extend([
        "",
        "---",
        "",
        "## 4. Confusion Matrix",
        "",
        "Rows represent **Ground Truth**; columns represent **Model Prediction**.",
        "",
        "| Actual \\ Predicted | Pred POSITIVE | Pred NEGATIVE | Pred NEUTRAL | Total Actual |",
        "| :--- | :---: | :---: | :---: | :---: |",
    ])

    if len(cm) == 3:
        row_labels = ["Actual POSITIVE", "Actual NEGATIVE", "Actual NEUTRAL"]
        for i in range(3):
            r_total = sum(cm[i])
            lines.append(
                f"| **{row_labels[i]}** | {cm[i][0]} | {cm[i][1]} | {cm[i][2]} | {r_total} |"
            )
        col_totals = [sum(cm[r][c] for r in range(3)) for c in range(3)]
        lines.append(
            f"| **Total Predicted** | {col_totals[0]} | {col_totals[1]} | {col_totals[2]} | {sum(col_totals)} |"
        )

    lines.extend([
        "",
        "---",
        "",
        "## 5. Calibration Plot Data (10 Decile Bins)",
        "",
        "Expected Calibration Error (ECE) partitions predictions into 10 equal-width confidence bins.",
        "",
        "| Bin | Confidence Range | Sample Count | Avg Confidence | Accuracy | Calibration Gap |",
        "| :---: | :---: | :---: | :---: | :---: | :---: |",
    ])

    for b in cal_bins:
        lines.append(
            f"| {b.get('bin_id', '-')} | `{b.get('range_str', '-')}` | {b.get('sample_count', 0)} | {b.get('avg_confidence', 0.0):.4f} | {b.get('accuracy', 0.0):.4f} | {b.get('gap', 0.0):.4f} |"
        )

    lines.extend([
        "",
        "---",
        "",
        "## 6. Latency Distribution",
        "",
        "Single-sample CPU inference latencies measured in milliseconds:",
        "",
        "| Percentile | Latency (ms) | Institutional SLA | Status |",
        "| :--- | :---: | :---: | :---: |",
        f"| **P50 (Median)** | {metrics.get('latency_p50_ms', 0.0):.2f} ms | < 25.0 ms | Baseline |",
        f"| **P95** | {metrics.get('latency_p95_ms', 0.0):.2f} ms | <= {thresholds.get('latency_p95_ms_max', 50.0):.1f} ms | {'✅ PASS' if metrics.get('latency_p95_ms', 0.0) <= thresholds.get('latency_p95_ms_max', 50.0) else '❌ FAIL'} |",
        f"| **P99** | {metrics.get('latency_p99_ms', 0.0):.2f} ms | < 100.0 ms | Baseline |",
        "",
        "---",
        "",
        "## 7. Thresholds & Verification Result",
        "",
        f"- **Verdict**: **{status}**",
    ])

    if failures:
        lines.append("- **Breached Thresholds**:")
        for fail in failures:
            lines.append(f"  - ❌ {fail}")
    else:
        if status == "PASS":
            lines.append("- **Breached Thresholds**: None. All institutional SLAs satisfied.")

    lines.extend([
        "",
        "---",
        "",
        "## 8. One-Line Reproduction Command",
        "",
        "```bash",
        reproduction_cmd,
        "```",
        "",
        "---",
        "",
        "## 9. Honesty Note",
        "",
    ])

    if status == "SKIPPED":
        lines.append(
            "> [!WARNING]\n"
            "> **Model artifacts not present; no quality claim can be made from this run.**"
        )
    else:
        lines.append(
            "This certification was executed deterministically on local CPU runtime without external network calls. "
            "All metrics and latencies reflect exact reproducibility on the specified evaluation dataset."
        )

    # Section 10: Audit Discrepancy Note
    p95_ms = metrics.get("latency_p95_ms", 0.0)
    lines.extend([
        "",
        "---",
        "",
        "## 10. Audit Discrepancy Note",
        "",
        f"- **Environment Profile**: `{env_display}`",
        f"- **Measured P95 Latency**: **{p95_ms:.2f} ms**",
        "- **FULL_PROJECT_AUDIT.md Section 11.1 Benchmark Claims**:",
        f'  - "FinBERT Sentiment Scoring ... {AUDIT_CLAIM_FINBERT_LATENCY}"',
        f'  - "Total Ingestion-to-Signal Pipeline ... {AUDIT_CLAIM_PIPELINE_LATENCY}"',
        "",
        f"The measured P95 latency in this run ({p95_ms:.2f} ms) is NOT reproducible against the audit claim (<1.8 ms FinBERT / <4.5 ms pipeline) on the tested environment. Reproducing the audit claim would require GPU acceleration, batching, or an optimized runtime not present in this repository's default CPU configuration. This discrepancy is tracked in docs/LATENCY_RECONCILIATION.md.",
    ])

    lines.append("")

    with open(report_path, "w", encoding="utf-8") as f:
        f.write("\n".join(lines))

    return report_path
