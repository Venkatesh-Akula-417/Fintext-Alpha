//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Options Implied Volatility & Greeks Analytics Engine
//! ═══════════════════════════════════════════════════════════════════════════════
//!
//! Provides institutional Black-Scholes analytical option pricing, analytical
//! Greeks calculation (Delta, Gamma, Theta, Vega, Rho), and numerical Implied
//! Volatility solving using Newton-Raphson with bisection fallback.
//! ═══════════════════════════════════════════════════════════════════════════════

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::{Datelike, NaiveDate, Utc};
use serde_json::json;
use std::f64::consts::PI;

use crate::models::{OptionContract, OptionsIvParams, OptionsIvResponse};
use crate::state::AppState;
use crate::storage::questdb_client::QuestDbClient;

/// Contract type enum for derivatives pricing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionType {
    Call,
    Put,
}

impl OptionType {
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s.trim().to_uppercase().as_str() {
            "CALL" | "C" => Ok(OptionType::Call),
            "PUT" | "P" => Ok(OptionType::Put),
            other => Err(format!(
                "Invalid option type '{}', expected 'call' or 'put'",
                other
            )),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            OptionType::Call => "CALL",
            OptionType::Put => "PUT",
        }
    }
}

/// Container for first and second order Black-Scholes Greeks.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Greeks {
    pub delta: f64,
    pub gamma: f64,
    pub theta: f64,
    pub vega: f64,
    pub rho: f64,
}

/// Standard normal probability density function N'(x) = (1 / sqrt(2*pi)) * exp(-x^2 / 2).
#[inline]
pub fn standard_normal_pdf(x: f64) -> f64 {
    (-0.5 * x * x).exp() / (2.0 * PI).sqrt()
}

/// High-precision Cumulative Normal Distribution Function Φ(x) using the
/// Abramowitz & Stegun rational approximation (formula 7.1.26).
/// Absolute numerical error is bounded by |ε| < 1.5e-7 across all real inputs.
#[inline]
pub fn standard_normal_cdf(x: f64) -> f64 {
    if x < -10.0 {
        return 0.0;
    }
    if x > 10.0 {
        return 1.0;
    }
    let z = x.abs();
    let t = 1.0 / (1.0 + 0.2316419 * z);
    let poly = t
        * (0.319381530
            + t * (-0.356563782 + t * (1.781477937 + t * (-1.821255978 + t * 1.330274429))));
    let cdf = 1.0 - standard_normal_pdf(z) * poly;
    if x < 0.0 {
        1.0 - cdf
    } else {
        cdf
    }
}

/// Calculates analytical Black-Scholes European option price.
///
/// # Arguments
/// * `option_type` - Call or Put
/// * `spot` - Current underlying asset price $S > 0$
/// * `strike` - Strike price $K > 0$
/// * `time_to_expiry_years` - Time to expiration in years $T > 0$
/// * `risk_free_rate` - Annualized continuously compounded risk-free rate $r$
/// * `dividend_yield` - Annualized continuous dividend yield $q$
/// * `volatility` - Annualized volatility $\sigma > 0$
pub fn black_scholes_price(
    option_type: OptionType,
    spot: f64,
    strike: f64,
    time_to_expiry_years: f64,
    risk_free_rate: f64,
    dividend_yield: f64,
    volatility: f64,
) -> f64 {
    if spot <= 0.0 || strike <= 0.0 || time_to_expiry_years <= 0.0 || volatility <= 0.0 {
        let df_q = (-dividend_yield * time_to_expiry_years.max(0.0)).exp();
        let df_r = (-risk_free_rate * time_to_expiry_years.max(0.0)).exp();
        return match option_type {
            OptionType::Call => (spot * df_q - strike * df_r).max(0.0),
            OptionType::Put => (strike * df_r - spot * df_q).max(0.0),
        };
    }

    let sqrt_t = time_to_expiry_years.sqrt();
    let denom = volatility * sqrt_t;
    let d1 = ((spot / strike).ln()
        + (risk_free_rate - dividend_yield + 0.5 * volatility * volatility) * time_to_expiry_years)
        / denom;
    let d2 = d1 - denom;

    let df_q = (-dividend_yield * time_to_expiry_years).exp();
    let df_r = (-risk_free_rate * time_to_expiry_years).exp();

    match option_type {
        OptionType::Call => {
            spot * df_q * standard_normal_cdf(d1) - strike * df_r * standard_normal_cdf(d2)
        }
        OptionType::Put => {
            strike * df_r * standard_normal_cdf(-d2) - spot * df_q * standard_normal_cdf(-d1)
        }
    }
}

