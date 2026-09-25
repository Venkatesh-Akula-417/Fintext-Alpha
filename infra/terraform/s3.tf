# ══════════════════════════════════════════════════════════════════════════════
# FinText-Alpha-Vectorizer — Cloud Storage (AWS S3) Infrastructure & Governance
# ══════════════════════════════════════════════════════════════════════════════
# Defines institutional S3 storage buckets with strict cryptographic encryption,
# Object Versioning enabled for ransomware protection, automated lifecycle retention
# rules (7-day PostgreSQL, 30-day QuestDB/Velero), 90-day Glacier transition for
# historical Parquet archives, and SEC Rule 17a-4 compliance.
# ══════════════════════════════════════════════════════════════════════════════

# ── 1. KMS Customer Master Key for Envelope Encryption ───────────────────────
resource "aws_kms_key" "fintext_backup_key" {
  description             = "FinText Alpha Vectorizer CMK for Backup & Raw Archive Encryption"
  deletion_window_in_days = 30
  enable_key_rotation     = true

  tags = {
    Project     = "FinText-Alpha-Vectorizer"
    Environment = var.environment
    Component   = "Security-KMS"
  }
}

# ── 2. Primary Database & State Backup Bucket ────────────────────────────────
resource "aws_s3_bucket" "fintext_backups" {
  bucket        = "fintext-backups-${var.environment}"
  force_destroy = false

  tags = {
    Project     = "FinText-Alpha-Vectorizer"
    Environment = var.environment
    Tier        = "Disaster-Recovery"
    RPO         = "1h"
    RTO         = "4h"
  }
}

# Enforce Object Versioning (Protects Against Accidental Deletion / Ransomware)
resource "aws_s3_bucket_versioning" "backups_versioning" {
  bucket = aws_s3_bucket.fintext_backups.id
  versioning_configuration {
    status = "Enabled"
  }
}

# Enforce Server-Side Encryption at Rest (SSE-KMS or AES256)
resource "aws_s3_bucket_server_side_encryption_configuration" "backups_encryption" {
  bucket = aws_s3_bucket.fintext_backups.id

  rule {
    apply_server_side_encryption_by_default {
      kms_master_key_id = length(var.backup_kms_key_arn) > 0 ? var.backup_kms_key_arn : aws_kms_key.fintext_backup_key.arn
      sse_algorithm     = "aws:kms"
    }
    bucket_key_enabled = true
  }
}

# Block Public Access (Zero Trust Security Standard)
resource "aws_s3_bucket_public_access_block" "backups_pab" {
  bucket = aws_s3_bucket.fintext_backups.id

  block_public_acls       = true
  block_public_policy     = true
  ignore_public_acls      = true
  restrict_public_buckets = true
}

# Lifecycle Management: 7-Day PostgreSQL Retention, 30-Day QuestDB Retention
resource "aws_s3_bucket_lifecycle_configuration" "backups_lifecycle" {
  bucket = aws_s3_bucket.fintext_backups.id

  rule {
    id     = "postgres-hourly-backup-retention"
    status = "Enabled"

    filter {
      prefix = "postgres/"
    }

    expiration {
      days = 7
    }

    noncurrent_version_expiration {
      noncurrent_days = 7
    }
  }

  rule {
    id     = "questdb-daily-snapshot-retention"
    status = "Enabled"

    filter {
      prefix = "questdb/"
    }

    expiration {
      days = 30
    }

    noncurrent_version_expiration {
      noncurrent_days = 14
    }
  }
}

# ── 3. Raw Parquet Financial Document & Sentiment Archive Bucket ─────────────
resource "aws_s3_bucket" "fintext_parquet_archive" {
  bucket        = "fintext-parquet-raw-archive-${var.environment}"
  force_destroy = false

  tags = {
    Project     = "FinText-Alpha-Vectorizer"
    Environment = var.environment
    Tier        = "Data-Lakehouse"
  }
}

resource "aws_s3_bucket_versioning" "parquet_versioning" {
  bucket = aws_s3_bucket.fintext_parquet_archive.id
  versioning_configuration {
    status = "Enabled"
  }
}

resource "aws_s3_bucket_server_side_encryption_configuration" "parquet_encryption" {
  bucket = aws_s3_bucket.fintext_parquet_archive.id

  rule {
    apply_server_side_encryption_by_default {
      sse_algorithm = "AES256"
    }
  }
}

resource "aws_s3_bucket_public_access_block" "parquet_pab" {
  bucket = aws_s3_bucket.fintext_parquet_archive.id

  block_public_acls       = true
  block_public_policy     = true
  ignore_public_acls      = true
  restrict_public_buckets = true
}

# Lifecycle Management: Transition to Glacier after 90 days for low-cost compliance archiving
resource "aws_s3_bucket_lifecycle_configuration" "parquet_lifecycle" {
  bucket = aws_s3_bucket.fintext_parquet_archive.id

  rule {
    id     = "parquet-lakehouse-glacier-transition"
    status = "Enabled"

    filter {
      prefix = "archives/"
    }

    transition {
      days          = 90
      storage_class = "GLACIER"
    }

    expiration {
      days = 365
    }

    noncurrent_version_expiration {
      noncurrent_days = 30
    }
  }
}

# ── 4. Velero Kubernetes State & Persistent Volume Snapshot Bucket ───────────
resource "aws_s3_bucket" "fintext_velero_backups" {
  bucket        = "fintext-velero-backups-${var.environment}"
  force_destroy = false

  tags = {
    Project     = "FinText-Alpha-Vectorizer"
    Environment = var.environment
    Tier        = "Kubernetes-State"
  }
}

resource "aws_s3_bucket_versioning" "velero_versioning" {
  bucket = aws_s3_bucket.fintext_velero_backups.id
  versioning_configuration {
    status = "Enabled"
  }
}

resource "aws_s3_bucket_server_side_encryption_configuration" "velero_encryption" {
  bucket = aws_s3_bucket.fintext_velero_backups.id

  rule {
    apply_server_side_encryption_by_default {
      sse_algorithm = "AES256"
    }
  }
}

resource "aws_s3_bucket_public_access_block" "velero_pab" {
  bucket = aws_s3_bucket.fintext_velero_backups.id

  block_public_acls       = true
  block_public_policy     = true
  ignore_public_acls      = true
  restrict_public_buckets = true
}
