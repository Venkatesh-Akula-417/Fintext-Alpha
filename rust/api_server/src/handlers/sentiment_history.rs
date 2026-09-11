//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Historical Sentiment Time Series JSON Query Handler
//! ═══════════════════════════════════════════════════════════════════════════════

use crate::auth::AuthErrorResponse;
use crate::models::{SentimentHistoryParams, SentimentHistoryResponse, SentimentRecord};
use crate::state::AppState;
use crate::storage::{QuestDbClient, QuestDbClientConfig};
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use chrono::{Duration, NaiveDate, Utc};
use once_cell::sync::Lazy;
use tracing::{error, info, warn};

static QUESTDB_CLIENT: Lazy<QuestDbClient> =
    Lazy::new(|| QuestDbClient::new(QuestDbClientConfig::default()));

pub const DEFAULT_HISTORY_LIMIT: u32 = 100;
pub const MAX_HISTORY_LIMIT: u32 = 1000;

/// Query Historical Sentiment Time Series (JSON).
///
/// Retrieves point-in-time historical sentiment scores, microstructure indicators (VPIN, GEX),
/// and publication metadata for a given stock ticker across a specified date range with pagination and sorting.
#[utoipa::path(
    get,
    path = "/sentiment/history",
    tag = "Sentiment Analysis",
    params(
        ("ticker" = String, Query, description = "Target stock ticker symbol (e.g., 'AAPL', 'NVDA')"),
        ("start_date" = String, Query, description = "Start date in ISO format YYYY-MM-DD (e.g., '2025-01-01')"),
        ("end_date" = String, Query, description = "End date in ISO format YYYY-MM-DD (e.g., '2025-03-31')"),
        ("limit" = Option<u32>, Query, description = "Maximum number of records per page (default: 100, max: 1000)"),
        ("offset" = Option<u32>, Query, description = "Number of records to skip for pagination (default: 0)"),
        ("sort" = Option<String>, Query, description = "Sort direction: 'asc' or 'desc' (default: 'asc')"),
        ("min_quality" = Option<f32>, Query, description = "Minimum data quality score filter (0.0 to 1.0, default: 0.0)")
    ),
    responses(
        (status = 200, description = "Historical sentiment time series retrieved successfully", body = SentimentHistoryResponse),
        (status = 400, description = "Invalid date range or parameter validation error", body = AuthErrorResponse),
        (status = 401, description = "Unauthorized (missing or invalid Bearer JWT or API key)", body = AuthErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::rate_limit::RateLimitErrorResponse)
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn get_sentiment_history_handler(
    State(state): State<AppState>,
    Query(params): Query<SentimentHistoryParams>,
) -> Response {
    // 1. Validate ticker
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

    // 2. Validate dates
    let start_date_str = params.start_date.trim();
    let end_date_str = params.end_date.trim();

    let parsed_start = match NaiveDate::parse_from_str(start_date_str, "%Y-%m-%d") {
        Ok(d) => d,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(AuthErrorResponse {
                    error: "Bad Request".to_string(),
                    message: format!(
                        "Invalid start_date format '{}', expected YYYY-MM-DD: {}",
                        start_date_str, e
                    ),
                }),
            )
                .into_response();
        }
    };

    let parsed_end = match NaiveDate::parse_from_str(end_date_str, "%Y-%m-%d") {
        Ok(d) => d,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(AuthErrorResponse {
                    error: "Bad Request".to_string(),
                    message: format!(
                        "Invalid end_date format '{}', expected YYYY-MM-DD: {}",
                        end_date_str, e
                    ),
                }),
            )
                .into_response();
        }
    };

    if parsed_start > parsed_end {
        return (
            StatusCode::BAD_REQUEST,
            Json(AuthErrorResponse {
                error: "Bad Request".to_string(),
                message: "start_date cannot be after end_date".to_string(),
            }),
        )
            .into_response();
    }

    // 2b. Validate as_of_utc if present
    if let Some(ref as_of_str) = params.as_of_utc {
        if !as_of_str.trim().is_empty() {
            if chrono::DateTime::parse_from_rfc3339(as_of_str.trim()).is_err() {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(AuthErrorResponse {
                        error: "Bad Request".to_string(),
                        message: format!("Invalid as_of_utc format '{}', expected RFC3339", as_of_str),
                    }),
                )
                    .into_response();
            }
        }
    }

    // 3. Validate pagination & sorting
    let limit = match params.limit {
        Some(l) if l == 0 || l > MAX_HISTORY_LIMIT => {
            return (
                StatusCode::BAD_REQUEST,
                Json(AuthErrorResponse {
                    error: "Bad Request".to_string(),
                    message: format!("limit must be between 1 and {}", MAX_HISTORY_LIMIT),
                }),
            )
                .into_response();
        }
        Some(l) => l,
        None => DEFAULT_HISTORY_LIMIT,
    };

    let offset = params.offset.unwrap_or(0);

    let sort_raw = params.sort.as_deref().unwrap_or("asc").trim();
    let sort_order = match sort_raw.to_lowercase().as_str() {
        "asc" => "asc".to_string(),
        "desc" => "desc".to_string(),
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(AuthErrorResponse {
                    error: "Bad Request".to_string(),
                    message: format!(
                        "Invalid sort parameter '{}', must be 'asc' or 'desc'",
                        sort_raw
                    ),
                }),
            )
                .into_response();
        }
    };

    let min_quality = match params.min_quality {
        Some(q) if !(0.0..=1.0).contains(&q) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(AuthErrorResponse {
                    error: "Bad Request".to_string(),
                    message: format!("min_quality must be between 0.0 and 1.0, got {}", q),
                }),
            )
                .into_response();
        }
        Some(q) => q,
        None => 0.0,
    };

    let model_version = Some(state.get_model_version());
    let pipeline_version = Some(state.get_pipeline_version());
    let data_provenance = Some(state.get_data_provenance());

    // 3b. Point-in-Time (PIT) Validation & Delisting Window Clamping
    let pit_data = crate::pit::GLOBAL_PIT_DATA.clone();
    let (effective_start, effective_end) = if pit_data.is_enabled() {
        if !pit_data.is_valid_ticker(&ticker, parsed_start) {
            info!("[PIT] Ticker '{}' was not active/listed on start_date '{}'. Returning empty history.", ticker, parsed_start);
            return (
                StatusCode::OK,
                Json(SentimentHistoryResponse {
                    ticker,
                    start_date: start_date_str.to_string(),
                    end_date: end_date_str.to_string(),
                    count: 0,
                    total: 0,
                    limit,
                    offset,
                    sort: sort_order,
                    records: Vec::new(),
                    model_version,
                    pipeline_version,
                    data_provenance,
                }),
            )
                .into_response();
        }

        match pit_data.clamp_query_range(&ticker, parsed_start, parsed_end) {
            Some((s, e)) => (s, e),
            None => {
                return (
                    StatusCode::OK,
                    Json(SentimentHistoryResponse {
                        ticker,
                        start_date: start_date_str.to_string(),
                        end_date: end_date_str.to_string(),
                        count: 0,
                        total: 0,
                        limit,
                        offset,
                        sort: sort_order,
                        records: Vec::new(),
                        model_version,
                        pipeline_version,
                        data_provenance,
                    }),
                )
                    .into_response();
            }
        }
    } else {
        (parsed_start, parsed_end)
    };

    let is_primary = state.timescaledb_primary();

    // Phase 2: If TimescaleDB is primary, query TimescaleDB first
    if is_primary {
        if let Some(resp) = fetch_timescaledb_history(
            &state,
            &ticker,
            effective_start,
            effective_end,
            start_date_str,
            end_date_str,
            limit,
            offset,
            &sort_order,
            model_version.clone(),
            pipeline_version.clone(),
            data_provenance.clone(),
        )
        .await
        {
            return resp;
        }
        warn!(
            "[TimescaleDB Primary] Historical records empty or error for ticker '{}'. Falling back to QuestDB",
            ticker
        );
    }

    // 4. Mock mode for unit and integration testing without Docker QuestDB
    if crate::state::is_questdb_mock_fallback_enabled() {
        let mut total_records = Vec::new();
        let days_diff = (effective_end - effective_start).num_days();
        let num_days = (days_diff + 1).max(1) as usize;

        for i in 0..num_days {
            let cur_date = effective_start + Duration::days(i as i64);
            let date_iso = format!("{}T14:30:00.000000Z", cur_date.format("%Y-%m-%d"));
            let score = 0.25 + 0.5 * ((i as f64) * 0.1).sin();
            let vpin = 0.55 + 0.1 * ((i as f64) * 0.05).cos();
            let gex = (i as f64) * 59940.0;

            let source = if i % 5 == 0 {
                "SEC EDGAR".to_string()
            } else if i % 3 == 0 {
                "Finnhub".to_string()
            } else {
                "Institutional Wire".to_string()
            };

            let title = format!(
                "{} Market Sentiment and Microstructure Flow Analysis Report",
                ticker
            );
            let (confidence, _) = crate::models::sentiment::compute_confidence_and_probabilities(
                score, "NEUTRAL", None, None, None,
            );

            let data_quality_score =
                crate::quality::compute_data_quality_score(&source, &title, confidence);

            let lang_res = crate::language::detect_language(&title);
            let (language, eff_model) = if lang_res.is_multilingual_model_applied {
                (
                    lang_res.language,
                    Some(crate::language::MULTILINGUAL_MODEL_VERSION.to_string()),
                )
            } else {
                (lang_res.language, model_version.clone())
            };

            let ingested_iso = format!("{}T14:30:00.050000Z", cur_date.format("%Y-%m-%d"));
            let db_commit_iso = format!("{}T14:30:00.100000Z", cur_date.format("%Y-%m-%d"));

            total_records.push(SentimentRecord {
                published_utc: date_iso,
                ticker: ticker.clone(),
                source: source.clone(),
                title,
                sentiment_score: (score * 10000.0).round() / 10000.0,
                vpin: (vpin * 10000.0).round() / 10000.0,
                gamma_exposure: gex,
                data_quality_score,
                model_version: eff_model,
                pipeline_version: pipeline_version.clone(),
                data_provenance: Some(vec![source]),
                language,
                ingested_utc: Some(ingested_iso.clone()),
                db_commit_utc: Some(db_commit_iso.clone()),
                valid_from: Some(db_commit_iso),
                valid_to: None,
                revision_number: Some(1),
                is_current: Some(true),
            });
        }

        if let Some(ref as_of_str) = params.as_of_utc {
            let reg_revs = state.scd2_registry.get_revisions_for_ticker(&ticker);
            if !reg_revs.is_empty() {
                total_records = reg_revs;
            }

            if let Ok(as_of_dt) = chrono::DateTime::parse_from_rfc3339(as_of_str.trim()) {
                let as_of_utc = as_of_dt.with_timezone(&chrono::Utc);
                total_records.retain(|r| {
                    let from_ok = r.valid_from.as_deref()
                        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                        .map(|dt| dt <= as_of_utc)
                        .unwrap_or(true);
                    let to_ok = match r.valid_to.as_deref() {
                        Some(to_str) => chrono::DateTime::parse_from_rfc3339(to_str)
                            .ok()
                            .map(|dt| dt > as_of_utc)
                            .unwrap_or(false),
                        None => true,
                    };
                    from_ok && to_ok
                });
            }
        }

        if sort_order == "desc" {
            total_records.reverse();
        }

        // Apply min_quality filter
        let filtered_records: Vec<SentimentRecord> = total_records
            .into_iter()
            .filter(|r| r.data_quality_score >= min_quality)
            .collect();

        let total = filtered_records.len();
        let paged_records: Vec<SentimentRecord> = filtered_records
            .into_iter()
            .skip(offset as usize)
            .take(limit as usize)
            .collect();
        let count = paged_records.len();

        return (
            StatusCode::OK,
            Json(SentimentHistoryResponse {
                ticker,
                start_date: start_date_str.to_string(),
                end_date: end_date_str.to_string(),
                count,
                total,
                limit,
                offset,
                sort: sort_order,
                records: paged_records,
                model_version,
                pipeline_version,
                data_provenance,
            }),
        )
            .into_response();
    }

    // 5. Query QuestDB SQL
    let sql = match QuestDbClient::build_sentiment_history_query(
        &ticker,
        start_date_str,
        end_date_str,
        limit,
        offset,
        &sort_order,
    ) {
        Ok(s) => s,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(AuthErrorResponse {
                    error: "Bad Request".to_string(),
                    message: e,
                }),
            )
                .into_response();
        }
    };

    info!("[Sentiment History SQL] Executing: {}", sql);

    let endpoint = format!("{}/exec", QUESTDB_CLIENT.config().url.trim_end_matches('/'));
    match reqwest::Client::new()
        .get(&endpoint)
        .query(&[("query", &sql)])
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            match resp.json::<serde_json::Value>().await {
                Ok(val) => {
                    match QuestDbClient::parse_sentiment_history_exec_response(&val) {
                        Ok(mut records) => {
                            for r in &mut records {
                                if r.model_version.is_none() {
                                    r.model_version = model_version.clone();
                                }
                                if r.pipeline_version.is_none() {
                                    r.pipeline_version = pipeline_version.clone();
                                }
                                if r.data_provenance.is_none() {
                                    r.data_provenance = Some(vec![r.source.clone()]);
                                }
                            }

                            // Apply min_quality filter
                            let filtered: Vec<SentimentRecord> = records
                                .into_iter()
                                .filter(|r| r.data_quality_score >= min_quality)
                                .collect();

                            let count = filtered.len();
                            let total = val
                                .get("count")
                                .and_then(|c| c.as_u64())
                                .unwrap_or(count as u64)
                                as usize;

                            (
                                StatusCode::OK,
                                Json(SentimentHistoryResponse {
                                    ticker,
                                    start_date: start_date_str.to_string(),
                                    end_date: end_date_str.to_string(),
                                    count,
                                    total: total.max(count),
                                    limit,
                                    offset,
                                    sort: sort_order,
                                    records: filtered,
                                    model_version,
                                    pipeline_version,
                                    data_provenance,
                                }),
                            )
                                .into_response()
                        }
                        Err(e) => {
                            error!("Failed to parse QuestDB dataset response: {}", e);
                            (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(AuthErrorResponse {
                                    error: "Internal Error".to_string(),
                                    message: format!("Failed to parse database records: {}", e),
                                }),
                            )
                                .into_response()
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to decode QuestDB JSON: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(AuthErrorResponse {
                            error: "Internal Error".to_string(),
                            message: "Failed to read database response".to_string(),
                        }),
                    )
                        .into_response()
                }
            }
        }
        Ok(resp) => {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            warn!("QuestDB error response ({}): {}", status, body);

            // If primary is false, check TimescaleDB fallback before returning error
            if !is_primary && state.timescaledb_client.is_enabled() {
                if let Some(res) = fetch_timescaledb_history(
                    &state,
                    &ticker,
                    effective_start,
                    effective_end,
                    start_date_str,
                    end_date_str,
                    limit,
                    offset,
                    &sort_order,
                    model_version.clone(),
                    pipeline_version.clone(),
                    data_provenance.clone(),
                )
                .await
                {
                    return res;
                }
            }

            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(AuthErrorResponse {
                    error: "Service Unavailable".to_string(),
                    message: "Database query failed".to_string(),
                }),
            )
                .into_response()
        }
        Err(e) => {
            warn!("QuestDB connection failure: {}", e);

            // If primary is false, check TimescaleDB fallback before returning error
            if !is_primary && state.timescaledb_client.is_enabled() {
                if let Some(res) = fetch_timescaledb_history(
                    &state,
                    &ticker,
                    effective_start,
                    effective_end,
                    start_date_str,
                    end_date_str,
                    limit,
                    offset,
                    &sort_order,
                    model_version.clone(),
                    pipeline_version.clone(),
                    data_provenance.clone(),
                )
                .await
                {
                    return res;
                }
            }

            let message = if crate::state::is_production_mode() {
                "Required data source unavailable in production mode.".to_string()
            } else {
                "Failed to connect to database".to_string()
            };
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(AuthErrorResponse {
                    error: "Service Unavailable".to_string(),
                    message,
                }),
            )
                .into_response()
        }
    }
}

