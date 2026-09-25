# ══════════════════════════════════════════════════════════════════════════════
# FinText-Alpha-Vectorizer — Terraform Provisioning Outputs
# ══════════════════════════════════════════════════════════════════════════════

output "vpc_id" {
  value       = aws_vpc.fintext_vpc.id
  description = "Unique ID of the FinText VPC"
}

output "ec2_instance_id" {
  value       = aws_instance.fintext_api.id
  description = "Instance ID of the FinText application host"
}

output "ec2_public_ip" {
  value       = aws_eip.fintext_api_eip.public_ip
  description = "Static Elastic IP address for institutional DNS / API Gateway routing"
}

output "rds_endpoint" {
  value       = aws_db_instance.fintext_metadata.endpoint
  description = "PostgreSQL TimescaleDB connection endpoint with port"
}

output "rds_address" {
  value       = aws_db_instance.fintext_metadata.address
  description = "PostgreSQL TimescaleDB private DNS host"
}

output "rds_port" {
  value       = aws_db_instance.fintext_metadata.port
  description = "PostgreSQL TimescaleDB port"
}

output "s3_backup_bucket" {
  value       = aws_s3_bucket.fintext_backups.id
  description = "Primary PostgreSQL backup S3 bucket"
}

output "s3_parquet_archive_bucket" {
  value       = aws_s3_bucket.fintext_parquet_archive.id
  description = "Columnar Parquet raw document and sentiment archive S3 bucket"
}

output "s3_velero_bucket" {
  value       = aws_s3_bucket.fintext_velero_backups.id
  description = "Velero cluster snapshot bucket name"
}

output "cloudwatch_api_log_group" {
  value       = aws_cloudwatch_log_group.api_server.name
  description = "CloudWatch log group for Axum API Gateway"
}

output "cloudwatch_ingestion_log_group" {
  value       = aws_cloudwatch_log_group.ingestion_engine.name
  description = "CloudWatch log group for Vector Ingestion Engine"
}
