//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Custom Universe Builder Architecture
//! ═══════════════════════════════════════════════════════════════════════════════

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use axum::Extension;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;
use tracing::{error, info, warn};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::auth::{AuthErrorResponse, Claims};
use crate::state::AppState;
use crate::storage::QuestDbClient;

pub const MAX_UNIVERSE_NAME_LEN: usize = 100;
pub const MAX_UNIVERSE_TICKERS: usize = 100;
pub const DEFAULT_LIST_LIMIT: usize = 100;
pub const MAX_LIST_LIMIT: usize = 1000;

/// Stored Custom Security Universe Model.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct Universe {
    /// Unique Universe identifier UUID
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: Uuid,
    /// Owner user identifier
    #[schema(example = "quant_fund_01")]
    pub user_id: String,
    /// Universe display name
    #[schema(example = "My Tech Watchlist")]
    pub name: String,
    /// Ticker symbols in the universe
    #[schema(example = json!(["AAPL", "MSFT", "NVDA"]))]
    pub tickers: Vec<String>,
    /// Creation timestamp (UTC)
    pub created_at: DateTime<Utc>,
    /// Last update timestamp (UTC)
    pub updated_at: DateTime<Utc>,
}

/// Request payload for creating a new custom universe (`POST /universes`).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateUniverseRequest {
    /// Universe display name (1-100 characters)
    #[schema(example = "My Tech Watchlist")]
    pub name: String,
    /// Non-empty list of ticker symbols (1-100 tickers)
    #[schema(example = json!(["AAPL", "MSFT", "NVDA"]))]
    pub tickers: Vec<String>,
}

/// Request payload for updating an existing universe (`PUT /universes/{id}`).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UpdateUniverseRequest {
    /// Optional updated universe display name (1-100 characters)
    #[schema(example = "Updated Tech Watchlist")]
    pub name: Option<String>,
    /// Optional updated list of ticker symbols (1-100 tickers)
    #[schema(example = json!(["AAPL", "MSFT", "NVDA", "AMZN"]))]
    pub tickers: Option<Vec<String>>,
}

/// Query parameters for listing universes (`GET /universes`).
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct ListUniversesParams {
    /// Optional pagination limit (default: 100, max: 1000)
    #[schema(example = 100)]
    pub limit: Option<usize>,
    /// Optional pagination offset (default: 0)
    #[schema(example = 0)]
    pub offset: Option<usize>,
}

/// Response payload for listing custom universes (`GET /universes`).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct ListUniversesResponse {
    pub universes: Vec<Universe>,
    pub count: usize,
}

/// Response payload for deleting a universe (`DELETE /universes/{id}`).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct DeleteUniverseResponse {
    pub id: Uuid,
    pub status: String,
    pub message: String,
}

/// In-memory and PostgreSQL synchronized Universe registry.
#[derive(Debug, Clone, Default)]
pub struct UniverseRegistry {
    entries: Arc<DashMap<Uuid, Universe>>,
}

impl UniverseRegistry {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(DashMap::new()),
        }
    }

    pub fn insert(&self, universe: Universe) {
        self.entries.insert(universe.id, universe);
    }

    pub fn get(&self, id: &Uuid) -> Option<Universe> {
        self.entries.get(id).map(|e| e.value().clone())
    }

    pub fn get_by_user(&self, id: &Uuid, user_id: &str) -> Option<Universe> {
        self.entries.get(id).and_then(|e| {
            if e.user_id == user_id {
                Some(e.value().clone())
            } else {
                None
            }
        })
    }

    pub fn list_by_user(&self, user_id: &str, limit: usize, offset: usize) -> Vec<Universe> {
        let mut list: Vec<Universe> = self
            .entries
            .iter()
            .filter(|e| e.user_id == user_id)
            .map(|e| e.value().clone())
            .collect();
        list.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        list.into_iter().skip(offset).take(limit).collect()
    }

    pub fn update(
        &self,
        id: &Uuid,
        user_id: &str,
        name: Option<String>,
        tickers: Option<Vec<String>>,
    ) -> Option<Universe> {
        if let Some(mut entry) = self.entries.get_mut(id) {
            if entry.user_id == user_id {
                if let Some(n) = name {
                    entry.name = n;
                }
                if let Some(t) = tickers {
                    entry.tickers = t;
                }
                entry.updated_at = Utc::now();
                return Some(entry.value().clone());
            }
        }
        None
    }

    pub fn remove(&self, id: &Uuid, user_id: &str) -> Option<Universe> {
        if let Some(entry) = self.entries.get(id) {
            if entry.user_id == user_id {
                drop(entry);
                return self.entries.remove(id).map(|(_, v)| v);
            }
        }
        None
    }

    pub fn count_by_user(&self, user_id: &str) -> usize {
        self.entries.iter().filter(|e| e.user_id == user_id).count()
    }

    pub fn count(&self) -> usize {
        self.entries.len()
    }
}

