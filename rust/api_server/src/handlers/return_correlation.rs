//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Return Correlation Matrix Handler
//! ═══════════════════════════════════════════════════════════════════════════════
//!
//! Calculates pairwise Pearson return correlations of daily stock returns across
//! a portfolio or universe of securities over a specified historical window.
//! ═══════════════════════════════════════════════════════════════════════════════

use axum::extract::Query;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use chrono::{NaiveDate, Utc};
use once_cell::sync::Lazy;
use std::collections::{HashMap, HashSet};
use tracing::info;

use crate::auth::AuthErrorResponse;
use crate::handlers::backtest::generate_mock_stock_prices;
use crate::models::{ReturnCorrelationItem, ReturnCorrelationParams, ReturnCorrelationResponse};
use crate::pit::GLOBAL_PIT_DATA;
use crate::storage::{QuestDbClient, QuestDbClientConfig};

static QUESTDB_CLIENT: Lazy<QuestDbClient> =
    Lazy::new(|| QuestDbClient::new(QuestDbClientConfig::default()));

pub const DEFAULT_MIN_PERIODS: usize = 20;
pub const MIN_PERIODS_LOWER: usize = 10;
pub const MAX_PERIODS_UPPER: usize = 1000;
pub const MAX_TICKERS_LIMIT: usize = 50;
pub const MAX_LOOKBACK_DAYS_LIMIT: i64 = 730; // 2 years maximum

/// Computes the Pearson correlation coefficient between two aligned numeric series.
///
/// Returns `None` if sample size < 2, or if either series has zero variance.
pub fn compute_pearson(x: &[f64], y: &[f64]) -> Option<f64> {
    if x.len() != y.len() || x.len() < 2 {
        return None;
    }

    let n = x.len() as f64;
    let mean_x = x.iter().sum::<f64>() / n;
    let mean_y = y.iter().sum::<f64>() / n;

    let mut cov = 0.0;
    let mut var_x = 0.0;
    let mut var_y = 0.0;

    for (&xi, &yi) in x.iter().zip(y.iter()) {
        let dx = xi - mean_x;
        let dy = yi - mean_y;
        cov += dx * dy;
        var_x += dx * dx;
        var_y += dy * dy;
    }

    if var_x < 1e-12 || var_y < 1e-12 {
        return None;
    }

    let denom = (var_x * var_y).sqrt();
    if denom < 1e-12 {
        return None;
    }

    let r = cov / denom;
    Some(r.clamp(-1.0, 1.0))
}

/// Compute daily simple returns: r_t = (P_t - P_{t-1}) / P_{t-1}.
pub fn compute_daily_returns(prices: &HashMap<NaiveDate, f64>) -> HashMap<NaiveDate, f64> {
    let mut sorted_dates: Vec<NaiveDate> = prices.keys().copied().collect();
    sorted_dates.sort();

    let mut returns = HashMap::with_capacity(sorted_dates.len().saturating_sub(1));
    for window in sorted_dates.windows(2) {
        let prev_date = window[0];
        let curr_date = window[1];

        if let (Some(&p_prev), Some(&p_curr)) = (prices.get(&prev_date), prices.get(&curr_date)) {
            if p_prev > 0.0 {
                let r = (p_curr - p_prev) / p_prev;
                returns.insert(curr_date, r);
            }
        }
    }

    returns
}

