# FinText Alpha Vectorizer — Certified Model Assets Manifest

**Manifest Version**: `1.0.0`  
**Distribution Channel**: [GitHub Releases: model-assets-v1.0.0](https://github.com/Venkatesh-Akula-417/Fintext-Alpha/releases/tag/model-assets-v1.0.0)  
**Standard Checksum File**: [models_release_v1/SHA256SUMS.txt](file:///d:/FinText-Alpha-Vectorizer/models_release_v1/SHA256SUMS.txt)  
**Security Policy**: Zero model binaries in Git tree. All binary weights distributed via immutable cryptographic release tags.

---

## 1. Model Artifact Inventory

| Asset Name | Release Tag | Format | Compressed Bytes | Uncompressed Bytes | SHA256 Cryptographic Checksum | License | Operational Role |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **`finbert-finetuned-v1.0.0.zip`** | `model-assets-v1.0.0` | ONNX (FP32) | 92,977,372 (88.67 MB) | 111,377,788 (106.22 MB) | `182788c0e554ac3891e88b09f98c6b92ab6cd85f5b1c7c039b1d561049b86d33` | Apache-2.0 | **PRODUCTION-INFERENCE** |
| **`ner-v1.0.0.zip`** | `model-assets-v1.0.0` | ONNX (FP32) | 398,841,579 (380.36 MB) | 430,502,722 (410.56 MB) | `d6ff862b8b2ae293bea50b3dd0058d0facf7d9e154f94ad7619a92e940f9bbdc` | MIT | **RESEARCH-ONLY** |
| **`SHA256SUMS.txt`** | `model-assets-v1.0.0` | Plaintext | 184 bytes | 184 bytes | Cryptographic integrity ledger | N/A | Verification Manifest |

---

## 2. Supply-Chain Provenance & Training Lineage

### A. FinBERT Fine-Tuned Sentiment Model
- **Base Architecture**: `ProsusAI/finbert` (Pretrained on financial communications).
- **License**: **Apache-2.0** ([ProsusAI/finbert](https://huggingface.co/ProsusAI/finbert)).
- **Fine-Tuning Lineage**: Fine-tuned on institutional financial news headlines and SEC 8-K disclosures using notebooks `notebooks/01_model_training.ipynb` through `notebooks/03_model_evaluation.ipynb`.
- **Signal Quality Certification**: Certified under [docs/SIGNAL_QUALITY_REPORT.md](file:///d:/FinText-Alpha-Vectorizer/docs/SIGNAL_QUALITY_REPORT.md) with In-Sample Information Coefficient (IC) $+0.0540$ and Out-of-Sample 2024-2025 IC $+0.0518$ ($\ge +0.0500$ target), annualized Net Sharpe $1.45$ ($\ge 1.40$ target).
- **Production Path**: Ingested directly by `rust/api_server/src/sentiment.rs` and `rust/ingestion_engine/src/nlp/onnx_sentiment.rs`.

### B. Named Entity Recognition (NER) Model
- **Base Architecture**: `dslim/bert-base-NER` (Fine-tuned on CoNLL-2003).
- **License**: **MIT License** ([dslim/bert-base-NER](https://huggingface.co/dslim/bert-base-NER)).
- **Export Lineage**: Exported to ONNX static graph format for entity identification (`ORG`, `PER`, `LOC`, `MISC`).
- **Operational Role (Research-Only)**: Used in the ingestion pipeline (`rust/ingestion_engine/src/nlp/ner.rs`) and quantitative research transcripts (`/v1/transcripts`). It is marked optional in `api_server/src/main.rs` and is **NOT** on the critical path for real-time market sentiment inference.

---

## 3. Why Large Model Binaries Are Excluded from Git Tree
1. **GitHub 100 MB Limit**: GitHub rejects commits containing files exceeding 100 MB. The uncompressed NER static ONNX model is 429.8 MB and its compressed zip is 380.4 MB.
2. **Git LFS Cost Inefficiency**: Git LFS incurs per-gigabyte bandwidth fees that compromise the platform's $295.00/mo cloud cost cap.
3. **GitHub Releases Free CDN**: GitHub Releases provides free, unlimited bandwidth artifact hosting (up to 2.0 GB per asset) with permanent, immutable tag URLs.

---

## 4. Download & Checksum Verification Commands

### Linux / macOS (Bash)
```bash
# 1. Create target directory
mkdir -p models_release_v1

# 2. Download assets
curl -L -o models_release_v1/finbert-finetuned-v1.0.0.zip \
  "https://github.com/Venkatesh-Akula-417/Fintext-Alpha/releases/download/model-assets-v1.0.0/finbert-finetuned-v1.0.0.zip"
curl -L -o models_release_v1/ner-v1.0.0.zip \
  "https://github.com/Venkatesh-Akula-417/Fintext-Alpha/releases/download/model-assets-v1.0.0/ner-v1.0.0.zip"
curl -L -o models_release_v1/SHA256SUMS.txt \
  "https://github.com/Venkatesh-Akula-417/Fintext-Alpha/releases/download/model-assets-v1.0.0/SHA256SUMS.txt"

# 3. Verify SHA256 checksums
cd models_release_v1
sha256sum -c SHA256SUMS.txt
# Expected:
# finbert-finetuned-v1.0.0.zip: OK
# ner-v1.0.0.zip: OK
```

### Windows (PowerShell)
```powershell
# 1. Create directory
New-Item -ItemType Directory -Force -Path models_release_v1

# 2. Download via curl.exe
curl.exe -L -o models_release_v1/finbert-finetuned-v1.0.0.zip `
  "https://github.com/Venkatesh-Akula-417/Fintext-Alpha/releases/download/model-assets-v1.0.0/finbert-finetuned-v1.0.0.zip"
curl.exe -L -o models_release_v1/ner-v1.0.0.zip `
  "https://github.com/Venkatesh-Akula-417/Fintext-Alpha/releases/download/model-assets-v1.0.0/ner-v1.0.0.zip"

# 3. Verify checksums
certutil -hashfile models_release_v1\finbert-finetuned-v1.0.0.zip SHA256
# Compare output against: 182788c0e554ac3891e88b09f98c6b92ab6cd85f5b1c7c039b1d561049b86d33

certutil -hashfile models_release_v1\ner-v1.0.0.zip SHA256
# Compare output against: d6ff862b8b2ae293bea50b3dd0058d0facf7d9e154f94ad7619a92e940f9bbdc
```

---

## 5. Deployment & Runtime Wiring

### Unpacking Models into Local Hierarchy
```bash
mkdir -p models/finbert-finetuned models/ner
unzip -q models_release_v1/finbert-finetuned-v1.0.0.zip -d models/finbert-finetuned/
unzip -q models_release_v1/ner-v1.0.0.zip -d models/ner/
```

### Docker Compose Volume Mount
In `docker-compose.yml`, both `fintext-api-gateway` and `fintext-ingestion-engine` mount:
```yaml
volumes:
  - ./models:/app/models:ro
```
Environment variable configuration:
```env
MODEL_DIR=/app/models/finbert-finetuned
NER_MODEL_DIR=/app/models/ner
```

### ONNX Runtime Notes
- **Linux Containers**: Runs on `ort 2.0.0-rc.13` linked against system C-runtime with multi-threaded CPU Level-3 optimizations (`intra_threads=4, inter_threads=1`).
- **Windows Local Development**: `onnxruntime.dll` in the workspace root is provided strictly for offline local execution of tests and native Windows debugging. In Docker and Linux CI, dynamic libraries are resolved automatically via the standard ONNX runtime wheel / package.

---

## 6. Model Asset Lifecycle & Update Procedure
When retraining producing new model weights (e.g., `v1.1.0`):
1. **Model Packaging**: Export optimized ONNX model and tokenizers into `models_release_v1/finbert-finetuned-v1.1.0.zip`.
2. **Checksum Generation**: Run `sha256sum finbert-finetuned-v1.1.0.zip >> models_release_v1/SHA256SUMS.txt`.
3. **Git Tagging**: Create an immutable tag: `git tag -a model-assets-v1.1.0 -m "FinText Model Assets v1.1.0"`.
4. **Release Publishing**: Execute `gh release create model-assets-v1.1.0 ...` to publish assets.
5. **Documentation & DDQ Sync**: Append the new row to this manifest and update Section 4.5 of [docs/SECURITY_DDQ_AIFMD_SEC.md](file:///d:/FinText-Alpha-Vectorizer/docs/SECURITY_DDQ_AIFMD_SEC.md).

---

## 7. Incident Playbook: Checksum Mismatch Remediation
If an automated CI job or deployment check fails the SHA256 integrity assertion:
1. **Quarantine Immediately**: Do not load the unverified weights into memory. Stop container deployment.
2. **Re-Download from Origin**: Download directly using the authenticated GitHub CLI or verified curl URL.
3. **Compare Against Immutable Tag**: Check the hash against the signed tag release notes:
   `https://github.com/Venkatesh-Akula-417/Fintext-Alpha/releases/tag/model-assets-v1.0.0`
4. **Security Notification**: If a hash mismatch persists after re-download, treat as a potential supply-chain integrity breach and notify Security via `security@fintext.internal`.
