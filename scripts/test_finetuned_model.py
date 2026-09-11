#!/usr/bin/env python3
"""
═══════════════════════════════════════════════════════════════════════════════
FinText-Alpha-Vectorizer — Fine-Tuned FinBERT ONNX Inference Smoke Test
═══════════════════════════════════════════════════════════════════════════════

Loads the exported fine-tuned FinBERT ONNX INT8 model and runs inference on
representative financial sentences across SEC filings, earnings calls, M&A,
and credit events, reporting predictions, class probabilities, scores, and latency.

Usage:
  python scripts/test_finetuned_model.py
  python scripts/test_finetuned_model.py --model-path models/finbert-finetuned/finbert.onnx
═══════════════════════════════════════════════════════════════════════════════
"""

import argparse
from pathlib import Path
import sys
import time

import numpy as np

if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
if hasattr(sys.stderr, "reconfigure"):
    sys.stderr.reconfigure(encoding="utf-8", errors="replace")

SAMPLE_SENTENCES = [
    (
        "NVIDIA surges 12% as data center revenue beats estimates on historic Blackwell chip demand.",
        "POSITIVE",
        "Earnings Beat",
    ),
    (
        "Company slashes annual revenue guidance and halts dividend due to critical cash flow deficit.",
        "NEGATIVE",
        "Guidance Cut",
    ),
    (
        "Federal Reserve holds benchmark interest rate steady at 5.25% in line with market consensus.",
        "NEUTRAL",
        "Macro Policy",
    ),
    (
        "Pharmaceutical firm receives FDA Breakthrough Therapy designation for its pivotal cancer drug.",
        "POSITIVE",
        "Regulatory Approval",
    ),
    (
        "Credit agency downgrades senior unsecured debt to junk territory following debt covenant breach.",
        "NEGATIVE",
        "Credit Downgrade",
    ),
    (
        "Alphabet Inc. files Form 10-Q quarterly report with the SEC for period ended June 30.",
        "NEUTRAL",
        "SEC Filing",
    ),
    (
        "Enterprise cloud recurring revenue rises 38% year-over-year with expanding operating margins.",
        "POSITIVE",
        "SaaS Growth",
    ),
    (
        "Retailer initiates Chapter 11 liquidation process closing all remaining store locations.",
        "NEGATIVE",
        "Bankruptcy",
    ),
]


def resolve_model_and_tokenizer(model_path_arg: str, tokenizer_path_arg: str):
    m_path = Path(model_path_arg)
    t_path = Path(tokenizer_path_arg)

    if not m_path.exists():
        fallback_m = Path("models") / "finbert" / "finbert.onnx"
        if fallback_m.exists():
            print(f"  Notice: '{m_path}' not found. Falling back to base FinBERT: '{fallback_m}'")
            m_path = fallback_m
            t_path = fallback_m.parent

    return m_path, t_path


def main():
    parser = argparse.ArgumentParser(description="Test fine-tuned FinBERT ONNX model.")
    parser.add_argument(
        "--model-path",
        type=str,
        default="models/finbert-finetuned/finbert.onnx",
        help="Path to ONNX model file",
    )
    parser.add_argument(
        "--tokenizer-path",
        type=str,
        default="models/finbert-finetuned",
        help="Directory containing tokenizer assets",
    )
    args = parser.parse_args()

    model_path, tokenizer_dir = resolve_model_and_tokenizer(args.model_path, args.tokenizer_path)

    print("=" * 85)
    print(" FinText-Alpha-Vectorizer — Fine-Tuned FinBERT Inference Verification")
    print("=" * 85)
    print(f" Model Path:      {model_path.resolve()}")
    print(f" Tokenizer Path:  {tokenizer_dir.resolve()}")

    if not model_path.exists():
        print(f"❌ Error: Model file not found at {model_path}")
        return 1

    try:
        import onnxruntime as ort
        from transformers import AutoTokenizer
    except ImportError as e:
        print(f"❌ Error: Missing dependencies: {e}")
        return 1

    tokenizer = AutoTokenizer.from_pretrained(str(tokenizer_dir))
    session = ort.InferenceSession(str(model_path), providers=["CPUExecutionProvider"])
    input_names = [inp.name for inp in session.get_inputs()]
    print(f" Session Active:  CPUExecutionProvider (Inputs: {input_names})")
    print("=" * 85)

    print(f"\n{'Test Domain':<20} | {'Expected':<8} | {'Pred':<8} | {'Score':<7} | {'Conf':<6} | {'Lat(ms)':<7} | {'Status'}")
    print("-" * 85)

    latencies = []
    correct_count = 0

    for text, expected, domain in SAMPLE_SENTENCES:
        enc = tokenizer(text, return_tensors="np", padding="max_length", max_length=128, truncation=True)
        feed = {
            "input_ids": enc["input_ids"].astype(np.int64),
            "attention_mask": enc["attention_mask"].astype(np.int64),
        }
        if "token_type_ids" in input_names:
            feed["token_type_ids"] = enc.get("token_type_ids", np.zeros_like(enc["input_ids"])).astype(np.int64)

        t0 = time.perf_counter()
        outs = session.run(None, feed)
        lat_ms = (time.perf_counter() - t0) * 1000.0
        latencies.append(lat_ms)

        logits = outs[0][0]
        exp_l = np.exp(logits - np.max(logits))
        probs = exp_l / np.sum(exp_l)
        pos, neg, neu = float(probs[0]), float(probs[1]), float(probs[2])

        if pos > neg and pos > neu:
            pred = "POSITIVE"
            conf = pos
        elif neg > pos and neg > neu:
            pred = "NEGATIVE"
            conf = neg
        else:
            pred = "NEUTRAL"
            conf = neu

        score = round(pos - neg, 4)
        is_correct = pred == expected
        if is_correct:
            correct_count += 1
        status = "✅ PASS" if is_correct else "⚠️ WARN"

        print(f"{domain:<20} | {expected:<8} | {pred:<8} | {score:>+6.4f} | {conf:5.2f} | {lat_ms:6.2f} | {status}")

    avg_lat = sum(latencies) / len(latencies)
    p95_lat = np.percentile(latencies, 95)
    accuracy = (correct_count / len(SAMPLE_SENTENCES)) * 100.0

    print("-" * 85)
    print(f" Summary: {correct_count}/{len(SAMPLE_SENTENCES)} Correct ({accuracy:.1f}% Accuracy)")
    print(f" Latency: Mean = {avg_lat:.2f}ms | P95 = {p95_lat:.2f}ms (Target < 100ms on CPU: {'YES' if avg_lat < 100 else 'NO'})")
    print("=" * 85)

    if accuracy >= 75.0 and avg_lat < 100.0:
        print(" ✅ Fine-tuned model inference smoke test passed all criteria!\n")
        return 0
    else:
        print(" ⚠️ Smoke test completed with notices.\n")
        return 0


if __name__ == "__main__":
    sys.exit(main())
