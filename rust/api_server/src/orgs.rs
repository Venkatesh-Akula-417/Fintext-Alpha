//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Multi-User Team Access & Organizations Architecture
//! ═══════════════════════════════════════════════════════════════════════════════
//!
//! Provides organizational multi-tenancy, team hierarchy, Role-Based Access Control
//! (RBAC: admin, member, viewer), organizational context switching, and shared quotas.
//! ═══════════════════════════════════════════════════════════════════════════════

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use axum::Extension;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;
use tracing::{debug, error, info};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::audit_logs::log_audit_event;
use crate::auth::{generate_jwt_with_org, AuthErrorResponse, Claims, DEFAULT_JWT_EXPIRY_SECS};
use crate::state::AppState;

pub const MAX_ORG_NAME_LEN: usize = 100;
pub const MIN_ORG_NAME_LEN: usize = 1;

// ─────────────────────────────────────────────────────────────────────────────
// Role Hierarchy & RBAC
// ─────────────────────────────────────────────────────────────────────────────

/// Organizational Role enumeration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum OrgRole {
    Admin,
    Member,
    Viewer,
}

impl OrgRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            OrgRole::Admin => "admin",
            OrgRole::Member => "member",
            OrgRole::Viewer => "viewer",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "admin" => Some(OrgRole::Admin),
            "member" => Some(OrgRole::Member),
            "viewer" => Some(OrgRole::Viewer),
            _ => None,
        }
    }

    pub fn is_admin(&self) -> bool {
        matches!(self, OrgRole::Admin)
    }

    pub fn can_manage_members(&self) -> bool {
        self.is_admin()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Stored Models
// ─────────────────────────────────────────────────────────────────────────────

/// Persistent internal organization record.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, sqlx::FromRow)]
pub struct StoredOrg {
    pub id: Uuid,
    pub name: String,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
}

/// Persistent internal organization membership record.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, sqlx::FromRow)]
pub struct StoredMember {
    pub org_id: Uuid,
    pub user_id: String,
    pub role: String,
    pub created_at: DateTime<Utc>,
}

// ─────────────────────────────────────────────────────────────────────────────
// API Request & Response DTOs
// ─────────────────────────────────────────────────────────────────────────────

/// Organization summary representation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct Organization {
    /// Organization UUID identifier
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: Uuid,
    /// Organization display name
    #[schema(example = "Acme Capital Management")]
    pub name: String,
    /// Identifier of the creating user
    #[schema(example = "quant_fund_admin")]
    pub created_by: String,
    /// Creation timestamp (UTC)
    pub created_at: DateTime<Utc>,
    /// Number of active members in the organization
    #[schema(example = 5)]
    pub member_count: usize,
    /// Requesting user's role in this organization
    #[schema(example = "admin")]
    pub my_role: String,
}

/// Organization member representation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct OrganizationMember {
    /// Member user identifier
    #[schema(example = "trader_alice")]
    pub user_id: String,
    /// Assigned organizational role (admin, member, viewer)
    #[schema(example = "member")]
    pub role: String,
    /// Membership join timestamp (UTC)
    pub joined_at: DateTime<Utc>,
}

/// Request payload for creating a new organization (`POST /orgs`).
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CreateOrgRequest {
    /// Organization display name (1-100 characters)
    #[schema(example = "Acme Capital Management")]
    pub name: String,
}

/// Response payload upon creating an organization.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct CreateOrgResponse {
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: Uuid,
    #[schema(example = "Acme Capital Management")]
    pub name: String,
    #[schema(example = "admin")]
    pub role: String,
    pub created_at: DateTime<Utc>,
}

/// Response payload for listing organizations (`GET /orgs`).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct ListOrgsResponse {
    pub organizations: Vec<Organization>,
    pub count: usize,
}

/// Response payload for organization details with member roster (`GET /orgs/{id}`).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct OrgDetailsResponse {
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: Uuid,
    #[schema(example = "Acme Capital Management")]
    pub name: String,
    #[schema(example = "quant_fund_admin")]
    pub created_by: String,
    pub created_at: DateTime<Utc>,
    pub members: Vec<OrganizationMember>,
    pub count: usize,
}

/// Request payload for inviting/adding a member to an organization (`POST /orgs/{id}/invites`).
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct InviteMemberRequest {
    /// Target user ID to add or invite to the organization
    #[schema(example = "trader_bob")]
    pub user_id: String,
    /// Role to assign (admin, member, viewer; defaults to 'member')
    #[serde(default)]
    #[schema(example = "member")]
    pub role: Option<String>,
}

