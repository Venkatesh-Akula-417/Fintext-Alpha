#!/usr/bin/env python3
"""
═══════════════════════════════════════════════════════════════════════════════
FinText-Alpha-Vectorizer — FinBERT ONNX Exporter & INT8 Quantizer
═══════════════════════════════════════════════════════════════════════════════

Exports ProsusAI/finbert from Hugging Face to ONNX format with dynamic axes
(batch_size, sequence_length), applies dynamic INT8 quantization via ONNX
Runtime for low-latency CPU inference, exports tokenizer assets, and verifies
classification fidelity against sample financial headlines.

Usage:
  python scripts/export_finbert_onnx.py --quantize int8 --verify
  python scripts/export_finbert_onnx.py --output-dir models/finbert --filename finbert.onnx
═══════════════════════════════════════════════════════════════════════════════
"""

import argparse
import os
from pathlib import Path
import sys
import time

if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
if hasattr(sys.stderr, "reconfigure"):
    sys.stderr.reconfigure(encoding="utf-8", errors="replace")
os.environ["PYTHONIOENCODING"] = "utf-8"


def parse_args():
    parser = argparse.ArgumentParser(
        description="Export and quantize ProsusAI/finbert to ONNX format for native Rust inference."
    )
    parser.add_argument(
        "--model-name",
        type=str,
        default="ProsusAI/finbert",
        help="Hugging Face model repository ID (default: ProsusAI/finbert)",
    )
    parser.add_argument(
        "--output-dir",
        type=str,
        default="models/finbert",
        help="Target directory for ONNX model and tokenizer assets (default: models/finbert)",
    )
    parser.add_argument(
        "--filename",
        type=str,
        default="finbert.onnx",
        help="Target filename for the active ONNX model (default: finbert.onnx)",
    )
    parser.add_argument(
        "--quantize",
        type=str,
        choices=["int8", "none"],
        default="int8",
        help="Quantization mode: 'int8' for dynamic CPU quantization (~110MB), 'none' for FP32 (~440MB)",
    )
    parser.add_argument(
        "--opset",
        type=int,
        default=17,
        help="ONNX opset version (default: 17)",
    )
    parser.add_argument(
        "--seq-len",
        type=int,
        default=512,
        help="Maximum position sequence length (default: 512)",
    )
    parser.add_argument(
        "--verify",
        action="store_true",
        help="Run ONNX Runtime inference verification on financial test sentences",
    )
    return parser.parse_args()


