//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Data Retention Policy Models & DTOs
//! ═══════════════════════════════════════════════════════════════════════════════

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// Stored compliance data retention policy entity.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct RetentionPolicy {
    /// Unique policy identifier UUID
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: Uuid,
    /// Optional organization context identifier (if policy is org-scoped)
    #[schema(example = "550e8400-e29b-41d4-a716-446655440001")]
    pub org_id: Option<Uuid>,
    /// User identifier owning or creating this policy
    #[schema(example = "quant_fund_admin")]
    pub user_id: String,
    /// Data category governed by this retention policy
    #[schema(example = "usage_events")]
    pub data_category: String,
    /// Retention period duration in days (1 to 3650)
    #[schema(example = 90)]
    pub retention_days: u32,
    /// Whether the retention policy is actively enforced
    #[schema(example = true)]
    pub is_active: bool,
    /// Policy creation timestamp (UTC)
    pub created_at: DateTime<Utc>,
    /// Policy last updated timestamp (UTC)
    pub updated_at: DateTime<Utc>,
}

/// Request payload for creating or updating a data retention policy (`POST /retention/policies`).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct CreateRetentionPolicyRequest {
    /// Target data category to configure (e.g. "usage_events", "audit_logs", "news_articles", "transcripts", "sentiment_history", "webhook_deliveries", "kafka_credentials", "email_digests")
    #[schema(example = "usage_events")]
    pub data_category: String,
    /// Retention period duration in days (min: 1, max: 3650)
    #[schema(example = 90)]
    pub retention_days: u32,
    /// Optional active flag (default: true)
    #[serde(default = "default_active_flag")]
    #[schema(example = true)]
    pub is_active: Option<bool>,
}

fn default_active_flag() -> Option<bool> {
    Some(true)
}

/// Response payload for listing retention policies (`GET /retention/policies`).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct RetentionPoliciesResponse {
    /// Array of configured data retention policies
    pub policies: Vec<RetentionPolicy>,
    /// Total count of configured policies
    #[schema(example = 2)]
    pub total_policies: usize,
}

/// Response payload for deleting/deactivating a retention policy (`DELETE /retention/policies/{id}`).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct DeleteRetentionPolicyResponse {
    /// Deletion status ('deleted')
    #[schema(example = "deleted")]
    pub status: String,
    /// Confirmation message
    #[schema(example = "Retention policy deleted successfully")]
    pub message: String,
    /// Target policy identifier UUID
    pub id: Uuid,
    /// Deletion timestamp (UTC)
    pub deleted_at: DateTime<Utc>,
}
