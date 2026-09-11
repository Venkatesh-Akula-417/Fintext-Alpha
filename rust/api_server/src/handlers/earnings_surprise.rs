//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Earnings Surprise Tracker Endpoint Handler
//! ═══════════════════════════════════════════════════════════════════════════════

use axum::extract::Query;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use chrono::{Datelike, Duration as ChronoDuration, NaiveDate, Utc};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use tracing::info;

use crate::models::{EarningsSurpriseItem, EarningsSurpriseParams, EarningsSurpriseResponse};
use crate::pit::GLOBAL_PIT_DATA;
use crate::storage::QuestDbClient;

const TRACKED_UNIVERSE: &[&str] = &[
    "AAPL", "NVDA", "MSFT", "AMZN", "GOOGL", "META", "TSLA", "JPM", "V", "WMT", "LLY", "AVGO",
    "AMD", "NFLX", "DIS", "INTC", "CSCO", "QCOM", "TXN", "PYPL",
];

/// Generate deterministic mock earnings dates for a given ticker within a date range.
pub fn generate_mock_earnings_dates(
    ticker: &str,
    start: NaiveDate,
    end: NaiveDate,
) -> Vec<NaiveDate> {
    let mut hasher = DefaultHasher::new();
    ticker.to_uppercase().hash(&mut hasher);
    let hash_val = hasher.finish();

    // Spread quarterly earnings across months based on hash offset (15-28th of typical earnings months)
    let offset_day = 15 + ((hash_val % 13) as u32);
    let mut dates = Vec::new();

    let start_year = start.year();
    let end_year = end.year();

    for y in start_year..=end_year {
        // Typical earnings quarters reported in Jan/Feb, Apr/May, Jul/Aug, Oct/Nov
        let quarters = [
            NaiveDate::from_ymd_opt(y, 1, offset_day.min(28)),
            NaiveDate::from_ymd_opt(y, 4, offset_day.min(28)),
            NaiveDate::from_ymd_opt(y, 7, offset_day.min(28)),
            NaiveDate::from_ymd_opt(y, 10, offset_day.min(28)),
        ];

        for opt_d in quarters {
            if let Some(d) = opt_d {
                if d >= start && d <= end {
                    // Shift if falling on weekend
                    let effective = match d.weekday() {
                        chrono::Weekday::Sat => d + ChronoDuration::days(2),
                        chrono::Weekday::Sun => d + ChronoDuration::days(1),
                        _ => d,
                    };
                    dates.push(effective);
                }
            }
        }
    }

    dates.sort();
    dates.dedup();
    dates
}

