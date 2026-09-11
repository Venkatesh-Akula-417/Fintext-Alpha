//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Market Breadth & Advance/Decline Analytics Handler
//! ═══════════════════════════════════════════════════════════════════════════════
//!
//! Calculates daily market breadth, advance/decline volume & counts, advance/decline
//! ratio, breadth index, and 52-week new highs/lows across tracked stock universes
//! (All, S&P 500, or custom constituent portfolios).
//! ═══════════════════════════════════════════════════════════════════════════════

use axum::extract::Query;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use chrono::{Datelike, Duration, NaiveDate, Utc};
use once_cell::sync::Lazy;
use std::collections::{HashMap, HashSet};
use tracing::info;

use crate::auth::AuthErrorResponse;
use crate::models::{MarketBreadthParams, MarketBreadthPoint, MarketBreadthResponse};
use crate::pit::GLOBAL_PIT_DATA;
use crate::storage::{QuestDbClient, QuestDbClientConfig};

static QUESTDB_CLIENT: Lazy<QuestDbClient> =
    Lazy::new(|| QuestDbClient::new(QuestDbClientConfig::default()));

pub const DEFAULT_BREADTH_LIMIT: usize = 50;
pub const MIN_BREADTH_LIMIT: usize = 1;
pub const MAX_BREADTH_LIMIT: usize = 200;
pub const MAX_CUSTOM_UNIVERSE_TICKERS: usize = 100;
pub const MAX_LOOKBACK_DAYS_LIMIT: i64 = 730; // 2 years maximum

/// Canonical core liquid universe for market breadth aggregation (100 large-cap US equities).
pub const CANONICAL_BREADTH_UNIVERSE: &[&str] = &[
    "AAPL", "MSFT", "NVDA", "AMZN", "GOOGL", "META", "TSLA", "BRK.B", "JPM", "JNJ", "V", "PG",
    "XOM", "MA", "UNH", "HD", "CVX", "MRK", "ABBV", "COST", "PEP", "KO", "ADBE", "WMT", "BAC",
    "TMO", "MCD", "CSCO", "CRM", "ACN", "ABT", "LIN", "NFLX", "AVGO", "ORCL", "AMD", "DIS", "NKE",
    "PM", "TXN", "CAT", "INTC", "QCOM", "VZ", "HON", "COP", "LOW", "IBM", "AMAT", "GE", "GS",
    "SPGI", "INTU", "AXP", "BLK", "NOW", "ISRG", "RTX", "MDLZ", "T", "PFE", "BKNG", "TJX", "LRCX",
    "SYK", "DE", "VRTX", "ADI", "C", "MMC", "ZTS", "LMT", "PANW", "PLD", "CI", "CB", "FI", "BSX",
    "REGN", "SCHW", "ETN", "MO", "MU", "EOG", "BDX", "KLAC", "SO", "DUK", "SNPS", "CDNS", "ITW",
    "CME", "EQIX", "SHW", "WM", "SLB", "CSX", "NOC", "APD", "CL",
];

/// Generates deterministic mock daily close prices for a ticker across a date window.
pub fn generate_deterministic_stock_prices(
    ticker: &str,
    start_date: NaiveDate,
    end_date: NaiveDate,
) -> HashMap<NaiveDate, f64> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    ticker.hash(&mut hasher);
    let seed = hasher.finish();

    // Base price between $20 and $500
    let base_price = 20.0 + ((seed % 480) as f64);
    let drift = (((seed >> 8) % 100) as f64 - 48.0) / 10000.0; // small daily drift
    let vol = 0.015 + (((seed >> 16) % 25) as f64) / 1000.0; // 1.5% - 4.0% daily vol

    let mut current_price = base_price;
    let mut prices = HashMap::new();
    let mut curr = start_date;

    while curr <= end_date {
        // Skip weekends
        let weekday = curr.weekday();
        if weekday != chrono::Weekday::Sat && weekday != chrono::Weekday::Sun {
            let day_num = curr.num_days_from_ce() as u64;
            let pseudo_rand =
                ((seed.wrapping_mul(6364136223846793005).wrapping_add(day_num)) >> 24) % 1000;
            let shock = ((pseudo_rand as f64 - 495.0) / 500.0) * vol;
            current_price = (current_price * (1.0 + drift + shock)).max(1.0);
            prices.insert(curr, (current_price * 100.0).round() / 100.0);
        }
        curr = match curr.succ_opt() {
            Some(d) => d,
            None => break,
        };
    }

    prices
}

