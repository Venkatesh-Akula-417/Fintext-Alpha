//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — User Authentication & API Key Management System
//! ═══════════════════════════════════════════════════════════════════════════════

use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use axum::Extension;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use hex::ToHex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::sync::Arc;
use tracing::{error, info, warn};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::audit_logs::log_audit_event;
use crate::auth::{generate_jwt, AuthErrorResponse, Claims, DEFAULT_JWT_EXPIRY_SECS};
use crate::state::AppState;

// ─────────────────────────────────────────────────────────────────────────────
// Stored Models & Schema
// ─────────────────────────────────────────────────────────────────────────────

/// Internal persistent user account representation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StoredUser {
    pub id: Uuid,
    pub email: String,
    pub password_hash: String,
    pub role: String,
    pub created_at: DateTime<Utc>,
    pub is_active: bool,
}

/// Internal persistent API key representation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StoredApiKey {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub key_hash: String,
    pub prefix: String,
    pub created_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub rotated_from: Option<Uuid>,
    pub rotation_status: String,
}

// ─────────────────────────────────────────────────────────────────────────────
// API Request & Response DTOs
// ─────────────────────────────────────────────────────────────────────────────

/// Request payload for user registration (`POST /auth/register`).
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct RegisterRequest {
    /// Valid institutional or personal email address
    #[schema(example = "quant.trader@hedgefund.com")]
    pub email: String,
    /// Strong password (min 8 chars, 1 uppercase letter, 1 number)
    #[schema(example = "AlphaQuant2026!")]
    pub password: String,
}

/// Response payload upon successful user registration.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct RegisterResponse {
    #[schema(example = "created")]
    pub status: String,
    #[schema(example = "User account registered successfully. Please proceed to login.")]
    pub message: String,
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub user_id: Uuid,
    #[schema(example = "quant.trader@hedgefund.com")]
    pub email: String,
}

/// Request payload for user login (`POST /auth/login`).
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct LoginRequest {
    /// Registered email address
    #[schema(example = "quant.trader@hedgefund.com")]
    pub email: String,
    /// Account password
    #[schema(example = "AlphaQuant2026!")]
    pub password: String,
}

/// Response payload upon successful login.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct LoginResponse {
    /// Signed JWT authentication bearer token
    #[schema(example = "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9...")]
    pub token: String,
    /// Token scheme name
    #[schema(example = "Bearer")]
    pub token_type: String,
    /// Token lifetime duration in seconds
    #[schema(example = 86400)]
    pub expires_in: u64,
    /// Authenticated User unique identifier UUID
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub user_id: Uuid,
    /// User email address
    #[schema(example = "quant.trader@hedgefund.com")]
    pub email: String,
    /// Assigned permission role
    #[schema(example = "institutional")]
    pub role: String,
}

/// Response payload for user profile (`GET /auth/me`).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct UserProfileResponse {
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: String,
    #[schema(example = "quant.trader@hedgefund.com")]
    pub email: String,
    #[schema(example = "institutional")]
    pub role: String,
    #[schema(example = "2026-08-29T00:00:00Z")]
    pub created_at: String,
    #[schema(example = true)]
    pub is_active: bool,
}

/// Request payload for generating a new API key (`POST /auth/api-keys`).
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CreateApiKeyRequest {
    /// Friendly label or identifier for the API key
    #[serde(default)]
    #[schema(example = "Production Trading Bot")]
    pub name: Option<String>,
}

/// Response payload upon generating a new API key.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct CreateApiKeyResponse {
    /// Unique API key identifier UUID
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: Uuid,
    /// Name assigned to this key
    #[schema(example = "Production Trading Bot")]
    pub name: String,
    /// Plaintext API key token string (WARNING: displayed ONLY once upon creation)
    #[schema(example = "ft_9b1deb4d8f1e4a7b9c2a3e5f60718293")]
    pub api_key: String,
    /// Key prefix for recognition in listings
    #[schema(example = "ft_9b1de")]
    pub prefix: String,
    /// Creation timestamp (UTC)
    pub created_at: DateTime<Utc>,
}

/// Publicly visible API key item in listing (`GET /auth/api-keys`).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct ApiKeyItem {
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: Uuid,
    #[schema(example = "Production Trading Bot")]
    pub name: String,
    #[schema(example = "ft_9b1de")]
    pub prefix: String,
    pub created_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub rotated_from: Option<Uuid>,
    #[schema(example = "none")]
    pub rotation_status: String,
}

/// Request payload for rotating an API key (`POST /auth/api-keys/{id}/rotate`).
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct RotateApiKeyRequest {
    /// Overlap period in hours during which the old key remains valid (min: 1, max: 168, default: 24)
    #[serde(default)]
    #[schema(example = 24)]
    pub overlap_hours: Option<u32>,
}

/// Response payload upon rotating an API key.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct RotateApiKeyResponse {
    /// Newly issued API key UUID
    #[schema(example = "550e8400-e29b-41d4-a716-446655440001")]
    pub id: Uuid,
    /// Name assigned to new key
    #[schema(example = "Production Trading Bot (Rotated)")]
    pub name: String,
    /// Plaintext API key token string (WARNING: displayed ONLY once upon creation)
    #[schema(example = "ft_3c8a9f0e1d2b4c5a6b7e8f9012345678")]
    pub api_key: String,
    /// Prefix of the new API key
    #[schema(example = "ft_3c8a9")]
    pub prefix: String,
    /// Creation timestamp (UTC)
    pub created_at: DateTime<Utc>,
    /// UUID of the previous key that was rotated
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub rotated_from: Uuid,
    /// UUID of the previous key that will expire
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub old_key_id: Uuid,
    /// Exact UTC timestamp when the old key will expire
    pub old_key_expires_at: DateTime<Utc>,
    /// Overlap duration in hours
    #[schema(example = 24)]
    pub overlap_hours: u32,
}

/// Response payload for listing API keys.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct ListApiKeysResponse {
    pub api_keys: Vec<ApiKeyItem>,
    pub count: usize,
}

/// Response payload upon revoking an API key (`DELETE /auth/api-keys/{id}`).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct DeleteApiKeyResponse {
    pub id: Uuid,
    pub status: String,
    pub message: String,
}

