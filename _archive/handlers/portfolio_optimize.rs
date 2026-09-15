//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Portfolio Optimization Handler (Mean-Variance & Risk Parity)
//! ═══════════════════════════════════════════════════════════════════════════════

use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use chrono::{NaiveDate, Utc};
use once_cell::sync::Lazy;
use std::collections::{HashMap, HashSet};
use tracing::info;

use crate::auth::AuthErrorResponse;
use crate::handlers::backtest::generate_mock_stock_prices;
use crate::models::{PortfolioOptimizeRequest, PortfolioOptimizeResponse, PortfolioWeight};
use crate::storage::{QuestDbClient, QuestDbClientConfig};

static QUESTDB_CLIENT: Lazy<QuestDbClient> =
    Lazy::new(|| QuestDbClient::new(QuestDbClientConfig::default()));

pub const MIN_TICKERS_LIMIT: usize = 2;
pub const MAX_TICKERS_LIMIT: usize = 20;
pub const MAX_LOOKBACK_DAYS_LIMIT: i64 = 730; // 2 years maximum

/// Multiplies matrix M (n x m) with vector v (m x 1).
pub fn mat_vec_mul(m: &[Vec<f64>], v: &[f64]) -> Vec<f64> {
    let n = m.len();
    let mut res = vec![0.0; n];
    for i in 0..n {
        for j in 0..v.len() {
            res[i] += m[i][j] * v[j];
        }
    }
    res
}

/// Computes inner dot product of two equal-length vectors.
pub fn dot_product(u: &[f64], v: &[f64]) -> f64 {
    u.iter().zip(v.iter()).map(|(&a, &b)| a * b).sum()
}

/// Computes annualized mean return vector and regularized sample covariance matrix from daily return series.
pub fn compute_returns_and_covariance(returns_matrix: &[Vec<f64>]) -> (Vec<f64>, Vec<Vec<f64>>) {
    let t = returns_matrix.len();
    let n = if t > 0 { returns_matrix[0].len() } else { 0 };
    if t < 2 || n == 0 {
        return (vec![0.0; n], vec![vec![0.0; n]; n]);
    }

    let mut means = vec![0.0; n];
    for row in returns_matrix {
        for j in 0..n {
            means[j] += row[j];
        }
    }
    for j in 0..n {
        means[j] /= t as f64;
    }

    let mut cov = vec![vec![0.0; n]; n];
    for row in returns_matrix {
        for i in 0..n {
            let di = row[i] - means[i];
            for j in 0..n {
                let dj = row[j] - means[j];
                cov[i][j] += di * dj;
            }
        }
    }

    let df = (t - 1) as f64;
    for i in 0..n {
        for j in 0..n {
            cov[i][j] = (cov[i][j] / df) * 252.0; // Annualized
        }
        cov[i][i] += 1e-8; // Diagonal regularization for positive definiteness
    }

    let annualized_means: Vec<f64> = means.iter().map(|&m| m * 252.0).collect();
    (annualized_means, cov)
}

/// Exact Euclidean projection of vector v onto probability simplex: {w >= 0, sum(w) = 1.0} (Duchi et al., 2008).
pub fn project_simplex(v: &[f64]) -> Vec<f64> {
    let n = v.len();
    if n == 0 {
        return Vec::new();
    }
    let mut u = v.to_vec();
    u.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));

    let mut cssv = 0.0;
    let mut rho = 0;
    for (j, &val) in u.iter().enumerate() {
        cssv += val;
        let cond = val - (cssv - 1.0) / ((j + 1) as f64);
        if cond > 0.0 {
            rho = j;
        }
    }

    let sum_u: f64 = u[..=rho].iter().sum();
    let theta = (sum_u - 1.0) / ((rho + 1) as f64);

    v.iter().map(|&x| (x - theta).max(0.0)).collect()
}

