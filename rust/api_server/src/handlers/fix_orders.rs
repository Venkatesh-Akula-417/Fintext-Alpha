//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — FIX Protocol Route Handlers
//! ═══════════════════════════════════════════════════════════════════════════════

use axum::body::Bytes;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use axum::Extension;
use chrono::Utc;
use serde_json::json;
use tracing::warn;
use uuid::Uuid;

use crate::audit_logs::log_audit_event;
use crate::auth::Claims;
use crate::fix::{
    build_execution_report, build_reject_message, compute_mock_price, parse_fix_message, FixOrder,
    FIX_EXEC_CANCELLED, FIX_EXEC_FILL, FIX_EXEC_NEW, FIX_MSG_NEW_ORDER_SINGLE,
    FIX_MSG_ORDER_CANCEL_REJECT, FIX_MSG_ORDER_CANCEL_REQUEST, FIX_MSG_REJECT,
    FIX_ORD_STATUS_CANCELLED, FIX_ORD_STATUS_FILLED, FIX_ORD_STATUS_NEW, FIX_ORD_TYPE_LIMIT,
    FIX_ORD_TYPE_MARKET, FIX_SIDE_BUY, FIX_SIDE_SELL, FIX_TAG_CL_ORD_ID, FIX_TAG_MSG_TYPE,
    FIX_TAG_ORDER_QTY, FIX_TAG_ORD_TYPE, FIX_TAG_ORIG_CL_ORD_ID, FIX_TAG_PRICE, FIX_TAG_SIDE,
    FIX_TAG_SYMBOL,
};
use crate::models::fix::{
    FIXCancelRequest, FIXOrderRequest, FIXOrderResponse, FIXOrdersListResponse,
    FIXOrdersQueryParams,
};
use crate::state::AppState;

/// Validates that the caller has an enterprise-tier role or admin privileges.
pub fn require_enterprise_role(claims: &Claims) -> Result<(), Response> {
    let role = claims.role.to_ascii_lowercase();
    if role == "enterprise" || role == "admin" || role == "superadmin" || role == "org_admin" {
        Ok(())
    } else {
        Err((
            StatusCode::FORBIDDEN,
            Json(json!({
                "error": "Forbidden",
                "message": "FIX Bridge is an enterprise-only feature. Please upgrade your plan."
            })),
        )
            .into_response())
    }
}