// ─────────────────────────────────────────────────────────────────────────────
// Thread-Safe In-Memory Registries (with Dual-Store PostgreSQL Sync)
// ─────────────────────────────────────────────────────────────────────────────

/// Thread-safe in-memory User Account registry.
#[derive(Debug, Clone, Default)]
pub struct UserRegistry {
    users_by_id: Arc<DashMap<Uuid, StoredUser>>,
    users_by_email: Arc<DashMap<String, Uuid>>,
}

impl UserRegistry {
    pub fn new() -> Self {
        Self {
            users_by_id: Arc::new(DashMap::new()),
            users_by_email: Arc::new(DashMap::new()),
        }
    }

    pub fn insert(&self, user: StoredUser) {
        self.users_by_email
            .insert(user.email.to_lowercase(), user.id);
        self.users_by_id.insert(user.id, user);
    }

    pub fn get_by_id(&self, id: &Uuid) -> Option<StoredUser> {
        self.users_by_id.get(id).map(|u| u.value().clone())
    }

    pub fn get_by_email(&self, email: &str) -> Option<StoredUser> {
        let norm_email = email.trim().to_lowercase();
        self.users_by_email
            .get(&norm_email)
            .and_then(|id_ref| self.get_by_id(&id_ref))
    }

    pub fn exists_email(&self, email: &str) -> bool {
        self.users_by_email
            .contains_key(&email.trim().to_lowercase())
    }

    pub fn count(&self) -> usize {
        self.users_by_id.len()
    }
}

/// Thread-safe in-memory API Key registry.
#[derive(Debug, Clone, Default)]
pub struct ApiKeyRegistry {
    keys_by_id: Arc<DashMap<Uuid, StoredApiKey>>,
    keys_by_hash: Arc<DashMap<String, Uuid>>,
}

impl ApiKeyRegistry {
    pub fn new() -> Self {
        Self {
            keys_by_id: Arc::new(DashMap::new()),
            keys_by_hash: Arc::new(DashMap::new()),
        }
    }

    pub fn insert(&self, key: StoredApiKey) {
        self.keys_by_hash.insert(key.key_hash.clone(), key.id);
        self.keys_by_id.insert(key.id, key);
    }

    pub fn get_by_id(&self, id: &Uuid) -> Option<StoredApiKey> {
        self.keys_by_id.get(id).map(|k| k.value().clone())
    }

    pub fn get_by_id_and_user(&self, id: &Uuid, user_id: &Uuid) -> Option<StoredApiKey> {
        self.keys_by_id.get(id).and_then(|k| {
            if k.user_id == *user_id {
                Some(k.value().clone())
            } else {
                None
            }
        })
    }

    pub fn get_by_hash(&self, key_hash: &str) -> Option<StoredApiKey> {
        self.keys_by_hash
            .get(key_hash)
            .and_then(|id_ref| self.get_by_id(&id_ref))
    }

    pub fn list_by_user(&self, user_id: &Uuid) -> Vec<StoredApiKey> {
        self.keys_by_id
            .iter()
            .filter(|e| e.user_id == *user_id)
            .map(|e| e.value().clone())
            .collect()
    }

    pub fn revoke(&self, id: &Uuid, user_id: &Uuid) -> Option<StoredApiKey> {
        if let Some(mut entry) = self.keys_by_id.get_mut(id) {
            if entry.user_id == *user_id && entry.revoked_at.is_none() {
                entry.revoked_at = Some(Utc::now());
                entry.rotation_status = "rotated".to_string();
                let updated = entry.clone();
                drop(entry);
                return Some(updated);
            }
        }
        None
    }

    pub fn rotate(
        &self,
        old_key_id: &Uuid,
        user_id: &Uuid,
        new_key: StoredApiKey,
        overlap_hours: u32,
    ) -> Result<(StoredApiKey, StoredApiKey), String> {
        let mut old_entry = self
            .keys_by_id
            .get_mut(old_key_id)
            .ok_or_else(|| format!("API key '{}' not found", old_key_id))?;

        if old_entry.user_id != *user_id {
            return Err(format!("API key '{}' not found", old_key_id));
        }

        if old_entry.revoked_at.is_some() {
            return Err("Cannot rotate a revoked API key".to_string());
        }

        if let Some(exp) = old_entry.expires_at {
            if exp <= Utc::now() {
                return Err("Cannot rotate an expired API key".to_string());
            }
        }

        if old_entry.rotation_status == "rotating" {
            return Err("API key is already rotating".to_string());
        }

        let old_expires_at = Utc::now() + chrono::Duration::hours(overlap_hours as i64);
        old_entry.expires_at = Some(old_expires_at);
        old_entry.rotation_status = "rotating".to_string();
        let updated_old = old_entry.clone();
        drop(old_entry);

        self.insert(new_key.clone());

        Ok((new_key, updated_old))
    }