/// Calculate daily market breadth statistics from a map of ticker prices.
pub fn calculate_market_breadth_series(
    ticker_prices: &HashMap<String, HashMap<NaiveDate, f64>>,
    start_date: NaiveDate,
    end_date: NaiveDate,
    limit: usize,
    include_new_highs_lows: bool,
) -> Vec<MarketBreadthPoint> {
    // Collect and sort all distinct trading dates present in the data within the requested window
    let mut all_dates_set: HashSet<NaiveDate> = HashSet::new();
    for prices in ticker_prices.values() {
        for &d in prices.keys() {
            if d >= start_date && d <= end_date {
                all_dates_set.insert(d);
            }
        }
    }

    let mut sorted_dates: Vec<NaiveDate> = all_dates_set.into_iter().collect();
    sorted_dates.sort();

    // Limit to the most recent `limit` dates or the first `limit` dates
    let eval_dates: Vec<NaiveDate> = if sorted_dates.len() > limit {
        sorted_dates[sorted_dates.len() - limit..].to_vec()
    } else {
        sorted_dates
    };

    let mut points = Vec::with_capacity(eval_dates.len());

    for &curr_date in &eval_dates {
        let mut advancers = 0usize;
        let mut decliners = 0usize;
        let mut unchanged = 0usize;
        let mut new_52w_highs_count = 0usize;
        let mut new_52w_lows_count = 0usize;

        for prices in ticker_prices.values() {
            if let Some(&curr_p) = prices.get(&curr_date) {
                // Find previous trading day's price
                let prev_dates: Vec<NaiveDate> =
                    prices.keys().filter(|&&d| d < curr_date).copied().collect();

                if let Some(&prev_date) = prev_dates.iter().max() {
                    if let Some(&prev_p) = prices.get(&prev_date) {
                        let diff = curr_p - prev_p;
                        if diff > 0.0001 {
                            advancers += 1;
                        } else if diff < -0.0001 {
                            decliners += 1;
                        } else {
                            unchanged += 1;
                        }
                    }
                }

                if include_new_highs_lows {
                    // Check rolling 252-day high/low (52 weeks)
                    let lookback_start = curr_date - Duration::days(365);
                    let mut is_high = true;
                    let mut is_low = true;
                    let mut history_count = 0;

                    for (&d, &p) in prices {
                        if d >= lookback_start && d < curr_date {
                            history_count += 1;
                            if p >= curr_p {
                                is_high = false;
                            }
                            if p <= curr_p {
                                is_low = false;
                            }
                        }
                    }

                    if history_count >= 5 {
                        if is_high {
                            new_52w_highs_count += 1;
                        }
                        if is_low {
                            new_52w_lows_count += 1;
                        }
                    }
                }
            }
        }

        let total_active = advancers + decliners + unchanged;
        let ad_sum = advancers + decliners;
        let advance_decline_ratio = if ad_sum > 0 {
            ((advancers as f64 / ad_sum as f64) * 1000.0).round() / 1000.0
        } else {
            0.5
        };

        let breadth_index = if total_active > 0 {
            let raw_idx = (advancers as f64 - decliners as f64) / (total_active as f64);
            ((raw_idx.clamp(-1.0, 1.0)) * 1000.0).round() / 1000.0
        } else {
            0.0
        };

        let (new_52w_highs, new_52w_lows) = if include_new_highs_lows {
            (Some(new_52w_highs_count), Some(new_52w_lows_count))
        } else {
            (None, None)
        };

        points.push(MarketBreadthPoint {
            date: curr_date.format("%Y-%m-%d").to_string(),
            advancers,
            decliners,
            unchanged,
            advance_decline_ratio,
            breadth_index,
            new_52w_highs,
            new_52w_lows,
        });
    }

    points
}

