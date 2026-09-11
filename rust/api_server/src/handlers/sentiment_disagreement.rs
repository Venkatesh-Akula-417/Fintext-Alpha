//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Sentiment Disagreement Index Endpoint Handler
//! ═══════════════════════════════════════════════════════════════════════════════

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use chrono::{Duration as ChronoDuration, NaiveDate, Utc};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use tracing::info;

use crate::models::{SentimentDisagreementParams, SentimentDisagreementResponse, SourceBreakdown};
use crate::pit::GLOBAL_PIT_DATA;
use crate::state::AppState;
use crate::storage::QuestDbClient;

/// Compute arithmetic mean of a slice of values.
pub fn compute_mean(data: &[f64]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }
    let sum: f64 = data.iter().sum();
    sum / (data.len() as f64)
}

/// Compute median of a slice of values.
pub fn compute_median(data: &[f64]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }
    let mut sorted = data.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = sorted.len();
    if n % 2 == 1 {
        sorted[n / 2]
    } else {
        (sorted[n / 2 - 1] + sorted[n / 2]) / 2.0
    }
}

/// Compute sample standard deviation of a slice of values.
pub fn compute_stddev(data: &[f64], mean: f64) -> f64 {
    if data.len() < 2 {
        return 0.0;
    }
    let variance: f64 = data
        .iter()
        .map(|x| {
            let diff = x - mean;
            diff * diff
        })
        .sum::<f64>()
        / ((data.len() - 1) as f64);
    variance.sqrt()
}

/// Compute percentile using linear interpolation.
pub fn compute_percentile(sorted_data: &[f64], p: f64) -> f64 {
    if sorted_data.is_empty() {
        return 0.0;
    }
    if sorted_data.len() == 1 {
        return sorted_data[0];
    }
    let p_clamped = p.clamp(0.0, 1.0);
    let index = p_clamped * ((sorted_data.len() - 1) as f64);
    let lower = index.floor() as usize;
    let upper = index.ceil() as usize;
    let weight = index - (lower as f64);

    if lower == upper {
        sorted_data[lower]
    } else {
        sorted_data[lower] * (1.0 - weight) + sorted_data[upper] * weight
    }
}

/// Compute Interquartile Range (IQR = Q3 - Q1).
pub fn compute_iqr(data: &[f64]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }
    let mut sorted = data.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let q1 = compute_percentile(&sorted, 0.25);
    let q3 = compute_percentile(&sorted, 0.75);
    (q3 - q1).max(0.0)
}

/// Compute Median Absolute Deviation (MAD) scaled by 1.4826 for normal distribution consistency.
pub fn compute_mad(data: &[f64], median: f64) -> f64 {
    if data.is_empty() {
        return 0.0;
    }
    let deviations: Vec<f64> = data.iter().map(|x| (x - median).abs()).collect();
    let median_dev = compute_median(&deviations);
    1.4826 * median_dev
}

/// Internal representation of a raw sentiment observation.
#[derive(Debug, Clone)]
pub struct RawSentimentObservation {
    pub source: String,
    pub sentiment_score: f64,
    pub date: NaiveDate,
}

/// Generate deterministic mock multi-source sentiment observations for development/testing.
pub fn generate_mock_sentiment_observations(
    ticker: &str,
    start: NaiveDate,
    end: NaiveDate,
) -> Vec<RawSentimentObservation> {
    let sources = [
        "SEC EDGAR",
        "Finnhub",
        "Bloomberg",
        "Reuters",
        "Dow Jones",
        "Seeking Alpha",
        "Wall Street Journal",
    ];

    let mut observations = Vec::new();
    let mut curr = start;

    while curr <= end {
        // Generate 1-4 articles per day across various sources
        let mut hasher = DefaultHasher::new();
        ticker.to_uppercase().hash(&mut hasher);
        curr.to_string().hash(&mut hasher);
        let seed = hasher.finish();

        let num_articles = 1 + ((seed % 4) as usize);
        for i in 0..num_articles {
            let article_seed = seed.wrapping_add((i as u64) * 1009);
            let src_idx = (article_seed as usize) % sources.len();
            let src = sources[src_idx];

            // Source-dependent bias + noise
            let base_bias = match src {
                "SEC EDGAR" => 0.05,
                "Finnhub" => 0.12,
                "Bloomberg" => 0.18,
                "Reuters" => 0.08,
                "Dow Jones" => 0.15,
                "Seeking Alpha" => -0.05,
                _ => 0.10,
            };

            let noise = (((article_seed >> 4) % 100) as f64 - 50.0) / 100.0 * 0.4;
            let score = (base_bias + noise).clamp(-1.0, 1.0);
            let rounded_score = (score * 10000.0).round() / 10000.0;

            observations.push(RawSentimentObservation {
                source: src.to_string(),
                sentiment_score: rounded_score,
                date: curr,
            });
        }

        curr = curr.succ_opt().unwrap_or(curr + ChronoDuration::days(1));
    }

    observations
}