    pub fn revoke_expired_keys(&self) -> Vec<StoredApiKey> {
        let now = Utc::now();
        let mut revoked = Vec::new();
        for mut entry in self.keys_by_id.iter_mut() {
            if entry.revoked_at.is_none() {
                if let Some(exp) = entry.expires_at {
                    if exp <= now {
                        entry.revoked_at = Some(now);
                        entry.rotation_status = "rotated".to_string();
                        revoked.push(entry.clone());
                    }
                }
            }
        }
        revoked
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Cryptographic Helpers & Validation Logic
// ─────────────────────────────────────────────────────────────────────────────

/// Initialize PostgreSQL `users` and `api_keys` tables idempotently.
pub async fn init_users_db(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS users (
            id UUID PRIMARY KEY,
            email TEXT UNIQUE NOT NULL,
            password_hash TEXT NOT NULL,
            role TEXT NOT NULL DEFAULT 'institutional',
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            is_active BOOLEAN NOT NULL DEFAULT TRUE
        );
        CREATE INDEX IF NOT EXISTS idx_users_email ON users(email);

        CREATE TABLE IF NOT EXISTS api_keys (
            id UUID PRIMARY KEY,
            user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
            name TEXT NOT NULL DEFAULT 'Default',
            key_hash TEXT NOT NULL,
            prefix TEXT NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            revoked_at TIMESTAMPTZ NULL,
            expires_at TIMESTAMPTZ NULL,
            rotated_from UUID NULL,
            rotation_status TEXT NOT NULL DEFAULT 'none'
        );
        ALTER TABLE api_keys ADD COLUMN IF NOT EXISTS expires_at TIMESTAMPTZ NULL;
        ALTER TABLE api_keys ADD COLUMN IF NOT EXISTS rotated_from UUID NULL;
        ALTER TABLE api_keys ADD COLUMN IF NOT EXISTS rotation_status TEXT NOT NULL DEFAULT 'none';
        CREATE INDEX IF NOT EXISTS idx_api_keys_key_hash ON api_keys(key_hash);
        CREATE INDEX IF NOT EXISTS idx_api_keys_user_id ON api_keys(user_id);
        CREATE INDEX IF NOT EXISTS idx_api_keys_expires_at ON api_keys(expires_at);
        "#,
    )
    .execute(pool)
    .await?;

    info!("[Users & API Keys] PostgreSQL schema and indexes verified.");
    Ok(())
}

/// Hashes a plaintext password using Argon2id with random salt.
pub fn hash_password(password: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    argon2
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|e| format!("Password hashing failed: {}", e))
}

/// Verifies a plaintext password against an Argon2id password hash.
pub fn verify_password(password: &str, password_hash: &str) -> bool {
    let parsed_hash = match PasswordHash::new(password_hash) {
        Ok(h) => h,
        Err(_) => return false,
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok()
}

/// Validates email address syntax.
pub fn validate_email(email: &str) -> Result<String, String> {
    let trimmed = email.trim();
    if trimmed.is_empty() {
        return Err("Email address cannot be empty".to_string());
    }
    if !trimmed.contains('@') {
        return Err("Email must contain an '@' symbol".to_string());
    }
    let parts: Vec<&str> = trimmed.split('@').collect();
    if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() || !parts[1].contains('.') {
        return Err("Invalid email domain format".to_string());
    }
    Ok(trimmed.to_lowercase())
}

/// Validates password security requirements (min 8 chars, 1 uppercase, 1 digit).
pub fn validate_password_strength(password: &str) -> Result<(), String> {
    if password.len() < 8 {
        return Err("Password must be at least 8 characters long".to_string());
    }
    if !password.chars().any(|c| c.is_ascii_uppercase()) {
        return Err("Password must contain at least one uppercase letter (A-Z)".to_string());
    }
    if !password.chars().any(|c| c.is_ascii_digit()) {
        return Err("Password must contain at least one numerical digit (0-9)".to_string());
    }
    Ok(())
}

/// Generates a new cryptographically secure API key string along with its prefix and SHA-256 hash.
pub fn generate_api_key_material() -> (String, String, String) {
    let u1 = Uuid::new_v4().as_u128();
    let raw_key = format!("ft_{:032x}", u1);
    let prefix = raw_key[..8].to_string(); // e.g. "ft_9b1de"

    let mut hasher = Sha256::new();
    hasher.update(raw_key.as_bytes());
    let key_hash = hasher.finalize().encode_hex::<String>();

    (raw_key, prefix, key_hash)
}

/// Computes the SHA-256 hex digest of a raw API key.
pub fn hash_api_key(raw_key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(raw_key.trim().as_bytes());
    hasher.finalize().encode_hex::<String>()
}

// ─────────────────────────────────────────────────────────────────────────────
// Route Handlers
// ─────────────────────────────────────────────────────────────────────────────

/// User Registration.
///
/// Registers a new user account with email and password. Password is securely hashed using Argon2id.
#[utoipa::path(
    post,
    path = "/auth/register",
    tag = "Authentication",
    request_body = RegisterRequest,
    responses(
        (status = 201, description = "User registered successfully", body = RegisterResponse),
        (status = 400, description = "Invalid email format or weak password", body = AuthErrorResponse),
        (status = 409, description = "Email already registered", body = AuthErrorResponse)
    )
)]
pub async fn register_user_handler(
    State(state): State<AppState>,
    Json(payload): Json<RegisterRequest>,
) -> Response {
    let email = match validate_email(&payload.email) {
        Ok(e) => e,
        Err(err) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(AuthErrorResponse {
                    error: "Bad Request".to_string(),
                    message: err,
                }),
            )
                .into_response();
        }
    };

    if let Err(err) = validate_password_strength(&payload.password) {
        return (
            StatusCode::BAD_REQUEST,
            Json(AuthErrorResponse {
                error: "Weak Password".to_string(),
                message: err,
            }),
        )
            .into_response();
    }

    // Check existing email in in-memory registry or Postgres
    if state.user_registry.exists_email(&email) {
        return (
            StatusCode::CONFLICT,
            Json(AuthErrorResponse {
                error: "Conflict".to_string(),
                message: format!("An account with email '{}' is already registered", email),
            }),
        )
            .into_response();
    }

    let password_hash = match hash_password(&payload.password) {
        Ok(h) => h,
        Err(e) => {
            error!("[Auth] Password hashing failed: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AuthErrorResponse {
                    error: "Internal Error".to_string(),
                    message: "Failed to process user credentials".to_string(),
                }),
            )
                .into_response();
        }
    };

    let user_id = Uuid::new_v4();
    let stored_user = StoredUser {
        id: user_id,
        email: email.clone(),
        password_hash: password_hash.clone(),
        role: "institutional".to_string(),
        created_at: Utc::now(),
        is_active: true,
    };

    // Persist in PostgreSQL if connected
    if let Some(pool) = &state.db_pool {
        let res = sqlx::query(
            r#"
            INSERT INTO users (id, email, password_hash, role, created_at, is_active)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(stored_user.id)
        .bind(&stored_user.email)
        .bind(&stored_user.password_hash)
        .bind(&stored_user.role)
        .bind(stored_user.created_at)
        .bind(stored_user.is_active)
        .execute(pool)
        .await;

        if let Err(e) = res {
            if !crate::state::allow_in_memory_fallback() {
                return (
                    StatusCode::SERVICE_UNAVAILABLE,
                    Json(AuthErrorResponse {
                        error: "Database Unavailable".to_string(),
                        message: format!(
                            "Database unavailable (fail-closed in production mode): {}",
                            e
                        ),
                    }),
                )
                    .into_response();
            }
            error!("[Auth] Failed to persist user to PostgreSQL: {}", e);
        }
    } else if !crate::state::allow_in_memory_fallback() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(AuthErrorResponse {
                error: "Database Unavailable".to_string(),
                message: "Database pool not configured (fail-closed in production mode)"
                    .to_string(),
            }),
        )
            .into_response();
    }

    // Insert into in-memory registry for fast lookup
    state.user_registry.insert(stored_user);

    info!(
        "[Auth] User registered successfully: id={}, email={}",
        user_id, email
    );

    log_audit_event(
        &state,
        None,
        &user_id.to_string(),
        "auth.register",
        "user",
        Some(&user_id.to_string()),
        serde_json::json!({"email": email}),
        None,
    )
    .await;

    (
        StatusCode::CREATED,
        Json(RegisterResponse {
            status: "created".to_string(),
            message: "User account registered successfully. Please proceed to login.".to_string(),
            user_id,
            email,
        }),
    )
        .into_response()
}

