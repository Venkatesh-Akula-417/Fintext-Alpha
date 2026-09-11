//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Unusual Options Activity (UOA) Detection Engine
//! ═══════════════════════════════════════════════════════════════════════════════
//!
//! Identifies options contracts with abnormally high trading volume relative to
//! open interest — a quantitative signal for informed trading, event anticipation,
//! or institutional positioning. Computes a composite score from volume/OI ratio
//! and volume z-score across a configurable lookback window.
//! ═══════════════════════════════════════════════════════════════════════════════

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::Utc;
use serde_json::json;

use crate::models::{UnusualOptionItem, UnusualOptionsParams, UnusualOptionsResponse};
use crate::state::AppState;
use crate::storage::questdb_client::QuestDbClient;

// ═══════════════════════════════════════════════════════════════════════════════
// Mock Data Generation
// ═══════════════════════════════════════════════════════════════════════════════

/// Seed data for deterministic mock unusual options activity.
/// Each entry: (underlying, expiration, strike, opt_type, volume, open_interest, avg_volume, stddev)
const MOCK_UOA_DATA: &[(&str, &str, f64, &str, u64, u64, f64, f64)] = &[
    // High UOA: massive volume vs tiny OI, high z-score
    (
        "AAPL",
        "2025-12-19",
        250.0,
        "CALL",
        48500,
        2200,
        3200.0,
        1800.0,
    ),
    (
        "AAPL",
        "2025-12-19",
        230.0,
        "PUT",
        32000,
        4500,
        4800.0,
        2100.0,
    ),
    (
        "NVDA",
        "2025-12-19",
        140.0,
        "CALL",
        95000,
        8000,
        12000.0,
        5500.0,
    ),
    (
        "NVDA",
        "2025-11-21",
        120.0,
        "PUT",
        28000,
        6500,
        5000.0,
        2800.0,
    ),
    (
        "TSLA",
        "2025-12-19",
        280.0,
        "CALL",
        72000,
        5500,
        9500.0,
        4200.0,
    ),
    (
        "TSLA",
        "2025-11-21",
        250.0,
        "PUT",
        41000,
        3800,
        6200.0,
        3100.0,
    ),
    (
        "META",
        "2025-12-19",
        550.0,
        "CALL",
        18500,
        1200,
        2800.0,
        1500.0,
    ),
    (
        "AMZN",
        "2025-12-19",
        200.0,
        "CALL",
        55000,
        4200,
        7800.0,
        3600.0,
    ),
    (
        "GOOGL",
        "2025-12-19",
        180.0,
        "PUT",
        22000,
        3100,
        4500.0,
        2200.0,
    ),
    (
        "SPY",
        "2025-12-19",
        580.0,
        "CALL",
        125000,
        18000,
        25000.0,
        12000.0,
    ),
    // Moderate UOA
    (
        "MSFT",
        "2025-12-19",
        450.0,
        "CALL",
        8500,
        3200,
        3800.0,
        1900.0,
    ),
    (
        "AMD",
        "2025-12-19",
        170.0,
        "CALL",
        15000,
        5800,
        6200.0,
        2800.0,
    ),
    // Below threshold: volume too low
    ("INTC", "2025-12-19", 28.0, "CALL", 50, 1200, 80.0, 30.0),
    // Below threshold: ratio too low
    (
        "JPM",
        "2025-12-19",
        220.0,
        "PUT",
        3500,
        35000,
        3200.0,
        800.0,
    ),
];