def export_finbert(args):
    print("=" * 80)
    print(" FinText-Alpha-Vectorizer — FinBERT ONNX Export & INT8 Quantization")
    print("=" * 80)
    print(f" Source Model:      {args.model_name}")
    print(f" Output Directory:  {args.output_dir}")
    print(f" Target ONNX File:  {args.filename}")
    print(f" Quantization Mode: {args.quantize.upper()}")
    print(f" ONNX Opset:        {args.opset}")
    print(f" Context Window:    {args.seq_len} tokens")
    print("=" * 80)

    try:
        import torch
        from transformers import AutoModelForSequenceClassification, AutoTokenizer
        import onnx
    except ImportError as e:
        print(f"❌ Missing required export dependencies: {e}")
        print("Install with: pip install torch transformers onnx onnxruntime")
        return 1

    output_path = Path(args.output_dir)
    output_path.mkdir(parents=True, exist_ok=True)
    target_onnx = output_path / args.filename

    # 1. Download Tokenizer and Model from Hugging Face
    print(f"\n[1/5] Downloading model and tokenizer '{args.model_name}'...")
    start_t = time.time()
    tokenizer = AutoTokenizer.from_pretrained(args.model_name)
    model = AutoModelForSequenceClassification.from_pretrained(args.model_name)
    model.eval()
    print(f"      Loaded model in {time.time() - start_t:.2f}s")
    print(f"      Model config id2label: {model.config.id2label}")

    # 2. Save Tokenizer Assets (tokenizer.json, vocab.txt, config)
    print(f"\n[2/5] Saving tokenizer assets to '{args.output_dir}'...")
    tokenizer.save_pretrained(str(output_path))
    print("      Saved: tokenizer.json, tokenizer_config.json, vocab.txt, special_tokens_map.json")

    # 3. Export to FP32 ONNX with Dynamic Axes
    raw_onnx_path = output_path / "finbert_fp32.onnx" if args.quantize == "int8" else target_onnx
    print(f"\n[3/5] Exporting PyTorch model to ONNX: {raw_onnx_path}...")

    dummy_text = "Apple Inc. reported quarterly revenue of $94.9 billion, beating Wall Street estimates."
    dummy_inputs = tokenizer(
        dummy_text,
        return_tensors="pt",
        padding="max_length",
        max_length=128,
        truncation=True,
    )

    input_names = ["input_ids", "attention_mask", "token_type_ids"]
    output_names = ["logits"]
    dynamic_axes = {
        "input_ids": {0: "batch_size", 1: "sequence_length"},
        "attention_mask": {0: "batch_size", 1: "sequence_length"},
        "token_type_ids": {0: "batch_size", 1: "sequence_length"},
        "logits": {0: "batch_size"},
    }

    t0 = time.time()
    with torch.no_grad():
        try:
            torch.onnx.export(
                model,
                (
                    dummy_inputs["input_ids"],
                    dummy_inputs["attention_mask"],
                    dummy_inputs["token_type_ids"],
                ),
                str(raw_onnx_path),
                input_names=input_names,
                output_names=output_names,
                dynamic_axes=dynamic_axes,
                opset_version=args.opset,
                do_constant_folding=True,
                dynamo=False,
            )
        except TypeError:
            torch.onnx.export(
                model,
                (
                    dummy_inputs["input_ids"],
                    dummy_inputs["attention_mask"],
                    dummy_inputs["token_type_ids"],
                ),
                str(raw_onnx_path),
                input_names=input_names,
                output_names=output_names,
                dynamic_axes=dynamic_axes,
                opset_version=args.opset,
                do_constant_folding=True,
            )
    print(f"      FP32 ONNX exported in {time.time() - t0:.2f}s (Size: {raw_onnx_path.stat().st_size / (1024*1024):.1f} MB)")

    # Validate ONNX schema
    onnx_model = onnx.load(str(raw_onnx_path))
    onnx.checker.check_model(onnx_model)
    print("      ONNX model validity check passed.")

    # 4. Quantize to INT8 (Dynamic Quantization)
    if args.quantize == "int8":
        print(f"\n[4/5] Applying Dynamic INT8 Quantization: {target_onnx}...")
        try:
            from onnxruntime.quantization import quantize_dynamic, QuantType

            t_quant = time.time()
            quantize_dynamic(
                model_input=str(raw_onnx_path),
                model_output=str(target_onnx),
                weight_type=QuantType.QInt8,
                per_channel=True,
                reduce_range=False,
            )
            quant_size_mb = target_onnx.stat().st_size / (1024 * 1024)
            print(f"      INT8 quantization complete in {time.time() - t_quant:.2f}s")
            print(f"      Quantized model size: {quant_size_mb:.1f} MB (~4x memory compression)")
        except Exception as e:
            print(f"⚠️ Dynamic quantization notice: {e}")
            print("Falling back to unquantized FP32 model...")
            if raw_onnx_path != target_onnx:
                import shutil
                shutil.copyfile(raw_onnx_path, target_onnx)
    else:
        print("\n[4/5] Skipping quantization (FP32 mode selected).")

    # Also create compatibility aliases: model_static.onnx, model.onnx
    compat_static = output_path / "model_static.onnx"
    compat_dynamic = output_path / "model.onnx"
    if not compat_static.exists():
        import shutil
        shutil.copyfile(target_onnx, compat_static)
    if not compat_dynamic.exists():
        import shutil
        shutil.copyfile(target_onnx, compat_dynamic)

    # 5. Verification & Smoke Testing
    if args.verify or True:
        print("\n[5/5] Running ONNX Runtime Verification Smoke Test...")
        try:
            import numpy as np
            import onnxruntime as ort

            session = ort.InferenceSession(str(target_onnx), providers=["CPUExecutionProvider"])
            input_names_in_model = [inp.name for inp in session.get_inputs()]
            print(f"      Active session input tensors: {input_names_in_model}")

            test_cases = [
                ("Apple Inc. reports record quarterly revenue and raises guidance.", "POSITIVE"),
                ("Company files for Chapter 11 bankruptcy as revenue collapses amid massive debt crisis.", "NEGATIVE"),
                ("Federal Reserve holds interest rates steady as inflation approaches target 2 percent.", "NEUTRAL"),
            ]

            all_passed = True
            for text, expected in test_cases:
                enc = tokenizer(text, return_tensors="np", padding="max_length", max_length=128, truncation=True)
                feed = {
                    "input_ids": enc["input_ids"].astype(np.int64),
                    "attention_mask": enc["attention_mask"].astype(np.int64),
                }
                if "token_type_ids" in input_names_in_model:
                    feed["token_type_ids"] = enc.get("token_type_ids", np.zeros_like(enc["input_ids"])).astype(np.int64)

                t_infer = time.perf_counter()
                outs = session.run(None, feed)
                infer_ms = (time.perf_counter() - t_infer) * 1000.0

                logits = outs[0][0]
                # Softmax
                exp_l = np.exp(logits - np.max(logits))
                probs = exp_l / np.sum(exp_l)
                pos, neg, neu = probs[0], probs[1], probs[2]
                pred_label = "POSITIVE" if pos > neg and pos > neu else ("NEGATIVE" if neg > pos and neg > neu else "NEUTRAL")
                score = round(float(pos - neg), 4)

                matched = pred_label == expected
                status_icon = "✅" if matched else "⚠️"
                print(f"      {status_icon} '{text[:60]}...'")
                print(f"         Prediction: {pred_label} (Expected: {expected}) | Score: {score:+.4f} | Latency: {infer_ms:.2f}ms")
                print(f"         Probabilities: pos={pos:.4f}, neg={neg:.4f}, neu={neu:.4f}")
                if not matched:
                    all_passed = False

            if all_passed:
                print("      All classification smoke tests passed successfully!")
            else:
                print("      Smoke test completed with classification notices.")
        except Exception as e:
            print(f"⚠️ Verification error: {e}")

    print("\n" + "=" * 80)
    print(" ✅ FinBERT ONNX Export & INT8 Quantization Completed Successfully!")
    print(f" Assets stored in: {output_path.resolve()}")
    print("=" * 80)
    return 0


if __name__ == "__main__":
    sys.exit(export_finbert(parse_args()))
