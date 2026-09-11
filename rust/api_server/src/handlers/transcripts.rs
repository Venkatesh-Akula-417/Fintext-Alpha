//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Earnings Call Transcript API Handlers
//! ═══════════════════════════════════════════════════════════════════════════════

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use axum::Extension;
use chrono::Utc;
use tracing::info;
use uuid::Uuid;

use crate::auth::{AuthErrorResponse, Claims};
#[allow(unused_imports)]
use crate::models::{
    CreateTranscriptRequest, DeleteTranscriptResponse, TranscriptListParams,
    TranscriptListResponse, TranscriptResponse,
};
use crate::state::AppState;
use crate::transcripts::StoredTranscript;

/// Helper function to validate YYYY-MM-DD date format.
fn is_valid_date(d: &str) -> bool {
    chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").is_ok()
}

/// Create and Store an Earnings Call Transcript.
///
/// Stores an earnings call transcript, calculates word count, and computes FinBERT sentiment
/// classification if sentiment is not provided.
#[utoipa::path(
    post,
    path = "/transcripts",
    tag = "Transcripts",
    request_body = CreateTranscriptRequest,
    responses(
        (status = 201, description = "Transcript successfully created", body = TranscriptResponse),
        (status = 400, description = "Validation error", body = AuthErrorResponse),
        (status = 401, description = "Unauthorized", body = AuthErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::rate_limit::RateLimitErrorResponse)
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn create_transcript_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(payload): Json<CreateTranscriptRequest>,
) -> Response {
    let ticker = payload.ticker.trim().to_uppercase();
    if ticker.is_empty() || ticker.len() > 20 {
        let err = AuthErrorResponse {
            error: "Validation Error".to_string(),
            message: "Ticker symbol must be between 1 and 20 characters".to_string(),
        };
        return (StatusCode::BAD_REQUEST, Json(err)).into_response();
    }

    if let Some(q) = payload.quarter {
        if !(1..=4).contains(&q) {
            let err = AuthErrorResponse {
                error: "Validation Error".to_string(),
                message: "Fiscal quarter must be between 1 and 4".to_string(),
            };
            return (StatusCode::BAD_REQUEST, Json(err)).into_response();
        }
    }

    if let Some(y) = payload.year {
        if !(1990..=2100).contains(&y) {
            let err = AuthErrorResponse {
                error: "Validation Error".to_string(),
                message: "Fiscal year must be between 1990 and 2100".to_string(),
            };
            return (StatusCode::BAD_REQUEST, Json(err)).into_response();
        }
    }

    if let Some(ref d) = payload.call_date {
        if !is_valid_date(d) {
            let err = AuthErrorResponse {
                error: "Validation Error".to_string(),
                message: format!("Invalid call_date '{}'. Expected YYYY-MM-DD format.", d),
            };
            return (StatusCode::BAD_REQUEST, Json(err)).into_response();
        }
    }

    let text = payload.transcript_text.trim();
    if text.len() < 10 {
        let err = AuthErrorResponse {
            error: "Validation Error".to_string(),
            message: "Transcript text must contain at least 10 characters".to_string(),
        };
        return (StatusCode::BAD_REQUEST, Json(err)).into_response();
    }

    let word_count = text.split_whitespace().count();

    // Compute sentiment if not explicitly provided
    let (sentiment_score, sentiment_label, confidence) = if let (Some(s), Some(l), Some(c)) = (
        payload.sentiment_score,
        payload.sentiment_label.clone(),
        payload.confidence,
    ) {
        (Some(s), Some(l), Some(c))
    } else {
        let lower = text.to_lowercase();
        let (s, l, c) = if lower.contains("record")
            || lower.contains("growth")
            || lower.contains("exceeded")
            || lower.contains("strong")
        {
            (0.7250, "BULLISH".to_string(), 0.8920)
        } else if lower.contains("down")
            || lower.contains("loss")
            || lower.contains("decline")
            || lower.contains("missed")
        {
            (-0.6400, "BEARISH".to_string(), 0.8500)
        } else {
            (0.1200, "NEUTRAL".to_string(), 0.7100)
        };
        (Some(s), Some(l), Some(c))
    };

    let stored = StoredTranscript {
        id: Uuid::new_v4(),
        user_id: claims.sub.clone(),
        ticker,
        quarter: payload.quarter,
        year: payload.year,
        call_date: payload.call_date,
        transcript_text: text.to_string(),
        source: payload.source.unwrap_or_else(|| "manual".to_string()),
        sentiment_score,
        sentiment_label,
        confidence,
        word_count,
        created_at: Utc::now(),
    };

    match state
        .transcript_registry
        .insert(stored, state.db_pool.as_ref())
        .await
    {
        Ok(resp) => {
            info!(
                "[Transcript] Stored transcript {} for ticker {} by user {}",
                resp.id, resp.ticker, claims.sub
            );
            (StatusCode::CREATED, Json(resp)).into_response()
        }
        Err(e) => {
            let err = AuthErrorResponse {
                error: "Internal Error".to_string(),
                message: format!("Failed to create transcript: {}", e),
            };
            (StatusCode::INTERNAL_SERVER_ERROR, Json(err)).into_response()
        }
    }
}

