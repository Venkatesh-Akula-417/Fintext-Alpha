# ══════════════════════════════════════════════════════════════════════════════
# FinText-Alpha-Vectorizer — CloudWatch Telemetry, Log Groups & Metric Alarms
# ══════════════════════════════════════════════════════════════════════════════
# Operational visibility into API gateway and ingestion services:
# - Dedicated log groups with configurable retention (30 days)
# - Automated alarms for host CPU saturation, status check failures, and RDS storage exhaustion
# ══════════════════════════════════════════════════════════════════════════════

resource "aws_cloudwatch_log_group" "api_server" {
  name              = "/fintext/api-server-${var.environment}"
  retention_in_days = var.cloudwatch_retention_days

  tags = {
    Name      = "fintext-api-logs-${var.environment}"
    Component = "API-Gateway"
  }
}

resource "aws_cloudwatch_log_group" "ingestion_engine" {
  name              = "/fintext/ingestion-engine-${var.environment}"
  retention_in_days = var.cloudwatch_retention_days

  tags = {
    Name      = "fintext-ingestion-logs-${var.environment}"
    Component = "Ingestion-Engine"
  }
}

# ── CloudWatch Metric Alarms ─────────────────────────────────────────────────

resource "aws_cloudwatch_metric_alarm" "ec2_high_cpu" {
  alarm_name          = "fintext-ec2-high-cpu-${var.environment}"
  comparison_operator = "GreaterThanThreshold"
  evaluation_periods  = 2
  metric_name         = "CPUUtilization"
  namespace           = "AWS/EC2"
  period              = 300
  statistic           = "Average"
  threshold           = 80
  alarm_description   = "EC2 application host CPU utilization exceeded 80% for 10 minutes"

  dimensions = {
    InstanceId = aws_instance.fintext_api.id
  }

  tags = {
    Severity = "Warning"
    Tier     = "Compute-Monitoring"
  }
}

resource "aws_cloudwatch_metric_alarm" "ec2_status_check" {
  alarm_name          = "fintext-ec2-status-check-${var.environment}"
  comparison_operator = "GreaterThanThreshold"
  evaluation_periods  = 2
  metric_name         = "StatusCheckFailed"
  namespace           = "AWS/EC2"
  period              = 60
  statistic           = "Maximum"
  threshold           = 0
  alarm_description   = "EC2 instance status check failed (Host or System failure detected)"

  dimensions = {
    InstanceId = aws_instance.fintext_api.id
  }

  tags = {
    Severity = "Critical"
    Tier     = "Compute-Monitoring"
  }
}

resource "aws_cloudwatch_metric_alarm" "rds_high_cpu" {
  alarm_name          = "fintext-rds-high-cpu-${var.environment}"
  comparison_operator = "GreaterThanThreshold"
  evaluation_periods  = 2
  metric_name         = "CPUUtilization"
  namespace           = "AWS/RDS"
  period              = 300
  statistic           = "Average"
  threshold           = 80
  alarm_description   = "RDS TimescaleDB CPU utilization exceeded 80% for 10 minutes"

  dimensions = {
    DBInstanceIdentifier = aws_db_instance.fintext_metadata.identifier
  }

  tags = {
    Severity = "Warning"
    Tier     = "Database-Monitoring"
  }
}

resource "aws_cloudwatch_metric_alarm" "rds_low_storage" {
  alarm_name          = "fintext-rds-low-storage-${var.environment}"
  comparison_operator = "LessThanThreshold"
  evaluation_periods  = 1
  metric_name         = "FreeStorageSpace"
  namespace           = "AWS/RDS"
  period              = 300
  statistic           = "Average"
  threshold           = 15000000000 # 15 GB threshold in bytes
  alarm_description   = "RDS TimescaleDB free storage space dropped below 15 GB"

  dimensions = {
    DBInstanceIdentifier = aws_db_instance.fintext_metadata.identifier
  }

  tags = {
    Severity = "Critical"
    Tier     = "Database-Monitoring"
  }
}