/// Response payload upon inviting/adding a member.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct InviteMemberResponse {
    #[schema(example = "success")]
    pub status: String,
    #[schema(example = "User successfully added to organization")]
    pub message: String,
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub org_id: Uuid,
    #[schema(example = "trader_bob")]
    pub user_id: String,
    #[schema(example = "member")]
    pub role: String,
}

/// Request payload for modifying a member's organizational role (`PATCH /orgs/{id}/members/{user_id}`).
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct UpdateMemberRoleRequest {
    /// New role to assign (admin, member, viewer)
    #[schema(example = "admin")]
    pub role: String,
}

/// Response payload upon updating a member's role.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct UpdateMemberRoleResponse {
    #[schema(example = "success")]
    pub status: String,
    #[schema(example = "Member role updated successfully")]
    pub message: String,
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub org_id: Uuid,
    #[schema(example = "trader_bob")]
    pub user_id: String,
    #[schema(example = "admin")]
    pub role: String,
}

/// Response payload upon removing a member (`DELETE /orgs/{id}/members/{user_id}`).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct RemoveMemberResponse {
    #[schema(example = "success")]
    pub status: String,
    #[schema(example = "Member removed from organization")]
    pub message: String,
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub org_id: Uuid,
    #[schema(example = "trader_bob")]
    pub user_id: String,
}

/// Response payload upon leaving an organization (`POST /orgs/{id}/leave`).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct LeaveOrgResponse {
    #[schema(example = "success")]
    pub status: String,
    #[schema(example = "Successfully left organization")]
    pub message: String,
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub org_id: Uuid,
}

/// Response payload upon switching active organization context (`POST /orgs/{id}/select`).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct SelectOrgResponse {
    #[schema(example = "success")]
    pub status: String,
    #[schema(example = "Switched active organization context")]
    pub message: String,
    /// Newly issued JWT token with embedded `org_id` claim
    #[schema(example = "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9...")]
    pub token: String,
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub org_id: Uuid,
    #[schema(example = "admin")]
    pub role: String,
}

// ─────────────────────────────────────────────────────────────────────────────
// In-Memory & PostgreSQL Dual-Store Registry
// ─────────────────────────────────────────────────────────────────────────────

/// Thread-safe in-memory Organization & Membership Registry.
#[derive(Debug, Clone, Default)]
pub struct OrgRegistry {
    orgs: Arc<DashMap<Uuid, StoredOrg>>,
    members: Arc<DashMap<(Uuid, String), StoredMember>>,
}

impl OrgRegistry {
    pub fn new() -> Self {
        Self {
            orgs: Arc::new(DashMap::new()),
            members: Arc::new(DashMap::new()),
        }
    }

    /// Creates a new organization with the creator assigned as `admin`.
    pub async fn create_org(
        &self,
        name: &str,
        created_by: &str,
        pool: Option<&PgPool>,
    ) -> Result<StoredOrg, String> {
        let org_id = Uuid::new_v4();
        let now = Utc::now();

        let org = StoredOrg {
            id: org_id,
            name: name.to_string(),
            created_by: created_by.to_string(),
            created_at: now,
        };

        let member = StoredMember {
            org_id,
            user_id: created_by.to_string(),
            role: "admin".to_string(),
            created_at: now,
        };

        // In-memory insert
        self.orgs.insert(org_id, org.clone());
        self.members
            .insert((org_id, created_by.to_string()), member);

        // PostgreSQL sync if connected
        if let Some(pool) = pool {
            let created_by_uuid = Uuid::parse_str(created_by).unwrap_or_else(|_| Uuid::new_v4());
            let _ = sqlx::query(
                r#"
                INSERT INTO organizations (id, name, created_by, created_at)
                VALUES ($1, $2, $3, $4)
                ON CONFLICT (id) DO NOTHING
                "#,
            )
            .bind(org_id)
            .bind(name)
            .bind(created_by_uuid)
            .bind(now)
            .execute(pool)
            .await;

            let _ = sqlx::query(
                r#"
                INSERT INTO organization_members (org_id, user_id, role, created_at)
                VALUES ($1, $2, $3, $4)
                ON CONFLICT (org_id, user_id) DO UPDATE SET role = EXCLUDED.role
                "#,
            )
            .bind(org_id)
            .bind(created_by_uuid)
            .bind("admin")
            .bind(now)
            .execute(pool)
            .await;
        }

        info!(
            "[Orgs] Created organization '{}' (id: {}) by user '{}'",
            name, org_id, created_by
        );
        Ok(org)
    }