/// Initialize the PostgreSQL `universes` table idempotently.
pub async fn init_universes_db(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS universes (
            id UUID PRIMARY KEY,
            user_id TEXT NOT NULL,
            name TEXT NOT NULL,
            tickers JSONB NOT NULL DEFAULT '[]',
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        );
        CREATE INDEX IF NOT EXISTS idx_universes_user_id ON universes(user_id);
        "#,
    )
    .execute(pool)
    .await?;

    info!("[Universes] PostgreSQL 'universes' table and index verified.");
    Ok(())
}

/// Helper function to validate universe name.
pub fn validate_universe_name(name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("Universe name cannot be empty".to_string());
    }
    if trimmed.len() > MAX_UNIVERSE_NAME_LEN {
        return Err(format!(
            "Universe name exceeds maximum length of {} characters",
            MAX_UNIVERSE_NAME_LEN
        ));
    }
    Ok(trimmed.to_string())
}

/// Helper function to validate and normalize ticker list.
pub fn validate_universe_tickers(raw_tickers: &[String]) -> Result<Vec<String>, String> {
    if raw_tickers.is_empty() {
        return Err("Tickers list cannot be empty".to_string());
    }
    if raw_tickers.len() > MAX_UNIVERSE_TICKERS {
        return Err(format!(
            "Universe tickers list ({}) exceeds maximum limit of {}",
            raw_tickers.len(),
            MAX_UNIVERSE_TICKERS
        ));
    }

    let mut validated: Vec<String> = Vec::with_capacity(raw_tickers.len());
    for raw in raw_tickers {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err("Ticker symbol cannot be empty or whitespace".to_string());
        }
        match QuestDbClient::validate_and_escape_ticker(trimmed) {
            Ok(clean_ticker) => {
                if !validated.contains(&clean_ticker) {
                    validated.push(clean_ticker);
                }
            }
            Err(e) => {
                return Err(format!("Invalid ticker symbol '{}': {}", raw, e));
            }
        }
    }

    if validated.is_empty() {
        return Err("No valid ticker symbols found in list".to_string());
    }

    Ok(validated)
}

// ─────────────────────────────────────────────────────────────────────────────
// Database Helper Functions (PostgreSQL with sqlx)
// ─────────────────────────────────────────────────────────────────────────────

pub async fn create_universe_in_db(pool: &PgPool, universe: &Universe) -> Result<(), sqlx::Error> {
    let tickers_json =
        serde_json::to_value(&universe.tickers).unwrap_or(serde_json::Value::Array(vec![]));

    sqlx::query(
        r#"
        INSERT INTO universes (id, user_id, name, tickers, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6)
        "#,
    )
    .bind(universe.id)
    .bind(&universe.user_id)
    .bind(&universe.name)
    .bind(tickers_json)
    .bind(universe.created_at)
    .bind(universe.updated_at)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn get_universe_from_db(
    pool: &PgPool,
    id: &Uuid,
    user_id: &str,
) -> Result<Option<Universe>, sqlx::Error> {
    let row = sqlx::query_as::<
        _,
        (
            Uuid,
            String,
            String,
            serde_json::Value,
            DateTime<Utc>,
            DateTime<Utc>,
        ),
    >(
        r#"
        SELECT id, user_id, name, tickers, created_at, updated_at
        FROM universes
        WHERE id = $1 AND user_id = $2
        "#,
    )
    .bind(id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    Ok(
        row.map(|(id, user_id, name, tickers_val, created_at, updated_at)| {
            let tickers: Vec<String> = serde_json::from_value(tickers_val).unwrap_or_default();
            Universe {
                id,
                user_id,
                name,
                tickers,
                created_at,
                updated_at,
            }
        }),
    )
}

pub async fn list_universes_from_db(
    pool: &PgPool,
    user_id: &str,
    limit: usize,
    offset: usize,
) -> Result<Vec<Universe>, sqlx::Error> {
    let rows = sqlx::query_as::<
        _,
        (
            Uuid,
            String,
            String,
            serde_json::Value,
            DateTime<Utc>,
            DateTime<Utc>,
        ),
    >(
        r#"
        SELECT id, user_id, name, tickers, created_at, updated_at
        FROM universes
        WHERE user_id = $1
        ORDER BY created_at DESC
        LIMIT $2 OFFSET $3
        "#,
    )
    .bind(user_id)
    .bind(limit as i64)
    .bind(offset as i64)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(id, user_id, name, tickers_val, created_at, updated_at)| {
            let tickers: Vec<String> = serde_json::from_value(tickers_val).unwrap_or_default();
            Universe {
                id,
                user_id,
                name,
                tickers,
                created_at,
                updated_at,
            }
        })
        .collect())
}

