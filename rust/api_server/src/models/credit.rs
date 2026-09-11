//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Credit Default Sentiment Models
//! ═══════════════════════════════════════════════════════════════════════════════

use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

fn default_lookback_days() -> Option<u32> {
    Some(30)
}

/// Query parameters for GET /risk/credit-sentiment.
#[derive(Debug, Clone, Serialize, Deserialize, IntoParams, ToSchema)]
pub struct CreditSentimentParams {
    /// Target equity ticker symbol (e.g. "AAPL", "MSFT", "NVDA").
    #[param(example = "AAPL")]
    pub ticker: String,
    /// Lookback observation window in calendar days (default: 30, min: 1, max: 90).
    #[serde(default = "default_lookback_days")]
    #[param(example = 30)]
    pub lookback_days: Option<u32>,
}

/// Complete response payload for the Credit Default Sentiment endpoint.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct CreditSentimentResponse {
    /// Target stock ticker symbol.
    #[schema(example = "AAPL")]
    pub ticker: String,
    /// Effective lookback observation window in calendar days.
    #[schema(example = 30)]
    pub lookback_days: u32,
    /// Composite credit sentiment score (-1.0 to +1.0; positive indicates healthy credit quality, negative indicates distress).
    #[schema(example = -0.15)]
    pub credit_sentiment_score: f64,
    /// Average sentiment score of credit-risk-related news and disclosures (-1.0 to +1.0).
    #[schema(example = -0.05)]
    pub news_sentiment_avg: f64,
    /// Number of SEC Form 8-K distress disclosure filings (e.g. Items 1.03, 2.04, 3.01).
    #[schema(example = 1)]
    pub eight_k_distress_count: usize,
    /// Current options put/call volume ratio (PCR).
    #[schema(example = 0.80)]
    pub put_call_ratio: f64,
    /// At-the-money (ATM) options 30-day implied volatility (IV).
    #[schema(example = 0.35)]
    pub implied_volatility: f64,
    /// ISO-8601 UTC timestamp of credit sentiment calculation.
    #[schema(example = "2025-08-31T12:00:00Z")]
    pub generated_at: String,
}
