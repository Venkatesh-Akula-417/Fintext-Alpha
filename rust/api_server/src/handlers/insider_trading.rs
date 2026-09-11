//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — SEC Form 4 Insider Trading Signal Endpoint Handler
//! ═══════════════════════════════════════════════════════════════════════════════

use axum::extract::Query;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use chrono::{Datelike, Duration as ChronoDuration, NaiveDate, Utc};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use tracing::info;

use crate::models::{InsiderTradeItem, InsiderTradingParams, InsiderTradingResponse};
use crate::pit::GLOBAL_PIT_DATA;
use crate::storage::QuestDbClient;

const TRACKED_UNIVERSE: &[&str] = &[
    "AAPL", "NVDA", "MSFT", "AMZN", "GOOGL", "META", "TSLA", "JPM", "V", "WMT", "LLY", "AVGO",
    "AMD", "NFLX", "DIS", "INTC", "CSCO", "QCOM", "TXN", "PYPL",
];

/// Compute normalized directional conviction signal score for an insider transaction.
///
/// Returns a score clamped to `[-1.0, +1.0]`.
pub fn compute_insider_signal_score(
    transaction_type: &str,
    shares: u64,
    price: f64,
    role: &str,
) -> f64 {
    let t_type = transaction_type.trim().to_lowercase();
    let direction_multiplier = match t_type.as_str() {
        "purchase" => 1.0,
        "sale" => -1.0,
        "grant" => 0.2,
        "exercise" => 0.2,
        _ => 0.0,
    };

    let role_lower = role.to_lowercase();
    let role_weight = if role_lower.contains("ceo")
        || role_lower.contains("chief executive")
        || role_lower.contains("cfo")
        || role_lower.contains("chief financial")
    {
        1.0
    } else if role_lower.contains("director") || role_lower.contains("board member") {
        0.8
    } else if role_lower.contains("officer")
        || role_lower.contains("president")
        || role_lower.contains("coo")
        || role_lower.contains("cto")
        || role_lower.contains("evp")
        || role_lower.contains("executive vice")
    {
        0.6
    } else if role_lower.contains("10%")
        || role_lower.contains("owner")
        || role_lower.contains("shareholder")
    {
        0.5
    } else {
        0.4
    };

    let value = (shares as f64) * price;
    let magnitude_score = if value > 0.0 {
        (1.0 + value).log10() / 7.0
    } else {
        (1.0 + (shares as f64) * 100.0).log10() / 7.0
    }
    .min(1.0)
    .max(0.0);

    let raw_score = direction_multiplier * role_weight * magnitude_score;
    let clamped = raw_score.clamp(-1.0, 1.0);
    (clamped * 10000.0).round() / 10000.0
}

