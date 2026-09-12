"""
FinText-Alpha-Vectorizer — Model Drift Baseline Management
Handles serialization, validation, and loading of canonical model performance baselines.
"""

from datetime import datetime, timezone
import json
from pathlib import Path
from typing import Any, Dict, Union

REQUIRED_METRIC_KEYS = [
    "accuracy",
    "f1_macro",
    "f1_weighted",
    "ece_10bins",
    "latency_p50_ms",
    "latency_p95_ms",
    "latency_p99_ms",
]


def load_baseline(path: Union[str, Path]) -> Dict[str, Any]:
    """
    Load and validate a committed baseline JSON file.

    Raises:
        EnvironmentError: If baseline file does not exist, contains invalid JSON,
                          or lacks mandatory schema keys.
    """
    baseline_path = Path(path)
    if not baseline_path.is_file():
        raise EnvironmentError(
            f"Baseline file not found at '{baseline_path}'. "
            "Run 'python scripts/model_drift_monitor.py --init-baseline' to establish an initial baseline."
        )

    try:
        with open(baseline_path, "r", encoding="utf-8") as f:
            data = json.load(f)
    except Exception as exc:
        raise EnvironmentError(
            f"Failed to parse baseline JSON file at '{baseline_path}': {exc}"
        ) from exc

    if not isinstance(data, dict):
        raise EnvironmentError(
            f"Invalid baseline format in '{baseline_path}': expected top-level JSON object."
        )

    if "metrics" not in data or not isinstance(data["metrics"], dict):
        raise EnvironmentError(
            f"Invalid baseline schema in '{baseline_path}': missing top-level 'metrics' dictionary."
        )

    metrics_dict = data["metrics"]
    missing_keys = [k for k in REQUIRED_METRIC_KEYS if k not in metrics_dict]
    if missing_keys:
        raise EnvironmentError(
            f"Baseline schema in '{baseline_path}' missing mandatory metric key(s): {missing_keys}"
        )

    return data


def save_baseline(
    path: Union[str, Path],
    metrics: Dict[str, Any],
    metadata: Dict[str, Any],
) -> None:
    """
    Write canonical baseline JSON file adhering strictly to institutional schema.
    """
    baseline_path = Path(path)
    baseline_path.parent.mkdir(parents=True, exist_ok=True)

    created_utc = metadata.get(
        "created_utc", datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    )
    git_commit_sha = metadata.get("git_commit_sha", "UNKNOWN")
    model_dir = str(metadata.get("model_dir", "models/finbert-finetuned"))
    dataset_path = str(metadata.get("dataset_path", "config/model_validation_dataset.json"))
    dataset_size = int(metadata.get("dataset_size", 0))

    baseline_payload = {
        "baseline_version": metadata.get("baseline_version", "1.0.0"),
        "created_utc": created_utc,
        "git_commit_sha": git_commit_sha,
        "model_dir": model_dir,
        "dataset_path": dataset_path,
        "dataset_size": dataset_size,
        "metrics": {
            "accuracy": round(float(metrics.get("accuracy", 0.0)), 4),
            "f1_macro": round(float(metrics.get("f1_macro", 0.0)), 4),
            "f1_weighted": round(float(metrics.get("f1_weighted", 0.0)), 4),
            "ece_10bins": round(float(metrics.get("ece_10bins", 0.0)), 4),
            "latency_p50_ms": round(float(metrics.get("latency_p50_ms", 0.0)), 2),
            "latency_p95_ms": round(float(metrics.get("latency_p95_ms", 0.0)), 2),
            "latency_p99_ms": round(float(metrics.get("latency_p99_ms", 0.0)), 2),
        },
    }

    with open(baseline_path, "w", encoding="utf-8") as f:
        json.dump(baseline_payload, f, indent=2)
