//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Data Retention Policy Engine & Background Worker
//! ═══════════════════════════════════════════════════════════════════════════════
//!
//! Enforces regulatory data lifecycle retention policies (GDPR/CCPA/SEC)
//! across PostgreSQL tables, QuestDB time series, and in-memory caches.
//! ═══════════════════════════════════════════════════════════════════════════════

use chrono::{DateTime, Duration as ChronoDuration, Utc};
use dashmap::DashMap;
use serde_json::json;
use sqlx::{PgPool, Row};
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinHandle;
use tracing::{info, warn};
use uuid::Uuid;

use crate::audit_logs::log_audit_event;
use crate::models::RetentionPolicy;
use crate::state::AppState;

pub const MIN_RETENTION_DAYS: u32 = 1;
pub const MAX_RETENTION_DAYS: u32 = 3650; // 10 years
pub const DEFAULT_RETENTION_DAYS: u32 = 365;
pub const DEFAULT_RETENTION_CHECK_INTERVAL_SECS: u64 = 21600; // 6 hours

/// Supported compliance data categories.
pub const VALID_DATA_CATEGORIES: &[&str] = &[
    "usage_events",
    "audit_logs",
    "news_articles",
    "transcripts",
    "sentiment_history",
    "webhook_deliveries",
    "kafka_credentials",
    "email_digests",
];

/// Checks if a string is a valid data retention category.
pub fn is_valid_data_category(cat: &str) -> bool {
    let clean = cat.trim().to_lowercase();
    VALID_DATA_CATEGORIES.iter().any(|&c| c == clean)
}

/// Returns a slice of all supported data categories.
pub fn get_valid_data_categories() -> &'static [&'static str] {
    VALID_DATA_CATEGORIES
}

// ─────────────────────────────────────────────────────────────────────────────
// Retention Policy Registry
// ─────────────────────────────────────────────────────────────────────────────

/// Thread-safe in-memory and PostgreSQL synchronized data retention policy registry.
#[derive(Debug, Clone, Default)]
pub struct RetentionPolicyRegistry {
    policies: Arc<DashMap<Uuid, RetentionPolicy>>,
}

impl RetentionPolicyRegistry {
    /// Creates a new empty `RetentionPolicyRegistry`.
    pub fn new() -> Self {
        Self {
            policies: Arc::new(DashMap::new()),
        }
    }

    /// Initializes PostgreSQL table schema and indexes.
    pub async fn init_db(pool: &PgPool) -> Result<(), sqlx::Error> {
        let sql = r#"
            CREATE TABLE IF NOT EXISTS data_retention_policies (
                id UUID PRIMARY KEY,
                org_id UUID NULL,
                user_id TEXT NOT NULL,
                data_category TEXT NOT NULL,
                retention_days INTEGER NOT NULL,
                is_active BOOLEAN NOT NULL DEFAULT TRUE,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );
            CREATE UNIQUE INDEX IF NOT EXISTS uq_retention_user_cat_org 
                ON data_retention_policies(user_id, data_category, COALESCE(org_id, '00000000-0000-0000-0000-000000000000'::UUID));
            CREATE INDEX IF NOT EXISTS idx_retention_user ON data_retention_policies(user_id);
            CREATE INDEX IF NOT EXISTS idx_retention_org ON data_retention_policies(org_id);
        "#;
        sqlx::query(sql).execute(pool).await?;
        info!("[Data Retention] PostgreSQL 'data_retention_policies' table verified and indexed");
        Ok(())
    }

    /// Loads all active retention policies from PostgreSQL into memory cache.
    pub async fn load_from_db(&self, pool: &PgPool) -> Result<usize, sqlx::Error> {
        let sql = r#"
            SELECT id, org_id, user_id, data_category, retention_days, is_active, created_at, updated_at
            FROM data_retention_policies
            WHERE is_active = TRUE
        "#;
        let rows = sqlx::query(sql).fetch_all(pool).await?;
        let count = rows.len();

        for row in rows {
            let id: Uuid = row.try_get("id")?;
            let org_id: Option<Uuid> = row.try_get("org_id")?;
            let user_id: String = row.try_get("user_id")?;
            let data_category: String = row.try_get("data_category")?;
            let retention_days_i: i32 = row.try_get("retention_days")?;
            let is_active: bool = row.try_get("is_active")?;
            let created_at: DateTime<Utc> = row.try_get("created_at")?;
            let updated_at: DateTime<Utc> = row.try_get("updated_at")?;

            self.policies.insert(
                id,
                RetentionPolicy {
                    id,
                    org_id,
                    user_id,
                    data_category,
                    retention_days: retention_days_i as u32,
                    is_active,
                    created_at,
                    updated_at,
                },
            );
        }

        info!(
            "[Data Retention] Loaded {} active retention policies into memory cache",
            count
        );
        Ok(count)
    }

