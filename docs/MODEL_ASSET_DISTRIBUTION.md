# Model Asset Distribution Architecture

## 1. Overview & Purpose
This document specifies the model asset distribution mechanism for FinText Alpha Vectorizer. It details how machine learning model weights (specifically ONNX runtime artifacts) are provisioned for Continuous Integration (CI), automated model drift monitoring, and local developer environments without bloating the Git repository.

---

## 2. Why Model Artifacts Are Not Stored in Git
Machine learning model weights (such as the fine-tuned FinBERT sentiment classifier, NER sequence taggers, and MiniLM embedding weights) are binary files totaling ~155 MB (`model.onnx` alone is ~110 MB).

1. **Repository Bloat**: Git tracks object history by snapshotting entire binary deltas. Committing model weights across multiple training epochs rapidly bloats clone size into gigabytes, degrading developer workflow and CI runner initialization speeds.
2. **Git LFS Bandwidth Quotas**: Git LFS introduces third-party storage dependencies and bandwidth caps on public/private CI runners.
3. **Model Licensing & Governance**: Financial NLP weights frequently require distinct access control boundaries and compliance attestation separate from core application source code.
4. **Git Ignore Policy**: `.gitignore` explicitly filters `**/*.onnx` to maintain an agile, clean codebase.

---

## 3. Current Behavior: Graceful Fallback (SKIP Mode)
When model assets are absent from `models/finbert-finetuned/` or other model directories:
- **`scripts/validate_model_quality.py`**: Detects missing `model.onnx` or `finbert.onnx`, generates an informational report with `status: "SKIPPED"`, and exits with code `3`.
- **`scripts/model_drift_monitor.py`**: Identifies missing artifacts, records monitoring state as `SKIPPED`, and exits cleanly with code `3`.
- **`fintext_ingestion_engine` (Rust Workspace)**: 10 model-dependent tests in `nlp::ner::tests` and `nlp::onnx_sentiment::tests` check for file existence and gracefully skip execution, preserving green test passes.
- **CI Workflows**: Model validation and drift workflows inspect the file system; if artifacts are absent, they run the harness in SKIP verification mode and exit cleanly.

This architecture ensures CI remains green (`8/8 workflows passing`) even when heavy model assets are not downloaded.

---

## 4. Model Distribution Options

### Option A: GitHub Release Asset (Recommended for Public / Team Repositories)
The simplest, zero-infrastructure approach is hosting model bundles on GitHub Releases under tag releases (e.g., `v1.0.0-models`).

#### 1. Package Artifacts
```bash
cd models/finbert-finetuned
zip -r ../../finbert-finetuned.zip model.onnx tokenizer.json config.json
cd ../..
```

#### 2. Upload to GitHub Release
Upload `finbert-finetuned.zip` as a release asset via the GitHub Web UI or GitHub CLI:
```bash
gh release create v1.0.0-models finbert-finetuned.zip --title "Model Assets v1.0.0" --notes "Production FinBERT INT8 ONNX assets"
```

#### 3. Compute SHA256 Checksum
```bash
# Linux / macOS
sha256sum finbert-finetuned.zip

# Windows PowerShell
Get-FileHash -Algorithm SHA256 finbert-finetuned.zip
```

#### 4. Update `config/models_manifest.json`
```json
{
  "id": "finbert-finetuned",
  "target_dir": "models/finbert-finetuned",
  "required_files": ["model.onnx", "tokenizer.json", "config.json"],
  "url": "https://github.com/OWNER/REPO/releases/download/v1.0.0-models/finbert-finetuned.zip",
  "sha256": "3a7b9c1d...",
  "archive_format": "zip",
  "size_mb": 110
}
```

---

### Option B: Private S3 or MinIO Bucket (Enterprise / Proprietary Models)
For proprietary weights requiring corporate network isolation, host assets in an AWS S3 or MinIO object store.

