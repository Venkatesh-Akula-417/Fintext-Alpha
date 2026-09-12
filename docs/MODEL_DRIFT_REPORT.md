# FinText Alpha Vectorizer — FinBERT Model Drift Monitoring Report

> **Overall Status**: ✅ OK (No Significant Performance Drift Detected)
> **Monitoring Tier**: Institutional Continuous Quality Assurance
> **Evaluation Target**: Multi-Class Macro PRF, Expected Calibration Error (ECE), and Inference Latency vs Committed Baseline.

---

## 1. Executive Summary

The continuous drift monitoring engine evaluated **105 samples** against baseline snapshot `2026-09-12T10:23:47Z` (`5e2daa40`).

### Headline Metric Drift
- **Macro F1 Score**: **0.9802** (Delta: `+0.0000` | Status: **OK**)
- **Expected Calibration Error (ECE)**: **0.0095** (Delta: `+0.0000` | Status: **OK**)
- **Inference Latency P95**: **140.03 ms** (Change: `-2.47%` | Status: **OK**)
- **Overall Verdict**: **OK**

---

## 2. Baseline vs Current Comparison Table

| Metric | Baseline Value | Current Value | Absolute Delta | Relative Change | Drift Status | Monitored Thresholds |
| :--- | :---: | :---: | :---: | :---: | :---: | :--- |
| **Macro F1 Score** | 0.9802 | 0.9802 | +0.0000 | +0.00% | ✅ OK | Drop >= 0.03 (W) / >= 0.07 (C) |
| **Expected Calibration Error (ECE)** | 0.0095 | 0.0095 | +0.0000 | +0.00% | ✅ OK | Inc >= 0.02 (W) / >= 0.05 (C) |
| **Inference Latency P95** | 143.57 ms | 140.03 ms | -3.54 ms | -2.47% | ✅ OK | Inc >= +30% (W) / >= +60% (C) |
| **Overall Accuracy** | 0.9810 | 0.9810 | +0.0000 | +0.00% | Reference | Baseline tracking |
| **Weighted F1 Score** | 0.9810 | 0.9810 | +0.0000 | +0.00% | Reference | Baseline tracking |
| **Inference Latency P50 (Median)** | 132.94 ms | 127.66 ms | -5.28 ms | — | Reference | Baseline tracking |
| **Inference Latency P99** | 168.23 ms | 143.03 ms | -25.20 ms | — | Reference | Baseline tracking |

---

## 3. Per-Metric Drift Detail

- **Classification Macro F1**: Baseline was `0.9802`; Current is `0.9802` (Absolute change: `+0.0000`). Status is `OK`.
- **Expected Calibration Error**: Baseline was `0.0095`; Current is `0.0095` (Absolute change: `+0.0000`). Status is `OK`.
- **Latency P95 Distribution**: Baseline was `143.57 ms`; Current is `140.03 ms` (`-2.47%`). Status is `OK`.

---

## 4. Breaches and Degradation Alerts

None detected. All monitored performance indicators operate within nominal variance tolerance.

---

## 5. One-Line Reproduction Command

```bash
python scripts/model_drift_monitor.py --dataset config/model_validation_dataset.json --model-dir models/finbert-finetuned --baseline data/model-drift/baseline.json --output-dir data/model-drift --report-path docs/MODEL_DRIFT_REPORT.md --f1-drop-warn 0.03 --f1-drop-critical 0.07 --ece-increase-warn 0.02 --ece-increase-critical 0.05 --latency-p95-pct-warn 30.0 --latency-p95-pct-critical 60.0
```

---

## 6. Honesty Note

This drift certification was executed deterministically on local CPU runtime without external network calls. All metrics and latencies reflect exact reproducibility against the committed baseline dataset.