/// User Login & JWT Token Issuance.
///
/// Authenticates user credentials and issues a signed JWT Bearer Token for API calls.
#[utoipa::path(
    post,
    path = "/auth/login",
    tag = "Authentication",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Authentication successful, returns JWT", body = LoginResponse),
        (status = 401, description = "Invalid email or password", body = AuthErrorResponse)
    )
)]
pub async fn login_user_handler(
    State(state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> Response {
    let email = payload.email.trim().to_lowercase();
    let mut user_opt = state.user_registry.get_by_email(&email);

    // If not in cache, check PostgreSQL
    if user_opt.is_none() {
        if let Some(pool) = &state.db_pool {
            let row = sqlx::query_as::<_, (Uuid, String, String, String, DateTime<Utc>, bool)>(
                "SELECT id, email, password_hash, role, created_at, is_active FROM users WHERE email = $1",
            )
            .bind(&email)
            .fetch_optional(pool)
            .await;

            match row {
                Ok(Some((id, email, password_hash, role, created_at, is_active))) => {
                    let u = StoredUser {
                        id,
                        email,
                        password_hash,
                        role,
                        created_at,
                        is_active,
                    };
                    state.user_registry.insert(u.clone());
                    user_opt = Some(u);
                }
                Ok(None) => {}
                Err(e) => {
                    if !crate::state::allow_in_memory_fallback() {
                        return (
                            StatusCode::SERVICE_UNAVAILABLE,
                            Json(AuthErrorResponse {
                                error: "Database Unavailable".to_string(),
                                message: format!(
                                    "Database unavailable (fail-closed in production mode): {}",
                                    e
                                ),
                            }),
                        )
                            .into_response();
                    }
                    warn!(
                        "[Auth] PostgreSQL user lookup failed, falling back to in-memory: {}",
                        e
                    );
                }
            }
        } else if !crate::state::allow_in_memory_fallback() {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(AuthErrorResponse {
                    error: "Database Unavailable".to_string(),
                    message: "Database pool not configured (fail-closed in production mode)"
                        .to_string(),
                }),
            )
                .into_response();
        }
    }

    let user = match user_opt {
        Some(u) if u.is_active => u,
        _ => {
            warn!("[Auth] Failed login attempt for email '{}'", email);
            return (
                StatusCode::UNAUTHORIZED,
                Json(AuthErrorResponse {
                    error: "Unauthorized".to_string(),
                    message: "Invalid email or password".to_string(),
                }),
            )
                .into_response();
        }
    };

    if !verify_password(&payload.password, &user.password_hash) {
        warn!("[Auth] Invalid password for user '{}'", email);
        return (
            StatusCode::UNAUTHORIZED,
            Json(AuthErrorResponse {
                error: "Unauthorized".to_string(),
                message: "Invalid email or password".to_string(),
            }),
        )
            .into_response();
    }

    let expiry_secs = DEFAULT_JWT_EXPIRY_SECS * 24; // 24-hour token
    let token = match generate_jwt(
        &user.id.to_string(),
        expiry_secs,
        Some(&user.role),
        state.jwt_secret.as_bytes(),
    ) {
        Ok(t) => t,
        Err(err) => {
            error!("[Auth] Failed to generate JWT token: {}", err);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AuthErrorResponse {
                    error: "Internal Error".to_string(),
                    message: "Failed to generate session token".to_string(),
                }),
            )
                .into_response();
        }
    };

    info!(
        "[Auth] User '{}' logged in successfully (id: {})",
        user.email, user.id
    );

    log_audit_event(
        &state,
        None,
        &user.id.to_string(),
        "auth.login",
        "user",
        Some(&user.id.to_string()),
        serde_json::json!({"email": user.email, "role": user.role}),
        None,
    )
    .await;

    (
        StatusCode::OK,
        Json(LoginResponse {
            token,
            token_type: "Bearer".to_string(),
            expires_in: expiry_secs,
            user_id: user.id,
            email: user.email,
            role: user.role,
        }),
    )
        .into_response()
}

