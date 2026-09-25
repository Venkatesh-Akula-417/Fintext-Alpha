# ══════════════════════════════════════════════════════════════════════════════
# FinText-Alpha-Vectorizer — IAM Roles, Instance Profiles & Policies
# ══════════════════════════════════════════════════════════════════════════════
# Least-privilege IAM profile for the EC2 platform host:
# - S3 access for automated daily database dumps & Parquet lakehouse writes
# - KMS Decrypt / GenerateDataKey for envelope encryption
# - CloudWatch Logs & Metrics publishing for real-time telemetry
# ══════════════════════════════════════════════════════════════════════════════

resource "aws_iam_role" "ec2_role" {
  name = "fintext-ec2-role-${var.environment}"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Action = "sts:AssumeRole"
        Effect = "Allow"
        Principal = {
          Service = "ec2.amazonaws.com"
        }
      }
    ]
  })

  tags = {
    Name = "fintext-ec2-role-${var.environment}"
    Tier = "Security-IAM"
  }
}

resource "aws_iam_policy" "ec2_policy" {
  name        = "fintext-ec2-policy-${var.environment}"
  description = "Least-privilege policy for FinText EC2 instance to access S3 backups and CloudWatch"

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Sid    = "S3BackupAndArchiveAccess"
        Effect = "Allow"
        Action = [
          "s3:ListBucket",
          "s3:GetBucketLocation",
          "s3:GetObject",
          "s3:PutObject",
          "s3:DeleteObject"
        ]
        Resource = [
          aws_s3_bucket.fintext_backups.arn,
          "${aws_s3_bucket.fintext_backups.arn}/*",
          aws_s3_bucket.fintext_parquet_archive.arn,
          "${aws_s3_bucket.fintext_parquet_archive.arn}/*",
          aws_s3_bucket.fintext_velero_backups.arn,
          "${aws_s3_bucket.fintext_velero_backups.arn}/*"
        ]
      },
      {
        Sid    = "KMSBackupKeyAccess"
        Effect = "Allow"
        Action = [
          "kms:Encrypt",
          "kms:Decrypt",
          "kms:ReEncrypt*",
          "kms:GenerateDataKey*",
          "kms:DescribeKey"
        ]
        Resource = [
          aws_kms_key.fintext_backup_key.arn
        ]
      },
      {
        Sid    = "CloudWatchTelemetryAccess"
        Effect = "Allow"
        Action = [
          "logs:CreateLogGroup",
          "logs:CreateLogStream",
          "logs:PutLogEvents",
          "logs:DescribeLogStreams",
          "cloudwatch:PutMetricData"
        ]
        Resource = "*"
      }
    ]
  })
}

resource "aws_iam_role_policy_attachment" "ec2_attach" {
  role       = aws_iam_role.ec2_role.name
  policy_arn = aws_iam_policy.ec2_policy.arn
}

resource "aws_iam_instance_profile" "ec2_profile" {
  name = "fintext-ec2-profile-${var.environment}"
  role = aws_iam_role.ec2_role.name
}