/// Calculates analytical Black-Scholes option Greeks (Delta, Gamma, Theta, Vega, Rho).
pub fn black_scholes_greeks(
    option_type: OptionType,
    spot: f64,
    strike: f64,
    time_to_expiry_years: f64,
    risk_free_rate: f64,
    dividend_yield: f64,
    volatility: f64,
) -> Greeks {
    if spot <= 0.0 || strike <= 0.0 || time_to_expiry_years <= 0.0 || volatility <= 0.0 {
        return Greeks {
            delta: match option_type {
                OptionType::Call => {
                    if spot > strike {
                        1.0
                    } else {
                        0.0
                    }
                }
                OptionType::Put => {
                    if spot < strike {
                        -1.0
                    } else {
                        0.0
                    }
                }
            },
            gamma: 0.0,
            theta: 0.0,
            vega: 0.0,
            rho: 0.0,
        };
    }

    let sqrt_t = time_to_expiry_years.sqrt();
    let denom = volatility * sqrt_t;
    let d1 = ((spot / strike).ln()
        + (risk_free_rate - dividend_yield + 0.5 * volatility * volatility) * time_to_expiry_years)
        / denom;
    let d2 = d1 - denom;

    let df_q = (-dividend_yield * time_to_expiry_years).exp();
    let df_r = (-risk_free_rate * time_to_expiry_years).exp();
    let n_prime_d1 = standard_normal_pdf(d1);

    // Gamma: same for Call and Put
    let gamma = (df_q * n_prime_d1) / (spot * denom);

    // Vega: dollar change per 1% vol move ($/1% vol)
    let raw_vega = spot * df_q * sqrt_t * n_prime_d1;
    let vega_per_pct = raw_vega / 100.0;

    let (delta, daily_theta, rho_per_pct) = match option_type {
        OptionType::Call => {
            let delta = df_q * standard_normal_cdf(d1);
            let annual_theta = -((spot * df_q * n_prime_d1 * volatility) / (2.0 * sqrt_t))
                - risk_free_rate * strike * df_r * standard_normal_cdf(d2)
                + dividend_yield * spot * df_q * standard_normal_cdf(d1);
            let daily_theta = annual_theta / 365.25;
            let rho = strike * time_to_expiry_years * df_r * standard_normal_cdf(d2) / 100.0;
            (delta, daily_theta, rho)
        }
        OptionType::Put => {
            let delta = -df_q * standard_normal_cdf(-d1);
            let annual_theta = -((spot * df_q * n_prime_d1 * volatility) / (2.0 * sqrt_t))
                + risk_free_rate * strike * df_r * standard_normal_cdf(-d2)
                - dividend_yield * spot * df_q * standard_normal_cdf(-d1);
            let daily_theta = annual_theta / 365.25;
            let rho = -strike * time_to_expiry_years * df_r * standard_normal_cdf(-d2) / 100.0;
            (delta, daily_theta, rho)
        }
    };

    Greeks {
        delta: (delta * 10000.0).round() / 10000.0,
        gamma: (gamma * 10000.0).round() / 10000.0,
        theta: (daily_theta * 10000.0).round() / 10000.0,
        vega: (vega_per_pct * 10000.0).round() / 10000.0,
        rho: (rho_per_pct * 10000.0).round() / 10000.0,
    }
}

