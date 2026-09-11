"""
FinText-Alpha-Vectorizer — Model Validation Metrics
Deterministic computation of Accuracy, Per-Class & Macro PRF,
Confusion Matrix, Expected Calibration Error (ECE), and Latency Percentiles.
"""

from typing import Any, Dict, List, Tuple
import numpy as np

CLASSES: List[str] = ["POSITIVE", "NEGATIVE", "NEUTRAL"]
CLASS_TO_IDX: Dict[str, int] = {"POSITIVE": 0, "NEGATIVE": 1, "NEUTRAL": 2}
IDX_TO_CLASS: Dict[int, str] = {0: "POSITIVE", 1: "NEGATIVE", 2: "NEUTRAL"}


def normalize_label(label: Any) -> str:
    """Normalize input label to canonical uppercase string."""
    lbl = str(label).strip().upper()
    if lbl in CLASS_TO_IDX:
        return lbl
    # Common variations
    if lbl in ("POS", "BULLISH", "1"):
        return "POSITIVE"
    if lbl in ("NEG", "BEARISH", "-1"):
        return "NEGATIVE"
    if lbl in ("NEU", "HOLD", "0"):
        return "NEUTRAL"
    raise ValueError(f"Unknown sentiment label: '{label}'. Expected one of {CLASSES}")


def compute_classification_metrics(
    y_true: List[str],
    y_pred: List[str],
) -> Dict[str, Any]:
    """
    Compute multi-class classification metrics:
    - Overall accuracy
    - Per-class precision, recall, F1, and support
    - Macro precision, recall, and F1
    - Weighted F1
    - 3x3 Confusion Matrix
    """
    n_samples = len(y_true)
    if n_samples == 0:
        raise ValueError("Cannot compute metrics on an empty dataset.")

    y_true_norm = [normalize_label(y) for y in y_true]
    y_pred_norm = [normalize_label(y) for y in y_pred]

    # Initialize 3x3 confusion matrix: rows = true, cols = predicted
    cm = np.zeros((3, 3), dtype=int)
    for yt, yp in zip(y_true_norm, y_pred_norm):
        r = CLASS_TO_IDX[yt]
        c = CLASS_TO_IDX[yp]
        cm[r, c] += 1

    total_correct = int(np.trace(cm))
    accuracy = float(total_correct / n_samples)

    precision_per_class: Dict[str, float] = {}
    recall_per_class: Dict[str, float] = {}
    f1_per_class: Dict[str, float] = {}
    support_per_class: Dict[str, int] = {}

    for cls_name in CLASSES:
        idx = CLASS_TO_IDX[cls_name]
        tp = float(cm[idx, idx])
        fp = float(np.sum(cm[:, idx]) - tp)
        fn = float(np.sum(cm[idx, :]) - tp)
        support = int(np.sum(cm[idx, :]))

        prec = tp / (tp + fp) if (tp + fp) > 0 else 0.0
        rec = tp / (tp + fn) if (tp + fn) > 0 else 0.0
        f1 = (2.0 * prec * rec / (prec + rec)) if (prec + rec) > 0 else 0.0

        precision_per_class[cls_name] = round(prec, 4)
        recall_per_class[cls_name] = round(rec, 4)
        f1_per_class[cls_name] = round(f1, 4)
        support_per_class[cls_name] = support

    precision_macro = round(float(np.mean(list(precision_per_class.values()))), 4)
    recall_macro = round(float(np.mean(list(recall_per_class.values()))), 4)
    f1_macro = round(float(np.mean(list(f1_per_class.values()))), 4)

    weighted_f1_sum = sum(
        f1_per_class[cls] * support_per_class[cls] for cls in CLASSES
    )
    f1_weighted = round(float(weighted_f1_sum / n_samples), 4)

    return {
        "total_samples": n_samples,
        "accuracy": round(accuracy, 4),
        "precision_macro": precision_macro,
        "recall_macro": recall_macro,
        "f1_macro": f1_macro,
        "f1_weighted": f1_weighted,
        "precision_per_class": precision_per_class,
        "recall_per_class": recall_per_class,
        "f1_per_class": f1_per_class,
        "support_per_class": support_per_class,
        "confusion_matrix": cm.tolist(),
    }