/// Solves Maximum Sharpe Ratio optimization problem.
pub fn optimize_max_sharpe(
    mu: &[f64],
    cov: &[Vec<f64>],
    risk_free_rate: f64,
    long_only: bool,
) -> Vec<f64> {
    let n = mu.len();
    if n == 0 {
        return Vec::new();
    }
    if n == 1 {
        return vec![1.0];
    }

    // Initialize with equal weights
    let mut w = vec![1.0 / (n as f64); n];
    let mut best_w = w.clone();
    let mut best_sharpe = -1e9;

    let lr = 0.05;
    let max_iters = 300;

    for _ in 0..max_iters {
        let ret = dot_product(&w, mu);
        let sigma_w = mat_vec_mul(cov, &w);
        let var = dot_product(&w, &sigma_w).max(1e-10);
        let vol = var.sqrt();
        let excess = ret - risk_free_rate;
        let sharpe = excess / vol;

        if sharpe > best_sharpe {
            best_sharpe = sharpe;
            best_w = w.clone();
        }

        // Gradient: dSR/dw = mu / vol - (excess / vol^3) * (cov * w)
        let mut grad = vec![0.0; n];
        for i in 0..n {
            grad[i] = (mu[i] / vol) - (excess / (vol * var)) * sigma_w[i];
        }

        let mut next_w = vec![0.0; n];
        for i in 0..n {
            next_w[i] = w[i] + lr * grad[i];
        }

        if long_only {
            w = project_simplex(&next_w);
        } else {
            let sum: f64 = next_w.iter().sum();
            if sum.abs() > 1e-8 {
                w = next_w.iter().map(|&x| x / sum).collect();
            } else {
                w = next_w;
            }
        }
    }

    let final_sigma = mat_vec_mul(cov, &best_w);
    let final_vol = dot_product(&best_w, &final_sigma).max(1e-10).sqrt();
    let final_sharpe = (dot_product(&best_w, mu) - risk_free_rate) / final_vol;

    let curr_sigma = mat_vec_mul(cov, &w);
    let curr_vol = dot_product(&w, &curr_sigma).max(1e-10).sqrt();
    let curr_sharpe = (dot_product(&w, mu) - risk_free_rate) / curr_vol;

    if curr_sharpe > final_sharpe {
        w
    } else {
        best_w
    }
}

/// Solves Equal Risk Contribution (Risk Parity) portfolio weights via Cyclical Coordinate Descent (CCD).
pub fn optimize_risk_parity(cov: &[Vec<f64>]) -> Vec<f64> {
    let n = cov.len();
    if n == 0 {
        return Vec::new();
    }
    if n == 1 {
        return vec![1.0];
    }

    // Initialize with inverse-volatility weights
    let mut w = vec![0.0; n];
    for i in 0..n {
        let vol = cov[i][i].max(1e-8).sqrt();
        w[i] = 1.0 / vol;
    }
    let sum_w: f64 = w.iter().sum();
    for i in 0..n {
        w[i] /= sum_w;
    }

    let b_target = 1.0 / (n as f64);
    let max_iters = 50;

    for _ in 0..max_iters {
        for i in 0..n {
            let a = cov[i][i].max(1e-8);
            let mut b = 0.0;
            for j in 0..n {
                if i != j {
                    b += cov[i][j] * w[j];
                }
            }

            let discriminant = b * b + 4.0 * a * b_target;
            w[i] = (-b + discriminant.sqrt()) / (2.0 * a);
        }
    }

    // Normalize weights to sum to 1.0
    let total: f64 = w.iter().sum();
    if total > 1e-12 {
        for i in 0..n {
            w[i] /= total;
        }
    } else {
        w = vec![1.0 / (n as f64); n];
    }

    w
}