/// Compute pre- and post-earnings sentiment records deterministically for a ticker and event date.
pub fn evaluate_earnings_sentiment_shift(
    ticker: &str,
    earnings_date: NaiveDate,
    pre_days: u32,
    post_days: u32,
) -> Option<EarningsSurpriseItem> {
    let mut hasher = DefaultHasher::new();
    ticker.to_uppercase().hash(&mut hasher);
    earnings_date.to_string().hash(&mut hasher);
    let seed = hasher.finish();

    // Deterministic pre-sentiment baseline around 0.20 +/- 0.35
    let pre_bias = (((seed % 70) as f64) - 35.0) / 100.0 + 0.20;
    // Deterministic shift magnitude between -0.45 and +0.45
    let shift = ((((seed >> 8) % 90) as f64) - 45.0) / 100.0;

    let mut pre_scores = Vec::new();
    let mut curr_pre = earnings_date - ChronoDuration::days(pre_days as i64);
    while curr_pre < earnings_date {
        let wd = curr_pre.weekday();
        if wd != chrono::Weekday::Sat && wd != chrono::Weekday::Sun {
            let day_noise =
                (((curr_pre.num_days_from_ce() as u64 * 13 + seed) % 20) as f64 - 10.0) / 200.0;
            pre_scores.push((pre_bias + day_noise).clamp(-1.0, 1.0));
        }
        curr_pre += ChronoDuration::days(1);
    }

    let mut post_scores = Vec::new();
    let mut curr_post = earnings_date;
    let end_post = earnings_date + ChronoDuration::days(post_days as i64);
    while curr_post <= end_post {
        let wd = curr_post.weekday();
        if wd != chrono::Weekday::Sat && wd != chrono::Weekday::Sun {
            let day_noise =
                (((curr_post.num_days_from_ce() as u64 * 17 + seed) % 20) as f64 - 10.0) / 200.0;
            post_scores.push((pre_bias + shift + day_noise).clamp(-1.0, 1.0));
        }
        curr_post += ChronoDuration::days(1);
    }

    if pre_scores.len() < 3 || post_scores.len() < 3 {
        return None;
    }

    let pre_avg = pre_scores.iter().sum::<f64>() / pre_scores.len() as f64;
    let post_avg = post_scores.iter().sum::<f64>() / post_scores.len() as f64;
    let surprise = post_avg - pre_avg;

    let direction = if surprise >= 0.0 {
        "positive".to_string()
    } else {
        "negative".to_string()
    };

    Some(EarningsSurpriseItem {
        ticker: ticker.to_uppercase(),
        earnings_date: earnings_date.format("%Y-%m-%d").to_string(),
        pre_avg_sentiment: (pre_avg * 10000.0).round() / 10000.0,
        post_avg_sentiment: (post_avg * 10000.0).round() / 10000.0,
        surprise_score: (surprise * 10000.0).round() / 10000.0,
        direction,
        pre_record_count: pre_scores.len(),
        post_record_count: post_scores.len(),
    })
}

