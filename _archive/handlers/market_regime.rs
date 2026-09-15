//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Quantitative Market Regime Detection Handler
//! ═══════════════════════════════════════════════════════════════════════════════
//!
//! Synthesizes sector-level aggregated sentiment, market breadth, cross-asset
//! information spillovers, and options volatility proxies into a statistical
//! macro market regime classification (Bullish, Bearish, Neutral, High Volatility).
//! ═══════════════════════════════════════════════════════════════════════════════

use axum::extract::Query;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use chrono::{DateTime, Duration, Utc};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use tracing::info;

use crate::auth::AuthErrorResponse;
use crate::models::{MarketRegimeParams, MarketRegimeResponse, RegimeComponents, SentimentRecord};
use crate::sector::GLOBAL_SECTOR_MAP;
use crate::storage::{QuestDbClient, QuestDbClientConfig};

static QUESTDB_CLIENT: Lazy<QuestDbClient> =
    Lazy::new(|| QuestDbClient::new(QuestDbClientConfig::default()));

pub const DEFAULT_LOOKBACK_DAYS: i64 = 5;
pub const MIN_LOOKBACK_DAYS: i64 = 1;
pub const MAX_LOOKBACK_DAYS: i64 = 30;

pub const DEFAULT_MIN_DATA_POINTS: usize = 50;
pub const MIN_DATA_POINTS_LOWER: usize = 1;
pub const MAX_DATA_POINTS_UPPER: usize = 1000;

pub const HIGH_VOLATILITY_THRESHOLD: f64 = 0.35;
pub const BULLISH_SENTIMENT_THRESHOLD: f64 = 0.15;
pub const BULLISH_BREADTH_THRESHOLD: f64 = 0.55;
pub const BEARISH_SENTIMENT_THRESHOLD: f64 = -0.15;
pub const BEARISH_BREADTH_THRESHOLD: f64 = 0.45;

/// Core representative universe for cross-asset spillover correlation.
pub const CORE_REGIME_UNIVERSE: &[&str] = &[
    "AAPL", "MSFT", "NVDA", "AMZN", "GOOGL", "META", "TSLA", "JPM", "JNJ", "XOM",
];

/// Classify macro market regime based on multi-factor quantitative metrics.
pub fn classify_market_regime(
    market_sentiment: f64,
    breadth: f64,
    volatility_proxy: f64,
) -> &'static str {
    if volatility_proxy >= HIGH_VOLATILITY_THRESHOLD {
        "High Volatility"
    } else if market_sentiment > BULLISH_SENTIMENT_THRESHOLD && breadth > BULLISH_BREADTH_THRESHOLD
    {
        "Bullish"
    } else if market_sentiment < BEARISH_SENTIMENT_THRESHOLD && breadth < BEARISH_BREADTH_THRESHOLD
    {
        "Bearish"
    } else {
        "Neutral"
    }
}

/// Compute statistical confidence based on data sample size and signal conviction.
pub fn compute_regime_confidence(
    total_data_points: usize,
    min_data_points: usize,
    market_sentiment: f64,
    breadth: f64,
    volatility_proxy: f64,
) -> f64 {
    let coverage = (total_data_points as f64 / min_data_points.max(1) as f64).min(1.0);
    let conviction = if volatility_proxy >= HIGH_VOLATILITY_THRESHOLD {
        ((volatility_proxy - HIGH_VOLATILITY_THRESHOLD) * 3.0 + 0.6).min(1.0)
    } else {
        (market_sentiment.abs() * 2.5 + (breadth - 0.5).abs() * 2.0)
            .min(1.0)
            .max(0.3)
    };

    let score = 0.5 * coverage + 0.5 * conviction;
    score.clamp(0.20, 0.99)
}