/// Get Current User Profile (`GET /auth/me`).
///
/// Returns the account details of the currently authenticated caller.
#[utoipa::path(
    get,
    path = "/auth/me",
    tag = "Authentication",
    responses(
        (status = 200, description = "Current authenticated user profile", body = UserProfileResponse),
        (status = 401, description = "Unauthorized", body = AuthErrorResponse)
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn get_me_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Response {
    let user_id = match Uuid::parse_str(&claims.sub) {
        Ok(u) => u,
        Err(_) => {
            // If claims.sub is an admin/static string
            return (
                StatusCode::OK,
                Json(UserProfileResponse {
                    id: claims.sub.clone(),
                    email: format!("{}@fintext.local", claims.sub),
                    role: claims.role.clone(),
                    created_at: Utc::now().to_rfc3339(),
                    is_active: true,
                }),
            )
                .into_response();
        }
    };

    if let Some(user) = state.user_registry.get_by_id(&user_id) {
        return (
            StatusCode::OK,
            Json(UserProfileResponse {
                id: user.id.to_string(),
                email: user.email,
                role: user.role,
                created_at: user.created_at.to_rfc3339(),
                is_active: user.is_active,
            }),
        )
            .into_response();
    }

    (
        StatusCode::OK,
        Json(UserProfileResponse {
            id: claims.sub.clone(),
            email: format!("{}@fintext.local", claims.sub),
            role: claims.role.clone(),
            created_at: Utc::now().to_rfc3339(),
            is_active: true,
        }),
    )
        .into_response()
}

/// Generate a new API Key (`POST /auth/api-keys`).
///
/// Creates a new long-lived API key token for the authenticated user.
/// WARNING: The full `api_key` string is only returned once upon creation.
#[utoipa::path(
    post,
    path = "/auth/api-keys",
    tag = "Authentication",
    request_body = CreateApiKeyRequest,
    responses(
        (status = 201, description = "API Key generated successfully", body = CreateApiKeyResponse),
        (status = 401, description = "Unauthorized", body = AuthErrorResponse)
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn create_api_key_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(payload): Json<CreateApiKeyRequest>,
) -> Response {
    let user_id = match Uuid::parse_str(&claims.sub) {
        Ok(u) => u,
        Err(_) => Uuid::new_v4(), // Fallback for synthetic/named accounts
    };

    let key_name = payload
        .name
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| "Default Key".to_string());

    let (raw_key, prefix, key_hash) = generate_api_key_material();
    let key_id = Uuid::new_v4();
    let created_at = Utc::now();

    let stored = StoredApiKey {
        id: key_id,
        user_id,
        name: key_name.clone(),
        key_hash: key_hash.clone(),
        prefix: prefix.clone(),
        created_at,
        revoked_at: None,
        expires_at: None,
        rotated_from: None,
        rotation_status: "none".to_string(),
    };

    // Store in PostgreSQL if connected
    if let Some(pool) = &state.db_pool {
        let res = sqlx::query(
            r#"
            INSERT INTO api_keys (id, user_id, name, key_hash, prefix, created_at, revoked_at, expires_at, rotated_from, rotation_status)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#,
        )
        .bind(stored.id)
        .bind(stored.user_id)
        .bind(&stored.name)
        .bind(&stored.key_hash)
        .bind(&stored.prefix)
        .bind(stored.created_at)
        .bind(stored.revoked_at)
        .bind(stored.expires_at)
        .bind(stored.rotated_from)
        .bind(&stored.rotation_status)
        .execute(pool)
        .await;

        if let Err(e) = res {
            if !crate::state::allow_in_memory_fallback() {
                return (
                    StatusCode::SERVICE_UNAVAILABLE,
                    Json(AuthErrorResponse {
                        error: "Database Unavailable".to_string(),
                        message: format!(
                            "Database unavailable (fail-closed in production mode): {}",
                            e
                        ),
                    }),
                )
                    .into_response();
            }
            error!("[Auth] Failed to persist API key to PostgreSQL: {}", e);
        }
    } else if !crate::state::allow_in_memory_fallback() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(AuthErrorResponse {
                error: "Database Unavailable".to_string(),
                message: "Database pool not configured (fail-closed in production mode)"
                    .to_string(),
            }),
        )
            .into_response();
    }

    // Insert into in-memory registry
    state.api_key_registry.insert(stored);

    info!(
        "[Auth] User '{}' generated API key '{}' (id: {})",
        claims.sub, prefix, key_id
    );

    log_audit_event(
        &state,
        None,
        &claims.sub,
        "apikey.create",
        "api_key",
        Some(&key_id.to_string()),
        serde_json::json!({"name": key_name, "prefix": prefix}),
        None,
    )
    .await;

    (
        StatusCode::CREATED,
        Json(CreateApiKeyResponse {
            id: key_id,
            name: key_name,
            api_key: raw_key,
            prefix,
            created_at,
        }),
    )
        .into_response()
}