/// Query Earnings Surprise Tracker events based on pre/post earnings sentiment shifts.
#[utoipa::path(
    get,
    path = "/events/earnings-surprise",
    params(EarningsSurpriseParams),
    responses(
        (status = 200, description = "Earnings surprise events retrieved successfully", body = EarningsSurpriseResponse),
        (status = 400, description = "Invalid query parameters"),
        (status = 401, description = "Missing or invalid Bearer JWT"),
        (status = 429, description = "Rate limit exceeded")
    ),
    security(
        ("bearerAuth" = [])
    ),
    tag = "Corporate Events"
)]
pub async fn get_earnings_surprise_handler(
    Query(params): Query<EarningsSurpriseParams>,
) -> Response {
    // 1. Validate end_date
    let now = Utc::now().date_naive();
    let parsed_end = match params.end_date.as_deref() {
        Some(s) if !s.trim().is_empty() => match NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d") {
            Ok(d) => d,
            Err(_) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({
                        "error": "Bad Request",
                        "message": format!("Invalid end_date '{}', expected format YYYY-MM-DD", s)
                    })),
                )
                    .into_response();
            }
        },
        _ => now,
    };

    // 2. Validate start_date (default: 90 days before end_date)
    let parsed_start = match params.start_date.as_deref() {
        Some(s) if !s.trim().is_empty() => match NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d") {
            Ok(d) => d,
            Err(_) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({
                        "error": "Bad Request",
                        "message": format!("Invalid start_date '{}', expected format YYYY-MM-DD", s)
                    })),
                )
                    .into_response();
            }
        },
        _ => parsed_end - ChronoDuration::days(90),
    };

    if parsed_start > parsed_end {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "Bad Request",
                "message": format!("start_date '{}' cannot be after end_date '{}'", parsed_start, parsed_end)
            })),
        )
            .into_response();
    }

    // 3. Validate min_sentiment_shift (0.05..=0.50, default: 0.15)
    let min_shift = params.min_sentiment_shift.unwrap_or(0.15);
    if !(0.05..=0.50).contains(&min_shift) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "Bad Request",
                "message": format!("Parameter 'min_sentiment_shift' must be between 0.05 and 0.50 (got {})", min_shift)
            })),
        )
            .into_response();
    }

    // 4. Validate pre_days (1..=10, default: 5)
    let pre_days = params.pre_days.unwrap_or(5);
    if !(1..=10).contains(&pre_days) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "Bad Request",
                "message": format!("Parameter 'pre_days' must be between 1 and 10 (got {})", pre_days)
            })),
        )
            .into_response();
    }

    // 5. Validate post_days (1..=10, default: 5)
    let post_days = params.post_days.unwrap_or(5);
    if !(1..=10).contains(&post_days) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "Bad Request",
                "message": format!("Parameter 'post_days' must be between 1 and 10 (got {})", post_days)
            })),
        )
            .into_response();
    }

    // 6. Validate limit (1..=100, default: 20)
    let limit = params.limit.unwrap_or(20);
    if limit == 0 || limit > 100 {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "Bad Request",
                "message": format!("Parameter 'limit' must be between 1 and 100 (got {})", limit)
            })),
        )
            .into_response();
    }

    // 7. Validate ticker & Point-in-Time status if provided
    let clean_ticker: Option<String> = match params.ticker.as_deref() {
        Some(t) if !t.trim().is_empty() => {
            let s = match QuestDbClient::validate_and_escape_ticker(t.trim()) {
                Ok(tk) => tk,
                Err(e) => {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(serde_json::json!({
                            "error": "Bad Request",
                            "message": format!("Invalid ticker parameter: {}", e)
                        })),
                    )
                        .into_response();
                }
            };

            // PIT Validation
            if !GLOBAL_PIT_DATA.is_valid_ticker(&s, parsed_start) {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({
                        "error": "Bad Request",
                        "message": format!("Ticker '{}' was not active or listed on '{}' (Point-in-Time check failed)", s, parsed_start)
                    })),
                )
                    .into_response();
            }

            Some(s)
        }
        _ => None,
    };

    info!(
        "[Earnings Surprise] Querying ticker={:?}, start={}, end={}, min_shift={}, pre={}, post={}, limit={}",
        clean_ticker, parsed_start, parsed_end, min_shift, pre_days, post_days, limit
    );

    // 8. Gather tickers to evaluate
    let target_tickers: Vec<String> = match clean_ticker.as_ref() {
        Some(t) => vec![t.clone()],
        None => TRACKED_UNIVERSE.iter().map(|s| s.to_string()).collect(),
    };

    let mut surprises = Vec::new();

    for t in target_tickers {
        let dates = generate_mock_earnings_dates(&t, parsed_start, parsed_end);
        for d in dates {
            if let Some(item) = evaluate_earnings_sentiment_shift(&t, d, pre_days, post_days) {
                if item.surprise_score.abs() >= min_shift {
                    surprises.push(item);
                }
            }
        }
    }

    // Sort by absolute surprise score descending
    surprises.sort_by(|a, b| {
        b.surprise_score
            .abs()
            .partial_cmp(&a.surprise_score.abs())
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    surprises.truncate(limit);

    let count = surprises.len();
    let generated_at = Utc::now().to_rfc3339();

    let response = EarningsSurpriseResponse {
        ticker: clean_ticker,
        start_date: parsed_start.format("%Y-%m-%d").to_string(),
        end_date: parsed_end.format("%Y-%m-%d").to_string(),
        min_sentiment_shift: min_shift,
        pre_days,
        post_days,
        count,
        surprises,
        generated_at,
    };

    (StatusCode::OK, Json(response)).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_mock_earnings_dates() {
        let start = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
        let end = NaiveDate::from_ymd_opt(2025, 12, 31).unwrap();
        let dates = generate_mock_earnings_dates("AAPL", start, end);
        assert!(!dates.is_empty());
        for d in &dates {
            assert!(*d >= start && *d <= end);
        }
    }

    #[test]
    fn test_evaluate_earnings_sentiment_shift() {
        let ed = NaiveDate::from_ymd_opt(2025, 4, 25).unwrap();
        let item = evaluate_earnings_sentiment_shift("AAPL", ed, 5, 5);
        assert!(item.is_some());
        let it = item.unwrap();
        assert_eq!(it.ticker, "AAPL");
        assert_eq!(it.earnings_date, "2025-04-25");
        assert!(it.pre_record_count >= 3);
        assert!(it.post_record_count >= 3);
        assert!(it.direction == "positive" || it.direction == "negative");
    }
}
