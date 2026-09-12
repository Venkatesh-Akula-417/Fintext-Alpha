//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Statistical Sentiment Anomaly Detection Handler
//! ═══════════════════════════════════════════════════════════════════════════════
//!
//! Scans historical point-in-time sentiment time series across universe or sector
//! constituent equities, computes rolling baseline distribution parameters (mean, stddev),
//! and flags statistically significant deviations (z-scores) indicating abnormal news sentiment.
//! ═══════════════════════════════════════════════════════════════════════════════

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use chrono::{DateTime, Duration, Utc};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use tracing::{error, info, warn};

use crate::auth::AuthErrorResponse;
use crate::models::{SentimentAnomaliesParams, SentimentAnomaliesResponse, SentimentAnomalyItem};
use crate::sector::GLOBAL_SECTOR_MAP;
use crate::state::AppState;
use crate::storage::{QuestDbClient, QuestDbClientConfig};

static QUESTDB_CLIENT: Lazy<QuestDbClient> =
    Lazy::new(|| QuestDbClient::new(QuestDbClientConfig::default()));

pub const DEFAULT_LOOKBACK_DAYS: u32 = 30;
pub const MAX_LOOKBACK_DAYS: u32 = 90;
pub const MIN_LOOKBACK_DAYS: u32 = 1;

pub const DEFAULT_ZSCORE_THRESHOLD: f64 = 2.0;
pub const MIN_ZSCORE_THRESHOLD: f64 = 1.0;
pub const MAX_ZSCORE_THRESHOLD: f64 = 5.0;

pub const DEFAULT_MIN_RECORDS: usize = 20;
pub const MAX_MIN_RECORDS: usize = 1000;

pub const DEFAULT_ANOMALY_LIMIT: usize = 20;
pub const MAX_ANOMALY_LIMIT: usize = 100;

/// Generate deterministic mock historical sentiment observations for anomaly detection.
pub fn generate_mock_anomaly_data(
    target_tickers: &[String],
    lookback_days: u32,
    now: DateTime<Utc>,
) -> Vec<(String, f64, String)> {
    let mut data = Vec::new();
    let num_points_per_ticker = (lookback_days as usize * 2).max(25);

    for (t_idx, ticker) in target_tickers.iter().enumerate() {
        // Base mean and variance deterministic per ticker
        let base_mean = ((t_idx as f64 * 0.8 + 0.1).sin() * 0.3).clamp(-0.4, 0.4);
        let base_std = 0.12 + ((t_idx as f64 * 1.3).cos().abs() * 0.08);

        for p_idx in 0..num_points_per_ticker {
            let offset_hours =
                (lookback_days as i64 * 24 * p_idx as i64) / num_points_per_ticker as i64;
            let ts = now - Duration::hours(offset_hours);

            let score = if p_idx == 0 {
                // Latest score: inject intentional anomalies for specific tickers
                if t_idx % 4 == 0 {
                    // Bullish anomaly: +3.2 std deviations above mean
                    (base_mean + base_std * 3.2).clamp(-1.0, 1.0)
                } else if t_idx % 4 == 1 {
                    // Bearish anomaly: -2.8 std deviations below mean
                    (base_mean - base_std * 2.8).clamp(-1.0, 1.0)
                } else {
                    // Normal score within 1 std dev
                    (base_mean + ((p_idx as f64 * 0.7).sin() * base_std * 0.8)).clamp(-1.0, 1.0)
                }
            } else {
                // Historical baseline score with normal noise
                (base_mean + ((p_idx as f64 * 1.1 + t_idx as f64).sin() * base_std * 1.1))
                    .clamp(-1.0, 1.0)
            };

            data.push((
                ticker.clone(),
                (score * 10000.0).round() / 10000.0,
                ts.to_rfc3339(),
            ));
        }
    }

    data
}