/// Solves for Black-Scholes implied volatility using Newton-Raphson with bisection fallback.
///
/// Ensures convergence within tolerance $\le 10^{-6}$ in $\le 100$ iterations.
pub fn calculate_implied_volatility(
    option_type: OptionType,
    market_price: f64,
    spot: f64,
    strike: f64,
    time_to_expiry_years: f64,
    risk_free_rate: f64,
    dividend_yield: f64,
) -> f64 {
    if market_price <= 0.0 || spot <= 0.0 || strike <= 0.0 || time_to_expiry_years <= 0.0 {
        return 0.0;
    }

    let df_q = (-dividend_yield * time_to_expiry_years).exp();
    let df_r = (-risk_free_rate * time_to_expiry_years).exp();

    // Check lower bound (intrinsic value)
    let intrinsic = match option_type {
        OptionType::Call => (spot * df_q - strike * df_r).max(0.0),
        OptionType::Put => (strike * df_r - spot * df_q).max(0.0),
    };

    if market_price <= intrinsic {
        return 0.01; // Intrinsic floor volatility
    }

    // Check upper bound (maximum possible theoretical price)
    let max_price = match option_type {
        OptionType::Call => spot * df_q,
        OptionType::Put => strike * df_r,
    };
    if market_price >= max_price {
        return 5.0; // Ceiling volatility
    }

    // Initial guess using Brenner-Subrahmanyam approximation if ATM or default 0.30
    let mut sigma = (2.0 * PI / time_to_expiry_years).sqrt() * (market_price / spot);
    if sigma <= 0.05 || sigma > 2.5 || sigma.is_nan() {
        sigma = 0.30;
    }

    let mut low_vol = 0.001;
    let mut high_vol = 5.0;
    let tol = 1e-6;
    let max_iter = 100;

    for _ in 0..max_iter {
        let price = black_scholes_price(
            option_type,
            spot,
            strike,
            time_to_expiry_years,
            risk_free_rate,
            dividend_yield,
            sigma,
        );
        let diff = price - market_price;

        if diff.abs() < tol {
            return (sigma * 10000.0).round() / 10000.0;
        }

        // Tighten bisection interval
        if diff > 0.0 {
            high_vol = sigma;
        } else {
            low_vol = sigma;
        }

        let sqrt_t = time_to_expiry_years.sqrt();
        let denom = sigma * sqrt_t;
        let d1 = ((spot / strike).ln()
            + (risk_free_rate - dividend_yield + 0.5 * sigma * sigma) * time_to_expiry_years)
            / denom;
        let vega = spot * df_q * sqrt_t * standard_normal_pdf(d1);

        if vega > 1e-8 {
            let step = diff / vega;
            let next_sigma = sigma - step;
            if next_sigma > low_vol && next_sigma < high_vol {
                sigma = next_sigma;
                continue;
            }
        }

        // Bisection step fallback
        sigma = 0.5 * (low_vol + high_vol);
    }

    (sigma * 10000.0).round() / 10000.0
}

/// Compute time to expiration in annualized years from valuation date to expiration date.
pub fn compute_time_to_expiry_years(val_date: NaiveDate, exp_date: NaiveDate) -> f64 {
    let days = (exp_date - val_date).num_days();
    if days <= 0 {
        0.001 // Minimum fraction of a day for expired/0-DTE options
    } else {
        (days as f64) / 365.25
    }
}

/// Deterministic mock spot price resolver for core equities.
pub fn resolve_mock_spot_price(ticker: &str) -> f64 {
    let clean = ticker.trim().to_uppercase();
    let hash_val = clean.bytes().map(|b| b as usize).sum::<usize>();

    match clean.as_str() {
        "AAPL" => 224.50,
        "NVDA" => 125.00,
        "MSFT" => 415.00,
        "GOOGL" | "GOOG" => 165.00,
        "AMZN" => 185.00,
        "META" => 510.00,
        "TSLA" => 210.00,
        "SPY" => 550.00,
        _ => 100.0 + ((hash_val % 300) as f64),
    }
}