    /// Retrieves an organization by UUID.
    pub async fn get_org(&self, id: Uuid, pool: Option<&PgPool>) -> Option<StoredOrg> {
        if let Some(entry) = self.orgs.get(&id) {
            return Some(entry.value().clone());
        }

        if let Some(pool) = pool {
            let row: Option<(Uuid, String, Uuid, DateTime<Utc>)> = sqlx::query_as(
                "SELECT id, name, created_by, created_at FROM organizations WHERE id = $1",
            )
            .bind(id)
            .fetch_optional(pool)
            .await
            .ok()?;

            if let Some((id, name, created_by_uuid, created_at)) = row {
                let org = StoredOrg {
                    id,
                    name,
                    created_by: created_by_uuid.to_string(),
                    created_at,
                };
                self.orgs.insert(id, org.clone());
                return Some(org);
            }
        }

        None
    }

    /// Lists all organizations a user belongs to.
    pub async fn list_user_orgs(&self, user_id: &str, _pool: Option<&PgPool>) -> Vec<Organization> {
        let mut result = Vec::new();

        for entry in self.members.iter() {
            let (org_id, member_uid) = entry.key();
            if member_uid == user_id {
                if let Some(org_entry) = self.orgs.get(org_id) {
                    let org = org_entry.value();
                    let member_count = self.members.iter().filter(|m| &m.key().0 == org_id).count();

                    result.push(Organization {
                        id: org.id,
                        name: org.name.clone(),
                        created_by: org.created_by.clone(),
                        created_at: org.created_at,
                        member_count,
                        my_role: entry.value().role.clone(),
                    });
                }
            }
        }

        result.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        result
    }

    /// Retrieves all members of an organization.
    pub async fn get_org_members(&self, org_id: Uuid, _pool: Option<&PgPool>) -> Vec<StoredMember> {
        let mut members = Vec::new();
        for entry in self.members.iter() {
            if entry.key().0 == org_id {
                members.push(entry.value().clone());
            }
        }
        members.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        members
    }

    /// Gets a specific member record.
    pub async fn get_member(
        &self,
        org_id: Uuid,
        user_id: &str,
        _pool: Option<&PgPool>,
    ) -> Option<StoredMember> {
        self.members
            .get(&(org_id, user_id.to_string()))
            .map(|e| e.value().clone())
    }

    /// Adds or updates an organization member.
    pub async fn add_or_update_member(
        &self,
        org_id: Uuid,
        user_id: &str,
        role: &str,
        pool: Option<&PgPool>,
    ) -> Result<StoredMember, String> {
        let now = Utc::now();
        let member = StoredMember {
            org_id,
            user_id: user_id.to_string(),
            role: role.to_string(),
            created_at: now,
        };

        self.members
            .insert((org_id, user_id.to_string()), member.clone());

        if let Some(pool) = pool {
            let user_uuid = Uuid::parse_str(user_id).unwrap_or_else(|_| Uuid::new_v4());
            let _ = sqlx::query(
                r#"
                INSERT INTO organization_members (org_id, user_id, role, created_at)
                VALUES ($1, $2, $3, $4)
                ON CONFLICT (org_id, user_id) DO UPDATE SET role = EXCLUDED.role
                "#,
            )
            .bind(org_id)
            .bind(user_uuid)
            .bind(role)
            .bind(now)
            .execute(pool)
            .await;
        }

        info!(
            "[Orgs] Added/Updated member user='{}' in org={} with role='{}'",
            user_id, org_id, role
        );
        Ok(member)
    }

    /// Removes a member from an organization.
    pub async fn remove_member(
        &self,
        org_id: Uuid,
        user_id: &str,
        pool: Option<&PgPool>,
    ) -> Result<bool, String> {
        let removed = self
            .members
            .remove(&(org_id, user_id.to_string()))
            .is_some();

        if let Some(pool) = pool {
            let user_uuid = Uuid::parse_str(user_id).unwrap_or_else(|_| Uuid::new_v4());
            let _ =
                sqlx::query("DELETE FROM organization_members WHERE org_id = $1 AND user_id = $2")
                    .bind(org_id)
                    .bind(user_uuid)
                    .execute(pool)
                    .await;
        }

        info!(
            "[Orgs] Removed member user='{}' from org={} (existed: {})",
            user_id, org_id, removed
        );
        Ok(removed)
    }