#### 1. Bucket Configuration & IAM Policy
Create an S3 bucket with strict private access:
```json
{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Sid": "AllowCIModelRead",
      "Effect": "Allow",
      "Principal": {
        "AWS": "arn:aws:iam::ACCOUNT_ID:role/FinText-CI-Runner"
      },
      "Action": "s3:GetObject",
      "Resource": "arn:aws:s3:::fintext-model-assets/*"
    }
  ]
}
```

#### 2. Pre-signed URLs vs Public Read
- **Pre-signed URL**: Generate a temporary signed download URL (valid up to 7 days) during CI job setup or deployment.
- **Direct S3 HTTPS**: If using GitHub Actions with AWS OIDC, authenticate runner and download via AWS CLI or secure proxy.
- **GitHub Secret**: Store the base download URL or secret token in repository secrets:
  `MODEL_BASE_URL: ${{ secrets.FINBERT_ASSET_URL }}`

---

### Option C: Git LFS (Large File Storage)
Git LFS tracks binary pointers in Git and stores binaries on a remote LFS server.

#### Trade-offs
- **Pros**: Seamless `git checkout` pulls models automatically.
- **Cons**: High bandwidth costs on large teams, repository migration required, complex developer onboarding, risk of quota lockouts on hosted runners.
- **Setup (If Selected)**:
  ```bash
  git lfs install
  git lfs track "*.onnx"
  git add .gitattributes
  ```
  *(Note: Not enabled by default in FinText to avoid external bandwidth limitations).*

---

## 5. Enable CI Model Fetching in 5 Minutes

Follow these steps to activate automated model retrieval in CI:

1. **Choose Hosting**: Create a GitHub Release or upload weights to an S3/MinIO bucket.
2. **Upload Archive**: Package your model directory into `.zip` or `.tar.gz` and upload to your chosen host.
3. **Compute SHA256**:
   ```bash
   sha256sum finbert-finetuned.zip
   ```
4. **Edit Manifest** (`config/models_manifest.json`):
   Populate `"url"`, `"sha256"`, and set `"archive_format": "zip"`.
5. **Commit and Push**:
   ```bash
   git add config/models_manifest.json
   git commit -m "chore(infra): configure model asset distribution URLs"
   git push origin main
   ```
6. **Verify CI Execution**:
   The next CI run will execute `python scripts/fetch_models.py`, download and unpack the models, and run full model quality and drift validations.

---

## 6. Security & Credential Protection
- **No Secrets in Git**: Never hardcode access tokens, AWS secret keys, or expiring SAS tokens directly in `config/models_manifest.json`.
- **Environment Variable Templating**: If private authenticated endpoints are required, configure URL templates resolved via environment variables:
  ```bash
  python -c "
  import os, json
  m = json.load(open('config/models_manifest.json'))
  m['assets'][0]['url'] = os.environ['FINBERT_MODEL_URL']
  json.dump(m, open('config/models_manifest.json', 'w'), indent=2)
  "
  ```
- **Integrity Enforcement**: `scripts/fetch_models.py` strictly verifies the SHA256 checksum whenever `"sha256"` is defined in the manifest. Tampered or corrupted downloads are rejected immediately.

---

## 7. Troubleshooting & Recovery

| Issue | Cause | Resolution |
| :--- | :--- | :--- |
| `SKIP: URL not configured for '<id>'` | `"url": null` in manifest | Expected when models are not externally hosted. No action needed for SKIP mode. |
| `FAIL: '<id>' download failed: HTTP error 404` | Asset URL broken or release deleted | Verify URL in browser or via `curl -I <URL>`. Update manifest. |
| `FAIL: '<id>' verification failed: SHA256 mismatch` | File corrupted during transit or outdated hash | Re-compute `sha256sum <file>` and update `"sha256"` in `config/models_manifest.json`. |
| `FAIL: Archive extraction failed` | Corrupted zip or unsupported format | Ensure `"archive_format"` matches archive type (`zip`, `tar.gz`, or `none`). |
| **Emergency Revert to SKIP Mode** | Remote host outage blocking full validation | Set `"url": null` in `config/models_manifest.json` and push. CI will cleanly revert to SKIP mode. |