pub async fn update_universe_in_db(
    pool: &PgPool,
    id: &Uuid,
    user_id: &str,
    name: &str,
    tickers: &[String],
    updated_at: DateTime<Utc>,
) -> Result<bool, sqlx::Error> {
    let tickers_json = serde_json::to_value(tickers).unwrap_or(serde_json::Value::Array(vec![]));

    let result = sqlx::query(
        r#"
        UPDATE universes
        SET name = $1, tickers = $2, updated_at = $3
        WHERE id = $4 AND user_id = $5
        "#,
    )
    .bind(name)
    .bind(tickers_json)
    .bind(updated_at)
    .bind(id)
    .bind(user_id)
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

pub async fn delete_universe_from_db(
    pool: &PgPool,
    id: &Uuid,
    user_id: &str,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        r#"
        DELETE FROM universes
        WHERE id = $1 AND user_id = $2
        "#,
    )
    .bind(id)
    .bind(user_id)
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

// ─────────────────────────────────────────────────────────────────────────────
// Route Handlers
// ─────────────────────────────────────────────────────────────────────────────

/// Create a new Custom Security Universe.
///
/// Creates a named collection of stock ticker symbols for the authenticated user.
/// Returns the created Universe object with a unique UUID.
#[utoipa::path(
    post,
    path = "/universes",
    tag = "Custom Universes",
    request_body = CreateUniverseRequest,
    responses(
        (status = 201, description = "Universe created successfully", body = Universe),
        (status = 400, description = "Invalid request payload, name, or ticker format", body = AuthErrorResponse),
        (status = 401, description = "Unauthorized", body = AuthErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::rate_limit::RateLimitErrorResponse)
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn create_universe_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(payload): Json<CreateUniverseRequest>,
) -> Response {
    let name = match validate_universe_name(&payload.name) {
        Ok(n) => n,
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

    let tickers = match validate_universe_tickers(&payload.tickers) {
        Ok(t) => t,
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

    let universe = Universe {
        id: Uuid::new_v4(),
        user_id: claims.sub.clone(),
        name,
        tickers,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    // Save to PostgreSQL if connected
    if let Some(ref pool) = state.db_pool {
        if let Err(e) = create_universe_in_db(pool, &universe).await {
            error!(
                "[Universes] Failed to insert universe '{}' into PostgreSQL: {}",
                universe.id, e
            );
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AuthErrorResponse {
                    error: "Internal Server Error".to_string(),
                    message: "Failed to persist universe record".to_string(),
                }),
            )
                .into_response();
        }
    }

    // Always store in memory for fast lookup / offline mode
    state.universe_registry.insert(universe.clone());

    info!(
        "[Universes] User '{}' created universe '{}' (id: {}, {} tickers)",
        claims.sub,
        universe.name,
        universe.id,
        universe.tickers.len()
    );

    (StatusCode::CREATED, Json(universe)).into_response()
}

/// List Custom Security Universes.
///
/// Retrieves all custom universes defined by the authenticated user with pagination support.
#[utoipa::path(
    get,
    path = "/universes",
    tag = "Custom Universes",
    params(
        ("limit" = Option<usize>, Query, description = "Pagination limit (default: 100, max: 1000)"),
        ("offset" = Option<usize>, Query, description = "Pagination offset (default: 0)")
    ),
    responses(
        (status = 200, description = "List of universes retrieved successfully", body = ListUniversesResponse),
        (status = 401, description = "Unauthorized", body = AuthErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::rate_limit::RateLimitErrorResponse)
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn list_universes_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Query(params): Query<ListUniversesParams>,
) -> Response {
    let limit = params
        .limit
        .unwrap_or(DEFAULT_LIST_LIMIT)
        .min(MAX_LIST_LIMIT);
    let offset = params.offset.unwrap_or(0);

    let universes = if let Some(ref pool) = state.db_pool {
        match list_universes_from_db(pool, &claims.sub, limit, offset).await {
            Ok(list) => list,
            Err(e) => {
                warn!(
                    "[Universes] PostgreSQL query failed, falling back to memory: {}",
                    e
                );
                state
                    .universe_registry
                    .list_by_user(&claims.sub, limit, offset)
            }
        }
    } else {
        state
            .universe_registry
            .list_by_user(&claims.sub, limit, offset)
    };

    let count = universes.len();
    (
        StatusCode::OK,
        Json(ListUniversesResponse { universes, count }),
    )
        .into_response()
}

/// Get a specific Custom Security Universe by ID.
///
/// Returns the full universe details including all constituent tickers.
#[utoipa::path(
    get,
    path = "/universes/{id}",
    tag = "Custom Universes",
    params(
        ("id" = Uuid, Path, description = "Unique Universe identifier UUID")
    ),
    responses(
        (status = 200, description = "Universe retrieved successfully", body = Universe),
        (status = 401, description = "Unauthorized", body = AuthErrorResponse),
        (status = 404, description = "Universe not found or not owned by user", body = AuthErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::rate_limit::RateLimitErrorResponse)
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn get_universe_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
) -> Response {
    let universe_opt = if let Some(ref pool) = state.db_pool {
        match get_universe_from_db(pool, &id, &claims.sub).await {
            Ok(u) => u,
            Err(e) => {
                warn!(
                    "[Universes] PostgreSQL query failed for '{}', falling back to memory: {}",
                    id, e
                );
                state.universe_registry.get_by_user(&id, &claims.sub)
            }
        }
    } else {
        state.universe_registry.get_by_user(&id, &claims.sub)
    };

    match universe_opt {
        Some(universe) => (StatusCode::OK, Json(universe)).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(AuthErrorResponse {
                error: "Not Found".to_string(),
                message: format!("Universe '{}' not found or not owned by user", id),
            }),
        )
            .into_response(),
    }
}

/// Update an existing Custom Security Universe.
///
/// Modifies the name and/or constituent tickers of a universe owned by the authenticated user.
#[utoipa::path(
    put,
    path = "/universes/{id}",
    tag = "Custom Universes",
    params(
        ("id" = Uuid, Path, description = "Unique Universe identifier UUID")
    ),
    request_body = UpdateUniverseRequest,
    responses(
        (status = 200, description = "Universe updated successfully", body = Universe),
        (status = 400, description = "Invalid name or ticker format", body = AuthErrorResponse),
        (status = 401, description = "Unauthorized", body = AuthErrorResponse),
        (status = 404, description = "Universe not found or not owned by user", body = AuthErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::rate_limit::RateLimitErrorResponse)
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn update_universe_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateUniverseRequest>,
) -> Response {
    // 1. Fetch current universe to verify existence and ownership
    let current_opt = if let Some(ref pool) = state.db_pool {
        match get_universe_from_db(pool, &id, &claims.sub).await {
            Ok(u) => u,
            Err(e) => {
                warn!("[Universes] DB fetch error for update '{}': {}", id, e);
                state.universe_registry.get_by_user(&id, &claims.sub)
            }
        }
    } else {
        state.universe_registry.get_by_user(&id, &claims.sub)
    };

    let current = match current_opt {
        Some(u) => u,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(AuthErrorResponse {
                    error: "Not Found".to_string(),
                    message: format!("Universe '{}' not found or not owned by user", id),
                }),
            )
                .into_response();
        }
    };

    // 2. Validate optional updates
    let updated_name = match payload.name {
        Some(ref n) => match validate_universe_name(n) {
            Ok(valid) => valid,
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
        },
        None => current.name,
    };

    let updated_tickers = match payload.tickers {
        Some(ref t) => match validate_universe_tickers(t) {
            Ok(valid) => valid,
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
        },
        None => current.tickers,
    };

    let updated_at = Utc::now();
    let updated_universe = Universe {
        id,
        user_id: claims.sub.clone(),
        name: updated_name.clone(),
        tickers: updated_tickers.clone(),
        created_at: current.created_at,
        updated_at,
    };

    // 3. Persist to DB if available
    if let Some(ref pool) = state.db_pool {
        if let Err(e) = update_universe_in_db(
            pool,
            &id,
            &claims.sub,
            &updated_name,
            &updated_tickers,
            updated_at,
        )
        .await
        {
            error!(
                "[Universes] Failed to update universe '{}' in PostgreSQL: {}",
                id, e
            );
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AuthErrorResponse {
                    error: "Internal Server Error".to_string(),
                    message: "Failed to persist universe updates".to_string(),
                }),
            )
                .into_response();
        }
    }

    // 4. Update in-memory registry
    state.universe_registry.insert(updated_universe.clone());

    info!(
        "[Universes] User '{}' updated universe '{}' (id: {}, {} tickers)",
        claims.sub,
        updated_universe.name,
        id,
        updated_universe.tickers.len()
    );

    (StatusCode::OK, Json(updated_universe)).into_response()
}

