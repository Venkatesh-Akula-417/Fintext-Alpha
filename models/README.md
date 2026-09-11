# Model Hierarchy

> **Last Verified**: 2026-09-10 (Suite #96) | **Audit Readiness**: Certified Clean | **Authoritative Deprecations**: [`docs/DEPRECATED.md`](../docs/DEPRECATED.md)

## Primary (Active)
- `finbert-finetuned/` — Fine-tuned FinBERT INT8 (v3.1.0)

## Fallback (Kept Intentionally)
- `finbert/` — Base ProsusAI/finbert INT8 (v3.0.0)
  - Used when `finbert-finetuned/` is unavailable
  - Preserved per Suite #258 fallback policy
- `minilm_seq32/` — Ultra-fast 32-token headline fallback (v2.1.0)
  - Used for microsecond-level headline sentiment evaluation

## Specialized
- `ner/` — Token NER for entity extraction
- `whisper/` — Whisper.cpp GGML audio models

## Archived
- `minilm_seq32/` (if removed) — Legacy model, see `docs/DEPRECATED.md`

**Never delete fallback models without verifying the primary is production-proven.**
