//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — FIX Protocol Models & DTOs
//! ═══════════════════════════════════════════════════════════════════════════════

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

/// Request payload for submitting a FIX NewOrderSingle (35=D) order (`POST /fix/order`).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct FIXOrderRequest {
    /// Raw pipe-delimited (`|`) or SOH-delimited FIX 4.4 message string
    #[schema(example = "8=FIX.4.4|9=60|35=D|11=ORD-2026-001|55=AAPL|54=1|38=100|40=1|10=000|")]
    pub fix_message: String,
}

/// Request payload for submitting a FIX OrderCancelRequest (35=F) (`POST /fix/cancel`).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct FIXCancelRequest {
    /// Raw pipe-delimited (`|`) or SOH-delimited FIX 4.4 cancel message string
    #[schema(
        example = "8=FIX.4.4|9=55|35=F|11=CANC-2026-001|41=ORD-2026-001|55=AAPL|54=1|38=100|10=000|"
    )]
    pub fix_message: String,
}

/// Response payload for FIX order execution reports (35=8) and lifecycle events.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct FIXOrderResponse {
    /// Server-assigned unique Order ID (UUID) (Tag 37)
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub order_id: String,
    /// Server-assigned unique Execution ID (UUID) (Tag 17)
    #[schema(example = "550e8400-e29b-41d4-a716-446655440001")]
    pub exec_id: String,
    /// Client-specified unique order identifier (Tag 11)
    #[schema(example = "ORD-2026-001")]
    pub cl_ord_id: String,
    /// Financial asset symbol (Tag 55)
    #[schema(example = "AAPL")]
    pub symbol: String,
    /// Order side: `1` (Buy) or `2` (Sell) (Tag 54)
    #[schema(example = "1")]
    pub side: String,
    /// Order type: `1` (Market) or `2` (Limit) (Tag 40)
    #[schema(example = "1")]
    pub order_type: String,
    /// Total requested order quantity (Tag 38)
    #[schema(example = 100.0)]
    pub qty: f64,
    /// Cumulative executed/filled quantity (Tag 14)
    #[schema(example = 100.0)]
    pub filled_qty: f64,
    /// Volume-weighted average executed price (Tag 6)
    #[schema(example = 175.50)]
    pub avg_price: Option<f64>,
    /// Order lifecycle status: `open`, `filled`, `cancelled`, or `rejected`
    #[schema(example = "filled")]
    pub status: String,
    /// FIX Execution Type: `0` (New), `1` (Partial Fill), `2` (Fill), `4` (Cancelled), `8` (Rejected) (Tag 150)
    #[schema(example = "2")]
    pub exec_type: String,
    /// Formatted FIX Execution Report (35=8) or Reject (35=3/35=9) protocol string
    #[schema(
        example = "8=FIX.4.4|9=112|35=8|37=550e8400-e29b-41d4-a716-446655440000|17=550e8400-e29b-41d4-a716-446655440001|150=2|39=2|11=ORD-2026-001|55=AAPL|54=1|38=100|32=100|31=175.50|14=100|151=0|6=175.50|10=000|"
    )]
    pub fix_message: String,
    /// Timestamp of order creation / execution report generation
    pub created_at: DateTime<Utc>,
}

/// Order summary item returned in the paginated order listing (`GET /fix/orders`).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct FixOrderItem {
    /// Server-assigned unique Order ID (UUID)
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub order_id: String,
    /// Client-specified order identifier
    #[schema(example = "ORD-2026-001")]
    pub cl_ord_id: String,
    /// Financial asset symbol
    #[schema(example = "AAPL")]
    pub symbol: String,
    /// Order side: `1` (Buy) or `2` (Sell)
    #[schema(example = "1")]
    pub side: String,
    /// Order type: `1` (Market) or `2` (Limit)
    #[schema(example = "1")]
    pub order_type: String,
    /// Total requested quantity
    #[schema(example = 100.0)]
    pub qty: f64,
    /// Cumulative executed quantity
    #[schema(example = 100.0)]
    pub filled_qty: f64,
    /// Volume-weighted average price (if executed)
    #[schema(example = 175.50)]
    pub avg_price: Option<f64>,
    /// Order lifecycle status: `open`, `filled`, `cancelled`, or `rejected`
    #[schema(example = "filled")]
    pub status: String,
    /// Order creation timestamp
    pub created_at: DateTime<Utc>,
}

/// Query parameters for listing user FIX orders (`GET /fix/orders`).
#[derive(Debug, Clone, Deserialize, IntoParams, ToSchema)]
pub struct FIXOrdersQueryParams {
    /// Optional status filter (`open`, `filled`, `cancelled`, `rejected`)
    #[schema(example = "open")]
    pub status: Option<String>,
    /// Maximum number of orders to return (default: 50, max: 1000)
    #[schema(example = 50)]
    pub limit: Option<usize>,
    /// Pagination offset index (default: 0)
    #[schema(example = 0)]
    pub offset: Option<usize>,
}

/// Paginated response payload for listing FIX orders (`GET /fix/orders`).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct FIXOrdersListResponse {
    /// List of order summaries matching the query
    pub orders: Vec<FixOrderItem>,
    /// Total count of matching orders before pagination
    #[schema(example = 42)]
    pub total: usize,
    /// Effective limit parameter applied
    #[schema(example = 50)]
    pub limit: usize,
    /// Effective offset parameter applied
    #[schema(example = 0)]
    pub offset: usize,
}
