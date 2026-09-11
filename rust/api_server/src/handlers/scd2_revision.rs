//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Slowly Changing Dimension Type 2 (SCD2) Handlers
//! ═══════════════════════════════════════════════════════════════════════════════
//!
//! Provides REST endpoints for ingesting, managing, and inspecting historical
//! revisions of financial sentiment records. Enforces point-in-time data integrity,
//! supersedes prior active records, and maintains an immutable audit lineage.
//! ═══════════════════════════════════════════════════════════════════════════════

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use serde::Deserialize;
use utoipa::IntoParams;

use crate::auth::AuthErrorResponse;
use crate::scd2::{RevisionHistoryListResponse, RevisionIngestRequest, RevisionIngestResponse};
use crate::state::AppState;
use crate::storage::QuestDbClient;

/// Request query parameters for inspecting sentiment revisions (`GET /sentiment/revisions`).
#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct GetRevisionsParams {
    /// Target stock ticker symbol (e.g., 'AAPL', 'MSFT')
    pub ticker: String,
}

/// Apply SCD Type 2 Revision to Sentiment Signal.
///
/// Supersedes the current active version of a sentiment record and creates a new
/// version with incremented `revision_number`, setting `valid_to` on the old version
/// and `valid_from` on the new version.
#[utoipa::path(
    post,
    path = "/sentiment/revision",
    tag = "Sentiment Analysis",
    request_body = RevisionIngestRequest,
    responses(
        (status = 201, description = "Revision created and prior version superseded successfully", body = RevisionIngestResponse),
        (status = 400, description = "Validation error (invalid ticker, score out of range, malformed timestamp)", body = AuthErrorResponse),
        (status = 401, description = "Unauthorized (missing or invalid Bearer JWT)", body = AuthErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::rate_limit::RateLimitErrorResponse)
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn post_sentiment_revision_handler(
    State(state): State<AppState>,
    Json(payload): Json<RevisionIngestRequest>,
) -> Response {
    // 1. Validate ticker
    let ticker = match QuestDbClient::validate_and_escape_ticker(&payload.ticker) {
        Ok(t) => t,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(AuthErrorResponse {
                    error: "Bad Request".to_string(),
                    message: format!("Invalid ticker parameter: {}", e),
                }),
            )
                .into_response();
        }
    };

    // 2. Validate sentiment score in [-1.0, 1.0]
    if payload.sentiment_score < -1.0 || payload.sentiment_score > 1.0 || payload.sentiment_score.is_nan() {
        return (
            StatusCode::BAD_REQUEST,
            Json(AuthErrorResponse {
                error: "Bad Request".to_string(),
                message: format!(
                    "sentiment_score must be between -1.0 and 1.0, got {}",
                    payload.sentiment_score
                ),
            }),
        )
            .into_response();
    }

    // 3. Validate ingested_utc and db_commit_utc if provided
    if let Some(ref ing) = payload.ingested_utc {
        if chrono::DateTime::parse_from_rfc3339(ing).is_err() {
            return (
                StatusCode::BAD_REQUEST,
                Json(AuthErrorResponse {
                    error: "Bad Request".to_string(),
                    message: format!("Invalid ingested_utc format '{}', expected RFC3339", ing),
                }),
            )
                .into_response();
        }
    }
    if let Some(ref db_ts) = payload.db_commit_utc {
        if chrono::DateTime::parse_from_rfc3339(db_ts).is_err() {
            return (
                StatusCode::BAD_REQUEST,
                Json(AuthErrorResponse {
                    error: "Bad Request".to_string(),
                    message: format!("Invalid db_commit_utc format '{}', expected RFC3339", db_ts),
                }),
            )
                .into_response();
        }
    }

    let mut clean_req = payload.clone();
    clean_req.ticker = ticker.clone();

    // Ingest into in-memory SCD2 registry
    match state.scd2_registry.insert_revision(clean_req) {
        Ok((active_rev, superseded_opt)) => {
            let msg = if let Some(ref prev) = superseded_opt {
                format!(
                    "Successfully created revision {}, superseded revision {}",
                    active_rev.revision_number.unwrap_or(1),
                    prev.revision_number.unwrap_or(0)
                )
            } else {
                format!(
                    "Successfully created initial revision {}",
                    active_rev.revision_number.unwrap_or(1)
                )
            };

            let resp = RevisionIngestResponse {
                ticker,
                message: msg,
                active_revision: active_rev,
                superseded_revision: superseded_opt,
            };

            (StatusCode::CREATED, Json(resp)).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(AuthErrorResponse {
                error: "Internal Server Error".to_string(),
                message: format!("Failed to apply revision: {}", e),
            }),
        )
            .into_response(),
    }
}

/// Retrieve All SCD Type 2 Revisions for a Ticker.
///
/// Returns complete chronological lineage of all sentiment revisions for an equity.
#[utoipa::path(
    get,
    path = "/sentiment/revisions",
    tag = "Sentiment Analysis",
    params(
        ("ticker" = String, Query, description = "Target stock ticker symbol (e.g., 'AAPL', 'NVDA')")
    ),
    responses(
        (status = 200, description = "Revision history lineage retrieved successfully", body = RevisionHistoryListResponse),
        (status = 400, description = "Invalid or empty ticker parameter", body = AuthErrorResponse),
        (status = 401, description = "Unauthorized (missing or invalid Bearer JWT)", body = AuthErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::rate_limit::RateLimitErrorResponse)
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn get_sentiment_revisions_handler(
    State(state): State<AppState>,
    Query(params): Query<GetRevisionsParams>,
) -> Response {
    let ticker = match QuestDbClient::validate_and_escape_ticker(&params.ticker) {
        Ok(t) => t,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(AuthErrorResponse {
                    error: "Bad Request".to_string(),
                    message: format!("Invalid ticker parameter: {}", e),
                }),
            )
                .into_response();
        }
    };

    let revisions = state.scd2_registry.get_revisions_for_ticker(&ticker);
    let total = revisions.len();

    let resp = RevisionHistoryListResponse {
        ticker,
        total_revisions: total,
        revisions,
    };

    (StatusCode::OK, Json(resp)).into_response()
}