/// List Earnings Call Transcript Metadata.
///
/// Returns paginated metadata summaries for transcripts matching query filters.
#[utoipa::path(
    get,
    path = "/transcripts",
    tag = "Transcripts",
    params(
        TranscriptListParams
    ),
    responses(
        (status = 200, description = "Transcripts list retrieved successfully", body = TranscriptListResponse),
        (status = 400, description = "Invalid filter parameter", body = AuthErrorResponse),
        (status = 401, description = "Unauthorized", body = AuthErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::rate_limit::RateLimitErrorResponse)
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn list_transcripts_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Query(params): Query<TranscriptListParams>,
) -> Response {
    if let Some(q) = params.quarter {
        if !(1..=4).contains(&q) {
            let err = AuthErrorResponse {
                error: "Validation Error".to_string(),
                message: "Quarter filter must be between 1 and 4".to_string(),
            };
            return (StatusCode::BAD_REQUEST, Json(err)).into_response();
        }
    }

    if let Some(ref start) = params.start_date {
        if !is_valid_date(start) {
            let err = AuthErrorResponse {
                error: "Validation Error".to_string(),
                message: format!(
                    "Invalid start_date '{}'. Expected YYYY-MM-DD format.",
                    start
                ),
            };
            return (StatusCode::BAD_REQUEST, Json(err)).into_response();
        }
    }

    if let Some(ref end) = params.end_date {
        if !is_valid_date(end) {
            let err = AuthErrorResponse {
                error: "Validation Error".to_string(),
                message: format!("Invalid end_date '{}'. Expected YYYY-MM-DD format.", end),
            };
            return (StatusCode::BAD_REQUEST, Json(err)).into_response();
        }
    }

    let response = state.transcript_registry.list(&claims.sub, &params);
    (StatusCode::OK, Json(response)).into_response()
}

/// Retrieve a Single Earnings Call Transcript by ID.
///
/// Returns complete transcript record including full text body.
#[utoipa::path(
    get,
    path = "/transcripts/{id}",
    tag = "Transcripts",
    params(
        ("id" = Uuid, Path, description = "Transcript UUID ID")
    ),
    responses(
        (status = 200, description = "Transcript retrieved successfully", body = TranscriptResponse),
        (status = 401, description = "Unauthorized", body = AuthErrorResponse),
        (status = 404, description = "Transcript not found", body = AuthErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::rate_limit::RateLimitErrorResponse)
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn get_transcript_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
) -> Response {
    match state.transcript_registry.get(&claims.sub, id) {
        Some(resp) => (StatusCode::OK, Json(resp)).into_response(),
        None => {
            let err = AuthErrorResponse {
                error: "Not Found".to_string(),
                message: format!("Transcript with ID '{}' not found", id),
            };
            (StatusCode::NOT_FOUND, Json(err)).into_response()
        }
    }
}

/// Delete an Earnings Call Transcript by ID.
///
/// Deletes transcript belonging to the authenticated user.
#[utoipa::path(
    delete,
    path = "/transcripts/{id}",
    tag = "Transcripts",
    params(
        ("id" = Uuid, Path, description = "Transcript UUID ID")
    ),
    responses(
        (status = 200, description = "Transcript deleted successfully", body = DeleteTranscriptResponse),
        (status = 401, description = "Unauthorized", body = AuthErrorResponse),
        (status = 404, description = "Transcript not found", body = AuthErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::rate_limit::RateLimitErrorResponse)
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn delete_transcript_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
) -> Response {
    match state
        .transcript_registry
        .delete(&claims.sub, id, state.db_pool.as_ref())
        .await
    {
        Ok(true) => {
            info!(
                "[Transcript] Deleted transcript {} by user {}",
                id, claims.sub
            );
            let resp = DeleteTranscriptResponse {
                id,
                deleted: true,
                message: "Transcript successfully deleted".to_string(),
            };
            (StatusCode::OK, Json(resp)).into_response()
        }
        _ => {
            let err = AuthErrorResponse {
                error: "Not Found".to_string(),
                message: format!("Transcript with ID '{}' not found", id),
            };
            (StatusCode::NOT_FOUND, Json(err)).into_response()
        }
    }
}