    /// Counts active admins in an organization.
    pub fn count_admins(&self, org_id: Uuid) -> usize {
        self.members
            .iter()
            .filter(|m| m.key().0 == org_id && m.value().role == "admin")
            .count()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Database Initializer
// ─────────────────────────────────────────────────────────────────────────────

/// Creates the `organizations` and `organization_members` tables in PostgreSQL.
pub async fn init_orgs_db(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS organizations (
            id UUID PRIMARY KEY,
            name TEXT NOT NULL,
            created_by UUID NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        );
        CREATE TABLE IF NOT EXISTS organization_members (
            org_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
            user_id UUID NOT NULL,
            role TEXT NOT NULL DEFAULT 'member',
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            PRIMARY KEY (org_id, user_id)
        );
        CREATE INDEX IF NOT EXISTS idx_org_members_user ON organization_members(user_id);
        CREATE INDEX IF NOT EXISTS idx_org_members_org ON organization_members(org_id);
        "#,
    )
    .execute(pool)
    .await?;

    info!("[Orgs] Verified 'organizations' and 'organization_members' tables in PostgreSQL");
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Axum Route Handlers
// ─────────────────────────────────────────────────────────────────────────────

/// Create a new organization (`POST /orgs`).
///
/// Registers a new organization with the requesting user as the initial `admin`.
#[utoipa::path(
    post,
    path = "/orgs",
    tag = "Organizations & Teams",
    request_body = CreateOrgRequest,
    responses(
        (status = 201, description = "Organization created successfully", body = CreateOrgResponse),
        (status = 400, description = "Bad Request (invalid name)", body = AuthErrorResponse),
        (status = 401, description = "Unauthorized", body = AuthErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::rate_limit::RateLimitErrorResponse)
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn create_org_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<CreateOrgRequest>,
) -> Response {
    let name = body.name.trim();
    if name.len() < MIN_ORG_NAME_LEN || name.len() > MAX_ORG_NAME_LEN {
        return (
            StatusCode::BAD_REQUEST,
            Json(AuthErrorResponse {
                error: "Bad Request".to_string(),
                message: format!(
                    "Organization name must be between {} and {} characters",
                    MIN_ORG_NAME_LEN, MAX_ORG_NAME_LEN
                ),
            }),
        )
            .into_response();
    }

    match state
        .org_registry
        .create_org(name, &claims.sub, state.db_pool.as_ref())
        .await
    {
        Ok(org) => {
            log_audit_event(
                &state,
                Some(org.id),
                &claims.sub,
                "org.created",
                "organization",
                Some(&org.id.to_string()),
                serde_json::json!({"name": org.name}),
                None,
            )
            .await;

            (
                StatusCode::CREATED,
                Json(CreateOrgResponse {
                    id: org.id,
                    name: org.name,
                    role: "admin".to_string(),
                    created_at: org.created_at,
                }),
            )
                .into_response()
        }
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(AuthErrorResponse {
                error: "Internal Server Error".to_string(),
                message: err,
            }),
        )
            .into_response(),
    }
}

/// List organizations the user belongs to (`GET /orgs`).
#[utoipa::path(
    get,
    path = "/orgs",
    tag = "Organizations & Teams",
    responses(
        (status = 200, description = "List of user organizations", body = ListOrgsResponse),
        (status = 401, description = "Unauthorized", body = AuthErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::rate_limit::RateLimitErrorResponse)
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn list_orgs_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Response {
    let orgs = state
        .org_registry
        .list_user_orgs(&claims.sub, state.db_pool.as_ref())
        .await;

    let count = orgs.len();
    (
        StatusCode::OK,
        Json(ListOrgsResponse {
            organizations: orgs,
            count,
        }),
    )
        .into_response()
}

/// Get organization details with roster (`GET /orgs/{id}`).
///
/// Requires that the authenticated user is an active member of the organization.
#[utoipa::path(
    get,
    path = "/orgs/{id}",
    tag = "Organizations & Teams",
    params(
        ("id" = Uuid, Path, description = "Organization unique UUID identifier")
    ),
    responses(
        (status = 200, description = "Organization details retrieved", body = OrgDetailsResponse),
        (status = 401, description = "Unauthorized", body = AuthErrorResponse),
        (status = 403, description = "Forbidden (not a member)", body = AuthErrorResponse),
        (status = 404, description = "Organization not found", body = AuthErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::rate_limit::RateLimitErrorResponse)
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn get_org_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
) -> Response {
    let pool = state.db_pool.as_ref();

    let org = match state.org_registry.get_org(id, pool).await {
        Some(o) => o,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(AuthErrorResponse {
                    error: "Not Found".to_string(),
                    message: format!("Organization with id '{}' not found", id),
                }),
            )
                .into_response();
        }
    };

    // Verify caller is a member
    if state
        .org_registry
        .get_member(id, &claims.sub, pool)
        .await
        .is_none()
    {
        return (
            StatusCode::FORBIDDEN,
            Json(AuthErrorResponse {
                error: "Forbidden".to_string(),
                message: "You are not a member of this organization".to_string(),
            }),
        )
            .into_response();
    }

