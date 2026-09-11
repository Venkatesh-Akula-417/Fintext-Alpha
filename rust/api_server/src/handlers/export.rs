//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Historical CSV Export Streaming Handler
//! ═══════════════════════════════════════════════════════════════════════════════

use crate::auth::AuthErrorResponse;
use crate::storage::{QuestDbClient, QuestDbClientConfig};
use axum::body::Body;
use axum::extract::Query;
use axum::http::header::{CACHE_CONTROL, CONTENT_DISPOSITION, CONTENT_TYPE};
use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse, Json, Response};
use bytes::Bytes;
use chrono::NaiveDate;
use once_cell::sync::Lazy;

use serde::Deserialize;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tracing::{error, info, warn};
use utoipa::ToSchema;

static QUESTDB_CLIENT: Lazy<QuestDbClient> =
    Lazy::new(|| QuestDbClient::new(QuestDbClientConfig::default()));

pub const DEFAULT_CSV_EXPORT_LIMIT: u32 = 10_000;
pub const MAX_CSV_EXPORT_LIMIT: u32 = 100_000;

/// Request parameters for historical CSV dataset export (`GET /export/csv`).
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct ExportCsvParams {
    /// Target stock ticker symbol (e.g., 'AAPL', 'NVDA', 'MSFT')
    #[schema(example = "AAPL")]
    pub ticker: String,
    /// Start date formatted as YYYY-MM-DD
    #[schema(example = "2025-01-01")]
    pub start_date: String,
    /// End date formatted as YYYY-MM-DD
    #[schema(example = "2025-03-31")]
    pub end_date: String,
    /// Maximum number of records to export (default: 10000, max: 100000)
    #[schema(example = 10000)]
    pub limit: Option<u32>,
    /// Export file format (currently only 'csv' is supported)
    #[schema(example = "csv")]
    pub format: Option<String>,
    /// Minimum data quality score filter threshold between 0.0 and 1.0 (default: 0.0)
    #[schema(example = 0.70)]
    pub min_quality: Option<f32>,
}