/// Helper function to query TimescaleDB for sentiment history and format SentimentHistoryResponse.
async fn fetch_timescaledb_history(
    state: &AppState,
    ticker: &str,
    effective_start: NaiveDate,
    effective_end: NaiveDate,
    start_date_str: &str,
    end_date_str: &str,
    limit: u32,
    offset: u32,
    sort_order: &str,
    model_version: Option<String>,
    pipeline_version: Option<String>,
    data_provenance: Option<Vec<String>>,
) -> Option<Response> {
    if !state.timescaledb_client.is_enabled() {
        return None;
    }

    let start_dt = chrono::DateTime::<Utc>::from_naive_utc_and_offset(
        effective_start.and_hms_opt(0, 0, 0)?,
        Utc,
    );
    let end_dt = chrono::DateTime::<Utc>::from_naive_utc_and_offset(
        effective_end.and_hms_opt(23, 59, 59)?,
        Utc,
    );

    if let Ok(ts_records) = state
        .timescaledb_client
        .query_sentiment_history(ticker, start_dt, end_dt, (limit + offset) as i64)
        .await
    {
        if !ts_records.is_empty() {
            let total = ts_records.len() as u32;
            let mut paged_records: Vec<SentimentRecord> = ts_records
                .into_iter()
                .skip(offset as usize)
                .take(limit as usize)
                .map(|r| SentimentRecord {
                    published_utc: r.published_utc.to_rfc3339(),
                    ticker: r.ticker,
                    source: r.source.clone(),
                    title: r.title,
                    sentiment_score: r.sentiment_score,
                    vpin: r.vpin.unwrap_or(0.0),
                    gamma_exposure: r.gamma_exposure.unwrap_or(0.0),
                    data_quality_score: r.data_quality_score as f32,
                    model_version: model_version.clone(),
                    pipeline_version: pipeline_version.clone(),
                    data_provenance: Some(vec![r.source]),
                    language: "en".to_string(),
                    ingested_utc: Some(r.ingested_utc.to_rfc3339()),
                    db_commit_utc: Some(r.db_commit_utc.to_rfc3339()),
                    valid_from: Some(r.valid_from.to_rfc3339()),
                    valid_to: r.valid_to.map(|dt| dt.to_rfc3339()),
                    revision_number: Some(r.revision_number),
                    is_current: Some(r.is_current),
                })
                .collect();

            if sort_order == "desc" {
                paged_records.sort_by(|a, b| b.published_utc.cmp(&a.published_utc));
            }

            let count = paged_records.len();
            return Some(
                (
                    StatusCode::OK,
                    Json(SentimentHistoryResponse {
                        ticker: ticker.to_string(),
                        start_date: start_date_str.to_string(),
                        end_date: end_date_str.to_string(),
                        count,
                        total: total as usize,
                        limit,
                        offset,
                        sort: sort_order.to_string(),
                        records: paged_records,
                        model_version,
                        pipeline_version,
                        data_provenance,
                    }),
                )
                    .into_response(),
            );
        }
    }
    None
}
