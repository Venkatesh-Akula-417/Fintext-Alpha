//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Compliance Audit Log & Export Module
//! ═══════════════════════════════════════════════════════════════════════════════
//!
//! Provides an immutable, regulatory-grade audit logging trail for all administrative,
//! security, billing, organization, and authentication actions. Supports indexed
//! queries, pagination, role-based filtering, and RFC 4180 CSV / JSON streaming export.

use chrono::{DateTime, NaiveDate, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;
use tracing::{info, warn};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::state::AppState;

// ─────────────────────────────────────────────────────────────────────────────
// Data Models & DTOs
// ─────────────────────────────────────────────────────────────────────────────

/// An immutable compliance audit log entry recording a critical user or system event.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct AuditLogEntry {
    /// Unique identifier of the audit log record (UUID)
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: Uuid,
    /// Optional organization identifier associated with the action
    #[schema(example = "550e8400-e29b-41d4-a716-446655440001")]
    pub org_id: Option<Uuid>,
    /// Subject/user identifier performing or triggering the event
    #[schema(example = "quant_macro_fund")]
    pub user_id: String,
    /// Categorical action code (e.g. `apikey.create`, `org.member_invited`, `security.ip_whitelist_added`)
    #[schema(example = "apikey.create")]
    pub action: String,
    /// Target entity type modified by this action (e.g. `api_key`, `organization`, `subscription`, `ip_whitelist`)
    #[schema(example = "api_key")]
    pub entity_type: String,
    /// Primary key or identifier of the affected target entity
    #[schema(example = "key_live_987654321")]
    pub entity_id: Option<String>,
    /// Arbitrary structured metadata and context for the event
    pub details: serde_json::Value,
    /// Client IP address from which the action was dispatched
    #[schema(example = "192.168.1.100")]
    pub ip_address: Option<String>,
    /// UTC timestamp at which the action occurred
    pub created_at: DateTime<Utc>,
}

/// Query parameters for retrieving paginated audit logs (`GET /audit/logs`).
#[derive(Debug, Clone, Deserialize, IntoParams, ToSchema)]
pub struct AuditLogsQuery {
    /// Filter events by organization UUID (restricted to org admins/owners)
    #[schema(example = "550e8400-e29b-41d4-a716-446655440001")]
    pub org_id: Option<Uuid>,
    /// Filter events by specific user ID
    #[schema(example = "quant_macro_fund")]
    pub user_id: Option<String>,
    /// Filter events by specific action code (e.g. `apikey.create`, `auth.login`)
    #[schema(example = "apikey.create")]
    pub action: Option<String>,
    /// Filter events by entity type (e.g. `api_key`, `organization`, `security`)
    #[schema(example = "api_key")]
    pub entity_type: Option<String>,
    /// Start date filter (YYYY-MM-DD or RFC3339). Defaults to 30 days prior.
    #[schema(example = "2026-08-01")]
    pub start_date: Option<String>,
    /// End date filter (YYYY-MM-DD or RFC3339). Defaults to current timestamp.
    #[schema(example = "2026-08-30")]
    pub end_date: Option<String>,
    /// Maximum number of audit records to return (default: 100, min: 1, max: 1000)
    #[schema(example = 100)]
    pub limit: Option<usize>,
    /// Pagination offset index (default: 0, min: 0)
    #[schema(example = 0)]
    pub offset: Option<usize>,
}

/// Query parameters for exporting compliance audit logs as CSV or JSON (`GET /audit/export`).
#[derive(Debug, Clone, Deserialize, IntoParams, ToSchema)]
pub struct AuditExportQuery {
    /// Filter events by organization UUID
    #[schema(example = "550e8400-e29b-41d4-a716-446655440001")]
    pub org_id: Option<Uuid>,
    /// Filter events by user ID
    #[schema(example = "quant_macro_fund")]
    pub user_id: Option<String>,
    /// Filter events by action code
    #[schema(example = "apikey.create")]
    pub action: Option<String>,
    /// Filter events by entity type
    #[schema(example = "api_key")]
    pub entity_type: Option<String>,
    /// Start date filter (YYYY-MM-DD or RFC3339)
    #[schema(example = "2026-08-01")]
    pub start_date: Option<String>,
    /// End date filter (YYYY-MM-DD or RFC3339)
    #[schema(example = "2026-08-30")]
    pub end_date: Option<String>,
    /// Export format (`csv` or `json`). Default is `json`.
    #[schema(example = "csv")]
    pub format: Option<String>,
}

/// Paginated audit logs response payload.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AuditLogsResponse {
    /// Paginated array of audit log entries
    pub logs: Vec<AuditLogEntry>,
    /// Total count of matching audit log entries before pagination
    #[schema(example = 42)]
    pub total: usize,
    /// Effective limit parameter applied
    #[schema(example = 100)]
    pub limit: usize,
    /// Effective offset parameter applied
    #[schema(example = 0)]
    pub offset: usize,
}

