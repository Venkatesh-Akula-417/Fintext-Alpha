//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Quantitative Market Microstructure Engine
//! ═══════════════════════════════════════════════════════════════════════════════
//!
//! Provides institutional calculation of:
//!   1. Volume-Synchronized Probability of Informed Trading (VPIN):
//!      Quantifies order flow toxicity and aggressive informed trading pressure
//!      using volume-synchronized bucket imbalances and tick rule side classification.
//!   2. Dealer Gamma Exposure (GEX):
//!      Measures market maker delta-hedging reflexivity across option chains via
//!      Black-Scholes analytical gamma aggregation.
//! ═══════════════════════════════════════════════════════════════════════════════

use crate::sources::polygon::{OptionTrade, PolygonClient};
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;
use tracing::debug;

/// Comprehensive breakdown of VPIN calculation metrics.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VpinResult {
    /// Bounded VPIN score between 0.0 (uninformed/symmetric) and 1.0 (pure toxic informed flow)
    pub vpin: f64,
    /// Total classified buy contract volume
    pub buy_volume: u64,
    /// Total classified sell contract volume
    pub sell_volume: u64,
    /// Total contract volume across all buckets
    pub total_volume: u64,
    /// Number of volume buckets analyzed
    pub num_buckets: usize,
}

/// Breakdown of Dealer Gamma Exposure (GEX) metrics.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GexResult {
    /// Net Dealer Gamma Exposure (Positive Call Gamma - Negative Put Gamma)
    pub total_gamma_exposure: f64,
    /// Total Call Gamma Exposure ($ per 1% spot move)
    pub positive_gamma: f64,
    /// Total Put Gamma Exposure ($ per 1% spot move)
    pub negative_gamma: f64,
}

/// Standard normal probability density function N'(x).
#[inline]
pub fn standard_normal_pdf(x: f64) -> f64 {
    (-0.5 * x * x).exp() / (2.0 * PI).sqrt()
}

/// Calculates Black-Scholes analytical gamma Γ:
/// Γ = N'(d1) / (S * σ * sqrt(T))
/// where d1 = (ln(S/K) + (r - q + 0.5 * σ^2) * T) / (σ * sqrt(T))
pub fn calculate_black_scholes_gamma(
    spot_price: f64,
    strike_price: f64,
    risk_free_rate: f64,
    dividend_yield: f64,
    time_to_expiry_years: f64,
    volatility: f64,
) -> Result<f64, String> {
    if spot_price <= 0.0 {
        return Err(format!("Spot price must be positive, got {}", spot_price));
    }
    if strike_price <= 0.0 {
        return Err(format!(
            "Strike price must be positive, got {}",
            strike_price
        ));
    }
    if volatility <= 0.0 {
        return Err(format!("Volatility must be positive, got {}", volatility));
    }
    if time_to_expiry_years <= 0.0 {
        return Err(format!(
            "Time to expiry must be positive, got {}",
            time_to_expiry_years
        ));
    }

    let sqrt_t = time_to_expiry_years.sqrt();
    let denom = volatility * sqrt_t;
    let d1 = ((spot_price / strike_price).ln()
        + (risk_free_rate - dividend_yield + 0.5 * volatility * volatility) * time_to_expiry_years)
        / denom;

    let n_prime_d1 = standard_normal_pdf(d1);
    let gamma = n_prime_d1 / (spot_price * denom);

    Ok(gamma)
}

/// Computes Volume-Synchronized Probability of Informed Trading (VPIN) from option trade prints.
///
/// Implements tick rule classification:
///   - If trade price > last price: classified as Buy
///   - If trade price < last price: classified as Sell
///   - If trade price == last price: retains previous trade's side (defaults to Buy for first trade)
///
/// Trades are aggregated into volume buckets of `bucket_size` contracts.
/// VPIN is calculated as the average absolute volume imbalance across all buckets.
///
/// # Arguments
/// * `trades` - Slice of `OptionTrade` execution records
/// * `bucket_size` - Contract volume per bucket (e.g. 50, 100, 500)
pub fn compute_vpin(trades: &[OptionTrade], bucket_size: usize) -> Result<f64, String> {
    let result = compute_vpin_detailed(trades, bucket_size)?;
    Ok(result.vpin)
}