/// Generates a realistic deterministic option chain around ATM with volatility skew/smile.
pub fn generate_mock_options_chain(
    ticker: &str,
    expiration_date: &str,
    spot_price: f64,
    risk_free_rate: f64,
    dividend_yield: f64,
    requested_strike: Option<f64>,
    option_type_filter: Option<&str>,
) -> Result<Vec<OptionContract>, String> {
    let exp_date = NaiveDate::parse_from_str(expiration_date.trim(), "%Y-%m-%d")
        .map_err(|e| format!("Invalid expiration_date '{}': {}", expiration_date, e))?;

    let today = Utc::now().date_naive();
    let time_to_expiry_years = compute_time_to_expiry_years(today, exp_date);

    let clean_ticker = ticker.trim().to_uppercase();
    let filter = option_type_filter.unwrap_or("all").trim().to_lowercase();

    // Determine strike interval step based on spot price level
    let strike_step = if spot_price > 300.0 {
        10.0
    } else if spot_price > 100.0 {
        5.0
    } else if spot_price > 50.0 {
        2.5
    } else {
        1.0
    };

    let atm_strike = (spot_price / strike_step).round() * strike_step;

    // Determine strikes to evaluate
    let strikes: Vec<f64> = if let Some(specific_k) = requested_strike {
        vec![specific_k]
    } else {
        // Return ATM ± 5 strikes (total 11 strikes)
        (-5..=5)
            .map(|offset| ((atm_strike + (offset as f64 * strike_step)) * 100.0).round() / 100.0)
            .filter(|&k| k > 0.0)
            .collect()
    };

    let exp_compact = format!(
        "{:02}{:02}{:02}",
        exp_date.year() % 100,
        exp_date.month(),
        exp_date.day()
    );

    let mut contracts = Vec::new();

    for strike in strikes {
        let moneyness = (strike - spot_price) / spot_price;
        // Volatility smile & skew curve: baseline 25% + quadratic smile - linear skew
        let base_vol = (0.25 + 0.18 * moneyness * moneyness - 0.08 * moneyness).clamp(0.10, 1.20);

        let types_to_gen = match filter.as_str() {
            "call" | "c" => vec![OptionType::Call],
            "put" | "p" => vec![OptionType::Put],
            _ => vec![OptionType::Call, OptionType::Put],
        };

        for opt_type in types_to_gen {
            let type_char = match opt_type {
                OptionType::Call => 'C',
                OptionType::Put => 'P',
            };
            let contract_ticker = format!(
                "O:{}{}{}{:08}",
                clean_ticker,
                exp_compact,
                type_char,
                (strike * 1000.0).round() as u64
            );

            // Calculate exact theoretical Black-Scholes price
            let bs_price = black_scholes_price(
                opt_type,
                spot_price,
                strike,
                time_to_expiry_years,
                risk_free_rate,
                dividend_yield,
                base_vol,
            );

            let rounded_price = (bs_price.max(0.01) * 100.0).round() / 100.0;
            let spread = (rounded_price * 0.015).clamp(0.05, 0.50);
            let bid = ((rounded_price - (spread / 2.0)).max(0.01) * 100.0).round() / 100.0;
            let ask = ((rounded_price + (spread / 2.0)).max(0.02) * 100.0).round() / 100.0;
            let mid = (bid + ask) / 2.0;

            // Invert market price to verify numerical IV solver
            let iv = calculate_implied_volatility(
                opt_type,
                mid,
                spot_price,
                strike,
                time_to_expiry_years,
                risk_free_rate,
                dividend_yield,
            );

            let greeks = black_scholes_greeks(
                opt_type,
                spot_price,
                strike,
                time_to_expiry_years,
                risk_free_rate,
                dividend_yield,
                iv.max(0.05),
            );

            // Liquidity profile: higher volume and open interest near ATM
            let dist_atm = (strike - atm_strike).abs() / strike_step;
            let volume = (5000.0 * (-0.3 * dist_atm).exp()).round() as u64 + 50;
            let open_interest = (25000.0 * (-0.25 * dist_atm).exp()).round() as u64 + 200;

            contracts.push(OptionContract {
                ticker: contract_ticker,
                underlying_ticker: clean_ticker.clone(),
                expiration_date: expiration_date.to_string(),
                strike,
                option_type: opt_type.as_str().to_string(),
                bid,
                ask,
                last: rounded_price,
                volume,
                open_interest,
                implied_volatility: iv,
                delta: greeks.delta,
                gamma: greeks.gamma,
                theta: greeks.theta,
                vega: greeks.vega,
                rho: greeks.rho,
            });
        }
    }

    Ok(contracts)
}

