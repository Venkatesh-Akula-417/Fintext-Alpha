# ══════════════════════════════════════════════════════════════════════════════
# FinText-Alpha-Vectorizer — SNS Operational Alerting & Notification Topic
# ══════════════════════════════════════════════════════════════════════════════
# Dispatches high-severity infrastructure alarms (CPU, status checks, storage,
# backup failures) to the SRE / DevOps on-call notification distribution list.
# Gated by var.ops_alert_email defaulting to "" for zero-cost default staging.
# ══════════════════════════════════════════════════════════════════════════════

locals {
  alerting_enabled = length(trimspace(var.ops_alert_email)) > 0
}

resource "aws_sns_topic" "fintext_ops" {
  count = local.alerting_enabled ? 1 : 0

  name              = "fintext-ops-alerts-${var.environment}"
  display_name      = "FinText Ops Alerts (${var.environment})"
  kms_master_key_id = "alias/aws/sns"

  tags = {
    Project     = "FinText-Alpha-Vectorizer"
    Environment = var.environment
    Component   = "Operations-Alerting"
  }
}

resource "aws_sns_topic_subscription" "ops_email" {
  count = local.alerting_enabled ? 1 : 0

  topic_arn = aws_sns_topic.fintext_ops[0].arn
  protocol  = "email"
  endpoint  = var.ops_alert_email
}
