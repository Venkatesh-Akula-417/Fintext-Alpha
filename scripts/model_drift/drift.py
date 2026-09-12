"""
FinText-Alpha-Vectorizer — Model Drift Analytics Engine
Calculates performance drift, calibration drift, and latency degradation
relative to a validated baseline.
"""

from dataclasses import dataclass
from typing import Any, Dict, List


@dataclass
class DriftThresholds:
    """Configurable thresholds for model drift warning and critical alerts."""
    f1_macro_drop_warn: float = 0.03
    f1_macro_drop_critical: float = 0.07
    ece_increase_warn: float = 0.02
    ece_increase_critical: float = 0.05
    latency_p95_increase_pct_warn: float = 30.0
    latency_p95_increase_pct_critical: float = 60.0


def compute_drift(
    baseline: Dict[str, Any],
    current: Dict[str, Any],
    thresholds: DriftThresholds,
) -> Dict[str, Any]:
    """
    Compute drift metrics comparing current evaluation against baseline.

    Returns dictionary conforming to the drift report specification:
        - f1_macro_delta, f1_macro_rel_change, f1_macro_status
        - ece_delta, ece_rel_change, ece_status
        - latency_p95_delta_ms, latency_p95_rel_change_pct, latency_p95_status
        - overall_status ("OK" | "WARN" | "CRITICAL")
        - breaches: List[str]
    """
    base_m = baseline.get("metrics", {})
    curr_m = current.get("metrics", {})

    breaches: List[str] = []

    # 1. Macro F1 Drift Evaluation
    b_f1 = float(base_m.get("f1_macro", 0.0))
    c_f1 = float(curr_m.get("f1_macro", 0.0))
    f1_delta = round(c_f1 - b_f1, 4)
    f1_rel_change = round((c_f1 - b_f1) / b_f1, 4) if b_f1 > 0 else 0.0
    f1_drop = b_f1 - c_f1

    if f1_drop >= thresholds.f1_macro_drop_critical:
        f1_status = "CRITICAL"
        breaches.append(
            f"Macro F1 drop {f1_drop:.4f} (baseline {b_f1:.4f} -> current {c_f1:.4f}) "
            f"breaches CRITICAL threshold (drop >= {thresholds.f1_macro_drop_critical:.4f})"
        )
    elif f1_drop >= thresholds.f1_macro_drop_warn:
        f1_status = "WARN"
        breaches.append(
            f"Macro F1 drop {f1_drop:.4f} (baseline {b_f1:.4f} -> current {c_f1:.4f}) "
            f"breaches WARNING threshold (drop >= {thresholds.f1_macro_drop_warn:.4f})"
        )
    else:
        f1_status = "OK"

    # 2. Expected Calibration Error (ECE) Drift Evaluation
    b_ece = float(base_m.get("ece_10bins", 0.0))
    c_ece = float(curr_m.get("ece_10bins", 0.0))
    ece_delta = round(c_ece - b_ece, 4)
    ece_rel_change = round((c_ece - b_ece) / b_ece, 4) if b_ece > 0 else 0.0
    ece_increase = c_ece - b_ece

    if ece_increase >= thresholds.ece_increase_critical:
        ece_status = "CRITICAL"
        breaches.append(
            f"ECE increase {ece_increase:+.4f} (baseline {b_ece:.4f} -> current {c_ece:.4f}) "
            f"breaches CRITICAL threshold (increase >= {thresholds.ece_increase_critical:.4f})"
        )
    elif ece_increase >= thresholds.ece_increase_warn:
        ece_status = "WARN"
        breaches.append(
            f"ECE increase {ece_increase:+.4f} (baseline {b_ece:.4f} -> current {c_ece:.4f}) "
            f"breaches WARNING threshold (increase >= {thresholds.ece_increase_warn:.4f})"
        )
    else:
        ece_status = "OK"

    # 3. P95 Latency Drift Evaluation
    b_p95 = float(base_m.get("latency_p95_ms", 0.0))
    c_p95 = float(curr_m.get("latency_p95_ms", 0.0))
    lat_p95_delta_ms = round(c_p95 - b_p95, 2)
    lat_p95_pct_change = round(((c_p95 - b_p95) / b_p95) * 100.0, 2) if b_p95 > 0 else 0.0

    if lat_p95_pct_change >= thresholds.latency_p95_increase_pct_critical:
        lat_p95_status = "CRITICAL"
        breaches.append(
            f"Latency P95 increase {lat_p95_pct_change:+.2f}% ({lat_p95_delta_ms:+.2f} ms: "
            f"baseline {b_p95:.2f} ms -> current {c_p95:.2f} ms) "
            f"breaches CRITICAL threshold (+{thresholds.latency_p95_increase_pct_critical:.1f}%)"
        )
    elif lat_p95_pct_change >= thresholds.latency_p95_increase_pct_warn:
        lat_p95_status = "WARN"
        breaches.append(
            f"Latency P95 increase {lat_p95_pct_change:+.2f}% ({lat_p95_delta_ms:+.2f} ms: "
            f"baseline {b_p95:.2f} ms -> current {c_p95:.2f} ms) "
            f"breaches WARNING threshold (+{thresholds.latency_p95_increase_pct_warn:.1f}%)"
        )
    else:
        lat_p95_status = "OK"

    # 4. Overall Status Aggregation
    statuses = [f1_status, ece_status, lat_p95_status]
    if "CRITICAL" in statuses:
        overall_status = "CRITICAL"
    elif "WARN" in statuses:
        overall_status = "WARN"
    else:
        overall_status = "OK"

    # Additional contextual deltas for reporting
    b_acc = float(base_m.get("accuracy", 0.0))
    c_acc = float(curr_m.get("accuracy", 0.0))
    acc_delta = round(c_acc - b_acc, 4)
    acc_rel_change = round((c_acc - b_acc) / b_acc, 4) if b_acc > 0 else 0.0

    b_f1w = float(base_m.get("f1_weighted", 0.0))
    c_f1w = float(curr_m.get("f1_weighted", 0.0))
    f1w_delta = round(c_f1w - b_f1w, 4)
    f1w_rel_change = round((c_f1w - b_f1w) / b_f1w, 4) if b_f1w > 0 else 0.0

    b_p50 = float(base_m.get("latency_p50_ms", 0.0))
    c_p50 = float(curr_m.get("latency_p50_ms", 0.0))
    lat_p50_delta_ms = round(c_p50 - b_p50, 2)

    b_p99 = float(base_m.get("latency_p99_ms", 0.0))
    c_p99 = float(curr_m.get("latency_p99_ms", 0.0))
    lat_p99_delta_ms = round(c_p99 - b_p99, 2)

    return {
        "f1_macro_delta": f1_delta,
        "f1_macro_rel_change": f1_rel_change,
        "f1_macro_status": f1_status,
        "ece_delta": ece_delta,
        "ece_rel_change": ece_rel_change,
        "ece_status": ece_status,
        "latency_p95_delta_ms": lat_p95_delta_ms,
        "latency_p95_rel_change_pct": lat_p95_pct_change,
        "latency_p95_status": lat_p95_status,
        "overall_status": overall_status,
        "breaches": breaches,
        # Context metrics
        "accuracy_delta": acc_delta,
        "accuracy_rel_change": acc_rel_change,
        "f1_weighted_delta": f1w_delta,
        "f1_weighted_rel_change": f1w_rel_change,
        "latency_p50_delta_ms": lat_p50_delta_ms,
        "latency_p99_delta_ms": lat_p99_delta_ms,
    }
