//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — User API Usage Statistics & Analytics Handler
//! ═══════════════════════════════════════════════════════════════════════════════

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use axum::Extension;
use chrono::{DateTime, Datelike, Duration, NaiveDate, Utc};
use serde_json::json;
use tracing::{debug, error};

use crate::auth::Claims;
use crate::models::{UsageGroupItem, UsageStatsParams, UsageStatsResponse, UsageStatsSummary};
use crate::state::AppState;

pub const DEFAULT_LOOKBACK_DAYS: i64 = 30;
pub const DEFAULT_LIMIT: u32 = 100;
pub const MAX_LIMIT: u32 = 1000;

/// Linear-interpolated exact percentile calculation on a pre-sorted slice.
pub fn compute_percentile(sorted_data: &[f64], p: f64) -> f64 {
    if sorted_data.is_empty() {
        return 0.0;
    }
    if sorted_data.len() == 1 {
        return sorted_data[0];
    }
    let p_clamped = p.clamp(0.0, 100.0);
    let idx = (sorted_data.len() as f64 - 1.0) * (p_clamped / 100.0);
    let low = idx.floor() as usize;
    let high = idx.ceil() as usize;
    if low == high {
        sorted_data[low]
    } else {
        let weight = idx - low as f64;
        sorted_data[low] * (1.0 - weight) + sorted_data[high] * weight
    }
}

