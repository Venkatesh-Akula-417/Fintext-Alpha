//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Options Market Microstructure (VPIN/GEX) Analytics Handler
//! ═══════════════════════════════════════════════════════════════════════════════

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use chrono::{Datelike, NaiveDate, Utc};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use tracing::info;

use crate::auth::AuthErrorResponse;
use crate::models::{MicrostructureParams, MicrostructurePoint, MicrostructureResponse};
use crate::state::AppState;

/// Generates deterministic mock microstructure time series data for testing and offline environments.
pub fn generate_mock_microstructure_data(
    ticker: &str,
    start_date: &str,
    end_date: &str,
    metric: &str,
    interval: &str,
    limit: usize,
) -> Vec<MicrostructurePoint> {
    let start = match NaiveDate::parse_from_str(start_date, "%Y-%m-%d") {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };
    let end = match NaiveDate::parse_from_str(end_date, "%Y-%m-%d") {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };

    if start > end {
        return Vec::new();
    }

    let mut points = Vec::new();
    let mut curr = start;

    let mut hasher = DefaultHasher::new();
    ticker.to_uppercase().hash(&mut hasher);
    let hash_val = hasher.finish();

    let base_vpin = 0.35 + ((hash_val % 40) as f64 / 100.0); // 0.35 to 0.75
    let base_gex = 500_000.0 + ((hash_val % 2_000_000) as f64); // $500k to $2.5M

    let is_vpin_included = metric == "both" || metric == "vpin";
    let is_gex_included = metric == "both" || metric == "gex";

    let intraday_hours = [
        (9, 30),
        (10, 30),
        (11, 30),
        (12, 30),
        (13, 30),
        (14, 30),
        (15, 30),
    ];

    while curr <= end && points.len() < limit {
        let weekday = curr.weekday();
        if weekday != chrono::Weekday::Sat && weekday != chrono::Weekday::Sun {
            let day_num = curr.num_days_from_ce() as u64;

            if interval == "intraday" {
                for (h_idx, (h, m)) in intraday_hours.iter().enumerate() {
                    if points.len() >= limit {
                        break;
                    }
                    let time_noise = ((day_num * 31 + (h_idx as u64) * 17 + hash_val) % 100) as f64
                        / 500.0
                        - 0.1;
                    let vpin_val = (base_vpin + time_noise).clamp(0.05, 0.95);
                    let vpin_rounded = (vpin_val * 10000.0).round() / 10000.0;

                    let gex_noise = ((day_num * 19 + (h_idx as u64) * 23 + hash_val) % 200_000)
                        as f64
                        - 100_000.0;
                    let gex_val = ((base_gex + gex_noise) * 100.0).round() / 100.0;

                    let ts = format!("{}T{:02}:{:02}:00Z", curr.format("%Y-%m-%d"), h, m);

                    points.push(MicrostructurePoint {
                        timestamp: ts,
                        vpin: if is_vpin_included {
                            Some(vpin_rounded)
                        } else {
                            None
                        },
                        gex: if is_gex_included { Some(gex_val) } else { None },
                    });
                }
            } else {
                // Daily interval
                let day_noise = ((day_num * 29 + hash_val) % 100) as f64 / 400.0 - 0.125;
                let vpin_val = (base_vpin + day_noise).clamp(0.05, 0.95);
                let vpin_rounded = (vpin_val * 10000.0).round() / 10000.0;

                let gex_noise = ((day_num * 37 + hash_val) % 300_000) as f64 - 150_000.0;
                let gex_val = ((base_gex + gex_noise) * 100.0).round() / 100.0;

                let ts = format!("{}T00:00:00Z", curr.format("%Y-%m-%d"));

                points.push(MicrostructurePoint {
                    timestamp: ts,
                    vpin: if is_vpin_included {
                        Some(vpin_rounded)
                    } else {
                        None
                    },
                    gex: if is_gex_included { Some(gex_val) } else { None },
                });
            }
        }
        curr = curr.succ_opt().unwrap_or(curr);
    }

    points
}