/// Parse and normalize user-supplied sector weights.
pub fn parse_sector_weights(
    weights_str: Option<&str>,
    active_sectors: &[String],
) -> Result<HashMap<String, f64>, String> {
    let mut weights: HashMap<String, f64> = HashMap::new();

    if let Some(s) = weights_str {
        let trimmed = s.trim();
        if !trimmed.is_empty() {
            if trimmed.contains(':') {
                // Key-value pairs: "Technology:0.4,Financials:0.3,Healthcare:0.3"
                for pair in trimmed.split(',') {
                    let mut parts = pair.splitn(2, ':');
                    let key = parts.next().unwrap_or("").trim();
                    let val_str = parts.next().unwrap_or("").trim();

                    let val: f64 = val_str.parse().map_err(|_| {
                        format!("Invalid float weight '{}' for sector '{}'", val_str, key)
                    })?;

                    if val < 0.0 {
                        return Err(format!("Sector weight for '{}' must be non-negative", key));
                    }

                    // Find canonical sector matching case-insensitively
                    let canonical = active_sectors
                        .iter()
                        .find(|sec| sec.eq_ignore_ascii_case(key))
                        .cloned()
                        .unwrap_or_else(|| key.to_string());

                    weights.insert(canonical, val);
                }
            } else {
                // Comma-separated floats matching alphabetical sector order
                let float_vals: Vec<f64> = trimmed
                    .split(',')
                    .map(|v| {
                        v.trim().parse::<f64>().map_err(|_| {
                            format!(
                                "Invalid float weight value '{}' in comma-separated list",
                                v.trim()
                            )
                        })
                    })
                    .collect::<Result<Vec<f64>, String>>()?;

                for (sec, &val) in active_sectors.iter().zip(float_vals.iter()) {
                    if val < 0.0 {
                        return Err(format!("Sector weight for '{}' must be non-negative", sec));
                    }
                    weights.insert(sec.clone(), val);
                }
            }
        }
    }

    // Default missing active sectors to equal weights or uniform
    let mut total_weight = 0.0;
    for sec in active_sectors {
        let w = weights.entry(sec.clone()).or_insert(1.0);
        total_weight += *w;
    }

    if total_weight > 0.0 {
        for w in weights.values_mut() {
            *w /= total_weight;
        }
    }

    Ok(weights)
}

/// Generate deterministic mock sentiment observations for a lookback window.
pub fn generate_regime_sentiment_records(
    lookback_days: i64,
    start_dt: DateTime<Utc>,
    _end_dt: DateTime<Utc>,
) -> Vec<SentimentRecord> {
    let mut records = Vec::new();
    let sector_map = GLOBAL_SECTOR_MAP.clone();

    // Iterate across all sectors and constituent tickers
    for sector in &sector_map.sectors {
        let tickers = sector_map
            .get_tickers_by_sector(sector)
            .cloned()
            .unwrap_or_default();

        for (ticker_idx, ticker) in tickers.iter().enumerate() {
            // Seed base sentiment per sector
            let base_bias = match sector.as_str() {
                "Technology" => 0.28,
                "Communication Services" => 0.22,
                "Consumer Cyclical" => 0.16,
                "Financials" => 0.12,
                "Healthcare" => 0.08,
                "Industrials" => 0.06,
                "Materials" => 0.02,
                "Real Estate" => -0.04,
                "Utilities" => -0.08,
                "Energy" => -0.14,
                _ => 0.05,
            };

            // Generate records across the lookback window
            let points_per_ticker = (lookback_days * 3).max(4) as usize;
            for i in 0..points_per_ticker {
                let offset_secs = (i as i64 * (lookback_days * 86400 / points_per_ticker as i64))
                    .min(lookback_days * 86400);
                let record_ts = start_dt + Duration::seconds(offset_secs);

                // Deterministic oscillation
                let var = (((ticker_idx * 17 + i * 31) % 100) as f64 - 45.0) / 150.0;
                let score = (base_bias + var).clamp(-0.95, 0.95);

                let ing_ts = record_ts + Duration::milliseconds(50);
                let com_ts = record_ts + Duration::milliseconds(100);

                records.push(SentimentRecord {
                    published_utc: record_ts.to_rfc3339(),
                    ticker: ticker.clone(),
                    source: "Institutional Wire".to_string(),
                    title: format!("{} sentiment and flow analysis", ticker),
                    sentiment_score: score,
                    vpin: 0.24 + (((i + ticker_idx) % 20) as f64 * 0.01),
                    gamma_exposure: 1_200_000.0,
                    data_quality_score: 0.88,
                    model_version: Some(crate::models::DEFAULT_MODEL_VERSION.to_string()),
                    pipeline_version: Some(crate::models::DEFAULT_PIPELINE_VERSION.to_string()),
                    data_provenance: Some(vec!["Institutional Wire".to_string()]),
                    language: "english".to_string(),
                    ingested_utc: Some(ing_ts.to_rfc3339()),
                    db_commit_utc: Some(com_ts.to_rfc3339()),
                    valid_from: Some(com_ts.to_rfc3339()),
                    valid_to: None,
                    revision_number: Some(1),
                    is_current: Some(true),
                    ..Default::default()
                });
            }
        }
    }

    records
}