    let raw_members = state.org_registry.get_org_members(id, pool).await;
    let members: Vec<OrganizationMember> = raw_members
        .into_iter()
        .map(|m| OrganizationMember {
            user_id: m.user_id,
            role: m.role,
            joined_at: m.created_at,
        })
        .collect();

    let count = members.len();
    (
        StatusCode::OK,
        Json(OrgDetailsResponse {
            id: org.id,
            name: org.name,
            created_by: org.created_by,
            created_at: org.created_at,
            members,
            count,
        }),
    )
        .into_response()
}

/// Invite or add a member to an organization (`POST /orgs/{id}/invites`).
///
/// **Admin only**: Adds or assigns a user to the organization with a specified role.
#[utoipa::path(
    post,
    path = "/orgs/{id}/invites",
    tag = "Organizations & Teams",
    params(
        ("id" = Uuid, Path, description = "Organization unique UUID identifier")
    ),
    request_body = InviteMemberRequest,
    responses(
        (status = 200, description = "Member added/invited successfully", body = InviteMemberResponse),
        (status = 400, description = "Bad Request (invalid role or user_id)", body = AuthErrorResponse),
        (status = 401, description = "Unauthorized", body = AuthErrorResponse),
        (status = 403, description = "Forbidden (admin permissions required)", body = AuthErrorResponse),
        (status = 404, description = "Organization not found", body = AuthErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::rate_limit::RateLimitErrorResponse)
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn invite_member_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
    Json(body): Json<InviteMemberRequest>,
) -> Response {
    let pool = state.db_pool.as_ref();

    if state.org_registry.get_org(id, pool).await.is_none() {
        return (
            StatusCode::NOT_FOUND,
            Json(AuthErrorResponse {
                error: "Not Found".to_string(),
                message: format!("Organization with id '{}' not found", id),
            }),
        )
            .into_response();
    }

    // Verify caller is an admin
    let caller_member = state.org_registry.get_member(id, &claims.sub, pool).await;
    match caller_member {
        Some(m) if m.role == "admin" => {}
        _ => {
            return (
                StatusCode::FORBIDDEN,
                Json(AuthErrorResponse {
                    error: "Forbidden".to_string(),
                    message: "Only organization admins can invite or add members".to_string(),
                }),
            )
                .into_response();
        }
    }

    let target_user = body.user_id.trim();
    if target_user.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(AuthErrorResponse {
                error: "Bad Request".to_string(),
                message: "Field 'user_id' cannot be empty".to_string(),
            }),
        )
            .into_response();
    }

    let role_str = body.role.as_deref().unwrap_or("member");
    let role = match OrgRole::from_str(role_str) {
        Some(r) => r.as_str(),
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(AuthErrorResponse {
                    error: "Bad Request".to_string(),
                    message: format!(
                        "Invalid role '{}'. Valid roles: admin, member, viewer",
                        role_str
                    ),
                }),
            )
                .into_response();
        }
    };

    match state
        .org_registry
        .add_or_update_member(id, target_user, role, pool)
        .await
    {
        Ok(_) => {
            log_audit_event(
                &state,
                Some(id),
                &claims.sub,
                "org.member_invited",
                "organization_member",
                Some(target_user),
                serde_json::json!({"role": role, "invited_user": target_user}),
                None,
            )
            .await;

            (
                StatusCode::OK,
                Json(InviteMemberResponse {
                    status: "success".to_string(),
                    message: format!("User '{}' successfully added to organization", target_user),
                    org_id: id,
                    user_id: target_user.to_string(),
                    role: role.to_string(),
                }),
            )
                .into_response()
        }
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(AuthErrorResponse {
                error: "Internal Server Error".to_string(),
                message: err,
            }),
        )
            .into_response(),
    }
}