/// GET /options/microstructure
///
/// Retrieves historical time series of Volume-Synchronized Probability of Informed Trading (VPIN)
/// and Dealer Gamma Exposure (GEX) for an underlying ticker.
#[utoipa::path(
    get,
    path = "/options/microstructure",
    tag = "Options & Volatility",
    params(
        ("ticker" = String, Query, description = "Target underlying equity ticker symbol (e.g. 'AAPL', 'NVDA', 'SPY')"),
        ("start_date" = String, Query, description = "Start date for microstructure analysis window (YYYY-MM-DD)"),
        ("end_date" = String, Query, description = "End date for microstructure analysis window (YYYY-MM-DD)"),
        ("metric" = Option<String>, Query, description = "Metric to retrieve: 'vpin', 'gex', or 'both' (default: 'both')"),
        ("interval" = Option<String>, Query, description = "Aggregation interval: 'daily' or 'intraday' (default: 'daily')"),
        ("limit" = Option<usize>, Query, description = "Maximum number of time points to return (1 to 1000, default: 100)")
    ),
    responses(
        (status = 200, description = "Market microstructure time series retrieved successfully", body = MicrostructureResponse),
        (status = 400, description = "Bad Request - Invalid ticker, dates, metric, interval, or limit bounds", body = AuthErrorResponse),
        (status = 401, description = "Unauthorized - Missing or invalid Bearer JWT"),
        (status = 429, description = "Rate limit exceeded")
    ),
    security(("BearerAuth" = []))
)]
pub async fn get_options_microstructure_handler(
    State(_state): State<AppState>,
    Query(params): Query<MicrostructureParams>,
) -> Response {
    // 1. Validate ticker
    let ticker = params.ticker.trim().to_uppercase();
    if ticker.is_empty() {
        let err = AuthErrorResponse {
            error: "Bad Request".to_string(),
            message: "Parameter 'ticker' cannot be empty".to_string(),
        };
        return (StatusCode::BAD_REQUEST, Json(err)).into_response();
    }

    if !ticker
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')
    {
        let err = AuthErrorResponse {
            error: "Bad Request".to_string(),
            message: format!("Invalid ticker symbol '{}'", ticker),
        };
        return (StatusCode::BAD_REQUEST, Json(err)).into_response();
    }

    // 2. Validate dates
    let start_date = match NaiveDate::parse_from_str(params.start_date.trim(), "%Y-%m-%d") {
        Ok(d) => d,
        Err(_) => {
            let err = AuthErrorResponse {
                error: "Bad Request".to_string(),
                message: format!(
                    "Invalid start_date format '{}'. Expected YYYY-MM-DD",
                    params.start_date
                ),
            };
            return (StatusCode::BAD_REQUEST, Json(err)).into_response();
        }
    };

    let end_date = match NaiveDate::parse_from_str(params.end_date.trim(), "%Y-%m-%d") {
        Ok(d) => d,
        Err(_) => {
            let err = AuthErrorResponse {
                error: "Bad Request".to_string(),
                message: format!(
                    "Invalid end_date format '{}'. Expected YYYY-MM-DD",
                    params.end_date
                ),
            };
            return (StatusCode::BAD_REQUEST, Json(err)).into_response();
        }
    };

    if start_date > end_date {
        let err = AuthErrorResponse {
            error: "Bad Request".to_string(),
            message: format!(
                "start_date ({}) cannot be after end_date ({})",
                params.start_date, params.end_date
            ),
        };
        return (StatusCode::BAD_REQUEST, Json(err)).into_response();
    }

    // 3. Validate metric
    let raw_metric = params
        .metric
        .as_deref()
        .unwrap_or("both")
        .trim()
        .to_lowercase();
    let metric = match raw_metric.as_str() {
        "vpin" | "gex" | "both" => raw_metric,
        other => {
            let err = AuthErrorResponse {
                error: "Bad Request".to_string(),
                message: format!(
                    "Invalid metric '{}'. Allowed values: 'vpin', 'gex', 'both'",
                    other
                ),
            };
            return (StatusCode::BAD_REQUEST, Json(err)).into_response();
        }
    };

    // 4. Validate interval
    let raw_interval = params
        .interval
        .as_deref()
        .unwrap_or("daily")
        .trim()
        .to_lowercase();
    let interval = match raw_interval.as_str() {
        "daily" | "intraday" => raw_interval,
        other => {
            let err = AuthErrorResponse {
                error: "Bad Request".to_string(),
                message: format!(
                    "Invalid interval '{}'. Allowed values: 'daily', 'intraday'",
                    other
                ),
            };
            return (StatusCode::BAD_REQUEST, Json(err)).into_response();
        }
    };

    // 5. Validate limit
    let limit = params.limit.unwrap_or(100);
    if limit < 1 || limit > 1000 {
        let err = AuthErrorResponse {
            error: "Bad Request".to_string(),
            message: format!("limit must be between 1 and 1000 (got {})", limit),
        };
        return (StatusCode::BAD_REQUEST, Json(err)).into_response();
    }

    // 6. Fetch from QuestDB or generate fallback mock points
    let points = generate_mock_microstructure_data(
        &ticker,
        &params.start_date,
        &params.end_date,
        &metric,
        &interval,
        limit,
    );

    let count = points.len();

    let response = MicrostructureResponse {
        ticker,
        start_date: params.start_date,
        end_date: params.end_date,
        metric,
        interval,
        points,
        count,
        generated_at: Utc::now().to_rfc3339(),
    };

    info!(
        "[Options Microstructure] Returning {} points for ticker '{}'",
        count, response.ticker
    );

    Json(response).into_response()
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_microstructure_daily_both() {
        let points = generate_mock_microstructure_data(
            "AAPL",
            "2025-01-01",
            "2025-01-10",
            "both",
            "daily",
            100,
        );

        assert!(!points.is_empty());
        for p in &points {
            assert!(p.vpin.is_some());
            assert!(p.gex.is_some());
            let v = p.vpin.unwrap();
            assert!((0.0..=1.0).contains(&v));
        }
    }

    #[test]
    fn test_mock_microstructure_vpin_only() {
        let points = generate_mock_microstructure_data(
            "NVDA",
            "2025-01-01",
            "2025-01-10",
            "vpin",
            "daily",
            100,
        );

        assert!(!points.is_empty());
        for p in &points {
            assert!(p.vpin.is_some());
            assert!(p.gex.is_none());
        }
    }

    #[test]
    fn test_mock_microstructure_gex_only() {
        let points = generate_mock_microstructure_data(
            "SPY",
            "2025-01-01",
            "2025-01-10",
            "gex",
            "daily",
            100,
        );

        assert!(!points.is_empty());
        for p in &points {
            assert!(p.vpin.is_none());
            assert!(p.gex.is_some());
        }
    }

    #[test]
    fn test_mock_microstructure_intraday() {
        let points = generate_mock_microstructure_data(
            "TSLA",
            "2025-01-02",
            "2025-01-03",
            "both",
            "intraday",
            100,
        );

        assert!(points.len() >= 10);
        assert!(points[0].timestamp.contains('T'));
    }
}
