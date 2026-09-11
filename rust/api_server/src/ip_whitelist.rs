//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — IP Whitelisting & CIDR Access Control Module
//! ═══════════════════════════════════════════════════════════════════════════════
//!
//! Provides per-user and per-organization IP and CIDR range access control,
//! sub-microsecond in-memory matching with PostgreSQL durability, and Axum
//! security middleware for zero-trust request enforcement.

use axum::{
    extract::{Path, Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use ipnet::IpNet;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use tracing::{info, warn};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::audit_logs::log_audit_event;
use crate::auth::Claims;
use crate::state::AppState;

// ─────────────────────────────────────────────────────────────────────────────
// Data Models & DTOs
// ─────────────────────────────────────────────────────────────────────────────

/// Represents a single configured IP or CIDR whitelist entry.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct IpWhitelistEntry {
    /// Unique identifier of the whitelist entry (UUID)
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: Uuid,
    /// Identifier of the owning user
    #[schema(example = "trader_007")]
    pub user_id: String,
    /// Whitelisted IP address or CIDR subnet (e.g. `203.0.113.0/24` or `198.51.100.14`)
    #[schema(example = "203.0.113.0/24")]
    pub ip_or_cidr: String,
    /// Optional human-readable description (e.g. "Primary Office VPN")
    #[schema(example = "Primary Office VPN")]
    pub description: Option<String>,
    /// Creation timestamp (UTC)
    pub created_at: DateTime<Utc>,
}

/// Request payload for adding a new IP address or CIDR range to the whitelist (`POST /security/ip-whitelist`).
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct AddIpWhitelistRequest {
    /// IP address or CIDR subnet to whitelist (e.g. "203.0.113.0/24", "10.0.0.1", "2001:db8::/32")
    #[schema(example = "203.0.113.0/24")]
    pub ip_or_cidr: String,
    /// Optional label or note for this entry
    #[serde(default)]
    #[schema(example = "London Trading Floor VPN")]
    pub description: Option<String>,
}

/// Response payload containing the list of active IP whitelist entries (`GET /security/ip-whitelist`).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct ListIpWhitelistResponse {
    /// Array of whitelist entries
    pub entries: Vec<IpWhitelistEntry>,
    /// Total count of active whitelist rules
    #[schema(example = 2)]
    pub count: usize,
}

/// Response payload upon deleting an IP whitelist entry (`DELETE /security/ip-whitelist/{id}`).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct DeleteIpWhitelistResponse {
    /// Operation status
    #[schema(example = "success")]
    pub status: String,
    /// Diagnostic summary message
    #[schema(example = "IP whitelist entry deleted successfully")]
    pub message: String,
    /// ID of the deleted entry
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: Uuid,
}

/// Error response structure for IP whitelisting operations.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct IpWhitelistErrorResponse {
    /// Error code
    #[schema(example = "Forbidden")]
    pub error: String,
    /// Human-readable diagnostic description
    #[schema(example = "IP address not allowed")]
    pub message: String,
}

// ─────────────────────────────────────────────────────────────────────────────
// CIDR Validation & Matching Helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Validates that an input string is a valid IPv4/IPv6 address or CIDR block,
/// returning the parsed canonical `IpNet`.
pub fn validate_and_canonicalize_cidr(ip_or_cidr: &str) -> Result<IpNet, String> {
    let trimmed = ip_or_cidr.trim();
    if trimmed.is_empty() {
        return Err("IP address or CIDR range cannot be empty".to_string());
    }

    // Try parsing as explicit CIDR first (e.g. "203.0.113.0/24" or "10.0.0.1/32")
    if let Ok(net) = trimmed.parse::<IpNet>() {
        return Ok(net);
    }

    // Fallback: parse as single IP address and convert to /32 (IPv4) or /128 (IPv6)
    if let Ok(ip) = trimmed.parse::<IpAddr>() {
        return Ok(IpNet::from(ip));
    }

    Err(format!(
        "Invalid IP address or CIDR format: '{}'. Expected valid IP (e.g. 192.0.2.1) or CIDR (e.g. 192.0.2.0/24)",
        trimmed
    ))
}

