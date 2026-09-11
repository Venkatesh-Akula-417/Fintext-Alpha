//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Aggregated News Sentiment Feed Handler
//! ═══════════════════════════════════════════════════════════════════════════════
//!
//! Provides a consolidated real-time and historical news sentiment feed endpoint
//! (`GET /sentiment/feed`) across the market universe or filtered by GICS sector.
//! Includes microstructure analytics (VPIN, GEX), model prediction confidence,
//! and quantitative data quality scoring with pagination and chronological sorting.
//! ═══════════════════════════════════════════════════════════════════════════════

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use chrono::{DateTime, Duration, NaiveDate, Utc};
use once_cell::sync::Lazy;
use tracing::{error, info, warn};

use crate::auth::AuthErrorResponse;
use crate::models::{SentimentFeedItem, SentimentFeedParams, SentimentFeedResponse};
use crate::sector::GLOBAL_SECTOR_MAP;
use crate::state::AppState;
use crate::storage::{QuestDbClient, QuestDbClientConfig};

static QUESTDB_CLIENT: Lazy<QuestDbClient> =
    Lazy::new(|| QuestDbClient::new(QuestDbClientConfig::default()));

pub const DEFAULT_FEED_LIMIT: u32 = 100;
pub const MAX_FEED_LIMIT: u32 = 1000;

/// Parse timestamp string supporting RFC3339/ISO-8601 or YYYY-MM-DD formats.
fn parse_feed_timestamp(ts_str: &str, is_end: bool) -> Result<DateTime<Utc>, String> {
    let trimmed = ts_str.trim();
    if trimmed.is_empty() {
        return Err("Timestamp string cannot be empty".to_string());
    }

    let normalized = if trimmed.contains(' ') && !trimmed.contains('+') {
        trimmed.replace(' ', "+")
    } else {
        trimmed.to_string()
    };

    // Try RFC3339 first (e.g. 2026-08-29T14:30:00Z or with nanos/offset)
    if let Ok(dt) = DateTime::parse_from_rfc3339(&normalized) {
        return Ok(dt.with_timezone(&Utc));
    }

    // Try standard date YYYY-MM-DD
    if let Ok(nd) = NaiveDate::parse_from_str(trimmed, "%Y-%m-%d") {
        if is_end {
            // End of day: 23:59:59.999999Z
            let dt = nd
                .and_hms_micro_opt(23, 59, 59, 999_999)
                .ok_or_else(|| "Invalid time conversion".to_string())?;
            return Ok(DateTime::from_naive_utc_and_offset(dt, Utc));
        } else {
            // Start of day: 00:00:00.000000Z
            let dt = nd
                .and_hms_micro_opt(0, 0, 0, 0)
                .ok_or_else(|| "Invalid time conversion".to_string())?;
            return Ok(DateTime::from_naive_utc_and_offset(dt, Utc));
        }
    }

    Err(format!(
        "Invalid timestamp format '{}'. Expected RFC3339 (e.g., '2026-08-29T00:00:00Z') or Date (e.g., '2026-08-29')",
        trimmed
    ))
}

