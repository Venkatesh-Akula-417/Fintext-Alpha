# ══════════════════════════════════════════════════════════════════════════════
# FinText-Alpha-Vectorizer — RDS PostgreSQL / TimescaleDB Data Tier
# ══════════════════════════════════════════════════════════════════════════════
# Enterprise-grade managed PostgreSQL with TimescaleDB time-series extensions:
# - db.m6i.large (2 vCPU, 8 GiB RAM) dedicated compute tier
# - 100 GiB gp3 encrypted storage with auto-scaling to 500 GiB
# - Private subnet isolation (strictly no public access)
# - Automated 7-day backups and custom parameter group with shared_preload_libraries
# ══════════════════════════════════════════════════════════════════════════════

resource "aws_db_subnet_group" "fintext_db_subnets" {
  name        = "fintext-db-subnet-group-${var.environment}"
  description = "Dual-AZ private subnet group for FinText RDS database"
  subnet_ids  = aws_subnet.private[*].id

  tags = {
    Name = "fintext-db-subnet-group-${var.environment}"
    Tier = "Database-SubnetGroup"
  }
}

resource "aws_db_parameter_group" "fintext_pg16" {
  name        = "fintext-pg16-timescaledb-${var.environment}"
  family      = "postgres16"
  description = "PostgreSQL 16 parameter group enabling TimescaleDB and institutional logging"

  # Load TimescaleDB and query performance statistics extensions
  parameter {
    name         = "shared_preload_libraries"
    value        = "timescaledb,pg_stat_statements"
    apply_method = "pending-reboot"
  }

  parameter {
    name         = "max_connections"
    value        = "200"
    apply_method = "immediate"
  }

  parameter {
    name         = "log_min_duration_statement"
    value        = "1000" # Log slow queries taking > 1,000ms
    apply_method = "immediate"
  }

  parameter {
    name         = "log_connections"
    value        = "1"
    apply_method = "immediate"
  }

  parameter {
    name         = "log_disconnections"
    value        = "1"
    apply_method = "immediate"
  }

  tags = {
    Name = "fintext-pg16-params-${var.environment}"
    Tier = "Database-Parameters"
  }
}

resource "aws_db_instance" "fintext_metadata" {
  identifier            = "fintext-metadata-${var.environment}"
  engine                = "postgres"
  engine_version        = "16.3"
  instance_class        = var.db_instance_class
  allocated_storage     = var.db_allocated_storage_gb
  max_allocated_storage = 500
  storage_type          = "gp3"
  storage_encrypted     = true

  db_name  = var.db_name
  username = var.db_username
  password = var.db_password
  port     = 5432

  # Network & High Availability Configuration
  multi_az               = false # Cost-capped single-AZ for initial beta ($129.94/mo vs $259.88/mo)
  publicly_accessible    = false
  db_subnet_group_name   = aws_db_subnet_group.fintext_db_subnets.name
  parameter_group_name   = aws_db_parameter_group.fintext_pg16.name
  vpc_security_group_ids = [aws_security_group.rds_sg.id]

  # Backup & Maintenance Windows (UTC)
  backup_retention_period    = 7
  backup_window              = "03:00-04:00"
  maintenance_window         = "Sun:04:30-Sun:05:30"
  auto_minor_version_upgrade = true
  copy_tags_to_snapshot      = true

  # Disaster Recovery & Deletion Protection
  deletion_protection       = var.db_deletion_protection
  skip_final_snapshot       = false
  final_snapshot_identifier = "fintext-metadata-final-snapshot-${var.environment}"

  tags = {
    Name = "fintext-timescaledb-${var.environment}"
    Tier = "Database-Primary"
  }
}
