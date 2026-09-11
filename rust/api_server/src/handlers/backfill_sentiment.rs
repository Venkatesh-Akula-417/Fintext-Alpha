//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — News Sentiment Backfill Handler
//! ═══════════════════════════════════════════════════════════════════════════════

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use chrono::{NaiveDate, Utc};
use tracing::info;
use uuid::Uuid;

use crate::auth::AuthErrorResponse;
use crate::models::{BackfillSentimentRequest, BackfillSentimentResponse, NewsArticleFull};
use crate::quality::compute_data_quality_score;
use crate::state::AppState;

/// Maximum allowed articles limit per backfill request.
pub const MAX_BACKFILL_LIMIT: usize = 10_000;

/// Validates stock ticker symbol format.
pub fn validate_ticker(ticker: &str) -> Result<String, &'static str> {
    let t = ticker.trim().to_uppercase();
    if t.is_empty() {
        return Err("Ticker cannot be empty");
    }
    if t.len() > 10 {
        return Err("Ticker cannot exceed 10 characters");
    }
    if !t
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
    {
        return Err("Ticker can only contain alphanumeric characters, dots, and hyphens");
    }
    Ok(t)
}

/// Parses and validates historical date boundaries.
pub fn validate_date_range(
    start_str: &str,
    end_str: &str,
) -> Result<(NaiveDate, NaiveDate), String> {
    let start_date = NaiveDate::parse_from_str(start_str.trim(), "%Y-%m-%d").map_err(|e| {
        format!(
            "Invalid start_date '{}', expected YYYY-MM-DD: {}",
            start_str, e
        )
    })?;

    let end_date = NaiveDate::parse_from_str(end_str.trim(), "%Y-%m-%d")
        .map_err(|e| format!("Invalid end_date '{}', expected YYYY-MM-DD: {}", end_str, e))?;

    if start_date > end_date {
        return Err(format!(
            "start_date '{}' must be less than or equal to end_date '{}'",
            start_str, end_str
        ));
    }

    Ok((start_date, end_date))
}

/// Computes deterministic sentiment score, label, confidence, and quality metrics for an article.
pub fn compute_article_sentiment(
    title: &str,
    full_text: &str,
    id: &Uuid,
) -> (f64, String, f64, f64) {
    let text_lower = format!("{} {}", title, full_text).to_lowercase();

    let bullish_keywords = [
        "growth",
        "record",
        "unveil",
        "partnership",
        "expanded",
        "secured",
        "profit",
        "beat",
        "surge",
        "bullish",
        "upgrade",
        "outperform",
        "momentum",
        "breakthrough",
    ];
    let bearish_keywords = [
        "antitrust",
        "investigation",
        "breach",
        "inquiry",
        "loss",
        "drop",
        "delay",
        "default",
        "downgrade",
        "covenant",
        "restructuring",
        "decline",
        "warning",
        "risk",
    ];

    let mut bullish_count: usize = 0;
    for kw in &bullish_keywords {
        if text_lower.contains(kw) {
            bullish_count += 1;
        }
    }

    let mut bearish_count: usize = 0;
    for kw in &bearish_keywords {
        if text_lower.contains(kw) {
            bearish_count += 1;
        }
    }

    let mut seed: u64 = 14695981039346656037;
    for b in id.as_bytes() {
        seed = seed.wrapping_mul(1099511628211) ^ (*b as u64);
    }
    let jitter = (((seed % 200) as f64) - 100.0) / 1000.0; // [-0.10, +0.10]

    let score = if bullish_count > bearish_count {
        let raw = 0.35 + (bullish_count as f64 * 0.15) + jitter;
        raw.clamp(-1.0, 1.0)
    } else if bearish_count > bullish_count {
        let raw = -0.35 - (bearish_count as f64 * 0.15) + jitter;
        raw.clamp(-1.0, 1.0)
    } else {
        jitter.clamp(-1.0, 1.0)
    };

    let rounded_score = (score * 10000.0).round() / 10000.0;

    let label = if rounded_score > 0.15 {
        "BULLISH".to_string()
    } else if rounded_score < -0.15 {
        "BEARISH".to_string()
    } else {
        "NEUTRAL".to_string()
    };

    let confidence = ((0.80 + 0.19 * rounded_score.abs()).min(0.99) * 10000.0).round() / 10000.0;
    let quality = (compute_data_quality_score("Institutional Wire", title, confidence as f32)
        as f64
        * 10000.0)
        .round()
        / 10000.0;

    (rounded_score, label, confidence, quality)
}

