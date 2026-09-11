use axum::extract::Query;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use chrono::{Datelike, NaiveDate, Utc};
use once_cell::sync::Lazy;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use tracing::info;

use crate::models::{PutCallRatioParams, PutCallRatioPoint, PutCallRatioResponse};
use crate::pit::GLOBAL_PIT_DATA;
use crate::storage::questdb_client::{QuestDbClient, QuestDbClientConfig};

static QUESTDB_CLIENT: Lazy<QuestDbClient> =
    Lazy::new(|| QuestDbClient::new(QuestDbClientConfig::default()));

/// Generate deterministic mock put/call daily aggregate data for offline/test environments.
pub fn generate_mock_put_call_data(
    ticker: Option<&str>,
    start_date: &str,
    end_date: &str,
) -> Vec<PutCallRatioPoint> {
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

    let base_call_vol = match ticker {
        Some(t) => {
            let mut hasher = DefaultHasher::new();
            t.to_uppercase().hash(&mut hasher);
            50_000 + (hasher.finish() % 100_000)
        }
        None => 12_000_000,
    };

    let base_put_vol = (base_call_vol as f64 * 0.68) as u64;
    let base_call_oi = base_call_vol * 15;
    let base_put_oi = base_put_vol * 15;

    while curr <= end {
        // Skip weekends
        let weekday = curr.weekday();
        if weekday != chrono::Weekday::Sat && weekday != chrono::Weekday::Sun {
            let day_num = curr.num_days_from_ce() as u64;
            let noise_call = (day_num * 17 + 31) % 15_000;
            let noise_put = (day_num * 23 + 47) % 12_000;

            let call_vol = base_call_vol + noise_call;
            let put_vol = base_put_vol + noise_put;

            let call_oi = base_call_oi + (noise_call * 10);
            let put_oi = base_put_oi + (noise_put * 10);

            let vol_ratio = if call_vol > 0 {
                Some(((put_vol as f64 / call_vol as f64) * 10000.0).round() / 10000.0)
            } else {
                None
            };

            points.push(PutCallRatioPoint {
                date: curr.format("%Y-%m-%d").to_string(),
                put_volume: put_vol,
                call_volume: call_vol,
                put_open_interest: Some(put_oi),
                call_open_interest: Some(call_oi),
                ratio: vol_ratio,
            });
        }
        curr = curr.succ_opt().unwrap_or(curr);
    }

    points
}

