# FinText Alpha Vectorizer — Latency Reconciliation

## 1. Purpose

This document provides a technical reconciliation between the benchmark latency claims recorded in `FULL_PROJECT_AUDIT.md` Section 11.1 and the empirical latency measurements captured during model quality validation in `scripts/validate_model_quality.py`.

During institutional verification of the FinBERT INT8 ONNX sentiment scoring model on host execution environments (Windows CPU / GitHub Actions CI runners), measured single-sample inference latency (P95 ~146.80 ms) materially diverged from the published audit benchmark (0.85 ms, < 2.0 ms SLA). This document catalogs the discrepancies, analyzes the underlying architectural factors, provides exact reproduction commands, and defines required reconciliation actions.

---

## 2. Claims vs Measured Latency Comparison

Benchmark claims extracted directly from `FULL_PROJECT_AUDIT.md` Section 11.1 evaluated against empirical measurements from the validation harness:

| Stage | Audit Claim | Measured (this repo, Windows CPU) | Reproducible? | Notes |
| :--- | :---: | :---: | :---: | :--- |
| **FinBERT Sentiment Scoring** | 0.85 ms | 146.80 ms P95 | **NO** | see note A |
| **Token NER Entity Extraction** | 1.10 ms | not tested | **UNKNOWN** | NER harness pending |
| **HTML Sanitization** | 0.35 ms | not tested | **UNKNOWN** | microbenchmark pending |
| **Acoustic Vocal Stress DSP** | 0.90 ms | not tested | **UNKNOWN** | audio harness pending |
| **Total Ingestion-to-Signal Pipeline** | < 4.5 ms | >= 146.8 ms (model alone) | **NO** | see note B |

---

## 3. Notes

- **Note A**: Empirical measurement from `scripts/validate_model_quality.py` on Windows CPU with `onnxruntime` `CPUExecutionProvider`, `intra_op_num_threads=1`, `inter_op_num_threads=1`, `max_length=128`. Values reproduce within ±5% across repeated runs.
- **Note B**: The pipeline claim is mathematically incompatible with the observed single-model latency. Even with zero overhead elsewhere, the model alone exceeds the pipeline claim by a factor of ~30×.

---

## 4. Required Actions to Close the Discrepancy

To reconcile the audit claims with empirical evidence, the following engineering actions are identified:

- **(a) Audit Report Update**: Update `FULL_PROJECT_AUDIT.md` Section 11.1 to reflect reproducible CPU values (or distinguish between CPU and GPU/accelerated runtime figures) (author action).
- **(b) Model & Runtime Optimization**: Optimize the model and runtime execution stack (e.g., CUDA/TensorRT execution provider, static sequence length truncation to `[1, 32]`, batched dispatch, OpenVINO / DirectML acceleration, or INT4/INT8 quantization tuning) and re-measure under controlled benchmarks.
- **(c) Hardware Context Documentation**: Confirm and document the exact specialized hardware environment that produced the audit figures (e.g., AMD EPYC 7763 / Intel Xeon Platinum 8380 with multi-socket AVX-512 VNNI acceleration or GPU offload) and specify the precise tokenizer/model static shape constraints used in those tests.

---

## 5. Reproduction Command

To reproduce the empirical single-sample inference latency measurements locally:

```bash
python scripts/validate_model_quality.py --dataset config/model_validation_dataset.json --model-dir models/finbert-finetuned --output-dir data/model-validation --report-path docs/MODEL_VALIDATION_REPORT.md --max-latency-p95-ms 250.0 --environment cpu-local
```

For CI runner execution:

```bash
python scripts/validate_model_quality.py --dataset config/model_validation_dataset.json --model-dir models/finbert-finetuned --output-dir data/model-validation --report-path docs/MODEL_VALIDATION_REPORT.md --max-latency-p95-ms 250.0 --environment cpu-ci
```

---

## 6. Status

**OPEN — PENDING AUDIT AUTHOR RECONCILIATION**
