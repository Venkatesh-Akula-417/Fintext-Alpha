//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Options Volatility Surface Analytics Engine
//! ═══════════════════════════════════════════════════════════════════════════════
//!
//! Computes and returns a 2D implied volatility matrix across multiple strike
//! prices and expiration dates for a target underlying equity ticker.
//! ═══════════════════════════════════════════════════════════════════════════════

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::{Datelike, Duration as ChronoDuration, NaiveDate, Utc, Weekday};
use serde_json::json;

use crate::handlers::options_iv::resolve_mock_spot_price;
use crate::models::{OptionsVolSurfaceParams, OptionsVolSurfaceResponse, VolSurfacePoint};
use crate::pit::GLOBAL_PIT_DATA;
use crate::state::AppState;
use crate::storage::questdb_client::QuestDbClient;

/// Generates a symmetric grid of $K$ strike prices centered around ATM within the specified range.
pub fn generate_strike_grid(
    spot: f64,
    min_mult: f64,
    max_mult: f64,
    strike_count: usize,
) -> Vec<f64> {
    if strike_count == 1 {
        return vec![(spot * 100.0).round() / 100.0];
    }

    let min_k = spot * min_mult;
    let max_k = spot * max_mult;
    let step = (max_k - min_k) / ((strike_count - 1) as f64);

    let mut strikes = Vec::with_capacity(strike_count);
    for i in 0..strike_count {
        let k = min_k + (i as f64 * step);
        strikes.push((k * 100.0).round() / 100.0);
    }
    strikes
}

/// Finds the 3rd Friday of a given year and month (standard US equity options expiration).
pub fn find_third_friday(year: i32, month: u32) -> Option<NaiveDate> {
    let mut fridays_seen = 0;
    for day in 1..=31 {
        if let Some(date) = NaiveDate::from_ymd_opt(year, month, day) {
            if date.weekday() == Weekday::Fri {
                fridays_seen += 1;
                if fridays_seen == 3 {
                    return Some(date);
                }
            }
        }
    }
    None
}

/// Generates standard expiration dates between `start_date` and `end_date`.
pub fn generate_expirations_schedule(start_date: NaiveDate, end_date: NaiveDate) -> Vec<NaiveDate> {
    let mut expiries = Vec::new();
    let mut curr_year = start_date.year();
    let mut curr_month = start_date.month();

    let end_year = end_date.year();
    let end_month = end_date.month();

    while (curr_year < end_year) || (curr_year == end_year && curr_month <= end_month) {
        if let Some(third_fri) = find_third_friday(curr_year, curr_month) {
            if third_fri >= start_date && third_fri <= end_date {
                expiries.push(third_fri);
            }
        }
        curr_month += 1;
        if curr_month > 12 {
            curr_month = 1;
            curr_year += 1;
        }
        if expiries.len() >= 12 {
            break;
        }
    }

    // Fallback if no 3rd Fridays found in range (e.g. narrow window)
    if expiries.is_empty() {
        let mut d = start_date + ChronoDuration::days(14);
        while d <= end_date && expiries.len() < 6 {
            expiries.push(d);
            d += ChronoDuration::days(30);
        }
        if expiries.is_empty() {
            expiries.push(end_date);
        }
    }

    expiries
}

/// Calculates the 2D volatility surface grid.
pub fn compute_volatility_surface(
    spot: f64,
    strikes: &[f64],
    expirations: &[NaiveDate],
    today: NaiveDate,
    risk_free_rate: f64,
    dividend_yield: f64,
) -> Vec<VolSurfacePoint> {
    let mut surface = Vec::with_capacity(expirations.len());

    for &exp_date in expirations {
        let days_to_exp = (exp_date - today).num_days().max(1) as f64;
        let tau = (days_to_exp / 365.25).max(1.0 / 365.25);
        let term_factor = 0.04 / tau.sqrt();

        let mut ivs = Vec::with_capacity(strikes.len());
        for &strike in strikes {
            let moneyness = (strike / spot).ln();
            // Parametric volatility smile & skew model:
            // sigma(K, tau) = base + term_decay + smile*m^2 - skew*m
            let iv = (0.22 + term_factor + 0.18 * moneyness * moneyness - 0.09 * moneyness
                + (risk_free_rate - dividend_yield) * 0.1)
                .clamp(0.05, 2.0);

            ivs.push((iv * 10000.0).round() / 10000.0);
        }

        surface.push(VolSurfacePoint {
            expiration: exp_date.format("%Y-%m-%d").to_string(),
            ivs,
        });
    }

    surface
}

