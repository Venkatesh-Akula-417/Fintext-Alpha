"""
FinText-Alpha-Vectorizer — Model Inference Engine
Deterministic, single-threaded ONNX Runtime execution for FinBERT evaluation.
"""

from pathlib import Path
import time
from typing import Any, Dict, List, Optional, Tuple

import numpy as np

from scripts.model_validation.metrics import (
    CLASSES,
    IDX_TO_CLASS,
    normalize_label,
)


class ModelNotFoundError(FileNotFoundError):
    """Raised when required model artifacts are absent."""
    pass


class ModelInferenceEngine:
    """
    Manages deterministic ONNX Runtime inference and tokenizer tokenization
    for institutional model quality verification.
    """

    def __init__(self, model_dir: Path):
        self.model_dir = Path(model_dir)
        self.model_path = self._resolve_model_path()
        self._validate_artifacts()

        # Lazy import of ML libraries to allow clean handling of missing dependencies
        try:
            import onnxruntime as ort
            from transformers import AutoTokenizer
        except ImportError as exc:
            raise RuntimeError(
                f"Required ML dependency missing: {exc}. "
                "Ensure onnxruntime and transformers are installed."
            ) from exc

        # Set up deterministic, single-threaded ONNX Runtime session
        session_options = ort.SessionOptions()
        session_options.intra_op_num_threads = 1
        session_options.inter_op_num_threads = 1
        session_options.execution_mode = ort.ExecutionMode.ORT_SEQUENTIAL
        session_options.graph_optimization_level = (
            ort.GraphOptimizationLevel.ORT_ENABLE_ALL
        )

        self.session = ort.InferenceSession(
            str(self.model_path),
            sess_options=session_options,
            providers=["CPUExecutionProvider"],
        )
        self.input_names = [inp.name for inp in self.session.get_inputs()]
        self.tokenizer = AutoTokenizer.from_pretrained(str(self.model_dir))

    def _resolve_model_path(self) -> Path:
        """Find the ONNX model binary in model_dir."""
        primary = self.model_dir / "model.onnx"
        if primary.is_file():
            return primary
        fallback = self.model_dir / "finbert.onnx"
        if fallback.is_file():
            return fallback
        return primary

    def _validate_artifacts(self) -> None:
        """Ensure model binary and tokenizer config are present."""
        if not self.model_dir.is_dir():
            raise ModelNotFoundError(
                f"SKIP: model artifacts not present; directory '{self.model_dir}' does not exist."
            )
        if not self.model_path.is_file():
            raise ModelNotFoundError(
                f"SKIP: model artifacts not present; harness requires {self.model_dir}/model.onnx"
            )

        tokenizer_assets = [
            self.model_dir / "tokenizer.json",
            self.model_dir / "vocab.txt",
            self.model_dir / "config.json",
        ]
        if not any(p.is_file() for p in tokenizer_assets):
            raise ModelNotFoundError(
                f"SKIP: tokenizer assets not present in '{self.model_dir}'."
            )

    def predict_single(self, text: str) -> Dict[str, Any]:
        """
        Run inference on a single text string.
        Returns:
            {
                "predicted_label": "POSITIVE"|"NEGATIVE"|"NEUTRAL",
                "confidence": float,
                "probabilities": [pos, neg, neu],
                "latency_ms": float
            }
        """
        enc = self.tokenizer(
            text,
            return_tensors="np",
            padding="max_length",
            max_length=128,
            truncation=True,
        )

        feed = {
            "input_ids": enc["input_ids"].astype(np.int64),
            "attention_mask": enc["attention_mask"].astype(np.int64),
        }
        if "token_type_ids" in self.input_names:
            feed["token_type_ids"] = enc.get(
                "token_type_ids", np.zeros_like(enc["input_ids"])
            ).astype(np.int64)

        t0 = time.perf_counter()
        outputs = self.session.run(None, feed)
        latency_ms = (time.perf_counter() - t0) * 1000.0

        logits = outputs[0][0]
        # Softmax with numerical stability
        exp_l = np.exp(logits - np.max(logits))
        probs = exp_l / np.sum(exp_l)

        pred_idx = int(np.argmax(probs))
        predicted_label = IDX_TO_CLASS[pred_idx]
        confidence = float(probs[pred_idx])

        return {
            "predicted_label": predicted_label,
            "confidence": round(confidence, 6),
            "probabilities": [round(float(p), 6) for p in probs],
            "latency_ms": round(latency_ms, 3),
        }

    def evaluate_dataset(
        self, samples: List[Dict[str, Any]]
    ) -> Tuple[List[str], List[str], List[float], List[List[float]], List[float]]:
        """
        Deterministically evaluate all samples in dataset.
        Returns:
            (y_true, y_pred, confidences, probabilities_list, latencies_ms)
        """
        # Ensure deterministic sample ordering by sample id if present
        sorted_samples = sorted(
            samples, key=lambda s: (s.get("id", 0), str(s.get("text", "")))
        )

        y_true = []
        y_pred = []
        confidences = []
        probabilities_list = []
        latencies_ms = []

        for item in sorted_samples:
            text = str(item.get("text", "")).strip()
            raw_label = item.get("label")
            if not text or raw_label is None:
                continue

            true_label = normalize_label(raw_label)
            res = self.predict_single(text)

            y_true.append(true_label)
            y_pred.append(res["predicted_label"])
            confidences.append(res["confidence"])
            probabilities_list.append(res["probabilities"])
            latencies_ms.append(res["latency_ms"])

        return y_true, y_pred, confidences, probabilities_list, latencies_ms