/// Generate deterministic synthetic usage statistics when PostgreSQL is unconfigured or offline.
pub fn generate_mock_usage_stats(
    user_id: &str,
    start_date: NaiveDate,
    end_date: NaiveDate,
    group_by: &str,
    limit: u32,
) -> UsageStatsResponse {
    let days_span = (end_date - start_date).num_days().max(1) as u64;
    let base_daily_reqs: u64 = 42;
    let total_requests = base_daily_reqs * days_span;
    let failed_requests = (total_requests as f64 * 0.036).round() as u64; // ~3.6% failure rate
    let rate_limited_requests = (total_requests as f64 * 0.010).round() as u64; // ~1.0% 429
    let successful_requests = total_requests.saturating_sub(failed_requests);

    let average_latency_ms = 4.85;
    let p95_latency_ms = 14.20;
    let max_latency_ms = 45.60;

    let summary = UsageStatsSummary {
        total_requests,
        successful_requests,
        failed_requests,
        rate_limited_requests,
        average_latency_ms,
        p95_latency_ms,
        max_latency_ms,
    };

    let mut breakdown = Vec::new();

    match group_by {
        "day" => {
            let mut curr = start_date;
            while curr <= end_date && (breakdown.len() as u32) < limit {
                let day_reqs = base_daily_reqs;
                let day_failed = (day_reqs as f64 * 0.036).round() as u64;
                let day_rate_limited = if curr.day() % 5 == 0 { 1 } else { 0 };
                let day_success = day_reqs.saturating_sub(day_failed);

                breakdown.push(UsageGroupItem {
                    key: curr.format("%Y-%m-%d").to_string(),
                    count: day_reqs,
                    successful_requests: day_success,
                    failed_requests: day_failed,
                    rate_limited_requests: day_rate_limited,
                    average_latency_ms: 4.5 + ((curr.day() % 7) as f64) * 0.1,
                    p95_latency_ms: 13.5 + ((curr.day() % 5) as f64) * 0.2,
                    max_latency_ms: 38.0 + ((curr.day() % 9) as f64) * 1.0,
                });
                curr += Duration::days(1);
            }
        }
        "endpoint" => {
            let endpoints = [
                ("/sentiment", 0.45, 3.2, 8.5, 24.0),
                ("/options/iv", 0.20, 5.8, 16.0, 42.0),
                ("/options/unusual", 0.15, 6.4, 18.2, 45.6),
                ("/sentiment/history", 0.10, 4.1, 11.0, 31.0),
                ("/spillovers", 0.06, 5.0, 14.5, 36.0),
                ("/backtest", 0.04, 18.5, 45.0, 120.0),
            ];

            for (ep, share, avg_lat, p95_lat, max_lat) in endpoints {
                if (breakdown.len() as u32) >= limit {
                    break;
                }
                let count = (total_requests as f64 * share).round() as u64;
                let failed = (count as f64 * 0.035).round() as u64;
                let rate_limited = (count as f64 * 0.01).round() as u64;
                let successful = count.saturating_sub(failed);

                breakdown.push(UsageGroupItem {
                    key: ep.to_string(),
                    count,
                    successful_requests: successful,
                    failed_requests: failed,
                    rate_limited_requests: rate_limited,
                    average_latency_ms: avg_lat,
                    p95_latency_ms: p95_lat,
                    max_latency_ms: max_lat,
                });
            }
        }
        "method" => {
            let methods = [
                ("GET", 0.94, 4.2, 12.5, 45.6),
                ("POST", 0.06, 15.0, 38.0, 120.0),
            ];

            for (m, share, avg_lat, p95_lat, max_lat) in methods {
                if (breakdown.len() as u32) >= limit {
                    break;
                }
                let count = (total_requests as f64 * share).round() as u64;
                let failed = (count as f64 * 0.035).round() as u64;
                let rate_limited = (count as f64 * 0.01).round() as u64;
                let successful = count.saturating_sub(failed);

                breakdown.push(UsageGroupItem {
                    key: m.to_string(),
                    count,
                    successful_requests: successful,
                    failed_requests: failed,
                    rate_limited_requests: rate_limited,
                    average_latency_ms: avg_lat,
                    p95_latency_ms: p95_lat,
                    max_latency_ms: max_lat,
                });
            }
        }
        "status_code" => {
            let statuses = [
                ("200", successful_requests, 0, 0, 4.2, 12.0, 38.0),
                (
                    "400",
                    (failed_requests as f64 * 0.6).round() as u64,
                    (failed_requests as f64 * 0.6).round() as u64,
                    0,
                    1.2,
                    2.5,
                    8.0,
                ),
                (
                    "429",
                    rate_limited_requests,
                    rate_limited_requests,
                    rate_limited_requests,
                    0.8,
                    1.5,
                    4.0,
                ),
                (
                    "500",
                    failed_requests
                        .saturating_sub((failed_requests as f64 * 0.6).round() as u64)
                        .saturating_sub(rate_limited_requests),
                    failed_requests
                        .saturating_sub((failed_requests as f64 * 0.6).round() as u64)
                        .saturating_sub(rate_limited_requests),
                    0,
                    8.5,
                    22.0,
                    45.6,
                ),
            ];

            for (st, count, failed, rate_lim, avg_lat, p95_lat, max_lat) in statuses {
                if (breakdown.len() as u32) >= limit {
                    break;
                }
                if count > 0 {
                    let successful = if st == "200" { count } else { 0 };
                    breakdown.push(UsageGroupItem {
                        key: st.to_string(),
                        count,
                        successful_requests: successful,
                        failed_requests: failed,
                        rate_limited_requests: rate_lim,
                        average_latency_ms: avg_lat,
                        p95_latency_ms: p95_lat,
                        max_latency_ms: max_lat,
                    });
                }
            }
        }
        _ => {}
    }

    UsageStatsResponse {
        user_id: user_id.to_string(),
        start_date: start_date.format("%Y-%m-%d").to_string(),
        end_date: end_date.format("%Y-%m-%d").to_string(),
        group_by: group_by.to_string(),
        summary,
        breakdown,
        message: "Usage statistics aggregated successfully (offline/mock mode)".to_string(),
    }
}

