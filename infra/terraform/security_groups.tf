# ══════════════════════════════════════════════════════════════════════════════
# FinText-Alpha-Vectorizer — Defense-in-Depth Security Groups
# ══════════════════════════════════════════════════════════════════════════════
# Zero-trust network access control:
# 1. EC2 Security Group: Ingress restricted to 80/443 public, 22 admin SSH
# 2. RDS Security Group: Ingress restricted strictly to EC2 SG on port 5432
# ══════════════════════════════════════════════════════════════════════════════

resource "aws_security_group" "ec2_sg" {
  name        = "fintext-ec2-sg-${var.environment}"
  description = "Security group for FinText Docker Compose application host"
  vpc_id      = aws_vpc.fintext_vpc.id

  # HTTPS Production Ingress
  ingress {
    description = "Public HTTPS ingress for API consumers & institutional WebSocket clients"
    from_port   = 443
    to_port     = 443
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"]
  }

  # HTTP Ingress (Redirect to HTTPS & ACME Let's Encrypt validation)
  ingress {
    description = "Public HTTP ingress for ACME challenges and HTTPS redirection"
    from_port   = 80
    to_port     = 80
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"]
  }

  # Administrative SSH Access (Restricted to Authorized CIDRs)
  ingress {
    description = "Bastion / administrative SSH access restricted to authorized networks"
    from_port   = 22
    to_port     = 22
    protocol    = "tcp"
    cidr_blocks = var.admin_ssh_cidr
  }

  # Outbound Egress (OS updates, S3 model downloads, public status push)
  egress {
    description = "Allow all outbound egress traffic"
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = ["0.0.0.0/0"]
  }

  tags = {
    Name = "fintext-ec2-sg-${var.environment}"
    Tier = "Security-Application"
  }
}

# ── RDS PostgreSQL / TimescaleDB Isolated Security Group ─────────────────────

resource "aws_security_group" "rds_sg" {
  name        = "fintext-rds-sg-${var.environment}"
  description = "Security group for private RDS TimescaleDB cluster"
  vpc_id      = aws_vpc.fintext_vpc.id

  # PostgreSQL Ingress strictly confined to EC2 application host
  ingress {
    description     = "PostgreSQL access strictly allowed from FinText EC2 instance"
    from_port       = 5432
    to_port         = 5432
    protocol        = "tcp"
    security_groups = [aws_security_group.ec2_sg.id]
  }

  # RDS Database has zero public egress
  egress {
    description = "Restricted outbound traffic within VPC"
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = [var.vpc_cidr]
  }

  tags = {
    Name = "fintext-rds-sg-${var.environment}"
    Tier = "Security-DataTier"
  }
}
