//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Historical Parquet Dataset Export Streaming Handler
//! ═══════════════════════════════════════════════════════════════════════════════
//!
//! Provides ultra-high-throughput streaming of historical financial sentiment data
//! and joined daily closing prices in Apache Parquet columnar format.
//! ═══════════════════════════════════════════════════════════════════════════════

use crate::auth::AuthErrorResponse;
use crate::models::export::ExportParquetParams;
use crate::storage::{QuestDbClient, QuestDbClientConfig};
use arrow::array::{Float64Builder, RecordBatch, StringBuilder, TimestampMicrosecondBuilder};
use arrow::datatypes::{DataType, Field, Schema, TimeUnit};
use axum::body::Body;
use axum::extract::Query;
use axum::http::header::{CACHE_CONTROL, CONTENT_DISPOSITION, CONTENT_TYPE};
use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse, Json, Response};
use chrono::{DateTime, NaiveDate, Utc};
use once_cell::sync::Lazy;
use parquet::arrow::ArrowWriter;
use parquet::file::properties::WriterProperties;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{error, info, warn};

static QUESTDB_CLIENT: Lazy<QuestDbClient> =
    Lazy::new(|| QuestDbClient::new(QuestDbClientConfig::default()));

pub const DEFAULT_PARQUET_EXPORT_LIMIT: u32 = 10_000;
pub const MAX_PARQUET_EXPORT_LIMIT: u32 = 100_000;

/// Internal sentiment row representation for Parquet batch encoding.
#[derive(Debug, Clone)]
pub struct SentimentParquetRow {
    pub published_micros: i64,
    pub ticker: String,
    pub source: String,
    pub title: String,
    pub sentiment_score: f64,
    pub vpin: f64,
    pub gamma_exposure: f64,
    pub data_quality_score: f64,
    pub close_price: Option<f64>,
}

/// Constructs the canonical Apache Arrow Schema for FinText historical dataset exports.
pub fn create_export_arrow_schema() -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new(
            "published_utc",
            DataType::Timestamp(TimeUnit::Microsecond, Some("+00:00".into())),
            false,
        ),
        Field::new("ticker", DataType::Utf8, false),
        Field::new("source", DataType::Utf8, false),
        Field::new("title", DataType::Utf8, false),
        Field::new("sentiment_score", DataType::Float64, false),
        Field::new("vpin", DataType::Float64, false),
        Field::new("gamma_exposure", DataType::Float64, false),
        Field::new("data_quality_score", DataType::Float64, false),
        Field::new("close_price", DataType::Float64, true),
    ]))
}

/// Converts an array of `SentimentParquetRow` into a serialized Apache Parquet binary byte buffer.
pub fn encode_rows_to_parquet(rows: &[SentimentParquetRow]) -> Result<Vec<u8>, String> {
    let schema = create_export_arrow_schema();
    let num_rows = rows.len();

    let mut published_builder = TimestampMicrosecondBuilder::with_capacity(num_rows);
    let mut ticker_builder = StringBuilder::with_capacity(num_rows, num_rows * 6);
    let mut source_builder = StringBuilder::with_capacity(num_rows, num_rows * 16);
    let mut title_builder = StringBuilder::with_capacity(num_rows, num_rows * 48);
    let mut sentiment_builder = Float64Builder::with_capacity(num_rows);
    let mut vpin_builder = Float64Builder::with_capacity(num_rows);
    let mut gamma_builder = Float64Builder::with_capacity(num_rows);
    let mut quality_builder = Float64Builder::with_capacity(num_rows);
    let mut close_price_builder = Float64Builder::with_capacity(num_rows);

    for row in rows {
        published_builder.append_value(row.published_micros);
        ticker_builder.append_value(&row.ticker);
        source_builder.append_value(&row.source);
        title_builder.append_value(&row.title);
        sentiment_builder.append_value(row.sentiment_score);
        vpin_builder.append_value(row.vpin);
        gamma_builder.append_value(row.gamma_exposure);
        quality_builder.append_value(row.data_quality_score);

        if let Some(price) = row.close_price {
            close_price_builder.append_value(price);
        } else {
            close_price_builder.append_null();
        }
    }

    let batch = RecordBatch::try_new(
        schema.clone(),
        vec![
            Arc::new(published_builder.finish().with_timezone("+00:00")),
            Arc::new(ticker_builder.finish()),
            Arc::new(source_builder.finish()),
            Arc::new(title_builder.finish()),
            Arc::new(sentiment_builder.finish()),
            Arc::new(vpin_builder.finish()),
            Arc::new(gamma_builder.finish()),
            Arc::new(quality_builder.finish()),
            Arc::new(close_price_builder.finish()),
        ],
    )
    .map_err(|e| format!("Failed to build Arrow RecordBatch: {}", e))?;

    let mut buffer = Vec::new();
    let props = WriterProperties::builder().build();
    let mut writer = ArrowWriter::try_new(&mut buffer, schema, Some(props))
        .map_err(|e| format!("Failed to create Parquet ArrowWriter: {}", e))?;

    writer
        .write(&batch)
        .map_err(|e| format!("Failed to write batch to Parquet: {}", e))?;

    writer
        .close()
        .map_err(|e| format!("Failed to close Parquet writer: {}", e))?;

    Ok(buffer)
}