/// Helper function to escape text for RFC 4180 CSV compliance.
pub fn escape_csv_field(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// Export Historical Financial Sentiment Data as Streamed CSV.
///
/// Streams point-in-time historical sentiment scores, microstructure signals (VPIN, gamma exposure),
/// and news headlines for quantitative research and offline strategy backtesting.
#[utoipa::path(
    get,
    path = "/export/csv",
    tag = "Historical Data Export",
    params(
        ("ticker" = String, Query, description = "Target stock ticker symbol (e.g., 'AAPL', 'NVDA')"),
        ("start_date" = String, Query, description = "Start date in ISO format YYYY-MM-DD (e.g., '2025-01-01')"),
        ("end_date" = String, Query, description = "End date in ISO format YYYY-MM-DD (e.g., '2025-03-31')"),
        ("limit" = Option<u32>, Query, description = "Maximum number of rows to export (default: 10000, max: 100000)"),
        ("format" = Option<String>, Query, description = "Export format (default: 'csv')"),
        ("min_quality" = Option<f32>, Query, description = "Minimum data quality score filter (0.0 to 1.0, default: 0.0)")
    ),
    responses(
        (status = 200, description = "Streamed CSV historical dataset", content_type = "text/csv"),
        (status = 400, description = "Invalid date range or parameter validation error", body = AuthErrorResponse),
        (status = 401, description = "Unauthorized (missing or invalid Bearer JWT)", body = AuthErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::rate_limit::RateLimitErrorResponse)
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn export_csv_handler(Query(params): Query<ExportCsvParams>) -> Response {
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

    // 2. Validate format if specified
    if let Some(ref fmt) = params.format {
        let fmt_lower = fmt.trim().to_lowercase();
        if fmt_lower != "csv" {
            return (
                StatusCode::BAD_REQUEST,
                Json(AuthErrorResponse {
                    error: "Bad Request".to_string(),
                    message: format!(
                        "Unsupported export format '{}'. Currently only 'csv' is supported.",
                        fmt
                    ),
                }),
            )
                .into_response();
        }
    }

    // 3. Parse and validate dates
    let start_date = match NaiveDate::parse_from_str(params.start_date.trim(), "%Y-%m-%d") {
        Ok(d) => d,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(AuthErrorResponse {
                    error: "Bad Request".to_string(),
                    message: format!(
                        "Invalid start_date format '{}', expected YYYY-MM-DD: {}",
                        params.start_date, e
                    ),
                }),
            )
                .into_response();
        }
    };

    let end_date = match NaiveDate::parse_from_str(params.end_date.trim(), "%Y-%m-%d") {
        Ok(d) => d,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(AuthErrorResponse {
                    error: "Bad Request".to_string(),
                    message: format!(
                        "Invalid end_date format '{}', expected YYYY-MM-DD: {}",
                        params.end_date, e
                    ),
                }),
            )
                .into_response();
        }
    };

    if start_date > end_date {
        return (
            StatusCode::BAD_REQUEST,
            Json(AuthErrorResponse {
                error: "Bad Request".to_string(),
                message: "start_date cannot be after end_date".to_string(),
            }),
        )
            .into_response();
    }

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

    // 3b. Point-in-Time (PIT) Validation & Delisting Window Clamping
    let pit_data = crate::pit::GLOBAL_PIT_DATA.clone();
    let (effective_start, effective_end) = if pit_data.is_enabled() {
        if !pit_data.is_valid_ticker(&ticker, start_date) {
            info!("[PIT Export] Ticker '{}' was not active/listed on start_date '{}'. Returning empty CSV.", ticker, start_date);
            let header_row = "published_utc,ticker,source,title,sentiment_score,vpin,gamma_exposure,data_quality_score\r\n";
            let filename = format!(
                "{}_sentiment_history_{}_to_{}.csv",
                ticker, params.start_date, params.end_date
            );
            let headers = [
                (
                    CONTENT_TYPE,
                    HeaderValue::from_static("text/csv; charset=utf-8"),
                ),
                (
                    CONTENT_DISPOSITION,
                    HeaderValue::from_str(&format!("attachment; filename=\"{}\"", filename))
                        .unwrap_or_else(|_| {
                            HeaderValue::from_static("attachment; filename=\"export.csv\"")
                        }),
                ),
                (
                    CACHE_CONTROL,
                    HeaderValue::from_static("no-cache, no-store, must-revalidate"),
                ),
            ];
            return (StatusCode::OK, headers, Body::from(header_row)).into_response();
        }

        match pit_data.clamp_query_range(&ticker, start_date, end_date) {
            Some((s, e)) => (s, e),
            None => {
                let header_row = "published_utc,ticker,source,title,sentiment_score,vpin,gamma_exposure,data_quality_score\r\n";
                let filename = format!(
                    "{}_sentiment_history_{}_to_{}.csv",
                    ticker, params.start_date, params.end_date
                );
                let headers = [
                    (
                        CONTENT_TYPE,
                        HeaderValue::from_static("text/csv; charset=utf-8"),
                    ),
                    (
                        CONTENT_DISPOSITION,
                        HeaderValue::from_str(&format!("attachment; filename=\"{}\"", filename))
                            .unwrap_or_else(|_| {
                                HeaderValue::from_static("attachment; filename=\"export.csv\"")
                            }),
                    ),
                    (
                        CACHE_CONTROL,
                        HeaderValue::from_static("no-cache, no-store, must-revalidate"),
                    ),
                ];
                return (StatusCode::OK, headers, Body::from(header_row)).into_response();
            }
        }
    } else {
        (start_date, end_date)
    };

    let limit = params
        .limit
        .unwrap_or(DEFAULT_CSV_EXPORT_LIMIT)
        .clamp(1, MAX_CSV_EXPORT_LIMIT);

    let start_str = effective_start.format("%Y-%m-%d").to_string();
    let end_str = effective_end.format("%Y-%m-%d").to_string();

    info!(
        "[CSV Export] Initiating stream for ticker='{}', range={} to {}, limit={}, min_quality={}",
        ticker, start_str, end_str, limit, min_quality
    );

    // In production mode, verify QuestDB is reachable before committing to streaming 200 OK
    if crate::state::is_production_mode() {
        let endpoint = format!("{}/exec", QUESTDB_CLIENT.config().url.trim_end_matches('/'));
        let test_sql = "SELECT 1;";
        let is_reachable = match reqwest::Client::new()
            .get(&endpoint)
            .query(&[("query", &test_sql)])
            .send()
            .await
        {
            Ok(resp) => resp.status().is_success(),
            Err(_) => false,
        };
        if !is_reachable {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({
                    "error": "Service Unavailable",
                    "message": "Required data source unavailable in production mode.",
                    "status": "service_unavailable"
                })),
            )
                .into_response();
        }
    }

    // 4. Set up asynchronous channel stream
    let (tx, rx) = mpsc::channel::<Result<Bytes, std::io::Error>>(64);
    let ticker_clone = ticker.clone();
    let start_clone = start_str.clone();
    let end_clone = end_str.clone();

    tokio::spawn(async move {
        // Stream CSV Header Row with data_quality_score
        let header_row = "published_utc,ticker,source,title,sentiment_score,vpin,gamma_exposure,data_quality_score\r\n";
        if tx.send(Ok(Bytes::from(header_row))).await.is_err() {
            return;
        }

        // Check if mock mode is active
        if crate::state::is_questdb_mock_fallback_enabled() {
            let mut curr = effective_start;
            let mut count = 0u32;

            while curr <= effective_end && count < limit {
                let day_offset = (curr - effective_start).num_days() as f64;
                let synthetic_score = (0.25 + (day_offset * 0.17).sin() * 0.65).clamp(-1.0, 1.0);
                let vpin = (0.35 + (day_offset * 0.11).cos() * 0.20).clamp(0.05, 0.95);
                let gamma = (1200000.0 * (day_offset * 0.05).sin()).round();

                let source = if (count % 5) == 0 {
                    "SEC EDGAR"
                } else if (count % 3) == 0 {
                    "Finnhub"
                } else {
                    "Institutional Wire"
                };

                let title = format!("{} Market Sentiment and Flow Analysis Report", ticker_clone);
                let (confidence, _) =
                    crate::models::sentiment::compute_confidence_and_probabilities(
                        synthetic_score,
                        "NEUTRAL",
                        None,
                        None,
                        None,
                    );

                let quality_score =
                    crate::quality::compute_data_quality_score(source, &title, confidence);

                if quality_score >= min_quality {
                    let row = format!(
                        "{}T14:30:00.000000Z,{},{},{},{:.4},{:.4},{:.0},{:.4}\r\n",
                        curr.format("%Y-%m-%d"),
                        ticker_clone,
                        source,
                        escape_csv_field(&title),
                        synthetic_score,
                        vpin,
                        gamma,
                        quality_score
                    );

                    if tx.send(Ok(Bytes::from(row))).await.is_err() {
                        break;
                    }
                    count += 1;
                }

                curr += chrono::Duration::days(1);
            }
            return;
        }

        // Live QuestDB Execution
        let sql = match QuestDbClient::build_csv_export_query(
            &ticker_clone,
            &start_clone,
            &end_clone,
            limit,
        ) {
            Ok(s) => s,
            Err(e) => {
                error!("[CSV Export] SQL build failed: {}", e);
                return;
            }
        };

        let endpoint = format!("{}/exec", QUESTDB_CLIENT.config().url.trim_end_matches('/'));

        match reqwest::Client::new()
            .get(&endpoint)
            .query(&[("query", &sql)])
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                if let Ok(val) = resp.json::<serde_json::Value>().await {
                    if let Ok(records) = QuestDbClient::parse_sentiment_history_exec_response(&val)
                    {
                        for record in records {
                            if record.data_quality_score >= min_quality {
                                let row = format!(
                                    "{},{},{},{},{:.4},{:.4},{:.0},{:.4}\r\n",
                                    record.published_utc,
                                    record.ticker,
                                    record.source,
                                    escape_csv_field(&record.title),
                                    record.sentiment_score,
                                    record.vpin,
                                    record.gamma_exposure,
                                    record.data_quality_score
                                );
                                if tx.send(Ok(Bytes::from(row))).await.is_err() {
                                    break;
                                }
                            }
                        }
                    }
                }
            }
            Ok(resp) => {
                warn!(
                    "[CSV Export] QuestDB /exec returned status {}: skipping to empty body",
                    resp.status()
                );
            }
            Err(e) => {
                warn!("[CSV Export] Failed to connect to QuestDB /exec: {}", e);
            }
        }
    });

    let filename = format!("sentiment_{}_{}_{}.csv", ticker, start_str, end_str);
    let disposition = format!("attachment; filename=\"{}\"", filename);

    let body = Body::from_stream(ReceiverStream::new(rx));

    let mut response = Response::builder()
        .status(StatusCode::OK)
        .body(body)
        .unwrap_or_else(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to construct response",
            )
                .into_response()
        });

    let headers = response.headers_mut();
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_static("text/csv; charset=utf-8"),
    );
    headers.insert(
        CONTENT_DISPOSITION,
        HeaderValue::from_str(&disposition).unwrap_or_else(|_| {
            HeaderValue::from_static("attachment; filename=\"sentiment_export.csv\"")
        }),
    );
    headers.insert(
        CACHE_CONTROL,
        HeaderValue::from_static("no-cache, no-store, must-revalidate"),
    );

    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_csv_field_escaping() {
        assert_eq!(escape_csv_field("AAPL"), "AAPL");
        assert_eq!(
            escape_csv_field("Apple, Inc. reports earnings"),
            "\"Apple, Inc. reports earnings\""
        );
        assert_eq!(
            escape_csv_field("CEO says \"Growth is accelerating\""),
            "\"CEO says \"\"Growth is accelerating\"\"\""
        );
        assert_eq!(escape_csv_field("Line 1\nLine 2"), "\"Line 1\nLine 2\"");
    }
}