/// Structured JSON payload for exported audit log reports.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AuditLogExportResponse {
    /// UTC timestamp when this export report was generated
    pub exported_at: DateTime<Utc>,
    /// Total number of exported audit records
    #[schema(example = 42)]
    pub total: usize,
    /// Complete array of matching audit log records
    pub logs: Vec<AuditLogEntry>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Registry & In-Memory Store
// ─────────────────────────────────────────────────────────────────────────────

/// High-performance concurrent registry for compliance audit log records with PostgreSQL backing.
#[derive(Debug, Clone)]
pub struct AuditLogRegistry {
    entries: Arc<DashMap<Uuid, AuditLogEntry>>,
}

impl Default for AuditLogRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl AuditLogRegistry {
    /// Creates a new, empty in-memory audit log registry.
    pub fn new() -> Self {
        Self {
            entries: Arc::new(DashMap::new()),
        }
    }

    /// Appends a new immutable audit record to the registry.
    pub fn insert(&self, entry: AuditLogEntry) {
        self.entries.insert(entry.id, entry);
    }

    /// Queries the audit log registry with filtering, RBAC constraints, date parsing, and pagination.
    pub fn query(
        &self,
        query: &AuditLogsQuery,
        enforced_user_id: Option<&str>,
        enforced_org_id: Option<Uuid>,
    ) -> Result<(Vec<AuditLogEntry>, usize), String> {
        let (start_dt, end_dt) =
            parse_date_window(query.start_date.as_deref(), query.end_date.as_deref())?;

        let limit = query.limit.unwrap_or(100);
        if limit == 0 || limit > 1000 {
            return Err("limit must be between 1 and 1000".to_string());
        }
        let offset = query.offset.unwrap_or(0);

        let mut matched: Vec<AuditLogEntry> = self
            .entries
            .iter()
            .map(|r| r.value().clone())
            .filter(|e| {
                // 1. RBAC constraints
                if let Some(uid) = enforced_user_id {
                    if e.user_id != uid {
                        return false;
                    }
                }
                if let Some(oid) = enforced_org_id {
                    if e.org_id != Some(oid) {
                        return false;
                    }
                }

                // 2. Query filters
                if let Some(target_oid) = query.org_id {
                    if e.org_id != Some(target_oid) {
                        return false;
                    }
                }
                if let Some(ref target_uid) = query.user_id {
                    if !e.user_id.eq_ignore_ascii_case(target_uid.trim()) {
                        return false;
                    }
                }
                if let Some(ref target_act) = query.action {
                    if !e.action.eq_ignore_ascii_case(target_act.trim()) {
                        return false;
                    }
                }
                if let Some(ref target_ent) = query.entity_type {
                    if !e.entity_type.eq_ignore_ascii_case(target_ent.trim()) {
                        return false;
                    }
                }

                // 3. Date window
                if e.created_at < start_dt || e.created_at > end_dt {
                    return false;
                }

                true
            })
            .collect();

        // Sort descending by created_at (newest first)
        matched.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        let total = matched.len();
        let paginated = matched.into_iter().skip(offset).take(limit).collect();

        Ok((paginated, total))
    }

    /// Queries all matching records for export (up to 10,000 records).
    pub fn query_for_export(
        &self,
        query: &AuditExportQuery,
        enforced_user_id: Option<&str>,
        enforced_org_id: Option<Uuid>,
    ) -> Result<Vec<AuditLogEntry>, String> {
        let (start_dt, end_dt) =
            parse_date_window(query.start_date.as_deref(), query.end_date.as_deref())?;

        let mut matched: Vec<AuditLogEntry> = self
            .entries
            .iter()
            .map(|r| r.value().clone())
            .filter(|e| {
                if let Some(uid) = enforced_user_id {
                    if e.user_id != uid {
                        return false;
                    }
                }
                if let Some(oid) = enforced_org_id {
                    if e.org_id != Some(oid) {
                        return false;
                    }
                }

                if let Some(target_oid) = query.org_id {
                    if e.org_id != Some(target_oid) {
                        return false;
                    }
                }
                if let Some(ref target_uid) = query.user_id {
                    if !e.user_id.eq_ignore_ascii_case(target_uid.trim()) {
                        return false;
                    }
                }
                if let Some(ref target_act) = query.action {
                    if !e.action.eq_ignore_ascii_case(target_act.trim()) {
                        return false;
                    }
                }
                if let Some(ref target_ent) = query.entity_type {
                    if !e.entity_type.eq_ignore_ascii_case(target_ent.trim()) {
                        return false;
                    }
                }

                if e.created_at < start_dt || e.created_at > end_dt {
                    return false;
                }

                true
            })
            .collect();

        matched.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        // Max export limit 10,000
        if matched.len() > 10_000 {
            matched.truncate(10_000);
        }

        Ok(matched)
    }