/// Compute statistical z-scores and extract anomalies from grouped ticker time-series.
pub fn compute_anomalies(
    records: Vec<(String, f64, String)>,
    min_records: usize,
    zscore_threshold: f64,
    limit: usize,
) -> (Vec<SentimentAnomalyItem>, usize, usize) {
    // Group records by ticker: ticker -> Vec<(score, timestamp)>
    let mut ticker_map: HashMap<String, Vec<(f64, String)>> = HashMap::new();
    for (ticker, score, ts) in records {
        ticker_map.entry(ticker).or_default().push((score, ts));
    }

    let scanned_tickers = ticker_map.len();
    let mut anomalies = Vec::new();

    for (ticker, mut series) in ticker_map {
        if series.len() < min_records {
            continue;
        }

        // Sort series descending by timestamp (latest first)
        series.sort_by(|a, b| b.1.cmp(&a.1));

        let record_count = series.len();
        let (latest_score, latest_timestamp) = series[0].clone();

        // Calculate sample mean
        let sum: f64 = series.iter().map(|(s, _)| *s).sum();
        let mean = sum / record_count as f64;

        // Calculate sample variance and standard deviation
        let variance_sum: f64 = series.iter().map(|(s, _)| (*s - mean).powi(2)).sum();
        let sample_variance = variance_sum / (record_count - 1) as f64;
        let stddev = sample_variance.sqrt();

        if stddev < 1e-6 {
            continue;
        }

        let zscore = (latest_score - mean) / stddev;

        if zscore.abs() >= zscore_threshold {
            let direction = if zscore > 0.0 {
                "bullish".to_string()
            } else {
                "bearish".to_string()
            };

            anomalies.push(SentimentAnomalyItem {
                ticker,
                latest_score: (latest_score * 10000.0).round() / 10000.0,
                mean_score: (mean * 10000.0).round() / 10000.0,
                stddev: (stddev * 10000.0).round() / 10000.0,
                zscore: (zscore * 10000.0).round() / 10000.0,
                direction,
                latest_timestamp,
                record_count,
                model_version: Some(crate::models::DEFAULT_MODEL_VERSION.to_string()),
                pipeline_version: Some(crate::models::DEFAULT_PIPELINE_VERSION.to_string()),
                data_provenance: Some(
                    crate::models::DEFAULT_DATA_PROVENANCE
                        .iter()
                        .map(|s| s.to_string())
                        .collect(),
                ),
            });
        }
    }

    let total_anomalies_detected = anomalies.len();

    // Sort ranked by absolute z-score descending
    anomalies.sort_by(|a, b| {
        b.zscore
            .abs()
            .partial_cmp(&a.zscore.abs())
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    anomalies.truncate(limit);

    (anomalies, total_anomalies_detected, scanned_tickers)
}

/// Detect Sentiment Anomalies across Universe or Sector.
///
/// Scans historical point-in-time sentiment time series over a configurable lookback window,
/// computes sample mean and standard deviation baselines, and flags statistically significant
/// sentiment score movements (|z-score| >= threshold).
#[utoipa::path(
    get,
    path = "/sentiment/anomalies",
    tag = "Sentiment Analysis",
    params(
        ("sector" = Option<String>, Query, description = "Optional GICS Sector filter (e.g., 'Technology', 'Financials', 'Healthcare')"),
        ("lookback_days" = Option<u32>, Query, description = "Number of calendar days to compute baseline statistics (default: 30, min: 1, max: 90)"),
        ("zscore_threshold" = Option<f64>, Query, description = "Minimum absolute z-score required to flag anomaly (default: 2.0, min: 1.0, max: 5.0)"),
        ("min_records" = Option<usize>, Query, description = "Minimum historical sentiment records required per ticker (default: 20, min: 1, max: 1000)"),
        ("limit" = Option<usize>, Query, description = "Maximum number of anomaly items to return (default: 20, min: 1, max: 100)")
    ),
    responses(
        (status = 200, description = "Sentiment anomalies detected successfully", body = SentimentAnomaliesResponse),
        (status = 400, description = "Parameter validation error (out-of-range bounds)", body = AuthErrorResponse),
        (status = 401, description = "Unauthorized (missing or invalid Bearer JWT or API key)", body = AuthErrorResponse),
        (status = 404, description = "Specified sector not found in GICS mapping", body = AuthErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::rate_limit::RateLimitErrorResponse)
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn get_sentiment_anomalies_handler(
    State(state): State<AppState>,
    Query(params): Query<SentimentAnomaliesParams>,
) -> Response {
    let model_version = Some(state.get_model_version());
    let pipeline_version = Some(state.get_pipeline_version());
    let data_provenance = Some(state.get_data_provenance());

    // 1. Resolve Sector Filter
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

    // 2. Validate Parameters
    let lookback_days = match params.lookback_days {
        Some(d) if d < MIN_LOOKBACK_DAYS || d > MAX_LOOKBACK_DAYS => {
            return (
                StatusCode::BAD_REQUEST,
                Json(AuthErrorResponse {
                    error: "Bad Request".to_string(),
                    message: format!(
                        "lookback_days must be between {} and {}, got {}",
                        MIN_LOOKBACK_DAYS, MAX_LOOKBACK_DAYS, d
                    ),
                }),
            )
                .into_response();
        }
        Some(d) => d,
        None => DEFAULT_LOOKBACK_DAYS,
    };

    let zscore_threshold = match params.zscore_threshold {
        Some(z) if z < MIN_ZSCORE_THRESHOLD || z > MAX_ZSCORE_THRESHOLD => {
            return (
                StatusCode::BAD_REQUEST,
                Json(AuthErrorResponse {
                    error: "Bad Request".to_string(),
                    message: format!(
                        "zscore_threshold must be between {:.1} and {:.1}, got {:.2}",
                        MIN_ZSCORE_THRESHOLD, MAX_ZSCORE_THRESHOLD, z
                    ),
                }),
            )
                .into_response();
        }
        Some(z) => z,
        None => DEFAULT_ZSCORE_THRESHOLD,
    };

    let min_records = match params.min_records {
        Some(m) if m == 0 || m > MAX_MIN_RECORDS => {
            return (
                StatusCode::BAD_REQUEST,
                Json(AuthErrorResponse {
                    error: "Bad Request".to_string(),
                    message: format!(
                        "min_records must be between 1 and {}, got {}",
                        MAX_MIN_RECORDS, m
                    ),
                }),
            )
                .into_response();
        }
        Some(m) => m,
        None => DEFAULT_MIN_RECORDS,
    };

    let limit = match params.limit {
        Some(l) if l == 0 || l > MAX_ANOMALY_LIMIT => {
            return (
                StatusCode::BAD_REQUEST,
                Json(AuthErrorResponse {
                    error: "Bad Request".to_string(),
                    message: format!(
                        "limit must be between 1 and {}, got {}",
                        MAX_ANOMALY_LIMIT, l
                    ),
                }),
            )
                .into_response();
        }
        Some(l) => l,
        None => DEFAULT_ANOMALY_LIMIT,
    };

    // 2b. Validate as_of_utc if present
    if let Some(ref as_of_str) = params.as_of_utc {
        if !as_of_str.trim().is_empty() {
            if chrono::DateTime::parse_from_rfc3339(as_of_str.trim()).is_err() {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(AuthErrorResponse {
                        error: "Bad Request".to_string(),
                        message: format!(
                            "Invalid as_of_utc format '{}', expected RFC3339",
                            as_of_str
                        ),
                    }),
                )
                    .into_response();
            }
        }
    }

    let now = Utc::now();
    let start_dt = now - Duration::days(lookback_days as i64);
    let start_iso = start_dt.to_rfc3339();

    // 3. Fallback / Mock Mode Check
    let force_mock = crate::state::is_questdb_mock_fallback_enabled();

    let default_universe = vec![
        "AAPL".to_string(),
        "NVDA".to_string(),
        "MSFT".to_string(),
        "AMZN".to_string(),
        "GOOGL".to_string(),
        "META".to_string(),
        "TSLA".to_string(),
        "JPM".to_string(),
        "GS".to_string(),
        "JNJ".to_string(),
        "PFE".to_string(),
        "XOM".to_string(),
        "CVX".to_string(),
    ];

    let target_tickers = constituent_tickers.unwrap_or(default_universe);

    if force_mock {
        let mock_records = generate_mock_anomaly_data(&target_tickers, lookback_days, now);
        let (items, total_anomalies_detected, scanned_tickers) =
            compute_anomalies(mock_records, min_records, zscore_threshold, limit);

        let count = items.len();
        return (
            StatusCode::OK,
            Json(SentimentAnomaliesResponse {
                count,
                total_anomalies_detected,
                lookback_days,
                zscore_threshold,
                min_records,
                sector: sector_filter,
                scanned_tickers,
                items,
                generated_at: now.to_rfc3339(),
                message: format!(
                    "Sentiment anomaly detection completed: {} anomalies detected across {} tickers",
                    total_anomalies_detected, scanned_tickers
                ),
                model_version,
                pipeline_version,
                data_provenance,
            }),
        )
            .into_response();
    }

    // 4. Query QuestDB Live Instance
    let sql =
        match QuestDbClient::build_sentiment_anomalies_query(Some(&target_tickers), &start_iso) {
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

    info!("[Sentiment Anomalies SQL] Executing: {}", sql);

    let endpoint = format!("{}/exec", QUESTDB_CLIENT.config().url.trim_end_matches('/'));
    let client = reqwest::Client::new();

    match client.get(&endpoint).query(&[("query", &sql)]).send().await {
        Ok(resp) if resp.status().is_success() => match resp.json::<serde_json::Value>().await {
            Ok(val) => match QuestDbClient::parse_sentiment_anomalies_exec_response(&val) {
                Ok(records) => {
                    let (items, total_anomalies_detected, scanned_tickers) =
                        compute_anomalies(records, min_records, zscore_threshold, limit);
                    let count = items.len();

                    (
                            StatusCode::OK,
                            Json(SentimentAnomaliesResponse {
                                count,
                                total_anomalies_detected,
                                lookback_days,
                                zscore_threshold,
                                min_records,
                                sector: sector_filter,
                                scanned_tickers,
                                items,
                                generated_at: now.to_rfc3339(),
                                message: format!(
                                    "Sentiment anomaly detection completed: {} anomalies detected across {} tickers",
                                    total_anomalies_detected, scanned_tickers
                                ),
                                model_version,
                                pipeline_version,
                                data_provenance,
                            }),
                        )
                            .into_response()
                }
                Err(e) => {
                    error!("Failed to parse QuestDB anomaly records: {}", e);
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
        _ => {
            if crate::state::is_production_mode() {
                warn!("QuestDB unreachable in production mode for sentiment anomalies");
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
            warn!("QuestDB unreachable or returned error, falling back to mock anomaly generator");
            let mock_records = generate_mock_anomaly_data(&target_tickers, lookback_days, now);
            let (items, total_anomalies_detected, scanned_tickers) =
                compute_anomalies(mock_records, min_records, zscore_threshold, limit);

            let count = items.len();
            (
                StatusCode::OK,
                Json(SentimentAnomaliesResponse {
                    count,
                    total_anomalies_detected,
                    lookback_days,
                    zscore_threshold,
                    min_records,
                    sector: sector_filter,
                    scanned_tickers,
                    items,
                    generated_at: now.to_rfc3339(),
                    message: format!(
                        "Sentiment anomaly detection completed: {} anomalies detected across {} tickers",
                        total_anomalies_detected, scanned_tickers
                    ),
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
    fn test_compute_anomalies_detection() {
        let now = Utc::now();
        let mut records = Vec::new();

        // Ticker AAPL: baseline mean ~0.1, std ~0.1, latest score = 0.85 -> high positive z-score (~+7.5)
        for i in 0..30 {
            let ts = (now - Duration::hours(i)).to_rfc3339();
            let score = if i == 0 {
                0.85
            } else {
                0.10 + (i as f64 * 0.002)
            };
            records.push(("AAPL".to_string(), score, ts));
        }

        // Ticker MSFT: baseline mean ~0.2, std ~0.1, latest score = -0.60 -> high negative z-score (~-8.0)
        for i in 0..30 {
            let ts = (now - Duration::hours(i)).to_rfc3339();
            let score = if i == 0 {
                -0.60
            } else {
                0.20 - (i as f64 * 0.002)
            };
            records.push(("MSFT".to_string(), score, ts));
        }

        // Ticker NVDA: normal scores within 1 std dev -> no anomaly
        for i in 0..30 {
            let ts = (now - Duration::hours(i)).to_rfc3339();
            let score = 0.30 + (i as f64 * 0.005).sin() * 0.05;
            records.push(("NVDA".to_string(), score, ts));
        }

        let (items, total, scanned) = compute_anomalies(records, 20, 2.0, 10);
        assert_eq!(scanned, 3);
        assert!(total >= 2);
        assert_eq!(items.len(), 2);

        let aapl = items.iter().find(|i| i.ticker == "AAPL").unwrap();
        assert_eq!(aapl.direction, "bullish");
        assert!(aapl.zscore >= 2.0);

        let msft = items.iter().find(|i| i.ticker == "MSFT").unwrap();
        assert_eq!(msft.direction, "bearish");
        assert!(msft.zscore <= -2.0);
    }
}