/// Modify a member's organizational role (`PATCH /orgs/{id}/members/{user_id}`).
///
/// **Admin only**: Updates the assigned role of an existing member.
#[utoipa::path(
    patch,
    path = "/orgs/{id}/members/{user_id}",
    tag = "Organizations & Teams",
    params(
        ("id" = Uuid, Path, description = "Organization unique UUID identifier"),
        ("user_id" = String, Path, description = "Target member user identifier")
    ),
    request_body = UpdateMemberRoleRequest,
    responses(
        (status = 200, description = "Member role updated successfully", body = UpdateMemberRoleResponse),
        (status = 400, description = "Bad Request (invalid role or demoting sole admin)", body = AuthErrorResponse),
        (status = 401, description = "Unauthorized", body = AuthErrorResponse),
        (status = 403, description = "Forbidden (admin permissions required)", body = AuthErrorResponse),
        (status = 404, description = "Member or organization not found", body = AuthErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::rate_limit::RateLimitErrorResponse)
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn update_member_role_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((id, user_id)): Path<(Uuid, String)>,
    Json(body): Json<UpdateMemberRoleRequest>,
) -> Response {
    let pool = state.db_pool.as_ref();

    if state.org_registry.get_org(id, pool).await.is_none() {
        return (
            StatusCode::NOT_FOUND,
            Json(AuthErrorResponse {
                error: "Not Found".to_string(),
                message: format!("Organization with id '{}' not found", id),
            }),
        )
            .into_response();
    }

    // Verify caller is an admin
    let caller_member = state.org_registry.get_member(id, &claims.sub, pool).await;
    match caller_member {
        Some(m) if m.role == "admin" => {}
        _ => {
            return (
                StatusCode::FORBIDDEN,
                Json(AuthErrorResponse {
                    error: "Forbidden".to_string(),
                    message: "Only organization admins can modify member roles".to_string(),
                }),
            )
                .into_response();
        }
    }

    let target_member = match state.org_registry.get_member(id, &user_id, pool).await {
        Some(m) => m,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(AuthErrorResponse {
                    error: "Not Found".to_string(),
                    message: format!(
                        "User '{}' is not a member of organization '{}'",
                        user_id, id
                    ),
                }),
            )
                .into_response();
        }
    };

    let new_role = match OrgRole::from_str(&body.role) {
        Some(r) => r.as_str(),
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(AuthErrorResponse {
                    error: "Bad Request".to_string(),
                    message: format!(
                        "Invalid role '{}'. Valid roles: admin, member, viewer",
                        body.role
                    ),
                }),
            )
                .into_response();
        }
    };

    // Sole admin guard: If target user is an admin and being demoted, ensure at least one other admin remains
    if target_member.role == "admin"
        && new_role != "admin"
        && state.org_registry.count_admins(id) <= 1
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(AuthErrorResponse {
                error: "Bad Request".to_string(),
                message: "Cannot demote the sole organization admin. Assign another admin first."
                    .to_string(),
            }),
        )
            .into_response();
    }

    match state
        .org_registry
        .add_or_update_member(id, &user_id, new_role, pool)
        .await
    {
        Ok(_) => {
            log_audit_event(
                &state,
                Some(id),
                &claims.sub,
                "org.member_role_updated",
                "organization_member",
                Some(&user_id),
                serde_json::json!({"old_role": target_member.role, "new_role": new_role}),
                None,
            )
            .await;

            (
                StatusCode::OK,
                Json(UpdateMemberRoleResponse {
                    status: "success".to_string(),
                    message: format!("Role for member '{}' updated to '{}'", user_id, new_role),
                    org_id: id,
                    user_id,
                    role: new_role.to_string(),
                }),
            )
                .into_response()
        }
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(AuthErrorResponse {
                error: "Internal Server Error".to_string(),
                message: err,
            }),
        )
            .into_response(),
    }
}