/// Query live usage events from PostgreSQL database.
pub async fn query_postgres_usage_stats(
    pool: &sqlx::PgPool,
    user_id: &str,
    start_dt: DateTime<Utc>,
    end_dt: DateTime<Utc>,
    start_date_str: &str,
    end_date_str: &str,
    group_by: &str,
    limit: u32,
) -> Result<UsageStatsResponse, sqlx::Error> {
    // 1. Summary Query
    let summary_row = sqlx::query_as::<_, (i64, i64, i64, i64, f64, f64)>(
        r#"
        SELECT 
            COUNT(*)::BIGINT,
            COALESCE(SUM(CASE WHEN status_code BETWEEN 200 AND 299 THEN 1 ELSE 0 END), 0)::BIGINT,
            COALESCE(SUM(CASE WHEN status_code >= 400 THEN 1 ELSE 0 END), 0)::BIGINT,
            COALESCE(SUM(CASE WHEN status_code = 429 THEN 1 ELSE 0 END), 0)::BIGINT,
            COALESCE(AVG(latency_ms), 0.0)::FLOAT8,
            COALESCE(MAX(latency_ms), 0.0)::FLOAT8
        FROM usage_events
        WHERE user_id = $1 AND created_at >= $2 AND created_at <= $3;
        "#,
    )
    .bind(user_id)
    .bind(start_dt)
    .bind(end_dt)
    .fetch_one(pool)
    .await?;

    let total_requests = summary_row.0 as u64;
    let successful_requests = summary_row.1 as u64;
    let failed_requests = summary_row.2 as u64;
    let rate_limited_requests = summary_row.3 as u64;
    let average_latency_ms = (summary_row.4 * 100.0).round() / 100.0;
    let max_latency_ms = (summary_row.5 * 100.0).round() / 100.0;

    // 2. Compute 95th percentile latency from raw latencies
    let latency_rows = sqlx::query_as::<_, (f32,)>(
        r#"
        SELECT latency_ms
        FROM usage_events
        WHERE user_id = $1 AND created_at >= $2 AND created_at <= $3
        ORDER BY latency_ms ASC;
        "#,
    )
    .bind(user_id)
    .bind(start_dt)
    .bind(end_dt)
    .fetch_all(pool)
    .await?;

    let sorted_latencies: Vec<f64> = latency_rows.into_iter().map(|(l,)| l as f64).collect();
    let p95_latency_ms = (compute_percentile(&sorted_latencies, 95.0) * 100.0).round() / 100.0;

    let summary = UsageStatsSummary {
        total_requests,
        successful_requests,
        failed_requests,
        rate_limited_requests,
        average_latency_ms,
        p95_latency_ms,
        max_latency_ms,
    };

    // 3. Breakdown Query based on group_by dimension
    let group_query_sql = match group_by {
        "day" => {
            r#"
            SELECT 
                to_char(date_trunc('day', created_at), 'YYYY-MM-DD') AS group_key,
                COUNT(*)::BIGINT,
                COALESCE(SUM(CASE WHEN status_code BETWEEN 200 AND 299 THEN 1 ELSE 0 END), 0)::BIGINT,
                COALESCE(SUM(CASE WHEN status_code >= 400 THEN 1 ELSE 0 END), 0)::BIGINT,
                COALESCE(SUM(CASE WHEN status_code = 429 THEN 1 ELSE 0 END), 0)::BIGINT,
                COALESCE(AVG(latency_ms), 0.0)::FLOAT8,
                COALESCE(MAX(latency_ms), 0.0)::FLOAT8
            FROM usage_events
            WHERE user_id = $1 AND created_at >= $2 AND created_at <= $3
            GROUP BY date_trunc('day', created_at)
            ORDER BY date_trunc('day', created_at) ASC
            LIMIT $4;
            "#
        }
        "endpoint" => {
            r#"
            SELECT 
                endpoint AS group_key,
                COUNT(*)::BIGINT,
                COALESCE(SUM(CASE WHEN status_code BETWEEN 200 AND 299 THEN 1 ELSE 0 END), 0)::BIGINT,
                COALESCE(SUM(CASE WHEN status_code >= 400 THEN 1 ELSE 0 END), 0)::BIGINT,
                COALESCE(SUM(CASE WHEN status_code = 429 THEN 1 ELSE 0 END), 0)::BIGINT,
                COALESCE(AVG(latency_ms), 0.0)::FLOAT8,
                COALESCE(MAX(latency_ms), 0.0)::FLOAT8
            FROM usage_events
            WHERE user_id = $1 AND created_at >= $2 AND created_at <= $3
            GROUP BY endpoint
            ORDER BY COUNT(*) DESC
            LIMIT $4;
            "#
        }
        "method" => {
            r#"
            SELECT 
                method AS group_key,
                COUNT(*)::BIGINT,
                COALESCE(SUM(CASE WHEN status_code BETWEEN 200 AND 299 THEN 1 ELSE 0 END), 0)::BIGINT,
                COALESCE(SUM(CASE WHEN status_code >= 400 THEN 1 ELSE 0 END), 0)::BIGINT,
                COALESCE(SUM(CASE WHEN status_code = 429 THEN 1 ELSE 0 END), 0)::BIGINT,
                COALESCE(AVG(latency_ms), 0.0)::FLOAT8,
                COALESCE(MAX(latency_ms), 0.0)::FLOAT8
            FROM usage_events
            WHERE user_id = $1 AND created_at >= $2 AND created_at <= $3
            GROUP BY method
            ORDER BY COUNT(*) DESC
            LIMIT $4;
            "#
        }
        "status_code" => {
            r#"
            SELECT 
                status_code::TEXT AS group_key,
                COUNT(*)::BIGINT,
                COALESCE(SUM(CASE WHEN status_code BETWEEN 200 AND 299 THEN 1 ELSE 0 END), 0)::BIGINT,
                COALESCE(SUM(CASE WHEN status_code >= 400 THEN 1 ELSE 0 END), 0)::BIGINT,
                COALESCE(SUM(CASE WHEN status_code = 429 THEN 1 ELSE 0 END), 0)::BIGINT,
                COALESCE(AVG(latency_ms), 0.0)::FLOAT8,
                COALESCE(MAX(latency_ms), 0.0)::FLOAT8
            FROM usage_events
            WHERE user_id = $1 AND created_at >= $2 AND created_at <= $3
            GROUP BY status_code
            ORDER BY COUNT(*) DESC
            LIMIT $4;
            "#
        }
        _ => return Err(sqlx::Error::Configuration("Invalid group_by".into())),
    };

    let group_rows = sqlx::query_as::<_, (String, i64, i64, i64, i64, f64, f64)>(group_query_sql)
        .bind(user_id)
        .bind(start_dt)
        .bind(end_dt)
        .bind(limit as i64)
        .fetch_all(pool)
        .await?;

    let breakdown: Vec<UsageGroupItem> = group_rows
        .into_iter()
        .map(|(key, count, succ, fail, ratelim, avg_lat, max_lat)| {
            let avg_rounded = (avg_lat * 100.0).round() / 100.0;
            let max_rounded = (max_lat * 100.0).round() / 100.0;
            // Approximate p95 for group as 1.8x average capped at max
            let p95_est = ((avg_rounded * 1.8).min(max_rounded) * 100.0).round() / 100.0;

            UsageGroupItem {
                key,
                count: count as u64,
                successful_requests: succ as u64,
                failed_requests: fail as u64,
                rate_limited_requests: ratelim as u64,
                average_latency_ms: avg_rounded,
                p95_latency_ms: p95_est,
                max_latency_ms: max_rounded,
            }
        })
        .collect();

    Ok(UsageStatsResponse {
        user_id: user_id.to_string(),
        start_date: start_date_str.to_string(),
        end_date: end_date_str.to_string(),
        group_by: group_by.to_string(),
        summary,
        breakdown,
        message: "Usage statistics aggregated successfully".to_string(),
    })
}

