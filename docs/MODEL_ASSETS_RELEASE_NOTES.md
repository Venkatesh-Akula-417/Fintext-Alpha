# FinText Model Assets v1.0.0 — Certified Release Notes

**Release Tag**: `model-assets-v1.0.0`  
**Release Date**: 2026-09-25  
**Target Platform**: FinText Alpha Vectorizer (Institutional Private Beta)  
**Distribution Channel**: GitHub Releases (Zero Git Tree Bloat, Free Tier Compliant)

---

## 1. Release Inventory & Cryptographic Manifest

| Asset Name | Compressed Size | Uncompressed Size | SHA256 Checksum | License | Operational Role |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **`finbert-finetuned-v1.0.0.zip`** | **92,977,372 bytes** (88.67 MB) | 111,377,788 bytes | `182788c0e554ac3891e88b09f98c6b92ab6cd85f5b1c7c039b1d561049b86d33` | Apache-2.0 | **PRODUCTION-INFERENCE** (Real-time sentiment scoring `/v1/sentiment*`) |
| **`ner-v1.0.0.zip`** | **398,841,579 bytes** (380.36 MB) | 430,502,722 bytes | `d6ff862b8b2ae293bea50b3dd0058d0facf7d9e154f94ad7619a92e940f9bbdc` | MIT | **RESEARCH-ONLY** (Entity extraction in ingestion engine & transcripts) |
| **`SHA256SUMS.txt`** | 184 bytes | 184 bytes | Cryptographic integrity ledger | N/A | Verification Manifest |

---

## 2. Supply-Chain Provenance & Licensing

### A. FinBERT Fine-Tuned Sentiment Model (`finbert-finetuned-v1.0.0.zip`)
- **Base Architecture**: `ProsusAI/finbert` (BERT-base-uncased fine-tuned on Financial PhraseBank).
- **License**: **Apache-2.0** (Permissive commercial use).
- **Artifacts Included**:
  - `finbert.onnx` (110,664,600 bytes) — Dynamic batching ONNX Runtime graph.
  - `tokenizer.json` (711,661 bytes) — Fast HuggingFace WordPiece tokenizer.
  - `config.json` (964 bytes) — Model architecture parameters.
  - `tokenizer_config.json` (563 bytes) — Tokenizer normalization settings.
- **Production Inference Role**: Powers all `/v1/sentiment`, `/v1/sentiment/feed`, `/v1/sentiment/batch`, and `/v1/sentiment/disagreement` endpoints in `fintext_api`.

### B. Named Entity Recognition Model (`ner-v1.0.0.zip`)
- **Base Architecture**: `dslim/bert-base-NER` (IOB2 entity recognition: ORG, PER, LOC, MISC).
- **License**: **MIT License** (Permissive commercial use).
- **Artifacts Included**:
  - `model_static.onnx` (429,833,413 bytes) — Static tensor ONNX Runtime graph.
  - `tokenizer.json` (668,923 bytes) — WordPiece tokenizer.
  - `tokenizer_config.json` (386 bytes) — Special token mappings.
- **Operational Role**: **RESEARCH-ONLY / AUXILIARY INGESTION**. Used for entity tagging in earnings call transcripts and offline financial news classification. It is optional in `api_server` and not in the critical path of real-time latency-sensitive sentiment scoring.

---

## 3. Why Assets Are Hosted in GitHub Releases (Zero Tree Bloat)
1. **GitHub 100 MB Tree Hard Limit**: GitHub blocks files exceeding 100 MB (`ner-v1.0.0.zip` is 380.36 MB, uncompressed 429.8 MB).
2. **Git LFS Cost Avoidance**: Git LFS incurs recurring monthly storage and bandwidth billing. GitHub Releases provides free, high-speed CDN delivery up to 2.0 GB per asset.
3. **Reproducible Research**: Version-pinned release tags ensure quantitative researchers and algorithmic trading clients can audit and reproduce exact backtest weights without model drift.

---

## 4. Download & Integrity Verification Commands

### Option 1: GitHub CLI (`gh`)
```bash
# Download all assets for model-assets-v1.0.0:
gh release download model-assets-v1.0.0 --repo Venkatesh-Akula-417/Fintext-Alpha --dir models_release_v1
```

### Option 2: cURL (Direct CDN Download)
```bash
mkdir -p models_release_v1

# Download FinBERT Production Bundle
curl -L "https://github.com/Venkatesh-Akula-417/Fintext-Alpha/releases/download/model-assets-v1.0.0/finbert-finetuned-v1.0.0.zip" \
  -o models_release_v1/finbert-finetuned-v1.0.0.zip

# Download NER Research Bundle
curl -L "https://github.com/Venkatesh-Akula-417/Fintext-Alpha/releases/download/model-assets-v1.0.0/ner-v1.0.0.zip" \
  -o models_release_v1/ner-v1.0.0.zip

# Download Checksum Ledger
curl -L "https://github.com/Venkatesh-Akula-417/Fintext-Alpha/releases/download/model-assets-v1.0.0/SHA256SUMS.txt" \
  -o models_release_v1/SHA256SUMS.txt
```

### Option 3: Cryptographic Integrity Verification

#### On Linux / Ubuntu / WSL:
```bash
cd models_release_v1
sha256sum -c SHA256SUMS.txt
# Expected Output:
# finbert-finetuned-v1.0.0.zip: OK
# ner-v1.0.0.zip: OK
```

#### On Windows (PowerShell):
```powershell
certutil -hashfile models_release_v1\finbert-finetuned-v1.0.0.zip SHA256
# Expected: 182788c0e554ac3891e88b09f98c6b92ab6cd85f5b1c7c039b1d561049b86d33

certutil -hashfile models_release_v1\ner-v1.0.0.zip SHA256
# Expected: d6ff862b8b2ae293bea50b3dd0058d0facf7d9e154f94ad7619a92e940f9bbdc
```

---

## 5. Deployment & Extraction Instructions

```bash
# Extract into models directory
mkdir -p models/finbert-finetuned models/ner

# Unzip FinBERT
unzip -q models_release_v1/finbert-finetuned-v1.0.0.zip -d models/finbert-finetuned/

# Unzip NER (optional for research/ingestion)
unzip -q models_release_v1/ner-v1.0.0.zip -d models/ner/
```

FinText Docker Compose automatically binds `./models:/app/models:ro` to mount these models into the API Gateway and Ingestion containers.