/// Compute UOA metrics for a single contract and return an `UnusualOptionItem` if
/// it passes the minimum volume and ratio filters.
fn compute_uoa_item(
    underlying: &str,
    expiration: &str,
    strike: f64,
    opt_type: &str,
    volume: u64,
    open_interest: u64,
    avg_volume: f64,
    stddev: f64,
    min_volume: u64,
    min_vol_oi_ratio: f64,
    timestamp: &str,
) -> Option<UnusualOptionItem> {
    // Filter by minimum absolute volume
    if volume < min_volume {
        return None;
    }

    // Compute volume / open_interest ratio (guard against zero OI)
    let oi_denom = if open_interest == 0 { 1 } else { open_interest };
    let volume_oi_ratio = volume as f64 / oi_denom as f64;

    // Filter by minimum volume/OI ratio
    if volume_oi_ratio < min_vol_oi_ratio {
        return None;
    }

    // Compute z-score: (volume - avg_volume) / max(stddev, 1.0)
    let safe_stddev = if stddev < 1.0 { 1.0 } else { stddev };
    let volume_zscore = (volume as f64 - avg_volume) / safe_stddev;

    // Composite score = volume_oi_ratio × max(volume_zscore, 0.1)
    // Floor z-score at 0.1 to ensure ratio-dominant contracts still score
    let score = volume_oi_ratio * volume_zscore.max(0.1);

    // Build option ticker in OCC format: O:<UNDERLYING>YYMMDD<C/P>00<STRIKE*1000>
    let opt_char = if opt_type.eq_ignore_ascii_case("CALL") {
        "C"
    } else {
        "P"
    };
    let exp_clean = expiration.replace('-', "");
    let exp_compact = if exp_clean.len() >= 8 {
        &exp_clean[2..]
    } else {
        &exp_clean
    };
    let ticker = format!(
        "O:{}{}{}{:08}",
        underlying,
        exp_compact,
        opt_char,
        (strike * 1000.0) as u64
    );

    Some(UnusualOptionItem {
        ticker,
        underlying_ticker: underlying.to_string(),
        expiration_date: expiration.to_string(),
        strike,
        option_type: opt_type.to_string(),
        volume,
        open_interest,
        avg_volume,
        volume_oi_ratio: (volume_oi_ratio * 10000.0).round() / 10000.0, // 4 decimal places
        volume_zscore: (volume_zscore * 10000.0).round() / 10000.0,
        score: (score * 10000.0).round() / 10000.0,
        timestamp: timestamp.to_string(),
    })
}

/// Generate deterministic mock unusual options data for offline testing.
fn generate_mock_unusual_options(
    ticker_filter: Option<&str>,
    min_vol_oi_ratio: f64,
    min_volume: u64,
    limit: u32,
) -> Vec<UnusualOptionItem> {
    let timestamp = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

    let mut items: Vec<UnusualOptionItem> = MOCK_UOA_DATA
        .iter()
        .filter(|(underlying, ..)| {
            ticker_filter
                .map(|t| t.eq_ignore_ascii_case(underlying))
                .unwrap_or(true)
        })
        .filter_map(
            |&(underlying, exp, strike, opt_type, vol, oi, avg_vol, stddev)| {
                compute_uoa_item(
                    underlying,
                    exp,
                    strike,
                    opt_type,
                    vol,
                    oi,
                    avg_vol,
                    stddev,
                    min_volume,
                    min_vol_oi_ratio,
                    &timestamp,
                )
            },
        )
        .collect();

    // Sort by composite score descending
    items.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Truncate to limit
    items.truncate(limit as usize);

    items
}

// ═══════════════════════════════════════════════════════════════════════════════
// Axum Handler
// ═══════════════════════════════════════════════════════════════════════════════