/// List Active API Keys (`GET /auth/api-keys`).
///
/// Returns all active API keys owned by the caller (without exposing full secret keys).
#[utoipa::path(
    get,
    path = "/auth/api-keys",
    tag = "Authentication",
    responses(
        (status = 200, description = "List of active API keys", body = ListApiKeysResponse),
        (status = 401, description = "Unauthorized", body = AuthErrorResponse)
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn list_api_keys_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Response {
    let user_id = match Uuid::parse_str(&claims.sub) {
        Ok(u) => u,
        Err(_) => {
            return (
                StatusCode::OK,
                Json(ListApiKeysResponse {
                    api_keys: Vec::new(),
                    count: 0,
                }),
            )
                .into_response();
        }
    };

    let keys = state.api_key_registry.list_by_user(&user_id);
    let items: Vec<ApiKeyItem> = keys
        .into_iter()
        .map(|k| ApiKeyItem {
            id: k.id,
            name: k.name,
            prefix: k.prefix,
            created_at: k.created_at,
            revoked_at: k.revoked_at,
            expires_at: k.expires_at,
            rotated_from: k.rotated_from,
            rotation_status: k.rotation_status,
        })
        .collect();

    let count = items.len();
    (
        StatusCode::OK,
        Json(ListApiKeysResponse {
            api_keys: items,
            count,
        }),
    )
        .into_response()
}

/// Get API Key Details (`GET /auth/api-keys/{id}`).
///
/// Returns metadata for a specific API key belonging to the authenticated user.
#[utoipa::path(
    get,
    path = "/auth/api-keys/{id}",
    tag = "Authentication",
    params(
        ("id" = Uuid, Path, description = "API Key UUID")
    ),
    responses(
        (status = 200, description = "API Key details", body = ApiKeyItem),
        (status = 401, description = "Unauthorized", body = AuthErrorResponse),
        (status = 404, description = "API Key not found or owned by another user", body = AuthErrorResponse)
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn get_api_key_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
) -> Response {
    let user_id = match Uuid::parse_str(&claims.sub) {
        Ok(u) => u,
        Err(_) => {
            return (
                StatusCode::NOT_FOUND,
                Json(AuthErrorResponse {
                    error: "Not Found".to_string(),
                    message: format!("API key '{}' not found", id),
                }),
            )
                .into_response();
        }
    };

    if let Some(key) = state.api_key_registry.get_by_id_and_user(&id, &user_id) {
        return (
            StatusCode::OK,
            Json(ApiKeyItem {
                id: key.id,
                name: key.name,
                prefix: key.prefix,
                created_at: key.created_at,
                revoked_at: key.revoked_at,
                expires_at: key.expires_at,
                rotated_from: key.rotated_from,
                rotation_status: key.rotation_status,
            }),
        )
            .into_response();
    }

    (
        StatusCode::NOT_FOUND,
        Json(AuthErrorResponse {
            error: "Not Found".to_string(),
            message: format!("API key '{}' not found", id),
        }),
    )
        .into_response()
}

/// Rotate an API Key (`POST /auth/api-keys/{id}/rotate`).
///
/// Initiates seamless zero-downtime rotation for an active API key with a configurable overlap window.
/// Generates and returns a new API key while keeping the previous key active until `expires_at`.
#[utoipa::path(
    post,
    path = "/auth/api-keys/{id}/rotate",
    tag = "Authentication",
    params(
        ("id" = Uuid, Path, description = "Existing API Key UUID to rotate")
    ),
    request_body = RotateApiKeyRequest,
    responses(
        (status = 200, description = "API Key rotated successfully", body = RotateApiKeyResponse),
        (status = 400, description = "Bad request (invalid overlap hours, already rotating, or expired)", body = AuthErrorResponse),
        (status = 401, description = "Unauthorized", body = AuthErrorResponse),
        (status = 404, description = "API Key not found or owned by another user", body = AuthErrorResponse)
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn rotate_api_key_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
    Json(payload): Json<RotateApiKeyRequest>,
) -> Response {
    let user_id = match Uuid::parse_str(&claims.sub) {
        Ok(u) => u,
        Err(_) => {
            return (
                StatusCode::NOT_FOUND,
                Json(AuthErrorResponse {
                    error: "Not Found".to_string(),
                    message: format!("API key '{}' not found", id),
                }),
            )
                .into_response();
        }
    };

    let overlap_hours = payload.overlap_hours.unwrap_or(24);
    if overlap_hours < 1 || overlap_hours > 168 {
        return (
            StatusCode::BAD_REQUEST,
            Json(AuthErrorResponse {
                error: "Bad Request".to_string(),
                message: "overlap_hours must be between 1 and 168 (1 week)".to_string(),
            }),
        )
            .into_response();
    }

    // Check if key exists
    let old_key = match state.api_key_registry.get_by_id_and_user(&id, &user_id) {
        Some(k) => k,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(AuthErrorResponse {
                    error: "Not Found".to_string(),
                    message: format!("API key '{}' not found", id),
                }),
            )
                .into_response();
        }
    };

    if old_key.revoked_at.is_some() {
        return (
            StatusCode::BAD_REQUEST,
            Json(AuthErrorResponse {
                error: "Bad Request".to_string(),
                message: "Cannot rotate a revoked API key".to_string(),
            }),
        )
            .into_response();
    }

    if let Some(exp) = old_key.expires_at {
        if exp <= Utc::now() {
            return (
                StatusCode::BAD_REQUEST,
                Json(AuthErrorResponse {
                    error: "Bad Request".to_string(),
                    message: "Cannot rotate an expired API key".to_string(),
                }),
            )
                .into_response();
        }
    }

    if old_key.rotation_status == "rotating" {
        return (
            StatusCode::BAD_REQUEST,
            Json(AuthErrorResponse {
                error: "Bad Request".to_string(),
                message: "API key is already rotating".to_string(),
            }),
        )
            .into_response();
    }

    let (raw_key, prefix, key_hash) = generate_api_key_material();
    let new_key_id = Uuid::new_v4();
    let created_at = Utc::now();
    let old_expires_at = created_at + chrono::Duration::hours(overlap_hours as i64);

    let new_key_name = if old_key.name.ends_with("(Rotated)") {
        old_key.name.clone()
    } else {
        format!("{} (Rotated)", old_key.name)
    };

    let new_stored = StoredApiKey {
        id: new_key_id,
        user_id,
        name: new_key_name.clone(),
        key_hash: key_hash.clone(),
        prefix: prefix.clone(),
        created_at,
        revoked_at: None,
        expires_at: None,
        rotated_from: Some(id),
        rotation_status: "none".to_string(),
    };

    // Perform atomic in-memory rotation
    let (_new_key, updated_old) =
        match state
            .api_key_registry
            .rotate(&id, &user_id, new_stored.clone(), overlap_hours)
        {
            Ok(res) => res,
            Err(err) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(AuthErrorResponse {
                        error: "Bad Request".to_string(),
                        message: err,
                    }),
                )
                    .into_response();
            }
        };

    // Update PostgreSQL if connected
    if let Some(pool) = &state.db_pool {
        let res1 = sqlx::query(
            r#"
            INSERT INTO api_keys (id, user_id, name, key_hash, prefix, created_at, revoked_at, expires_at, rotated_from, rotation_status)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#,
        )
        .bind(new_stored.id)
        .bind(new_stored.user_id)
        .bind(&new_stored.name)
        .bind(&new_stored.key_hash)
        .bind(&new_stored.prefix)
        .bind(new_stored.created_at)
        .bind(new_stored.revoked_at)
        .bind(new_stored.expires_at)
        .bind(new_stored.rotated_from)
        .bind(&new_stored.rotation_status)
        .execute(pool)
        .await;

        if let Err(e) = res1 {
            if !crate::state::allow_in_memory_fallback() {
                return (
                    StatusCode::SERVICE_UNAVAILABLE,
                    Json(AuthErrorResponse {
                        error: "Database Unavailable".to_string(),
                        message: format!(
                            "Database unavailable (fail-closed in production mode): {}",
                            e
                        ),
                    }),
                )
                    .into_response();
            }
            error!(
                "[Auth] Failed to persist rotated API key to PostgreSQL: {}",
                e
            );
        }

        let res2 =
            sqlx::query("UPDATE api_keys SET expires_at = $1, rotation_status = $2 WHERE id = $3")
                .bind(updated_old.expires_at)
                .bind(&updated_old.rotation_status)
                .bind(id)
                .execute(pool)
                .await;

        if let Err(e) = res2 {
            if !crate::state::allow_in_memory_fallback() {
                return (
                    StatusCode::SERVICE_UNAVAILABLE,
                    Json(AuthErrorResponse {
                        error: "Database Unavailable".to_string(),
                        message: format!(
                            "Database unavailable (fail-closed in production mode): {}",
                            e
                        ),
                    }),
                )
                    .into_response();
            }
            error!("[Auth] Failed to update old API key in PostgreSQL: {}", e);
        }
    } else if !crate::state::allow_in_memory_fallback() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(AuthErrorResponse {
                error: "Database Unavailable".to_string(),
                message: "Database pool not configured (fail-closed in production mode)"
                    .to_string(),
            }),
        )
            .into_response();
    }

    info!(
        "[Auth] User '{}' rotated API key '{}' -> '{}' (expires in {}h)",
        claims.sub, id, new_key_id, overlap_hours
    );

    log_audit_event(
        &state,
        None,
        &claims.sub,
        "apikey.rotation_started",
        "api_key",
        Some(&new_key_id.to_string()),
        serde_json::json!({
            "old_key_id": id.to_string(),
            "new_key_id": new_key_id.to_string(),
            "overlap_hours": overlap_hours,
            "old_key_expires_at": old_expires_at.to_rfc3339()
        }),
        None,
    )
    .await;

    (
        StatusCode::OK,
        Json(RotateApiKeyResponse {
            id: new_key_id,
            name: new_key_name,
            api_key: raw_key,
            prefix,
            created_at,
            rotated_from: id,
            old_key_id: id,
            old_key_expires_at: old_expires_at,
            overlap_hours,
        }),
    )
        .into_response()
}