/// Detailed calculation of VPIN returning full volume breakdown and bucket count.
pub fn compute_vpin_detailed(
    trades: &[OptionTrade],
    bucket_size: usize,
) -> Result<VpinResult, String> {
    if trades.is_empty() {
        return Err("Cannot compute VPIN on empty trades list".to_string());
    }
    if bucket_size == 0 {
        return Err("VPIN bucket size must be strictly greater than 0".to_string());
    }

    // Sort trades chronologically by execution timestamp
    let mut sorted_trades = trades.to_vec();
    sorted_trades.sort_by_key(|t| t.timestamp_us);

    let mut last_price: Option<f64> = None;
    let mut last_side = true; // true = Buy, false = Sell
    let mut current_bucket_vol = 0u64;
    let mut current_buy_vol = 0u64;
    let mut current_sell_vol = 0u64;
    let mut bucket_imbalances: Vec<f64> = Vec::new();

    let mut total_buy_volume = 0u64;
    let mut total_sell_volume = 0u64;

    for trade in &sorted_trades {
        if trade.size == 0 {
            continue;
        }

        // Tick rule side classification
        let side = match last_price {
            None => true,
            Some(lp) => {
                if trade.price > lp {
                    true
                } else if trade.price < lp {
                    false
                } else {
                    last_side
                }
            }
        };

        last_price = Some(trade.price);
        last_side = side;

        if side {
            current_buy_vol += trade.size;
            total_buy_volume += trade.size;
        } else {
            current_sell_vol += trade.size;
            total_sell_volume += trade.size;
        }
        current_bucket_vol += trade.size;

        // Close bucket when target volume reached
        if current_bucket_vol >= bucket_size as u64 {
            let imbalance = (current_buy_vol as f64 - current_sell_vol as f64).abs()
                / (current_bucket_vol as f64);
            bucket_imbalances.push(imbalance);
            current_bucket_vol = 0;
            current_buy_vol = 0;
            current_sell_vol = 0;
        }
    }

    // Include partial trailing bucket if present
    if current_bucket_vol > 0 {
        let imbalance =
            (current_buy_vol as f64 - current_sell_vol as f64).abs() / (current_bucket_vol as f64);
        bucket_imbalances.push(imbalance);
    }

    if bucket_imbalances.is_empty() {
        return Err("No trade volume available to form VPIN buckets".to_string());
    }

    let num_buckets = bucket_imbalances.len();
    let raw_vpin = bucket_imbalances.iter().sum::<f64>() / (num_buckets as f64);
    let vpin = raw_vpin.clamp(0.0, 1.0);

    debug!(
        "[VPIN] Computed VPIN = {:.4} across {} buckets (Total Vol: {}, Buy: {}, Sell: {})",
        vpin,
        num_buckets,
        total_buy_volume + total_sell_volume,
        total_buy_volume,
        total_sell_volume
    );

    Ok(VpinResult {
        vpin,
        buy_volume: total_buy_volume,
        sell_volume: total_sell_volume,
        total_volume: total_buy_volume + total_sell_volume,
        num_buckets,
    })
}

/// Computes Dealer Gamma Exposure (GEX) across option trades.
///
/// Formula:
///   Call GEX = + (Γ * Size * Multiplier * Spot^2 * 0.01)
///   Put GEX  = - (Γ * Size * Multiplier * Spot^2 * 0.01)
///   Net GEX  = Sum(Call GEX) - Sum(Put GEX)
///
/// # Arguments
/// * `trades` - Slice of option trade records
/// * `spot_price` - Current underlying asset spot price ($)
/// * `risk_free_rate` - Annualized risk-free interest rate (e.g. 0.045)
/// * `dividend_yield` - Annualized continuous dividend yield (e.g. 0.015)
/// * `time_to_expiry_years` - Time to expiration in years (e.g. 30.0 / 365.0)
/// * `volatility` - Annualized implied volatility (e.g. 0.25)
/// * `contract_multiplier` - Number of shares per option contract (default 100.0)
pub fn compute_gex(
    trades: &[OptionTrade],
    spot_price: f64,
    risk_free_rate: f64,
    dividend_yield: f64,
    time_to_expiry_years: f64,
    volatility: f64,
    contract_multiplier: f64,
) -> Result<GexResult, String> {
    if trades.is_empty() {
        return Err("Cannot compute GEX on empty trades list".to_string());
    }
    if spot_price <= 0.0 {
        return Err(format!("Spot price must be positive, got {}", spot_price));
    }
    if volatility <= 0.0 {
        return Err(format!("Volatility must be positive, got {}", volatility));
    }
    if time_to_expiry_years <= 0.0 {
        return Err(format!(
            "Time to expiry must be positive, got {}",
            time_to_expiry_years
        ));
    }
    if contract_multiplier <= 0.0 {
        return Err(format!(
            "Contract multiplier must be positive, got {}",
            contract_multiplier
        ));
    }

    let mut positive_gamma = 0.0;
    let mut negative_gamma = 0.0;

    for trade in trades {
        if trade.size == 0 || trade.strike <= 0.0 {
            continue;
        }

        let gamma = calculate_black_scholes_gamma(
            spot_price,
            trade.strike,
            risk_free_rate,
            dividend_yield,
            time_to_expiry_years,
            volatility,
        )?;

        // Standard 1% move dollar gamma exposure: Γ * contracts * multiplier * Spot^2 * 0.01
        let dollar_gamma =
            gamma * (trade.size as f64) * contract_multiplier * (spot_price * spot_price) * 0.01;

        if trade.option_type.to_uppercase() == "CALL" {
            positive_gamma += dollar_gamma;
        } else {
            negative_gamma += dollar_gamma;
        }
    }

    let total_gamma_exposure = positive_gamma - negative_gamma;

    debug!(
        "[GEX] Spot: ${:.2} | Net GEX: ${:.2} (Call: +${:.2}, Put: -${:.2})",
        spot_price, total_gamma_exposure, positive_gamma, negative_gamma
    );

    Ok(GexResult {
        total_gamma_exposure,
        positive_gamma,
        negative_gamma,
    })
}

