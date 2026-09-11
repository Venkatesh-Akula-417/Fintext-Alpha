//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Factor Exposure Report & Multi-Factor OLS Engine
//! ═══════════════════════════════════════════════════════════════════════════════

use axum::{
    extract::Query,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use chrono::{Datelike, NaiveDate, Utc};
use once_cell::sync::Lazy;
use std::collections::{HashMap, HashSet};
use tracing::info;

use crate::auth::AuthErrorResponse;
use crate::handlers::backtest::generate_mock_stock_prices;
use crate::handlers::return_correlation::compute_daily_returns;
use crate::models::{
    FactorExposureItem, FactorExposureParams, FactorExposureResponse, OLSStatistics,
};
use crate::pit::GLOBAL_PIT_DATA;
use crate::storage::{QuestDbClient, QuestDbClientConfig};

static QUESTDB_CLIENT: Lazy<QuestDbClient> =
    Lazy::new(|| QuestDbClient::new(QuestDbClientConfig::default()));

pub const DEFAULT_BENCHMARK: &str = "SPY";
pub const DEFAULT_FACTORS: &str = "market,momentum,sentiment,volatility";
pub const ALLOWED_FACTORS: &[&str] = &[
    "market",
    "momentum",
    "sentiment",
    "volatility",
    "size",
    "value",
];
pub const MIN_OBSERVATIONS_REQUIRED: usize = 10;
pub const MAX_LOOKBACK_DAYS_LIMIT: i64 = 730; // 2 years

/// Approximate standard normal Cumulative Distribution Function (CDF)
/// using Abramowitz and Stegun formula 7.1.26 (maximum error < 1.5e-7).
pub fn normal_cdf(x: f64) -> f64 {
    if x < -8.0 {
        return 0.0;
    }
    if x > 8.0 {
        return 1.0;
    }

    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let abs_x = x.abs();

    let p = 0.2316419;
    let b1 = 0.319381530;
    let b2 = -0.356563782;
    let b3 = 1.781477937;
    let b4 = -1.821255978;
    let b5 = 1.330274429;

    let t = 1.0 / (1.0 + p * abs_x);
    let phi = (-abs_x * abs_x / 2.0).exp() / (2.0 * std::f64::consts::PI).sqrt();
    let poly = t * (b1 + t * (b2 + t * (b3 + t * (b4 + t * b5))));
    let cdf_abs = 1.0 - phi * poly;

    if sign < 0.0 {
        1.0 - cdf_abs
    } else {
        cdf_abs
    }
}

/// Compute two-tailed p-value from a t-statistic using normal/Student-t approximation.
pub fn compute_two_tailed_p_value(t_stat: f64, df: f64) -> f64 {
    if df <= 0.0 || t_stat.is_nan() {
        return 1.0;
    }
    let abs_t = t_stat.abs();
    // Hill's transformation / normal approximation for Student's t
    let z = if df >= 30.0 {
        abs_t
    } else {
        // Welch approximation for small df
        abs_t * (1.0 - 1.0 / (4.0 * df))
    };

    let p = 2.0 * (1.0 - normal_cdf(z));
    p.clamp(0.0001, 1.0)
}

/// Invert a square matrix of size PxP using Gauss-Jordan elimination with partial pivoting.
pub fn invert_matrix(matrix: &[Vec<f64>]) -> Result<Vec<Vec<f64>>, String> {
    let p = matrix.len();
    if p == 0 {
        return Err("Cannot invert empty matrix".to_string());
    }
    for row in matrix {
        if row.len() != p {
            return Err("Matrix is not square".to_string());
        }
    }

    // Augmented matrix [A | I]
    let mut aug = vec![vec![0.0; 2 * p]; p];
    for r in 0..p {
        for c in 0..p {
            aug[r][c] = matrix[r][c];
        }
        aug[r][p + r] = 1.0;
    }

    for i in 0..p {
        // Find pivot
        let mut pivot_row = i;
        let mut max_val = aug[i][i].abs();
        for r in (i + 1)..p {
            if aug[r][i].abs() > max_val {
                max_val = aug[r][i].abs();
                pivot_row = r;
            }
        }

        if max_val < 1e-12 {
            return Err(format!(
                "Matrix is singular or ill-conditioned at pivot {}",
                i
            ));
        }

        if pivot_row != i {
            aug.swap(i, pivot_row);
        }

        let pivot = aug[i][i];
        for c in 0..(2 * p) {
            aug[i][c] /= pivot;
        }

        for r in 0..p {
            if r != i {
                let factor = aug[r][i];
                for c in 0..(2 * p) {
                    aug[r][c] -= factor * aug[i][c];
                }
            }
        }
    }

    let mut inv = vec![vec![0.0; p]; p];
    for r in 0..p {
        for c in 0..p {
            inv[r][c] = aug[r][p + c];
        }
    }

    Ok(inv)
}

/// Result of multiple linear OLS regression.
#[derive(Debug, Clone)]
pub struct OLSResult {
    pub alpha: f64,
    pub alpha_se: f64,
    pub alpha_t_stat: f64,
    pub alpha_p_value: f64,
    pub betas: Vec<f64>,
    pub standard_errors: Vec<f64>,
    pub t_stats: Vec<f64>,
    pub p_values: Vec<f64>,
    pub r_squared: f64,
    pub adjusted_r_squared: f64,
    pub f_statistic: f64,
    pub f_p_value: f64,
    pub num_observations: usize,
}

/// Solves Multiple Linear Regression: y = X * beta + epsilon
/// where column 0 of X is the intercept (1.0), and columns 1..K are independent factor variables.
pub fn solve_ols_multiple(y: &[f64], factor_columns: &[Vec<f64>]) -> Result<OLSResult, String> {
    let n = y.len();
    let k = factor_columns.len();
    if k == 0 {
        return Err("No factors provided for regression".to_string());
    }
    if n < k + 2 {
        return Err(format!(
            "Insufficient observations ({} points) for {} factors + intercept",
            n, k
        ));
    }
    for (idx, col) in factor_columns.iter().enumerate() {
        if col.len() != n {
            return Err(format!(
                "Factor column {} length ({}) does not match y length ({})",
                idx,
                col.len(),
                n
            ));
        }
    }

    let p = k + 1; // Intercept + K factors

    // Construct design matrix X: N x P
    let mut x = vec![vec![1.0; p]; n];
    for i in 0..n {
        for j in 0..k {
            x[i][j + 1] = factor_columns[j][i];
        }
    }

    // Compute A = X^T * X (P x P) and b = X^T * y (P x 1)
    let mut a = vec![vec![0.0; p]; p];
    let mut b = vec![0.0; p];

    for i in 0..n {
        let yi = y[i];
        for r in 0..p {
            let xr = x[i][r];
            b[r] += xr * yi;
            for c in 0..p {
                a[r][c] += xr * x[i][c];
            }
        }
    }

    // Attempt Gauss-Jordan inversion of X^T * X
    let inv_a = match invert_matrix(&a) {
        Ok(inv) => inv,
        Err(_) => {
            // Add slight Ridge Tikhonov regularization (1e-6) if singular due to collinearity
            let mut a_reg = a.clone();
            for diag in 0..p {
                a_reg[diag][diag] += 1e-6;
            }
            invert_matrix(&a_reg).map_err(|e| format!("OLS Matrix inversion failed: {}", e))?
        }
    };

    // beta_hat = (X^T * X)^-1 * (X^T * y)
    let mut beta_hat = vec![0.0; p];
    for r in 0..p {
        for c in 0..p {
            beta_hat[r] += inv_a[r][c] * b[c];
        }
    }

    // Residuals and sums of squares
    let mut ssr = 0.0;
    let sum_y: f64 = y.iter().sum();
    let mean_y = sum_y / (n as f64);
    let mut sst = 0.0;

    for i in 0..n {
        let mut y_pred = 0.0;
        for c in 0..p {
            y_pred += x[i][c] * beta_hat[c];
        }
        let e = y[i] - y_pred;
        ssr += e * e;
        let dy = y[i] - mean_y;
        sst += dy * dy;
    }

    if sst < 1e-12 {
        sst = 1e-12;
    }

    let r_squared = (1.0 - (ssr / sst)).clamp(0.0, 1.0);
    let df_res = ((n - p) as f64).max(1.0);
    let adj_r_squared = (1.0 - (1.0 - r_squared) * ((n - 1) as f64) / df_res).clamp(0.0, 1.0);

    let s2 = ssr / df_res;

    // Standard errors & t-statistics
    let mut se = vec![0.0; p];
    let mut t_stats = vec![0.0; p];
    let mut p_values = vec![0.0; p];

    for j in 0..p {
        let var_beta = (s2 * inv_a[j][j]).max(0.0);
        let std_err = var_beta.sqrt();
        se[j] = std_err;
        if std_err > 1e-12 {
            let t = beta_hat[j] / std_err;
            t_stats[j] = t;
            p_values[j] = compute_two_tailed_p_value(t, df_res);
        } else {
            t_stats[j] = 0.0;
            p_values[j] = 1.0;
        }
    }

    // Overall F-statistic: F = ((SST - SSR) / K) / (SSR / df_res)
    let ssm = (sst - ssr).max(0.0);
    let f_stat = if k > 0 && s2 > 1e-12 {
        (ssm / (k as f64)) / s2
    } else {
        0.0
    };
    let f_p_value = compute_two_tailed_p_value(f_stat.sqrt(), df_res);

    Ok(OLSResult {
        alpha: beta_hat[0],
        alpha_se: se[0],
        alpha_t_stat: t_stats[0],
        alpha_p_value: p_values[0],
        betas: beta_hat[1..].to_vec(),
        standard_errors: se[1..].to_vec(),
        t_stats: t_stats[1..].to_vec(),
        p_values: p_values[1..].to_vec(),
        r_squared: (r_squared * 10000.0).round() / 10000.0,
        adjusted_r_squared: (adj_r_squared * 10000.0).round() / 10000.0,
        f_statistic: (f_stat * 100.0).round() / 100.0,
        f_p_value: (f_p_value * 10000.0).round() / 10000.0,
        num_observations: n,
    })
}

/// Generates a deterministic daily sentiment series in [-0.8, 0.8] for a given ticker and date list.
pub fn generate_deterministic_sentiment_series(ticker: &str, dates: &[NaiveDate]) -> Vec<f64> {
    let mut scores = Vec::with_capacity(dates.len());
    let mut seed: u64 = 0;
    for b in ticker.bytes() {
        seed = seed.wrapping_mul(31).wrapping_add(b as u64);
    }

    for d in dates {
        let day_num = d.num_days_from_ce() as u64;
        let h = seed.wrapping_mul(6364136223846793005).wrapping_add(day_num);
        let val = ((h % 1000) as f64) / 500.0 - 1.0; // [-1.0, 1.0]
        scores.push((val * 0.6).clamp(-0.8, 0.8));
    }
    scores
}

/// Compute rolling 20-day cumulative momentum series.
pub fn compute_rolling_momentum_series(returns: &[f64], window: usize) -> Vec<f64> {
    let n = returns.len();
    let mut mom = Vec::with_capacity(n);
    for i in 0..n {
        let start = i.saturating_sub(window - 1);
        let sum: f64 = returns[start..=i].iter().sum();
        mom.push(sum);
    }
    mom
}

/// Compute rolling 20-day volatility series (negative of rolling standard deviation).
pub fn compute_rolling_volatility_series(returns: &[f64], window: usize) -> Vec<f64> {
    let n = returns.len();
    let mut vol = Vec::with_capacity(n);
    for i in 0..n {
        let start = i.saturating_sub(window - 1);
        let slice = &returns[start..=i];
        let count = slice.len();
        if count < 2 {
            vol.push(0.0);
            continue;
        }
        let mean = slice.iter().sum::<f64>() / (count as f64);
        let var = slice.iter().map(|&r| (r - mean).powi(2)).sum::<f64>() / ((count - 1) as f64);
        let std_dev = var.sqrt();
        // Negative so that higher volatility maps to lower factor exposure score
        vol.push(-std_dev);
    }
    vol
}

/// Compute size factor proxy series (small-cap minus large-cap proxy).
pub fn compute_size_proxy_series(market_returns: &[f64]) -> Vec<f64> {
    market_returns
        .iter()
        .map(|&rm| rm * 0.15 - 0.0002)
        .collect()
}

/// Compute value factor proxy series (high book-to-market minus low book-to-market proxy).
pub fn compute_value_proxy_series(market_returns: &[f64]) -> Vec<f64> {
    market_returns
        .iter()
        .map(|&rm| rm * (-0.10) + 0.0001)
        .collect()
}

/// GET /risk/factor-exposure
///
/// Computes quantitative factor exposures (Betas, t-stats, p-values, R^2)
/// via multiple OLS regression against common risk factors (Market, Momentum, Sentiment, Volatility, Size, Value).
#[utoipa::path(
    get,
    path = "/risk/factor-exposure",
    tag = "Risk & Factor Analytics",
    params(
        ("ticker" = String, Query, description = "Stock ticker symbol (e.g. 'AAPL', 'MSFT', 'NVDA')"),
        ("start_date" = String, Query, description = "Start date for historical estimation window (YYYY-MM-DD)"),
        ("end_date" = String, Query, description = "End date for historical estimation window (YYYY-MM-DD)"),
        ("factors" = Option<String>, Query, description = "Comma-separated list of factors (default: 'market,momentum,sentiment,volatility')"),
        ("benchmark_ticker" = Option<String>, Query, description = "Benchmark ticker for market factor (default: 'SPY')")
    ),
    responses(
        (status = 200, description = "Factor exposure regression computed successfully", body = FactorExposureResponse),
        (status = 400, description = "Invalid ticker, date format, unsupported factors, or insufficient observations", body = AuthErrorResponse),
        (status = 401, description = "Unauthorized - Missing or invalid Bearer JWT"),
        (status = 429, description = "Rate limit exceeded")
    ),
    security(("BearerAuth" = []))
)]
pub async fn get_factor_exposure_handler(Query(params): Query<FactorExposureParams>) -> Response {
    // 1. Validate Ticker
    let raw_ticker = params.ticker.trim().to_uppercase();
    if raw_ticker.is_empty() {
        let err = AuthErrorResponse {
            error: "Bad Request".to_string(),
            message: "Parameter 'ticker' must not be empty".to_string(),
        };
        return (StatusCode::BAD_REQUEST, Json(err)).into_response();
    }

    let ticker = match QuestDbClient::validate_and_escape_ticker(&raw_ticker) {
        Ok(t) => t,
        Err(e) => {
            let err = AuthErrorResponse {
                error: "Bad Request".to_string(),
                message: format!("Invalid ticker '{}': {}", raw_ticker, e),
            };
            return (StatusCode::BAD_REQUEST, Json(err)).into_response();
        }
    };

    // 2. Validate Benchmark Ticker
    let raw_benchmark = params
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

    // 3. Validate Dates
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

    if end_date < start_date {
        let err = AuthErrorResponse {
            error: "Bad Request".to_string(),
            message: format!(
                "end_date ('{}') must be on or after start_date ('{}')",
                params.end_date, params.start_date
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

    // 4. Validate Factors
    let raw_factors_str = params.factors.as_deref().unwrap_or(DEFAULT_FACTORS).trim();
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

    // 5. Point-in-Time validation
    let pit_data = GLOBAL_PIT_DATA.clone();
    if pit_data.is_enabled() {
        if !pit_data.is_valid_ticker(&ticker, start_date) {
            let err = AuthErrorResponse {
                error: "Bad Request".to_string(),
                message: format!(
                    "Ticker '{}' was not active or listed on start_date '{}'",
                    ticker, params.start_date
                ),
            };
            return (StatusCode::BAD_REQUEST, Json(err)).into_response();
        }
    }

    let is_mock = crate::state::is_questdb_mock_fallback_enabled()
        || crate::state::is_polygon_mock_fallback_enabled();

    let start_str = start_date.format("%Y-%m-%d").to_string();
    let end_str = end_date.format("%Y-%m-%d").to_string();

    // 6. Fetch historical daily price bars for ticker & benchmark
    let requested_tickers = vec![ticker.clone(), benchmark_ticker.clone()];
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

    // 7. Compute Daily Returns
    let empty_map = HashMap::new();
    let asset_prices = ticker_prices.get(&ticker).unwrap_or(&empty_map);
    let benchmark_prices = ticker_prices.get(&benchmark_ticker).unwrap_or(&empty_map);

    let asset_returns_map = compute_daily_returns(asset_prices);
    let benchmark_returns_map = compute_daily_returns(benchmark_prices);

    // Common trading dates sorted chronologically
    let mut common_dates: Vec<NaiveDate> = asset_returns_map
        .keys()
        .filter(|d| benchmark_returns_map.contains_key(d))
        .copied()
        .collect();
    common_dates.sort();

    let num_obs = common_dates.len();
    if num_obs < MIN_OBSERVATIONS_REQUIRED {
        let err = AuthErrorResponse {
            error: "Bad Request".to_string(),
            message: format!(
                "Insufficient overlapping trading days (got {}, minimum required: {}) for factor exposure regression",
                num_obs, MIN_OBSERVATIONS_REQUIRED
            ),
        };
        return (StatusCode::BAD_REQUEST, Json(err)).into_response();
    }

    // Vectorize aligned return series
    let mut y_returns = Vec::with_capacity(num_obs);
    let mut market_returns = Vec::with_capacity(num_obs);

    for d in &common_dates {
        y_returns.push(asset_returns_map[d]);
        market_returns.push(benchmark_returns_map[d]);
    }

    // 8. Construct Factor Series
    let momentum_series = compute_rolling_momentum_series(&y_returns, 20);
    let sentiment_series = generate_deterministic_sentiment_series(&ticker, &common_dates);
    let volatility_series = compute_rolling_volatility_series(&y_returns, 20);
    let size_series = compute_size_proxy_series(&market_returns);
    let value_series = compute_value_proxy_series(&market_returns);

    let mut factor_columns: Vec<Vec<f64>> = Vec::with_capacity(factors_included.len());

    for f in &factors_included {
        match f.as_str() {
            "market" => factor_columns.push(market_returns.clone()),
            "momentum" => factor_columns.push(momentum_series.clone()),
            "sentiment" => factor_columns.push(sentiment_series.clone()),
            "volatility" => factor_columns.push(volatility_series.clone()),
            "size" => factor_columns.push(size_series.clone()),
            "value" => factor_columns.push(value_series.clone()),
            _ => unreachable!(),
        }
    }

    // 9. Execute OLS Multiple Regression
    let ols_result = match solve_ols_multiple(&y_returns, &factor_columns) {
        Ok(res) => res,
        Err(err_msg) => {
            let err = AuthErrorResponse {
                error: "Bad Request".to_string(),
                message: format!("Factor regression failed: {}", err_msg),
            };
            return (StatusCode::BAD_REQUEST, Json(err)).into_response();
        }
    };

    // 10. Assemble Response DTO
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

    let response = FactorExposureResponse {
        ticker,
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
    fn test_normal_cdf_properties() {
        assert!((normal_cdf(0.0) - 0.5).abs() < 1e-4);
        assert!((normal_cdf(1.96) - 0.975).abs() < 1e-3);
        assert!((normal_cdf(-1.96) - 0.025).abs() < 1e-3);
    }

    #[test]
    fn test_matrix_inversion_2x2() {
        let m = vec![vec![4.0, 7.0], vec![2.0, 6.0]];
        let inv = invert_matrix(&m).unwrap();
        // Determinant = 24 - 14 = 10
        // Expected: [[0.6, -0.7], [-0.2, 0.4]]
        assert!((inv[0][0] - 0.6).abs() < 1e-6);
        assert!((inv[0][1] - (-0.7)).abs() < 1e-6);
        assert!((inv[1][0] - (-0.2)).abs() < 1e-6);
        assert!((inv[1][1] - 0.4).abs() < 1e-6);
    }

    #[test]
    fn test_solve_ols_synthetic_multi_factor() {
        // True model: y = 0.02 + 1.2 * f1 - 0.5 * f2
        let n = 100;
        let mut y = Vec::with_capacity(n);
        let mut f1 = Vec::with_capacity(n);
        let mut f2 = Vec::with_capacity(n);

        for i in 0..n {
            let x1 = (i as f64) * 0.01;
            let x2 = ((i * 3 % 7) as f64) * 0.05;
            let target = 0.02 + 1.2 * x1 - 0.5 * x2;
            f1.push(x1);
            f2.push(x2);
            y.push(target);
        }

        let result = solve_ols_multiple(&y, &[f1, f2]).unwrap();

        assert_eq!(result.num_observations, 100);
        assert!((result.alpha - 0.02).abs() < 1e-4);
        assert!((result.betas[0] - 1.2).abs() < 1e-4);
        assert!((result.betas[1] - (-0.5)).abs() < 1e-4);
        assert!((result.r_squared - 1.0).abs() < 1e-4);
    }

    #[test]
    fn test_rolling_momentum_and_volatility() {
        let r = vec![0.01, 0.02, -0.01, 0.03, -0.02];
        let mom = compute_rolling_momentum_series(&r, 3);
        assert_eq!(mom.len(), 5);
        assert!((mom[0] - 0.01).abs() < 1e-6);
        assert!((mom[2] - 0.02).abs() < 1e-6); // 0.01 + 0.02 - 0.01 = 0.02

        let vol = compute_rolling_volatility_series(&r, 3);
        assert_eq!(vol.len(), 5);
        assert!(vol[2] < 0.0); // negative standard deviation
    }
}
