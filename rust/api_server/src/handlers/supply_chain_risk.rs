//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Supply Chain Risk Alerts & Disruption Propagation Handler
//! ═══════════════════════════════════════════════════════════════════════════════

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::Json;
use axum::Extension;
use chrono::Utc;
use serde_json::json;
use tracing::debug;

use crate::auth::Claims;
use crate::models::supply_chain::{SupplyChainRiskParams, SupplyChainRiskResponse};
use crate::state::AppState;
use crate::storage::QuestDbClient;

/// GET /events/supply-chain-risk — Identify upstream supplier and downstream customer risk propagation alerts.
#[utoipa::path(
    get,
    path = "/events/supply-chain-risk",
    params(SupplyChainRiskParams),
    responses(
        (status = 200, description = "Ranked supply chain risk propagation alerts with relationship graph tiers", body = SupplyChainRiskResponse),
        (status = 400, description = "Invalid parameters or Point-in-Time survivorship violation"),
        (status = 401, description = "Missing or invalid Bearer JWT authentication"),
        (status = 429, description = "Per-user rate limit quota exceeded")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Events"
)]
pub async fn get_supply_chain_risk_handler(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
    Query(params): Query<SupplyChainRiskParams>,
) -> Result<Json<SupplyChainRiskResponse>, (StatusCode, Json<serde_json::Value>)> {
    let clean_ticker = params.ticker.trim();
    if clean_ticker.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Bad Request",
                "message": "Field 'ticker' is required and cannot be empty."
            })),
        ));
    }

    let safe_ticker = QuestDbClient::validate_and_escape_ticker(clean_ticker).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Bad Request",
                "message": format!("Invalid ticker '{}': {}", clean_ticker, e)
            })),
        )
    })?;

    let depth = params.depth.unwrap_or(1);
    if depth < 1 || depth > 3 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Bad Request",
                "message": "Field 'depth' must be between 1 (direct tier-1) and 3 (tier-3)."
            })),
        ));
    }

    let event_days = params.event_days.unwrap_or(30);
    if event_days < 1 || event_days > 90 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Bad Request",
                "message": "Field 'event_days' must be between 1 and 90 calendar days."
            })),
        ));
    }

    let min_risk_score = params.min_risk_score.unwrap_or(0.5);
    if !(0.0..=1.0).contains(&min_risk_score) || min_risk_score.is_nan() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Bad Request",
                "message": "Field 'min_risk_score' must be between 0.0 and 1.0."
            })),
        ));
    }

    let limit = params.limit.unwrap_or(20);
    if limit < 1 || limit > 100 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Bad Request",
                "message": "Field 'limit' must be between 1 and 100 results."
            })),
        ));
    }

    // Point-in-Time (PIT) survivorship check
    let today = Utc::now().date_naive();
    if state.pit_data.is_enabled() {
        if !state.pit_data.is_valid_ticker(&safe_ticker, today) {
            let delist_info = state.pit_data.get_delisted_detail(&safe_ticker);
            let delist_date_str = delist_info
                .as_ref()
                .map(|d| d.delisting_date_iso.as_str())
                .unwrap_or("unknown date");
            let delist_reason_str = delist_info
                .as_ref()
                .and_then(|d| d.delisting_reason.as_deref())
                .unwrap_or("corporate action");
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Bad Request",
                    "message": format!(
                        "Point-in-Time Violation: Security '{}' was delisted on {} (reason: {}) and has no active supply chain graph.",
                        safe_ticker,
                        delist_date_str,
                        delist_reason_str
                    )
                })),
            ));
        }
    }

    // Compute supply chain risk alerts from graph
    let alerts = state.supply_chain_graph.compute_risk_alerts(
        &safe_ticker,
        depth,
        event_days,
        min_risk_score,
        limit,
    );

    // Dispatch webhook notifications if subscribers exist
    let subscribers = state
        .webhook_registry
        .find_subscribers_for_event("supply_chain");
    if !subscribers.is_empty() && !alerts.is_empty() {
        debug!(
            "Found {} webhook subscriber(s) for supply chain risk alerts",
            subscribers.len()
        );
        let alerts_clone = alerts.clone();
        let focal_clone = safe_ticker.clone();
        tokio::spawn(async move {
            for sub in subscribers {
                for alert in &alerts_clone {
                    let payload = json!({
                        "event": "supply_chain_risk",
                        "focal_ticker": focal_clone,
                        "related_ticker": alert.related_ticker,
                        "relationship_type": alert.relationship_type,
                        "depth": alert.depth,
                        "event_type": alert.event_type,
                        "event_description": alert.event_description,
                        "event_date": alert.event_date,
                        "risk_score": alert.risk_score,
                    });
                    debug!(
                        "Dispatching supply chain risk webhook alert to {}: {:?}",
                        sub.url, payload
                    );
                }
            }
        });
    }

    let count = alerts.len();
    let response = SupplyChainRiskResponse {
        focal_ticker: safe_ticker.clone(),
        depth,
        event_days,
        min_risk_score,
        count,
        alerts,
        message: format!(
            "Supply chain risk analysis completed for {}: {} alert(s) identified at depth <= {} (min_risk_score: {:.2})",
            safe_ticker, count, depth, min_risk_score
        ),
    };

    Ok(Json(response))
}