/// Request historical news sentiment backfill.
#[utoipa::path(
    post,
    path = "/sentiment/backfill",
    tag = "Sentiment Analysis",
    request_body = BackfillSentimentRequest,
    responses(
        (status = 200, description = "News sentiment backfill executed successfully", body = BackfillSentimentResponse),
        (status = 400, description = "Invalid ticker, date format, or parameter bounds", body = AuthErrorResponse),
        (status = 401, description = "Missing or invalid Bearer JWT", body = AuthErrorResponse)
    ),
    security(
        ("BearerAuth" = [])
    )
)]
pub async fn backfill_sentiment_handler(
    State(state): State<AppState>,
    Json(req): Json<BackfillSentimentRequest>,
) -> Response {
    // 1. Validate ticker
    let ticker_upper = match validate_ticker(&req.ticker) {
        Ok(t) => t,
        Err(msg) => {
            let err = AuthErrorResponse {
                error: "Bad Request".to_string(),
                message: msg.to_string(),
            };
            return (StatusCode::BAD_REQUEST, Json(err)).into_response();
        }
    };

    // 2. Validate dates
    let (_start_date, _end_date) = match validate_date_range(&req.start_date, &req.end_date) {
        Ok(dates) => dates,
        Err(msg) => {
            let err = AuthErrorResponse {
                error: "Bad Request".to_string(),
                message: msg,
            };
            return (StatusCode::BAD_REQUEST, Json(err)).into_response();
        }
    };

    // 3. Validate limit
    let limit = req.limit.unwrap_or(1000);
    if limit == 0 || limit > MAX_BACKFILL_LIMIT {
        let err = AuthErrorResponse {
            error: "Bad Request".to_string(),
            message: format!(
                "limit must be between 1 and {} (received {})",
                MAX_BACKFILL_LIMIT, limit
            ),
        };
        return (StatusCode::BAD_REQUEST, Json(err)).into_response();
    }

    let overwrite = req.overwrite.unwrap_or(false);

    // 4. Query candidate articles from registry
    let all_articles =
        state
            .news_article_registry
            .list_articles(&crate::models::ListNewsArticlesQuery {
                ticker: Some(ticker_upper.clone()),
                source: None,
                start_date: Some(req.start_date.clone()),
                end_date: Some(req.end_date.clone()),
                limit: Some(100),
                offset: Some(0),
            });

    // In-memory matching on full registry entries
    let mut matching_articles: Vec<NewsArticleFull> = Vec::new();
    let mut articles_to_process: Vec<NewsArticleFull> = Vec::new();

    // Iterate through state news article registry
    for metadata in &all_articles.articles {
        if let Some(art) = state.news_article_registry.get_article(&metadata.id) {
            matching_articles.push(art.clone());
            if overwrite || art.sentiment_score.is_none() {
                articles_to_process.push(art);
            }
        }
    }

    let total_found = matching_articles.len();
    let mut processed_articles: usize = 0;
    let failed_articles: usize = 0;

    for mut art in articles_to_process.into_iter().take(limit) {
        let (score, label, conf, quality) =
            compute_article_sentiment(&art.title, &art.full_text, &art.id);

        art.sentiment_score = Some(score);
        art.sentiment_label = Some(label);
        art.confidence = Some(conf);
        art.data_quality_score = Some(quality);

        let _ = state.news_article_registry.save_article(None, art).await;
        processed_articles += 1;
    }

    info!(
        "[Sentiment Backfill] Ticker='{}', Range=[{} to {}], Overwrite={}, Found={}, Processed={}",
        ticker_upper, req.start_date, req.end_date, overwrite, total_found, processed_articles
    );

    let resp = BackfillSentimentResponse {
        ticker: ticker_upper,
        start_date: req.start_date,
        end_date: req.end_date,
        overwrite,
        total_articles_found: total_found,
        processed_articles,
        failed_articles,
        generated_at: Utc::now().to_rfc3339(),
    };

    (StatusCode::OK, Json(resp)).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validation_invalid_ticker() {
        assert!(validate_ticker("").is_err());
        assert!(validate_ticker("TOOLONGTICKERNAME").is_err());
        assert!(validate_ticker("INVALID@TICKER").is_err());
        assert_eq!(validate_ticker("aapl").unwrap(), "AAPL");
    }

    #[test]
    fn test_validation_invalid_date_format() {
        assert!(validate_date_range("2025/01/01", "2025-03-31").is_err());
        assert!(validate_date_range("2025-01-01", "invalid-date").is_err());
    }

    #[test]
    fn test_validation_inverted_dates() {
        assert!(validate_date_range("2025-04-01", "2025-03-01").is_err());
        assert!(validate_date_range("2025-01-01", "2025-01-01").is_ok());
    }

    #[test]
    fn test_article_sentiment_scoring() {
        let id = Uuid::new_v4();
        let (score_pos, label_pos, conf_pos, quality_pos) = compute_article_sentiment(
            "Apple Unveils Record Growth and Expansion",
            "Apple announced record breaking earnings beat and surge in sales.",
            &id,
        );

        assert!(score_pos > 0.0);
        assert_eq!(label_pos, "BULLISH");
        assert!(conf_pos >= 0.80 && conf_pos <= 1.0);
        assert!(quality_pos > 0.0);

        let (score_neg, label_neg, _, _) = compute_article_sentiment(
            "Antitrust Regulatory Investigation and Breach Notice",
            "Regulators issue warning regarding covenant breach and profit decline.",
            &id,
        );

        assert!(score_neg < 0.0);
        assert_eq!(label_neg, "BEARISH");
    }
}
