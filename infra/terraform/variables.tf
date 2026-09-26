# ══════════════════════════════════════════════════════════════════════════════
# FinText-Alpha-Vectorizer — Terraform Variables & Configuration Schema
# ══════════════════════════════════════════════════════════════════════════════

variable "aws_region" {
  type        = string
  default     = "us-east-1"
  description = "Target AWS region for institutional deployment (N. Virginia colocation)"
}

variable "environment" {
  type        = string
  default     = "production"
  description = "Deployment lifecycle environment (production, staging, dev)"
}

# ── Network Topology Variables ───────────────────────────────────────────────

variable "vpc_cidr" {
  type        = string
  default     = "10.0.0.0/16"
  description = "IPv4 CIDR block for the FinText isolated VPC"
}

variable "availability_zones" {
  type        = list(string)
  default     = ["us-east-1a", "us-east-1b"]
  description = "Target Availability Zones for dual-AZ high availability"
}

variable "public_subnet_cidrs" {
  type        = list(string)
  default     = ["10.0.1.0/24", "10.0.2.0/24"]
  description = "Public subnet CIDR blocks for EC2 gateway ingress"
}

variable "private_subnet_cidrs" {
  type        = list(string)
  default     = ["10.0.10.0/24", "10.0.11.0/24"]
  description = "Private subnet CIDR blocks for RDS TimescaleDB isolation"
}

variable "admin_ssh_cidr" {
  type        = list(string)
  default     = ["0.0.0.0/0"]
  description = "Allowed CIDR blocks for administrative SSH access (port 22). Restrict to VPN in production."
}

# ── Compute (EC2) Variables ──────────────────────────────────────────────────

variable "ec2_instance_type" {
  type        = string
  default     = "c6i.xlarge"
  description = "EC2 compute instance type for Docker Compose platform host (c6i.xlarge = 4 vCPU / 8 GiB RAM)"
}

variable "ec2_root_volume_size_gb" {
  type        = number
  default     = 100
  description = "Size in GiB for EC2 gp3 root volume (OS + Docker container images)"
}

# ── Database (RDS TimescaleDB) Variables ─────────────────────────────────────

variable "db_instance_class" {
  type        = string
  default     = "db.m6i.large"
  description = "RDS PostgreSQL instance class (db.m6i.large = 2 vCPU / 8 GiB RAM)"
}

variable "db_allocated_storage_gb" {
  type        = number
  default     = 100
  description = "Allocated gp3 storage in GiB for RDS PostgreSQL TimescaleDB"
}

variable "db_name" {
  type        = string
  default     = "fintext_metadata"
  description = "Primary PostgreSQL database name"
}

variable "db_username" {
  type        = string
  default     = "fintext_admin"
  description = "Master username for RDS PostgreSQL instance"
}

variable "db_password" {
  type        = string
  sensitive   = true
  description = "Master password for RDS PostgreSQL instance (pass via TF_VAR_db_password or Secrets Manager)"
  default     = "FinText2026_SecureBetaPass!"
}

variable "db_deletion_protection" {
  type        = bool
  default     = true
  description = "Enforce deletion protection on production RDS instance"
}

# ── Storage & Observability Variables ────────────────────────────────────────

variable "backup_kms_key_arn" {
  type        = string
  default     = ""
  description = "Optional customer-managed KMS key ARN for SSE-KMS envelope encryption"
}

variable "cloudwatch_retention_days" {
  type        = number
  default     = 30
  description = "Log retention period in days for CloudWatch log groups"
}

# ── GA High Availability Staged Variables (Defaults False) ───────────────────

variable "rds_multi_az" {
  type        = bool
  default     = false
  description = "Enable RDS PostgreSQL Multi-AZ synchronous standby deployment for GA cutover"
}

variable "ha_compute_enabled" {
  type        = bool
  default     = false
  description = "Enable dual-node EC2 compute architecture across availability zones for GA"
}

variable "alb_enabled" {
  type        = bool
  default     = false
  description = "Deploy Application Load Balancer across public subnets for GA compute failover"
}