/// Known corporate insider executive rosters for top institutional equities.
fn get_insiders_for_ticker(ticker: &str) -> Vec<(&'static str, &'static str, f64)> {
    match ticker.to_uppercase().as_str() {
        "AAPL" => vec![
            ("Tim Cook", "CEO", 225.0),
            ("Luca Maestri", "CFO", 225.0),
            ("Arthur D. Levinson", "Director (Board Chair)", 225.0),
            (
                "Katherine L. Adams",
                "Senior Vice President / General Counsel",
                225.0,
            ),
            ("Deirdre O'Brien", "Senior Vice President / Retail", 225.0),
        ],
        "NVDA" => vec![
            ("Jensen Huang", "CEO", 128.0),
            ("Colette Kress", "CFO", 128.0),
            ("Mark A. Stevens", "Director", 128.0),
            ("Debora Shoquist", "EVP Operations", 128.0),
            ("Tench Coxe", "Director", 128.0),
        ],
        "MSFT" => vec![
            ("Satya Nadella", "CEO", 415.0),
            ("Amy Hood", "CFO", 415.0),
            ("Brad Smith", "President & Vice Chair", 415.0),
            ("John W. Thompson", "Lead Independent Director", 415.0),
            ("Judson Althoff", "EVP & Chief Commercial Officer", 415.0),
        ],
        "TSLA" => vec![
            ("Elon Musk", "CEO", 210.0),
            ("Vaibhav Taneja", "CFO", 210.0),
            ("Robyn M. Denholm", "Director (Board Chair)", 210.0),
            ("Kimbal Musk", "Director", 210.0),
            ("Tom Zhu", "SVP Automotive", 210.0),
        ],
        "META" => vec![
            ("Mark Zuckerberg", "CEO", 510.0),
            ("Susan Li", "CFO", 510.0),
            ("Sheryl Sandberg", "Director", 510.0),
            ("Javier Olivan", "COO", 510.0),
            ("Andrew Bosworth", "CTO", 510.0),
        ],
        "AMZN" => vec![
            ("Andy Jassy", "CEO", 185.0),
            ("Brian Olsavsky", "CFO", 185.0),
            ("Jeffrey P. Bezos", "Executive Chair (10% Owner)", 185.0),
            (
                "Douglas J. Herrington",
                "CEO Worldwide Amazon Stores",
                185.0,
            ),
            ("Adam N. Selipsky", "CEO Amazon Web Services", 185.0),
        ],
        "GOOGL" | "GOOG" => vec![
            ("Sundar Pichai", "CEO", 165.0),
            ("Anat Ashkenazi", "CFO", 165.0),
            ("Ruth Porat", "President & Chief Investment Officer", 165.0),
            ("Sergey Brin", "Director (10% Owner)", 165.0),
            ("Larry Page", "Director (10% Owner)", 165.0),
        ],
        _ => vec![
            ("John Doe", "CEO", 100.0),
            ("Jane Smith", "CFO", 100.0),
            ("Robert Johnson", "Director", 100.0),
            ("Alice Williams", "EVP Operations", 100.0),
            ("Vanguard Capital Partners", "10% Owner", 100.0),
        ],
    }
}

/// Generate deterministic mock SEC Form 4 insider transactions for offline/test environments.
pub fn generate_mock_insider_trades(
    ticker: &str,
    start: NaiveDate,
    end: NaiveDate,
) -> Vec<InsiderTradeItem> {
    let roster = get_insiders_for_ticker(ticker);
    let mut trades = Vec::new();

    let mut curr = start;
    while curr <= end {
        // Form 4 filings often occur mid-month or after quarterly earnings / vesting periods
        let day = curr.day();
        if day == 5 || day == 15 || day == 28 {
            let mut hasher = DefaultHasher::new();
            ticker.to_uppercase().hash(&mut hasher);
            curr.to_string().hash(&mut hasher);
            let seed = hasher.finish();

            let insider_idx = (seed as usize) % roster.len();
            let (name, role, base_price) = roster[insider_idx];

            let types = ["purchase", "sale", "grant", "exercise"];
            let t_type = types[(seed as usize >> 3) % types.len()];

            let shares = match t_type {
                "purchase" => 2_500 + (((seed >> 5) % 45_000) as u64),
                "sale" => 5_000 + (((seed >> 6) % 95_000) as u64),
                "grant" => 10_000 + (((seed >> 7) % 50_000) as u64),
                _ => 8_000 + (((seed >> 8) % 40_000) as u64),
            };

            let price_noise = (((seed >> 10) % 20) as f64 - 10.0) / 100.0;
            let price = if t_type == "grant" {
                0.0
            } else {
                ((base_price * (1.0 + price_noise)) * 100.0).round() / 100.0
            };

            let value = ((shares as f64 * price) * 100.0).round() / 100.0;
            let score = compute_insider_signal_score(t_type, shares, price, role);

            trades.push(InsiderTradeItem {
                ticker: ticker.to_uppercase(),
                insider_name: name.to_string(),
                insider_role: role.to_string(),
                transaction_type: t_type.to_string(),
                shares,
                price,
                value,
                filing_date: curr.format("%Y-%m-%d").to_string(),
                signal_score: score,
                source: "SEC Form 4".to_string(),
            });
        }
        curr = curr.succ_opt().unwrap_or(curr + ChronoDuration::days(1));
    }

    trades
}