/// Scan options universe for unusual activity — contracts with abnormally high
/// trading volume relative to open interest.
///
/// Returns a ranked list of UOA candidates sorted by composite score
/// (volume/OI ratio × volume z-score), enabling alpha generation from
/// informed flow detection.
#[utoipa::path(
    get,
    path = "/options/unusual",
    params(UnusualOptionsParams),
    responses(
        (status = 200, description = "Unusual options activity scan results", body = UnusualOptionsResponse),
        (status = 400, description = "Invalid query parameters"),
        (status = 401, description = "Missing or invalid Bearer JWT"),
        (status = 429, description = "Rate limit exceeded")
    ),
    security(
        ("bearerAuth" = [])
    ),
    tag = "Options & Derivatives"
)]
pub async fn get_unusual_options_handler(
    State(_state): State<AppState>,
    Query(params): Query<UnusualOptionsParams>,
) -> Result<Json<UnusualOptionsResponse>, (StatusCode, Json<serde_json::Value>)> {
    // 1. Validate ticker if provided
    let ticker_filter = if let Some(ref t) = params.ticker {
        let safe = QuestDbClient::validate_and_escape_ticker(t).map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Invalid Parameter",
                    "message": format!("Validation failed for field 'ticker': {}", e),
                    "field": "ticker"
                })),
            )
        })?;
        Some(safe)
    } else {
        None
    };

    // 2. Validate min_volume_oi_ratio
    let min_vol_oi_ratio = params.min_volume_oi_ratio.unwrap_or(2.0);
    if min_vol_oi_ratio < 0.0 || min_vol_oi_ratio.is_nan() || min_vol_oi_ratio.is_infinite() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Invalid Parameter",
                "message": format!("min_volume_oi_ratio must be non-negative, got {}", min_vol_oi_ratio),
                "field": "min_volume_oi_ratio"
            })),
        ));
    }

    // 3. Validate days (1..=7)
    let days = params.days.unwrap_or(1);
    if days == 0 || days > 7 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Invalid Parameter",
                "message": format!("days must be between 1 and 7, got {}", days),
                "field": "days"
            })),
        ));
    }

    // 4. Validate limit (1..=100)
    let limit = params.limit.unwrap_or(20);
    if limit == 0 || limit > 100 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Invalid Parameter",
                "message": format!("limit must be between 1 and 100, got {}", limit),
                "field": "limit"
            })),
        ));
    }

    // 5. Validate min_volume
    let min_volume = params.min_volume.unwrap_or(100);

    // 6. Generate / query unusual options data
    let items = generate_mock_unusual_options(
        ticker_filter.as_deref(),
        min_vol_oi_ratio,
        min_volume,
        limit,
    );

    let display_ticker = ticker_filter.unwrap_or_else(|| "ALL".to_string());
    let count = items.len();

    Ok(Json(UnusualOptionsResponse {
        ticker: display_ticker,
        min_volume_oi_ratio: min_vol_oi_ratio,
        min_volume,
        days,
        count,
        items,
        message: format!(
            "Unusual options activity scan completed: {} contract(s) flagged",
            count
        ),
    }))
}