    /// Serializes a slice of audit log entries into RFC 4180 compliant CSV bytes.
    pub fn export_csv(logs: &[AuditLogEntry]) -> Result<Vec<u8>, String> {
        let mut wtr = csv::WriterBuilder::new()
            .has_headers(true)
            .from_writer(Vec::new());

        // Write header
        wtr.write_record([
            "id",
            "org_id",
            "user_id",
            "action",
            "entity_type",
            "entity_id",
            "details",
            "ip_address",
            "created_at",
        ])
        .map_err(|e| format!("CSV write header failed: {}", e))?;

        for log in logs {
            let details_str = serde_json::to_string(&log.details).unwrap_or_default();
            wtr.write_record(&[
                log.id.to_string(),
                log.org_id.map(|o| o.to_string()).unwrap_or_default(),
                log.user_id.clone(),
                log.action.clone(),
                log.entity_type.clone(),
                log.entity_id.clone().unwrap_or_default(),
                details_str,
                log.ip_address.clone().unwrap_or_default(),
                log.created_at.to_rfc3339(),
            ])
            .map_err(|e| format!("CSV write record failed: {}", e))?;
        }

        wtr.into_inner()
            .map_err(|e| format!("CSV flush failed: {}", e))
    }

    /// Serializes a slice of audit log entries into structured JSON string.
    pub fn export_json(logs: &[AuditLogEntry]) -> Result<String, String> {
        let resp = AuditLogExportResponse {
            exported_at: Utc::now(),
            total: logs.len(),
            logs: logs.to_vec(),
        };
        serde_json::to_string_pretty(&resp).map_err(|e| format!("JSON serialization failed: {}", e))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Date Parsing & Helpers
// ─────────────────────────────────────────────────────────────────────────────

fn parse_date_window(
    start_str: Option<&str>,
    end_str: Option<&str>,
) -> Result<(DateTime<Utc>, DateTime<Utc>), String> {
    let now = Utc::now();
    let default_start = now - chrono::Duration::days(30);

    let start_dt = if let Some(s) = start_str {
        if s.trim().is_empty() {
            default_start
        } else if let Ok(dt) = DateTime::parse_from_rfc3339(s.trim()) {
            dt.with_timezone(&Utc)
        } else if let Ok(d) = NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d") {
            d.and_hms_opt(0, 0, 0)
                .map(|naive| DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc))
                .ok_or_else(|| "Invalid start_date time".to_string())?
        } else {
            return Err(format!(
                "Invalid start_date format '{}'. Use YYYY-MM-DD or RFC3339.",
                s
            ));
        }
    } else {
        default_start
    };

    let end_dt = if let Some(e) = end_str {
        if e.trim().is_empty() {
            now
        } else if let Ok(dt) = DateTime::parse_from_rfc3339(e.trim()) {
            dt.with_timezone(&Utc)
        } else if let Ok(d) = NaiveDate::parse_from_str(e.trim(), "%Y-%m-%d") {
            d.and_hms_opt(23, 59, 59)
                .map(|naive| DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc))
                .ok_or_else(|| "Invalid end_date time".to_string())?
        } else {
            return Err(format!(
                "Invalid end_date format '{}'. Use YYYY-MM-DD or RFC3339.",
                e
            ));
        }
    } else {
        now
    };

    if start_dt > end_dt {
        return Err(format!(
            "start_date ({}) must not be after end_date ({})",
            start_dt.to_rfc3339(),
            end_dt.to_rfc3339()
        ));
    }

    Ok((start_dt, end_dt))
}

// ─────────────────────────────────────────────────────────────────────────────
// Core Logging Helper Function
// ─────────────────────────────────────────────────────────────────────────────