/// High-level composite function to fetch option trades from Polygon.io and compute both VPIN and GEX.
pub async fn compute_vpin_and_gex_from_polygon(
    client: &PolygonClient,
    options_ticker: &str,
    date: &str,
    spot_price: f64,
    volatility: f64,
    risk_free_rate: f64,
) -> Result<(f64, GexResult), String> {
    let trades = client
        .fetch_options_trades(options_ticker, date, 5000)
        .await?;

    let bucket_size = 100;
    let vpin = compute_vpin(&trades, bucket_size)?;

    let time_to_expiry_years = 30.0 / 365.0;
    let gex = compute_gex(
        &trades,
        spot_price,
        risk_free_rate,
        0.0,
        time_to_expiry_years,
        volatility,
        100.0,
    )?;

    Ok((vpin, gex))
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn test_vpin_with_known_imbalance() {
        // Create 4 trades: 2 buys (price rising), 2 sells (price falling)
        let trades = vec![
            OptionTrade {
                options_ticker: "O:AAPL240119C00150000".to_string(),
                underlying: "AAPL".to_string(),
                expiry: "2024-01-19".to_string(),
                option_type: "CALL".to_string(),
                strike: 150.0,
                price: 5.00,
                size: 50,
                timestamp_us: 1000,
                exchange: None,
                conditions: vec![],
            },
            OptionTrade {
                options_ticker: "O:AAPL240119C00150000".to_string(),
                underlying: "AAPL".to_string(),
                expiry: "2024-01-19".to_string(),
                option_type: "CALL".to_string(),
                strike: 150.0,
                price: 5.10, // Higher -> Buy
                size: 50,
                timestamp_us: 2000,
                exchange: None,
                conditions: vec![],
            },
            OptionTrade {
                options_ticker: "O:AAPL240119C00150000".to_string(),
                underlying: "AAPL".to_string(),
                expiry: "2024-01-19".to_string(),
                option_type: "CALL".to_string(),
                strike: 150.0,
                price: 4.90, // Lower -> Sell
                size: 50,
                timestamp_us: 3000,
                exchange: None,
                conditions: vec![],
            },
            OptionTrade {
                options_ticker: "O:AAPL240119C00150000".to_string(),
                underlying: "AAPL".to_string(),
                expiry: "2024-01-19".to_string(),
                option_type: "CALL".to_string(),
                strike: 150.0,
                price: 4.80, // Lower -> Sell
                size: 50,
                timestamp_us: 4000,
                exchange: None,
                conditions: vec![],
            },
        ];

        // Bucket size 100:
        // Bucket 1: 50 Buy + 50 Buy = 100 Buy, 0 Sell -> Imbalance = |100 - 0| / 100 = 1.0
        // Bucket 2: 50 Sell + 50 Sell = 100 Sell, 0 Buy -> Imbalance = |0 - 100| / 100 = 1.0
        // Expected VPIN = (1.0 + 1.0) / 2 = 1.0
        let detailed = compute_vpin_detailed(&trades, 100).expect("VPIN calculation failed");
        assert_eq!(detailed.buy_volume, 100);
        assert_eq!(detailed.sell_volume, 100);
        assert_eq!(detailed.total_volume, 200);
        assert_eq!(detailed.num_buckets, 2);
        assert!((detailed.vpin - 1.0).abs() < 1e-6);

        // Bucket size 200:
        // Bucket 1: 100 Buy + 100 Sell = 200 Total -> Imbalance = |100 - 100| / 200 = 0.0
        let vpin_balanced = compute_vpin(&trades, 200).expect("VPIN calculation failed");
        assert_eq!(vpin_balanced, 0.0);
    }

    #[test]
    fn test_black_scholes_gamma_and_gex() {
        let spot = 100.0;
        let strike = 100.0;
        let r = 0.05;
        let q = 0.0;
        let t = 1.0; // 1 year
        let vol = 0.20;

        let gamma =
            calculate_black_scholes_gamma(spot, strike, r, q, t, vol).expect("BS gamma failed");

        // Manually compute d1: (ln(1.0) + (0.05 + 0.5*0.04)*1.0) / (0.20*1.0) = 0.07 / 0.20 = 0.35
        // N'(0.35) = exp(-0.35^2 / 2) / sqrt(2*pi) = exp(-0.06125) / 2.50663 = 0.94059 / 2.50663 ≈ 0.37524
        // Gamma = N'(d1) / (S * sigma * sqrt(T)) = 0.37524 / (100 * 0.20 * 1) = 0.37524 / 20 ≈ 0.01876
        assert!((gamma - 0.01876).abs() < 0.001);

        // 1 Call trade (10 contracts) and 1 Put trade (5 contracts)
        let trades = vec![
            OptionTrade {
                options_ticker: "O:TEST240119C00100000".to_string(),
                underlying: "TEST".to_string(),
                expiry: "2024-01-19".to_string(),
                option_type: "CALL".to_string(),
                strike: 100.0,
                price: 5.0,
                size: 10,
                timestamp_us: 1000,
                exchange: None,
                conditions: vec![],
            },
            OptionTrade {
                options_ticker: "O:TEST240119P00100000".to_string(),
                underlying: "TEST".to_string(),
                expiry: "2024-01-19".to_string(),
                option_type: "PUT".to_string(),
                strike: 100.0,
                price: 5.0,
                size: 5,
                timestamp_us: 2000,
                exchange: None,
                conditions: vec![],
            },
        ];

        let gex_res =
            compute_gex(&trades, spot, r, q, t, vol, 100.0).expect("GEX calculation failed");

        // Call GEX = 0.01876 * 10 * 100 * 100^2 * 0.01 = 0.01876 * 1000 * 100 = 1876.0
        // Put GEX = 0.01876 * 5 * 100 * 100^2 * 0.01 = 938.0
        // Net GEX = 1876.0 - 938.0 = 938.0
        assert!(gex_res.positive_gamma > 0.0);
        assert!(gex_res.negative_gamma > 0.0);
        assert!(gex_res.total_gamma_exposure > 0.0);
        assert!((gex_res.positive_gamma - 2.0 * gex_res.negative_gamma).abs() < 1e-4);
    }

    #[test]
    fn test_error_handling_empty_and_invalid_inputs() {
        let empty_trades: Vec<OptionTrade> = vec![];
        assert!(compute_vpin(&empty_trades, 100).is_err());
        assert!(compute_gex(&empty_trades, 100.0, 0.05, 0.0, 1.0, 0.20, 100.0).is_err());

        // Zero bucket size
        let sample_trade = vec![OptionTrade {
            options_ticker: "O:AAPL240119C00150000".to_string(),
            underlying: "AAPL".to_string(),
            expiry: "2024-01-19".to_string(),
            option_type: "CALL".to_string(),
            strike: 150.0,
            price: 5.0,
            size: 10,
            timestamp_us: 1000,
            exchange: None,
            conditions: vec![],
        }];
        assert!(compute_vpin(&sample_trade, 0).is_err());

        // Invalid spot price or volatility
        assert!(compute_gex(&sample_trade, -100.0, 0.05, 0.0, 1.0, 0.20, 100.0).is_err());
        assert!(compute_gex(&sample_trade, 100.0, 0.05, 0.0, 1.0, -0.20, 100.0).is_err());
    }
}