/// Extracts the client's public or local IP address from request headers or socket info.
pub fn extract_client_ip<B>(req: &Request<B>) -> IpAddr {
    // 1. Check X-Forwarded-For header (standard for reverse proxies, taking first client IP)
    if let Some(forwarded) = req
        .headers()
        .get("x-forwarded-for")
        .and_then(|h| h.to_str().ok())
    {
        if let Some(first_ip) = forwarded.split(',').next() {
            if let Ok(ip) = first_ip.trim().parse::<IpAddr>() {
                return ip;
            }
        }
    }

    // 2. Check X-Real-IP header
    if let Some(real_ip) = req.headers().get("x-real-ip").and_then(|h| h.to_str().ok()) {
        if let Ok(ip) = real_ip.trim().parse::<IpAddr>() {
            return ip;
        }
    }

    // 3. Check ConnectInfo from extensions if configured
    if let Some(connect_info) = req
        .extensions()
        .get::<axum::extract::ConnectInfo<SocketAddr>>()
    {
        return connect_info.0.ip();
    }

    // 4. Default fallback to loopback
    IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))
}

// ─────────────────────────────────────────────────────────────────────────────
// Thread-Safe In-Memory IP Whitelist Registry
// ─────────────────────────────────────────────────────────────────────────────

/// High-performance thread-safe registry for IP whitelist entries with dual-store PostgreSQL sync.
#[derive(Debug, Clone)]
pub struct IpWhitelistRegistry {
    entries: Arc<DashMap<Uuid, IpWhitelistEntry>>,
    user_index: Arc<DashMap<String, Vec<Uuid>>>,
}