/// High-level, fail-safe logging helper called across all critical route handlers.
///
/// Records the audit event to the in-memory registry immediately, and spawns an
/// asynchronous persistence write to PostgreSQL if a DB pool is configured.
pub async fn log_audit_event(
    state: &AppState,
    org_id: Option<Uuid>,
    user_id: &str,
    action: &str,
    entity_type: &str,
    entity_id: Option<&str>,
    details: serde_json::Value,
    ip_address: Option<&str>,
) -> AuditLogEntry {
    let entry = AuditLogEntry {
        id: Uuid::new_v4(),
        org_id,
        user_id: user_id.to_string(),
        action: action.to_string(),
        entity_type: entity_type.to_string(),
        entity_id: entity_id.map(|s| s.to_string()),
        details: details.clone(),
        ip_address: ip_address.map(|s| s.to_string()),
        created_at: Utc::now(),
    };

    // 1. Insert into in-memory registry (zero-latency durability)
    state.audit_log_registry.insert(entry.clone());

    // 2. Persist to PostgreSQL asynchronously if available
    if let Some(pool) = state.db_pool.clone() {
        let entry_clone = entry.clone();
        tokio::spawn(async move {
            let res = sqlx::query(
                r#"
                INSERT INTO audit_logs (id, org_id, user_id, action, entity_type, entity_id, details, ip_address, created_at)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                "#,
            )
            .bind(entry_clone.id)
            .bind(entry_clone.org_id)
            .bind(entry_clone.user_id)
            .bind(entry_clone.action)
            .bind(entry_clone.entity_type)
            .bind(entry_clone.entity_id)
            .bind(entry_clone.details)
            .bind(entry_clone.ip_address)
            .bind(entry_clone.created_at)
            .execute(&pool)
            .await;

            if let Err(e) = res {
                warn!(
                    "Failed to persist audit log entry {} to PostgreSQL: {}",
                    entry_clone.id, e
                );
            }
        });
    }

    info!(
        audit_id = %entry.id,
        user = %entry.user_id,
        action = %entry.action,
        entity = %entry.entity_type,
        "Compliance audit event logged"
    );

    entry
}

/// Initializes the `audit_logs` table and indexes in PostgreSQL if not already present.
pub async fn init_audit_logs_table(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS audit_logs (
            id UUID PRIMARY KEY,
            org_id UUID NULL,
            user_id TEXT NOT NULL,
            action TEXT NOT NULL,
            entity_type TEXT NOT NULL,
            entity_id TEXT NULL,
            details JSONB NOT NULL DEFAULT '{}'::jsonb,
            ip_address TEXT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        );
        CREATE INDEX IF NOT EXISTS idx_audit_logs_org_time ON audit_logs(org_id, created_at DESC);
        CREATE INDEX IF NOT EXISTS idx_audit_logs_user_time ON audit_logs(user_id, created_at DESC);
        CREATE INDEX IF NOT EXISTS idx_audit_logs_action_time ON audit_logs(action, created_at DESC);
        "#,
    )
    .execute(pool)
    .await?;

    info!("Audit logs database schema verified");
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_registry_insert_and_query() {
        let reg = AuditLogRegistry::new();
        let uid = "test_user_01";
        let entry = AuditLogEntry {
            id: Uuid::new_v4(),
            org_id: None,
            user_id: uid.to_string(),
            action: "apikey.create".to_string(),
            entity_type: "api_key".to_string(),
            entity_id: Some("key_123".to_string()),
            details: serde_json::json!({"label": "Production Key"}),
            ip_address: Some("127.0.0.1".to_string()),
            created_at: Utc::now(),
        };

        reg.insert(entry.clone());

        let q = AuditLogsQuery {
            org_id: None,
            user_id: Some(uid.to_string()),
            action: Some("apikey.create".to_string()),
            entity_type: None,
            start_date: None,
            end_date: None,
            limit: Some(10),
            offset: Some(0),
        };

        let (res, total) = reg.query(&q, None, None).unwrap();
        assert_eq!(total, 1);
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].id, entry.id);
        assert_eq!(res[0].action, "apikey.create");
    }

    #[test]
    fn test_audit_csv_and_json_export() {
        let entry = AuditLogEntry {
            id: Uuid::new_v4(),
            org_id: None,
            user_id: "trader_99".to_string(),
            action: "org.member_invited".to_string(),
            entity_type: "organization_member".to_string(),
            entity_id: Some("invite_456".to_string()),
            details: serde_json::json!({"role": "analyst", "email": "analyst@fund.com"}),
            ip_address: Some("10.0.0.5".to_string()),
            created_at: Utc::now(),
        };

        let logs = vec![entry.clone()];

        // CSV Export
        let csv_bytes = AuditLogRegistry::export_csv(&logs).unwrap();
        let csv_str = String::from_utf8(csv_bytes).unwrap();
        assert!(csv_str.contains("id,org_id,user_id,action,entity_type"));
        assert!(csv_str.contains("trader_99"));
        assert!(csv_str.contains("org.member_invited"));

        // JSON Export
        let json_str = AuditLogRegistry::export_json(&logs).unwrap();
        assert!(json_str.contains("\"total\": 1"));
        assert!(json_str.contains("trader_99"));
        assert!(json_str.contains("org.member_invited"));
    }

    #[test]
    fn test_date_window_validation() {
        // Valid date range
        let res = parse_date_window(Some("2026-08-01"), Some("2026-08-30"));
        assert!(res.is_ok());

        // Inverted range
        let res = parse_date_window(Some("2026-08-30"), Some("2026-08-01"));
        assert!(res.is_err());

        // Invalid date string
        let res = parse_date_window(Some("not-a-date"), None);
        assert!(res.is_err());
    }
}
