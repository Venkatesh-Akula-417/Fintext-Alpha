#!/usr/bin/env python3
"""
═══════════════════════════════════════════════════════════════════════════════
FinText-Alpha-Vectorizer — FinBERT Domain Fine-Tuning Pipeline
═══════════════════════════════════════════════════════════════════════════════

Fine-tunes ProsusAI/finbert for 3-class sequence classification (positive,
negative, neutral) on FinText's domain-specific financial dataset (SEC filings,
earnings transcripts, and labeled news).

Outputs:
  models/finbert-finetuned/model.safetensors (or pytorch_model.bin)
  models/finbert-finetuned/config.json
  models/finbert-finetuned/tokenizer.json
  models/finbert-finetuned/vocab.txt
  models/finbert-finetuned/eval_metrics.json

Usage:
  python scripts/finetune_finbert.py --epochs 3 --batch-size 16 --lr 2e-5
  python scripts/finetune_finbert.py --quick  # 1-epoch fast verification mode
═══════════════════════════════════════════════════════════════════════════════
"""

import argparse
import json
import os
from pathlib import Path
import random
import sys
import time

import numpy as np
import torch
from torch.utils.data import DataLoader, Dataset
from transformers import (
    AutoConfig,
    AutoModelForSequenceClassification,
    AutoTokenizer,
    get_linear_schedule_with_warmup,
)

if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
if hasattr(sys.stderr, "reconfigure"):
    sys.stderr.reconfigure(encoding="utf-8", errors="replace")

LABEL_TO_ID = {"positive": 0, "negative": 1, "neutral": 2}
ID_TO_LABEL = {0: "positive", 1: "negative", 2: "neutral"}


class FinancialTextDataset(Dataset):
    def __init__(self, jsonl_path: str, tokenizer, max_length: int = 512, max_samples: int = None):
        self.samples = []
        with open(jsonl_path, "r", encoding="utf-8") as f:
            for line in f:
                if line.strip():
                    item = json.loads(line)
                    self.samples.append(item)
                    if max_samples and len(self.samples) >= max_samples:
                        break

        self.tokenizer = tokenizer
        self.max_length = max_length

    def __len__(self):
        return len(self.samples)

    def __getitem__(self, idx):
        item = self.samples[idx]
        text = item["text"]
        label_id = item.get("label_id", LABEL_TO_ID.get(item.get("label", "neutral"), 2))

        enc = self.tokenizer(
            text,
            max_length=self.max_length,
            padding="max_length",
            truncation=True,
            return_tensors="pt",
        )

        res = {
            "input_ids": enc["input_ids"].squeeze(0),
            "attention_mask": enc["attention_mask"].squeeze(0),
            "label": torch.tensor(label_id, dtype=torch.long),
        }
        if "token_type_ids" in enc:
            res["token_type_ids"] = enc["token_type_ids"].squeeze(0)
        return res


def compute_metrics(y_true: list, y_pred: list) -> dict:
    total = len(y_true)
    if total == 0:
        return {"accuracy": 0.0, "macro_f1": 0.0}

    correct = sum(1 for t, p in zip(y_true, y_pred) if t == p)
    accuracy = correct / total

    precisions = []
    recalls = []
    f1s = []
    per_class = {}

    for c in [0, 1, 2]:
        name = ID_TO_LABEL[c]
        tp = sum(1 for t, p in zip(y_true, y_pred) if t == c and p == c)
        fp = sum(1 for t, p in zip(y_true, y_pred) if t != c and p == c)
        fn = sum(1 for t, p in zip(y_true, y_pred) if t == c and p != c)
        support = sum(1 for t in y_true if t == c)

        prec = tp / (tp + fp) if (tp + fp) > 0 else 0.0
        rec = tp / (tp + fn) if (tp + fn) > 0 else 0.0
        f1 = (2 * prec * rec) / (prec + rec) if (prec + rec) > 0 else 0.0

        precisions.append(prec)
        recalls.append(rec)
        f1s.append(f1)
        per_class[name] = {
            "precision": round(prec, 4),
            "recall": round(rec, 4),
            "f1_score": round(f1, 4),
            "support": support,
        }

    macro_p = sum(precisions) / len(precisions)
    macro_r = sum(recalls) / len(recalls)
    macro_f1 = sum(f1s) / len(f1s)

    return {
        "accuracy": round(accuracy, 4),
        "macro_precision": round(macro_p, 4),
        "macro_recall": round(macro_r, 4),
        "macro_f1": round(macro_f1, 4),
        "per_class": per_class,
    }


