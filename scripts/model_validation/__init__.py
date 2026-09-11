"""
FinText-Alpha-Vectorizer — Model Quality & Calibration Validation Package
Institutional verification harness for FinBERT INT8 ONNX models.
"""

from scripts.model_validation.inference import (
    ModelInferenceEngine,
    ModelNotFoundError,
)
from scripts.model_validation.metrics import (
    CLASSES,
    compute_calibration_error,
    compute_classification_metrics,
    evaluate_metrics_against_thresholds,
)
from scripts.model_validation.report import (
    generate_json_report,
    generate_markdown_report,
    get_git_commit_sha,
)

__all__ = [
    "CLASSES",
    "ModelInferenceEngine",
    "ModelNotFoundError",
    "compute_calibration_error",
    "compute_classification_metrics",
    "evaluate_metrics_against_thresholds",
    "generate_json_report",
    "generate_markdown_report",
    "get_git_commit_sha",
]