/// Endpoint Handler: `GET /usage/stats`
///
/// Fetches aggregated usage metrics and consumption statistics for the authenticated user.
#[utoipa::path(
    get,
    path = "/usage/stats",
    tag = "Usage & Analytics",
    params(
        UsageStatsParams
    ),
    responses(
        (status = 200, description = "Aggregated usage statistics and breakdown", body = UsageStatsResponse),
        (status = 400, description = "Invalid request parameters or malformed dates", body = crate::rate_limit::RateLimitErrorResponse),
        (status = 401, description = "Missing or invalid authentication token", body = crate::auth::AuthErrorResponse),
        (status = 503, description = "Database service unavailable", body = crate::rate_limit::RateLimitErrorResponse)
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn get_usage_stats_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Query(params): Query<UsageStatsParams>,
) -> Response {
    let now = Utc::now();
    let today = now.date_naive();

    // 1. Validate and resolve end_date
    let end_date = match &params.end_date {
        Some(s) if !s.trim().is_empty() => match NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d") {
            Ok(d) => d,
            Err(_) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({
                        "error": "Invalid Parameter",
                        "message": "Invalid end_date format. Expected ISO YYYY-MM-DD.",
                        "field": "end_date"
                    })),
                )
                    .into_response();
            }
        },
        _ => today,
    };

    // 2. Validate and resolve start_date
    let start_date = match &params.start_date {
        Some(s) if !s.trim().is_empty() => match NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d") {
            Ok(d) => d,
            Err(_) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({
                        "error": "Invalid Parameter",
                        "message": "Invalid start_date format. Expected ISO YYYY-MM-DD.",
                        "field": "start_date"
                    })),
                )
                    .into_response();
            }
        },
        _ => end_date - Duration::days(DEFAULT_LOOKBACK_DAYS),
    };

    // 3. Verify date range ordering
    if start_date > end_date {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Invalid Date Range",
                "message": format!("start_date ({}) cannot be after end_date ({})", start_date, end_date),
                "field": "start_date"
            })),
        )
            .into_response();
    }

    // 4. Validate group_by dimension
    let group_by = params
        .group_by
        .as_deref()
        .map(|s| s.trim().to_lowercase())
        .unwrap_or_else(|| "day".to_string());

    if !matches!(
        group_by.as_str(),
        "day" | "endpoint" | "method" | "status_code"
    ) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Invalid Parameter",
                "message": format!("Invalid group_by '{}'. Allowed values are: 'day', 'endpoint', 'method', 'status_code'.", group_by),
                "field": "group_by"
            })),
        )
            .into_response();
    }

    // 5. Validate limit
    let limit = params.limit.unwrap_or(DEFAULT_LIMIT);
    if limit == 0 || limit > MAX_LIMIT {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Invalid Parameter",
                "message": format!("limit must be between 1 and {}.", MAX_LIMIT),
                "field": "limit"
            })),
        )
            .into_response();
    }

    let start_date_str = start_date.format("%Y-%m-%d").to_string();
    let end_date_str = end_date.format("%Y-%m-%d").to_string();
    let user_id = &claims.sub;

    debug!(
        "[Usage Stats] Querying stats for user '{}' from {} to {}, grouped by '{}', limit {}",
        user_id, start_date_str, end_date_str, group_by, limit
    );

    // 6. Execute query against database pool or fall back to mock generator
    if let Some(pool) = &state.db_pool {
        let start_dt = start_date.and_hms_opt(0, 0, 0).unwrap().and_utc();
        let end_dt = end_date.and_hms_opt(23, 59, 59).unwrap().and_utc();

        match query_postgres_usage_stats(
            pool,
            user_id,
            start_dt,
            end_dt,
            &start_date_str,
            &end_date_str,
            &group_by,
            limit,
        )
        .await
        {
            Ok(resp) => (StatusCode::OK, Json(resp)).into_response(),
            Err(e) => {
                error!("[Usage Stats] PostgreSQL query failed: {}", e);
                (
                    StatusCode::SERVICE_UNAVAILABLE,
                    Json(json!({
                        "error": "Service Unavailable",
                        "message": format!("Failed to query usage statistics from database: {}", e)
                    })),
                )
                    .into_response()
            }
        }
    } else {
        let resp = generate_mock_usage_stats(user_id, start_date, end_date, &group_by, limit);
        (StatusCode::OK, Json(resp)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_percentile_empty_and_single() {
        assert_eq!(compute_percentile(&[], 95.0), 0.0);
        assert_eq!(compute_percentile(&[42.0], 95.0), 42.0);
    }

    #[test]
    fn test_compute_percentile_multi_element() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
        let p50 = compute_percentile(&data, 50.0);
        assert!((p50 - 5.5).abs() < 1e-6);

        let p95 = compute_percentile(&data, 95.0);
        assert!((p95 - 9.55).abs() < 1e-6);

        let p100 = compute_percentile(&data, 100.0);
        assert_eq!(p100, 10.0);

        let p0 = compute_percentile(&data, 0.0);
        assert_eq!(p0, 1.0);
    }

    #[test]
    fn test_mock_usage_stats_day_grouping() {
        let start = NaiveDate::from_ymd_opt(2026, 8, 1).unwrap();
        let end = NaiveDate::from_ymd_opt(2026, 8, 10).unwrap();

        let resp = generate_mock_usage_stats("trader_01", start, end, "day", 50);
        assert_eq!(resp.user_id, "trader_01");
        assert_eq!(resp.group_by, "day");
        assert_eq!(resp.start_date, "2026-08-01");
        assert_eq!(resp.end_date, "2026-08-10");
        assert!(resp.summary.total_requests > 0);
        assert_eq!(
            resp.summary.total_requests,
            resp.summary.successful_requests + resp.summary.failed_requests
        );
        assert!(resp.summary.rate_limited_requests <= resp.summary.failed_requests);
        assert_eq!(resp.breakdown.len(), 10);
        assert_eq!(resp.breakdown[0].key, "2026-08-01");
        assert_eq!(resp.breakdown[9].key, "2026-08-10");
    }

    #[test]
    fn test_mock_usage_stats_endpoint_grouping() {
        let start = NaiveDate::from_ymd_opt(2026, 8, 1).unwrap();
        let end = NaiveDate::from_ymd_opt(2026, 8, 30).unwrap();

        let resp = generate_mock_usage_stats("trader_01", start, end, "endpoint", 5);
        assert_eq!(resp.group_by, "endpoint");
        assert_eq!(resp.breakdown.len(), 5);
        assert_eq!(resp.breakdown[0].key, "/sentiment");
        assert_eq!(resp.breakdown[1].key, "/options/iv");
        assert!(resp.breakdown[0].count >= resp.breakdown[1].count);
    }

    #[test]
    fn test_mock_usage_stats_method_grouping() {
        let start = NaiveDate::from_ymd_opt(2026, 8, 1).unwrap();
        let end = NaiveDate::from_ymd_opt(2026, 8, 30).unwrap();

        let resp = generate_mock_usage_stats("trader_01", start, end, "method", 10);
        assert_eq!(resp.group_by, "method");
        assert_eq!(resp.breakdown.len(), 2);
        assert_eq!(resp.breakdown[0].key, "GET");
        assert_eq!(resp.breakdown[1].key, "POST");
    }

    #[test]
    fn test_mock_usage_stats_status_code_grouping() {
        let start = NaiveDate::from_ymd_opt(2026, 8, 1).unwrap();
        let end = NaiveDate::from_ymd_opt(2026, 8, 30).unwrap();

        let resp = generate_mock_usage_stats("trader_01", start, end, "status_code", 10);
        assert_eq!(resp.group_by, "status_code");
        assert!(resp.breakdown.iter().any(|b| b.key == "200"));
        assert!(resp.breakdown.iter().any(|b| b.key == "400"));
        assert!(resp.breakdown.iter().any(|b| b.key == "429"));
    }
}
