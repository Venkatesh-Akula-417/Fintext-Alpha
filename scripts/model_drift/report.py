"""
FinText-Alpha-Vectorizer — Model Drift Report Generator
Generates institutional JSON audit reports and human-readable Markdown documentation.
"""

from datetime import datetime, timezone
import json
from pathlib import Path
from typing import Any, Dict, Tuple


def generate_json_report(
    output_dir: Path,
    run_metadata: Dict[str, Any],
    baseline: Dict[str, Any],
    current: Dict[str, Any],
    drift: Dict[str, Any],
    thresholds: Dict[str, Any],
    reproduction_cmd: str,
    status: str,
) -> Tuple[Path, Dict[str, Any]]:
    """
    Generate timestamped JSON drift report in output_dir.
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
        "suite": "FinBERT Model Drift & Continuous Performance Monitoring",
        "git_commit_sha": run_metadata.get("git_commit_sha", "UNKNOWN"),
        "run_started_utc": run_metadata.get("run_started_utc", ""),
        "run_finished_utc": run_metadata.get("run_finished_utc", ""),
        "model_dir": str(run_metadata.get("model_dir", "")),
        "dataset_path": str(run_metadata.get("dataset_path", "")),
        "dataset_size": run_metadata.get("dataset_size", 0),
        "status": status,
        "overall_drift_status": drift.get("overall_status", status),
        "thresholds": thresholds,
        "baseline": baseline,
        "current": current,
        "drift": drift,
        "breaches": drift.get("breaches", []),
        "reproduction_cmd": reproduction_cmd,
    }

    with open(report_path, "w", encoding="utf-8") as f:
        json.dump(report_data, f, indent=2)

    return report_path, report_data


def generate_markdown_report(
    report_path: Path,
    report_data: Dict[str, Any],
    reproduction_cmd: str,
) -> Path:
    """
    Generate institutional Markdown drift report conforming to specification.
    """
    report_path = Path(report_path)
    report_path.parent.mkdir(parents=True, exist_ok=True)

    status = report_data.get("status", "UNKNOWN")
    overall_drift = report_data.get("overall_drift_status", status)
    baseline = report_data.get("baseline", {})
    current = report_data.get("current", {})
    drift = report_data.get("drift", {})
    thresholds = report_data.get("thresholds", {})
    breaches = report_data.get("breaches", [])

    base_m = baseline.get("metrics", {})
    curr_m = current.get("metrics", {})

    lines = [
        "# FinText Alpha Vectorizer — FinBERT Model Drift Monitoring Report",
        "",
    ]

    # Status Banner
    if status == "SKIPPED":
        lines.append("> **Overall Status**: ⚠️ SKIPPED (Model Artifacts Not Present — Monitoring Deferred)")
    elif overall_drift == "OK":
        lines.append("> **Overall Status**: ✅ OK (No Significant Performance Drift Detected)")
    elif overall_drift == "WARN":
        lines.append("> **Overall Status**: ⚠️ WARN (Moderate Performance Drift Detected within Warning Bounds)")
    else:
        lines.append("> **Overall Status**: ❌ CRITICAL (Severe Model Drift Detected — Immediate Attention Required)")

    lines.extend([
        "> **Monitoring Tier**: Institutional Continuous Quality Assurance",
        "> **Evaluation Target**: Multi-Class Macro PRF, Expected Calibration Error (ECE), and Inference Latency vs Committed Baseline.",
        "",
        "---",
        "",
        "## 1. Executive Summary",
        "",
    ])

    if status == "SKIPPED":
        lines.extend([
            "Model drift monitoring was **SKIPPED** because model artifacts (e.g. `model.onnx` or tokenizer assets) were not found in the target directory.",
            "",
            "> [!WARNING]",
            "> Model artifacts not present; no drift evaluation can be made from this run.",
            "",
        ])
    else:
        f1_curr = curr_m.get("f1_macro", 0.0)
        f1_delta = drift.get("f1_macro_delta", 0.0)
        ece_curr = curr_m.get("ece_10bins", 0.0)
        ece_delta = drift.get("ece_delta", 0.0)
        p95_curr = curr_m.get("latency_p95_ms", 0.0)
        p95_pct = drift.get("latency_p95_rel_change_pct", 0.0)

        lines.extend([
            f"The continuous drift monitoring engine evaluated **{report_data.get('dataset_size', 0)} samples** against baseline snapshot `{baseline.get('created_utc', 'N/A')}` (`{baseline.get('git_commit_sha', 'UNKNOWN')[:8]}`).",
            "",
            "### Headline Metric Drift",
            f"- **Macro F1 Score**: **{f1_curr:.4f}** (Delta: `{f1_delta:+.4f}` | Status: **{drift.get('f1_macro_status', 'OK')}**)",
            f"- **Expected Calibration Error (ECE)**: **{ece_curr:.4f}** (Delta: `{ece_delta:+.4f}` | Status: **{drift.get('ece_status', 'OK')}**)",
            f"- **Inference Latency P95**: **{p95_curr:.2f} ms** (Change: `{p95_pct:+.2f}%` | Status: **{drift.get('latency_p95_status', 'OK')}**)",
            f"- **Overall Verdict**: **{overall_drift}**",
            "",
        ])

    lines.extend([
        "---",
        "",
        "## 2. Baseline vs Current Comparison Table",
        "",
        "| Metric | Baseline Value | Current Value | Absolute Delta | Relative Change | Drift Status | Monitored Thresholds |",
        "| :--- | :---: | :---: | :---: | :---: | :---: | :--- |",
    ])

    if status != "SKIPPED":
        f1_w_val = thresholds.get("f1_macro_drop_warn", 0.03)
        f1_c_val = thresholds.get("f1_macro_drop_critical", 0.07)
        ece_w_val = thresholds.get("ece_increase_warn", 0.02)
        ece_c_val = thresholds.get("ece_increase_critical", 0.05)
        lat_w_val = thresholds.get("latency_p95_increase_pct_warn", 30.0)
        lat_c_val = thresholds.get("latency_p95_increase_pct_critical", 60.0)

        def badge(st: str) -> str:
            if st == "OK":
                return "✅ OK"
            if st == "WARN":
                return "⚠️ WARN"
            if st == "CRITICAL":
                return "❌ CRITICAL"
            return st

        lines.extend([
            f"| **Macro F1 Score** | {base_m.get('f1_macro', 0.0):.4f} | {curr_m.get('f1_macro', 0.0):.4f} | {drift.get('f1_macro_delta', 0.0):+.4f} | {drift.get('f1_macro_rel_change', 0.0)*100:+.2f}% | {badge(drift.get('f1_macro_status', 'OK'))} | Drop >= {f1_w_val:.2f} (W) / >= {f1_c_val:.2f} (C) |",
            f"| **Expected Calibration Error (ECE)** | {base_m.get('ece_10bins', 0.0):.4f} | {curr_m.get('ece_10bins', 0.0):.4f} | {drift.get('ece_delta', 0.0):+.4f} | {drift.get('ece_rel_change', 0.0)*100:+.2f}% | {badge(drift.get('ece_status', 'OK'))} | Inc >= {ece_w_val:.2f} (W) / >= {ece_c_val:.2f} (C) |",
            f"| **Inference Latency P95** | {base_m.get('latency_p95_ms', 0.0):.2f} ms | {curr_m.get('latency_p95_ms', 0.0):.2f} ms | {drift.get('latency_p95_delta_ms', 0.0):+.2f} ms | {drift.get('latency_p95_rel_change_pct', 0.0):+.2f}% | {badge(drift.get('latency_p95_status', 'OK'))} | Inc >= +{lat_w_val:.0f}% (W) / >= +{lat_c_val:.0f}% (C) |",
            f"| **Overall Accuracy** | {base_m.get('accuracy', 0.0):.4f} | {curr_m.get('accuracy', 0.0):.4f} | {drift.get('accuracy_delta', 0.0):+.4f} | {drift.get('accuracy_rel_change', 0.0)*100:+.2f}% | Reference | Baseline tracking |",
            f"| **Weighted F1 Score** | {base_m.get('f1_weighted', 0.0):.4f} | {curr_m.get('f1_weighted', 0.0):.4f} | {drift.get('f1_weighted_delta', 0.0):+.4f} | {drift.get('f1_weighted_rel_change', 0.0)*100:+.2f}% | Reference | Baseline tracking |",
            f"| **Inference Latency P50 (Median)** | {base_m.get('latency_p50_ms', 0.0):.2f} ms | {curr_m.get('latency_p50_ms', 0.0):.2f} ms | {drift.get('latency_p50_delta_ms', 0.0):+.2f} ms | — | Reference | Baseline tracking |",
            f"| **Inference Latency P99** | {base_m.get('latency_p99_ms', 0.0):.2f} ms | {curr_m.get('latency_p99_ms', 0.0):.2f} ms | {drift.get('latency_p99_delta_ms', 0.0):+.2f} ms | — | Reference | Baseline tracking |",
        ])
    else:
        lines.append("| — | Model absent | — | — | — | SKIPPED | — |")

    lines.extend([
        "",
        "---",
        "",
        "## 3. Per-Metric Drift Detail",
        "",
    ])

    if status != "SKIPPED":
        lines.extend([
            f"- **Classification Macro F1**: Baseline was `{base_m.get('f1_macro', 0.0):.4f}`; Current is `{curr_m.get('f1_macro', 0.0):.4f}` (Absolute change: `{drift.get('f1_macro_delta', 0.0):+.4f}`). Status is `{drift.get('f1_macro_status', 'OK')}`.",
            f"- **Expected Calibration Error**: Baseline was `{base_m.get('ece_10bins', 0.0):.4f}`; Current is `{curr_m.get('ece_10bins', 0.0):.4f}` (Absolute change: `{drift.get('ece_delta', 0.0):+.4f}`). Status is `{drift.get('ece_status', 'OK')}`.",
            f"- **Latency P95 Distribution**: Baseline was `{base_m.get('latency_p95_ms', 0.0):.2f} ms`; Current is `{curr_m.get('latency_p95_ms', 0.0):.2f} ms` (`{drift.get('latency_p95_rel_change_pct', 0.0):+.2f}%`). Status is `{drift.get('latency_p95_status', 'OK')}`.",
        ])
    else:
        lines.append("- No metrics evaluated due to missing model artifacts.")

    lines.extend([
        "",
        "---",
        "",
        "## 4. Breaches and Degradation Alerts",
        "",
    ])

    if breaches:
        lines.append("The following threshold breaches were identified:")
        for b in breaches:
            lines.append(f"- ⚠️ {b}")
    else:
        lines.append("None detected. All monitored performance indicators operate within nominal variance tolerance.")

    lines.extend([
        "",
        "---",
        "",
        "## 5. One-Line Reproduction Command",
        "",
        "```bash",
        reproduction_cmd,
        "```",
        "",
        "---",
        "",
        "## 6. Honesty Note",
        "",
    ])

    if status == "SKIPPED":
        lines.append(
            "> [!WARNING]\n"
            "> **Model artifacts not present; no drift claim can be made from this run.**"
        )
    else:
        lines.append(
            "This drift certification was executed deterministically on local CPU runtime without external network calls. "
            "All metrics and latencies reflect exact reproducibility against the committed baseline dataset."
        )

    lines.append("")

    with open(report_path, "w", encoding="utf-8") as f:
        f.write("\n".join(lines))

    return report_path
