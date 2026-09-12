"""
FinText-Alpha-Vectorizer — Model Drift Monitoring Package
Continuous drift detection, baseline comparison, and alerting for FinBERT.
"""

from scripts.model_drift.baseline import load_baseline, save_baseline
from scripts.model_drift.drift import DriftThresholds, compute_drift
from scripts.model_drift.report import (
    generate_json_report,
    generate_markdown_report,
)

__all__ = [
    "DriftThresholds",
    "compute_drift",
    "generate_json_report",
    "generate_markdown_report",
    "load_baseline",
    "save_baseline",
]