/// Query options put/call volume and open interest ratios for a ticker or market-wide universe.
///
/// Returns daily time series or aggregate period totals with options market sentiment metrics.
#[utoipa::path(
    get,
    path = "/options/put-call-ratio",
    params(PutCallRatioParams),
    responses(
        (status = 200, description = "Put/Call ratio calculation results", body = PutCallRatioResponse),
        (status = 400, description = "Invalid query parameters"),
        (status = 401, description = "Missing or invalid Bearer JWT"),
        (status = 429, description = "Rate limit exceeded")
    ),
    security(
        ("bearerAuth" = [])
    ),
    tag = "Options Microstructure"
)]
pub async fn get_put_call_ratio_handler(Query(params): Query<PutCallRatioParams>) -> Response {
    // 1. Validate start_date and end_date
    let start_date = params.start_date.trim();
    if start_date.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "Bad Request",
                "message": "Parameter 'start_date' must not be empty"
            })),
        )
            .into_response();
    }

    let end_date = params.end_date.trim();
    if end_date.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "Bad Request",
                "message": "Parameter 'end_date' must not be empty"
            })),
        )
            .into_response();
    }

    let parsed_start = match NaiveDate::parse_from_str(start_date, "%Y-%m-%d") {
        Ok(d) => d,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "Bad Request",
                    "message": format!("Invalid start_date '{}', expected format YYYY-MM-DD", start_date)
                })),
            )
                .into_response();
        }
    };

    let parsed_end = match NaiveDate::parse_from_str(end_date, "%Y-%m-%d") {
        Ok(d) => d,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "Bad Request",
                    "message": format!("Invalid end_date '{}', expected format YYYY-MM-DD", end_date)
                })),
            )
                .into_response();
        }
    };

    if parsed_start > parsed_end {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "Bad Request",
                "message": format!("start_date '{}' cannot be after end_date '{}'", start_date, end_date)
            })),
        )
            .into_response();
    }

    let days_diff = (parsed_end - parsed_start).num_days();
    if days_diff > 730 {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "Bad Request",
                "message": format!("Observation window of {} days exceeds maximum allowed limit of 730 days (2 years)", days_diff)
            })),
        )
            .into_response();
    }

    // 2. Validate ratio_type
    let ratio_type = params
        .ratio_type
        .as_deref()
        .unwrap_or("volume")
        .trim()
        .to_lowercase();
    if ratio_type != "volume" && ratio_type != "open_interest" {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "Bad Request",
                "message": format!("Invalid ratio_type '{}', allowed values are 'volume' or 'open_interest'", ratio_type)
            })),
        )
            .into_response();
    }

    // 3. Validate granularity
    let granularity = params
        .granularity
        .as_deref()
        .unwrap_or("daily")
        .trim()
        .to_lowercase();
    if granularity != "daily" && granularity != "total" {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "Bad Request",
                "message": format!("Invalid granularity '{}', allowed values are 'daily' or 'total'", granularity)
            })),
        )
            .into_response();
    }

    // 4. Validate ticker and PIT if provided
    let clean_ticker: Option<String> = match params.ticker.as_deref() {
        Some(t) if !t.trim().is_empty() => {
            let s = t.trim().to_uppercase();
            if s.len() > 10 || !s.chars().all(|c| c.is_ascii_alphanumeric()) {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({
                        "error": "Bad Request",
                        "message": format!("Invalid ticker symbol '{}'", t)
                    })),
                )
                    .into_response();
            }

            // PIT Validation
            if !GLOBAL_PIT_DATA.is_valid_ticker(&s, parsed_start) {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({
                        "error": "Bad Request",
                        "message": format!("Ticker '{}' was not active or listed on '{}' (Point-in-Time check failed)", s, start_date)
                    })),
                )
                    .into_response();
            }
            Some(s)
        }
        _ => None,
    };

    info!(
        "[Put/Call Ratio] Querying ticker={:?}, start={}, end={}, ratio_type={}, granularity={}",
        clean_ticker, start_date, end_date, ratio_type, granularity
    );

    // 5. Fetch daily points from QuestDB or fallback mock generator
    let mock_mode = crate::state::is_questdb_mock_fallback_enabled();

    let mut points = if mock_mode {
        generate_mock_put_call_data(clean_ticker.as_deref(), start_date, end_date)
    } else {
        match QUESTDB_CLIENT
            .query_put_call_ratio(clean_ticker.as_deref(), start_date, end_date)
            .await
        {
            Ok(pts) if !pts.is_empty() => pts,
            Ok(_) if crate::state::is_production_mode() => Vec::new(),
            Err(e) if crate::state::is_production_mode() => {
                return (
                    StatusCode::SERVICE_UNAVAILABLE,
                    Json(serde_json::json!({
                        "error": "Service Unavailable",
                        "message": "Required data source unavailable in production mode.",
                        "detail": format!("{}", e),
                        "status": "service_unavailable"
                    })),
                )
                    .into_response();
            }
            _ => generate_mock_put_call_data(clean_ticker.as_deref(), start_date, end_date),
        }
    };

    // 6. Recalculate ratios based on requested ratio_type
    for pt in &mut points {
        if ratio_type == "open_interest" {
            let put_oi = pt.put_open_interest.unwrap_or(0);
            let call_oi = pt.call_open_interest.unwrap_or(0);
            pt.ratio = if call_oi > 0 {
                Some(((put_oi as f64 / call_oi as f64) * 10000.0).round() / 10000.0)
            } else {
                None
            };
        } else {
            pt.ratio = if pt.call_volume > 0 {
                Some(((pt.put_volume as f64 / pt.call_volume as f64) * 10000.0).round() / 10000.0)
            } else {
                None
            };
        }
    }

    let generated_at = Utc::now().to_rfc3339();

    // 7. Assemble response envelope based on granularity
    if granularity == "total" {
        let mut tot_put_vol = 0u64;
        let mut tot_call_vol = 0u64;
        let mut tot_put_oi = 0u64;
        let mut tot_call_oi = 0u64;

        for pt in &points {
            tot_put_vol += pt.put_volume;
            tot_call_vol += pt.call_volume;
            tot_put_oi += pt.put_open_interest.unwrap_or(0);
            tot_call_oi += pt.call_open_interest.unwrap_or(0);
        }

        let total_ratio = if ratio_type == "open_interest" {
            if tot_call_oi > 0 {
                Some(((tot_put_oi as f64 / tot_call_oi as f64) * 10000.0).round() / 10000.0)
            } else {
                None
            }
        } else if tot_call_vol > 0 {
            Some(((tot_put_vol as f64 / tot_call_vol as f64) * 10000.0).round() / 10000.0)
        } else {
            None
        };

        let response = PutCallRatioResponse {
            ticker: clean_ticker,
            start_date: start_date.to_string(),
            end_date: end_date.to_string(),
            ratio_type,
            granularity,
            points: None,
            average_ratio: None,
            total_put_volume: Some(tot_put_vol),
            total_call_volume: Some(tot_call_vol),
            total_put_open_interest: Some(tot_put_oi),
            total_call_open_interest: Some(tot_call_oi),
            total_ratio,
            generated_at,
        };

        (StatusCode::OK, Json(response)).into_response()
    } else {
        let valid_ratios: Vec<f64> = points.iter().filter_map(|p| p.ratio).collect();
        let avg_ratio = if !valid_ratios.is_empty() {
            let sum: f64 = valid_ratios.iter().sum();
            Some(((sum / valid_ratios.len() as f64) * 10000.0).round() / 10000.0)
        } else {
            None
        };

        let response = PutCallRatioResponse {
            ticker: clean_ticker,
            start_date: start_date.to_string(),
            end_date: end_date.to_string(),
            ratio_type,
            granularity,
            points: Some(points),
            average_ratio: avg_ratio,
            total_put_volume: None,
            total_call_volume: None,
            total_put_open_interest: None,
            total_call_open_interest: None,
            total_ratio: None,
            generated_at,
        };

        (StatusCode::OK, Json(response)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_mock_put_call_data_ticker() {
        let pts = generate_mock_put_call_data(Some("AAPL"), "2025-01-01", "2025-01-10");
        assert!(!pts.is_empty());
        for pt in &pts {
            assert!(pt.call_volume > 0);
            assert!(pt.put_volume > 0);
            assert!(pt.ratio.is_some());
            let r = pt.ratio.unwrap();
            assert!(r > 0.0 && r < 5.0);
        }
    }

    #[test]
    fn test_generate_mock_put_call_data_market_wide() {
        let pts = generate_mock_put_call_data(None, "2025-01-01", "2025-01-05");
        assert!(!pts.is_empty());
        for pt in &pts {
            assert!(pt.call_volume > 1_000_000);
            assert!(pt.put_volume > 1_000_000);
        }
    }
}