// ═══════════════════════════════════════════════════════════════════════════════
// Unit Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_uoa_item_basic() {
        let item = compute_uoa_item(
            "AAPL",
            "2025-12-19",
            250.0,
            "CALL",
            48500,
            2200,
            3200.0,
            1800.0,
            100,
            2.0,
            "2025-08-29T12:00:00Z",
        );
        assert!(item.is_some());
        let item = item.unwrap();
        assert_eq!(item.underlying_ticker, "AAPL");
        assert_eq!(item.strike, 250.0);
        assert_eq!(item.option_type, "CALL");
        assert_eq!(item.volume, 48500);
        assert_eq!(item.open_interest, 2200);

        // volume_oi_ratio = 48500 / 2200 = 22.0454...
        assert!(item.volume_oi_ratio > 22.0);
        assert!(item.volume_oi_ratio < 22.1);

        // z-score = (48500 - 3200) / 1800 = 25.1666...
        assert!(item.volume_zscore > 25.0);
        assert!(item.volume_zscore < 26.0);

        // score = ratio * z-score > 0
        assert!(item.score > 0.0);
    }

    #[test]
    fn test_compute_uoa_item_filters_low_volume() {
        let item = compute_uoa_item(
            "INTC",
            "2025-12-19",
            28.0,
            "CALL",
            50,
            1200,
            80.0,
            30.0,
            100,
            2.0,
            "2025-08-29T12:00:00Z",
        );
        assert!(item.is_none(), "Should filter out volume < min_volume");
    }

    #[test]
    fn test_compute_uoa_item_filters_low_ratio() {
        let item = compute_uoa_item(
            "JPM",
            "2025-12-19",
            220.0,
            "PUT",
            3500,
            35000,
            3200.0,
            800.0,
            100,
            2.0,
            "2025-08-29T12:00:00Z",
        );
        assert!(item.is_none(), "Should filter out ratio < min_vol_oi_ratio");
    }

    #[test]
    fn test_compute_uoa_item_zero_oi() {
        let item = compute_uoa_item(
            "TEST",
            "2025-12-19",
            100.0,
            "CALL",
            5000,
            0,
            1000.0,
            500.0,
            100,
            2.0,
            "2025-08-29T12:00:00Z",
        );
        assert!(item.is_some());
        let item = item.unwrap();
        // With OI=0, ratio = 5000/1 = 5000
        assert_eq!(item.volume_oi_ratio, 5000.0);
    }

    #[test]
    fn test_compute_uoa_item_zero_stddev() {
        let item = compute_uoa_item(
            "TEST",
            "2025-12-19",
            100.0,
            "CALL",
            5000,
            500,
            1000.0,
            0.0,
            100,
            2.0,
            "2025-08-29T12:00:00Z",
        );
        assert!(item.is_some());
        let item = item.unwrap();
        // With stddev=0, safe_stddev=1.0, z-score = (5000-1000)/1 = 4000
        assert_eq!(item.volume_zscore, 4000.0);
    }

    #[test]
    fn test_mock_generation_all_tickers() {
        let items = generate_mock_unusual_options(None, 2.0, 100, 50);
        assert!(!items.is_empty());
        // Should be sorted by score descending
        for pair in items.windows(2) {
            assert!(
                pair[0].score >= pair[1].score,
                "Items must be sorted by score descending"
            );
        }
    }

    #[test]
    fn test_mock_generation_ticker_filter() {
        let items = generate_mock_unusual_options(Some("AAPL"), 2.0, 100, 50);
        assert!(!items.is_empty());
        for item in &items {
            assert_eq!(item.underlying_ticker, "AAPL");
        }
    }

    #[test]
    fn test_mock_generation_limit() {
        let items = generate_mock_unusual_options(None, 1.0, 50, 3);
        assert!(items.len() <= 3);
    }

    #[test]
    fn test_mock_generation_high_threshold_filters_all() {
        let items = generate_mock_unusual_options(None, 1000.0, 100, 50);
        // Most contracts won't have ratio >= 1000
        // Only zero-OI contracts could match
        for item in &items {
            assert!(item.volume_oi_ratio >= 1000.0);
        }
    }

    #[test]
    fn test_mock_generation_nonexistent_ticker() {
        let items = generate_mock_unusual_options(Some("ZZZZ"), 2.0, 100, 50);
        assert!(items.is_empty());
    }

    #[test]
    fn test_composite_score_ordering() {
        // Verify that higher volume/OI ratio and higher z-score produce higher composite scores
        let high_item = compute_uoa_item(
            "A",
            "2025-12-19",
            100.0,
            "CALL",
            100000,
            1000,
            5000.0,
            2000.0,
            100,
            0.0,
            "2025-08-29T12:00:00Z",
        )
        .unwrap();

        let low_item = compute_uoa_item(
            "B",
            "2025-12-19",
            100.0,
            "CALL",
            5000,
            2000,
            4500.0,
            2000.0,
            100,
            0.0,
            "2025-08-29T12:00:00Z",
        )
        .unwrap();

        assert!(
            high_item.score > low_item.score,
            "Higher volume/OI and z-score should produce higher composite score"
        );
    }
}