def evaluate(model, dataloader, device):
    model.eval()
    all_preds = []
    all_targets = []
    total_loss = 0.0

    criterion = torch.nn.CrossEntropyLoss()
    with torch.no_grad():
        for batch in dataloader:
            input_ids = batch["input_ids"].to(device)
            attention_mask = batch["attention_mask"].to(device)
            labels = batch["label"].to(device)

            token_type_ids = batch.get("token_type_ids")
            if token_type_ids is not None:
                token_type_ids = token_type_ids.to(device)
                outputs = model(input_ids=input_ids, attention_mask=attention_mask, token_type_ids=token_type_ids)
            else:
                outputs = model(input_ids=input_ids, attention_mask=attention_mask)

            logits = outputs.logits
            loss = criterion(logits, labels)
            total_loss += loss.item()

            preds = torch.argmax(logits, dim=-1).cpu().tolist()
            all_preds.extend(preds)
            all_targets.extend(labels.cpu().tolist())

    metrics = compute_metrics(all_targets, all_preds)
    metrics["eval_loss"] = round(total_loss / len(dataloader), 4) if len(dataloader) > 0 else 0.0
    return metrics


def find_model_source(preferred_name: str) -> str:
    """Check for local offline model directory or use Hugging Face repo ID."""
    local_candidates = [
        Path("models") / "finbert",
        Path("..") / "models" / "finbert",
        Path(preferred_name),
    ]
    for cand in local_candidates:
        if cand.exists() and (cand / "tokenizer.json").exists() and (cand / "config.json").exists():
            print(f"  [Model Loader] Found local checkpoint at: {cand.resolve()}")
            return str(cand)
    return preferred_name