/// Query Market Breadth & Advance/Decline Series.
///
/// Computes daily advance/decline counts, advance/decline ratio, breadth index (-1.0 to 1.0),
/// and optional 52-week new highs/lows across tracked stock universes or custom portfolios.
#[utoipa::path(
    get,
    path = "/market/breadth",
    tag = "Market Intelligence",
    params(
        ("start_date" = String, Query, description = "Start date for historical breadth window (YYYY-MM-DD)"),
        ("end_date" = String, Query, description = "End date for historical breadth window (YYYY-MM-DD)"),
        ("universe" = Option<String>, Query, description = "Filter universe: 'all' (default), 'sp500', or comma-separated list of tickers (max 100)"),
        ("limit" = Option<usize>, Query, description = "Maximum number of observation days to return (1 to 200, default: 50)"),
        ("include_new_highs_lows" = Option<bool>, Query, description = "Whether to calculate and include new 52-week highs and lows (default: true)")
    ),
    responses(
        (status = 200, description = "Daily market breadth time series calculated successfully", body = MarketBreadthResponse),
        (status = 400, description = "Invalid date format, inverted date range, or invalid universe parameter", body = AuthErrorResponse),
        (status = 401, description = "Missing or invalid Bearer authentication token", body = AuthErrorResponse)
    ),
    security(
        ("BearerAuth" = [])
    )
)]
pub async fn get_market_breadth_handler(Query(params): Query<MarketBreadthParams>) -> Response {
    // 1. Validate start_date
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

    // 2. Validate end_date
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
                "start_date ({}) cannot be after end_date ({})",
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

    // 3. Validate limit
    let limit = params.limit.unwrap_or(DEFAULT_BREADTH_LIMIT);
    if limit < MIN_BREADTH_LIMIT || limit > MAX_BREADTH_LIMIT {
        let err = AuthErrorResponse {
            error: "Bad Request".to_string(),
            message: format!(
                "limit must be between {} and {} (received: {})",
                MIN_BREADTH_LIMIT, MAX_BREADTH_LIMIT, limit
            ),
        };
        return (StatusCode::BAD_REQUEST, Json(err)).into_response();
    }

    let include_new_highs_lows = params.include_new_highs_lows.unwrap_or(true);

    // 4. Resolve Universe
    let raw_universe = params.universe.as_deref().unwrap_or("all").trim();
    let pit_data = GLOBAL_PIT_DATA.clone();

    let (universe_label, tickers): (String, Vec<String>) =
        if raw_universe.eq_ignore_ascii_case("all") {
            let mut list: Vec<String> = CANONICAL_BREADTH_UNIVERSE
                .iter()
                .map(|&s| s.to_string())
                .collect();
            if pit_data.is_enabled() {
                list = pit_data.filter_universe_by_date(&list, start_date);
            }
            ("all".to_string(), list)
        } else if raw_universe.eq_ignore_ascii_case("sp500") {
            let mut list: Vec<String> = CANONICAL_BREADTH_UNIVERSE
                .iter()
                .filter(|&&s| pit_data.is_in_sp500(s, start_date))
                .map(|&s| s.to_string())
                .collect();
            if list.is_empty() {
                list = CANONICAL_BREADTH_UNIVERSE
                    .iter()
                    .take(50)
                    .map(|&s| s.to_string())
                    .collect();
            }
            ("sp500".to_string(), list)
        } else {
            // Custom comma-separated list
            let raw_tokens: Vec<&str> = raw_universe
                .split(',')
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .collect();
            if raw_tokens.is_empty() {
                let err = AuthErrorResponse {
                    error: "Bad Request".to_string(),
                    message: "Custom universe ticker list cannot be empty".to_string(),
                };
                return (StatusCode::BAD_REQUEST, Json(err)).into_response();
            }
            if raw_tokens.len() > MAX_CUSTOM_UNIVERSE_TICKERS {
                let err = AuthErrorResponse {
                    error: "Bad Request".to_string(),
                    message: format!(
                        "Custom universe ticker count ({}) exceeds maximum limit of {}",
                        raw_tokens.len(),
                        MAX_CUSTOM_UNIVERSE_TICKERS
                    ),
                };
                return (StatusCode::BAD_REQUEST, Json(err)).into_response();
            }

            let mut unique_tickers = Vec::new();
            let mut seen = HashSet::new();
            for t in raw_tokens {
                let upper = t.to_uppercase();
                if !upper
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
                {
                    let err = AuthErrorResponse {
                        error: "Bad Request".to_string(),
                        message: format!("Invalid ticker symbol in custom universe: '{}'", t),
                    };
                    return (StatusCode::BAD_REQUEST, Json(err)).into_response();
                }
                if seen.insert(upper.clone()) {
                    unique_tickers.push(upper);
                }
            }
            (raw_universe.to_string(), unique_tickers)
        };

    let is_mock = crate::state::is_questdb_mock_fallback_enabled()
        || crate::state::is_polygon_mock_fallback_enabled();

    // Extend lookback start for previous day & 52w highs calculation
    let fetch_start_date = if include_new_highs_lows {
        start_date - Duration::days(375)
    } else {
        start_date - Duration::days(14)
    };
    let fetch_start_str = fetch_start_date.format("%Y-%m-%d").to_string();
    let end_str = end_date.format("%Y-%m-%d").to_string();

    // 5. Ingest / Query Stock Prices
    let ticker_prices: HashMap<String, HashMap<NaiveDate, f64>> = if is_mock {
        let mut prices = HashMap::new();
        for t in &tickers {
            prices.insert(
                t.clone(),
                generate_deterministic_stock_prices(t, fetch_start_date, end_date),
            );
        }
        prices
    } else {
        match QUESTDB_CLIENT
            .query_stock_prices(&tickers, &fetch_start_str, &end_str)
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
                            generate_deterministic_stock_prices(t, fetch_start_date, end_date),
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
                info!(
                    "QuestDB stock prices fallback to deterministic simulation: {}",
                    e
                );
                let mut prices = HashMap::new();
                for t in &tickers {
                    prices.insert(
                        t.clone(),
                        generate_deterministic_stock_prices(t, fetch_start_date, end_date),
                    );
                }
                prices
            }
        }
    };

    // 6. Calculate Market Breadth Time Series Points
    let points = calculate_market_breadth_series(
        &ticker_prices,
        start_date,
        end_date,
        limit,
        include_new_highs_lows,
    );

    let count = points.len();
    let total_tickers = tickers.len();

    (
        StatusCode::OK,
        Json(MarketBreadthResponse {
            start_date: params.start_date,
            end_date: params.end_date,
            universe: universe_label,
            total_tickers,
            points,
            count,
            generated_at: Utc::now().to_rfc3339(),
        }),
    )
        .into_response()
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn test_market_breadth_calculation_basic() {
        let mut ticker_prices = HashMap::new();

        let mut aapl = HashMap::new();
        aapl.insert(NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(), 100.0);
        aapl.insert(NaiveDate::from_ymd_opt(2025, 1, 2).unwrap(), 105.0); // + (advancer)
        aapl.insert(NaiveDate::from_ymd_opt(2025, 1, 3).unwrap(), 104.0); // - (decliner)
        ticker_prices.insert("AAPL".to_string(), aapl);

        let mut msft = HashMap::new();
        msft.insert(NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(), 200.0);
        msft.insert(NaiveDate::from_ymd_opt(2025, 1, 2).unwrap(), 202.0); // + (advancer)
        msft.insert(NaiveDate::from_ymd_opt(2025, 1, 3).unwrap(), 206.0); // + (advancer)
        ticker_prices.insert("MSFT".to_string(), msft);

        let mut nvda = HashMap::new();
        nvda.insert(NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(), 300.0);
        nvda.insert(NaiveDate::from_ymd_opt(2025, 1, 2).unwrap(), 290.0); // - (decliner)
        nvda.insert(NaiveDate::from_ymd_opt(2025, 1, 3).unwrap(), 290.0); // = (unchanged)
        ticker_prices.insert("NVDA".to_string(), nvda);

        let start = NaiveDate::from_ymd_opt(2025, 1, 2).unwrap();
        let end = NaiveDate::from_ymd_opt(2025, 1, 3).unwrap();

        let points = calculate_market_breadth_series(&ticker_prices, start, end, 10, false);
        assert_eq!(points.len(), 2);

        // Day 1 (2025-01-02): AAPL (+), MSFT (+), NVDA (-) -> 2 adv, 1 dec, 0 unch
        assert_eq!(points[0].date, "2025-01-02");
        assert_eq!(points[0].advancers, 2);
        assert_eq!(points[0].decliners, 1);
        assert_eq!(points[0].unchanged, 0);
        assert!((points[0].advance_decline_ratio - (2.0 / 3.0)).abs() < 0.01);
        assert!((points[0].breadth_index - (1.0 / 3.0)).abs() < 0.01);
        assert_eq!(points[0].new_52w_highs, None);
        assert_eq!(points[0].new_52w_lows, None);

        // Day 2 (2025-01-03): AAPL (-), MSFT (+), NVDA (=) -> 1 adv, 1 dec, 1 unch
        assert_eq!(points[1].date, "2025-01-03");
        assert_eq!(points[1].advancers, 1);
        assert_eq!(points[1].decliners, 1);
        assert_eq!(points[1].unchanged, 1);
        assert!((points[1].advance_decline_ratio - 0.5).abs() < 0.01);
        assert!((points[1].breadth_index - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_deterministic_prices_generation() {
        let start = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
        let end = NaiveDate::from_ymd_opt(2025, 1, 31).unwrap();
        let prices = generate_deterministic_stock_prices("AAPL", start, end);
        assert!(!prices.is_empty());
        for (&d, &p) in &prices {
            assert!(d >= start && d <= end);
            assert!(p > 0.0);
        }
    }
}
