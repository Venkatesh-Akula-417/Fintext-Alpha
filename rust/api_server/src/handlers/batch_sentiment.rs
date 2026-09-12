//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Batch Multi-Ticker Sentiment Handler
//! ═══════════════════════════════════════════════════════════════════════════════

use crate::auth::{AuthErrorResponse, Claims};
use crate::models::{BatchSentimentParams, BatchSentimentResponse, SentimentResponse};
use crate::pit::GLOBAL_PIT_DATA;
use crate::state::AppState;
use crate::storage::{QuestDbClient, QuestDbClientConfig};
use crate::universes::get_universe_from_db;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use axum::Extension;
use chrono::{NaiveDate, Utc};
use once_cell::sync::Lazy;
use tracing::{error, info, warn};

static QUESTDB_CLIENT: Lazy<QuestDbClient> =
    Lazy::new(|| QuestDbClient::new(QuestDbClientConfig::default()));

pub const MAX_BATCH_TICKERS: usize = 50;

/// Query Point-in-Time Asset Sentiment for Multiple Tickers in Batch.
///
/// Efficiently retrieves aggregated financial sentiment signals, classification labels,
/// confidence scores, and probability distributions for up to 50 stock tickers in a single HTTP request.
/// Tickers can be provided directly via `tickers` or dynamically retrieved from a saved `universe_id`.
#[utoipa::path(
    get,
    path = "/sentiment/batch",
    tag = "Sentiment Analysis",
    params(
        ("tickers" = Option<String>, Query, description = "Optional comma-separated list of stock tickers (max 50, e.g. 'AAPL,MSFT,NVDA')"),
        ("universe_id" = Option<Uuid>, Query, description = "Optional custom universe UUID to retrieve constituent tickers from"),
        ("date" = Option<String>, Query, description = "Optional date filter formatted as YYYY-MM-DD (defaults to latest available sentiment)")
    ),
    responses(
        (status = 200, description = "Batch sentiment signals retrieved successfully", body = BatchSentimentResponse),
        (status = 400, description = "Invalid ticker list, excessive batch size (>50), conflicting params, or invalid date format", body = AuthErrorResponse),
        (status = 401, description = "Unauthorized (missing or invalid Bearer JWT or API key)", body = AuthErrorResponse),
        (status = 404, description = "Specified universe_id not found or not owned by user", body = AuthErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::rate_limit::RateLimitErrorResponse)
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn get_batch_sentiment_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Query(params): Query<BatchSentimentParams>,
) -> Response {
    let has_tickers = params
        .tickers
        .as_ref()
        .map(|t| !t.trim().is_empty())
        .unwrap_or(false);
    let has_universe = params.universe_id.is_some();

    if has_tickers && has_universe {
        let err = AuthErrorResponse {
            error: "Bad Request".to_string(),
            message: "Specify either tickers or universe_id, not both".to_string(),
        };
        return (StatusCode::BAD_REQUEST, Json(err)).into_response();
    }

    if !has_tickers && !has_universe {
        let err = AuthErrorResponse {
            error: "Bad Request".to_string(),
            message: "Either tickers or universe_id must be provided".to_string(),
        };
        return (StatusCode::BAD_REQUEST, Json(err)).into_response();
    }

    // 0b. Validate as_of_utc if provided
    if let Some(ref as_of_str) = params.as_of_utc {
        if !as_of_str.trim().is_empty() {
            if chrono::DateTime::parse_from_rfc3339(as_of_str.trim()).is_err() {
                let err = AuthErrorResponse {
                    error: "Bad Request".to_string(),
                    message: format!("Invalid as_of_utc format '{}', expected RFC3339", as_of_str),
                };
                return (StatusCode::BAD_REQUEST, Json(err)).into_response();
            }
        }
    }

    // 1. Resolve raw tickers list
    let candidate_tickers: Vec<String> = if let Some(universe_id) = params.universe_id {
        let universe_opt = if let Some(ref pool) = state.db_pool {
            match get_universe_from_db(pool, &universe_id, &claims.sub).await {
                Ok(u) => u,
                Err(e) => {
                    warn!(
                        "[Batch Sentiment] DB lookup error for universe '{}': {}",
                        universe_id, e
                    );
                    state
                        .universe_registry
                        .get_by_user(&universe_id, &claims.sub)
                }
            }
        } else {
            state
                .universe_registry
                .get_by_user(&universe_id, &claims.sub)
        };

        match universe_opt {
            Some(u) => u.tickers,
            None => {
                let err = AuthErrorResponse {
                    error: "Not Found".to_string(),
                    message: format!("Universe '{}' not found or not owned by user", universe_id),
                };
                return (StatusCode::NOT_FOUND, Json(err)).into_response();
            }
        }
    } else {
        let raw = params.tickers.as_deref().unwrap_or("");
        raw.split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    };

    if candidate_tickers.is_empty() {
        let err = AuthErrorResponse {
            error: "Bad Request".to_string(),
            message: "No valid ticker symbols found".to_string(),
        };
        return (StatusCode::BAD_REQUEST, Json(err)).into_response();
    }

    if candidate_tickers.len() > MAX_BATCH_TICKERS {
        let err = AuthErrorResponse {
            error: "Bad Request".to_string(),
            message: format!(
                "Requested {} tickers exceeds maximum batch limit of {}",
                candidate_tickers.len(),
                MAX_BATCH_TICKERS
            ),
        };
        return (StatusCode::BAD_REQUEST, Json(err)).into_response();
    }

    let mut validated_tickers: Vec<String> = Vec::with_capacity(candidate_tickers.len());
    for raw in &candidate_tickers {
        match QuestDbClient::validate_and_escape_ticker(raw) {
            Ok(t) => {
                if !validated_tickers.contains(&t) {
                    validated_tickers.push(t);
                }
            }
            Err(e) => {
                let err = AuthErrorResponse {
                    error: "Bad Request".to_string(),
                    message: format!("Invalid ticker '{}': {}", raw, e),
                };
                return (StatusCode::BAD_REQUEST, Json(err)).into_response();
            }
        }
    }

    // 2. Validate optional date
    let date_opt = params
        .date
        .as_deref()
        .filter(|d| !d.trim().is_empty() && d.trim().to_uppercase() != "LATEST");
    let fallback_date = date_opt.unwrap_or("LATEST");

    let parsed_date = if let Some(d_str) = date_opt {
        match NaiveDate::parse_from_str(d_str.trim(), "%Y-%m-%d") {
            Ok(d) => Some(d),
            Err(e) => {
                let err = AuthErrorResponse {
                    error: "Bad Request".to_string(),
                    message: format!(
                        "Invalid date format '{}', expected YYYY-MM-DD: {}",
                        d_str, e
                    ),
                };
                return (StatusCode::BAD_REQUEST, Json(err)).into_response();
            }
        }
    } else {
        None
    };

    // 3. Point-in-Time (PIT) Universe Filtering
    let pit_data = GLOBAL_PIT_DATA.clone();
    let active_tickers: Vec<String> = if let Some(date) = parsed_date {
        if pit_data.is_enabled() {
            pit_data.filter_universe_by_date(&validated_tickers, date)
        } else {
            validated_tickers
        }
    } else {
        validated_tickers
    };

    // 4. Fast-path Mock Mode Fallback
    if crate::state::is_questdb_mock_fallback_enabled() {
        let mut results = Vec::new();
        let now = Utc::now();
        let pub_dt = now - chrono::Duration::milliseconds(100);
        let ing_dt = now - chrono::Duration::milliseconds(50);
        let com_dt = now;

        for (idx, ticker) in active_tickers.iter().enumerate() {
            if let Some(record) = state
                .scd2_registry
                .query_as_of(ticker, params.as_of_utc.as_deref())
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
                results.push(SentimentResponse {
                    ticker: ticker.clone(),
                    date: fallback_date.to_string(),
                    sentiment_score: record.sentiment_score,
                    sentiment_label,
                    confidence,
                    probabilities,
                    signal_available_ts_us: Utc::now().timestamp_micros() as u64,
                    data_quality_score: record.data_quality_score,
                    message: "Point-in-time sentiment signal retrieved (SCD2)".to_string(),
                    model_version: record
                        .model_version
                        .or_else(|| Some(state.get_model_version())),
                    pipeline_version: record
                        .pipeline_version
                        .or_else(|| Some(state.get_pipeline_version())),
                    data_provenance: record
                        .data_provenance
                        .or_else(|| Some(state.get_data_provenance())),
                    language: record.language,
                    published_utc: Some(record.published_utc),
                    ingested_utc: record.ingested_utc,
                    db_commit_utc: record.db_commit_utc,
                    valid_from: record.valid_from,
                    valid_to: record.valid_to,
                    revision_number: record.revision_number,
                    is_current: record.is_current,
                    ..Default::default()
                });
                continue;
            }

            let hash_val = ticker.bytes().map(|b| b as usize).sum::<usize>();
            let phase = ((hash_val + idx * 23) as f64) * 0.17;
            let sentiment_score = ((phase.sin() * 0.65) + 0.15).clamp(-1.0, 1.0);
            let sentiment_label = if sentiment_score > 0.15 {
                "BULLISH".to_string()
            } else if sentiment_score < -0.15 {
                "BEARISH".to_string()
            } else {
                "NEUTRAL".to_string()
            };

            let (confidence, probabilities) =
                crate::models::sentiment::compute_confidence_and_probabilities(
                    sentiment_score,
                    &sentiment_label,
                    None,
                    None,
                    None,
                );

            let data_quality_score = crate::quality::compute_data_quality_score(
                "Institutional Wire",
                &format!(
                    "{} Point-in-time financial news sentiment and microstructure flow",
                    ticker
                ),
                confidence,
            );

            results.push(SentimentResponse {
                ticker: ticker.clone(),
                date: fallback_date.to_string(),
                sentiment_score: (sentiment_score * 100.0).round() / 100.0,
                sentiment_label,
                confidence,
                probabilities,
                signal_available_ts_us: Utc::now().timestamp_micros() as u64,
                data_quality_score,
                message: "Point-in-time sentiment signal retrieved".to_string(),
                model_version: Some(state.get_model_version()),
                pipeline_version: Some(state.get_pipeline_version()),
                data_provenance: Some(state.get_data_provenance()),
                language: "english".to_string(),
                published_utc: Some(pub_dt.to_rfc3339()),
                ingested_utc: Some(ing_dt.to_rfc3339()),
                db_commit_utc: Some(com_dt.to_rfc3339()),
                valid_from: Some(com_dt.to_rfc3339()),
                valid_to: None,
                revision_number: Some(1),
                is_current: Some(true),
                ..Default::default()
            });
        }

        let resp = BatchSentimentResponse {
            count: results.len(),
            results,
            model_version: Some(state.get_model_version()),
            pipeline_version: Some(state.get_pipeline_version()),
            data_provenance: Some(state.get_data_provenance()),
        };
        return (StatusCode::OK, Json(resp)).into_response();
    }

    // 5. Live Production QuestDB Execution
    info!(
        "[Batch Sentiment] Executing batch query for {} tickers, date='{}'",
        active_tickers.len(),
        fallback_date
    );

    match QUESTDB_CLIENT
        .query_batch_sentiment(&active_tickers, date_opt)
        .await
    {
        Ok(mut results) => {
            for r in &mut results {
                if r.model_version.is_none() {
                    r.model_version = Some(state.get_model_version());
                }
                if r.pipeline_version.is_none() {
                    r.pipeline_version = Some(state.get_pipeline_version());
                }
                if r.data_provenance.is_none() {
                    r.data_provenance = Some(state.get_data_provenance());
                }
            }
            let resp = BatchSentimentResponse {
                count: results.len(),
                results,
                model_version: Some(state.get_model_version()),
                pipeline_version: Some(state.get_pipeline_version()),
                data_provenance: Some(state.get_data_provenance()),
            };
            (StatusCode::OK, Json(resp)).into_response()
        }
        Err(err) => {
            error!("[Batch Sentiment] QuestDB error: {}", err);
            let message = if crate::state::is_production_mode() {
                "Required data source unavailable in production mode.".to_string()
            } else {
                format!("QuestDB query execution failed: {}", err)
            };
            let err_body = AuthErrorResponse {
                error: "Service Unavailable".to_string(),
                message,
            };
            (StatusCode::SERVICE_UNAVAILABLE, Json(err_body)).into_response()
        }
    }
}