impl Default for IpWhitelistRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl IpWhitelistRegistry {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(DashMap::new()),
            user_index: Arc::new(DashMap::new()),
        }
    }

    /// Adds a new IP/CIDR rule for a user.
    pub async fn add_entry(
        &self,
        user_id: &str,
        ip_or_cidr: &str,
        description: Option<String>,
        pool: Option<&PgPool>,
    ) -> Result<IpWhitelistEntry, String> {
        let canonical_net = validate_and_canonicalize_cidr(ip_or_cidr)?;
        let entry_id = Uuid::new_v4();
        let now = Utc::now();
        let canonical_str = canonical_net.to_string();

        let entry = IpWhitelistEntry {
            id: entry_id,
            user_id: user_id.to_string(),
            ip_or_cidr: canonical_str.clone(),
            description: description.clone(),
            created_at: now,
        };

        // In-memory insert
        self.entries.insert(entry_id, entry.clone());
        self.user_index
            .entry(user_id.to_string())
            .or_default()
            .push(entry_id);

        // PostgreSQL sync if connected
        if let Some(pool) = pool {
            let query_res = sqlx::query(
                r#"
                INSERT INTO ip_whitelist (id, user_id, ip_or_cidr, description, created_at)
                VALUES ($1, $2, $3, $4, $5)
                ON CONFLICT (id) DO NOTHING
                "#,
            )
            .bind(entry_id)
            .bind(user_id)
            .bind(&canonical_str)
            .bind(description.as_deref())
            .bind(now)
            .execute(pool)
            .await;

            if let Err(e) = query_res {
                warn!(
                    "[Security] Failed to persist IP whitelist entry to PostgreSQL: {}",
                    e
                );
            }
        }

        info!(
            "[Security] Added IP whitelist entry {} ('{}') for user '{}'",
            entry_id, canonical_str, user_id
        );
        Ok(entry)
    }

    /// Lists all IP whitelist entries for a user.
    pub fn list_entries(&self, user_id: &str) -> Vec<IpWhitelistEntry> {
        if let Some(ids) = self.user_index.get(user_id) {
            ids.iter()
                .filter_map(|id| self.entries.get(id).map(|e| e.clone()))
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Deletes an IP whitelist entry by ID if owned by the user.
    pub async fn delete_entry(
        &self,
        user_id: &str,
        entry_id: Uuid,
        pool: Option<&PgPool>,
    ) -> Result<bool, String> {
        let entry = match self.entries.get(&entry_id) {
            Some(e) if e.user_id == user_id => e.clone(),
            _ => return Ok(false),
        };

        // In-memory delete
        self.entries.remove(&entry_id);
        if let Some(mut ids) = self.user_index.get_mut(user_id) {
            ids.retain(|id| *id != entry_id);
        }

        // PostgreSQL sync if connected
        if let Some(pool) = pool {
            let query_res = sqlx::query(
                r#"
                DELETE FROM ip_whitelist
                WHERE id = $1 AND user_id = $2
                "#,
            )
            .bind(entry_id)
            .bind(user_id)
            .execute(pool)
            .await;

            if let Err(e) = query_res {
                warn!(
                    "[Security] Failed to delete IP whitelist entry from PostgreSQL: {}",
                    e
                );
            }
        }

        info!(
            "[Security] Removed IP whitelist entry {} ('{}') for user '{}'",
            entry_id, entry.ip_or_cidr, user_id
        );
        Ok(true)
    }

    /// Checks if a client IP address is allowed for a user.
    ///
    /// Rules:
    /// - If user has NO whitelist entries (empty), access is ALLOWED by default.
    /// - If user has 1+ whitelist entries, the client IP MUST match at least one CIDR/IP.
    pub fn is_user_ip_allowed(&self, user_id: &str, client_ip: IpAddr) -> bool {
        let entries = self.list_entries(user_id);
        if entries.is_empty() {
            // Default open
            return true;
        }

        for entry in entries {
            if let Ok(net) = entry.ip_or_cidr.parse::<IpNet>() {
                if net.contains(&client_ip) {
                    return true;
                }
            } else if let Ok(ip) = entry.ip_or_cidr.parse::<IpAddr>() {
                if ip == client_ip {
                    return true;
                }
            }
        }

        false
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Database Initialization
// ─────────────────────────────────────────────────────────────────────────────

/// Initializes the `ip_whitelist` table in PostgreSQL if it does not already exist.
pub async fn init_ip_whitelist_db(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS ip_whitelist (
            id UUID PRIMARY KEY,
            user_id VARCHAR(128) NOT NULL,
            ip_or_cidr TEXT NOT NULL,
            description TEXT,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        );
        CREATE INDEX IF NOT EXISTS idx_ip_whitelist_user_id ON ip_whitelist(user_id);
        "#,
    )
    .execute(pool)
    .await?;

    info!("[Security] Verified PostgreSQL 'ip_whitelist' schema");
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Axum Middleware
// ─────────────────────────────────────────────────────────────────────────────

/// Middleware that enforces IP whitelist checks on authenticated requests.
///
/// Execution details:
/// - Runs after `auth_middleware` (which injects `Claims` into request extensions).
/// - Extracts client IP from `X-Forwarded-For`, `X-Real-IP`, or connection info.
/// - If the authenticated user has configured IP whitelist entries and the client IP
///   is outside all allowed subnets, terminates request immediately with `403 Forbidden`.
pub async fn ip_whitelist_middleware(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Response {
    if let Some(claims) = req.extensions().get::<Claims>() {
        let client_ip = extract_client_ip(&req);
        let user_id = &claims.sub;

        if !state
            .ip_whitelist_registry
            .is_user_ip_allowed(user_id, client_ip)
        {
            warn!(
                "[Security] Rejected request from IP '{}' for user '{}' — IP not whitelisted",
                client_ip, user_id
            );
            let err_body = Json(IpWhitelistErrorResponse {
                error: "Forbidden".to_string(),
                message: "IP address not allowed".to_string(),
            });
            return (StatusCode::FORBIDDEN, err_body).into_response();
        }
    }

    next.run(req).await
}

// ─────────────────────────────────────────────────────────────────────────────
// Route Handlers
// ─────────────────────────────────────────────────────────────────────────────

/// Retrieve IP Whitelist
///
/// Returns all active IP addresses and CIDR subnets allowed for the authenticated user.
#[utoipa::path(
    get,
    path = "/security/ip-whitelist",
    responses(
        (status = 200, description = "Active IP whitelist entries retrieved successfully", body = ListIpWhitelistResponse),
        (status = 401, description = "Unauthorized - Missing or invalid JWT / API key", body = AuthErrorResponse)
    ),
    security(
        ("BearerAuth" = []),
        ("ApiKeyAuth" = [])
    ),
    tag = "Security & IP Whitelisting"
)]
pub async fn get_ip_whitelist_handler(
    State(state): State<AppState>,
    req: Request,
) -> Result<Json<ListIpWhitelistResponse>, (StatusCode, Json<IpWhitelistErrorResponse>)> {
    let claims = req.extensions().get::<Claims>().ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            Json(IpWhitelistErrorResponse {
                error: "Unauthorized".to_string(),
                message: "Missing authentication claims".to_string(),
            }),
        )
    })?;

    let entries = state.ip_whitelist_registry.list_entries(&claims.sub);
    let count = entries.len();

    Ok(Json(ListIpWhitelistResponse { entries, count }))
}