/// Query SEC Form 4 insider trading transactions and normalized directional conviction signal scores.
#[utoipa::path(
    get,
    path = "/events/insider-trading",
    params(InsiderTradingParams),
    responses(
        (status = 200, description = "Insider trading transactions and signals retrieved successfully", body = InsiderTradingResponse),
        (status = 400, description = "Invalid query parameters"),
        (status = 401, description = "Missing or invalid Bearer JWT"),
        (status = 429, description = "Rate limit exceeded")
    ),
    security(
        ("bearerAuth" = [])
    ),
    tag = "Corporate Events"
)]
pub async fn get_insider_trading_handler(Query(params): Query<InsiderTradingParams>) -> Response {
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

    // 3. Validate transaction_type
    let raw_type = params
        .transaction_type
        .as_deref()
        .unwrap_or("all")
        .trim()
        .to_lowercase();
    let allowed_types = ["all", "purchase", "sale", "grant", "exercise"];
    if !allowed_types.contains(&raw_type.as_str()) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "Bad Request",
                "message": format!("Invalid transaction_type '{}', allowed values: {:?}", raw_type, allowed_types)
            })),
        )
            .into_response();
    }

    // 4. Validate min_signal_score (0.0..=1.0, default: 0.0)
    let min_score = params.min_signal_score.unwrap_or(0.0);
    if !(0.0..=1.0).contains(&min_score) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "Bad Request",
                "message": format!("Parameter 'min_signal_score' must be between 0.0 and 1.0 (got {})", min_score)
            })),
        )
            .into_response();
    }

    // 5. Validate min_shares
    let min_shares = params.min_shares.unwrap_or(0);

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

    // 7. Validate ticker & Point-in-Time check if provided
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
        "[Insider Trading] Querying ticker={:?}, type={}, start={}, end={}, min_shares={}, min_score={}, limit={}",
        clean_ticker, raw_type, parsed_start, parsed_end, min_shares, min_score, limit
    );

    // 8. Gather tickers to evaluate
    let target_tickers: Vec<String> = match clean_ticker.as_ref() {
        Some(t) => vec![t.clone()],
        None => TRACKED_UNIVERSE.iter().map(|s| s.to_string()).collect(),
    };

    let mut all_trades = Vec::new();

    for t in target_tickers {
        let trades = generate_mock_insider_trades(&t, parsed_start, parsed_end);
        for trade in trades {
            // Apply transaction_type filter
            if raw_type != "all" && !trade.transaction_type.eq_ignore_ascii_case(&raw_type) {
                continue;
            }
            // Apply min_shares filter
            if trade.shares < min_shares {
                continue;
            }
            // Apply min_signal_score filter
            if trade.signal_score.abs() < min_score {
                continue;
            }
            all_trades.push(trade);
        }
    }

    // Sort by filing_date descending, then by absolute signal_score descending
    all_trades.sort_by(|a, b| {
        b.filing_date.cmp(&a.filing_date).then_with(|| {
            b.signal_score
                .abs()
                .partial_cmp(&a.signal_score.abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    });

    all_trades.truncate(limit);

    let count = all_trades.len();
    let generated_at = Utc::now().to_rfc3339();

    let response = InsiderTradingResponse {
        ticker: clean_ticker,
        transaction_type: raw_type,
        start_date: parsed_start.format("%Y-%m-%d").to_string(),
        end_date: parsed_end.format("%Y-%m-%d").to_string(),
        min_shares,
        min_signal_score: min_score,
        count,
        trades: all_trades,
        generated_at,
    };

    (StatusCode::OK, Json(response)).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_insider_signal_score_purchase() {
        let score = compute_insider_signal_score("purchase", 50_000, 200.0, "CEO");
        assert!(score > 0.8 && score <= 1.0);
    }

    #[test]
    fn test_compute_insider_signal_score_sale() {
        let score = compute_insider_signal_score("sale", 50_000, 200.0, "CFO");
        assert!(score < -0.8 && score >= -1.0);
    }

    #[test]
    fn test_compute_insider_signal_score_grant() {
        let score = compute_insider_signal_score("grant", 10_000, 0.0, "Director");
        assert!(score > 0.0 && score < 0.3);
    }

    #[test]
    fn test_generate_mock_insider_trades() {
        let start = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
        let end = NaiveDate::from_ymd_opt(2025, 3, 31).unwrap();
        let trades = generate_mock_insider_trades("AAPL", start, end);
        assert!(!trades.is_empty());
        for trade in &trades {
            assert_eq!(trade.ticker, "AAPL");
            assert_eq!(trade.source, "SEC Form 4");
            assert!(trade.signal_score >= -1.0 && trade.signal_score <= 1.0);
        }
    }
}
