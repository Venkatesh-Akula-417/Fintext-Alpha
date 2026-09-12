//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Sentiment Query Handler (QuestDB SQL Engine)
//! ═══════════════════════════════════════════════════════════════════════════════

use crate::models::{SentimentQuery, SentimentResponse};
use crate::state::AppState;
use crate::storage::{QuestDbClient, QuestDbClientConfig};
use axum::extract::{Query, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Json, Response};
use chrono::Utc;
use once_cell::sync::Lazy;
use tracing::{error, info, warn};

static QUESTDB_CLIENT: Lazy<QuestDbClient> =
    Lazy::new(|| QuestDbClient::new(QuestDbClientConfig::default()));

/// Query Point-in-Time Asset Sentiment.
///
/// Retrieves the aggregated point-in-time financial sentiment score, classification label,
/// and publication timestamp for a given stock ticker from QuestDB.
#[utoipa::path(
    get,
    path = "/sentiment",
    tag = "Sentiment Analysis",
    params(
        ("ticker" = String, Query, description = "Target stock ticker symbol (e.g., 'AAPL', 'NVDA', 'MSFT')"),
        ("date" = Option<String>, Query, description = "Optional date filter formatted as YYYY-MM-DD (defaults to latest available sentiment)")
    ),
    responses(
        (status = 200, description = "Sentiment signal retrieved successfully", body = SentimentResponse),
        (status = 400, description = "Invalid or empty ticker parameter", body = crate::auth::AuthErrorResponse),
        (status = 401, description = "Unauthorized (missing or invalid Bearer JWT)", body = crate::auth::AuthErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::rate_limit::RateLimitErrorResponse)
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn get_sentiment_handler(
    State(state): State<AppState>,
    Query(params): Query<SentimentQuery>,
) -> Response {
    let ticker = match QuestDbClient::validate_and_escape_ticker(&params.ticker) {
        Ok(t) => t,
        Err(e) => {
            error!("Invalid ticker parameter '{}': {}", params.ticker, e);
            let err_body = serde_json::json!({
                "error": e,
                "status": "bad_request"
            });
            return (StatusCode::BAD_REQUEST, Json(err_body)).into_response();
        }
    };

    let date = params.date.filter(|d| !d.trim().is_empty());
    let date_str = date.as_deref().unwrap_or_else(|| "LATEST");

    // Validate as_of_utc format (RFC3339)
    if let Some(ref as_of_str) = params.as_of_utc {
        if !as_of_str.trim().is_empty() {
            if chrono::DateTime::parse_from_rfc3339(as_of_str.trim()).is_err() {
                let err_body = serde_json::json!({
                    "error": format!("Invalid as_of_utc timestamp '{}', expected RFC3339/ISO-8601 format", as_of_str),
                    "status": "bad_request"
                });
                return (StatusCode::BAD_REQUEST, Json(err_body)).into_response();
            }
        }
    }

    let requested_language = params
        .language
        .as_deref()
        .unwrap_or("english")
        .to_lowercase();
    let is_non_english = requested_language != "english" && !requested_language.is_empty();

    let model_version = if is_non_english {
        Some(crate::language::MULTILINGUAL_MODEL_VERSION.to_string())
    } else {
        Some(state.get_model_version())
    };
    let pipeline_version = Some(state.get_pipeline_version());
    let data_provenance = Some(state.get_data_provenance());

    let is_primary = state.timescaledb_primary();

    // 1. If TimescaleDB is primary, query TimescaleDB first
    if is_primary {
        if let Some(resp) = fetch_timescaledb_sentiment(
            &state,
            &ticker,
            params.as_of_utc.as_deref(),
            date_str,
            &requested_language,
            model_version.clone(),
            pipeline_version.clone(),
            data_provenance.clone(),
            true,
        )
        .await
        {
            return (StatusCode::OK, Json(resp)).into_response();
        }
        warn!(
            "[TimescaleDB Primary] No sentiment record found for ticker '{}'. Falling back to QuestDB",
            ticker
        );
    }

    // Fast-path mock mode for isolated unit/integration tests without running Docker
    if crate::state::is_questdb_mock_fallback_enabled() {
        // Check if ticker has SCD2 revisions registered in state
        if !state
            .scd2_registry
            .get_revisions_for_ticker(&ticker)
            .is_empty()
        {
            if let Some(record) = state
                .scd2_registry
                .query_as_of(&ticker, params.as_of_utc.as_deref())
            {
                let sentiment_label = if record.sentiment_score > 0.15 {
                    "BULLISH".to_string()
                } else if record.sentiment_score < -0.15 {
                    "BEARISH".to_string()
                } else {
                    "NEUTRAL".to_string()
                };
                let (confidence, probabilities) =
                    crate::models::sentiment::compute_confidence_and_probabilities(
                        record.sentiment_score,
                        &sentiment_label,
                        None,
                        None,
                        None,
                    );

                let mock = SentimentResponse {
                    ticker: ticker.clone(),
                    date: date_str.to_string(),
                    sentiment_score: record.sentiment_score,
                    sentiment_label,
                    confidence,
                    probabilities,
                    signal_available_ts_us: Utc::now().timestamp_micros() as u64,
                    data_quality_score: record.data_quality_score,
                    message: "Point-in-time sentiment signal retrieved (SCD2 revision history)"
                        .to_string(),
                    model_version: record.model_version.or(model_version),
                    pipeline_version: record.pipeline_version.or(pipeline_version),
                    data_provenance: record.data_provenance.or(data_provenance),
                    language: record.language,
                    published_utc: Some(record.published_utc),
                    ingested_utc: record.ingested_utc,
                    db_commit_utc: record.db_commit_utc,
                    valid_from: record.valid_from,
                    valid_to: record.valid_to,
                    revision_number: record.revision_number,
                    is_current: record.is_current,
                    ..Default::default()
                };
                return (StatusCode::OK, Json(mock)).into_response();
            } else {
                let not_found_body = serde_json::json!({
                    "error": format!("No sentiment events found as of '{}' for ticker '{}'", params.as_of_utc.as_deref().unwrap_or("latest"), ticker),
                    "ticker": ticker,
                    "date": date_str,
                    "status": "not_found"
                });
                return (StatusCode::NOT_FOUND, Json(not_found_body)).into_response();
            }
        }

        let (sentiment_score, sentiment_label, _raw_conf, effective_model) = if is_non_english {
            crate::language::compute_multilingual_sentiment(
                &ticker,
                &requested_language,
                "Point-in-time financial news",
            )
        } else {
            (0.25, "NEUTRAL".to_string(), 0.85, state.get_model_version())
        };

        let (confidence, probabilities) =
            crate::models::sentiment::compute_confidence_and_probabilities(
                sentiment_score,
                &sentiment_label,
                Some(0.20),
                Some(0.10),
                Some(0.70),
            );

        let data_quality_score = crate::quality::compute_data_quality_score(
            "Institutional Wire",
            &format!(
                "{} Point-in-time financial news sentiment and microstructure flow",
                ticker
            ),
            confidence,
        );

        let now = Utc::now();
        let pub_dt = now - chrono::Duration::milliseconds(100);
        let ing_dt = now - chrono::Duration::milliseconds(50);
        let com_dt = now;

        let mock = SentimentResponse {
            ticker: ticker.clone(),
            date: date_str.to_string(),
            sentiment_score,
            sentiment_label,
            confidence,
            probabilities,
            signal_available_ts_us: Utc::now().timestamp_micros() as u64,
            data_quality_score,
            message: "Mock fallback sentiment (QuestDB mock mode)".to_string(),
            model_version: Some(effective_model),
            pipeline_version,
            data_provenance,
            language: requested_language,
            published_utc: Some(pub_dt.to_rfc3339()),
            ingested_utc: Some(ing_dt.to_rfc3339()),
            db_commit_utc: Some(com_dt.to_rfc3339()),
            valid_from: Some(com_dt.to_rfc3339()),
            valid_to: None,
            revision_number: Some(1),
            is_current: Some(true),
            ..Default::default()
        };
        return (StatusCode::OK, Json(mock)).into_response();
    }

    info!(
        "Executing QuestDB sentiment lookup for ticker='{}', date='{}'",
        ticker, date_str
    );

    match QUESTDB_CLIENT
        .query_latest_sentiment(&ticker, date.as_deref())
        .await
    {
        Ok(Some(mut sentiment)) => {
            if sentiment.valid_from.is_none() {
                sentiment.valid_from = sentiment.db_commit_utc.clone();
            }
            if sentiment.revision_number.is_none() {
                sentiment.revision_number = Some(1);
            }
            if sentiment.is_current.is_none() {
                sentiment.is_current = Some(true);
            }
            if is_non_english {
                sentiment.language = requested_language;
                sentiment.model_version =
                    Some(crate::language::MULTILINGUAL_MODEL_VERSION.to_string());
            }
            if sentiment.model_version.is_none() {
                sentiment.model_version = model_version;
            }
            if sentiment.pipeline_version.is_none() {
                sentiment.pipeline_version = pipeline_version;
            }
            if sentiment.data_provenance.is_none() {
                sentiment.data_provenance = data_provenance;
            }
            (StatusCode::OK, Json(sentiment)).into_response()
        }

        Ok(None) => {
            // If primary is false, check TimescaleDB fallback before returning 404
            if !is_primary && state.timescaledb_client.is_enabled() {
                if let Some(resp) = fetch_timescaledb_sentiment(
                    &state,
                    &ticker,
                    params.as_of_utc.as_deref(),
                    date_str,
                    &requested_language,
                    model_version.clone(),
                    pipeline_version.clone(),
                    data_provenance.clone(),
                    false,
                )
                .await
                {
                    return (StatusCode::OK, Json(resp)).into_response();
                }
            }

            let not_found_body = serde_json::json!({
                "error": format!("No sentiment events found in QuestDB for ticker '{}'", ticker),
                "ticker": ticker,
                "date": date_str,
                "status": "not_found"
            });
            (StatusCode::NOT_FOUND, Json(not_found_body)).into_response()
        }
        Err(err) => {
            warn!("QuestDB error during sentiment lookup: {}", err);

            // If primary is false, check TimescaleDB fallback before returning 503
            if !is_primary && state.timescaledb_client.is_enabled() {
                if let Some(resp) = fetch_timescaledb_sentiment(
                    &state,
                    &ticker,
                    params.as_of_utc.as_deref(),
                    date_str,
                    &requested_language,
                    model_version.clone(),
                    pipeline_version.clone(),
                    data_provenance.clone(),
                    false,
                )
                .await
                {
                    return (StatusCode::OK, Json(resp)).into_response();
                }
            }

            let mut headers = HeaderMap::new();
            headers.insert(header::RETRY_AFTER, HeaderValue::from_static("5"));

            let err_body = if crate::state::is_production_mode() {
                serde_json::json!({
                    "error": "Service Unavailable",
                    "message": "Required data source unavailable in production mode.",
                    "detail": err,
                    "status": "service_unavailable"
                })
            } else {
                serde_json::json!({
                    "error": "QuestDB time-series database is temporarily unreachable.",
                    "detail": err,
                    "status": "service_unavailable"
                })
            };

            (StatusCode::SERVICE_UNAVAILABLE, headers, Json(err_body)).into_response()
        }
    }
}

/// Helper function to query TimescaleDB for sentiment record and format SentimentResponse.
async fn fetch_timescaledb_sentiment(
    state: &AppState,
    ticker: &str,
    as_of_str: Option<&str>,
    date_str: &str,
    requested_language: &str,
    model_version: Option<String>,
    pipeline_version: Option<String>,
    data_provenance: Option<Vec<String>>,
    is_primary: bool,
) -> Option<SentimentResponse> {
    if !state.timescaledb_client.is_enabled() {
        return None;
    }

    let ts_records_res = if let Some(as_of) = as_of_str {
        if let Ok(as_of_dt) = chrono::DateTime::parse_from_rfc3339(as_of.trim()) {
            state
                .timescaledb_client
                .query_sentiment_as_of(ticker, as_of_dt.with_timezone(&Utc), 1)
                .await
        } else {
            state
                .timescaledb_client
                .query_latest_sentiment(ticker, 1)
                .await
        }
    } else {
        state
            .timescaledb_client
            .query_latest_sentiment(ticker, 1)
            .await
    };

    if let Ok(records) = ts_records_res {
        if let Some(record) = records.into_iter().next() {
            let (confidence, probabilities) =
                crate::models::sentiment::compute_confidence_and_probabilities(
                    record.sentiment_score,
                    &record.sentiment_label,
                    None,
                    None,
                    None,
                );

            let message = if is_primary {
                "Point-in-time sentiment signal retrieved from TimescaleDB (primary)".to_string()
            } else {
                "Point-in-time sentiment signal retrieved from TimescaleDB".to_string()
            };

            return Some(SentimentResponse {
                ticker: record.ticker,
                date: date_str.to_string(),
                sentiment_score: record.sentiment_score,
                sentiment_label: record.sentiment_label,
                confidence,
                probabilities,
                signal_available_ts_us: Utc::now().timestamp_micros() as u64,
                data_quality_score: record.data_quality_score as f32,
                message,
                model_version,
                pipeline_version,
                data_provenance,
                language: requested_language.to_string(),
                published_utc: Some(record.published_utc.to_rfc3339()),
                ingested_utc: Some(record.ingested_utc.to_rfc3339()),
                db_commit_utc: Some(record.db_commit_utc.to_rfc3339()),
                valid_from: Some(record.valid_from.to_rfc3339()),
                valid_to: record.valid_to.map(|dt| dt.to_rfc3339()),
                revision_number: Some(record.revision_number),
                is_current: Some(record.is_current),
                ..Default::default()
            });
        }
    }
    None
}