/// Add IP or CIDR to Whitelist
///
/// Adds an allowed IP address or CIDR range for the authenticated user.
/// Once at least one entry is added, only requests matching the whitelist will be permitted.
#[utoipa::path(
    post,
    path = "/security/ip-whitelist",
    request_body = AddIpWhitelistRequest,
    responses(
        (status = 201, description = "IP address / CIDR subnet added to whitelist", body = IpWhitelistEntry),
        (status = 400, description = "Bad Request - Invalid IP address or CIDR format", body = IpWhitelistErrorResponse),
        (status = 401, description = "Unauthorized - Missing or invalid JWT / API key", body = AuthErrorResponse)
    ),
    security(
        ("BearerAuth" = []),
        ("ApiKeyAuth" = [])
    ),
    tag = "Security & IP Whitelisting"
)]
pub async fn add_ip_whitelist_handler(
    State(state): State<AppState>,
    req: Request,
) -> Result<(StatusCode, Json<IpWhitelistEntry>), (StatusCode, Json<IpWhitelistErrorResponse>)> {
    let claims = req.extensions().get::<Claims>().cloned().ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            Json(IpWhitelistErrorResponse {
                error: "Unauthorized".to_string(),
                message: "Missing authentication claims".to_string(),
            }),
        )
    })?;

    // Extract body
    let body_bytes = axum::body::to_bytes(req.into_body(), 1024 * 64)
        .await
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(IpWhitelistErrorResponse {
                    error: "Bad Request".to_string(),
                    message: format!("Failed to read request body: {}", e),
                }),
            )
        })?;

    let payload: AddIpWhitelistRequest = serde_json::from_slice(&body_bytes).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(IpWhitelistErrorResponse {
                error: "Bad Request".to_string(),
                message: format!("Invalid JSON payload: {}", e),
            }),
        )
    })?;

    match state
        .ip_whitelist_registry
        .add_entry(
            &claims.sub,
            &payload.ip_or_cidr,
            payload.description,
            state.db_pool.as_ref(),
        )
        .await
    {
        Ok(entry) => {
            log_audit_event(
                &state,
                None,
                &claims.sub,
                "security.ip_whitelist_added",
                "ip_whitelist",
                Some(&entry.id.to_string()),
                serde_json::json!({"ip_or_cidr": entry.ip_or_cidr, "description": entry.description}),
                None,
            )
            .await;

            Ok((StatusCode::CREATED, Json(entry)))
        }
        Err(err) => Err((
            StatusCode::BAD_REQUEST,
            Json(IpWhitelistErrorResponse {
                error: "Bad Request".to_string(),
                message: err,
            }),
        )),
    }
}

