//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Quantitative Data Quality Scoring Engine
//! ═══════════════════════════════════════════════════════════════════════════════

use once_cell::sync::Lazy;
use std::collections::HashMap;

/// Source reliability mapping (0.0 to 1.0).
/// Includes active production sources (SEC EDGAR, Finnhub, Polygon) and historical
/// benchmark weights (Tiingo, Alpha Vantage, NewsAPI, RSS) retained for PIT replay and scoring.
pub static SOURCE_RELIABILITY: Lazy<HashMap<&'static str, f32>> = Lazy::new(|| {
    let mut m = HashMap::new();
    m.insert("SEC EDGAR", 0.95);
    m.insert("SEC_EDGAR", 0.95);
    m.insert("BLOOMBERG", 0.90);
    m.insert("REUTERS", 0.90);
    m.insert("POLYGON", 0.90);
    m.insert("POLYGON.IO", 0.90);
    m.insert("INSTITUTIONAL WIRE", 0.85);
    m.insert("WIRE", 0.85);
    m.insert("FINNHUB", 0.80);
    m.insert("TIINGO", 0.80);
    m.insert("ALPHAVANTAGE", 0.75);
    m.insert("ALPHA VANTAGE", 0.75);
    m.insert("MOCK", 0.70);
    m.insert("NEWSAPI", 0.60);
    m.insert("RSS", 0.50);
    m.insert("RSS FEEDS", 0.50);
    m
});

/// Default source reliability when unlisted
pub const DEFAULT_SOURCE_RELIABILITY: f32 = 0.60;

/// Spam keywords/phrases to penalize low-quality promotional or clickbait content
const SPAM_KEYWORDS: &[&str] = &[
    "sponsored",
    "advertisement",
    "click here",
    "promoted",
    "buy now",
    "free trial",
    "subscribe now",
    "giveaway",
    "100% free",
    "guaranteed returns",
    "casino",
    "crypto pump",
    "risk-free",
];

/// Calculate the text length factor based on headline / title character count.
pub fn calculate_text_length_factor(title: &str) -> f32 {
    let len = title.trim().chars().count();
    if len == 0 {
        0.1
    } else if len < 15 {
        0.3
    } else if len < 50 {
        0.7
    } else {
        1.0
    }
}

/// Detect promotional spam content in title and return a penalty fraction (0.0 or 0.3).
pub fn calculate_spam_penalty(title: &str) -> f32 {
    let title_lower = title.to_lowercase();
    for &keyword in SPAM_KEYWORDS {
        if title_lower.contains(keyword) {
            return 0.3;
        }
    }
    0.0
}

/// Retrieve the reliability score for a given data source.
pub fn get_source_reliability(source: &str) -> f32 {
    let s_clean = source.trim().to_uppercase();
    SOURCE_RELIABILITY
        .get(s_clean.as_str())
        .copied()
        .unwrap_or(DEFAULT_SOURCE_RELIABILITY)
}

/// Compute a comprehensive data quality score (0.0 to 1.0) for a financial sentiment record.
///
/// Formula:
/// `quality = 0.4 * source_reliability + 0.2 * text_length_factor + 0.2 * (1.0 - spam_penalty) + 0.2 * confidence`
pub fn compute_data_quality_score(source: &str, title: &str, confidence: f32) -> f32 {
    let source_rel = get_source_reliability(source);
    let len_factor = calculate_text_length_factor(title);
    let spam_penalty = calculate_spam_penalty(title);
    let spam_factor = (1.0 - spam_penalty).clamp(0.0, 1.0);
    let conf_factor = confidence.clamp(0.0, 1.0);

    let raw_quality =
        (0.4 * source_rel) + (0.2 * len_factor) + (0.2 * spam_factor) + (0.2 * conf_factor);
    let clamped = raw_quality.clamp(0.0, 1.0);
    (clamped * 10000.0).round() / 10000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sec_edgar_high_quality_record() {
        let score = compute_data_quality_score(
            "SEC EDGAR",
            "Apple Inc. Files Form 10-K Annual Report for Fiscal Year 2025 with Comprehensive Disclosures",
            0.90,
        );
        // 0.4 * 0.95 (0.38) + 0.2 * 1.0 (0.20) + 0.2 * 1.0 (0.20) + 0.2 * 0.90 (0.18) = 0.96
        assert!((score - 0.96).abs() < 0.001);
    }

    #[test]
    fn test_spam_penalized_record() {
        let score = compute_data_quality_score(
            "RSS",
            "Click here for sponsored trading tips and guaranteed returns",
            0.50,
        );
        // source: 0.50 -> 0.4*0.5 = 0.20
        // len: 61 >= 50 -> 1.0 -> 0.2*1.0 = 0.20
        // spam: contains 'click here' & 'sponsored' -> penalty 0.3 -> factor 0.7 -> 0.2*0.7 = 0.14
        // conf: 0.50 -> 0.2*0.5 = 0.10
        // total: 0.20 + 0.20 + 0.14 + 0.10 = 0.64
        assert!((score - 0.64).abs() < 0.001);
    }

    #[test]
    fn test_short_headline_penalty() {
        let score = compute_data_quality_score("Finnhub", "AAPL up", 0.70);
        // source: 0.80 -> 0.4*0.8 = 0.32
        // len: 7 < 15 -> 0.3 -> 0.2*0.3 = 0.06
        // spam: clean -> 1.0 -> 0.2*1.0 = 0.20
        // conf: 0.70 -> 0.2*0.7 = 0.14
        // total: 0.32 + 0.06 + 0.20 + 0.14 = 0.72
        assert!((score - 0.72).abs() < 0.001);
    }

    #[test]
    fn test_unknown_source_fallback() {
        let score = compute_data_quality_score(
            "UnknownFeed",
            "Moderate length financial headline analysis",
            0.80,
        );
        // source: 0.60 -> 0.24
        // len: 43 -> 0.7 -> 0.14
        // spam: 1.0 -> 0.20
        // conf: 0.80 -> 0.16
        // total: 0.74
        assert!((score - 0.74).abs() < 0.001);
    }
}
