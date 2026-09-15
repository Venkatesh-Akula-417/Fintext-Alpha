//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Language Detection & Model Routing Handler
//! ═══════════════════════════════════════════════════════════════════════════════

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use tracing::{debug, info};

use crate::language::detect_language;
use crate::models::language::{LanguageDetectionQuery, LanguageDetectionResponse};
use crate::state::AppState;

/// Detect Language & Select Inference Model.
///
/// Analyzes the input text (headline, financial document, or transcript excerpt),
/// classifies the language using statistical stopword analysis and Unicode script
/// recognition, and returns the assigned model routing metadata.
#[utoipa::path(
    get,
    path = "/language/detect",
    tag = "Multilingual & NLP Services",
    params(
        LanguageDetectionQuery
    ),
    responses(
        (status = 200, description = "Language detected and model routed successfully", body = LanguageDetectionResponse),
        (status = 400, description = "Invalid or empty text parameter", body = crate::auth::AuthErrorResponse),
        (status = 401, description = "Unauthorized (missing or invalid Bearer JWT)", body = crate::auth::AuthErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::rate_limit::RateLimitErrorResponse)
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn get_language_detect_handler(
    State(_state): State<AppState>,
    Query(query): Query<LanguageDetectionQuery>,
) -> Response {
    let trimmed = query.text.trim();
    if trimmed.is_empty() {
        let err_body = serde_json::json!({
            "error": "Query parameter 'text' cannot be empty",
            "status": "bad_request"
        });
        return (StatusCode::BAD_REQUEST, Json(err_body)).into_response();
    }

    debug!("Analyzing language for text of length {}", trimmed.len());
    let res = detect_language(trimmed);

    info!(
        "Detected language '{}' ({}) with confidence {:.2} (multilingual_model={})",
        res.language, res.language_code, res.confidence, res.is_multilingual_model_applied
    );

    let resp = LanguageDetectionResponse {
        language: res.language,
        language_code: res.language_code,
        confidence: res.confidence,
        analyzed_chars: res.analyzed_chars,
        is_multilingual_model_applied: res.is_multilingual_model_applied,
        model_version: res.model_version,
        message: "Language detected and model routed successfully".to_string(),
    };

    (StatusCode::OK, Json(resp)).into_response()
}
