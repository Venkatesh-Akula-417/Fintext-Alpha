# ══════════════════════════════════════════════════════════════════════════════
# FinText-Alpha-Vectorizer — AWS Production Infrastructure (Terraform)
# ══════════════════════════════════════════════════════════════════════════════
# Authoritative AWS Terraform IaC for FinText Beta Production Deployment
# Target Region: us-east-1 (N. Virginia)
# Monthly Budget Target: <= $301.44 / month
# ══════════════════════════════════════════════════════════════════════════════

terraform {
  required_version = ">= 1.5.0"

  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.0"
    }
  }

  # S3 backend configuration note:
  # In production, initialize remote state using:
  # terraform init -backend-config="bucket=fintext-tfstate-production" -backend-config="key=prod/terraform.tfstate" -backend-config="region=us-east-1"
}

provider "aws" {
  region = var.aws_region

  default_tags {
    tags = {
      Project     = "FinText-Alpha-Vectorizer"
      Environment = var.environment
      ManagedBy   = "Terraform"
      CostCenter  = "Quant-Beta-Platform"
    }
  }
}