/// GET /options/iv
///
/// Query options implied volatility (IV) and Black-Scholes Greeks (Delta, Gamma, Theta, Vega, Rho)
/// for a specified underlying equity ticker and expiration date.
#[utoipa::path(
    get,
    path = "/options/iv",
    params(OptionsIvParams),
    responses(
        (status = 200, description = "Options implied volatility and Greeks calculated successfully", body = OptionsIvResponse),
        (status = 400, description = "Invalid query parameters (malformed ticker, invalid expiration date, invalid strike, or rates out of bounds)"),
        (status = 401, description = "Missing or invalid Bearer JWT / API Key authentication"),
        (status = 429, description = "Rate limit capacity exceeded")
    ),
    security(
        ("bearerAuth" = [])
    ),
    tag = "Options & Derivatives"
)]
pub async fn get_options_iv_handler(
    State(_state): State<AppState>,
    Query(params): Query<OptionsIvParams>,
) -> Result<Json<OptionsIvResponse>, (StatusCode, Json<serde_json::Value>)> {
    // 1. Validate underlying ticker
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

    // 2. Validate expiration date format
    let exp_date = NaiveDate::parse_from_str(params.expiration_date.trim(), "%Y-%m-%d").map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Invalid Parameter",
                "message": format!("Invalid expiration_date format '{}', expected YYYY-MM-DD: {}", params.expiration_date, e),
                "field": "expiration_date"
            })),
        )
    })?;

    // Validate expiration date is within reasonable range (e.g. not more than 2 years in future)
    let today = Utc::now().date_naive();
    let days_diff = (exp_date - today).num_days();
    if days_diff > 730 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Invalid Parameter",
                "message": "Expiration date cannot exceed 2 years from today",
                "field": "expiration_date"
            })),
        ));
    }

    // 3. Validate option_type
    let raw_opt_type = params
        .option_type
        .as_deref()
        .unwrap_or("all")
        .trim()
        .to_lowercase();
    if !["all", "call", "put", "c", "p"].contains(&raw_opt_type.as_str()) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Invalid Parameter",
                "message": format!("Invalid option_type '{}', allowed values: 'all', 'call', 'put'", raw_opt_type),
                "field": "option_type"
            })),
        ));
    }

    // 4. Validate strike if provided
    if let Some(k) = params.strike {
        if k <= 0.0 || k.is_nan() || k.is_infinite() {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Invalid Parameter",
                    "message": format!("Strike price must be a positive number, got {}", k),
                    "field": "strike"
                })),
            ));
        }
    }

    // 5. Validate risk-free rate
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

    // 6. Validate continuous dividend yield
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

    // 7. Resolve underlying spot price
    let spot_price = resolve_mock_spot_price(&safe_ticker);

    // 8. Generate / query options chain with Black-Scholes IV and Greeks
    let contracts = generate_mock_options_chain(
        &safe_ticker,
        &params.expiration_date,
        spot_price,
        r,
        q,
        params.strike,
        Some(&raw_opt_type),
    )
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": "Derivatives Pricing Error",
                "message": e
            })),
        )
    })?;

    let count = contracts.len();

    let message = if params.strike.is_some() {
        format!(
            "Options IV and Greeks calculated for {} strike {:.2} expiring on {}",
            safe_ticker,
            params.strike.unwrap(),
            params.expiration_date
        )
    } else {
        format!(
            "Options IV and Greeks chain calculated for {} across {} strikes near ATM (${:.2}) expiring on {}",
            safe_ticker, count, spot_price, params.expiration_date
        )
    };

    Ok(Json(OptionsIvResponse {
        ticker: safe_ticker,
        expiration_date: params.expiration_date,
        underlying_price: spot_price,
        risk_free_rate: r,
        dividend_yield: q,
        count,
        contracts,
        message,
    }))
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn test_standard_normal_cdf_and_pdf() {
        // N(0) = 0.5
        assert!((standard_normal_cdf(0.0) - 0.5).abs() < 1e-6);
        // N(1.96) ~= 0.975
        assert!((standard_normal_cdf(1.96) - 0.9750).abs() < 1e-4);
        // N(-1.96) ~= 0.025
        assert!((standard_normal_cdf(-1.96) - 0.0250).abs() < 1e-4);
        // N'(0) = 1/sqrt(2pi) ~= 0.398942
        assert!((standard_normal_pdf(0.0) - 0.398942).abs() < 1e-5);
    }

    #[test]
    fn test_black_scholes_call_and_put_pricing_benchmark() {
        // Textbook Hull Example: S=42, K=40, r=0.10, q=0.0, T=0.5, sigma=0.20
        let s = 42.0;
        let k = 40.0;
        let r = 0.10;
        let q = 0.0;
        let t = 0.5;
        let sigma = 0.20;

        let call_price = black_scholes_price(OptionType::Call, s, k, t, r, q, sigma);
        let put_price = black_scholes_price(OptionType::Put, s, k, t, r, q, sigma);

        // Expected Call ~= 4.76, Put ~= 0.81
        assert!(
            (call_price - 4.7594).abs() < 1e-3,
            "Got call price: {}",
            call_price
        );
        assert!(
            (put_price - 0.8086).abs() < 1e-3,
            "Got put price: {}",
            put_price
        );
    }

    #[test]
    fn test_put_call_parity() {
        // Parity: C - P = S*e^(-qT) - K*e^(-rT)
        let s = 225.0;
        let k = 230.0;
        let r = 0.05;
        let q = 0.005;
        let t = 0.25; // 3 months
        let sigma = 0.28;

        let c = black_scholes_price(OptionType::Call, s, k, t, r, q, sigma);
        let p = black_scholes_price(OptionType::Put, s, k, t, r, q, sigma);

        let lhs = c - p;
        let rhs = (s * (-q * t).exp()) - (k * (-r * t).exp());

        assert!(
            (lhs - rhs).abs() < 1e-4,
            "Parity check failed: LHS={}, RHS={}",
            lhs,
            rhs
        );
    }

    #[test]
    fn test_black_scholes_greeks_properties() {
        let s = 100.0;
        let k = 100.0;
        let r = 0.05;
        let q = 0.0;
        let t = 0.5;
        let sigma = 0.25;

        let call_greeks = black_scholes_greeks(OptionType::Call, s, k, t, r, q, sigma);
        let put_greeks = black_scholes_greeks(OptionType::Put, s, k, t, r, q, sigma);

        // Delta: Call in (0, 1), Put in (-1, 0)
        assert!(call_greeks.delta > 0.0 && call_greeks.delta < 1.0);
        assert!(put_greeks.delta > -1.0 && put_greeks.delta < 0.0);
        // Call Delta - Put Delta = e^(-qT) ~= 1.0
        assert!((call_greeks.delta - put_greeks.delta - 1.0).abs() < 1e-3);

        // Gamma must be strictly positive and identical for Call and Put
        assert!(call_greeks.gamma > 0.0);
        assert!((call_greeks.gamma - put_greeks.gamma).abs() < 1e-5);

        // Vega must be strictly positive
        assert!(call_greeks.vega > 0.0);

        // Theta is typically negative for long options (time decay)
        assert!(call_greeks.theta < 0.0);
    }

    #[test]
    fn test_implied_volatility_solver_convergence() {
        let s = 224.50;
        let k = 225.00;
        let r = 0.05;
        let q = 0.0;
        let t = 0.25;
        let target_vol = 0.3250;

        let call_price = black_scholes_price(OptionType::Call, s, k, t, r, q, target_vol);
        let solved_iv = calculate_implied_volatility(OptionType::Call, call_price, s, k, t, r, q);

        assert!(
            (solved_iv - target_vol).abs() < 1e-3,
            "Target vol {}, solved IV {}",
            target_vol,
            solved_iv
        );

        let put_price = black_scholes_price(OptionType::Put, s, k, t, r, q, target_vol);
        let solved_put_iv = calculate_implied_volatility(OptionType::Put, put_price, s, k, t, r, q);

        assert!(
            (solved_put_iv - target_vol).abs() < 1e-3,
            "Target vol {}, solved Put IV {}",
            target_vol,
            solved_put_iv
        );
    }

    #[test]
    fn test_generate_mock_options_chain_atm() {
        let chain =
            generate_mock_options_chain("AAPL", "2025-12-19", 224.50, 0.05, 0.0, None, Some("all"))
                .expect("Should generate chain");

        assert!(!chain.is_empty());
        assert!(chain.len() >= 10);
        for c in &chain {
            assert_eq!(c.underlying_ticker, "AAPL");
            assert!(c.strike > 0.0);
            assert!(c.implied_volatility > 0.0);
            assert!(c.bid > 0.0);
            assert!(c.ask >= c.bid);
        }
    }

    #[test]
    fn test_generate_mock_options_chain_single_strike_call() {
        let chain = generate_mock_options_chain(
            "NVDA",
            "2025-12-19",
            125.00,
            0.05,
            0.0,
            Some(130.0),
            Some("call"),
        )
        .expect("Should generate single contract");

        assert_eq!(chain.len(), 1);
        assert_eq!(chain[0].underlying_ticker, "NVDA");
        assert_eq!(chain[0].strike, 130.0);
        assert_eq!(chain[0].option_type, "CALL");
        assert!(chain[0].delta > 0.0 && chain[0].delta < 1.0);
    }
}
