use axum::{
    extract::Query,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use chrono::{Duration, NaiveDate};
use serde_json::json;
use std::collections::HashMap;

use crate::handlers::backtest::generate_mock_stock_prices;
use crate::models::events::{AbnormalReturnPoint, EventStudyParams, EventStudyResponse};
use crate::pit::GLOBAL_PIT_DATA;
use crate::storage::{QuestDbClient, QuestDbClientConfig};
use once_cell::sync::Lazy;

static QUESTDB_CLIENT: Lazy<QuestDbClient> =
    Lazy::new(|| QuestDbClient::new(QuestDbClientConfig::default()));

/// Compute ordinary least squares (OLS) regression parameters (Alpha, Beta, R^2) for the market model:
/// r_asset = alpha + beta * r_benchmark + epsilon
pub fn compute_ols_market_model(
    asset_returns: &[f64],
    benchmark_returns: &[f64],
) -> (f64, f64, f64, &'static str) {
    let n = asset_returns.len();
    if n < 5 || n != benchmark_returns.len() {
        return (
            0.0,
            1.0,
            0.0,
            "Simple benchmark adjustment (fallback - insufficient estimation window points)",
        );
    }

    let sum_x: f64 = benchmark_returns.iter().sum();
    let sum_y: f64 = asset_returns.iter().sum();
    let mean_x = sum_x / (n as f64);
    let mean_y = sum_y / (n as f64);

    let mut var_x = 0.0;
    let mut cov_xy = 0.0;
    let mut ss_tot = 0.0;

    for i in 0..n {
        let dx = benchmark_returns[i] - mean_x;
        let dy = asset_returns[i] - mean_y;
        var_x += dx * dx;
        cov_xy += dx * dy;
        ss_tot += dy * dy;
    }

    if var_x < 1e-10 {
        return (
            0.0,
            1.0,
            0.0,
            "Simple benchmark adjustment (fallback - benchmark variance near zero)",
        );
    }

    let beta = cov_xy / var_x;
    let alpha = mean_y - beta * mean_x;

    let mut ss_res = 0.0;
    for i in 0..n {
        let y_pred = alpha + beta * benchmark_returns[i];
        let res = asset_returns[i] - y_pred;
        ss_res += res * res;
    }

    let r_squared = if ss_tot > 1e-12 {
        (1.0 - (ss_res / ss_tot)).clamp(0.0, 1.0)
    } else {
        0.0
    };

    (
        alpha,
        beta,
        r_squared,
        "Market model OLS regression over estimation window",
    )
}

/// Helper to build and compute the Event Study response given asset and benchmark price maps.
pub fn execute_event_study(
    ticker: &str,
    event_date: NaiveDate,
    event_window: u32,
    estimation_window: u32,
    benchmark_ticker: &str,
    asset_prices: &HashMap<NaiveDate, f64>,
    benchmark_prices: &HashMap<NaiveDate, f64>,
) -> Result<EventStudyResponse, String> {
    // 1. Find all shared dates and sort them chronologically
    let mut shared_dates: Vec<NaiveDate> = asset_prices
        .keys()
        .filter(|d| benchmark_prices.contains_key(d))
        .copied()
        .collect();
    shared_dates.sort();

    if shared_dates.len() < 5 {
        return Err("Insufficient historical trading dates available for event study".to_string());
    }

    // 2. Compute daily returns: (date, asset_return, benchmark_return)
    let mut daily_returns: Vec<(NaiveDate, f64, f64)> = Vec::with_capacity(shared_dates.len() - 1);
    for i in 1..shared_dates.len() {
        let prev_d = shared_dates[i - 1];
        let curr_d = shared_dates[i];
        let p_prev = asset_prices[&prev_d];
        let p_curr = asset_prices[&curr_d];
        let b_prev = benchmark_prices[&prev_d];
        let b_curr = benchmark_prices[&curr_d];

        if p_prev > 0.0 && b_prev > 0.0 {
            let r_asset = (p_curr - p_prev) / p_prev;
            let r_bench = (b_curr - b_prev) / b_prev;
            daily_returns.push((curr_d, r_asset, r_bench));
        }
    }

    if daily_returns.is_empty() {
        return Err("No valid returns could be computed from price series".to_string());
    }

    // 3. Locate the event date index
    let event_idx_opt = daily_returns
        .iter()
        .position(|(d, _, _)| *d >= event_date)
        .or_else(|| {
            // If event_date is past all dates, fallback to last date
            if !daily_returns.is_empty() {
                Some(daily_returns.len() - 1)
            } else {
                None
            }
        });

    let event_idx = match event_idx_opt {
        Some(idx) => idx,
        None => {
            return Err(format!(
                "Event date '{}' is outside available price range",
                event_date
            ))
        }
    };

    // 4. Determine event window indices: [event_idx - event_window, event_idx + event_window]
    let ew = event_window as usize;
    let est_w = estimation_window as usize;

    let event_start_idx = event_idx.saturating_sub(ew);
    let event_end_idx = (event_idx + ew).min(daily_returns.len().saturating_sub(1));

    // Estimation window precedes event window: [event_start_idx - est_w, event_start_idx - 1]
    let est_start_idx = event_start_idx.saturating_sub(est_w);
    let est_end_idx = event_start_idx.saturating_sub(1);

    let (est_asset_rets, est_bench_rets): (Vec<f64>, Vec<f64>) = if est_start_idx < event_start_idx
    {
        daily_returns[est_start_idx..=est_end_idx]
            .iter()
            .map(|(_, ra, rb)| (*ra, *rb))
            .unzip()
    } else {
        (Vec::new(), Vec::new())
    };

    // 5. Estimate Market Model OLS Parameters
    let (alpha, beta, r_squared, methodology_msg) =
        compute_ols_market_model(&est_asset_rets, &est_bench_rets);

    // 6. Compute Abnormal Returns & CAR across Event Window
    let mut abnormal_returns = Vec::with_capacity(event_end_idx - event_start_idx + 1);
    let mut cumulative_ar = 0.0;
    let mut car_pre_event = 0.0;
    let mut car_post_event = 0.0;

    for idx in event_start_idx..=event_end_idx {
        let (d, r_a, r_b) = daily_returns[idx];
        let day_offset = (idx as i64 - event_idx as i64) as i32;
        let expected_ret = alpha + beta * r_b;
        let abnormal_ret = r_a - expected_ret;
        cumulative_ar += abnormal_ret;

        if day_offset < 0 {
            car_pre_event += abnormal_ret;
        } else if day_offset > 0 {
            car_post_event += abnormal_ret;
        }

        abnormal_returns.push(AbnormalReturnPoint {
            date: d.format("%Y-%m-%d").to_string(),
            day_offset,
            actual_return: (r_a * 1_000_000.0).round() / 1_000_000.0,
            benchmark_return: (r_b * 1_000_000.0).round() / 1_000_000.0,
            expected_return: (expected_ret * 1_000_000.0).round() / 1_000_000.0,
            abnormal_return: (abnormal_ret * 1_000_000.0).round() / 1_000_000.0,
            cumulative_abnormal_return: (cumulative_ar * 1_000_000.0).round() / 1_000_000.0,
        });
    }

    let car_full_window = cumulative_ar;

    Ok(EventStudyResponse {
        ticker: ticker.to_uppercase(),
        event_date: event_date.format("%Y-%m-%d").to_string(),
        event_window,
        estimation_window,
        benchmark_ticker: benchmark_ticker.to_uppercase(),
        alpha: (alpha * 1_000_000.0).round() / 1_000_000.0,
        beta: (beta * 10_000.0).round() / 10_000.0,
        r_squared: (r_squared * 10_000.0).round() / 10_000.0,
        car_full_window: (car_full_window * 1_000_000.0).round() / 1_000_000.0,
        car_pre_event: (car_pre_event * 1_000_000.0).round() / 1_000_000.0,
        car_post_event: (car_post_event * 1_000_000.0).round() / 1_000_000.0,
        count: abnormal_returns.len(),
        abnormal_returns,
        message: methodology_msg.to_string(),
    })
}

/// GET /events/study - Compute Cumulative Abnormal Returns (CAR) around corporate events.
#[utoipa::path(
    get,
    path = "/events/study",
    params(EventStudyParams),
    responses(
        (status = 200, description = "Event study statistics and CAR trajectory computed successfully", body = EventStudyResponse),
        (status = 400, description = "Invalid parameters, dates, or delisted security violation"),
        (status = 401, description = "Unauthorized - Missing or invalid Bearer JWT / API Key"),
        (status = 429, description = "Too Many Requests - Rate limit quota exceeded")
    ),
    security(
        ("bearer_auth" = []),
        ("api_key_auth" = [])
    ),
    tag = "Corporate Events & Event Studies"
)]
pub async fn get_event_study_handler(Query(params): Query<EventStudyParams>) -> Response {
    let clean_ticker = params.ticker.trim().to_uppercase();
    if clean_ticker.is_empty() || clean_ticker.len() > 20 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Bad Request",
                "message": "Parameter 'ticker' must be a non-empty string of at most 20 characters"
            })),
        )
            .into_response();
    }

    let clean_benchmark = params
        .benchmark_ticker
        .as_deref()
        .unwrap_or("SPY")
        .trim()
        .to_uppercase();
    if clean_benchmark.is_empty() || clean_benchmark.len() > 20 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Bad Request",
                "message": "Parameter 'benchmark_ticker' must be a non-empty string of at most 20 characters"
            })),
        )
            .into_response();
    }

    let parsed_event_date = match NaiveDate::parse_from_str(params.event_date.trim(), "%Y-%m-%d") {
        Ok(d) => d,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Bad Request",
                    "message": format!("Invalid event_date format '{}', expected YYYY-MM-DD: {}", params.event_date, e)
                })),
            )
                .into_response();
        }
    };

    let event_window = params.event_window.unwrap_or(5);
    if event_window < 1 || event_window > 20 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Bad Request",
                "message": "event_window must be between 1 and 20 trading days"
            })),
        )
            .into_response();
    }

    let estimation_window = params.estimation_window.unwrap_or(60);
    if estimation_window < 10 || estimation_window > 120 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Bad Request",
                "message": "estimation_window must be between 10 and 120 trading days"
            })),
        )
            .into_response();
    }

    // Point-in-Time Active Listing Verification
    if !GLOBAL_PIT_DATA.is_valid_ticker(&clean_ticker, parsed_event_date) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Bad Request",
                "message": format!(
                    "Point-in-Time rejection: Ticker '{}' was not active/listed on event date '{}'",
                    clean_ticker, parsed_event_date
                )
            })),
        )
            .into_response();
    }

    // Calculate calendar span required:
    // Calendar lookback = (event_window + estimation_window + 10) * 7 / 5 + 30 days
    // Calendar lookahead = (event_window + 5) * 7 / 5 + 15 days
    let total_lookback_days = ((event_window + estimation_window + 10) as i64 * 7 / 5) + 30;
    let total_lookahead_days = ((event_window + 5) as i64 * 7 / 5) + 15;

    let start_date = parsed_event_date - Duration::days(total_lookback_days);
    let end_date = parsed_event_date + Duration::days(total_lookahead_days);

    let start_str = start_date.format("%Y-%m-%d").to_string();
    let end_str = end_date.format("%Y-%m-%d").to_string();

    // Query QuestDB or generate synthetic prices
    let mut asset_prices = HashMap::new();
    let mut benchmark_prices = HashMap::new();

    let query_tickers = vec![clean_ticker.clone(), clean_benchmark.clone()];
    if let Ok(queried) = QUESTDB_CLIENT
        .query_stock_prices(&query_tickers, &start_str, &end_str)
        .await
    {
        if let Some(p) = queried.get(&clean_ticker) {
            asset_prices = p.clone();
        }
        if let Some(bp) = queried.get(&clean_benchmark) {
            benchmark_prices = bp.clone();
        }
    }

    if asset_prices.is_empty() {
        asset_prices = generate_mock_stock_prices(&clean_ticker, start_date, end_date);
    }
    if benchmark_prices.is_empty() {
        benchmark_prices = generate_mock_stock_prices(&clean_benchmark, start_date, end_date);
    }

    match execute_event_study(
        &clean_ticker,
        parsed_event_date,
        event_window,
        estimation_window,
        &clean_benchmark,
        &asset_prices,
        &benchmark_prices,
    ) {
        Ok(study_resp) => (StatusCode::OK, Json(study_resp)).into_response(),
        Err(err_msg) => (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Bad Request",
                "message": err_msg
            })),
        )
            .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ols_market_model_exact_linear_fit() {
        let bench = vec![0.01, -0.02, 0.015, -0.005, 0.02, 0.005, -0.01, 0.03];
        // asset = 0.002 + 1.5 * bench
        let asset: Vec<f64> = bench.iter().map(|&x| 0.002 + 1.5 * x).collect();

        let (alpha, beta, r2, msg) = compute_ols_market_model(&asset, &bench);
        assert!((alpha - 0.002).abs() < 1e-6);
        assert!((beta - 1.5).abs() < 1e-6);
        assert!((r2 - 1.0).abs() < 1e-6);
        assert!(msg.contains("Market model OLS"));
    }

    #[test]
    fn test_event_study_calculation_car_properties() {
        let event_date = NaiveDate::from_ymd_opt(2025, 6, 15).unwrap();
        let start = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
        let end = NaiveDate::from_ymd_opt(2025, 7, 30).unwrap();

        let asset_prices = generate_mock_stock_prices("AAPL", start, end);
        let bench_prices = generate_mock_stock_prices("SPY", start, end);

        let resp = execute_event_study(
            "AAPL",
            event_date,
            5,
            60,
            "SPY",
            &asset_prices,
            &bench_prices,
        )
        .unwrap();

        assert_eq!(resp.ticker, "AAPL");
        assert_eq!(resp.event_window, 5);
        assert_eq!(resp.estimation_window, 60);
        assert_eq!(resp.benchmark_ticker, "SPY");
        assert_eq!(resp.abnormal_returns.len(), 11); // 2 * 5 + 1

        // CAR decomposition check: CAR_full == CAR_pre + AR_0 + CAR_post
        let ar_0 = resp
            .abnormal_returns
            .iter()
            .find(|p| p.day_offset == 0)
            .unwrap()
            .abnormal_return;
        let sum_decomp = resp.car_pre_event + ar_0 + resp.car_post_event;
        assert!(
            (resp.car_full_window - sum_decomp).abs() < 1e-4,
            "CAR full {} != sum of components {}",
            resp.car_full_window,
            sum_decomp
        );

        // Check cumulative accumulation monotonic property: CAR_t == CAR_{t-1} + AR_t
        for i in 1..resp.abnormal_returns.len() {
            let prev_car = resp.abnormal_returns[i - 1].cumulative_abnormal_return;
            let curr_ar = resp.abnormal_returns[i].abnormal_return;
            let curr_car = resp.abnormal_returns[i].cumulative_abnormal_return;
            assert!(
                (curr_car - (prev_car + curr_ar)).abs() < 1e-4,
                "Cumulative CAR mismatch at index {}: {} vs ({} + {})",
                i,
                curr_car,
                prev_car,
                curr_ar
            );
        }
    }

    #[test]
    fn test_event_study_fallback_when_insufficient_points() {
        let bench = vec![0.01, -0.02, 0.015];
        let asset = vec![0.02, -0.01, 0.025];
        let (alpha, beta, r2, msg) = compute_ols_market_model(&asset, &bench);
        assert_eq!(alpha, 0.0);
        assert_eq!(beta, 1.0);
        assert_eq!(r2, 0.0);
        assert!(msg.contains("insufficient estimation window points"));
    }

    #[test]
    fn test_event_study_fallback_zero_variance_benchmark() {
        let bench = vec![0.01, 0.01, 0.01, 0.01, 0.01, 0.01];
        let asset = vec![0.02, -0.01, 0.025, 0.03, -0.02, 0.01];
        let (alpha, beta, r2, msg) = compute_ols_market_model(&asset, &bench);
        assert_eq!(alpha, 0.0);
        assert_eq!(beta, 1.0);
        assert_eq!(r2, 0.0);
        assert!(msg.contains("benchmark variance near zero"));
    }
}