/// Calculate standard deviation of sentiment scores.
pub fn calculate_sentiment_std_dev(scores: &[f64]) -> f64 {
    if scores.len() < 2 {
        return 0.18; // Sensible baseline volatility proxy
    }

    let n = scores.len() as f64;
    let mean = scores.iter().sum::<f64>() / n;
    let variance = scores.iter().map(|s| (s - mean).powi(2)).sum::<f64>() / (n - 1.0);
    variance.sqrt().max(0.05)
}

/// Compute average pairwise absolute cross-asset spillover correlation.
pub fn calculate_avg_spillover_correlation(records_by_ticker: &HashMap<String, Vec<f64>>) -> f64 {
    let active_tickers: Vec<String> = CORE_REGIME_UNIVERSE
        .iter()
        .map(|&s| s.to_string())
        .filter(|t| records_by_ticker.contains_key(t))
        .collect();

    if active_tickers.len() < 2 {
        return 0.42; // Default baseline cross-asset correlation
    }

    let mut total_corr = 0.0;
    let mut pair_count = 0;

    for i in 0..active_tickers.len() {
        for j in (i + 1)..active_tickers.len() {
            let t1 = &active_tickers[i];
            let t2 = &active_tickers[j];

            if let (Some(s1), Some(s2)) = (records_by_ticker.get(t1), records_by_ticker.get(t2)) {
                let min_len = s1.len().min(s2.len());
                if min_len >= 2 {
                    let sub1 = &s1[..min_len];
                    let sub2 = &s2[..min_len];

                    let mean1 = sub1.iter().sum::<f64>() / min_len as f64;
                    let mean2 = sub2.iter().sum::<f64>() / min_len as f64;

                    let mut cov = 0.0;
                    let mut var1 = 0.0;
                    let mut var2 = 0.0;

                    for k in 0..min_len {
                        let d1 = sub1[k] - mean1;
                        let d2 = sub2[k] - mean2;
                        cov += d1 * d2;
                        var1 += d1 * d1;
                        var2 += d2 * d2;
                    }

                    if var1 > 1e-9 && var2 > 1e-9 {
                        let r = (cov / (var1.sqrt() * var2.sqrt())).clamp(-1.0, 1.0);
                        total_corr += r.abs();
                        pair_count += 1;
                    }
                }
            }
        }
    }

    if pair_count > 0 {
        (total_corr / pair_count as f64).clamp(0.0, 1.0)
    } else {
        0.42
    }
}

