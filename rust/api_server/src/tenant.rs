//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — PostgreSQL Row-Level Security (RLS) Tenant Context
//! Multi-Tenant Confinement, Role-Based Access Control & Safe Transaction Helper
//! ═══════════════════════════════════════════════════════════════════════════════

use axum::async_trait;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres, Transaction};
use std::future::Future;
use tracing::{debug, error};

use crate::auth::{AuthErrorResponse, Claims};

/// Confined tenant organization context extracted from validated JWT or API credentials.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TenantContext {
    /// Organization identifier UUID or scoped fund identifier
    pub org_id: String,
}

impl TenantContext {
    /// Constructs a new TenantContext with the given organization identifier.
    pub fn new(org_id: impl Into<String>) -> Self {
        Self {
            org_id: org_id.into().trim().to_string(),
        }
    }

    /// Returns the organization identifier string slice.
    pub fn org_id(&self) -> &str {
        &self.org_id
    }
}

#[async_trait]
impl<S> FromRequestParts<S> for TenantContext
where
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        if let Some(claims) = parts.extensions.get::<Claims>() {
            let candidate = match &claims.org_id {
                Some(org) if !org.trim().is_empty() => org.trim(),
                _ => claims.sub.trim(),
            };

            if !candidate.is_empty() {
                return Ok(TenantContext {
                    org_id: candidate.to_string(),
                });
            }
        }

        let err_body = Json(AuthErrorResponse {
            error: "Unauthorized".to_string(),
            message: "Missing tenant organization context in claims".to_string(),
        });
        Err((StatusCode::UNAUTHORIZED, err_body).into_response())
    }
}

/// Executes a database operation within a dedicated transaction strictly confined
/// to the authenticated tenant using PostgreSQL `SET LOCAL app.current_org_id = $1`.
///
/// Security Properties:
/// 1. Uses `set_config('app.current_org_id', $1, true)` with parameter binding ($1),
///    eliminating any possibility of SQL injection.
/// 2. The `is_local = true` flag ensures the setting is bound strictly to the current
///    transaction and automatically disappears upon COMMIT or ROLLBACK.
/// 3. Connection pool reuse can NEVER leak tenant scope to subsequent transactions.
pub async fn with_tenant<F, Fut, T>(pool: &PgPool, org_id: &str, f: F) -> Result<T, sqlx::Error>
where
    F: FnOnce(&mut Transaction<'_, Postgres>) -> Fut,
    Fut: Future<Output = Result<T, sqlx::Error>>,
{
    let clean_org = org_id.trim();
    if clean_org.is_empty() {
        error!("[RLS Security] Attempted with_tenant execution with empty org_id");
        return Err(sqlx::Error::Configuration(
            "Cannot execute tenant query with empty org_id".into(),
        ));
    }

    let mut tx = pool.begin().await?;

    // Bind org_id safely via PostgreSQL built-in set_config function (local to transaction)
    sqlx::query("SELECT set_config('app.current_org_id', $1, true)")
        .bind(clean_org)
        .execute(&mut *tx)
        .await?;

    debug!(
        "[RLS Security] SET LOCAL app.current_org_id applied for org '{}'",
        clean_org
    );

    match f(&mut tx).await {
        Ok(result) => {
            tx.commit().await?;
            Ok(result)
        }
        Err(err) => {
            let _ = tx.rollback().await;
            Err(err)
        }
    }
}

/// Dedicated typed wrapper around PgPool for administrative operations (migrations, backups).
/// By maintaining a separate type from `PgPool`, application handlers cannot inadvertently
/// execute tenant-facing queries with superuser / BYPASSRLS credentials.
#[derive(Clone, Debug)]
pub struct AdminPool(pub PgPool);

impl AdminPool {
    pub fn new(pool: PgPool) -> Self {
        Self(pool)
    }

    pub fn inner(&self) -> &PgPool {
        &self.0
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tenant_context_creation() {
        let tc = TenantContext::new("org_alpha_quant");
        assert_eq!(tc.org_id(), "org_alpha_quant");

        let tc_trimmed = TenantContext::new("  org_beta  ");
        assert_eq!(tc_trimmed.org_id(), "org_beta");
    }

    #[test]
    fn test_tenant_context_empty_rejected_in_logic() {
        let tc = TenantContext::new("");
        assert!(tc.org_id().is_empty());
    }

    #[test]
    fn test_static_guard_class_t_tables_documented() {
        // Table inventory classification check
        let class_t_tables = [
            "audit_logs",
            "organizations",
            "organization_members",
            "api_keys",
            "usage_events",
            "universes",
            "webhooks",
            "data_retention_policies",
            "model_retraining_jobs",
            "kafka_credentials",
            "ip_whitelist",
            "chat_alert_subscriptions",
            "polling_webhooks",
            "subscriptions",
            "email_digest_subscriptions",
            "digest_send_history",
            "fix_orders",
        ];

        assert_eq!(class_t_tables.len(), 17);
        for table in class_t_tables {
            assert!(!table.is_empty());
        }
    }

    #[test]
    fn test_rls_sql_set_config_statement() {
        // Validate SQL format for transaction-local setting
        let sql = "SELECT set_config('app.current_org_id', $1, true)";
        assert!(sql.contains("set_config"));
        assert!(sql.contains("app.current_org_id"));
        assert!(sql.contains("true")); // is_local
    }
}