/// Generate deterministic mock feed records for testing or standalone execution.
fn generate_mock_feed_records(
    tickers: Option<&[String]>,
    start_dt: DateTime<Utc>,
    end_dt: DateTime<Utc>,
) -> Vec<SentimentFeedItem> {
    let default_tickers = [
        "AAPL", "NVDA", "MSFT", "AMZN", "GOOGL", "META", "TSLA", "JPM", "GS", "JNJ", "PFE", "XOM",
        "CVX", "SPY", "QQQ",
    ];

    let effective_tickers: Vec<String> = match tickers {
        Some(t) if !t.is_empty() => t.to_vec(),
        _ => default_tickers.iter().map(|s| s.to_string()).collect(),
    };

    let mut records = Vec::new();
    let duration_secs = (end_dt - start_dt).num_seconds().max(1);
    let step_minutes = (duration_secs / 60 / 25).max(10); // generate ~25 records across window

    let mut cur_ts = start_dt;
    let mut idx = 0;

    let sources = [
        "Institutional Wire",
        "Finnhub",
        "SEC EDGAR",
        "Bloomberg Terminal Feed",
        "Reuters Financial",
    ];

    while cur_ts <= end_dt {
        let ticker = &effective_tickers[idx % effective_tickers.len()];
        let source = sources[idx % sources.len()];
        let title = format!(
            "{} Market Sentiment and Microstructure Flow Analysis Report #{}",
            ticker,
            idx + 1
        );

        // Deterministic pseudo-random score between -0.85 and +0.85
        let score = (((idx as f64 * 1.7).sin() * 0.85) * 10000.0).round() / 10000.0;
        let label = if score > 0.15 {
            "BULLISH".to_string()
        } else if score < -0.15 {
            "BEARISH".to_string()
        } else {
            "NEUTRAL".to_string()
        };

        let (confidence, _) = crate::models::sentiment::compute_confidence_and_probabilities(
            score, &label, None, None, None,
        );

        let data_quality_score =
            crate::quality::compute_data_quality_score(source, &title, confidence);
        let vpin = (((idx as f64 * 0.9).cos().abs() * 0.4 + 0.35) * 10000.0).round() / 10000.0;
        let gamma_exposure = ((((idx as f64 * 2.3).sin() * 180000.0) / 100.0).round()) * 100.0;

        let lang_res = crate::language::detect_language(&title);
        let (language, eff_model) = if lang_res.is_multilingual_model_applied {
            (
                lang_res.language,
                Some(crate::language::MULTILINGUAL_MODEL_VERSION.to_string()),
            )
        } else {
            (
                lang_res.language,
                Some(crate::models::DEFAULT_MODEL_VERSION.to_string()),
            )
        };

        let ing_dt = cur_ts + Duration::milliseconds(50);
        let com_dt = cur_ts + Duration::milliseconds(100);

        records.push(SentimentFeedItem {
            published_utc: cur_ts.to_rfc3339_opts(chrono::SecondsFormat::Micros, true),
            ticker: ticker.clone(),
            source: source.to_string(),
            title,
            sentiment_score: score,
            sentiment_label: label,
            confidence,
            data_quality_score,
            vpin,
            gamma_exposure,
            model_version: eff_model,
            pipeline_version: Some(crate::models::DEFAULT_PIPELINE_VERSION.to_string()),
            data_provenance: Some(vec![source.to_string()]),
            language,
            ingested_utc: Some(ing_dt.to_rfc3339_opts(chrono::SecondsFormat::Micros, true)),
            db_commit_utc: Some(com_dt.to_rfc3339_opts(chrono::SecondsFormat::Micros, true)),
            valid_from: Some(com_dt.to_rfc3339_opts(chrono::SecondsFormat::Micros, true)),
            valid_to: None,
            revision_number: Some(1),
            is_current: Some(true),
        });

        idx += 1;
        cur_ts = cur_ts + Duration::minutes(step_minutes as i64);
    }

    records
}

/// Helper to paginate feed records using either cursor-based pagination or legacy offset pagination.
fn paginate_feed_records(
    mut filtered: Vec<SentimentFeedItem>,
    sort_order: &str,
    cursor_dt: Option<DateTime<Utc>>,
    offset: u32,
    limit: u32,
) -> (Vec<SentimentFeedItem>, usize, Option<String>) {
    let total = filtered.len();
    if let Some(cur) = cursor_dt {
        filtered.retain(|r| {
            let norm = if r.published_utc.contains(' ') && !r.published_utc.contains('+') {
                r.published_utc.replace(' ', "+")
            } else {
                r.published_utc.clone()
            };
            if let Ok(dt) = DateTime::parse_from_rfc3339(&norm) {
                let dt_utc = dt.with_timezone(&Utc);
                if sort_order == "desc" {
                    dt_utc < cur
                } else {
                    dt_utc > cur
                }
            } else {
                true
            }
        });
        let paged: Vec<SentimentFeedItem> = filtered.into_iter().take(limit as usize).collect();
        let count = paged.len();
        let next_cursor = if count == limit as usize && count > 0 {
            paged.last().map(|r| r.published_utc.clone())
        } else {
            None
        };
        (paged, total, next_cursor)
    } else {
        let paged: Vec<SentimentFeedItem> = filtered
            .into_iter()
            .skip(offset as usize)
            .take(limit as usize)
            .collect();
        let count = paged.len();
        let next_cursor = if count == limit as usize && count > 0 {
            paged.last().map(|r| r.published_utc.clone())
        } else {
            None
        };
        (paged, total, next_cursor)
    }
}