/// Remove a member from an organization (`DELETE /orgs/{id}/members/{user_id}`).
///
/// **Admin only**: Removes a member from the organization.
#[utoipa::path(
    delete,
    path = "/orgs/{id}/members/{user_id}",
    tag = "Organizations & Teams",
    params(
        ("id" = Uuid, Path, description = "Organization unique UUID identifier"),
        ("user_id" = String, Path, description = "Target member user identifier to remove")
    ),
    responses(
        (status = 200, description = "Member removed successfully", body = RemoveMemberResponse),
        (status = 400, description = "Bad Request (cannot remove sole admin)", body = AuthErrorResponse),
        (status = 401, description = "Unauthorized", body = AuthErrorResponse),
        (status = 403, description = "Forbidden (admin permissions required)", body = AuthErrorResponse),
        (status = 404, description = "Member or organization not found", body = AuthErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::rate_limit::RateLimitErrorResponse)
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn remove_member_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((id, user_id)): Path<(Uuid, String)>,
) -> Response {
    let pool = state.db_pool.as_ref();

    if state.org_registry.get_org(id, pool).await.is_none() {
        return (
            StatusCode::NOT_FOUND,
            Json(AuthErrorResponse {
                error: "Not Found".to_string(),
                message: format!("Organization with id '{}' not found", id),
            }),
        )
            .into_response();
    }

    // Verify caller is an admin
    let caller_member = state.org_registry.get_member(id, &claims.sub, pool).await;
    match caller_member {
        Some(m) if m.role == "admin" => {}
        _ => {
            return (
                StatusCode::FORBIDDEN,
                Json(AuthErrorResponse {
                    error: "Forbidden".to_string(),
                    message: "Only organization admins can remove members".to_string(),
                }),
            )
                .into_response();
        }
    }

    let target_member = match state.org_registry.get_member(id, &user_id, pool).await {
        Some(m) => m,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(AuthErrorResponse {
                    error: "Not Found".to_string(),
                    message: format!(
                        "User '{}' is not a member of organization '{}'",
                        user_id, id
                    ),
                }),
            )
                .into_response();
        }
    };

    // Sole admin guard
    if target_member.role == "admin" && state.org_registry.count_admins(id) <= 1 {
        return (
            StatusCode::BAD_REQUEST,
            Json(AuthErrorResponse {
                error: "Bad Request".to_string(),
                message: "Cannot remove the sole organization admin. Assign another admin first."
                    .to_string(),
            }),
        )
            .into_response();
    }

    match state.org_registry.remove_member(id, &user_id, pool).await {
        Ok(_) => {
            log_audit_event(
                &state,
                Some(id),
                &claims.sub,
                "org.member_removed",
                "organization_member",
                Some(&user_id),
                serde_json::json!({"removed_user": user_id}),
                None,
            )
            .await;

            (
                StatusCode::OK,
                Json(RemoveMemberResponse {
                    status: "success".to_string(),
                    message: format!("User '{}' removed from organization", user_id),
                    org_id: id,
                    user_id,
                }),
            )
                .into_response()
        }
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(AuthErrorResponse {
                error: "Internal Server Error".to_string(),
                message: err,
            }),
        )
            .into_response(),
    }
}

/// Leave an organization (`POST /orgs/{id}/leave`).
///
/// Removes the authenticated caller from the organization.
#[utoipa::path(
    post,
    path = "/orgs/{id}/leave",
    tag = "Organizations & Teams",
    params(
        ("id" = Uuid, Path, description = "Organization unique UUID identifier")
    ),
    responses(
        (status = 200, description = "Left organization successfully", body = LeaveOrgResponse),
        (status = 400, description = "Bad Request (sole admin cannot leave)", body = AuthErrorResponse),
        (status = 401, description = "Unauthorized", body = AuthErrorResponse),
        (status = 404, description = "Organization or membership not found", body = AuthErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::rate_limit::RateLimitErrorResponse)
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn leave_org_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
) -> Response {
    let pool = state.db_pool.as_ref();

    if state.org_registry.get_org(id, pool).await.is_none() {
        return (
            StatusCode::NOT_FOUND,
            Json(AuthErrorResponse {
                error: "Not Found".to_string(),
                message: format!("Organization with id '{}' not found", id),
            }),
        )
            .into_response();
    }

    let caller_member = match state.org_registry.get_member(id, &claims.sub, pool).await {
        Some(m) => m,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(AuthErrorResponse {
                    error: "Not Found".to_string(),
                    message: "You are not a member of this organization".to_string(),
                }),
            )
                .into_response();
        }
    };

    // Sole admin guard
    if caller_member.role == "admin" && state.org_registry.count_admins(id) <= 1 {
        return (
            StatusCode::BAD_REQUEST,
            Json(AuthErrorResponse {
                error: "Bad Request".to_string(),
                message: "Cannot leave organization: You are the sole admin. Transfer admin role or delete organization first.".to_string(),
            }),
        )
            .into_response();
    }

    match state
        .org_registry
        .remove_member(id, &claims.sub, pool)
        .await
    {
        Ok(_) => {
            log_audit_event(
                &state,
                Some(id),
                &claims.sub,
                "org.member_left",
                "organization_member",
                Some(&claims.sub),
                serde_json::json!({"left_user": claims.sub}),
                None,
            )
            .await;

            (
                StatusCode::OK,
                Json(LeaveOrgResponse {
                    status: "success".to_string(),
                    message: "Successfully left organization".to_string(),
                    org_id: id,
                }),
            )
                .into_response()
        }
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(AuthErrorResponse {
                error: "Internal Server Error".to_string(),
                message: err,
            }),
        )
            .into_response(),
    }
}