/// Delete IP Whitelist Entry
///
/// Removes an IP whitelist entry by ID. If all entries are removed, access reverts to default open.
#[utoipa::path(
    delete,
    path = "/security/ip-whitelist/{id}",
    params(
        ("id" = Uuid, Path, description = "Unique UUID of the whitelist entry to remove")
    ),
    responses(
        (status = 200, description = "IP whitelist entry deleted successfully", body = DeleteIpWhitelistResponse),
        (status = 404, description = "Not Found - Whitelist entry does not exist or does not belong to user", body = IpWhitelistErrorResponse),
        (status = 401, description = "Unauthorized - Missing or invalid JWT / API key", body = AuthErrorResponse)
    ),
    security(
        ("BearerAuth" = []),
        ("ApiKeyAuth" = [])
    ),
    tag = "Security & IP Whitelisting"
)]
pub async fn delete_ip_whitelist_handler(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    req: Request,
) -> Result<Json<DeleteIpWhitelistResponse>, (StatusCode, Json<IpWhitelistErrorResponse>)> {
    let claims = req.extensions().get::<Claims>().ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            Json(IpWhitelistErrorResponse {
                error: "Unauthorized".to_string(),
                message: "Missing authentication claims".to_string(),
            }),
        )
    })?;

    match state
        .ip_whitelist_registry
        .delete_entry(&claims.sub, id, state.db_pool.as_ref())
        .await
    {
        Ok(true) => {
            log_audit_event(
                &state,
                None,
                &claims.sub,
                "security.ip_whitelist_deleted",
                "ip_whitelist",
                Some(&id.to_string()),
                serde_json::json!({"entry_id": id}),
                None,
            )
            .await;

            Ok(Json(DeleteIpWhitelistResponse {
                status: "success".to_string(),
                message: "IP whitelist entry deleted successfully".to_string(),
                id,
            }))
        }
        Ok(false) => Err((
            StatusCode::NOT_FOUND,
            Json(IpWhitelistErrorResponse {
                error: "Not Found".to_string(),
                message: format!("IP whitelist entry '{}' not found", id),
            }),
        )),
        Err(err) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(IpWhitelistErrorResponse {
                error: "Internal Server Error".to_string(),
                message: err,
            }),
        )),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cidr_validation_and_parsing() {
        // Valid IPv4 single address -> /32
        let res = validate_and_canonicalize_cidr("192.168.1.1").unwrap();
        assert_eq!(res.to_string(), "192.168.1.1/32");

        // Valid IPv4 CIDR subnet
        let res = validate_and_canonicalize_cidr("203.0.113.0/24").unwrap();
        assert_eq!(res.to_string(), "203.0.113.0/24");

        // Valid IPv6 address -> /128
        let res = validate_and_canonicalize_cidr("::1").unwrap();
        assert_eq!(res.to_string(), "::1/128");

        // Valid IPv6 CIDR subnet
        let res = validate_and_canonicalize_cidr("2001:db8::/32").unwrap();
        assert_eq!(res.to_string(), "2001:db8::/32");

        // Invalid formats
        assert!(validate_and_canonicalize_cidr("").is_err());
        assert!(validate_and_canonicalize_cidr("invalid.ip.format").is_err());
        assert!(validate_and_canonicalize_cidr("256.256.256.256").is_err());
        assert!(validate_and_canonicalize_cidr("192.168.1.0/33").is_err());
    }

    #[tokio::test]
    async fn test_ip_whitelist_registry_crud_and_matching() {
        let registry = IpWhitelistRegistry::new();
        let user = "trader_alice";

        // 1. Initial state: empty whitelist -> allowed by default
        assert!(registry.is_user_ip_allowed(user, "192.168.1.50".parse().unwrap()));
        assert!(registry.is_user_ip_allowed(user, "8.8.8.8".parse().unwrap()));

        // 2. Add an IPv4 subnet
        let entry1 = registry
            .add_entry(
                user,
                "192.168.1.0/24",
                Some("Office Subnet".to_string()),
                None,
            )
            .await
            .expect("Failed to add entry");
        assert_eq!(entry1.ip_or_cidr, "192.168.1.0/24");

        // 3. Now only matching IPs in 192.168.1.0/24 should be allowed
        assert!(registry.is_user_ip_allowed(user, "192.168.1.1".parse().unwrap()));
        assert!(registry.is_user_ip_allowed(user, "192.168.1.254".parse().unwrap()));
        assert!(!registry.is_user_ip_allowed(user, "192.168.2.1".parse().unwrap()));
        assert!(!registry.is_user_ip_allowed(user, "8.8.8.8".parse().unwrap()));

        // 4. Add a specific public IP
        let entry2 = registry
            .add_entry(user, "203.0.113.42", Some("Home IP".to_string()), None)
            .await
            .expect("Failed to add entry 2");
        assert_eq!(entry2.ip_or_cidr, "203.0.113.42/32");

        // 5. Both subnet and single IP allowed
        assert!(registry.is_user_ip_allowed(user, "192.168.1.100".parse().unwrap()));
        assert!(registry.is_user_ip_allowed(user, "203.0.113.42".parse().unwrap()));
        assert!(!registry.is_user_ip_allowed(user, "203.0.113.43".parse().unwrap()));

        // 6. List entries
        let entries = registry.list_entries(user);
        assert_eq!(entries.len(), 2);

        // 7. Delete entry
        let deleted = registry
            .delete_entry(user, entry1.id, None)
            .await
            .expect("Delete error");
        assert!(deleted);

        // 8. Verify only entry 2 remains
        assert!(!registry.is_user_ip_allowed(user, "192.168.1.100".parse().unwrap()));
        assert!(registry.is_user_ip_allowed(user, "203.0.113.42".parse().unwrap()));

        // 9. Delete last entry -> reverts to empty whitelist -> allowed by default
        let deleted2 = registry
            .delete_entry(user, entry2.id, None)
            .await
            .expect("Delete error 2");
        assert!(deleted2);
        assert!(registry.is_user_ip_allowed(user, "8.8.8.8".parse().unwrap()));
    }

    #[tokio::test]
    async fn test_user_isolation() {
        let registry = IpWhitelistRegistry::new();
        let user_a = "user_a";
        let user_b = "user_b";

        // User A restricts to 10.0.0.0/8
        registry
            .add_entry(user_a, "10.0.0.0/8", None, None)
            .await
            .unwrap();

        // User A blocked from 192.168.1.1
        assert!(!registry.is_user_ip_allowed(user_a, "192.168.1.1".parse().unwrap()));

        // User B has no rules -> allowed
        assert!(registry.is_user_ip_allowed(user_b, "192.168.1.1".parse().unwrap()));
    }
}