/// Helper to parse ISO / RFC3339 timestamp into microseconds since Unix epoch.
pub fn parse_iso_to_micros(iso: &str) -> i64 {
    if let Ok(dt) = DateTime::parse_from_rfc3339(iso) {
        return dt.timestamp_micros();
    }
    if let Ok(ndt) = chrono::NaiveDateTime::parse_from_str(iso, "%Y-%m-%dT%H:%M:%S%.fZ") {
        return ndt.and_utc().timestamp_micros();
    }
    if let Ok(d) = NaiveDate::parse_from_str(iso, "%Y-%m-%d") {
        return d
            .and_hms_opt(14, 30, 0)
            .unwrap()
            .and_utc()
            .timestamp_micros();
    }
    Utc::now().timestamp_micros()
}

/// Export Historical Financial Sentiment & Prices as Streamed Apache Parquet.
///
/// Streams point-in-time historical sentiment scores, microstructure signals (VPIN, gamma exposure),
/// data quality reliability metrics, and joined daily closing prices in Apache Parquet format.
#[utoipa::path(
    get,
    path = "/export/parquet",
    tag = "Historical Data Export",
    params(
        ("ticker" = String, Query, description = "Target stock ticker symbol (e.g., 'AAPL', 'NVDA')"),
        ("start_date" = String, Query, description = "Start date in ISO format YYYY-MM-DD (e.g., '2025-01-01')"),
        ("end_date" = String, Query, description = "End date in ISO format YYYY-MM-DD (e.g., '2025-03-31')"),
        ("limit" = Option<u32>, Query, description = "Maximum number of rows to export (default: 10000, max: 100000)"),
        ("min_quality" = Option<f32>, Query, description = "Minimum data quality score filter (0.0 to 1.0, default: 0.0)"),
        ("include_prices" = Option<bool>, Query, description = "Whether to left-join daily closing stock prices on date (default: false)")
    ),
    responses(
        (status = 200, description = "Streamed Apache Parquet dataset binary file", content_type = "application/octet-stream"),
        (status = 400, description = "Invalid date range or parameter validation error", body = AuthErrorResponse),
        (status = 401, description = "Unauthorized (missing or invalid Bearer JWT / API Key)", body = AuthErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::rate_limit::RateLimitErrorResponse)
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn export_parquet_handler(Query(params): Query<ExportParquetParams>) -> Response {
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

    // 2. Parse and validate dates
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

    let include_prices = params.include_prices.unwrap_or(false);

    // 3. Point-in-Time (PIT) Validation & Delisting Window Clamping
    let pit_data = crate::pit::GLOBAL_PIT_DATA.clone();
    let (effective_start, effective_end) = if pit_data.is_enabled() {
        if !pit_data.is_valid_ticker(&ticker, start_date) {
            info!("[PIT Parquet Export] Ticker '{}' was not active/listed on start_date '{}'. Returning empty Parquet.", ticker, start_date);
            let empty_parquet = encode_rows_to_parquet(&[]).unwrap_or_default();
            let filename = format!(
                "sentiment_{}_{}_{}.parquet",
                ticker, params.start_date, params.end_date
            );
            let headers = [
                (
                    CONTENT_TYPE,
                    HeaderValue::from_static("application/octet-stream"),
                ),
                (
                    CONTENT_DISPOSITION,
                    HeaderValue::from_str(&format!("attachment; filename=\"{}\"", filename))
                        .unwrap_or_else(|_| {
                            HeaderValue::from_static("attachment; filename=\"export.parquet\"")
                        }),
                ),
                (
                    CACHE_CONTROL,
                    HeaderValue::from_static("no-cache, no-store, must-revalidate"),
                ),
            ];
            return (StatusCode::OK, headers, Body::from(empty_parquet)).into_response();
        }

        match pit_data.clamp_query_range(&ticker, start_date, end_date) {
            Some((s, e)) => (s, e),
            None => {
                let empty_parquet = encode_rows_to_parquet(&[]).unwrap_or_default();
                let filename = format!(
                    "sentiment_{}_{}_{}.parquet",
                    ticker, params.start_date, params.end_date
                );
                let headers = [
                    (
                        CONTENT_TYPE,
                        HeaderValue::from_static("application/octet-stream"),
                    ),
                    (
                        CONTENT_DISPOSITION,
                        HeaderValue::from_str(&format!("attachment; filename=\"{}\"", filename))
                            .unwrap_or_else(|_| {
                                HeaderValue::from_static("attachment; filename=\"export.parquet\"")
                            }),
                    ),
                    (
                        CACHE_CONTROL,
                        HeaderValue::from_static("no-cache, no-store, must-revalidate"),
                    ),
                ];
                return (StatusCode::OK, headers, Body::from(empty_parquet)).into_response();
            }
        }
    } else {
        (start_date, end_date)
    };

    let limit = params
        .limit
        .unwrap_or(DEFAULT_PARQUET_EXPORT_LIMIT)
        .clamp(1, MAX_PARQUET_EXPORT_LIMIT);

    let start_str = effective_start.format("%Y-%m-%d").to_string();
    let end_str = effective_end.format("%Y-%m-%d").to_string();

    info!(
        "[Parquet Export] Processing request for ticker='{}', range={} to {}, limit={}, min_quality={}, include_prices={}",
        ticker, start_str, end_str, limit, min_quality, include_prices
    );

    let mut rows: Vec<SentimentParquetRow> = Vec::new();

    // 4. Data Fetching (Mock Fallback vs Live QuestDB)
    if crate::state::is_questdb_mock_fallback_enabled() {
        let mut curr = effective_start;
        let mut count = 0u32;

        let base_price = match ticker.as_str() {
            "AAPL" => 224.50,
            "NVDA" => 128.20,
            "MSFT" => 418.00,
            "AMZN" => 184.30,
            "GOOGL" => 172.10,
            "META" => 512.40,
            "TSLA" => 248.80,
            "PYPL" => 68.50,
            _ => 150.00,
        };

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

            let title = format!("{} Market Sentiment and Flow Analysis Report", ticker);
            let (confidence, _) = crate::models::sentiment::compute_confidence_and_probabilities(
                synthetic_score,
                "NEUTRAL",
                None,
                None,
                None,
            );

            let quality_score =
                crate::quality::compute_data_quality_score(source, &title, confidence);

            if quality_score >= min_quality {
                let close_price = if include_prices {
                    Some(base_price + (day_offset * 0.15).sin() * 12.0)
                } else {
                    None
                };

                let micros = curr
                    .and_hms_opt(14, 30, 0)
                    .unwrap()
                    .and_utc()
                    .timestamp_micros();

                rows.push(SentimentParquetRow {
                    published_micros: micros,
                    ticker: ticker.clone(),
                    source: source.to_string(),
                    title,
                    sentiment_score: (synthetic_score * 10000.0).round() / 10000.0,
                    vpin: (vpin * 10000.0).round() / 10000.0,
                    gamma_exposure: gamma,
                    data_quality_score: ((quality_score as f64) * 10000.0).round() / 10000.0,
                    close_price,
                });
                count += 1;
            }

            curr += chrono::Duration::days(1);
        }
    } else {
        // Live QuestDB Execution
        // Optionally fetch price map if include_prices
        let mut price_map: HashMap<NaiveDate, f64> = HashMap::new();
        if include_prices {
            if let Ok(prices_sql) =
                QuestDbClient::build_stock_prices_query(&[ticker.clone()], &start_str, &end_str)
            {
                let endpoint =
                    format!("{}/exec", QUESTDB_CLIENT.config().url.trim_end_matches('/'));
                if let Ok(resp) = reqwest::Client::new()
                    .get(&endpoint)
                    .query(&[("query", &prices_sql)])
                    .send()
                    .await
                {
                    if resp.status().is_success() {
                        if let Ok(val) = resp.json::<serde_json::Value>().await {
                            if let Ok(parsed) = QuestDbClient::parse_stock_prices_dataset(&val) {
                                if let Some(t_map) = parsed.get(&ticker) {
                                    price_map = t_map.clone();
                                }
                            }
                        }
                    }
                }
            }
        }

        let sql = match QuestDbClient::build_csv_export_query(&ticker, &start_str, &end_str, limit)
        {
            Ok(s) => s,
            Err(e) => {
                error!("[Parquet Export] SQL build failed: {}", e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(AuthErrorResponse {
                        error: "Internal Server Error".to_string(),
                        message: "Failed to construct export query".to_string(),
                    }),
                )
                    .into_response();
            }
        };

        let endpoint = format!("{}/exec", QUESTDB_CLIENT.config().url.trim_end_matches('/'));
        if let Ok(resp) = reqwest::Client::new()
            .get(&endpoint)
            .query(&[("query", &sql)])
            .send()
            .await
        {
            if resp.status().is_success() {
                if let Ok(val) = resp.json::<serde_json::Value>().await {
                    if let Ok(records) = QuestDbClient::parse_sentiment_history_exec_response(&val)
                    {
                        for record in records {
                            if record.data_quality_score >= min_quality {
                                let micros = parse_iso_to_micros(&record.published_utc);
                                let close_price = if include_prices {
                                    let rec_date = NaiveDate::parse_from_str(
                                        if record.published_utc.len() >= 10 {
                                            &record.published_utc[0..10]
                                        } else {
                                            &record.published_utc
                                        },
                                        "%Y-%m-%d",
                                    )
                                    .ok();
                                    rec_date.and_then(|d| price_map.get(&d).copied())
                                } else {
                                    None
                                };

                                rows.push(SentimentParquetRow {
                                    published_micros: micros,
                                    ticker: record.ticker,
                                    source: record.source,
                                    title: record.title,
                                    sentiment_score: record.sentiment_score,
                                    vpin: record.vpin,
                                    gamma_exposure: record.gamma_exposure,
                                    data_quality_score: record.data_quality_score as f64,
                                    close_price,
                                });
                            }
                        }
                    }
                }
            } else {
                warn!(
                    "[Parquet Export] QuestDB /exec returned status {}",
                    resp.status()
                );
            }
        }
    }

    if rows.is_empty() && crate::state::is_production_mode() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(AuthErrorResponse {
                error: "Service Unavailable".to_string(),
                message: "Required data source unavailable in production mode.".to_string(),
            }),
        )
            .into_response();
    }

    // 5. Serialize rows to Parquet buffer
    let parquet_bytes = match encode_rows_to_parquet(&rows) {
        Ok(b) => b,
        Err(e) => {
            error!("[Parquet Export] Parquet serialization failed: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AuthErrorResponse {
                    error: "Serialization Error".to_string(),
                    message: format!("Failed to encode Apache Parquet buffer: {}", e),
                }),
            )
                .into_response();
        }
    };

    let filename = format!(
        "sentiment_{}_{}_{}.parquet",
        ticker, params.start_date, params.end_date
    );
    let headers = [
        (
            CONTENT_TYPE,
            HeaderValue::from_static("application/octet-stream"),
        ),
        (
            CONTENT_DISPOSITION,
            HeaderValue::from_str(&format!("attachment; filename=\"{}\"", filename))
                .unwrap_or_else(|_| {
                    HeaderValue::from_static("attachment; filename=\"export.parquet\"")
                }),
        ),
        (
            CACHE_CONTROL,
            HeaderValue::from_static("no-cache, no-store, must-revalidate"),
        ),
    ];

    (StatusCode::OK, headers, Body::from(parquet_bytes)).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parquet_schema_and_encoding() {
        let rows = vec![
            SentimentParquetRow {
                published_micros: 1735689600000000, // 2025-01-01T00:00:00Z
                ticker: "AAPL".to_string(),
                source: "SEC EDGAR".to_string(),
                title: "Apple Inc. Form 10-K Annual Report".to_string(),
                sentiment_score: 0.85,
                vpin: 0.22,
                gamma_exposure: 4500000.0,
                data_quality_score: 0.95,
                close_price: Some(230.50),
            },
            SentimentParquetRow {
                published_micros: 1735776000000000,
                ticker: "AAPL".to_string(),
                source: "Finnhub".to_string(),
                title: "Analysts Upgrade Apple Price Targets".to_string(),
                sentiment_score: 0.72,
                vpin: 0.31,
                gamma_exposure: 3800000.0,
                data_quality_score: 0.88,
                close_price: None,
            },
        ];

        let bytes = encode_rows_to_parquet(&rows).expect("Parquet encoding should succeed");
        assert!(bytes.len() > 12);
        // Verify Parquet magic bytes at start and end: "PAR1"
        assert_eq!(&bytes[0..4], b"PAR1");
        assert_eq!(&bytes[bytes.len() - 4..], b"PAR1");
    }

    #[test]
    fn test_empty_parquet_generation() {
        let bytes = encode_rows_to_parquet(&[]).expect("Empty parquet encoding should succeed");
        assert!(bytes.len() > 12);
        assert_eq!(&bytes[0..4], b"PAR1");
        assert_eq!(&bytes[bytes.len() - 4..], b"PAR1");
    }
}
