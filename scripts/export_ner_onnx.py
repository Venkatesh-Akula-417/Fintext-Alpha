"""
═══════════════════════════════════════════════════════════════════════════════
FinText-Alpha-Vectorizer — Static Named Entity Recognition (NER) ONNX Exporter
═══════════════════════════════════════════════════════════════════════════════

Exports the pre-trained token classification NER model (dslim/bert-base-NER)
with static shape [1, 64] and Transformer optimizations for fast in-process
extraction of financial entities (ORG, PER, LOC, MISC).
═══════════════════════════════════════════════════════════════════════════════
"""

import os
from pathlib import Path
import sys

# Ensure UTF-8 stdout on Windows
if sys.stdout.encoding != "utf-8":
    sys.stdout.reconfigure(encoding="utf-8")
if sys.stderr.encoding != "utf-8":
    sys.stderr.reconfigure(encoding="utf-8")

os.environ["PYTHONIOENCODING"] = "utf-8"

import numpy as np
import onnx
import onnxruntime as ort
from onnxruntime.transformers.optimizer import optimize_model
import torch
import torch.nn as nn
from transformers import AutoModelForTokenClassification, AutoTokenizer

PROJECT_ROOT = Path(__file__).resolve().parent.parent
MODEL_DIR = PROJECT_ROOT / "models" / "ner"
OUTPUT_RAW_ONNX = MODEL_DIR / "model_static_raw.onnx"
OUTPUT_ONNX = MODEL_DIR / "model_static.onnx"
HF_MODEL_NAME = "dslim/bert-base-NER"
SEQ_LEN = 64


class StaticNERWrapper(nn.Module):
    """Wraps BERT token classifier with static 64-token position IDs for constant graph inference."""
    def __init__(self, model: nn.Module, seq_len: int = 64):
        super().__init__()
        self.model = model
        self.register_buffer("position_ids", torch.arange(seq_len, dtype=torch.long).unsqueeze(0))
        self.register_buffer("token_type_ids", torch.zeros((1, seq_len), dtype=torch.long))

    def forward(self, input_ids: torch.Tensor, attention_mask: torch.Tensor) -> torch.Tensor:
        outputs = self.model(
            input_ids=input_ids,
            attention_mask=attention_mask,
            token_type_ids=self.token_type_ids,
            position_ids=self.position_ids,
            return_dict=False,
        )
        return outputs[0]  # shape [batch_size, seq_len, 9]


def export_ner(batch_size: int = 1, seq_len: int = 64):
    MODEL_DIR.mkdir(parents=True, exist_ok=True)
    print(f"[NER ONNX Export] Loading NER model from HuggingFace: {HF_MODEL_NAME}")

    tokenizer = AutoTokenizer.from_pretrained(HF_MODEL_NAME)
    raw_model = AutoModelForTokenClassification.from_pretrained(HF_MODEL_NAME)
    raw_model.eval()

    print(f"[NER ONNX Export] Saving tokenizer assets to: {MODEL_DIR}")
    tokenizer.save_pretrained(str(MODEL_DIR))

    wrapper = StaticNERWrapper(raw_model, seq_len=seq_len).eval()

    print(f"[NER ONNX Export] Generating dummy static inputs [shape: {batch_size}, {seq_len}]...")
    dummy_input_ids = torch.zeros((batch_size, seq_len), dtype=torch.long)
    dummy_attention_mask = torch.ones((batch_size, seq_len), dtype=torch.long)

    with torch.no_grad():
        pt_logits = wrapper(dummy_input_ids, dummy_attention_mask).numpy()

    print(f"[NER ONNX Export] Step 1: Exporting raw static ONNX to: {OUTPUT_RAW_ONNX}")
    torch.onnx.export(
        wrapper,
        (dummy_input_ids, dummy_attention_mask),
        str(OUTPUT_RAW_ONNX),
        input_names=["input_ids", "attention_mask"],
        output_names=["logits"],
        dynamic_axes=None,  # Strictly static input shapes [1, 64]
        opset_version=14,
        do_constant_folding=True,
        dynamo=False,
    )

    print(f"[NER ONNX Export] Step 2: Applying ONNX Runtime Transformer Optimizations (num_heads=12, hidden_size=768)...")
    optimized_model = optimize_model(
        str(OUTPUT_RAW_ONNX),
        model_type="bert",
        num_heads=12,
        hidden_size=768,
        opt_level=99,
        use_gpu=False,
    )

    optimized_model.save_model_to_file(str(OUTPUT_ONNX))
    print(f"[NER ONNX Export] Step 3: Saved fully fused model to: {OUTPUT_ONNX}")

    if OUTPUT_RAW_ONNX.exists():
        try:
            OUTPUT_RAW_ONNX.unlink()
        except Exception:
            pass

    print("[NER ONNX Export] Validating ONNX model graph structure...")
    onnx_model = onnx.load(str(OUTPUT_ONNX))
    onnx.checker.check_model(onnx_model)

    print("[NER ONNX Export] Validating ONNX Runtime inference consistency...")
    session = ort.InferenceSession(str(OUTPUT_ONNX), providers=["CPUExecutionProvider"])
    ort_inputs = {
        "input_ids": dummy_input_ids.numpy(),
        "attention_mask": dummy_attention_mask.numpy(),
    }
    ort_outputs = session.run(["logits"], ort_inputs)
    ort_logits = ort_outputs[0]

    np.testing.assert_allclose(pt_logits, ort_logits, rtol=1e-3, atol=1e-4)
    print(f"[NER ONNX Export] Numerical consistency verified! Max diff: {np.max(np.abs(pt_logits - ort_logits)):.6f}")

    for inp in onnx_model.graph.input:
        shape = [dim.dim_value for dim in inp.type.tensor_type.shape.dim]
        print(f"  Input: {inp.name} -> Shape: {shape}")
    for out in onnx_model.graph.output:
        shape = [dim.dim_value for dim in out.type.tensor_type.shape.dim]
        print(f"  Output: {out.name} -> Shape: {shape}")

    print(f"\n[NER ONNX Export] SUCCESS! Static NER model saved at: {OUTPUT_ONNX}")


if __name__ == "__main__":
    export_ner(batch_size=1, seq_len=SEQ_LEN)