/// Revoke an API Key (`DELETE /auth/api-keys/{id}`).
///
/// Revokes and invalidates an API key token. Subsequent requests with this key will return 401.
#[utoipa::path(
    delete,
    path = "/auth/api-keys/{id}",
    tag = "Authentication",
    params(
        ("id" = Uuid, Path, description = "API Key UUID to revoke")
    ),
    responses(
        (status = 200, description = "API Key revoked successfully", body = DeleteApiKeyResponse),
        (status = 401, description = "Unauthorized", body = AuthErrorResponse),
        (status = 404, description = "API Key not found or owned by another user", body = AuthErrorResponse)
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn delete_api_key_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
) -> Response {
    let user_id = match Uuid::parse_str(&claims.sub) {
        Ok(u) => u,
        Err(_) => {
            return (
                StatusCode::NOT_FOUND,
                Json(AuthErrorResponse {
                    error: "Not Found".to_string(),
                    message: format!("API key '{}' not found", id),
                }),
            )
                .into_response();
        }
    };

    let removed = state.api_key_registry.revoke(&id, &user_id);

    // Also update PostgreSQL
    if let Some(pool) = &state.db_pool {
        let res = sqlx::query(
            "UPDATE api_keys SET revoked_at = NOW(), rotation_status = 'rotated' WHERE id = $1 AND user_id = $2 AND revoked_at IS NULL",
        )
        .bind(id)
        .bind(user_id)
        .execute(pool)
        .await;

        if let Err(e) = res {
            if !crate::state::allow_in_memory_fallback() {
                return (
                    StatusCode::SERVICE_UNAVAILABLE,
                    Json(AuthErrorResponse {
                        error: "Database Unavailable".to_string(),
                        message: format!(
                            "Database unavailable (fail-closed in production mode): {}",
                            e
                        ),
                    }),
                )
                    .into_response();
            }
            error!("[Auth] Failed to revoke API key in PostgreSQL: {}", e);
        }
    } else if !crate::state::allow_in_memory_fallback() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(AuthErrorResponse {
                error: "Database Unavailable".to_string(),
                message: "Database pool not configured (fail-closed in production mode)"
                    .to_string(),
            }),
        )
            .into_response();
    }

    if removed.is_none() {
        return (
            StatusCode::NOT_FOUND,
            Json(AuthErrorResponse {
                error: "Not Found".to_string(),
                message: format!("API key '{}' not found or already revoked", id),
            }),
        )
            .into_response();
    }

    info!("[Auth] User '{}' revoked API key '{}'", claims.sub, id);

    log_audit_event(
        &state,
        None,
        &claims.sub,
        "apikey.revoke",
        "api_key",
        Some(&id.to_string()),
        serde_json::json!({"key_id": id}),
        None,
    )
    .await;

    (
        StatusCode::OK,
        Json(DeleteApiKeyResponse {
            id,
            status: "revoked".to_string(),
            message: "API key revoked successfully".to_string(),
        }),
    )
        .into_response()
}