/// Query Macro Market Regime Classification.
///
/// Synthesizes aggregate cross-sector sentiment, market breadth, cross-asset
/// information spillovers, and options volatility proxies into an actionable macro regime.
#[utoipa::path(
    get,
    path = "/market/regime",
    tag = "Market Intelligence",
    params(
        ("lookback_days" = Option<i64>, Query, description = "Lookback window in days (default: 5, min: 1, max: 30)"),
        ("sector_weights" = Option<String>, Query, description = "Optional sector weighting string (e.g. 'Technology:0.3,Financials:0.2' or comma-separated floats)"),
        ("min_data_points" = Option<usize>, Query, description = "Minimum sentiment records required for high statistical confidence (default: 50, min: 1, max: 1000)")
    ),
    responses(
        (status = 200, description = "Market regime classification synthesized successfully", body = MarketRegimeResponse),
        (status = 400, description = "Invalid lookback days, data points bounds, or malformed weights", body = AuthErrorResponse),
        (status = 401, description = "Unauthorized (missing or invalid Bearer JWT or API key)", body = AuthErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::rate_limit::RateLimitErrorResponse)
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn get_market_regime_handler(Query(params): Query<MarketRegimeParams>) -> Response {
    let lookback_days = params.lookback_days.unwrap_or(DEFAULT_LOOKBACK_DAYS);
    if lookback_days < MIN_LOOKBACK_DAYS || lookback_days > MAX_LOOKBACK_DAYS {
        let err = AuthErrorResponse {
            error: "Bad Request".to_string(),
            message: format!(
                "lookback_days must be between {} and {} days (received: {})",
                MIN_LOOKBACK_DAYS, MAX_LOOKBACK_DAYS, lookback_days
            ),
        };
        return (StatusCode::BAD_REQUEST, Json(err)).into_response();
    }

    let min_data_points = params.min_data_points.unwrap_or(DEFAULT_MIN_DATA_POINTS);
    if min_data_points < MIN_DATA_POINTS_LOWER || min_data_points > MAX_DATA_POINTS_UPPER {
        let err = AuthErrorResponse {
            error: "Bad Request".to_string(),
            message: format!(
                "min_data_points must be between {} and {} (received: {})",
                MIN_DATA_POINTS_LOWER, MAX_DATA_POINTS_UPPER, min_data_points
            ),
        };
        return (StatusCode::BAD_REQUEST, Json(err)).into_response();
    }

    let sector_map = GLOBAL_SECTOR_MAP.clone();
    let mut active_sectors = sector_map.sectors.clone();
    if active_sectors.is_empty() {
        active_sectors = vec![
            "Technology".to_string(),
            "Financials".to_string(),
            "Healthcare".to_string(),
            "Energy".to_string(),
            "Consumer Cyclical".to_string(),
            "Industrials".to_string(),
            "Communication Services".to_string(),
            "Utilities".to_string(),
            "Real Estate".to_string(),
            "Materials".to_string(),
        ];
    }
    active_sectors.sort();

    // Parse sector weights
    let normalized_weights =
        match parse_sector_weights(params.sector_weights.as_deref(), &active_sectors) {
            Ok(w) => w,
            Err(e) => {
                let err = AuthErrorResponse {
                    error: "Bad Request".to_string(),
                    message: format!("Invalid sector_weights parameter: {}", e),
                };
                return (StatusCode::BAD_REQUEST, Json(err)).into_response();
            }
        };

    let now = Utc::now();
    let start_dt = now - Duration::days(lookback_days);

    // 1. Fetch sentiment observations (QuestDB query with mock fallback)
    let start_str = start_dt.format("%Y-%m-%d").to_string();
    let end_str = now.format("%Y-%m-%d").to_string();

    let records = if crate::state::is_questdb_mock_fallback_enabled() {
        generate_regime_sentiment_records(lookback_days, start_dt, now)
    } else {
        let sql = format!(
            "SELECT timestamp, ticker, source, title, sentiment_score, vpin, gamma_exposure \
             FROM sentiment_news \
             WHERE timestamp >= '{}' AND timestamp <= '{}' \
             ORDER BY timestamp ASC \
             LIMIT 5000;",
            start_str, end_str
        );
        let endpoint = format!("{}/exec", QUESTDB_CLIENT.config().url.trim_end_matches('/'));
        match reqwest::Client::new()
            .get(&endpoint)
            .query(&[("query", &sql)])
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                if let Ok(val) = resp.json::<serde_json::Value>().await {
                    if let Ok(parsed) = QuestDbClient::parse_sentiment_history_exec_response(&val) {
                        parsed
                    } else if crate::state::is_production_mode() {
                        return (
                            StatusCode::SERVICE_UNAVAILABLE,
                            Json(serde_json::json!({
                                "error": "Service Unavailable",
                                "message": "Required data source unavailable in production mode.",
                                "status": "service_unavailable"
                            })),
                        )
                            .into_response();
                    } else {
                        generate_regime_sentiment_records(lookback_days, start_dt, now)
                    }
                } else if crate::state::is_production_mode() {
                    return (
                        StatusCode::SERVICE_UNAVAILABLE,
                        Json(serde_json::json!({
                            "error": "Service Unavailable",
                            "message": "Required data source unavailable in production mode.",
                            "status": "service_unavailable"
                        })),
                    )
                        .into_response();
                } else {
                    generate_regime_sentiment_records(lookback_days, start_dt, now)
                }
            }
            _ => {
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
                generate_regime_sentiment_records(lookback_days, start_dt, now)
            }
        }
    };

    // 2. Aggregate sentiment by ticker and by sector
    let mut scores_by_ticker: HashMap<String, Vec<f64>> = HashMap::new();
    let mut scores_by_sector: HashMap<String, Vec<f64>> = HashMap::new();
    let mut all_sentiment_scores: Vec<f64> = Vec::with_capacity(records.len());

    for rec in &records {
        all_sentiment_scores.push(rec.sentiment_score);
        scores_by_ticker
            .entry(rec.ticker.to_uppercase())
            .or_default()
            .push(rec.sentiment_score);

        if let Some(sec) = sector_map.ticker_to_sector.get(&rec.ticker.to_uppercase()) {
            scores_by_sector
                .entry(sec.clone())
                .or_default()
                .push(rec.sentiment_score);
        }
    }

    // 3. Compute sector-level average sentiments
    let mut sector_sentiments: HashMap<String, f64> = HashMap::new();
    for sec in &active_sectors {
        let mean = if let Some(scores) = scores_by_sector.get(sec) {
            if !scores.is_empty() {
                scores.iter().sum::<f64>() / scores.len() as f64
            } else {
                0.0
            }
        } else {
            0.0
        };
        sector_sentiments.insert(sec.clone(), (mean * 10000.0).round() / 10000.0);
    }

    // 4. Compute composite market sentiment score
    let mut weighted_sentiment = 0.0;
    for (sec, &score) in &sector_sentiments {
        let weight = normalized_weights.get(sec).copied().unwrap_or(0.0);
        weighted_sentiment += score * weight;
    }
    let market_sentiment = (weighted_sentiment * 10000.0).round() / 10000.0;

    // 5. Compute market breadth (% of universe with positive mean sentiment)
    let mut bullish_count = 0;
    let mut bearish_count = 0;
    let mut neutral_count = 0;

    for (_ticker, scores) in &scores_by_ticker {
        if scores.is_empty() {
            continue;
        }
        let mean = scores.iter().sum::<f64>() / scores.len() as f64;
        if mean > 0.0001 {
            bullish_count += 1;
        } else if mean < -0.0001 {
            bearish_count += 1;
        } else {
            neutral_count += 1;
        }
    }

    let total_tickers = bullish_count + bearish_count + neutral_count;
    let breadth = if total_tickers > 0 {
        ((bullish_count as f64 / total_tickers as f64) * 10000.0).round() / 10000.0
    } else {
        0.50
    };

    // 6. Compute volatility proxy
    let volatility_proxy =
        ((calculate_sentiment_std_dev(&all_sentiment_scores)) * 10000.0).round() / 10000.0;

    // 7. Compute cross-asset average absolute spillover correlation
    let avg_spillover_corr =
        ((calculate_avg_spillover_correlation(&scores_by_ticker)) * 10000.0).round() / 10000.0;

    // 8. Classify regime and compute confidence
    let regime = classify_market_regime(market_sentiment, breadth, volatility_proxy).to_string();
    let confidence = ((compute_regime_confidence(
        records.len(),
        min_data_points,
        market_sentiment,
        breadth,
        volatility_proxy,
    )) * 10000.0)
        .round()
        / 10000.0;

    info!(
        "[MarketRegime] Computed regime: '{}' (confidence: {:.2}, market_sent: {:.4}, breadth: {:.2}%, vol_proxy: {:.4}, records: {})",
        regime, confidence, market_sentiment, breadth * 100.0, volatility_proxy, records.len()
    );

    let response = MarketRegimeResponse {
        regime,
        confidence,
        market_sentiment,
        breadth,
        avg_spillover_corr,
        volatility_proxy,
        lookback_days,
        generated_at: now.to_rfc3339(),
        components: RegimeComponents {
            sector_sentiments,
            total_data_points: records.len(),
            positive_sentiment_ratio: breadth,
            bullish_tickers_count: bullish_count,
            bearish_tickers_count: bearish_count,
            neutral_tickers_count: neutral_count,
        },
    };

    (StatusCode::OK, Json(response)).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_market_regime_states() {
        // High Volatility takes precedence
        assert_eq!(classify_market_regime(0.30, 0.70, 0.45), "High Volatility");
        assert_eq!(classify_market_regime(-0.30, 0.20, 0.38), "High Volatility");

        // Bullish State
        assert_eq!(classify_market_regime(0.22, 0.65, 0.18), "Bullish");

        // Bearish State
        assert_eq!(classify_market_regime(-0.25, 0.35, 0.20), "Bearish");

        // Neutral / Mixed States
        assert_eq!(classify_market_regime(0.05, 0.52, 0.16), "Neutral");
        assert_eq!(classify_market_regime(0.20, 0.48, 0.18), "Neutral"); // Bullish sentiment but poor breadth
        assert_eq!(classify_market_regime(-0.20, 0.55, 0.18), "Neutral"); // Bearish sentiment but high breadth
    }

    #[test]
    fn test_parse_sector_weights_key_value() {
        let active = vec![
            "Technology".to_string(),
            "Financials".to_string(),
            "Energy".to_string(),
        ];
        let weights = parse_sector_weights(Some("Technology:0.6,Financials:0.4"), &active).unwrap();

        assert!((weights["Technology"] - 0.6 / 2.0).abs() < 1e-4 || weights["Technology"] > 0.2);
        assert!(weights.contains_key("Technology"));
        assert!(weights.contains_key("Financials"));
        assert!(weights.contains_key("Energy"));
    }

    #[test]
    fn test_parse_sector_weights_invalid() {
        let active = vec!["Technology".to_string(), "Financials".to_string()];
        let err = parse_sector_weights(Some("Technology:-0.5,Financials:0.5"), &active);
        assert!(err.is_err());

        let err2 = parse_sector_weights(Some("Technology:abc"), &active);
        assert!(err2.is_err());
    }

    #[test]
    fn test_confidence_and_std_dev() {
        let scores = vec![0.20, 0.25, 0.15, -0.10, 0.40, 0.30];
        let std_dev = calculate_sentiment_std_dev(&scores);
        assert!(std_dev > 0.05);

        let conf = compute_regime_confidence(100, 50, 0.25, 0.65, 0.18);
        assert!(conf >= 0.20 && conf <= 0.99);
    }
}