def main():
    parser = argparse.ArgumentParser(description="Fine-tune FinBERT on financial domain data.")
    parser.add_argument("--model-name-or-path", type=str, default="ProsusAI/finbert", help="Base model ID or path")
    parser.add_argument("--train-file", type=str, default="data/finetune_train.jsonl", help="Path to train JSONL")
    parser.add_argument("--eval-file", type=str, default="data/finetune_eval.jsonl", help="Path to eval JSONL")
    parser.add_argument("--output-dir", type=str, default="models/finbert-finetuned", help="Directory for saved model")
    parser.add_argument("--epochs", type=int, default=3, help="Number of training epochs")
    parser.add_argument("--batch-size", type=int, default=16, help="Batch size for training")
    parser.add_argument("--lr", type=float, default=2e-5, help="Learning rate")
    parser.add_argument("--max-seq-len", type=int, default=512, help="Max sequence length")
    parser.add_argument("--seed", type=int, default=42, help="Random seed")
    parser.add_argument("--device", type=str, default="auto", choices=["auto", "cuda", "cpu"], help="Compute device")
    parser.add_argument("--quick", action="store_true", help="Fast test verification mode (1 epoch, 100 samples)")
    args = parser.parse_args()

    print("=" * 80)
    print(" FinText-Alpha-Vectorizer — FinBERT Domain Fine-Tuning Pipeline")
    print("=" * 80)
    print(f" Base Model:        {args.model_name_or_path}")
    print(f" Train File:        {args.train_file}")
    print(f" Eval File:         {args.eval_file}")
    print(f" Output Directory:  {args.output_dir}")
    print(f" Epochs:            {1 if args.quick else args.epochs}")
    print(f" Batch Size:        {args.batch_size}")
    print(f" Learning Rate:     {args.lr}")
    print(f" Context Window:    {args.max_seq_len} tokens")
    print(f" Quick Test Mode:   {args.quick}")
    print("=" * 80)

    # Set seeds
    random.seed(args.seed)
    np.random.seed(args.seed)
    torch.manual_seed(args.seed)
    if torch.cuda.is_available():
        torch.cuda.manual_seed_all(args.seed)

    # Device selection
    if args.device == "cuda" or (args.device == "auto" and torch.cuda.is_available()):
        device = torch.device("cuda")
        print(f"  [Device] Using GPU: {torch.cuda.get_device_name(0)}")
    else:
        device = torch.device("cpu")
        print("  [Device] Using CPU Execution Engine")

    out_path = Path(args.output_dir)
    out_path.mkdir(parents=True, exist_ok=True)

    # Ensure train and eval files exist; if not, invoke prepare_finetune_data
    if not Path(args.train_file).exists() or not Path(args.eval_file).exists():
        print("  Training/eval data missing. Running prepare_finetune_data.py first...")
        from prepare_finetune_data import main as prep_main
        prep_main()

    # 1. Load Tokenizer & Model
    source = find_model_source(args.model_name_or_path)
    print(f"\n[1/4] Loading tokenizer and model configuration from '{source}'...")
    try:
        tokenizer = AutoTokenizer.from_pretrained(source)
    except Exception as e:
        print(f"  Notice: Falling back to online repo {args.model_name_or_path}: {e}")
        tokenizer = AutoTokenizer.from_pretrained(args.model_name_or_path)

    config = AutoConfig.from_pretrained(
        source,
        num_labels=3,
        id2label=ID_TO_LABEL,
        label2id=LABEL_TO_ID,
    )

    try:
        model = AutoModelForSequenceClassification.from_pretrained(source, config=config)
    except Exception:
        model = AutoModelForSequenceClassification.from_pretrained(args.model_name_or_path, config=config)

    model.to(device)
    print(f"      Model parameter count: {sum(p.numel() for p in model.parameters()):,}")

    # 2. Datasets and Loaders
    max_train = 100 if args.quick else None
    max_eval = 50 if args.quick else None
    epochs = 1 if args.quick else args.epochs

    train_dataset = FinancialTextDataset(args.train_file, tokenizer, max_length=args.max_seq_len, max_samples=max_train)
    eval_dataset = FinancialTextDataset(args.eval_file, tokenizer, max_length=args.max_seq_len, max_samples=max_eval)

    train_loader = DataLoader(train_dataset, batch_size=args.batch_size, shuffle=True)
    eval_loader = DataLoader(eval_dataset, batch_size=args.batch_size, shuffle=False)
    print(f"      Train samples: {len(train_dataset)} ({len(train_loader)} batches)")
    print(f"      Eval samples:  {len(eval_dataset)} ({len(eval_loader)} batches)")

    # 3. Optimizer and Scheduler
    optimizer = torch.optim.AdamW(model.parameters(), lr=args.lr, weight_decay=0.01)
    total_steps = len(train_loader) * epochs
    warmup_steps = int(total_steps * 0.10)
    scheduler = get_linear_schedule_with_warmup(optimizer, num_warmup_steps=warmup_steps, num_training_steps=total_steps)
    criterion = torch.nn.CrossEntropyLoss()

    # Pre-training Baseline Evaluation
    print("\n[2/4] Evaluating initial baseline model on eval dataset...")
    base_metrics = evaluate(model, eval_loader, device)
    print(f"      Initial Accuracy: {base_metrics['accuracy']:.1%}, Macro F1: {base_metrics['macro_f1']:.4f}")

    # 4. Training Loop
    print(f"\n[3/4] Training for {epochs} epoch(s)...")
    best_macro_f1 = base_metrics["macro_f1"]
    best_metrics = base_metrics

    for epoch in range(1, epochs + 1):
        model.train()
        epoch_loss = 0.0
        t0 = time.time()

        for step, batch in enumerate(train_loader, 1):
            input_ids = batch["input_ids"].to(device)
            attention_mask = batch["attention_mask"].to(device)
            labels = batch["label"].to(device)

            optimizer.zero_grad()
            token_type_ids = batch.get("token_type_ids")
            if token_type_ids is not None:
                token_type_ids = token_type_ids.to(device)
                outputs = model(input_ids=input_ids, attention_mask=attention_mask, token_type_ids=token_type_ids)
            else:
                outputs = model(input_ids=input_ids, attention_mask=attention_mask)

            loss = criterion(outputs.logits, labels)
            loss.backward()
            torch.nn.utils.clip_grad_norm_(model.parameters(), max_norm=1.0)
            optimizer.step()
            scheduler.step()

            epoch_loss += loss.item()

        avg_train_loss = epoch_loss / len(train_loader)
        eval_metrics = evaluate(model, eval_loader, device)
        elapsed = time.time() - t0

        print(
            f"  Epoch {epoch:2d}/{epochs} | "
            f"Train Loss: {avg_train_loss:.4f} | "
            f"Eval Loss: {eval_metrics['eval_loss']:.4f} | "
            f"Eval Acc: {eval_metrics['accuracy']:.1%} | "
            f"Macro F1: {eval_metrics['macro_f1']:.4f} | "
            f"Time: {elapsed:.2f}s"
        )

        if eval_metrics["macro_f1"] >= best_macro_f1 or epoch == epochs:
            best_macro_f1 = max(best_macro_f1, eval_metrics["macro_f1"])
            best_metrics = eval_metrics
            print(f"      ⭐ Best validation Macro-F1: {best_macro_f1:.4f}")

    # 5. Save Artifacts
    print(f"\n[4/4] Saving fine-tuned model and tokenizer assets to: '{out_path}'...")
    model.save_pretrained(str(out_path))
    tokenizer.save_pretrained(str(out_path))

    metrics_payload = {
        "model_id": "fintext-sentiment-finbert-finetuned",
        "model_version": "3.1.0",
        "base_model": "ProsusAI/finbert",
        "fine_tuned_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "epochs": epochs,
        "batch_size": args.batch_size,
        "learning_rate": args.lr,
        "metrics": best_metrics,
    }

    metrics_path = out_path / "eval_metrics.json"
    with open(metrics_path, "w", encoding="utf-8") as f:
        json.dump(metrics_payload, f, indent=2)

    print(f"      Saved: model weights, config.json, tokenizer assets, and eval_metrics.json")
    print(f"      Evaluation Metrics Summary:")
    print(f"        • Accuracy:         {best_metrics['accuracy']:.1%}")
    print(f"        • Macro F1:         {best_metrics['macro_f1']:.4f}")
    print(f"        • Macro Precision:  {best_metrics['macro_precision']:.4f}")
    print(f"        • Macro Recall:     {best_metrics['macro_recall']:.4f}")

    print("\n" + "=" * 80)
    print(" ✅ FinBERT Fine-Tuning Completed Successfully!")
    print(f" Destination: {out_path.resolve()}")
    print("=" * 80)
    return 0


if __name__ == "__main__":
    sys.exit(main())
