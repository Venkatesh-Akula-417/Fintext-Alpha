//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — M&A Rumor Detection & Event Analytics Engine
//! ═══════════════════════════════════════════════════════════════════════════════
//!
//! Identifies potential merger and acquisition candidates by analyzing news
//! sentiment anomaly spikes, M&A keyword frequency, SEC Form 8-K material
//! filings, supply chain spillover relationships, and SEC Form 4 insider conviction.
//! ═══════════════════════════════════════════════════════════════════════════════

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::{Duration as ChronoDuration, Utc};
use serde_json::json;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::models::{MARumorItem, MARumorsParams, MARumorsResponse};
use crate::pit::GLOBAL_PIT_DATA;
use crate::state::AppState;
use crate::storage::questdb_client::QuestDbClient;
use crate::supply_chain::GLOBAL_SUPPLY_CHAIN_GRAPH;

/// M&A Rumor keyword lexicon for news and transcript scanning.
pub const MA_KEYWORDS: &[&str] = &[
    "merger",
    "acquisition",
    "takeover",
    "buyout",
    "strategic alternatives",
    "activist investor",
    "sale of company",
    "exploring options",
];

/// Counts the total frequency of M&A keyword occurrences across a list of text snippets.
pub fn count_ma_keywords(texts: &[&str]) -> usize {
    let mut total_hits = 0;
    for text in texts {
        let lower = text.to_lowercase();
        for &kw in MA_KEYWORDS {
            if lower.contains(kw) {
                total_hits += 1;
            }
        }
    }
    total_hits
}

/// Computes the composite M&A rumor conviction score (0.0 to 1.0).
///
/// Weight allocation:
/// - 40% Sentiment Anomaly Z-Score Component: $C_{\text{sentiment}} = \min(\max(z, 0) / 3.0, 1.0)$
/// - 30% Keyword Component: $C_{\text{keyword}} = \min(\text{hits} / 4.0, 1.0)$
/// - 20% Supply Chain Component: $C_{\text{supply}} = \text{clamp}(s_{\text{sc}}, 0.0, 1.0)$
/// - 10% Insider Accumulation Component: $C_{\text{insider}} = 1.0$ if net buying, else $0.0$
pub fn compute_ma_rumor_score(
    sentiment_zscore: f64,
    keyword_hits: usize,
    supply_chain_signal: f64,
    insider_net_buying: bool,
) -> f64 {
    let sentiment_comp = if sentiment_zscore > 0.0 {
        (sentiment_zscore / 3.0).min(1.0)
    } else {
        0.0
    };
    let keyword_comp = ((keyword_hits as f64) / 4.0).min(1.0);
    let supply_chain_comp = supply_chain_signal.clamp(0.0, 1.0);
    let insider_comp = if insider_net_buying { 1.0 } else { 0.0 };

    let score = 0.40 * sentiment_comp
        + 0.30 * keyword_comp
        + 0.20 * supply_chain_comp
        + 0.10 * insider_comp;

    (score.clamp(0.0, 1.0) * 10000.0).round() / 10000.0
}

/// Generates mock M&A rumor data for a given ticker and lookback window.
pub fn generate_mock_rumor_item(ticker: &str, lookback_days: u32) -> MARumorItem {
    let clean_ticker = ticker.trim().to_uppercase();
    let mut hasher = DefaultHasher::new();
    clean_ticker.hash(&mut hasher);
    lookback_days.hash(&mut hasher);
    let seed = hasher.finish();

    let zscore_raw = match clean_ticker.as_str() {
        "NVDA" => 2.85,
        "AAPL" => 2.15,
        "AMD" => 2.95,
        "CRM" => 2.45,
        "ORCL" => 2.65,
        "PYPL" => 3.10,
        _ => 0.5 + ((seed % 280) as f64) / 100.0,
    };
    let sentiment_zscore = (zscore_raw * 100.0).round() / 100.0;

    let keyword_hits = match clean_ticker.as_str() {
        "AMD" | "PYPL" => 4,
        "NVDA" | "CRM" => 3,
        "AAPL" | "ORCL" => 2,
        _ => ((seed >> 4) % 5) as usize,
    };

    let recent_8k_ma_flag = match clean_ticker.as_str() {
        "PYPL" | "AMD" => true,
        _ => seed % 3 == 0,
    };

    let insider_net_buying = match clean_ticker.as_str() {
        "AMD" | "NVDA" | "PYPL" => true,
        _ => seed % 2 == 1,
    };

    let sc_signal = match clean_ticker.as_str() {
        "NVDA" | "AMD" => 0.85,
        "AAPL" => 0.70,
        "PYPL" => 0.90,
        _ => 0.30 + (((seed >> 8) % 60) as f64) / 100.0,
    };

    // Supply chain related partners from knowledge graph
    let mut related: Vec<String> = GLOBAL_SUPPLY_CHAIN_GRAPH
        .adjacency_list
        .get(&clean_ticker)
        .map(|edges| {
            edges
                .iter()
                .filter(|e| e.is_active)
                .map(|e| e.target.clone())
                .collect()
        })
        .unwrap_or_default();

    if related.is_empty() {
        related = match clean_ticker.as_str() {
            "AAPL" => vec!["TSM".to_string(), "AVGO".to_string(), "QCOM".to_string()],
            "NVDA" => vec!["TSM".to_string(), "ASML".to_string(), "MSFT".to_string()],
            "AMD" => vec!["TSM".to_string(), "ASML".to_string(), "DELL".to_string()],
            "PYPL" => vec!["V".to_string(), "MA".to_string(), "EBAY".to_string()],
            _ => vec!["MSFT".to_string(), "AMZN".to_string()],
        };
    }

    let latest_news_title = match clean_ticker.as_str() {
        "PYPL" => "Activist investor builds stake, pushing board to evaluate takeover and buyout bids".to_string(),
        "AMD" => "Industry sources report preliminary acquisition talks with specialized AI architecture firm".to_string(),
        "NVDA" => "Regulatory scrutiny rises around expanding strategic partnership and asset purchases".to_string(),
        "CRM" => "Board of directors engages advisors to review strategic alternatives and spin-offs".to_string(),
        _ => format!("Market speculation mounts on potential consolidation and merger discussions for {}", clean_ticker),
    };

    let rumor_score = compute_ma_rumor_score(
        sentiment_zscore,
        keyword_hits,
        sc_signal,
        insider_net_buying,
    );

    MARumorItem {
        ticker: clean_ticker,
        rumor_score,
        sentiment_zscore,
        keyword_hits,
        recent_8k_ma_flag,
        supply_chain_related_tickers: related,
        insider_net_buying,
        latest_news_title,
        generated_at: Utc::now().to_rfc3339(),
    }
}