/// Delete a Custom Security Universe.
///
/// Permanently deletes a custom universe belonging to the authenticated user.
#[utoipa::path(
    delete,
    path = "/universes/{id}",
    tag = "Custom Universes",
    params(
        ("id" = Uuid, Path, description = "Unique Universe identifier UUID")
    ),
    responses(
        (status = 200, description = "Universe deleted successfully", body = DeleteUniverseResponse),
        (status = 401, description = "Unauthorized", body = AuthErrorResponse),
        (status = 404, description = "Universe not found or not owned by user", body = AuthErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::rate_limit::RateLimitErrorResponse)
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn delete_universe_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
) -> Response {
    // 1. Delete from PostgreSQL if available
    let db_deleted = if let Some(ref pool) = state.db_pool {
        match delete_universe_from_db(pool, &id, &claims.sub).await {
            Ok(deleted) => deleted,
            Err(e) => {
                error!(
                    "[Universes] Failed to delete universe '{}' from DB: {}",
                    id, e
                );
                false
            }
        }
    } else {
        false
    };

    // 2. Delete from in-memory registry
    let mem_deleted = state.universe_registry.remove(&id, &claims.sub).is_some();

    if !db_deleted && !mem_deleted {
        return (
            StatusCode::NOT_FOUND,
            Json(AuthErrorResponse {
                error: "Not Found".to_string(),
                message: format!("Universe '{}' not found or not owned by user", id),
            }),
        )
            .into_response();
    }

    info!(
        "[Universes] User '{}' deleted universe '{}'",
        claims.sub, id
    );

    (
        StatusCode::OK,
        Json(DeleteUniverseResponse {
            id,
            status: "deleted".to_string(),
            message: format!("Universe '{}' was successfully deleted", id),
        }),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_universe_name() {
        assert!(validate_universe_name("Tech Watchlist").is_ok());
        assert!(validate_universe_name("   ").is_err());
        assert!(validate_universe_name(&"A".repeat(101)).is_err());
        assert_eq!(
            validate_universe_name("  Quant Alpha  ").unwrap(),
            "Quant Alpha"
        );
    }

    #[test]
    fn test_validate_universe_tickers() {
        let valid = vec!["aapl".to_string(), "MSFT".to_string(), "nvda".to_string()];
        let normalized = validate_universe_tickers(&valid).unwrap();
        assert_eq!(normalized, vec!["AAPL", "MSFT", "NVDA"]);

        // Empty tickers
        assert!(validate_universe_tickers(&[]).is_err());

        // Too many tickers (>100)
        let too_many: Vec<String> = (0..105).map(|i| format!("TICK{}", i)).collect();
        assert!(validate_universe_tickers(&too_many).is_err());

        // Invalid ticker symbol
        let invalid = vec!["VALID".to_string(), "INVALID$$$".to_string()];
        assert!(validate_universe_tickers(&invalid).is_err());

        // Deduplication
        let duplicates = vec!["AAPL".to_string(), "aapl".to_string(), "MSFT".to_string()];
        let deduped = validate_universe_tickers(&duplicates).unwrap();
        assert_eq!(deduped, vec!["AAPL", "MSFT"]);
    }

    #[test]
    fn test_universe_registry_crud() {
        let registry = UniverseRegistry::new();
        let uid = Uuid::new_v4();
        let u = Universe {
            id: uid,
            user_id: "trader_1".to_string(),
            name: "Energy".to_string(),
            tickers: vec!["XOM".to_string(), "CVX".to_string()],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        registry.insert(u.clone());
        assert_eq!(registry.count(), 1);
        assert_eq!(registry.count_by_user("trader_1"), 1);
        assert_eq!(registry.count_by_user("trader_2"), 0);

        // Get by user
        assert!(registry.get_by_user(&uid, "trader_1").is_some());
        assert!(registry.get_by_user(&uid, "trader_2").is_none());

        // Update
        let updated = registry
            .update(&uid, "trader_1", Some("Big Oil".to_string()), None)
            .unwrap();
        assert_eq!(updated.name, "Big Oil");

        // Delete
        assert!(registry.remove(&uid, "trader_2").is_none());
        assert!(registry.remove(&uid, "trader_1").is_some());
        assert_eq!(registry.count(), 0);
    }
}