/// Handler for `GET /options/vol-surface`
#[utoipa::path(
    get,
    path = "/options/vol-surface",
    params(
        OptionsVolSurfaceParams
    ),
    responses(
        (status = 200, description = "Options 2D volatility surface grid calculated successfully", body = OptionsVolSurfaceResponse),
        (status = 400, description = "Invalid query parameters (ticker format, invalid dates, strike range bounds, or rates out of range)"),
        (status = 401, description = "Missing or invalid Bearer JWT / API Key authentication"),
        (status = 429, description = "Rate limit capacity exceeded")
    ),
    security(
        ("bearerAuth" = [])
    ),
    tag = "Options & Derivatives"
)]
pub async fn get_options_vol_surface_handler(
    State(_state): State<AppState>,
    Query(params): Query<OptionsVolSurfaceParams>,
) -> Result<Json<OptionsVolSurfaceResponse>, (StatusCode, Json<serde_json::Value>)> {
    // 1. Validate ticker
    let safe_ticker = QuestDbClient::validate_and_escape_ticker(&params.ticker).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Invalid Parameter",
                "message": format!("Validation failed for field 'ticker': {}", e),
                "field": "ticker"
            })),
        )
    })?;

    let today = Utc::now().date_naive();

    // 2. Validate start_date
    let start_date = if let Some(ref s) = params.start_date {
        NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d").map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Invalid Parameter",
                    "message": format!("Invalid start_date format '{}', expected YYYY-MM-DD: {}", s, e),
                    "field": "start_date"
                })),
            )
        })?
    } else {
        today
    };

    // 3. Point-in-Time (PIT) delisted security check
    if !GLOBAL_PIT_DATA.is_valid_ticker(&safe_ticker, start_date) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Point-in-Time Security Invalidation",
                "message": format!("Ticker '{}' was not active or listed on '{}' (Point-in-Time check failed)", safe_ticker, start_date),
                "ticker": safe_ticker
            })),
        ));
    }

    // 4. Validate end_date
    let end_date = if let Some(ref e_str) = params.end_date {
        NaiveDate::parse_from_str(e_str.trim(), "%Y-%m-%d").map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Invalid Parameter",
                    "message": format!("Invalid end_date format '{}', expected YYYY-MM-DD: {}", e_str, e),
                    "field": "end_date"
                })),
            )
        })?
    } else {
        today + ChronoDuration::days(180)
    };

    if start_date > end_date {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Invalid Parameter",
                "message": format!("start_date ({}) cannot be after end_date ({})", start_date, end_date),
                "field": "start_date"
            })),
        ));
    }

    if (end_date - today).num_days() > 730 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Invalid Parameter",
                "message": "end_date cannot exceed 2 years from today",
                "field": "end_date"
            })),
        ));
    }

    // 5. Validate strike_range
    let raw_range = params.strike_range.as_deref().unwrap_or("0.8-1.2").trim();
    let parts: Vec<&str> = raw_range.split('-').collect();
    if parts.len() != 2 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Invalid Parameter",
                "message": format!("Invalid strike_range format '{}', expected 'min_mult-max_mult' (e.g. '0.8-1.2')", raw_range),
                "field": "strike_range"
            })),
        ));
    }

    let min_mult: f64 = parts[0].trim().parse().map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Invalid Parameter",
                "message": format!("Invalid lower bound in strike_range '{}'", parts[0]),
                "field": "strike_range"
            })),
        )
    })?;

    let max_mult: f64 = parts[1].trim().parse().map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Invalid Parameter",
                "message": format!("Invalid upper bound in strike_range '{}'", parts[1]),
                "field": "strike_range"
            })),
        )
    })?;

    if min_mult <= 0.0 || max_mult <= min_mult || max_mult > 3.0 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Invalid Parameter",
                "message": format!("strike_range multipliers must satisfy 0 < min < max <= 3.0, got '{}-{}'", min_mult, max_mult),
                "field": "strike_range"
            })),
        ));
    }

    // 6. Validate strike_count
    let strike_count = params.strike_count.unwrap_or(9);
    if !(3..=15).contains(&strike_count) || strike_count % 2 == 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Invalid Parameter",
                "message": format!("strike_count must be an odd integer between 3 and 15 inclusive (3, 5, 7, 9, 11, 13, 15), got {}", strike_count),
                "field": "strike_count"
            })),
        ));
    }

    // 7. Validate risk_free_rate
    let r = params.risk_free_rate.unwrap_or(0.05);
    if !(0.0..=0.20).contains(&r) || r.is_nan() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Invalid Parameter",
                "message": format!("risk_free_rate must be between 0.0 and 0.20 (0% to 20%), got {}", r),
                "field": "risk_free_rate"
            })),
        ));
    }

    // 8. Validate dividend_yield
    let q = params.dividend_yield.unwrap_or(0.0);
    if !(0.0..=0.10).contains(&q) || q.is_nan() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Invalid Parameter",
                "message": format!("dividend_yield must be between 0.0 and 0.10 (0% to 10%), got {}", q),
                "field": "dividend_yield"
            })),
        ));
    }

    // 9. Resolve underlying spot price
    let spot = resolve_mock_spot_price(&safe_ticker);

    // 10. Generate strikes & expirations grids
    let strikes = generate_strike_grid(spot, min_mult, max_mult, strike_count);
    let expirations_dates = generate_expirations_schedule(start_date, end_date);
    let expirations: Vec<String> = expirations_dates
        .iter()
        .map(|d| d.format("%Y-%m-%d").to_string())
        .collect();

    // 11. Compute 2D Volatility Surface
    let surface = compute_volatility_surface(spot, &strikes, &expirations_dates, today, r, q);

    Ok(Json(OptionsVolSurfaceResponse {
        ticker: safe_ticker,
        spot,
        generated_at: Utc::now().to_rfc3339(),
        strikes,
        expirations,
        surface,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_strike_grid() {
        let spot = 200.0;
        let strikes = generate_strike_grid(spot, 0.8, 1.2, 5);
        assert_eq!(strikes.len(), 5);
        assert_eq!(strikes[0], 160.0);
        assert_eq!(strikes[2], 200.0);
        assert_eq!(strikes[4], 240.0);
    }

    #[test]
    fn test_find_third_friday() {
        let fri = find_third_friday(2025, 4).unwrap();
        assert_eq!(fri, NaiveDate::from_ymd_opt(2025, 4, 18).unwrap());
    }

    #[test]
    fn test_compute_volatility_surface_bounds() {
        let spot = 100.0;
        let strikes = vec![80.0, 90.0, 100.0, 110.0, 120.0];
        let today = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
        let exp = vec![
            NaiveDate::from_ymd_opt(2025, 1, 17).unwrap(),
            NaiveDate::from_ymd_opt(2025, 2, 21).unwrap(),
        ];
        let surface = compute_volatility_surface(spot, &strikes, &exp, today, 0.05, 0.0);

        assert_eq!(surface.len(), 2);
        for row in surface {
            assert_eq!(row.ivs.len(), 5);
            for &iv in &row.ivs {
                assert!(iv >= 0.05 && iv <= 2.0);
            }
            // OTM puts (strike 80) should have higher IV than ATM (strike 100) due to skew
            assert!(row.ivs[0] > row.ivs[2]);
        }
    }
}
