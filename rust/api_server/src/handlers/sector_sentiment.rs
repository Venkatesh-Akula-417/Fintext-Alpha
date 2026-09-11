//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Sector-Level Sentiment Aggregation Handler
//! ═══════════════════════════════════════════════════════════════════════════════

use crate::auth::AuthErrorResponse;
use crate::models::{SectorSentimentParams, SectorSentimentResponse};
use crate::pit::GLOBAL_PIT_DATA;
use crate::sector::GLOBAL_SECTOR_MAP;
use crate::state::AppState;
use crate::storage::{QuestDbClient, QuestDbClientConfig};
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use chrono::{Duration, NaiveDate, Utc};
use once_cell::sync::Lazy;
use tracing::{error, info};

static QUESTDB_CLIENT: Lazy<QuestDbClient> =
    Lazy::new(|| QuestDbClient::new(QuestDbClientConfig::default()));

pub const MAX_SECTOR_DATE_SPAN_DAYS: i64 = 730;

/// Query Sector-Level Aggregated Sentiment Metrics.
///
/// Computes aggregate financial sentiment statistics (average, sum, count, median, or confidence-weighted average)
/// across all constituent tickers in a specified GICS sector or industry over a historical date range.
#[utoipa::path(
    get,
    path = "/sentiment/sector",
    tag = "Sentiment Analysis",
    params(
        ("sector" = String, Query, description = "GICS Sector name (e.g. 'Technology', 'Financials', 'Healthcare', 'Energy')"),
        ("start_date" = String, Query, description = "Start date formatted as YYYY-MM-DD (e.g., '2025-01-01')"),
        ("end_date" = String, Query, description = "End date formatted as YYYY-MM-DD (e.g., '2025-03-31')"),
        ("aggregation" = Option<String>, Query, description = "Aggregation operator: 'average', 'sum', 'count', 'median', 'weighted_average' (default: 'average')"),
        ("min_confidence" = Option<f64>, Query, description = "Minimum confidence threshold between 0.0 and 1.0 (default: 0.0)")
    ),
    responses(
        (status = 200, description = "Sector sentiment metric calculated successfully", body = SectorSentimentResponse),
        (status = 400, description = "Invalid request parameters or date range", body = AuthErrorResponse),
        (status = 401, description = "Unauthorized (missing or invalid Bearer JWT or API key)", body = AuthErrorResponse),
        (status = 404, description = "Specified sector not found in GICS mapping", body = AuthErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::rate_limit::RateLimitErrorResponse)
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn get_sector_sentiment_handler(
    State(state): State<AppState>,
    Query(params): Query<SectorSentimentParams>,
) -> Response {
    let sector_raw = params.sector.trim();
    if sector_raw.is_empty() {
        let err = AuthErrorResponse {
            error: "Bad Request".to_string(),
            message: "sector parameter cannot be empty".to_string(),
        };
        return (StatusCode::BAD_REQUEST, Json(err)).into_response();
    }

    let model_version = Some(state.get_model_version());
    let pipeline_version = Some(state.get_pipeline_version());
    let data_provenance = Some(state.get_data_provenance());

    // 1. Resolve Sector Mapping
    let sector_map = GLOBAL_SECTOR_MAP.clone();
    let tickers = match sector_map.get_tickers_by_sector(sector_raw) {
        Some(t) if !t.is_empty() => t.clone(),
        _ => {
            let available = sector_map.list_sectors();
            let err = AuthErrorResponse {
                error: "Not Found".to_string(),
                message: format!(
                    "Unknown sector '{}'. Available GICS sectors: {:?}",
                    sector_raw, available
                ),
            };
            return (StatusCode::NOT_FOUND, Json(err)).into_response();
        }
    };
    let canonical_sector = sector_map
        .get_canonical_name(sector_raw)
        .unwrap_or_else(|| sector_raw.to_string());

    // 2. Validate dates
    let start_str = params.start_date.trim();
    let end_str = params.end_date.trim();

    let start_date = match NaiveDate::parse_from_str(start_str, "%Y-%m-%d") {
        Ok(d) => d,
        Err(e) => {
            let err = AuthErrorResponse {
                error: "Bad Request".to_string(),
                message: format!(
                    "Invalid start_date format '{}', expected YYYY-MM-DD: {}",
                    start_str, e
                ),
            };
            return (StatusCode::BAD_REQUEST, Json(err)).into_response();
        }
    };

    let end_date = match NaiveDate::parse_from_str(end_str, "%Y-%m-%d") {
        Ok(d) => d,
        Err(e) => {
            let err = AuthErrorResponse {
                error: "Bad Request".to_string(),
                message: format!(
                    "Invalid end_date format '{}', expected YYYY-MM-DD: {}",
                    end_str, e
                ),
            };
            return (StatusCode::BAD_REQUEST, Json(err)).into_response();
        }
    };

    if start_date > end_date {
        let err = AuthErrorResponse {
            error: "Bad Request".to_string(),
            message: format!(
                "start_date '{}' cannot be after end_date '{}'",
                start_str, end_str
            ),
        };
        return (StatusCode::BAD_REQUEST, Json(err)).into_response();
    }

    let date_span = (end_date - start_date).num_days();
    if date_span > MAX_SECTOR_DATE_SPAN_DAYS {
        let err = AuthErrorResponse {
            error: "Bad Request".to_string(),
            message: format!(
                "Date range ({} days) exceeds maximum allowed span of {} days",
                date_span, MAX_SECTOR_DATE_SPAN_DAYS
            ),
        };
        return (StatusCode::BAD_REQUEST, Json(err)).into_response();
    }

    // 2b. Validate as_of_utc if present
    if let Some(ref as_of_str) = params.as_of_utc {
        if !as_of_str.trim().is_empty() {
            if chrono::DateTime::parse_from_rfc3339(as_of_str.trim()).is_err() {
                let err = AuthErrorResponse {
                    error: "Bad Request".to_string(),
                    message: format!("Invalid as_of_utc format '{}', expected RFC3339", as_of_str),
                };
                return (StatusCode::BAD_REQUEST, Json(err)).into_response();
            }
        }
    }

    // 3. Validate aggregation method
    let agg_raw = params
        .aggregation
        .unwrap_or_else(|| "average".to_string())
        .trim()
        .to_lowercase();
    let valid_aggs = ["average", "sum", "count", "median", "weighted_average"];
    if !valid_aggs.contains(&agg_raw.as_str()) {
        let err = AuthErrorResponse {
            error: "Bad Request".to_string(),
            message: format!(
                "Unsupported aggregation operator '{}'. Allowed: {:?}",
                agg_raw, valid_aggs
            ),
        };
        return (StatusCode::BAD_REQUEST, Json(err)).into_response();
    }

    // 4. Validate min_confidence
    let min_conf = params.min_confidence.unwrap_or(0.0);
    if !(0.0..=1.0).contains(&min_conf) {
        let err = AuthErrorResponse {
            error: "Bad Request".to_string(),
            message: format!(
                "min_confidence must be between 0.0 and 1.0, got {}",
                min_conf
            ),
        };
        return (StatusCode::BAD_REQUEST, Json(err)).into_response();
    }

    // 5. Point-in-Time Active Ticker Filtering
    let pit_data = GLOBAL_PIT_DATA.clone();
    let active_tickers: Vec<String> = if pit_data.is_enabled() {
        pit_data.filter_universe_by_date(&tickers, start_date)
    } else {
        tickers
    };

    let tickers_included = active_tickers.len();

    // 6. Fast-path Mock Mode Fallback
    if crate::state::is_questdb_mock_fallback_enabled() {
        let (value, record_count) = compute_mock_sector_aggregation(
            &active_tickers,
            start_date,
            end_date,
            &agg_raw,
            min_conf,
        );

        let resp = SectorSentimentResponse {
            sector: canonical_sector,
            start_date: start_str.to_string(),
            end_date: end_str.to_string(),
            aggregation: agg_raw,
            min_confidence: min_conf,
            value,
            record_count,
            tickers_included,
            generated_at: Utc::now().to_rfc3339(),
            model_version: model_version.clone(),
            pipeline_version: pipeline_version.clone(),
            data_provenance: data_provenance.clone(),
        };
        return (StatusCode::OK, Json(resp)).into_response();
    }

    // 7. Live Production QuestDB Execution
    if active_tickers.is_empty() {
        let resp = SectorSentimentResponse {
            sector: canonical_sector,
            start_date: start_str.to_string(),
            end_date: end_str.to_string(),
            aggregation: agg_raw,
            min_confidence: min_conf,
            value: 0.0,
            record_count: 0,
            tickers_included: 0,
            generated_at: Utc::now().to_rfc3339(),
            model_version: model_version.clone(),
            pipeline_version: pipeline_version.clone(),
            data_provenance: data_provenance.clone(),
        };
        return (StatusCode::OK, Json(resp)).into_response();
    }

    let in_clause: String = active_tickers
        .iter()
        .map(|t| format!("'{}'", t.replace('\'', "''")))
        .collect::<Vec<String>>()
        .join(", ");

    let start_iso = format!("{}T00:00:00.000000Z", start_date.format("%Y-%m-%d"));
    let next_end = end_date + Duration::days(1);
    let end_iso = format!("{}T00:00:00.000000Z", next_end.format("%Y-%m-%d"));

    let sql = format!(
        "SELECT ticker, sentiment_score, sentiment_label, prob_positive, prob_negative, prob_neutral \
         FROM sentiment_news \
         WHERE ticker IN ({}) AND timestamp >= '{}' AND timestamp < '{}';",
        in_clause, start_iso, end_iso
    );

    info!(
        "[Sector Sentiment] Executing QuestDB SQL for sector='{}' ({} tickers)",
        canonical_sector, tickers_included
    );

    match QUESTDB_CLIENT.exec_raw_query(&sql).await {
        Ok(resp_json) => {
            let mut items: Vec<(f64, f64)> = Vec::new(); // (score, confidence)

            if let Some(dataset) = resp_json.get("dataset").and_then(|d| d.as_array()) {
                for row in dataset {
                    if let Some(row_arr) = row.as_array() {
                        if row_arr.len() >= 6 {
                            let score = row_arr[1].as_f64().unwrap_or(0.0);
                            let label = row_arr[2].as_str().unwrap_or("NEUTRAL");
                            let p_pos = row_arr[3].as_f64();
                            let p_neg = row_arr[4].as_f64();
                            let p_neu = row_arr[5].as_f64();

                            let (conf, _) =
                                crate::models::sentiment::compute_confidence_and_probabilities(
                                    score, label, p_pos, p_neg, p_neu,
                                );

                            if (conf as f64) >= min_conf {
                                items.push((score, conf as f64));
                            }
                        }
                    }
                }
            }

            let (value, record_count) = compute_aggregation_from_items(&items, &agg_raw);

            let resp = SectorSentimentResponse {
                sector: canonical_sector,
                start_date: start_str.to_string(),
                end_date: end_str.to_string(),
                aggregation: agg_raw,
                min_confidence: min_conf,
                value,
                record_count,
                tickers_included,
                generated_at: Utc::now().to_rfc3339(),
                model_version,
                pipeline_version,
                data_provenance,
            };
            (StatusCode::OK, Json(resp)).into_response()
        }
        Err(e) => {
            error!("[Sector Sentiment] QuestDB error: {}", e);
            let message = if crate::state::is_production_mode() {
                "Required data source unavailable in production mode.".to_string()
            } else {
                format!("QuestDB query error: {}", e)
            };
            let err = AuthErrorResponse {
                error: "Service Unavailable".to_string(),
                message,
            };
            (StatusCode::SERVICE_UNAVAILABLE, Json(err)).into_response()
        }
    }
}

/// Compute aggregation value and record count from (score, confidence) items.
pub fn compute_aggregation_from_items(items: &[(f64, f64)], aggregation: &str) -> (f64, usize) {
    let count = items.len();
    if count == 0 {
        return (0.0, 0);
    }

    let val = match aggregation {
        "count" => count as f64,
        "sum" => items.iter().map(|(s, _)| *s).sum::<f64>(),
        "average" => items.iter().map(|(s, _)| *s).sum::<f64>() / (count as f64),
        "median" => {
            let mut scores: Vec<f64> = items.iter().map(|(s, _)| *s).collect();
            scores.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            if count % 2 == 1 {
                scores[count / 2]
            } else {
                (scores[count / 2 - 1] + scores[count / 2]) / 2.0
            }
        }
        "weighted_average" => {
            let total_weight: f64 = items.iter().map(|(_, c)| *c).sum();
            if total_weight > 0.0001 {
                items.iter().map(|(s, c)| s * c).sum::<f64>() / total_weight
            } else {
                items.iter().map(|(s, _)| *s).sum::<f64>() / (count as f64)
            }
        }
        _ => items.iter().map(|(s, _)| *s).sum::<f64>() / (count as f64),
    };

    let rounded = (val * 10000.0).round() / 10000.0;
    (rounded, count)
}

/// Generate deterministic mock aggregation values for isolated unit and integration testing.
pub fn compute_mock_sector_aggregation(
    tickers: &[String],
    start_date: NaiveDate,
    end_date: NaiveDate,
    aggregation: &str,
    min_confidence: f64,
) -> (f64, usize) {
    if tickers.is_empty() {
        return (0.0, 0);
    }

    let num_days = (end_date - start_date).num_days().max(1) as usize;
    let mut items: Vec<(f64, f64)> = Vec::new();

    for (t_idx, ticker) in tickers.iter().enumerate() {
        let ticker_hash: usize = ticker.bytes().map(|b| b as usize).sum();
        let step = (num_days / 10).max(1);

        for day in (0..num_days).step_by(step) {
            let phase = ((day + ticker_hash + t_idx * 17) as f64) * 0.13;
            let score = ((phase.sin() * 0.65) + 0.15).clamp(-1.0, 1.0);
            let confidence = (0.55 + (phase.cos() * 0.35).abs()).clamp(0.1, 0.98);

            if confidence >= min_confidence {
                items.push((score, confidence));
            }
        }
    }

    compute_aggregation_from_items(&items, aggregation)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_aggregation_methods() {
        let items = vec![(0.20, 0.80), (0.40, 0.60), (0.60, 0.90), (0.80, 0.50)];

        // Average = (0.2 + 0.4 + 0.6 + 0.8) / 4 = 0.50
        let (avg, cnt) = compute_aggregation_from_items(&items, "average");
        assert_eq!(cnt, 4);
        assert_eq!(avg, 0.50);

        // Sum = 2.0
        let (sum, _) = compute_aggregation_from_items(&items, "sum");
        assert_eq!(sum, 2.0);

        // Count = 4.0
        let (count_val, _) = compute_aggregation_from_items(&items, "count");
        assert_eq!(count_val, 4.0);

        // Median = (0.4 + 0.6) / 2 = 0.50
        let (median, _) = compute_aggregation_from_items(&items, "median");
        assert_eq!(median, 0.50);

        // Weighted Average = (0.2*0.8 + 0.4*0.6 + 0.6*0.9 + 0.8*0.5) / (0.8 + 0.6 + 0.9 + 0.5)
        // = (0.16 + 0.24 + 0.54 + 0.40) / 2.80 = 1.34 / 2.80 = 0.4786
        let (w_avg, _) = compute_aggregation_from_items(&items, "weighted_average");
        assert_eq!(w_avg, 0.4786);
    }

    #[test]
    fn test_empty_items_aggregation() {
        let items: Vec<(f64, f64)> = Vec::new();
        let (val, count) = compute_aggregation_from_items(&items, "average");
        assert_eq!(val, 0.0);
        assert_eq!(count, 0);
    }
}