/// Spawns a Tokio background worker that periodically scans and revokes expired API keys.
pub fn spawn_api_key_revocation_worker(state: AppState) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(300));
        loop {
            interval.tick().await;
            let revoked = state.api_key_registry.revoke_expired_keys();
            for key in revoked {
                info!(
                    "[Auth] Auto-revoked expired API key '{}' (user_id: '{}')",
                    key.id, key.user_id
                );

                if let Some(pool) = &state.db_pool {
                    let res = sqlx::query(
                        "UPDATE api_keys SET revoked_at = $1, rotation_status = 'rotated' WHERE id = $2",
                    )
                    .bind(key.revoked_at)
                    .bind(key.id)
                    .execute(pool)
                    .await;

                    if let Err(e) = res {
                        if !crate::state::allow_in_memory_fallback() {
                            error!(
                                "[Auth] Failed to update expired API key in PostgreSQL (fail-closed in production mode): {}",
                                e
                            );
                        } else {
                            warn!(
                                "[Auth] Failed to update expired API key in PostgreSQL: {}",
                                e
                            );
                        }
                    }
                } else if !crate::state::allow_in_memory_fallback() {
                    error!(
                        "[Auth] PostgreSQL pool not configured for API key revocation worker (fail-closed in production mode)"
                    );
                }

                log_audit_event(
                    &state,
                    None,
                    &key.user_id.to_string(),
                    "apikey.rotation_completed",
                    "api_key",
                    Some(&key.id.to_string()),
                    serde_json::json!({
                        "key_id": key.id.to_string(),
                        "expired_at": key.expires_at.map(|e| e.to_rfc3339()),
                        "revoked_at": key.revoked_at.map(|r| r.to_rfc3339()),
                    }),
                    None,
                )
                .await;
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_argon2_password_hashing_and_verification() {
        let pass = "SecureAlpha2026!";
        let hash = hash_password(pass).unwrap();
        assert!(hash.starts_with("$argon2"));

        assert!(verify_password(pass, &hash));
        assert!(!verify_password("WrongPassword123!", &hash));
    }

    #[test]
    fn test_email_and_password_strength_validation() {
        // Valid
        assert!(validate_email("user@example.com").is_ok());
        assert!(validate_email("trader.fund@hedge.co.uk").is_ok());
        assert!(validate_password_strength("StrongPass1!").is_ok());

        // Invalid Emails
        assert!(validate_email("").is_err());
        assert!(validate_email("notanemail").is_err());
        assert!(validate_email("@missinguser.com").is_err());

        // Invalid Passwords
        assert!(validate_password_strength("short1A").is_err()); // < 8
        assert!(validate_password_strength("alllowercase123").is_err()); // no uppercase
        assert!(validate_password_strength("ALLUPPERCASENO_DIGIT").is_err()); // no digit
    }

    #[test]
    fn test_api_key_generation_and_hashing() {
        let (raw_key, prefix, hash) = generate_api_key_material();
        assert!(raw_key.starts_with("ft_"));
        assert_eq!(prefix, raw_key[..8]);
        assert_eq!(hash.len(), 64);

        let recomputed_hash = hash_api_key(&raw_key);
        assert_eq!(hash, recomputed_hash);
    }

    #[test]
    fn test_user_and_api_key_registry_operations() {
        let user_reg = UserRegistry::new();
        let user_id = Uuid::new_v4();
        let user = StoredUser {
            id: user_id,
            email: "test@quant.com".to_string(),
            password_hash: "hash123".to_string(),
            role: "institutional".to_string(),
            created_at: Utc::now(),
            is_active: true,
        };

        user_reg.insert(user.clone());
        assert_eq!(user_reg.count(), 1);
        assert!(user_reg.exists_email("test@quant.com"));
        assert_eq!(
            user_reg.get_by_id(&user_id).unwrap().email,
            "test@quant.com"
        );
        assert_eq!(user_reg.get_by_email("TEST@QUANT.COM").unwrap().id, user_id);

        let key_reg = ApiKeyRegistry::new();
        let (_raw_key, prefix, key_hash) = generate_api_key_material();
        let key_id = Uuid::new_v4();
        let key = StoredApiKey {
            id: key_id,
            user_id,
            name: "Bot 1".to_string(),
            key_hash: key_hash.clone(),
            prefix,
            created_at: Utc::now(),
            revoked_at: None,
            expires_at: None,
            rotated_from: None,
            rotation_status: "none".to_string(),
        };

        key_reg.insert(key);
        assert!(key_reg.get_by_hash(&key_hash).is_some());
        assert_eq!(key_reg.list_by_user(&user_id).len(), 1);

        assert!(key_reg.revoke(&key_id, &user_id).is_some());
        assert!(key_reg.list_by_user(&user_id)[0].revoked_at.is_some());
    }

    #[test]
    fn test_api_key_rotation_and_revocation() {
        let key_reg = ApiKeyRegistry::new();
        let user_id = Uuid::new_v4();
        let (_raw_key1, prefix1, key_hash1) = generate_api_key_material();
        let old_id = Uuid::new_v4();
        let old_key = StoredApiKey {
            id: old_id,
            user_id,
            name: "Old Prod Key".to_string(),
            key_hash: key_hash1,
            prefix: prefix1,
            created_at: Utc::now(),
            revoked_at: None,
            expires_at: None,
            rotated_from: None,
            rotation_status: "none".to_string(),
        };
        key_reg.insert(old_key);

        let (_raw_key2, prefix2, key_hash2) = generate_api_key_material();
        let new_id = Uuid::new_v4();
        let new_key = StoredApiKey {
            id: new_id,
            user_id,
            name: "Old Prod Key (Rotated)".to_string(),
            key_hash: key_hash2,
            prefix: prefix2,
            created_at: Utc::now(),
            revoked_at: None,
            expires_at: None,
            rotated_from: Some(old_id),
            rotation_status: "none".to_string(),
        };

        // Rotate key with 24h overlap
        let res = key_reg.rotate(&old_id, &user_id, new_key, 24);
        assert!(res.is_ok());

        let (new_res, old_res) = res.unwrap();
        assert_eq!(new_res.id, new_id);
        assert_eq!(new_res.rotated_from, Some(old_id));
        assert_eq!(old_res.rotation_status, "rotating");
        assert!(old_res.expires_at.is_some());

        // Attempting to rotate again while 'rotating' fails
        let (_raw_key3, prefix3, key_hash3) = generate_api_key_material();
        let duplicate_attempt = StoredApiKey {
            id: Uuid::new_v4(),
            user_id,
            name: "Duplicate Attempt".to_string(),
            key_hash: key_hash3,
            prefix: prefix3,
            created_at: Utc::now(),
            revoked_at: None,
            expires_at: None,
            rotated_from: Some(old_id),
            rotation_status: "none".to_string(),
        };
        let dup_res = key_reg.rotate(&old_id, &user_id, duplicate_attempt, 24);
        assert!(dup_res.is_err());

        // Test expired revocation
        // Manually set old_key expires_at to past
        if let Some(mut entry) = key_reg.keys_by_id.get_mut(&old_id) {
            entry.expires_at = Some(Utc::now() - chrono::Duration::hours(1));
        }

        let revoked = key_reg.revoke_expired_keys();
        assert_eq!(revoked.len(), 1);
        assert_eq!(revoked[0].id, old_id);
        assert_eq!(revoked[0].rotation_status, "rotated");
        assert!(revoked[0].revoked_at.is_some());
    }
}