/// Submits a new FIX 4.4 order (NewOrderSingle, 35=D) for deterministic simulated execution. Enterprise plan required (`POST /fix/order`).
#[utoipa::path(
    post,
    path = "/fix/order",
    tag = "Trading & Execution",
    request_body = FIXOrderRequest,
    responses(
        (status = 200, description = "FIX order accepted and execution report generated", body = FIXOrderResponse),
        (status = 400, description = "Malformed FIX message or missing required tags", body = AuthErrorResponse),
        (status = 401, description = "Unauthorized", body = AuthErrorResponse),
        (status = 403, description = "Forbidden - Enterprise tier required", body = AuthErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::rate_limit::RateLimitErrorResponse)
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn submit_fix_order_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    body_bytes: Bytes,
) -> Response {
    if let Err(forbidden_resp) = require_enterprise_role(&claims) {
        return forbidden_resp;
    }
    let body_str = match String::from_utf8(body_bytes.to_vec()) {
        Ok(s) => s,
        Err(_) => {
            let reject_fix =
                build_reject_message(FIX_MSG_REJECT, "UNKNOWN", None, "Invalid UTF-8 payload");
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Invalid Payload",
                    "message": "Request body must be valid UTF-8",
                    "fix_message": reject_fix
                })),
            )
                .into_response();
        }
    };

    // Support both JSON wrapper { "fix_message": "..." } and raw FIX string
    let fix_raw = if let Ok(req) = serde_json::from_str::<FIXOrderRequest>(&body_str) {
        req.fix_message
    } else {
        body_str.trim().to_string()
    };

    // 1. Parse FIX message tags
    let tags = match parse_fix_message(&fix_raw) {
        Ok(t) => t,
        Err(err) => {
            let reject_fix = build_reject_message(FIX_MSG_REJECT, "UNKNOWN", None, &err);
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "FIX Parse Error",
                    "message": err,
                    "fix_message": reject_fix
                })),
            )
                .into_response();
        }
    };

    // 2. Validate MsgType (35=D)
    let msg_type = tags
        .get(&FIX_TAG_MSG_TYPE)
        .map(|s| s.as_str())
        .unwrap_or("");
    if msg_type != FIX_MSG_NEW_ORDER_SINGLE {
        let reject_fix = build_reject_message(
            FIX_MSG_REJECT,
            tags.get(&FIX_TAG_CL_ORD_ID)
                .map(|s| s.as_str())
                .unwrap_or("UNKNOWN"),
            Some(FIX_TAG_MSG_TYPE),
            "Expected MsgType D (NewOrderSingle)",
        );
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Invalid MsgType",
                "message": format!("Expected 35=D (NewOrderSingle), got '{}'", msg_type),
                "fix_message": reject_fix
            })),
        )
            .into_response();
    }

    // 3. Extract and validate required fields: ClOrdID (11), Symbol (55), Side (54), OrderQty (38), OrdType (40)
    let cl_ord_id = match tags.get(&FIX_TAG_CL_ORD_ID) {
        Some(s) if !s.trim().is_empty() => s.trim().to_string(),
        _ => {
            let reject_fix = build_reject_message(
                FIX_MSG_REJECT,
                "UNKNOWN",
                Some(FIX_TAG_CL_ORD_ID),
                "Missing ClOrdID (Tag 11)",
            );
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Missing Required Field",
                    "message": "Tag 11 (ClOrdID) is required",
                    "fix_message": reject_fix
                })),
            )
                .into_response();
        }
    };

    let symbol = match tags.get(&FIX_TAG_SYMBOL) {
        Some(s) if !s.trim().is_empty() => s.trim().to_uppercase(),
        _ => {
            let reject_fix = build_reject_message(
                FIX_MSG_REJECT,
                &cl_ord_id,
                Some(FIX_TAG_SYMBOL),
                "Missing Symbol (Tag 55)",
            );
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Missing Required Field",
                    "message": "Tag 55 (Symbol) is required",
                    "fix_message": reject_fix
                })),
            )
                .into_response();
        }
    };

    let raw_side = tags.get(&FIX_TAG_SIDE).map(|s| s.as_str()).unwrap_or("");
    let side = match raw_side {
        "1" | "BUY" | "buy" => FIX_SIDE_BUY.to_string(),
        "2" | "SELL" | "sell" => FIX_SIDE_SELL.to_string(),
        _ => {
            let reject_fix = build_reject_message(
                FIX_MSG_REJECT,
                &cl_ord_id,
                Some(FIX_TAG_SIDE),
                "Invalid Side (Tag 54: 1=Buy, 2=Sell)",
            );
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Invalid Side",
                    "message": format!("Tag 54 must be '1' (Buy) or '2' (Sell), got '{}'", raw_side),
                    "fix_message": reject_fix
                })),
            )
                .into_response();
        }
    };

    let qty: f64 = match tags
        .get(&FIX_TAG_ORDER_QTY)
        .and_then(|s| s.parse::<f64>().ok())
    {
        Some(q) if q > 0.0 => q,
        _ => {
            let reject_fix = build_reject_message(
                FIX_MSG_REJECT,
                &cl_ord_id,
                Some(FIX_TAG_ORDER_QTY),
                "Invalid OrderQty (Tag 38)",
            );
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Invalid OrderQty",
                    "message": "Tag 38 (OrderQty) must be a positive number",
                    "fix_message": reject_fix
                })),
            )
                .into_response();
        }
    };

    let raw_ord_type = tags
        .get(&FIX_TAG_ORD_TYPE)
        .map(|s| s.as_str())
        .unwrap_or("");
    let ord_type = match raw_ord_type {
        "1" | "MARKET" | "market" => FIX_ORD_TYPE_MARKET.to_string(),
        "2" | "LIMIT" | "limit" => FIX_ORD_TYPE_LIMIT.to_string(),
        _ => {
            let reject_fix = build_reject_message(
                FIX_MSG_REJECT,
                &cl_ord_id,
                Some(FIX_TAG_ORD_TYPE),
                "Invalid OrdType (Tag 40: 1=Market, 2=Limit)",
            );
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Invalid OrdType",
                    "message": format!("Tag 40 must be '1' (Market) or '2' (Limit), got '{}'", raw_ord_type),
                    "fix_message": reject_fix
                })),
            )
                .into_response();
        }
    };

    let limit_price: Option<f64> = tags.get(&FIX_TAG_PRICE).and_then(|s| s.parse::<f64>().ok());
    if ord_type == FIX_ORD_TYPE_LIMIT && (limit_price.is_none() || limit_price.unwrap() <= 0.0) {
        let reject_fix = build_reject_message(
            FIX_MSG_REJECT,
            &cl_ord_id,
            Some(FIX_TAG_PRICE),
            "Missing Price for Limit order (Tag 44)",
        );
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Missing Limit Price",
                "message": "Tag 44 (Price) is required and must be positive for Limit orders",
                "fix_message": reject_fix
            })),
        )
            .into_response();
    }

    // 4. Generate OrderID and ExecID
    let order_id = Uuid::new_v4().to_string();
    let exec_id = Uuid::new_v4().to_string();
    let mock_market_price = compute_mock_price(&symbol);

    // 5. Simulate execution logic
    // Market Orders: immediately filled at mock market price
    // Limit Orders: filled if Buy limit >= market price or Sell limit <= market price; otherwise remain Open (New)
    let (
        status,
        exec_type,
        ord_status,
        filled_qty,
        avg_price,
        last_qty,
        last_px,
        leaves_qty,
        cum_qty,
    ) = if ord_type == FIX_ORD_TYPE_MARKET {
        (
            "filled".to_string(),
            FIX_EXEC_FILL.to_string(),
            FIX_ORD_STATUS_FILLED.to_string(),
            qty,
            Some(mock_market_price),
            qty,
            mock_market_price,
            0.0,
            qty,
        )
    } else {
        let lp = limit_price.unwrap();
        let is_filled = if side == FIX_SIDE_BUY {
            lp >= mock_market_price
        } else {
            lp <= mock_market_price
        };

        if is_filled {
            (
                "filled".to_string(),
                FIX_EXEC_FILL.to_string(),
                FIX_ORD_STATUS_FILLED.to_string(),
                qty,
                Some(mock_market_price),
                qty,
                mock_market_price,
                0.0,
                qty,
            )
        } else {
            (
                "open".to_string(),
                FIX_EXEC_NEW.to_string(),
                FIX_ORD_STATUS_NEW.to_string(),
                0.0,
                None,
                0.0,
                0.0,
                qty,
                0.0,
            )
        }
    };

    let now = Utc::now();
    let fix_order = FixOrder {
        id: Uuid::new_v4(),
        user_id: claims.sub.clone(),
        cl_ord_id: cl_ord_id.clone(),
        order_id: order_id.clone(),
        symbol: symbol.clone(),
        side: side.clone(),
        order_type: ord_type.clone(),
        price: limit_price,
        qty,
        filled_qty,
        avg_price,
        status: status.clone(),
        created_at: now,
        updated_at: now,
    };

    // 6. Insert order into in-memory registry
    state.fix_order_registry.insert(fix_order.clone());

    // 7. Persist to PostgreSQL asynchronously if DB pool is present
    if let Some(pool) = state.db_pool.clone() {
        let order_clone = fix_order.clone();
        tokio::spawn(async move {
            let res = sqlx::query(
                r#"
                INSERT INTO fix_orders (id, user_id, cl_ord_id, order_id, symbol, side, order_type, qty, filled_qty, avg_price, status, created_at, updated_at)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
                "#,
            )
            .bind(order_clone.id)
            .bind(order_clone.user_id)
            .bind(order_clone.cl_ord_id)
            .bind(order_clone.order_id)
            .bind(order_clone.symbol)
            .bind(order_clone.side)
            .bind(order_clone.order_type)
            .bind(order_clone.qty)
            .bind(order_clone.filled_qty)
            .bind(order_clone.avg_price)
            .bind(order_clone.status)
            .bind(order_clone.created_at)
            .bind(order_clone.updated_at)
            .execute(&pool)
            .await;

            if let Err(e) = res {
                warn!(
                    "[FIX Registry] Failed to persist order {} to PostgreSQL: {}",
                    order_clone.id, e
                );
            }
        });
    }

    // 8. Log compliance audit events
    let org_uuid = claims
        .org_id
        .as_deref()
        .and_then(|s| Uuid::parse_str(s).ok());
    log_audit_event(
        &state,
        org_uuid,
        &claims.sub,
        "fix.order_submitted",
        "fix_order",
        Some(&order_id),
        json!({
            "cl_ord_id": cl_ord_id,
            "symbol": symbol,
            "side": side,
            "order_type": ord_type,
            "qty": qty,
            "status": status,
            "market_price": mock_market_price,
        }),
        None,
    )
    .await;

    if status == "filled" {
        log_audit_event(
            &state,
            org_uuid,
            &claims.sub,
            "fix.order_filled",
            "fix_order",
            Some(&order_id),
            json!({
                "cl_ord_id": cl_ord_id,
                "symbol": symbol,
                "qty": qty,
                "avg_price": avg_price,
                "exec_id": exec_id,
            }),
            None,
        )
        .await;
    }

    // 9. Generate FIX Execution Report (35=8)
    let fix_response_msg = build_execution_report(
        &order_id,
        &exec_id,
        &exec_type,
        &ord_status,
        &cl_ord_id,
        &symbol,
        &side,
        qty,
        last_qty,
        last_px,
        cum_qty,
        leaves_qty,
        avg_price,
    );

    let response = FIXOrderResponse {
        order_id,
        exec_id,
        cl_ord_id,
        symbol,
        side,
        order_type: ord_type,
        qty,
        filled_qty,
        avg_price,
        status,
        exec_type,
        fix_message: fix_response_msg,
        created_at: now,
    };

    (StatusCode::OK, Json(response)).into_response()
}