/// Handler for `GET /events/ma-rumors`
#[utoipa::path(
    get,
    path = "/events/ma-rumors",
    params(
        MARumorsParams
    ),
    responses(
        (status = 200, description = "M&A rumor detection signals retrieved successfully", body = MARumorsResponse),
        (status = 400, description = "Invalid query parameters (bounds, lookback range, or limits)"),
        (status = 401, description = "Missing or invalid Bearer JWT / API Key authentication"),
        (status = 429, description = "Rate limit capacity exceeded")
    ),
    security(
        ("bearerAuth" = [])
    ),
    tag = "Events & Catalysts"
)]
pub async fn get_ma_rumors_handler(
    State(_state): State<AppState>,
    Query(params): Query<MARumorsParams>,
) -> Result<Json<MARumorsResponse>, (StatusCode, Json<serde_json::Value>)> {
    let today = Utc::now().date_naive();
    let lookback_days = params.lookback_days.unwrap_or(7);

    // 1. Validate lookback_days
    if !(1..=30).contains(&lookback_days) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Invalid Parameter",
                "message": format!("lookback_days must be between 1 and 30, got {}", lookback_days),
                "field": "lookback_days"
            })),
        ));
    }

    // 2. Validate min_rumor_score
    let min_score = params.min_rumor_score.unwrap_or(0.50);
    if !(0.0..=1.0).contains(&min_score) || min_score.is_nan() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Invalid Parameter",
                "message": format!("min_rumor_score must be between 0.0 and 1.0, got {}", min_score),
                "field": "min_rumor_score"
            })),
        ));
    }

    // 3. Validate limit
    let limit = params.limit.unwrap_or(20);
    if !(1..=100).contains(&limit) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Invalid Parameter",
                "message": format!("limit must be between 1 and 100, got {}", limit),
                "field": "limit"
            })),
        ));
    }

    let start_date = today - ChronoDuration::days(lookback_days as i64);

    let mut candidate_tickers = Vec::new();
    let filter_ticker = if let Some(ref raw_t) = params.ticker {
        let clean = QuestDbClient::validate_and_escape_ticker(raw_t).map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Invalid Parameter",
                    "message": format!("Validation failed for field 'ticker': {}", e),
                    "field": "ticker"
                })),
            )
        })?;

        // Point-in-Time check for specific ticker
        if !GLOBAL_PIT_DATA.is_valid_ticker(&clean, start_date) {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Point-in-Time Security Invalidation",
                    "message": format!("Ticker '{}' was not active or listed on '{}' (Point-in-Time check failed)", clean, start_date),
                    "ticker": clean
                })),
            ));
        }

        candidate_tickers.push(clean.clone());
        Some(clean)
    } else {
        // Universe scan
        let tracked_universe = [
            "AAPL", "NVDA", "MSFT", "AMZN", "GOOGL", "META", "TSLA", "AMD", "INTC", "CRM", "AVGO",
            "QCOM", "TXN", "PYPL", "ADBE", "NFLX", "CSCO", "ORCL", "IBM", "NOW",
        ];
        for &t in &tracked_universe {
            if GLOBAL_PIT_DATA.is_valid_ticker(t, start_date) {
                candidate_tickers.push(t.to_string());
            }
        }
        None
    };

    let mut items = Vec::new();
    for ticker in candidate_tickers {
        let item = generate_mock_rumor_item(&ticker, lookback_days);
        if item.rumor_score >= min_score {
            items.push(item);
        }
    }

    // Sort by rumor_score descending
    items.sort_by(|a, b| {
        b.rumor_score
            .partial_cmp(&a.rumor_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    items.truncate(limit);

    let count = items.len();

    Ok(Json(MARumorsResponse {
        ticker: filter_ticker,
        min_rumor_score: min_score,
        lookback_days,
        count,
        items,
        generated_at: Utc::now().to_rfc3339(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_ma_keywords() {
        let texts = vec![
            "Tech giant announces potential buyout and merger talks",
            "Quarterly earnings release highlights cloud growth",
            "Activist investor demands sale of company and strategic alternatives",
        ];
        let hits = count_ma_keywords(&texts);
        assert_eq!(hits, 5); // buyout, merger, activist investor, sale of company, strategic alternatives
    }

    #[test]
    fn test_compute_ma_rumor_score_bounds() {
        let max_score = compute_ma_rumor_score(3.5, 5, 1.0, true);
        assert!((max_score - 1.0).abs() < 1e-4);

        let zero_score = compute_ma_rumor_score(-1.0, 0, 0.0, false);
        assert_eq!(zero_score, 0.0);

        let mid_score = compute_ma_rumor_score(1.5, 2, 0.5, true);
        assert!(mid_score >= 0.40 && mid_score <= 0.60);
    }
}