/// Query Daily Return Correlation Matrix.
///
/// Computes the Pearson correlation matrix of daily stock returns across an
/// arbitrary universe of tickers (up to 50) over a user-specified date range.
#[utoipa::path(
    get,
    path = "/market/correlation",
    tag = "Market Intelligence",
    params(
        ("tickers" = String, Query, description = "Comma-separated list of ticker symbols (max 50, e.g. 'AAPL,MSFT,NVDA,AMZN')"),
        ("start_date" = String, Query, description = "Start date for correlation window (YYYY-MM-DD)"),
        ("end_date" = String, Query, description = "End date for correlation window (YYYY-MM-DD)"),
        ("min_periods" = Option<usize>, Query, description = "Minimum overlapping return periods required for valid correlation (default: 20, min: 10)"),
        ("include_self" = Option<bool>, Query, description = "Include self-correlation pairs (1.0) in the output matrix (default: false)")
    ),
    responses(
        (status = 200, description = "Return correlation matrix computed successfully", body = ReturnCorrelationResponse),
        (status = 400, description = "Invalid ticker format, exceeded ticker limit, invalid date range, or bounds violation", body = AuthErrorResponse),
        (status = 401, description = "Unauthorized (missing or invalid Bearer JWT or API key)", body = AuthErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::rate_limit::RateLimitErrorResponse)
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn get_return_correlation_handler(
    Query(params): Query<ReturnCorrelationParams>,
) -> Response {
    // 1. Validate and Parse Tickers
    let raw_ticker_str = params.tickers.trim();
    if raw_ticker_str.is_empty() {
        let err = AuthErrorResponse {
            error: "Bad Request".to_string(),
            message: "Parameter 'tickers' must not be empty".to_string(),
        };
        return (StatusCode::BAD_REQUEST, Json(err)).into_response();
    }

    let mut tickers = Vec::new();
    let mut seen = HashSet::new();

    for t in raw_ticker_str.split(',') {
        let clean = t.trim().to_uppercase();
        if clean.is_empty() {
            continue;
        }
        if !seen.insert(clean.clone()) {
            continue;
        }

        match QuestDbClient::validate_and_escape_ticker(&clean) {
            Ok(valid) => tickers.push(valid),
            Err(e) => {
                let err = AuthErrorResponse {
                    error: "Bad Request".to_string(),
                    message: format!("Invalid ticker '{}': {}", clean, e),
                };
                return (StatusCode::BAD_REQUEST, Json(err)).into_response();
            }
        }
    }

    if tickers.is_empty() {
        let err = AuthErrorResponse {
            error: "Bad Request".to_string(),
            message: "No valid ticker symbols found in 'tickers' parameter".to_string(),
        };
        return (StatusCode::BAD_REQUEST, Json(err)).into_response();
    }

    if tickers.len() > MAX_TICKERS_LIMIT {
        let err = AuthErrorResponse {
            error: "Bad Request".to_string(),
            message: format!(
                "Requested universe size ({} tickers) exceeds maximum allowed limit of {} tickers",
                tickers.len(),
                MAX_TICKERS_LIMIT
            ),
        };
        return (StatusCode::BAD_REQUEST, Json(err)).into_response();
    }

    // 2. Validate Dates
    let start_date = match NaiveDate::parse_from_str(params.start_date.trim(), "%Y-%m-%d") {
        Ok(d) => d,
        Err(e) => {
            let err = AuthErrorResponse {
                error: "Bad Request".to_string(),
                message: format!(
                    "Invalid start_date format '{}', expected YYYY-MM-DD: {}",
                    params.start_date, e
                ),
            };
            return (StatusCode::BAD_REQUEST, Json(err)).into_response();
        }
    };

    let end_date = match NaiveDate::parse_from_str(params.end_date.trim(), "%Y-%m-%d") {
        Ok(d) => d,
        Err(e) => {
            let err = AuthErrorResponse {
                error: "Bad Request".to_string(),
                message: format!(
                    "Invalid end_date format '{}', expected YYYY-MM-DD: {}",
                    params.end_date, e
                ),
            };
            return (StatusCode::BAD_REQUEST, Json(err)).into_response();
        }
    };

    if start_date > end_date {
        let err = AuthErrorResponse {
            error: "Bad Request".to_string(),
            message: format!(
                "start_date '{}' cannot be after end_date '{}'",
                params.start_date, params.end_date
            ),
        };
        return (StatusCode::BAD_REQUEST, Json(err)).into_response();
    }

    let date_range_days = (end_date - start_date).num_days();
    if date_range_days > MAX_LOOKBACK_DAYS_LIMIT {
        let err = AuthErrorResponse {
            error: "Bad Request".to_string(),
            message: format!(
                "Date range ({} days) exceeds maximum allowed window of {} days (2 years)",
                date_range_days, MAX_LOOKBACK_DAYS_LIMIT
            ),
        };
        return (StatusCode::BAD_REQUEST, Json(err)).into_response();
    }

    // 3. Validate min_periods
    let min_periods = params.min_periods.unwrap_or(DEFAULT_MIN_PERIODS);
    if min_periods < MIN_PERIODS_LOWER || min_periods > MAX_PERIODS_UPPER {
        let err = AuthErrorResponse {
            error: "Bad Request".to_string(),
            message: format!(
                "min_periods must be between {} and {} (received: {})",
                MIN_PERIODS_LOWER, MAX_PERIODS_UPPER, min_periods
            ),
        };
        return (StatusCode::BAD_REQUEST, Json(err)).into_response();
    }

    let include_self = params.include_self.unwrap_or(false);

    // 4. Point-in-Time (PIT) Validation across requested tickers
    let pit_data = GLOBAL_PIT_DATA.clone();
    if pit_data.is_enabled() {
        for t in &tickers {
            if !pit_data.is_valid_ticker(t, start_date) {
                let err = AuthErrorResponse {
                    error: "Bad Request".to_string(),
                    message: format!(
                        "Ticker '{}' was not active or listed on start_date '{}'",
                        t, params.start_date
                    ),
                };
                return (StatusCode::BAD_REQUEST, Json(err)).into_response();
            }
        }
    }

    let is_mock = crate::state::is_questdb_mock_fallback_enabled()
        || crate::state::is_polygon_mock_fallback_enabled();

    let start_str = start_date.format("%Y-%m-%d").to_string();
    let end_str = end_date.format("%Y-%m-%d").to_string();

    // 5. Ingest Historical Daily Stock Prices
    let ticker_prices: HashMap<String, HashMap<NaiveDate, f64>> = if is_mock {
        let mut prices = HashMap::new();
        for t in &tickers {
            prices.insert(
                t.clone(),
                generate_mock_stock_prices(t, start_date, end_date),
            );
        }
        prices
    } else {
        match QUESTDB_CLIENT
            .query_stock_prices(&tickers, &start_str, &end_str)
            .await
        {
            Ok(mut prices) => {
                // Ensure all requested tickers have prices
                for t in &tickers {
                    if !prices.contains_key(t)
                        || prices.get(t).map(|m| m.is_empty()).unwrap_or(true)
                    {
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
                        prices.insert(
                            t.clone(),
                            generate_mock_stock_prices(t, start_date, end_date),
                        );
                    }
                }
                prices
            }
            Err(e) => {
                if crate::state::is_production_mode() {
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
                info!("QuestDB query_stock_prices fallback to mock data: {}", e);
                let mut prices = HashMap::new();
                for t in &tickers {
                    prices.insert(
                        t.clone(),
                        generate_mock_stock_prices(t, start_date, end_date),
                    );
                }
                prices
            }
        }
    };

    // 6. Compute Daily Returns per Ticker
    let mut ticker_returns: HashMap<String, HashMap<NaiveDate, f64>> =
        HashMap::with_capacity(tickers.len());
    for t in &tickers {
        if let Some(prices) = ticker_prices.get(t) {
            ticker_returns.insert(t.clone(), compute_daily_returns(prices));
        } else {
            ticker_returns.insert(t.clone(), HashMap::new());
        }
    }

    // 7. Compute Pairwise Correlations
    let mut matrix = Vec::new();
    let n = tickers.len();

    for i in 0..n {
        let t_a = &tickers[i];
        let returns_a = ticker_returns.get(t_a);

        if include_self {
            let periods = returns_a.map(|m| m.len()).unwrap_or(0);
            let corr = if periods >= min_periods {
                Some(1.0)
            } else {
                None
            };
            matrix.push(ReturnCorrelationItem {
                ticker_a: t_a.clone(),
                ticker_b: t_a.clone(),
                correlation: corr,
                periods,
            });
        }

        for j in (i + 1)..n {
            let t_b = &tickers[j];
            let returns_b = ticker_returns.get(t_b);

            let (corr, periods) = match (returns_a, returns_b) {
                (Some(ra), Some(rb)) => {
                    // Find common dates
                    let mut common_dates: Vec<NaiveDate> =
                        ra.keys().filter(|d| rb.contains_key(d)).copied().collect();
                    common_dates.sort();

                    let count = common_dates.len();
                    if count < min_periods {
                        (None, count)
                    } else {
                        let x: Vec<f64> = common_dates.iter().map(|d| ra[d]).collect();
                        let y: Vec<f64> = common_dates.iter().map(|d| rb[d]).collect();

                        let p_corr =
                            compute_pearson(&x, &y).map(|c| (c * 10000.0).round() / 10000.0);
                        (p_corr, count)
                    }
                }
                _ => (None, 0),
            };

            matrix.push(ReturnCorrelationItem {
                ticker_a: t_a.clone(),
                ticker_b: t_b.clone(),
                correlation: corr,
                periods,
            });
        }
    }

    info!(
        "[ReturnCorrelation] Computed correlation matrix for {} tickers ({} pairs, window: {} to {})",
        tickers.len(), matrix.len(), start_str, end_str
    );

    let response = ReturnCorrelationResponse {
        tickers,
        start_date: start_str,
        end_date: end_str,
        min_periods,
        matrix,
        generated_at: Utc::now().to_rfc3339(),
    };

    (StatusCode::OK, Json(response)).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_pearson_math() {
        // Perfect positive correlation
        let x = vec![0.01, 0.02, 0.03, 0.04, 0.05];
        let y = vec![0.02, 0.04, 0.06, 0.08, 0.10];
        let corr = compute_pearson(&x, &y);
        assert!(corr.is_some());
        assert!((corr.unwrap() - 1.0).abs() < 1e-6);

        // Perfect negative correlation
        let y_neg = vec![-0.02, -0.04, -0.06, -0.08, -0.10];
        let corr_neg = compute_pearson(&x, &y_neg);
        assert!(corr_neg.is_some());
        assert!((corr_neg.unwrap() - (-1.0)).abs() < 1e-6);

        // Zero variance returns None
        let x_const = vec![0.05, 0.05, 0.05, 0.05];
        let y_const = vec![0.01, 0.02, 0.03, 0.04];
        assert!(compute_pearson(&x_const, &y_const).is_none());

        // Insufficient data (< 2)
        assert!(compute_pearson(&[0.01], &[0.02]).is_none());
    }

    #[test]
    fn test_compute_daily_returns() {
        let mut prices = HashMap::new();
        let d1 = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
        let d2 = NaiveDate::from_ymd_opt(2025, 1, 2).unwrap();
        let d3 = NaiveDate::from_ymd_opt(2025, 1, 3).unwrap();

        prices.insert(d1, 100.0);
        prices.insert(d2, 110.0); // +10%
        prices.insert(d3, 121.0); // +10%

        let returns = compute_daily_returns(&prices);
        assert_eq!(returns.len(), 2);
        assert!((returns[&d2] - 0.10).abs() < 1e-6);
        assert!((returns[&d3] - 0.10).abs() < 1e-6);
    }
}