/// Lists FIX orders for the authenticated user with optional status filter and pagination. Enterprise plan required (`GET /fix/orders`).
#[utoipa::path(
    get,
    path = "/fix/orders",
    tag = "Trading & Execution",
    params(FIXOrdersQueryParams),
    responses(
        (status = 200, description = "List of FIX orders retrieved successfully", body = FIXOrdersListResponse),
        (status = 401, description = "Unauthorized", body = AuthErrorResponse),
        (status = 403, description = "Forbidden - Enterprise tier required", body = AuthErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::rate_limit::RateLimitErrorResponse)
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn list_fix_orders_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Query(params): Query<FIXOrdersQueryParams>,
) -> Response {
    if let Err(forbidden_resp) = require_enterprise_role(&claims) {
        return forbidden_resp;
    }
    let limit = params.limit.unwrap_or(50).clamp(1, 1000);
    let offset = params.offset.unwrap_or(0);

    let (orders, total) =
        state
            .fix_order_registry
            .query(&claims.sub, params.status.as_deref(), limit, offset);

    let resp = FIXOrdersListResponse {
        orders,
        total,
        limit,
        offset,
    };

    (StatusCode::OK, Json(resp)).into_response()
}

/// Cancels an open FIX order (OrderCancelRequest, 35=F). Enterprise plan required (`POST /fix/cancel`).
#[utoipa::path(
    post,
    path = "/fix/cancel",
    tag = "Trading & Execution",
    request_body = FIXCancelRequest,
    responses(
        (status = 200, description = "Order cancelled successfully and Execution Report (150=4) returned", body = FIXOrderResponse),
        (status = 400, description = "Order already filled/cancelled or invalid cancel request", body = AuthErrorResponse),
        (status = 404, description = "Original order not found", body = AuthErrorResponse),
        (status = 401, description = "Unauthorized", body = AuthErrorResponse),
        (status = 403, description = "Forbidden - Enterprise tier required", body = AuthErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::rate_limit::RateLimitErrorResponse)
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn cancel_fix_order_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    body_bytes: Bytes,
) -> Response {
    if let Err(forbidden_resp) = require_enterprise_role(&claims) {
        return forbidden_resp;
    }
    let body_str = match String::from_utf8(body_bytes.to_vec()) {
        Ok(s) => s,
        Err(_) => {
            let reject_fix = build_reject_message(
                FIX_MSG_ORDER_CANCEL_REJECT,
                "UNKNOWN",
                None,
                "Invalid UTF-8 payload",
            );
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Invalid Payload",
                    "message": "Request body must be valid UTF-8",
                    "fix_message": reject_fix
                })),
            )
                .into_response();
        }
    };

    let fix_raw = if let Ok(req) = serde_json::from_str::<FIXCancelRequest>(&body_str) {
        req.fix_message
    } else {
        body_str.trim().to_string()
    };

    let tags = match parse_fix_message(&fix_raw) {
        Ok(t) => t,
        Err(err) => {
            let reject_fix =
                build_reject_message(FIX_MSG_ORDER_CANCEL_REJECT, "UNKNOWN", None, &err);
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "FIX Parse Error",
                    "message": err,
                    "fix_message": reject_fix
                })),
            )
                .into_response();
        }
    };

    // Validate MsgType (35=F)
    let msg_type = tags
        .get(&FIX_TAG_MSG_TYPE)
        .map(|s| s.as_str())
        .unwrap_or("");
    if msg_type != FIX_MSG_ORDER_CANCEL_REQUEST {
        let reject_fix = build_reject_message(
            FIX_MSG_ORDER_CANCEL_REJECT,
            tags.get(&FIX_TAG_CL_ORD_ID)
                .map(|s| s.as_str())
                .unwrap_or("UNKNOWN"),
            Some(FIX_TAG_MSG_TYPE),
            "Expected MsgType F (OrderCancelRequest)",
        );
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Invalid MsgType",
                "message": format!("Expected 35=F (OrderCancelRequest), got '{}'", msg_type),
                "fix_message": reject_fix
            })),
        )
            .into_response();
    }

    let cancel_cl_ord_id = tags
        .get(&FIX_TAG_CL_ORD_ID)
        .cloned()
        .unwrap_or_else(|| "CANC-REQ".to_string());

    // Extract OrigClOrdID (Tag 41) or fallback to ClOrdID (Tag 11)
    let orig_cl_ord_id = match tags.get(&FIX_TAG_ORIG_CL_ORD_ID) {
        Some(s) if !s.trim().is_empty() => s.trim().to_string(),
        _ => match tags.get(&FIX_TAG_CL_ORD_ID) {
            Some(s) if !s.trim().is_empty() => s.trim().to_string(),
            _ => {
                let reject_fix = build_reject_message(
                    FIX_MSG_ORDER_CANCEL_REJECT,
                    &cancel_cl_ord_id,
                    Some(FIX_TAG_ORIG_CL_ORD_ID),
                    "Missing OrigClOrdID (Tag 41)",
                );
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({
                        "error": "Missing Required Field",
                        "message": "Tag 41 (OrigClOrdID) is required for cancel requests",
                        "fix_message": reject_fix
                    })),
                )
                    .into_response();
            }
        },
    };

    // Find original order for this user
    let existing_order = state
        .fix_order_registry
        .find_user_order(&claims.sub, &orig_cl_ord_id);
    let order = match existing_order {
        Some(o) => o,
        None => {
            let reject_fix = build_reject_message(
                FIX_MSG_ORDER_CANCEL_REJECT,
                &orig_cl_ord_id,
                Some(FIX_TAG_ORIG_CL_ORD_ID),
                "Original order not found",
            );
            return (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "error": "Order Not Found",
                    "message": format!("No active order found with identifier '{}'", orig_cl_ord_id),
                    "fix_message": reject_fix
                })),
            )
                .into_response();
        }
    };

    // Check if order can be cancelled (must be "open")
    if order.status != "open" {
        let reason = format!("Cannot cancel order with status '{}'", order.status);
        let reject_fix = build_reject_message(
            FIX_MSG_ORDER_CANCEL_REJECT,
            &order.cl_ord_id,
            Some(FIX_TAG_ORIG_CL_ORD_ID),
            &reason,
        );
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Order Not Cancellable",
                "message": reason,
                "fix_message": reject_fix
            })),
        )
            .into_response();
    }

    // Update order status to "cancelled"
    let updated_order =
        match state
            .fix_order_registry
            .update_status(&order.id, "cancelled", 0.0, None)
        {
            Some(o) => o,
            None => order.clone(),
        };

    // Persist status change to PostgreSQL if available
    if let Some(pool) = state.db_pool.clone() {
        let order_id = updated_order.id;
        tokio::spawn(async move {
            let res = sqlx::query(
                r#"
                UPDATE fix_orders
                SET status = 'cancelled', updated_at = NOW()
                WHERE id = $1
                "#,
            )
            .bind(order_id)
            .execute(&pool)
            .await;

            if let Err(e) = res {
                warn!(
                    "[FIX Registry] Failed to update order status to cancelled in PostgreSQL: {}",
                    e
                );
            }
        });
    }

    let exec_id = Uuid::new_v4().to_string();
    let org_uuid = claims
        .org_id
        .as_deref()
        .and_then(|s| Uuid::parse_str(s).ok());

    log_audit_event(
        &state,
        org_uuid,
        &claims.sub,
        "fix.order_cancelled",
        "fix_order",
        Some(&updated_order.order_id),
        json!({
            "cl_ord_id": updated_order.cl_ord_id,
            "orig_cl_ord_id": orig_cl_ord_id,
            "symbol": updated_order.symbol,
            "qty": updated_order.qty,
            "exec_id": exec_id,
        }),
        None,
    )
    .await;

    // Generate Execution Report with ExecType=4 (Cancelled) and OrdStatus=4 (Cancelled)
    let fix_response_msg = build_execution_report(
        &updated_order.order_id,
        &exec_id,
        FIX_EXEC_CANCELLED,
        FIX_ORD_STATUS_CANCELLED,
        &updated_order.cl_ord_id,
        &updated_order.symbol,
        &updated_order.side,
        updated_order.qty,
        0.0,
        0.0,
        0.0,
        0.0,
        None,
    );

    let response = FIXOrderResponse {
        order_id: updated_order.order_id,
        exec_id,
        cl_ord_id: updated_order.cl_ord_id,
        symbol: updated_order.symbol,
        side: updated_order.side,
        order_type: updated_order.order_type,
        qty: updated_order.qty,
        filled_qty: 0.0,
        avg_price: None,
        status: "cancelled".to_string(),
        exec_type: FIX_EXEC_CANCELLED.to_string(),
        fix_message: fix_response_msg,
        created_at: updated_order.created_at,
    };

    (StatusCode::OK, Json(response)).into_response()
}
