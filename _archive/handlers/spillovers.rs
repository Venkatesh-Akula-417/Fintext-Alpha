//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Cross-Asset Spillover API Handler (QuestDB SQL Engine)
//! ═══════════════════════════════════════════════════════════════════════════════

use crate::models::{SpilloverItem, SpilloverQuery, SpilloverResponse};
use crate::storage::{QuestDbClient, QuestDbClientConfig};
use axum::extract::Query;
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Json, Response};
use chrono::Utc;
use once_cell::sync::Lazy;
use tracing::{error, info, warn};

static QUESTDB_CLIENT: Lazy<QuestDbClient> =
    Lazy::new(|| QuestDbClient::new(QuestDbClientConfig::default()));

/// Query Cross-Asset Lead-Lag Information Spillovers.
///
/// Computes lead-lag cross-correlation and Granger causality spillovers between the queried
/// ticker and related cross-asset equity tickers.
#[utoipa::path(
    get,
    path = "/spillovers",
    tag = "Cross-Asset Spillovers",
    params(
        ("ticker" = String, Query, description = "Primary stock ticker symbol (e.g., 'AAPL', 'MSFT')"),
        ("limit" = Option<usize>, Query, description = "Maximum number of correlated spillover relationships to return (default: 10, max: 50)"),
        ("min_correlation" = Option<f64>, Query, description = "Optional minimum correlation filter threshold")
    ),
    responses(
        (status = 200, description = "Cross-asset spillovers computed successfully", body = SpilloverResponse),
        (status = 400, description = "Invalid or missing ticker parameter", body = crate::auth::AuthErrorResponse),
        (status = 401, description = "Unauthorized (missing or invalid Bearer JWT)", body = crate::auth::AuthErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::rate_limit::RateLimitErrorResponse)
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn get_spillovers_handler(Query(params): Query<SpilloverQuery>) -> Response {
    let ticker = match QuestDbClient::validate_and_escape_ticker(&params.ticker) {
        Ok(t) => t,
        Err(e) => {
            error!("Invalid ticker parameter '{}': {}", params.ticker, e);
            let err_body = serde_json::json!({
                "error": e,
                "status": "bad_request"
            });
            return (StatusCode::BAD_REQUEST, Json(err_body)).into_response();
        }
    };

    let limit = params.limit.unwrap_or(10).clamp(1, 50);

    // Fast-path mock mode for isolated unit/integration tests without running Docker
    if crate::state::is_questdb_mock_fallback_enabled() {
        let other_ticker = if ticker == "AAPL" {
            "MSFT".to_string()
        } else {
            "AAPL".to_string()
        };
        let mock_items = vec![
            SpilloverItem {
                related_ticker: other_ticker.clone(),
                lag_hours: 1,
                correlation: 0.745,
                relationship: format!("{} LEADS {} by 1h", ticker, other_ticker),
                updated_at: Utc::now().to_rfc3339(),
            },
            SpilloverItem {
                related_ticker: "NVDA".to_string(),
                lag_hours: -2,
                correlation: 0.682,
                relationship: format!("NVDA LEADS {} by 2h", ticker),
                updated_at: Utc::now().to_rfc3339(),
            },
        ];

        let mock_resp = SpilloverResponse {
            ticker: ticker.clone(),
            count: mock_items.len(),
            spillovers: mock_items,
            status: "ok".to_string(),
            message: "Mock fallback cross-asset spillovers (QuestDB mock mode)".to_string(),
        };
        return (StatusCode::OK, Json(mock_resp)).into_response();
    }

    info!(
        "Executing QuestDB spillover lookup for ticker='{}', limit={}",
        ticker, limit
    );

    match QUESTDB_CLIENT.query_spillovers(&ticker, limit).await {
        Ok(mut spillovers) => {
            if let Some(min_corr) = params.min_correlation {
                spillovers.retain(|s| s.correlation.abs() >= min_corr);
            }

            let resp = SpilloverResponse {
                ticker: ticker.clone(),
                count: spillovers.len(),
                spillovers,
                status: "ok".to_string(),
                message: "Retrieved directly from QuestDB".to_string(),
            };
            (StatusCode::OK, Json(resp)).into_response()
        }
        Err(err) => {
            warn!("QuestDB error during spillover lookup: {}", err);

            let mut headers = HeaderMap::new();
            headers.insert(header::RETRY_AFTER, HeaderValue::from_static("5"));

            let err_body = if crate::state::is_production_mode() {
                serde_json::json!({
                    "error": "Service Unavailable",
                    "message": "Required data source unavailable in production mode.",
                    "detail": err,
                    "status": "service_unavailable"
                })
            } else {
                serde_json::json!({
                    "error": "QuestDB time-series database is temporarily unreachable.",
                    "detail": err,
                    "status": "service_unavailable"
                })
            };

            (StatusCode::SERVICE_UNAVAILABLE, headers, Json(err_body)).into_response()
        }
    }
}