/// Query multi-source sentiment dispersion and disagreement index for a given ticker.
#[utoipa::path(
    get,
    path = "/sentiment/disagreement",
    params(SentimentDisagreementParams),
    responses(
        (status = 200, description = "Sentiment disagreement index retrieved successfully", body = SentimentDisagreementResponse),
        (status = 400, description = "Invalid query parameters or insufficient records"),
        (status = 401, description = "Missing or invalid Bearer JWT"),
        (status = 429, description = "Rate limit exceeded")
    ),
    security(
        ("bearerAuth" = [])
    ),
    tag = "Sentiment Analysis"
)]
pub async fn get_sentiment_disagreement_handler(
    State(state): State<AppState>,
    Query(params): Query<SentimentDisagreementParams>,
) -> Response {
    let model_version = Some(state.get_model_version());
    let pipeline_version = Some(state.get_pipeline_version());
    let data_provenance = Some(state.get_data_provenance());

    // 1. Validate ticker
    let clean_ticker = match QuestDbClient::validate_and_escape_ticker(&params.ticker) {
        Ok(t) if !t.is_empty() => t,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "Bad Request",
                    "message": "Parameter 'ticker' is required and must be a valid non-empty equity symbol"
                })),
            )
                .into_response();
        }
    };

    // 2. Validate start_date & end_date
    let parsed_start = match NaiveDate::parse_from_str(params.start_date.trim(), "%Y-%m-%d") {
        Ok(d) => d,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "Bad Request",
                    "message": format!("Invalid start_date '{}', expected format YYYY-MM-DD", params.start_date)
                })),
            )
                .into_response();
        }
    };

    let parsed_end = match NaiveDate::parse_from_str(params.end_date.trim(), "%Y-%m-%d") {
        Ok(d) => d,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "Bad Request",
                    "message": format!("Invalid end_date '{}', expected format YYYY-MM-DD", params.end_date)
                })),
            )
                .into_response();
        }
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

    // 3. PIT Validation
    if !GLOBAL_PIT_DATA.is_valid_ticker(&clean_ticker, parsed_start) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "Bad Request",
                "message": format!("Ticker '{}' was not active or listed on '{}' (Point-in-Time check failed)", clean_ticker, parsed_start)
            })),
        )
            .into_response();
    }

    // 4. Validate aggregation method ("stddev", "iqr", "mad", default: "stddev")
    let raw_agg = params
        .aggregation
        .as_deref()
        .unwrap_or("stddev")
        .trim()
        .to_lowercase();
    let allowed_aggs = ["stddev", "iqr", "mad"];
    if !allowed_aggs.contains(&raw_agg.as_str()) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "Bad Request",
                "message": format!("Invalid aggregation '{}', allowed values: {:?}", raw_agg, allowed_aggs)
            })),
        )
            .into_response();
    }

    // 5. Validate min_records (min: 5, default: 10)
    let min_records = params.min_records.unwrap_or(10);
    if min_records < 5 {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "Bad Request",
                "message": format!("Parameter 'min_records' must be at least 5 (got {})", min_records)
            })),
        )
            .into_response();
    }

    // 6. Optional source filter
    let source_filter = params.source.as_ref().map(|s| s.trim().to_lowercase());

    info!(
        "[Sentiment Disagreement] ticker={}, start={}, end={}, agg={}, min_records={}, source={:?}",
        clean_ticker, parsed_start, parsed_end, raw_agg, min_records, source_filter
    );

    // 7. Fetch or generate raw observations
    let all_obs = generate_mock_sentiment_observations(&clean_ticker, parsed_start, parsed_end);

    // Apply filters
    let mut filtered_scores = Vec::new();
    let mut source_groups: HashMap<String, Vec<f64>> = HashMap::new();

    for obs in all_obs {
        if obs.date >= parsed_start && obs.date <= parsed_end {
            if let Some(ref target_src) = source_filter {
                if !obs.source.to_lowercase().contains(target_src) {
                    continue;
                }
            }
            filtered_scores.push(obs.sentiment_score);
            source_groups
                .entry(obs.source)
                .or_default()
                .push(obs.sentiment_score);
        }
    }

    let record_count = filtered_scores.len();
    if record_count < min_records {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "Bad Request",
                "message": format!("Insufficient sentiment records for ticker '{}' (found {}, required at least {})", clean_ticker, record_count, min_records)
            })),
        )
            .into_response();
    }

    // 8. Compute central tendencies
    let mean_sentiment = (compute_mean(&filtered_scores) * 10000.0).round() / 10000.0;
    let median_sentiment = (compute_median(&filtered_scores) * 10000.0).round() / 10000.0;

    // 9. Compute chosen dispersion index
    let raw_dispersion = match raw_agg.as_str() {
        "stddev" => compute_stddev(&filtered_scores, mean_sentiment),
        "iqr" => compute_iqr(&filtered_scores),
        "mad" => compute_mad(&filtered_scores, median_sentiment),
        _ => compute_stddev(&filtered_scores, mean_sentiment),
    };
    let disagreement_index = (raw_dispersion * 10000.0).round() / 10000.0;

    // 10. Compute source breakdown
    let mut sources_breakdown: Vec<SourceBreakdown> = source_groups
        .into_iter()
        .map(|(src, scores)| {
            let m = (compute_mean(&scores) * 10000.0).round() / 10000.0;
            let c = scores.len();
            SourceBreakdown {
                source: src,
                mean: m,
                count: c,
            }
        })
        .collect();

    // Sort source breakdown by count descending, then by name
    sources_breakdown.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.source.cmp(&b.source)));

    let source_count = sources_breakdown.len();
    let generated_at = Utc::now().to_rfc3339();

    let response = SentimentDisagreementResponse {
        ticker: clean_ticker,
        start_date: parsed_start.format("%Y-%m-%d").to_string(),
        end_date: parsed_end.format("%Y-%m-%d").to_string(),
        aggregation: raw_agg,
        disagreement_index,
        mean_sentiment,
        median_sentiment,
        record_count,
        source_count,
        sources_breakdown,
        generated_at,
        model_version,
        pipeline_version,
        data_provenance,
    };

    (StatusCode::OK, Json(response)).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_mean_and_median() {
        let data = vec![0.1, 0.2, 0.3, 0.4, 0.5];
        assert!((compute_mean(&data) - 0.3).abs() < 1e-6);
        assert!((compute_median(&data) - 0.3).abs() < 1e-6);

        let even_data = vec![0.1, 0.2, 0.4, 0.5];
        assert!((compute_median(&even_data) - 0.3).abs() < 1e-6);
    }

    #[test]
    fn test_compute_stddev() {
        let data = vec![0.10, 0.20, 0.30, 0.40, 0.50];
        let mean = compute_mean(&data);
        let s = compute_stddev(&data, mean);
        assert!((s - 0.1581).abs() < 0.001);
    }

    #[test]
    fn test_compute_iqr() {
        let data = vec![0.10, 0.20, 0.30, 0.40, 0.50, 0.60, 0.70];
        let iqr = compute_iqr(&data);
        assert!(iqr > 0.20 && iqr < 0.40);
    }

    #[test]
    fn test_compute_mad() {
        let data = vec![0.10, 0.20, 0.30, 0.40, 0.50];
        let median = compute_median(&data);
        let mad = compute_mad(&data, median);
        assert!(mad > 0.0);
    }

    #[test]
    fn test_generate_mock_sentiment_observations() {
        let start = NaiveDate::from_ymd_opt(2026, 8, 1).unwrap();
        let end = NaiveDate::from_ymd_opt(2026, 8, 30).unwrap();
        let obs = generate_mock_sentiment_observations("AAPL", start, end);
        assert!(!obs.is_empty());
        assert_eq!(obs[0].source.is_empty(), false);
    }
}