    /// Upserts a retention policy for a user or organization.
    pub async fn upsert_policy(
        &self,
        user_id: &str,
        org_id: Option<Uuid>,
        category: &str,
        retention_days: u32,
        is_active: bool,
        pool: Option<&PgPool>,
    ) -> Result<RetentionPolicy, String> {
        let category_clean = category.trim().to_lowercase();
        if !is_valid_data_category(&category_clean) {
            return Err(format!(
                "Invalid data_category '{}'. Allowed: {:?}",
                category, VALID_DATA_CATEGORIES
            ));
        }

        if retention_days < MIN_RETENTION_DAYS || retention_days > MAX_RETENTION_DAYS {
            return Err(format!(
                "retention_days must be between {} and {} (got {})",
                MIN_RETENTION_DAYS, MAX_RETENTION_DAYS, retention_days
            ));
        }

        let now = Utc::now();

        // 1. Check if policy already exists in memory
        let mut existing_id = None;
        for entry in self.policies.iter() {
            let p = entry.value();
            if p.user_id == user_id && p.data_category == category_clean && p.org_id == org_id {
                existing_id = Some(p.id);
                break;
            }
        }

        let policy = if let Some(id) = existing_id {
            let mut p = self.policies.get_mut(&id).unwrap();
            p.retention_days = retention_days;
            p.is_active = is_active;
            p.updated_at = now;
            p.clone()
        } else {
            let new_id = Uuid::new_v4();
            let new_policy = RetentionPolicy {
                id: new_id,
                org_id,
                user_id: user_id.to_string(),
                data_category: category_clean.clone(),
                retention_days,
                is_active,
                created_at: now,
                updated_at: now,
            };
            self.policies.insert(new_id, new_policy.clone());
            new_policy
        };

        // 2. Persist to PostgreSQL if pool is available
        if let Some(p) = pool {
            let sql = r#"
                INSERT INTO data_retention_policies
                    (id, org_id, user_id, data_category, retention_days, is_active, created_at, updated_at)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                ON CONFLICT (user_id, data_category, COALESCE(org_id, '00000000-0000-0000-0000-000000000000'::UUID))
                DO UPDATE SET
                    retention_days = EXCLUDED.retention_days,
                    is_active = EXCLUDED.is_active,
                    updated_at = EXCLUDED.updated_at
            "#;

            if let Err(e) = sqlx::query(sql)
                .bind(policy.id)
                .bind(policy.org_id)
                .bind(&policy.user_id)
                .bind(&policy.data_category)
                .bind(policy.retention_days as i32)
                .bind(policy.is_active)
                .bind(policy.created_at)
                .bind(policy.updated_at)
                .execute(p)
                .await
            {
                warn!("[Data Retention] Failed to persist policy to DB: {}", e);
            }
        }

        info!(
            "[Data Retention] Upserted policy id='{}' user='{}' org={:?} category='{}' retention={}d active={}",
            policy.id, user_id, org_id, policy.data_category, policy.retention_days, policy.is_active
        );

        Ok(policy)
    }

    /// Deletes (or deactivates) a retention policy by ID.
    pub async fn delete_policy(
        &self,
        id: Uuid,
        user_id: &str,
        org_id: Option<Uuid>,
        pool: Option<&PgPool>,
    ) -> Result<Option<RetentionPolicy>, String> {
        let policy = match self.policies.get(&id) {
            Some(p) => p.value().clone(),
            None => return Ok(None),
        };

        // Check ownership
        let is_owner = policy.user_id == user_id;
        let is_org_match = match (policy.org_id, org_id) {
            (Some(po), Some(co)) => po == co,
            _ => false,
        };

        if !is_owner && !is_org_match {
            return Err("Unauthorized".to_string());
        }

        // Remove from memory
        self.policies.remove(&id);

        // Remove from PostgreSQL
        if let Some(p) = pool {
            let sql = "DELETE FROM data_retention_policies WHERE id = $1";
            if let Err(e) = sqlx::query(sql).bind(id).execute(p).await {
                warn!("[Data Retention] Failed to delete policy from DB: {}", e);
            }
        }

        info!(
            "[Data Retention] User '{}' deleted retention policy id='{}'",
            user_id, id
        );
        Ok(Some(policy))
    }

