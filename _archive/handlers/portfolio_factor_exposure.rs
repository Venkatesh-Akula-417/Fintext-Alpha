//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Portfolio Factor Exposure & Risk Attribution Engine
//! ═══════════════════════════════════════════════════════════════════════════════

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use chrono::{NaiveDate, Utc};
use once_cell::sync::Lazy;
use std::collections::{HashMap, HashSet};
use tracing::info;

use crate::auth::AuthErrorResponse;
use crate::handlers::backtest::generate_mock_stock_prices;
use crate::handlers::factor_exposure::{
    compute_rolling_momentum_series, compute_rolling_volatility_series, compute_size_proxy_series,
    compute_value_proxy_series, generate_deterministic_sentiment_series, solve_ols_multiple,
    ALLOWED_FACTORS, DEFAULT_BENCHMARK, DEFAULT_FACTORS, MAX_LOOKBACK_DAYS_LIMIT,
    MIN_OBSERVATIONS_REQUIRED,
};
use crate::handlers::return_correlation::compute_daily_returns;
use crate::models::{
    FactorExposureItem, OLSStatistics, PortfolioFactorExposureRequest,
    PortfolioFactorExposureResponse,
};
use crate::pit::GLOBAL_PIT_DATA;
use crate::storage::{QuestDbClient, QuestDbClientConfig};

static QUESTDB_CLIENT: Lazy<QuestDbClient> =
    Lazy::new(|| QuestDbClient::new(QuestDbClientConfig::default()));

pub const MIN_TICKERS: usize = 2;
pub const MAX_TICKERS: usize = 20;
pub const MIN_WEIGHT_SUM: f64 = 0.95;
pub const MAX_WEIGHT_SUM: f64 = 1.05;