def compute_calibration_error(
    y_true: List[str],
    y_pred: List[str],
    confidences: List[float],
    num_bins: int = 10,
) -> Tuple[float, List[Dict[str, Any]]]:
    """
    Compute Expected Calibration Error (ECE) across equal-width confidence bins.
    Formula: ECE = sum_{b=1}^{B} (|B_b| / N) * |acc(B_b) - conf(B_b)|
    """
    n_samples = len(y_true)
    if n_samples == 0:
        return 0.0, []

    y_true_norm = [normalize_label(y) for y in y_true]
    y_pred_norm = [normalize_label(y) for y in y_pred]

    accuracies = np.array(
        [1.0 if yt == yp else 0.0 for yt, yp in zip(y_true_norm, y_pred_norm)],
        dtype=float,
    )
    confs = np.array(confidences, dtype=float)

    bin_edges = np.linspace(0.0, 1.0, num_bins + 1)
    ece = 0.0
    bins_data = []

    for b in range(num_bins):
        bin_lower = bin_edges[b]
        bin_upper = bin_edges[b + 1]

        if b == num_bins - 1:
            in_bin = (confs >= bin_lower) & (confs <= bin_upper)
        else:
            in_bin = (confs >= bin_lower) & (confs < bin_upper)

        bin_count = int(np.sum(in_bin))
        if bin_count > 0:
            bin_acc = float(np.mean(accuracies[in_bin]))
            bin_conf = float(np.mean(confs[in_bin]))
            bin_gap = abs(bin_acc - bin_conf)
            ece += (bin_count / n_samples) * bin_gap
        else:
            bin_acc = 0.0
            bin_conf = 0.0
            bin_gap = 0.0

        bins_data.append(
            {
                "bin_id": b + 1,
                "range_min": round(bin_lower, 2),
                "range_max": round(bin_upper, 2),
                "range_str": f"[{bin_lower:.1f}, {bin_upper:.1f}]",
                "sample_count": bin_count,
                "avg_confidence": round(bin_conf, 4),
                "accuracy": round(bin_acc, 4),
                "gap": round(bin_gap, 4),
            }
        )

    return round(float(ece), 4), bins_data


def compute_latency_percentiles(latencies_ms: List[float]) -> Dict[str, float]:
    """Compute P50, P95, and P99 latency percentiles in milliseconds."""
    if not latencies_ms:
        return {"latency_p50_ms": 0.0, "latency_p95_ms": 0.0, "latency_p99_ms": 0.0}

    arr = np.array(latencies_ms, dtype=float)
    return {
        "latency_p50_ms": round(float(np.percentile(arr, 50)), 2),
        "latency_p95_ms": round(float(np.percentile(arr, 95)), 2),
        "latency_p99_ms": round(float(np.percentile(arr, 99)), 2),
    }


def evaluate_metrics_against_thresholds(
    metrics: Dict[str, Any],
    min_f1_macro: float = 0.75,
    max_ece: float = 0.15,
    max_latency_p95_ms: float = 50.0,
) -> Tuple[bool, List[str]]:
    """
    Compare computed metrics against institutional quality thresholds.
    Returns: (passed: bool, failures: list of failure description strings)
    """
    failures = []

    f1_macro = metrics.get("f1_macro", 0.0)
    if f1_macro < min_f1_macro:
        failures.append(
            f"Macro F1 ({f1_macro:.4f}) is below threshold ({min_f1_macro:.4f})"
        )

    ece = metrics.get("ece_10bins", 1.0)
    if ece > max_ece:
        failures.append(
            f"Expected Calibration Error ({ece:.4f}) exceeds threshold ({max_ece:.4f})"
        )

    latency_p95 = metrics.get("latency_p95_ms", 9999.0)
    if latency_p95 > max_latency_p95_ms:
        failures.append(
            f"Latency P95 ({latency_p95:.2f}ms) exceeds threshold ({max_latency_p95_ms:.2f}ms)"
        )

    return len(failures) == 0, failures