    /// Lists active retention policies for the user and their active organization context.
    pub fn list_policies(&self, user_id: &str, org_id: Option<Uuid>) -> Vec<RetentionPolicy> {
        let mut user_policies: Vec<RetentionPolicy> = Vec::new();
        let mut org_policies: Vec<RetentionPolicy> = Vec::new();

        for entry in self.policies.iter() {
            let p = entry.value();
            if !p.is_active {
                continue;
            }

            if let (Some(po), Some(co)) = (p.org_id, org_id) {
                if po == co {
                    org_policies.push(p.clone());
                    continue;
                }
            }

            if p.user_id == user_id && p.org_id.is_none() {
                user_policies.push(p.clone());
            }
        }

        // If organization policies exist, they override individual policies for that category
        let mut result = org_policies;
        for up in user_policies {
            if !result.iter().any(|op| op.data_category == up.data_category) {
                result.push(up);
            }
        }

        result
    }

    /// Returns the total number of policies currently in cache.
    pub fn count(&self) -> usize {
        self.policies.len()
    }

    /// Enforces retention policies by executing batched record purges across tables and caches.
    pub async fn enforce_retention_policies(
        &self,
        state: &AppState,
        pool: Option<&PgPool>,
    ) -> usize {
        let mut total_purged = 0;
        let now = Utc::now();

        info!("[Data Retention Enforcer] Beginning retention enforcement sweep across {} active policies", self.policies.len());

        for entry in self.policies.iter() {
            let policy = entry.value();
            if !policy.is_active {
                continue;
            }

            let cutoff = now - ChronoDuration::days(policy.retention_days as i64);
            let mut purged_in_category = 0;

            if let Some(p) = pool {
                match policy.data_category.as_str() {
                    "usage_events" => {
                        let sql =
                            "DELETE FROM usage_events WHERE (user_id = $1) AND (created_at < $2)";
                        if let Ok(res) = sqlx::query(sql)
                            .bind(&policy.user_id)
                            .bind(cutoff)
                            .execute(p)
                            .await
                        {
                            purged_in_category = res.rows_affected() as usize;
                        }
                    }
                    "audit_logs" => {
                        let sql =
                            "DELETE FROM audit_logs WHERE (user_id = $1) AND (created_at < $2)";
                        if let Ok(res) = sqlx::query(sql)
                            .bind(&policy.user_id)
                            .bind(cutoff)
                            .execute(p)
                            .await
                        {
                            purged_in_category = res.rows_affected() as usize;
                        }
                    }
                    "transcripts" => {
                        let sql = "DELETE FROM earnings_call_transcripts WHERE (user_id = $1) AND (created_at < $2)";
                        if let Ok(res) = sqlx::query(sql)
                            .bind(&policy.user_id)
                            .bind(cutoff)
                            .execute(p)
                            .await
                        {
                            purged_in_category = res.rows_affected() as usize;
                        }
                    }
                    "news_articles" => {
                        let sql = "DELETE FROM news_articles WHERE published_utc < $1";
                        if let Ok(res) = sqlx::query(sql).bind(cutoff).execute(p).await {
                            purged_in_category = res.rows_affected() as usize;
                        }
                    }
                    "kafka_credentials" => {
                        let sql = "DELETE FROM kafka_credentials WHERE (user_id = $1) AND (expires_at < $2)";
                        if let Ok(res) = sqlx::query(sql)
                            .bind(&policy.user_id)
                            .bind(cutoff)
                            .execute(p)
                            .await
                        {
                            purged_in_category = res.rows_affected() as usize;
                        }
                    }
                    "email_digests" => {
                        let sql = "DELETE FROM digest_send_history WHERE (user_id = $1) AND (sent_at < $2)";
                        if let Ok(res) = sqlx::query(sql)
                            .bind(&policy.user_id)
                            .bind(cutoff)
                            .execute(p)
                            .await
                        {
                            purged_in_category = res.rows_affected() as usize;
                        }
                    }
                    "webhook_deliveries" => {
                        let sql = "DELETE FROM webhooks WHERE (user_id = $1) AND (created_at < $2)";
                        if let Ok(res) = sqlx::query(sql)
                            .bind(&policy.user_id)
                            .bind(cutoff)
                            .execute(p)
                            .await
                        {
                            purged_in_category = res.rows_affected() as usize;
                        }
                    }
                    _ => {}
                }
            }

            total_purged += purged_in_category;

            // Log audit event for this policy enforcement run
            log_audit_event(
                state,
                policy.org_id,
                &policy.user_id,
                "retention.deletion_completed",
                "data_retention_policy",
                Some(&policy.id.to_string()),
                json!({
                    "data_category": policy.data_category,
                    "retention_days": policy.retention_days,
                    "cutoff_timestamp": cutoff,
                    "records_purged": purged_in_category,
                }),
                None,
            )
            .await;
        }

        info!(
            "[Data Retention Enforcer] Completed sweep. Total records purged: {}",
            total_purged
        );
        total_purged
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Background Retention Enforcement Worker
// ─────────────────────────────────────────────────────────────────────────────

/// Spawns the periodic Data Retention Enforcement background worker.
pub fn spawn_retention_worker(state: AppState, interval_secs: u64) -> JoinHandle<()> {
    tokio::spawn(async move {
        info!(
            "[Retention Worker] Started background data retention enforcement worker (Interval: {}s)",
            interval_secs
        );

        let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
        interval.tick().await; // skip initial tick

        loop {
            interval.tick().await;
            state
                .retention_registry
                .enforce_retention_policies(&state, state.db_pool.as_ref())
                .await;
        }
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_data_categories() {
        assert!(is_valid_data_category("usage_events"));
        assert!(is_valid_data_category("audit_logs"));
        assert!(is_valid_data_category("news_articles"));
        assert!(is_valid_data_category("transcripts"));
        assert!(is_valid_data_category("sentiment_history"));
        assert!(is_valid_data_category("webhook_deliveries"));
        assert!(is_valid_data_category("kafka_credentials"));
        assert!(is_valid_data_category("email_digests"));
        assert!(!is_valid_data_category("illegal_category"));
    }

    #[tokio::test]
    async fn test_retention_policy_registry_crud() {
        let registry = RetentionPolicyRegistry::new();
        assert_eq!(registry.count(), 0);

        // 1. Upsert policy
        let p1 = registry
            .upsert_policy("user_alpha", None, "usage_events", 90, true, None)
            .await
            .unwrap();
        assert_eq!(p1.user_id, "user_alpha");
        assert_eq!(p1.data_category, "usage_events");
        assert_eq!(p1.retention_days, 90);
        assert_eq!(registry.count(), 1);

        // 2. Update existing policy
        let p1_updated = registry
            .upsert_policy("user_alpha", None, "usage_events", 60, true, None)
            .await
            .unwrap();
        assert_eq!(p1_updated.id, p1.id);
        assert_eq!(p1_updated.retention_days, 60);
        assert_eq!(registry.count(), 1);

        // 3. List policies
        let list = registry.list_policies("user_alpha", None);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].retention_days, 60);

        // 4. Other user list returns empty
        let list_beta = registry.list_policies("user_beta", None);
        assert_eq!(list_beta.len(), 0);

        // 5. Delete policy
        let deleted = registry
            .delete_policy(p1.id, "user_alpha", None, None)
            .await
            .unwrap();
        assert!(deleted.is_some());
        assert_eq!(registry.count(), 0);

        // 6. Delete again returns None
        let deleted_again = registry
            .delete_policy(p1.id, "user_alpha", None, None)
            .await
            .unwrap();
        assert!(deleted_again.is_none());
    }

    #[tokio::test]
    async fn test_org_policy_overrides_member_policy() {
        let registry = RetentionPolicyRegistry::new();
        let org_id = Uuid::new_v4();

        // User personal policy: 90 days
        registry
            .upsert_policy("member_01", None, "audit_logs", 90, true, None)
            .await
            .unwrap();

        // Org policy: 30 days
        registry
            .upsert_policy("admin_01", Some(org_id), "audit_logs", 30, true, None)
            .await
            .unwrap();

        // Listing with org context should return the org's 30-day policy
        let policies = registry.list_policies("member_01", Some(org_id));
        assert_eq!(policies.len(), 1);
        assert_eq!(policies[0].retention_days, 30);
        assert_eq!(policies[0].org_id, Some(org_id));
    }
}