/// Execute Quantitative Portfolio Optimization.
#[utoipa::path(
    post,
    path = "/portfolio/optimize",
    tag = "Portfolio & Risk Analytics",
    request_body = PortfolioOptimizeRequest,
    responses(
        (status = 200, description = "Portfolio optimization computed successfully", body = PortfolioOptimizeResponse),
        (status = 400, description = "Invalid tickers universe, date format, or optimization parameters", body = AuthErrorResponse),
        (status = 401, description = "Missing or invalid Bearer JWT", body = AuthErrorResponse)
    ),
    security(
        ("BearerAuth" = [])
    )
)]
pub async fn portfolio_optimize_handler(Json(req): Json<PortfolioOptimizeRequest>) -> Response {
    // 1. Validate tickers list
    let mut tickers = Vec::new();
    for t in &req.tickers {
        let clean = t.trim().to_uppercase();
        if !clean.is_empty()
            && clean
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
        {
            if !tickers.contains(&clean) {
                tickers.push(clean);
            }
        } else {
            let err = AuthErrorResponse {
                error: "Bad Request".to_string(),
                message: format!("Invalid ticker symbol '{}' in tickers parameter", t),
            };
            return (StatusCode::BAD_REQUEST, Json(err)).into_response();
        }
    }

    if tickers.len() < MIN_TICKERS_LIMIT {
        let err = AuthErrorResponse {
            error: "Bad Request".to_string(),
            message: format!(
                "Portfolio optimization requires at least {} tickers (received {})",
                MIN_TICKERS_LIMIT,
                tickers.len()
            ),
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

    // 2. Validate dates
    let start_date = match NaiveDate::parse_from_str(req.start_date.trim(), "%Y-%m-%d") {
        Ok(d) => d,
        Err(e) => {
            let err = AuthErrorResponse {
                error: "Bad Request".to_string(),
                message: format!(
                    "Invalid start_date format '{}', expected YYYY-MM-DD: {}",
                    req.start_date, e
                ),
            };
            return (StatusCode::BAD_REQUEST, Json(err)).into_response();
        }
    };

    let end_date = match NaiveDate::parse_from_str(req.end_date.trim(), "%Y-%m-%d") {
        Ok(d) => d,
        Err(e) => {
            let err = AuthErrorResponse {
                error: "Bad Request".to_string(),
                message: format!(
                    "Invalid end_date format '{}', expected YYYY-MM-DD: {}",
                    req.end_date, e
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
                req.start_date, req.end_date
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

    // 3. Validate optimization_type
    let opt_type = req
        .optimization_type
        .unwrap_or_else(|| "max_sharpe".to_string())
        .trim()
        .to_lowercase();

    if opt_type != "max_sharpe" && opt_type != "risk_parity" {
        let err = AuthErrorResponse {
            error: "Bad Request".to_string(),
            message: format!(
                "Invalid optimization_type '{}'. Allowed values are 'max_sharpe' and 'risk_parity'",
                opt_type
            ),
        };
        return (StatusCode::BAD_REQUEST, Json(err)).into_response();
    }

    // 4. Validate risk_free_rate
    let risk_free_rate = req.risk_free_rate.unwrap_or(0.0);
    if risk_free_rate < 0.0 || risk_free_rate > 0.10 {
        let err = AuthErrorResponse {
            error: "Bad Request".to_string(),
            message: format!(
                "risk_free_rate ({}) must be between 0.0 and 0.10 (0% to 10%)",
                risk_free_rate
            ),
        };
        return (StatusCode::BAD_REQUEST, Json(err)).into_response();
    }

    let long_only = req
        .constraints
        .as_ref()
        .and_then(|c| c.long_only)
        .unwrap_or(true);

    // 5. Ingest Historical Stock Prices
    let is_mock = crate::state::is_questdb_mock_fallback_enabled()
        || crate::state::is_polygon_mock_fallback_enabled();

    let start_str = start_date.format("%Y-%m-%d").to_string();
    let end_str = end_date.format("%Y-%m-%d").to_string();

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

    // 6. Compute Daily Return Series for Each Ticker
    let mut ticker_returns: HashMap<String, HashMap<NaiveDate, f64>> = HashMap::new();
    for (t, prices) in &ticker_prices {
        let mut sorted_dates: Vec<NaiveDate> = prices.keys().copied().collect();
        sorted_dates.sort();
        let mut ret_map = HashMap::new();
        for w in sorted_dates.windows(2) {
            let p_prev = prices[&w[0]];
            let p_curr = prices[&w[1]];
            if p_prev > 0.0 {
                ret_map.insert(w[1], (p_curr - p_prev) / p_prev);
            }
        }
        ticker_returns.insert(t.clone(), ret_map);
    }

    // 7. Align Dates Across All Constituent Tickers
    let mut common_dates: Option<HashSet<NaiveDate>> = None;
    for t in &tickers {
        let dates: HashSet<NaiveDate> = ticker_returns
            .get(t)
            .map(|m| m.keys().copied().collect())
            .unwrap_or_default();

        match common_dates {
            None => common_dates = Some(dates),
            Some(ref mut current_set) => {
                *current_set = current_set.intersection(&dates).copied().collect();
            }
        }
    }

    let mut aligned_dates: Vec<NaiveDate> = common_dates.unwrap_or_default().into_iter().collect();
    aligned_dates.sort();

    // If insufficient data points, fallback to synthetic series
    let returns_matrix: Vec<Vec<f64>> = if aligned_dates.len() >= 5 {
        aligned_dates
            .iter()
            .map(|&d| {
                tickers
                    .iter()
                    .map(|t| ticker_returns[t].get(&d).copied().unwrap_or(0.0))
                    .collect()
            })
            .collect()
    } else {
        // Generate aligned mock series
        let mock_prices: HashMap<String, HashMap<NaiveDate, f64>> = tickers
            .iter()
            .map(|t| {
                (
                    t.clone(),
                    generate_mock_stock_prices(t, start_date, end_date),
                )
            })
            .collect();

        let mut all_d: Vec<NaiveDate> = mock_prices[&tickers[0]].keys().copied().collect();
        all_d.sort();

        let mut matrix = Vec::new();
        for w in all_d.windows(2) {
            let row: Vec<f64> = tickers
                .iter()
                .map(|t| {
                    let p0 = mock_prices[t][&w[0]];
                    let p1 = mock_prices[t][&w[1]];
                    if p0 > 0.0 {
                        (p1 - p0) / p0
                    } else {
                        0.0
                    }
                })
                .collect();
            matrix.push(row);
        }
        matrix
    };

    // 8. Compute Expected Annual Returns and Covariance Matrix
    let (annual_mu, annual_cov) = compute_returns_and_covariance(&returns_matrix);

    // 9. Execute Optimization
    let optimal_weights = if opt_type == "risk_parity" {
        optimize_risk_parity(&annual_cov)
    } else {
        optimize_max_sharpe(&annual_mu, &annual_cov, risk_free_rate, long_only)
    };

    // 10. Compute Portfolio Metrics
    let port_return = dot_product(&optimal_weights, &annual_mu);
    let sigma_w = mat_vec_mul(&annual_cov, &optimal_weights);
    let port_vol = dot_product(&optimal_weights, &sigma_w).max(1e-10).sqrt();
    let port_sharpe = (port_return - risk_free_rate) / port_vol;

    let weights_vec: Vec<PortfolioWeight> = tickers
        .iter()
        .zip(optimal_weights.iter())
        .map(|(t, &w)| PortfolioWeight {
            ticker: t.clone(),
            weight: (w * 10000.0).round() / 10000.0,
        })
        .collect();

    info!(
        "[Portfolio Optimize] Tickers={:?}, OptType='{}', ExpReturn={:.4}, Vol={:.4}, Sharpe={:.4}",
        tickers, opt_type, port_return, port_vol, port_sharpe
    );

    let resp = PortfolioOptimizeResponse {
        tickers,
        start_date: req.start_date,
        end_date: req.end_date,
        optimization_type: opt_type,
        risk_free_rate,
        weights: weights_vec,
        expected_annual_return: (port_return * 10000.0).round() / 10000.0,
        expected_annual_volatility: (port_vol * 10000.0).round() / 10000.0,
        sharpe_ratio: (port_sharpe * 10000.0).round() / 10000.0,
        generated_at: Utc::now().to_rfc3339(),
    };

    (StatusCode::OK, Json(resp)).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simplex_projection() {
        let v = vec![0.5, 0.5, 0.5];
        let p = project_simplex(&v);
        assert!((p.iter().sum::<f64>() - 1.0).abs() < 1e-6);
        for &x in &p {
            assert!(x >= 0.0);
        }

        let v2 = vec![-1.0, 2.0, 0.0];
        let p2 = project_simplex(&v2);
        assert!((p2.iter().sum::<f64>() - 1.0).abs() < 1e-6);
        assert_eq!(p2[0], 0.0);
        assert_eq!(p2[1], 1.0);
        assert_eq!(p2[2], 0.0);
    }

    #[test]
    fn test_returns_and_covariance() {
        let returns = vec![
            vec![0.01, 0.02],
            vec![0.02, 0.01],
            vec![-0.01, 0.00],
            vec![0.03, 0.02],
        ];

        let (mu, cov) = compute_returns_and_covariance(&returns);
        assert_eq!(mu.len(), 2);
        assert_eq!(cov.len(), 2);
        assert_eq!(cov[0].len(), 2);
        assert!(cov[0][0] > 0.0);
        assert!(cov[1][1] > 0.0);
    }

    #[test]
    fn test_max_sharpe_optimizer() {
        let mu = vec![0.15, 0.10];
        let cov = vec![vec![0.04, 0.01], vec![0.01, 0.02]];

        let weights = optimize_max_sharpe(&mu, &cov, 0.02, true);
        assert_eq!(weights.len(), 2);
        assert!((weights.iter().sum::<f64>() - 1.0).abs() < 1e-4);
        assert!(weights[0] >= 0.0 && weights[1] >= 0.0);
    }

    #[test]
    fn test_risk_parity_optimizer() {
        let cov = vec![vec![0.09, 0.02], vec![0.02, 0.04]];

        let weights = optimize_risk_parity(&cov);
        assert_eq!(weights.len(), 2);
        assert!((weights.iter().sum::<f64>() - 1.0).abs() < 1e-4);
        // Asset 2 has lower volatility, so it should have higher weight in risk parity
        assert!(weights[1] > weights[0]);
    }
}
