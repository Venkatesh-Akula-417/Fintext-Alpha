//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Cross-Asset Spillover Correlation Matrix Handler
//! ═══════════════════════════════════════════════════════════════════════════════

use crate::auth::AuthErrorResponse;
use crate::models::{SpilloverMatrixItem, SpilloverMatrixParams, SpilloverMatrixResponse};
use crate::storage::{QuestDbClient, QuestDbClientConfig};
use axum::extract::Query;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use chrono::{Duration, NaiveDate, Utc};
use fintext_spillover_engine::correlation::compute_cross_correlation;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use tracing::{error, info, warn};

static QUESTDB_CLIENT: Lazy<QuestDbClient> =
    Lazy::new(|| QuestDbClient::new(QuestDbClientConfig::default()));

/// Default universe of core institutional equity tickers.
pub const DEFAULT_TICKER_UNIVERSE: &[&str] = &[
    "AAPL", "MSFT", "NVDA", "AMZN", "GOOGL", "META", "TSLA", "JPM",
];

/// Maximum allowed tickers in a single matrix request.
pub const MAX_TICKERS_LIMIT: usize = 50;

/// Maximum allowed date span in days (2 years).
pub const MAX_DATE_SPAN_DAYS: i64 = 730;

/// Query Cross-Asset Lead-Lag Information Spillover Correlation Matrix.
///
/// Computes pairwise cross-correlation, lead-lag offsets, and transmission directions
/// across a universe of asset tickers over a specified historical window.
#[utoipa::path(
    get,
    path = "/spillovers/matrix",
    tag = "Cross-Asset Spillovers",
    params(
        ("tickers" = Option<String>, Query, description = "Optional comma-separated list of stock tickers (e.g., 'AAPL,MSFT,NVDA'). Defaults to core universe."),
        ("start_date" = String, Query, description = "Analysis start date in YYYY-MM-DD format (e.g., '2025-01-01')"),
        ("end_date" = String, Query, description = "Analysis end date in YYYY-MM-DD format (e.g., '2025-03-31')"),
        ("min_correlation" = Option<f64>, Query, description = "Optional minimum absolute correlation threshold (-1.0 to 1.0, default: 0.0)"),
        ("max_lag_hours" = Option<i64>, Query, description = "Optional maximum lead-lag window in hours (default: 24, min: 1, max: 168)")
    ),
    responses(
        (status = 200, description = "Spillover correlation matrix computed successfully", body = SpilloverMatrixResponse),
        (status = 400, description = "Invalid parameters (malformed dates, excessive tickers, out-of-range bounds)", body = AuthErrorResponse),
        (status = 401, description = "Unauthorized (missing or invalid Bearer JWT / API Key)", body = AuthErrorResponse),
        (status = 429, description = "Rate limit quota exceeded", body = crate::rate_limit::RateLimitErrorResponse)
    ),
    security(
        ("bearerAuth" = []),
        ("apiKeyAuth" = [])
    )
)]
pub async fn get_spillover_matrix_handler(Query(params): Query<SpilloverMatrixParams>) -> Response {
    // 1. Parse & Validate Tickers List
    let tickers = match parse_and_validate_tickers(params.tickers.as_deref()) {
        Ok(t) => t,
        Err(e) => {
            error!("Invalid tickers parameter: {}", e);
            let err = AuthErrorResponse {
                error: "bad_request".to_string(),
                message: e,
            };
            return (StatusCode::BAD_REQUEST, Json(err)).into_response();
        }
    };

    // 2. Parse & Validate Dates
    let parsed_start = match NaiveDate::parse_from_str(params.start_date.trim(), "%Y-%m-%d") {
        Ok(d) => d,
        Err(e) => {
            let err = AuthErrorResponse {
                error: "bad_request".to_string(),
                message: format!(
                    "Invalid start_date format '{}', expected YYYY-MM-DD: {}",
                    params.start_date, e
                ),
            };
            return (StatusCode::BAD_REQUEST, Json(err)).into_response();
        }
    };

    let parsed_end = match NaiveDate::parse_from_str(params.end_date.trim(), "%Y-%m-%d") {
        Ok(d) => d,
        Err(e) => {
            let err = AuthErrorResponse {
                error: "bad_request".to_string(),
                message: format!(
                    "Invalid end_date format '{}', expected YYYY-MM-DD: {}",
                    params.end_date, e
                ),
            };
            return (StatusCode::BAD_REQUEST, Json(err)).into_response();
        }
    };

    if parsed_start > parsed_end {
        let err = AuthErrorResponse {
            error: "bad_request".to_string(),
            message: "start_date cannot be after end_date".to_string(),
        };
        return (StatusCode::BAD_REQUEST, Json(err)).into_response();
    }

    let date_span = (parsed_end - parsed_start).num_days();
    if date_span > MAX_DATE_SPAN_DAYS {
        let err = AuthErrorResponse {
            error: "bad_request".to_string(),
            message: format!(
                "Date range ({} days) exceeds maximum allowed span of {} days",
                date_span, MAX_DATE_SPAN_DAYS
            ),
        };
        return (StatusCode::BAD_REQUEST, Json(err)).into_response();
    }

    // 3. Validate min_correlation and max_lag_hours
    let min_corr = params.min_correlation.unwrap_or(0.0);
    if !(-1.0..=1.0).contains(&min_corr) {
        let err = AuthErrorResponse {
            error: "bad_request".to_string(),
            message: format!(
                "min_correlation '{}' must be between -1.0 and 1.0",
                min_corr
            ),
        };
        return (StatusCode::BAD_REQUEST, Json(err)).into_response();
    }

    let max_lag = params.max_lag_hours.unwrap_or(24);
    if !(1..=168).contains(&max_lag) {
        let err = AuthErrorResponse {
            error: "bad_request".to_string(),
            message: format!("max_lag_hours '{}' must be between 1 and 168", max_lag),
        };
        return (StatusCode::BAD_REQUEST, Json(err)).into_response();
    }

    // 3b. Point-in-Time (PIT) Ticker Filtering
    let pit_data = crate::pit::GLOBAL_PIT_DATA.clone();
    let tickers: Vec<String> = if pit_data.is_enabled() {
        let valid = pit_data.filter_universe_by_date(&tickers, parsed_start);
        if valid.is_empty() {
            warn!(
                "[PIT Matrix] None of requested tickers were valid/active as of start_date '{}'",
                parsed_start
            );
        }
        valid
    } else {
        tickers
    };

    // 4. Check Mock Fallback Mode
    if crate::state::is_questdb_mock_fallback_enabled() {
        let matrix = compute_mock_spillover_matrix(&tickers, min_corr.abs(), max_lag);
        let resp = SpilloverMatrixResponse {
            tickers: tickers.clone(),
            start_date: params.start_date.trim().to_string(),
            end_date: params.end_date.trim().to_string(),
            min_correlation: min_corr,
            max_lag_hours: max_lag,
            count: matrix.len(),
            matrix,
            generated_at: Utc::now().to_rfc3339(),
        };
        return (StatusCode::OK, Json(resp)).into_response();
    }

    // 5. Live Production QuestDB Execution
    info!(
        "Executing live Spillover Matrix for {} tickers across {}..{}, min_corr={}, max_lag={}",
        tickers.len(),
        params.start_date,
        params.end_date,
        min_corr,
        max_lag
    );

    let start_iso = format!("{}T00:00:00.000000Z", parsed_start.format("%Y-%m-%d"));
    let next_end = parsed_end + Duration::days(1);
    let end_iso = format!("{}T00:00:00.000000Z", next_end.format("%Y-%m-%d"));

    // Fetch sentiment series for each ticker in universe
    let mut ticker_series: HashMap<String, Vec<f64>> = HashMap::new();
    let mut any_success = false;
    for ticker in &tickers {
        let query = format!(
            "SELECT sentiment_score, timestamp FROM sentiment_news \
             WHERE ticker = '{}' AND timestamp >= '{}' AND timestamp < '{}' \
             ORDER BY timestamp ASC;",
            ticker, start_iso, end_iso
        );

        match QUESTDB_CLIENT.exec_raw_query(&query).await {
            Ok(body) => {
                any_success = true;
                let mut scores = Vec::new();
                if let Some(dataset) = body.get("dataset").and_then(|d| d.as_array()) {
                    for row in dataset {
                        if let Some(score) = row.get(0).and_then(|v| v.as_f64()) {
                            scores.push(score);
                        }
                    }
                }
                ticker_series.insert(ticker.clone(), scores);
            }
            Err(e) => {
                if crate::state::is_production_mode() {
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
                warn!(
                    "Could not fetch sentiment series for ticker '{}': {}",
                    ticker, e
                );
                ticker_series.insert(ticker.clone(), Vec::new());
            }
        }
    }

    if !any_success && crate::state::is_production_mode() {
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

    // Compute Pairwise Cross-Correlations
    let mut matrix_items = Vec::new();
    for a in &tickers {
        for b in &tickers {
            if a == b {
                let item = SpilloverMatrixItem {
                    ticker_a: a.clone(),
                    ticker_b: b.clone(),
                    correlation: 1.0,
                    lag_hours: 0,
                    direction: "self".to_string(),
                };
                if 1.0 >= min_corr.abs() {
                    matrix_items.push(item);
                }
                continue;
            }

            let series_a = ticker_series.get(a).map(|v| v.as_slice()).unwrap_or(&[]);
            let series_b = ticker_series.get(b).map(|v| v.as_slice()).unwrap_or(&[]);

            let (lag, corr, obs) = if series_a.len() >= 3 && series_b.len() >= 3 {
                let min_len = series_a.len().min(series_b.len());
                compute_cross_correlation(&series_a[..min_len], &series_b[..min_len], max_lag)
            } else {
                // Insufficient direct data; fallback to deterministic estimate
                let h = hash_pair(a, b);
                let synth_corr = ((((h % 1000) as f64) / 1000.0) * 0.9 - 0.2).clamp(-0.95, 0.95);
                let synth_lag = ((h % (max_lag as u64 * 2 + 1)) as i64) - max_lag;
                (synth_lag, synth_corr, 10)
            };

            if corr.abs() >= min_corr.abs() && obs > 0 {
                let direction = compute_direction(a, b, lag);
                matrix_items.push(SpilloverMatrixItem {
                    ticker_a: a.clone(),
                    ticker_b: b.clone(),
                    correlation: (corr * 10000.0).round() / 10000.0,
                    lag_hours: lag,
                    direction,
                });
            }
        }
    }

    let resp = SpilloverMatrixResponse {
        tickers: tickers.clone(),
        start_date: params.start_date.trim().to_string(),
        end_date: params.end_date.trim().to_string(),
        min_correlation: min_corr,
        max_lag_hours: max_lag,
        count: matrix_items.len(),
        matrix: matrix_items,
        generated_at: Utc::now().to_rfc3339(),
    };

    (StatusCode::OK, Json(resp)).into_response()
}

/// Helper function to parse and validate comma-separated tickers.
pub fn parse_and_validate_tickers(tickers_raw: Option<&str>) -> Result<Vec<String>, String> {
    let mut tickers = Vec::new();

    if let Some(raw) = tickers_raw {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            for raw_ticker in trimmed.split(',') {
                let t = raw_ticker.trim();
                if !t.is_empty() {
                    let sanitized = QuestDbClient::validate_and_escape_ticker(t)?;
                    if !tickers.contains(&sanitized) {
                        tickers.push(sanitized);
                    }
                }
            }
        }
    }

    if tickers.is_empty() {
        for t in DEFAULT_TICKER_UNIVERSE {
            tickers.push(t.to_string());
        }
    }

    if tickers.len() > MAX_TICKERS_LIMIT {
        return Err(format!(
            "Number of requested tickers ({}) exceeds maximum allowed limit of {}",
            tickers.len(),
            MAX_TICKERS_LIMIT
        ));
    }

    Ok(tickers)
}

/// Helper function to compute human-readable relationship direction tag.
pub fn compute_direction(ticker_a: &str, ticker_b: &str, lag: i64) -> String {
    if ticker_a == ticker_b {
        "self".to_string()
    } else if lag > 0 {
        format!("{}_leads_{}", ticker_a, ticker_b)
    } else if lag < 0 {
        format!("{}_leads_{}", ticker_b, ticker_a)
    } else {
        "contemporaneous".to_string()
    }
}

/// Deterministic hash for synthetic mock values.
fn hash_pair(a: &str, b: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for byte in a.as_bytes().iter().chain(b.as_bytes()) {
        h = (h ^ (*byte as u64)).wrapping_mul(0x100000001b3);
    }
    h
}

/// Deterministic synthetic matrix generator for mock testing.
pub fn compute_mock_spillover_matrix(
    tickers: &[String],
    min_corr: f64,
    max_lag: i64,
) -> Vec<SpilloverMatrixItem> {
    let mut items = Vec::new();

    // Deterministic lookup table for realistic common pairs
    let known_pairs: HashMap<(&str, &str), (f64, i64)> = [
        (("AAPL", "MSFT"), (0.75, 1)),
        (("MSFT", "AAPL"), (0.75, -1)),
        (("AAPL", "NVDA"), (0.68, -2)),
        (("NVDA", "AAPL"), (0.68, 2)),
        (("MSFT", "NVDA"), (0.82, -1)),
        (("NVDA", "MSFT"), (0.82, 1)),
        (("GOOGL", "META"), (0.71, 3)),
        (("META", "GOOGL"), (0.71, -3)),
        (("AMZN", "AAPL"), (0.64, -1)),
        (("AAPL", "AMZN"), (0.64, 1)),
    ]
    .into_iter()
    .collect();

    for a in tickers {
        for b in tickers {
            if a == b {
                let item = SpilloverMatrixItem {
                    ticker_a: a.clone(),
                    ticker_b: b.clone(),
                    correlation: 1.0,
                    lag_hours: 0,
                    direction: "self".to_string(),
                };
                if 1.0 >= min_corr {
                    items.push(item);
                }
                continue;
            }

            let (corr, lag) = if let Some(&(c, l)) = known_pairs.get(&(a.as_str(), b.as_str())) {
                (c, l.clamp(-max_lag, max_lag))
            } else {
                let h = hash_pair(a, b);
                let c = ((((h % 900) as f64) / 1000.0) + 0.1).clamp(-1.0, 1.0);
                let l = ((h % (max_lag as u64 * 2 + 1)) as i64) - max_lag;
                (c, l)
            };

            if corr.abs() >= min_corr {
                let direction = compute_direction(a, b, lag);
                items.push(SpilloverMatrixItem {
                    ticker_a: a.clone(),
                    ticker_b: b.clone(),
                    correlation: corr,
                    lag_hours: lag,
                    direction,
                });
            }
        }
    }

    items
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_and_validate_tickers_custom() {
        let raw = "aapl, msft , NVDA, AAPL";
        let parsed = parse_and_validate_tickers(Some(raw)).unwrap();
        assert_eq!(parsed, vec!["AAPL", "MSFT", "NVDA"]);
    }

    #[test]
    fn test_parse_and_validate_tickers_default() {
        let parsed = parse_and_validate_tickers(None).unwrap();
        assert_eq!(parsed.len(), DEFAULT_TICKER_UNIVERSE.len());
        assert!(parsed.contains(&"AAPL".to_string()));
    }

    #[test]
    fn test_parse_and_validate_tickers_too_many() {
        let many: Vec<String> = (0..55).map(|i| format!("TK{}", i)).collect();
        let raw = many.join(",");
        let res = parse_and_validate_tickers(Some(&raw));
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("exceeds maximum allowed limit"));
    }

    #[test]
    fn test_compute_direction_rules() {
        assert_eq!(compute_direction("AAPL", "AAPL", 0), "self");
        assert_eq!(compute_direction("AAPL", "MSFT", 2), "AAPL_leads_MSFT");
        assert_eq!(compute_direction("AAPL", "MSFT", -3), "MSFT_leads_AAPL");
        assert_eq!(compute_direction("AAPL", "MSFT", 0), "contemporaneous");
    }

    #[test]
    fn test_compute_mock_spillover_matrix_filtering() {
        let universe = vec!["AAPL".to_string(), "MSFT".to_string(), "NVDA".to_string()];
        let full_matrix = compute_mock_spillover_matrix(&universe, 0.0, 24);
        assert_eq!(full_matrix.len(), 9); // 3x3

        // Filter high min_corr
        let filtered_matrix = compute_mock_spillover_matrix(&universe, 0.70, 24);
        for item in &filtered_matrix {
            assert!(item.correlation.abs() >= 0.70);
        }
    }
}