/// Select active organization context (`POST /orgs/{id}/select`).
///
/// Switches active organizational scope by issuing a new JWT containing the `org_id` claim.
#[utoipa::path(
    post,
    path = "/orgs/{id}/select",
    tag = "Organizations & Teams",
    params(
        ("id" = Uuid, Path, description = "Organization unique UUID identifier to activate")
    ),
    responses(
        (status = 200, description = "Active organization switched", body = SelectOrgResponse),
        (status = 401, description = "Unauthorized", body = AuthErrorResponse),
        (status = 403, description = "Forbidden (not a member of this organization)", body = AuthErrorResponse),
        (status = 404, description = "Organization not found", body = AuthErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::rate_limit::RateLimitErrorResponse)
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn select_org_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
) -> Response {
    let pool = state.db_pool.as_ref();

    if state.org_registry.get_org(id, pool).await.is_none() {
        return (
            StatusCode::NOT_FOUND,
            Json(AuthErrorResponse {
                error: "Not Found".to_string(),
                message: format!("Organization with id '{}' not found", id),
            }),
        )
            .into_response();
    }

    let member = match state.org_registry.get_member(id, &claims.sub, pool).await {
        Some(m) => m,
        None => {
            return (
                StatusCode::FORBIDDEN,
                Json(AuthErrorResponse {
                    error: "Forbidden".to_string(),
                    message: "You are not a member of this organization".to_string(),
                }),
            )
                .into_response();
        }
    };

    let org_id_str = id.to_string();
    match generate_jwt_with_org(
        &claims.sub,
        DEFAULT_JWT_EXPIRY_SECS,
        Some(&claims.role),
        Some(&org_id_str),
        state.jwt_secret.as_bytes(),
    ) {
        Ok(token) => {
            debug!(
                "[Orgs] Issued organization context JWT for user='{}' org='{}'",
                claims.sub, id
            );

            log_audit_event(
                &state,
                Some(id),
                &claims.sub,
                "org.selected",
                "organization",
                Some(&id.to_string()),
                serde_json::json!({"role": member.role}),
                None,
            )
            .await;

            (
                StatusCode::OK,
                Json(SelectOrgResponse {
                    status: "success".to_string(),
                    message: "Active organization context switched".to_string(),
                    token,
                    org_id: id,
                    role: member.role,
                }),
            )
                .into_response()
        }
        Err(err) => {
            error!("[Orgs] Failed to issue organization JWT: {}", err);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AuthErrorResponse {
                    error: "Internal Server Error".to_string(),
                    message: "Failed to generate organization token".to_string(),
                }),
            )
                .into_response()
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_org_registry_lifecycle_and_rbac() {
        let registry = OrgRegistry::new();

        // 1. Create org
        let org = registry
            .create_org("Alpha Quant Fund", "alice_admin", None)
            .await
            .unwrap();

        assert_eq!(org.name, "Alpha Quant Fund");
        assert_eq!(org.created_by, "alice_admin");

        // Alice must be admin
        let alice_m = registry
            .get_member(org.id, "alice_admin", None)
            .await
            .unwrap();
        assert_eq!(alice_m.role, "admin");
        assert_eq!(registry.count_admins(org.id), 1);

        // 2. Add members with different roles
        registry
            .add_or_update_member(org.id, "bob_trader", "member", None)
            .await
            .unwrap();
        registry
            .add_or_update_member(org.id, "carol_auditor", "viewer", None)
            .await
            .unwrap();

        let members = registry.get_org_members(org.id, None).await;
        assert_eq!(members.len(), 3);

        // 3. Update role
        registry
            .add_or_update_member(org.id, "bob_trader", "admin", None)
            .await
            .unwrap();
        assert_eq!(registry.count_admins(org.id), 2);

        // 4. Remove member
        assert!(registry
            .remove_member(org.id, "carol_auditor", None)
            .await
            .unwrap());
        assert_eq!(registry.get_org_members(org.id, None).await.len(), 2);

        // 5. List user orgs
        let alice_orgs = registry.list_user_orgs("alice_admin", None).await;
        assert_eq!(alice_orgs.len(), 1);
        assert_eq!(alice_orgs[0].name, "Alpha Quant Fund");
        assert_eq!(alice_orgs[0].member_count, 2);
        assert_eq!(alice_orgs[0].my_role, "admin");
    }

    #[test]
    fn test_org_role_permissions() {
        assert!(OrgRole::Admin.is_admin());
        assert!(OrgRole::Admin.can_manage_members());

        assert!(!OrgRole::Member.is_admin());
        assert!(!OrgRole::Member.can_manage_members());

        assert!(!OrgRole::Viewer.is_admin());
        assert!(!OrgRole::Viewer.can_manage_members());

        assert_eq!(OrgRole::from_str("admin"), Some(OrgRole::Admin));
        assert_eq!(OrgRole::from_str("MEMBER"), Some(OrgRole::Member));
        assert_eq!(OrgRole::from_str("Viewer"), Some(OrgRole::Viewer));
        assert_eq!(OrgRole::from_str("superadmin"), None);
    }
}