/// POST /risk/portfolio-factor-exposure
///
/// Computes quantitative multi-factor risk exposures (Betas, t-stats, p-values, R^2)
/// for a portfolio of constituent assets via multiple OLS regression against common risk factors
/// (Market, Momentum, Sentiment, Volatility, Size, Value).
#[utoipa::path(
    post,
    path = "/risk/portfolio-factor-exposure",
    tag = "Risk & Factor Analytics",
    request_body = PortfolioFactorExposureRequest,
    responses(
        (status = 200, description = "Portfolio factor exposure regression computed successfully", body = PortfolioFactorExposureResponse),
        (status = 400, description = "Invalid tickers, weights, date format, unsupported factors, or insufficient observations", body = AuthErrorResponse),
        (status = 401, description = "Unauthorized - Missing or invalid Bearer JWT"),
        (status = 429, description = "Rate limit exceeded")
    ),
    security(("BearerAuth" = []))
)]
pub async fn portfolio_factor_exposure_handler(
    Json(payload): Json<PortfolioFactorExposureRequest>,
) -> Response {
    // 1. Validate Tickers
    let raw_tickers = &payload.tickers;
    if raw_tickers.len() < MIN_TICKERS {
        let err = AuthErrorResponse {
            error: "Bad Request".to_string(),
            message: format!(
                "Portfolio must contain at least {} tickers (got {})",
                MIN_TICKERS,
                raw_tickers.len()
            ),
        };
        return (StatusCode::BAD_REQUEST, Json(err)).into_response();
    }
    if raw_tickers.len() > MAX_TICKERS {
        let err = AuthErrorResponse {
            error: "Bad Request".to_string(),
            message: format!(
                "Portfolio cannot contain more than {} tickers (got {})",
                MAX_TICKERS,
                raw_tickers.len()
            ),
        };
        return (StatusCode::BAD_REQUEST, Json(err)).into_response();
    }

    let mut clean_tickers = Vec::with_capacity(raw_tickers.len());
    let mut seen_tickers = HashSet::new();

    for t in raw_tickers {
        let clean = t.trim().to_uppercase();
        if clean.is_empty() {
            let err = AuthErrorResponse {
                error: "Bad Request".to_string(),
                message: "Constituent ticker cannot be empty".to_string(),
            };
            return (StatusCode::BAD_REQUEST, Json(err)).into_response();
        }
        match QuestDbClient::validate_and_escape_ticker(&clean) {
            Ok(validated) => {
                if !seen_tickers.insert(validated.clone()) {
                    let err = AuthErrorResponse {
                        error: "Bad Request".to_string(),
                        message: format!("Duplicate ticker '{}' found in portfolio", validated),
                    };
                    return (StatusCode::BAD_REQUEST, Json(err)).into_response();
                }
                clean_tickers.push(validated);
            }
            Err(e) => {
                let err = AuthErrorResponse {
                    error: "Bad Request".to_string(),
                    message: format!("Invalid constituent ticker '{}': {}", clean, e),
                };
                return (StatusCode::BAD_REQUEST, Json(err)).into_response();
            }
        }
    }

    // 2. Validate Weights
    let raw_weights = &payload.weights;
    if raw_weights.len() != clean_tickers.len() {
        let err = AuthErrorResponse {
            error: "Bad Request".to_string(),
            message: format!(
                "Length of weights array ({}) must match tickers array ({})",
                raw_weights.len(),
                clean_tickers.len()
            ),
        };
        return (StatusCode::BAD_REQUEST, Json(err)).into_response();
    }

    for (idx, &w) in raw_weights.iter().enumerate() {
        if w.is_nan() || w.is_infinite() {
            let err = AuthErrorResponse {
                error: "Bad Request".to_string(),
                message: format!(
                    "Weight at index {} for ticker '{}' is not a valid finite number",
                    idx, clean_tickers[idx]
                ),
            };
            return (StatusCode::BAD_REQUEST, Json(err)).into_response();
        }
    }

    let weight_sum: f64 = raw_weights.iter().sum();
    if weight_sum < MIN_WEIGHT_SUM || weight_sum > MAX_WEIGHT_SUM {
        let err = AuthErrorResponse {
            error: "Bad Request".to_string(),
            message: format!(
                "Sum of portfolio weights ({:.4}) must be within [{:.2}, {:.2}] (1.0 ± 0.05)",
                weight_sum, MIN_WEIGHT_SUM, MAX_WEIGHT_SUM
            ),
        };
        return (StatusCode::BAD_REQUEST, Json(err)).into_response();
    }

    // Normalize weights to sum strictly to 1.0
    let normalized_weights: Vec<f64> = raw_weights.iter().map(|&w| w / weight_sum).collect();

    // 3. Validate Benchmark Ticker
    let raw_benchmark = payload
        .benchmark_ticker
        .as_deref()
        .unwrap_or(DEFAULT_BENCHMARK)
        .trim()
        .to_uppercase();
    let benchmark_ticker = if raw_benchmark.is_empty() {
        DEFAULT_BENCHMARK.to_string()
    } else {
        match QuestDbClient::validate_and_escape_ticker(&raw_benchmark) {
            Ok(b) => b,
            Err(e) => {
                let err = AuthErrorResponse {
                    error: "Bad Request".to_string(),
                    message: format!("Invalid benchmark_ticker '{}': {}", raw_benchmark, e),
                };
                return (StatusCode::BAD_REQUEST, Json(err)).into_response();
            }
        }
    };

    // 4. Validate Dates
    let start_date = match NaiveDate::parse_from_str(payload.start_date.trim(), "%Y-%m-%d") {
        Ok(d) => d,
        Err(e) => {
            let err = AuthErrorResponse {
                error: "Bad Request".to_string(),
                message: format!(
                    "Invalid start_date format '{}', expected YYYY-MM-DD: {}",
                    payload.start_date, e
                ),
            };
            return (StatusCode::BAD_REQUEST, Json(err)).into_response();
        }
    };

    let end_date = match NaiveDate::parse_from_str(payload.end_date.trim(), "%Y-%m-%d") {
        Ok(d) => d,
        Err(e) => {
            let err = AuthErrorResponse {
                error: "Bad Request".to_string(),
                message: format!(
                    "Invalid end_date format '{}', expected YYYY-MM-DD: {}",
                    payload.end_date, e
                ),
            };
            return (StatusCode::BAD_REQUEST, Json(err)).into_response();
        }
    };

    if end_date < start_date {
        let err = AuthErrorResponse {
            error: "Bad Request".to_string(),
            message: format!(
                "end_date ('{}') must be on or after start_date ('{}')",
                payload.end_date, payload.start_date
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

    // 5. Validate Factors
    let raw_factors_str = payload.factors.as_deref().unwrap_or(DEFAULT_FACTORS).trim();
    if raw_factors_str.is_empty() {
        let err = AuthErrorResponse {
            error: "Bad Request".to_string(),
            message: "Parameter 'factors' cannot be empty".to_string(),
        };
        return (StatusCode::BAD_REQUEST, Json(err)).into_response();
    }

    let mut factors_included = Vec::new();
    let mut seen_factors = HashSet::new();

    for f in raw_factors_str.split(',') {
        let clean = f.trim().to_lowercase();
        if clean.is_empty() {
            continue;
        }
        if !ALLOWED_FACTORS.contains(&clean.as_str()) {
            let err = AuthErrorResponse {
                error: "Bad Request".to_string(),
                message: format!(
                    "Unsupported factor '{}'. Allowed factors: {:?}",
                    clean, ALLOWED_FACTORS
                ),
            };
            return (StatusCode::BAD_REQUEST, Json(err)).into_response();
        }
        if seen_factors.insert(clean.clone()) {
            factors_included.push(clean);
        }
    }

    if factors_included.is_empty() {
        let err = AuthErrorResponse {
            error: "Bad Request".to_string(),
            message: "No valid risk factors specified".to_string(),
        };
        return (StatusCode::BAD_REQUEST, Json(err)).into_response();
    }

    // 6. Point-in-Time validation
    let pit_data = GLOBAL_PIT_DATA.clone();
    if pit_data.is_enabled() {
        for t in &clean_tickers {
            if !pit_data.is_valid_ticker(t, start_date) {
                let err = AuthErrorResponse {
                    error: "Bad Request".to_string(),
                    message: format!(
                        "Ticker '{}' was not active or listed on start_date '{}'",
                        t, payload.start_date
                    ),
                };
                return (StatusCode::BAD_REQUEST, Json(err)).into_response();
            }
        }
        if !pit_data.is_valid_ticker(&benchmark_ticker, start_date) {
            let err = AuthErrorResponse {
                error: "Bad Request".to_string(),
                message: format!(
                    "Benchmark ticker '{}' was not active or listed on start_date '{}'",
                    benchmark_ticker, payload.start_date
                ),
            };
            return (StatusCode::BAD_REQUEST, Json(err)).into_response();
        }
    }

    let is_mock = crate::state::is_questdb_mock_fallback_enabled()
        || crate::state::is_polygon_mock_fallback_enabled();

    let start_str = start_date.format("%Y-%m-%d").to_string();
    let end_str = end_date.format("%Y-%m-%d").to_string();

    // 7. Fetch historical daily price bars for all constituents & benchmark
    let mut requested_tickers = clean_tickers.clone();
    if !requested_tickers.contains(&benchmark_ticker) {
        requested_tickers.push(benchmark_ticker.clone());
    }

    let ticker_prices: HashMap<String, HashMap<NaiveDate, f64>> = if is_mock {
        let mut prices = HashMap::new();
        for t in &requested_tickers {
            prices.insert(
                t.clone(),
                generate_mock_stock_prices(t, start_date, end_date),
            );
        }
        prices
    } else {
        match QUESTDB_CLIENT
            .query_stock_prices(&requested_tickers, &start_str, &end_str)
            .await
        {
            Ok(mut prices) => {
                for t in &requested_tickers {
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
                for t in &requested_tickers {
                    prices.insert(
                        t.clone(),
                        generate_mock_stock_prices(t, start_date, end_date),
                    );
                }
                prices
            }
        }
    };

    // 8. Compute Daily Returns for each asset and benchmark
    let empty_map = HashMap::new();
    let mut asset_returns_maps: Vec<HashMap<NaiveDate, f64>> =
        Vec::with_capacity(clean_tickers.len());
    for t in &clean_tickers {
        let prices = ticker_prices.get(t).unwrap_or(&empty_map);
        asset_returns_maps.push(compute_daily_returns(prices));
    }

    let benchmark_prices = ticker_prices.get(&benchmark_ticker).unwrap_or(&empty_map);
    let benchmark_returns_map = compute_daily_returns(benchmark_prices);

    // Common trading dates across all assets and benchmark
    let mut common_dates: Vec<NaiveDate> = if let Some(first_map) = asset_returns_maps.first() {
        first_map
            .keys()
            .copied()
            .filter(|d| {
                benchmark_returns_map.contains_key(d)
                    && asset_returns_maps.iter().all(|m| m.contains_key(d))
            })
            .collect()
    } else {
        Vec::new()
    };
    common_dates.sort();

    let num_obs = common_dates.len();
    if num_obs < MIN_OBSERVATIONS_REQUIRED {
        let err = AuthErrorResponse {
            error: "Bad Request".to_string(),
            message: format!(
                "Insufficient overlapping trading days (got {}, minimum required: {}) for portfolio factor exposure regression",
                num_obs, MIN_OBSERVATIONS_REQUIRED
            ),
        };
        return (StatusCode::BAD_REQUEST, Json(err)).into_response();
    }

    // 9. Compute Daily Portfolio Return Series: R_portfolio_t = Σ w_i * r_i_t
    let mut portfolio_returns = Vec::with_capacity(num_obs);
    let mut constituent_return_series: Vec<Vec<f64>> =
        vec![Vec::with_capacity(num_obs); clean_tickers.len()];
    let mut market_returns = Vec::with_capacity(num_obs);

    for d in &common_dates {
        let mut port_ret_day = 0.0;
        for (i, ret_map) in asset_returns_maps.iter().enumerate() {
            let r = ret_map[d];
            constituent_return_series[i].push(r);
            port_ret_day += normalized_weights[i] * r;
        }
        portfolio_returns.push(port_ret_day);
        market_returns.push(benchmark_returns_map[d]);
    }

    // 10. Construct Factor Series
    // Momentum: weighted sum of constituent 20-day momentum series
    let mut portfolio_momentum_series = vec![0.0; num_obs];
    for (i, r_series) in constituent_return_series.iter().enumerate() {
        let ind_mom = compute_rolling_momentum_series(r_series, 20);
        let w = normalized_weights[i];
        for t in 0..num_obs {
            portfolio_momentum_series[t] += w * ind_mom[t];
        }
    }

    // Sentiment: weighted sum of constituent daily sentiment series
    let mut portfolio_sentiment_series = vec![0.0; num_obs];
    for (i, t_name) in clean_tickers.iter().enumerate() {
        let ind_sent = generate_deterministic_sentiment_series(t_name, &common_dates);
        let w = normalized_weights[i];
        for t in 0..num_obs {
            portfolio_sentiment_series[t] += w * ind_sent[t];
        }
    }

    // Volatility: negative of rolling 20-day portfolio return volatility
    let portfolio_volatility_series = compute_rolling_volatility_series(&portfolio_returns, 20);

    // Size & Value proxies from market returns
    let size_series = compute_size_proxy_series(&market_returns);
    let value_series = compute_value_proxy_series(&market_returns);

    let mut factor_columns: Vec<Vec<f64>> = Vec::with_capacity(factors_included.len());

    for f in &factors_included {
        match f.as_str() {
            "market" => factor_columns.push(market_returns.clone()),
            "momentum" => factor_columns.push(portfolio_momentum_series.clone()),
            "sentiment" => factor_columns.push(portfolio_sentiment_series.clone()),
            "volatility" => factor_columns.push(portfolio_volatility_series.clone()),
            "size" => factor_columns.push(size_series.clone()),
            "value" => factor_columns.push(value_series.clone()),
            _ => unreachable!(),
        }
    }

    // 11. Execute OLS Multiple Regression
    let ols_result = match solve_ols_multiple(&portfolio_returns, &factor_columns) {
        Ok(res) => res,
        Err(err_msg) => {
            let err = AuthErrorResponse {
                error: "Bad Request".to_string(),
                message: format!("Portfolio factor regression failed: {}", err_msg),
            };
            return (StatusCode::BAD_REQUEST, Json(err)).into_response();
        }
    };

    // 12. Assemble Response DTO
    let mut exposures = Vec::with_capacity(factors_included.len());
    for (idx, f_name) in factors_included.iter().enumerate() {
        let beta = (ols_result.betas[idx] * 10000.0).round() / 10000.0;
        let t_stat = (ols_result.t_stats[idx] * 100.0).round() / 100.0;
        let p_val = (ols_result.p_values[idx] * 10000.0).round() / 10000.0;

        exposures.push(FactorExposureItem {
            factor: f_name.clone(),
            beta,
            t_stat,
            p_value: p_val,
        });
    }

    let response = PortfolioFactorExposureResponse {
        tickers: clean_tickers,
        weights: normalized_weights
            .iter()
            .map(|w| (w * 10000.0).round() / 10000.0)
            .collect(),
        start_date: start_date.format("%Y-%m-%d").to_string(),
        end_date: end_date.format("%Y-%m-%d").to_string(),
        benchmark_ticker,
        factors_included,
        ols_summary: OLSStatistics {
            r_squared: ols_result.r_squared,
            adjusted_r_squared: ols_result.adjusted_r_squared,
            num_observations: ols_result.num_observations,
            f_statistic: ols_result.f_statistic,
            p_value: ols_result.f_p_value,
        },
        exposures,
        generated_at: Utc::now().to_rfc3339(),
    };

    Json(response).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_portfolio_weighted_return_calculation() {
        let weights: Vec<f64> = vec![0.5, 0.3, 0.2];
        let asset1: Vec<f64> = vec![0.01, 0.02, -0.01];
        let asset2: Vec<f64> = vec![0.02, -0.01, 0.03];
        let asset3: Vec<f64> = vec![0.00, 0.04, -0.02];

        let mut port_returns: Vec<f64> = Vec::new();
        for t in 0..3 {
            let ret: f64 = weights[0] * asset1[t] + weights[1] * asset2[t] + weights[2] * asset3[t];
            port_returns.push(ret);
        }

        assert!((port_returns[0] - (0.5 * 0.01 + 0.3 * 0.02 + 0.2 * 0.00)).abs() < 1e-9);
        assert!((port_returns[1] - (0.5 * 0.02 + 0.3 * (-0.01) + 0.2 * 0.04)).abs() < 1e-9);
        assert!((port_returns[2] - (0.5 * (-0.01) + 0.3 * 0.03 + 0.2 * (-0.02))).abs() < 1e-9);
    }

    #[test]
    fn test_portfolio_factor_ols_regression_synthetic() {
        // Construct 50 observation days
        let n = 50;
        let mut market = Vec::with_capacity(n);
        let mut momentum = Vec::with_capacity(n);
        let mut port_returns = Vec::with_capacity(n);

        for i in 0..n {
            let m = ((i as f64) * 0.1).sin() * 0.02;
            let mom = ((i as f64) * 0.2).cos() * 0.015;
            market.push(m);
            momentum.push(mom);
            // True beta_market = 1.2, beta_mom = 0.5, alpha = 0.0005
            port_returns.push(0.0005 + 1.2 * m + 0.5 * mom);
        }

        let factor_cols = vec![market, momentum];
        let ols = solve_ols_multiple(&port_returns, &factor_cols).unwrap();

        assert!((ols.betas[0] - 1.2).abs() < 1e-3);
        assert!((ols.betas[1] - 0.5).abs() < 1e-3);
        assert!((ols.alpha - 0.0005).abs() < 1e-3);
        assert!(ols.r_squared > 0.99);
    }
}