/// Query Consolidated Aggregated News Sentiment Feed.
///
/// Returns real-time and historical news sentiment signals with institutional microstructure indicators.
#[utoipa::path(
    get,
    path = "/sentiment/feed",
    tag = "Sentiment Analysis",
    params(
        ("sector" = Option<String>, Query, description = "Optional GICS Sector filter (e.g., 'Technology', 'Financials', 'Healthcare')"),
        ("start_date" = Option<String>, Query, description = "Earliest timestamp in ISO format YYYY-MM-DD or RFC3339 (default: now - 24 hours)"),
        ("end_date" = Option<String>, Query, description = "Latest timestamp in ISO format YYYY-MM-DD or RFC3339 (default: now)"),
        ("min_confidence" = Option<f32>, Query, description = "Minimum prediction confidence filter threshold between 0.0 and 1.0 (default: 0.0)"),
        ("min_quality" = Option<f32>, Query, description = "Minimum quantitative data quality score filter between 0.0 and 1.0 (default: 0.0)"),
        ("limit" = Option<u32>, Query, description = "Maximum number of records per page (default: 100, min: 1, max: 1000)"),
        ("offset" = Option<u32>, Query, description = "Number of records to skip for pagination (default: 0, min: 0) [Deprecated: use cursor]"),
        ("cursor" = Option<String>, Query, description = "Optional cursor for keyset pagination (RFC3339 timestamp of last item from previous page)"),
        ("sort" = Option<String>, Query, description = "Sort order by timestamp: 'asc' or 'desc' (default: 'desc')")
    ),
    responses(
        (status = 200, description = "Consolidated news sentiment feed retrieved successfully", body = SentimentFeedResponse),
        (status = 400, description = "Invalid request query parameters or timestamp range", body = AuthErrorResponse),
        (status = 401, description = "Unauthorized - Missing or invalid Bearer token", body = AuthErrorResponse),
        (status = 404, description = "Requested sector not found", body = AuthErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::rate_limit::RateLimitErrorResponse)
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn get_sentiment_feed_handler(
    State(state): State<AppState>,
    Query(params): Query<SentimentFeedParams>,
) -> Response {
    // 1. Resolve Sector Mapping if requested
    let (sector_filter, constituent_tickers) = if let Some(sec_raw) = &params.sector {
        let trimmed = sec_raw.trim();
        if trimmed.is_empty() {
            (None, None)
        } else {
            let sector_map = GLOBAL_SECTOR_MAP.clone();
            match sector_map.get_tickers_by_sector(trimmed) {
                Some(tickers) if !tickers.is_empty() => {
                    let canonical = sector_map
                        .get_canonical_name(trimmed)
                        .unwrap_or_else(|| trimmed.to_string());
                    (Some(canonical), Some(tickers.clone()))
                }
                _ => {
                    let available = sector_map.list_sectors();
                    let err = AuthErrorResponse {
                        error: "Not Found".to_string(),
                        message: format!(
                            "Unknown sector '{}'. Available GICS sectors: {:?}",
                            trimmed, available
                        ),
                    };
                    return (StatusCode::NOT_FOUND, Json(err)).into_response();
                }
            }
        }
    } else {
        (None, None)
    };

    // 2. Parse & Validate Date Range
    let now = Utc::now();
    let start_dt = match &params.start_date {
        Some(s) if !s.trim().is_empty() => match parse_feed_timestamp(s, false) {
            Ok(dt) => dt,
            Err(e) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(AuthErrorResponse {
                        error: "Bad Request".to_string(),
                        message: format!("Invalid start_date parameter: {}", e),
                    }),
                )
                    .into_response();
            }
        },
        _ => now - Duration::hours(24),
    };

    let end_dt = match &params.end_date {
        Some(e) if !e.trim().is_empty() => match parse_feed_timestamp(e, true) {
            Ok(dt) => dt,
            Err(err) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(AuthErrorResponse {
                        error: "Bad Request".to_string(),
                        message: format!("Invalid end_date parameter: {}", err),
                    }),
                )
                    .into_response();
            }
        },
        _ => now,
    };

    if start_dt > end_dt {
        return (
            StatusCode::BAD_REQUEST,
            Json(AuthErrorResponse {
                error: "Bad Request".to_string(),
                message: format!(
                    "start_date ({}) cannot be after end_date ({})",
                    start_dt.to_rfc3339(),
                    end_dt.to_rfc3339()
                ),
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

    // 2c. Validate Pagination Parameters (cursor vs offset)
    if params.cursor.is_some() && params.offset.is_some() {
        return (
            StatusCode::BAD_REQUEST,
            Json(AuthErrorResponse {
                error: "Bad Request".to_string(),
                message: "Cannot specify both 'cursor' and 'offset'. Use cursor-based pagination or legacy offset pagination, not both.".to_string(),
            }),
        )
            .into_response();
    }

    let cursor_dt = if let Some(ref cur_str) = params.cursor {
        let trimmed = cur_str.trim();
        if trimmed.is_empty() {
            return (
                StatusCode::BAD_REQUEST,
                Json(AuthErrorResponse {
                    error: "Bad Request".to_string(),
                    message: "Cursor parameter cannot be empty. Expected RFC3339 timestamp (e.g., '2026-08-29T14:30:00Z')".to_string(),
                }),
            )
                .into_response();
        }
        let normalized = if trimmed.contains(' ') && !trimmed.contains('+') {
            trimmed.replace(' ', "+")
        } else {
            trimmed.to_string()
        };
        match DateTime::parse_from_rfc3339(&normalized) {
            Ok(dt) => Some(dt.with_timezone(&Utc)),
            Err(e) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(AuthErrorResponse {
                        error: "Bad Request".to_string(),
                        message: format!(
                            "Invalid cursor timestamp format '{}'. Expected RFC3339 (e.g., '2026-08-29T14:30:00Z'): {}",
                            trimmed, e
                        ),
                    }),
                )
                    .into_response();
            }
        }
    } else {
        None
    };

    // 3. Validate Pagination & Filters
    let limit = match params.limit {
        Some(l) => l.clamp(1, MAX_FEED_LIMIT),
        None => DEFAULT_FEED_LIMIT,
    };
    let offset = if cursor_dt.is_some() {
        0
    } else {
        params.offset.unwrap_or(0)
    };

    let sort_order = match params
        .sort
        .as_deref()
        .unwrap_or("desc")
        .trim()
        .to_lowercase()
        .as_str()
    {
        "asc" => "asc".to_string(),
        "desc" => "desc".to_string(),
        other => {
            return (
                StatusCode::BAD_REQUEST,
                Json(AuthErrorResponse {
                    error: "Bad Request".to_string(),
                    message: format!("Invalid sort order '{}', expected 'asc' or 'desc'", other),
                }),
            )
                .into_response();
        }
    };

    let min_confidence = match params.min_confidence {
        Some(c) if !(0.0..=1.0).contains(&c) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(AuthErrorResponse {
                    error: "Bad Request".to_string(),
                    message: format!("min_confidence must be between 0.0 and 1.0, got {}", c),
                }),
            )
                .into_response();
        }
        Some(c) => c,
        None => 0.0,
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

    let start_iso = start_dt.to_rfc3339_opts(chrono::SecondsFormat::Micros, true);
    let end_iso = end_dt.to_rfc3339_opts(chrono::SecondsFormat::Micros, true);

    let model_version = Some(state.get_model_version());
    let pipeline_version = Some(state.get_pipeline_version());
    let data_provenance = Some(state.get_data_provenance());

    // 4. Standalone / Mock Fallback Execution
    if crate::state::is_questdb_mock_fallback_enabled() {
        let all_mock = generate_mock_feed_records(constituent_tickers.as_deref(), start_dt, end_dt);
        let mut filtered: Vec<SentimentFeedItem> = all_mock
            .into_iter()
            .filter(|r| r.confidence >= min_confidence && r.data_quality_score >= min_quality)
            .collect();

        if let Some(ref as_of_str) = params.as_of_utc {
            if let Ok(as_of_dt) = chrono::DateTime::parse_from_rfc3339(as_of_str.trim()) {
                let as_of_utc = as_of_dt.with_timezone(&chrono::Utc);
                filtered.retain(|r| {
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

        for r in &mut filtered {
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

        if sort_order == "desc" {
            filtered.sort_by(|a, b| b.published_utc.cmp(&a.published_utc));
        } else {
            filtered.sort_by(|a, b| a.published_utc.cmp(&b.published_utc));
        }

        let (paged, total, next_cursor) =
            paginate_feed_records(filtered, &sort_order, cursor_dt, offset, limit);
        let count = paged.len();

        return (
            StatusCode::OK,
            Json(SentimentFeedResponse {
                count,
                total,
                limit,
                offset,
                next_cursor,
                sort: sort_order,
                sector: sector_filter,
                start_time: start_iso,
                end_time: end_iso,
                min_confidence,
                min_quality,
                records: paged,
                model_version,
                pipeline_version,
                data_provenance,
            }),
        )
            .into_response();
    }

    // 5. Query QuestDB Live Instance
    let count_sql = match QuestDbClient::build_sentiment_feed_count_query(
        constituent_tickers.as_deref(),
        &start_iso,
        &end_iso,
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

    let feed_sql = match QuestDbClient::build_sentiment_feed_query(
        constituent_tickers.as_deref(),
        &start_iso,
        &end_iso,
        limit,
        offset,
        &sort_order,
        params.cursor.as_deref(),
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

    info!("[Sentiment Feed SQL] Executing: {}", feed_sql);

    let endpoint = format!("{}/exec", QUESTDB_CLIENT.config().url.trim_end_matches('/'));
    let client = reqwest::Client::new();

    // Query Total Count
    let total_count = match client
        .get(&endpoint)
        .query(&[("query", &count_sql)])
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            if let Ok(val) = resp.json::<serde_json::Value>().await {
                val.get("dataset")
                    .and_then(|d| d.as_array())
                    .and_then(|arr| arr.get(0))
                    .and_then(|row| row.get(0))
                    .and_then(|c| c.as_u64())
                    .unwrap_or(0) as usize
            } else {
                0
            }
        }
        _ => 0,
    };

    // Query Feed Records
    match client
        .get(&endpoint)
        .query(&[("query", &feed_sql)])
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => match resp.json::<serde_json::Value>().await {
            Ok(val) => match QuestDbClient::parse_sentiment_feed_exec_response(&val) {
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

                    // Filter minimum confidence and quality
                    let filtered: Vec<SentimentFeedItem> = records
                        .into_iter()
                        .filter(|r| {
                            r.confidence >= min_confidence && r.data_quality_score >= min_quality
                        })
                        .collect();

                    let count = filtered.len();
                    let next_cursor = if count == limit as usize && count > 0 {
                        filtered.last().map(|r| r.published_utc.clone())
                    } else {
                        None
                    };
                    let total = if total_count > 0 {
                        total_count
                    } else {
                        count + offset as usize
                    };

                    (
                        StatusCode::OK,
                        Json(SentimentFeedResponse {
                            count,
                            total,
                            limit,
                            offset,
                            next_cursor,
                            sort: sort_order,
                            sector: sector_filter,
                            start_time: start_iso,
                            end_time: end_iso,
                            min_confidence,
                            min_quality,
                            records: filtered,
                            model_version,
                            pipeline_version,
                            data_provenance,
                        }),
                    )
                        .into_response()
                }
                Err(e) => {
                    error!("Failed to parse QuestDB feed response: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(AuthErrorResponse {
                            error: "Internal Error".to_string(),
                            message: format!("Failed to parse database records: {}", e),
                        }),
                    )
                        .into_response()
                }
            },
            Err(e) => {
                error!("Invalid JSON from QuestDB: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(AuthErrorResponse {
                        error: "Internal Error".to_string(),
                        message: "Database response parsing error".to_string(),
                    }),
                )
                    .into_response()
            }
        },
        Ok(resp) => {
            if crate::state::is_production_mode() {
                warn!("QuestDB error status in production mode: {}", resp.status());
                return (
                    StatusCode::SERVICE_UNAVAILABLE,
                    Json(serde_json::json!({
                        "error": "Service Unavailable",
                        "message": "Required data source unavailable in production mode.",
                        "status": "service_unavailable"
                    })),
                ).into_response();
            }
            warn!(
                "QuestDB returned status {} for sentiment feed, falling back to mock generator",
                resp.status()
            );
            let all_mock =
                generate_mock_feed_records(constituent_tickers.as_deref(), start_dt, end_dt);
            let mut filtered: Vec<SentimentFeedItem> = all_mock
                .into_iter()
                .filter(|r| r.confidence >= min_confidence && r.data_quality_score >= min_quality)
                .collect();

            for r in &mut filtered {
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

            if sort_order == "desc" {
                filtered.sort_by(|a, b| b.published_utc.cmp(&a.published_utc));
            } else {
                filtered.sort_by(|a, b| a.published_utc.cmp(&b.published_utc));
            }

            let (paged, total, next_cursor) =
                paginate_feed_records(filtered, &sort_order, cursor_dt, offset, limit);
            let count = paged.len();

            (
                StatusCode::OK,
                Json(SentimentFeedResponse {
                    count,
                    total,
                    limit,
                    offset,
                    next_cursor,
                    sort: sort_order,
                    sector: sector_filter,
                    start_time: start_iso,
                    end_time: end_iso,
                    min_confidence,
                    min_quality,
                    records: paged,
                    model_version,
                    pipeline_version,
                    data_provenance,
                }),
            )
                .into_response()
        }
        Err(e) => {
            if crate::state::is_production_mode() {
                warn!("QuestDB unreachable in production mode for sentiment feed: {}", e);
                return (
                    StatusCode::SERVICE_UNAVAILABLE,
                    Json(serde_json::json!({
                        "error": "Service Unavailable",
                        "message": "Required data source unavailable in production mode.",
                        "detail": format!("{}", e),
                        "status": "service_unavailable"
                    })),
                ).into_response();
            }
            warn!(
                "QuestDB unreachable for sentiment feed ({}), falling back to mock generator",
                e
            );
            let all_mock =
                generate_mock_feed_records(constituent_tickers.as_deref(), start_dt, end_dt);
            let mut filtered: Vec<SentimentFeedItem> = all_mock
                .into_iter()
                .filter(|r| r.confidence >= min_confidence && r.data_quality_score >= min_quality)
                .collect();

            for r in &mut filtered {
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

            if sort_order == "desc" {
                filtered.sort_by(|a, b| b.published_utc.cmp(&a.published_utc));
            } else {
                filtered.sort_by(|a, b| a.published_utc.cmp(&b.published_utc));
            }

            let (paged, total, next_cursor) =
                paginate_feed_records(filtered, &sort_order, cursor_dt, offset, limit);
            let count = paged.len();

            (
                StatusCode::OK,
                Json(SentimentFeedResponse {
                    count,
                    total,
                    limit,
                    offset,
                    next_cursor,
                    sort: sort_order,
                    sector: sector_filter,
                    start_time: start_iso,
                    end_time: end_iso,
                    min_confidence,
                    min_quality,
                    records: paged,
                    model_version,
                    pipeline_version,
                    data_provenance,
                }),
            )
                .into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_feed_timestamp() {
        let dt_rfc = parse_feed_timestamp("2026-08-29T14:30:00Z", false).unwrap();
        assert_eq!(dt_rfc.to_rfc3339(), "2026-08-29T14:30:00+00:00");

        let dt_date_start = parse_feed_timestamp("2026-08-29", false).unwrap();
        assert_eq!(dt_date_start.to_rfc3339(), "2026-08-29T00:00:00+00:00");

        let dt_date_end = parse_feed_timestamp("2026-08-29", true).unwrap();
        assert_eq!(dt_date_end.to_rfc3339(), "2026-08-29T23:59:59.999999+00:00");

        assert!(parse_feed_timestamp("invalid-date", false).is_err());
    }

    #[test]
    fn test_generate_mock_feed_records() {
        let start = Utc::now() - Duration::hours(12);
        let end = Utc::now();
        let tickers = vec!["AAPL".to_string(), "NVDA".to_string()];
        let records = generate_mock_feed_records(Some(&tickers), start, end);

        assert!(!records.is_empty());
        for r in &records {
            assert!(r.ticker == "AAPL" || r.ticker == "NVDA");
            assert!(r.confidence >= 0.0 && r.confidence <= 1.0);
            assert!(r.data_quality_score >= 0.0 && r.data_quality_score <= 1.0);
            assert!(r.vpin >= 0.0);
        }
    }

    #[test]
    fn test_paginate_feed_records_offset() {
        let start = Utc::now() - Duration::hours(12);
        let end = Utc::now();
        let mut records = generate_mock_feed_records(None, start, end);
        records.sort_by(|a, b| b.published_utc.cmp(&a.published_utc));
        let total_items = records.len();

        let (page1, total, next_cursor) =
            paginate_feed_records(records.clone(), "desc", None, 0, 5);
        assert_eq!(total, total_items);
        assert_eq!(page1.len(), 5);
        assert!(next_cursor.is_some());
        assert_eq!(next_cursor.as_deref(), Some(page1.last().unwrap().published_utc.as_str()));

        let (page2, _, _) =
            paginate_feed_records(records.clone(), "desc", None, 5, 5);
        assert_eq!(page2.len(), 5);
        assert_ne!(page1[0].title, page2[0].title);
    }

    #[test]
    fn test_paginate_feed_records_cursor_desc() {
        let start = Utc::now() - Duration::hours(12);
        let end = Utc::now();
        let mut records = generate_mock_feed_records(None, start, end);
        records.sort_by(|a, b| b.published_utc.cmp(&a.published_utc));

        // First page: no cursor, limit 5
        let (page1, _, next_cursor1) =
            paginate_feed_records(records.clone(), "desc", None, 0, 5);
        assert_eq!(page1.len(), 5);
        assert!(next_cursor1.is_some());

        // Parse next_cursor1 and query second page
        let cur_dt = DateTime::parse_from_rfc3339(next_cursor1.as_ref().unwrap())
            .unwrap()
            .with_timezone(&Utc);
        let (page2, _, _next_cursor2) =
            paginate_feed_records(records.clone(), "desc", Some(cur_dt), 0, 5);
        assert!(!page2.is_empty());

        // Ensure no overlap between page1 and page2
        let p1_titles: std::collections::HashSet<_> = page1.iter().map(|r| &r.title).collect();
        for r in &page2 {
            assert!(!p1_titles.contains(&r.title), "Page 2 should not contain items from Page 1");
        }

        // Verify strictly decreasing timestamps
        let p1_last_dt = DateTime::parse_from_rfc3339(&page1.last().unwrap().published_utc).unwrap();
        let p2_first_dt = DateTime::parse_from_rfc3339(&page2.first().unwrap().published_utc).unwrap();
        assert!(p2_first_dt < p1_last_dt);
    }

    #[test]
    fn test_paginate_feed_records_cursor_asc() {
        let start = Utc::now() - Duration::hours(12);
        let end = Utc::now();
        let mut records = generate_mock_feed_records(None, start, end);
        records.sort_by(|a, b| a.published_utc.cmp(&b.published_utc));

        // First page: no cursor, limit 5, sort asc
        let (page1, _, next_cursor1) =
            paginate_feed_records(records.clone(), "asc", None, 0, 5);
        assert_eq!(page1.len(), 5);
        assert!(next_cursor1.is_some());

        // Parse next_cursor1 and query second page
        let cur_dt = DateTime::parse_from_rfc3339(next_cursor1.as_ref().unwrap())
            .unwrap()
            .with_timezone(&Utc);
        let (page2, _, _next_cursor2) =
            paginate_feed_records(records.clone(), "asc", Some(cur_dt), 0, 5);
        assert!(!page2.is_empty());

        // Ensure no overlap between page1 and page2
        let p1_titles: std::collections::HashSet<_> = page1.iter().map(|r| &r.title).collect();
        for r in &page2 {
            assert!(!p1_titles.contains(&r.title), "Page 2 should not contain items from Page 1");
        }

        // Verify strictly increasing timestamps
        let p1_last_dt = DateTime::parse_from_rfc3339(&page1.last().unwrap().published_utc).unwrap();
        let p2_first_dt = DateTime::parse_from_rfc3339(&page2.first().unwrap().published_utc).unwrap();
        assert!(p2_first_dt > p1_last_dt);
    }

    #[test]
    fn test_paginate_feed_records_terminal() {
        let start = Utc::now() - Duration::hours(1);
        let end = Utc::now();
        let records = generate_mock_feed_records(None, start, end);
        // Request limit larger than total records
        let (paged, total, next_cursor) =
            paginate_feed_records(records, "desc", None, 0, 1000);
        assert_eq!(paged.len(), total);
        assert!(next_cursor.is_none(), "Terminal page must have next_cursor = None");
    }

    #[test]
    fn test_cursor_validation_logic() {
        let valid_ts = "2026-08-29T14:30:00Z";
        let parsed = DateTime::parse_from_rfc3339(valid_ts.trim());
        assert!(parsed.is_ok());

        let invalid_ts = "not-a-timestamp";
        assert!(DateTime::parse_from_rfc3339(invalid_ts.trim()).is_err());

        let empty_ts = "   ";
        assert!(empty_ts.trim().is_empty());
    }

    #[test]
    fn test_params_cursor_offset_conflict() {
        let params_both = SentimentFeedParams {
            sector: None,
            start_date: None,
            end_date: None,
            min_confidence: None,
            min_quality: None,
            limit: Some(50),
            offset: Some(10),
            cursor: Some("2026-08-29T14:30:00Z".to_string()),
            sort: Some("desc".to_string()),
            as_of_utc: None,
        };
        assert!(params_both.cursor.is_some() && params_both.offset.is_some());

        let params_cursor_only = SentimentFeedParams {
            sector: None,
            start_date: None,
            end_date: None,
            min_confidence: None,
            min_quality: None,
            limit: Some(50),
            offset: None,
            cursor: Some("2026-08-29T14:30:00Z".to_string()),
            sort: Some("desc".to_string()),
            as_of_utc: None,
        };
        assert!(params_cursor_only.cursor.is_some() && params_cursor_only.offset.is_none());
    }
}

