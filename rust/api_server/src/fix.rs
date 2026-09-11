//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — FIX Protocol Engine & Mock Execution Registry
//! ═══════════════════════════════════════════════════════════════════════════════
//!
//! Implements a FIX 4.4 parser, deterministic mock execution pricing engine,
//! simulated order lifecycle management (New -> Filled / Cancelled / Rejected),
//! and high-concurrency DashMap + PostgreSQL backed registry.

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use tracing::info;
use uuid::Uuid;

use crate::models::fix::FixOrderItem;

// ─────────────────────────────────────────────────────────────────────────────
// FIX Protocol Constants & Tag Numbers
// ─────────────────────────────────────────────────────────────────────────────

pub const FIX_TAG_BEGIN_STRING: i32 = 8;
pub const FIX_TAG_BODY_LENGTH: i32 = 9;
pub const FIX_TAG_CHECKSUM: i32 = 10;
pub const FIX_TAG_MSG_TYPE: i32 = 35;
pub const FIX_TAG_CL_ORD_ID: i32 = 11;
pub const FIX_TAG_ORIG_CL_ORD_ID: i32 = 41;
pub const FIX_TAG_ACCOUNT: i32 = 1;
pub const FIX_TAG_ORDER_ID: i32 = 37;
pub const FIX_TAG_EXEC_ID: i32 = 17;
pub const FIX_TAG_EXEC_TYPE: i32 = 150;
pub const FIX_TAG_ORD_STATUS: i32 = 39;
pub const FIX_TAG_SYMBOL: i32 = 55;
pub const FIX_TAG_SIDE: i32 = 54;
pub const FIX_TAG_ORDER_QTY: i32 = 38;
pub const FIX_TAG_ORD_TYPE: i32 = 40;
pub const FIX_TAG_PRICE: i32 = 44;
pub const FIX_TAG_AVG_PX: i32 = 6;
pub const FIX_TAG_LAST_PX: i32 = 31;
pub const FIX_TAG_LAST_QTY: i32 = 32;
pub const FIX_TAG_CUM_QTY: i32 = 14;
pub const FIX_TAG_LEAVES_QTY: i32 = 151;
pub const FIX_TAG_TEXT: i32 = 58;
pub const FIX_TAG_REF_TAG_ID: i32 = 45;

// Message Types (35)
pub const FIX_MSG_NEW_ORDER_SINGLE: &str = "D";
pub const FIX_MSG_ORDER_CANCEL_REQUEST: &str = "F";
pub const FIX_MSG_EXECUTION_REPORT: &str = "8";
pub const FIX_MSG_REJECT: &str = "3";
pub const FIX_MSG_ORDER_CANCEL_REJECT: &str = "9";

// ExecType (150) & OrdStatus (39)
pub const FIX_EXEC_NEW: &str = "0";
pub const FIX_EXEC_PARTIAL_FILL: &str = "1";
pub const FIX_EXEC_FILL: &str = "2";
pub const FIX_EXEC_CANCELLED: &str = "4";
pub const FIX_EXEC_REJECTED: &str = "8";

pub const FIX_ORD_STATUS_NEW: &str = "0";
pub const FIX_ORD_STATUS_PARTIALLY_FILLED: &str = "1";
pub const FIX_ORD_STATUS_FILLED: &str = "2";
pub const FIX_ORD_STATUS_CANCELLED: &str = "4";
pub const FIX_ORD_STATUS_REJECTED: &str = "8";

// Side (54)
pub const FIX_SIDE_BUY: &str = "1";
pub const FIX_SIDE_SELL: &str = "2";

// OrdType (40)
pub const FIX_ORD_TYPE_MARKET: &str = "1";
pub const FIX_ORD_TYPE_LIMIT: &str = "2";

// ─────────────────────────────────────────────────────────────────────────────
// Core Internal FixOrder Model
// ─────────────────────────────────────────────────────────────────────────────

/// Internal state representation of an order in the FIX registry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FixOrder {
    pub id: Uuid,
    pub user_id: String,
    pub cl_ord_id: String,
    pub order_id: String,
    pub symbol: String,
    pub side: String,
    pub order_type: String,
    pub price: Option<f64>,
    pub qty: f64,
    pub filled_qty: f64,
    pub avg_price: Option<f64>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl FixOrder {
    pub fn to_summary(&self) -> FixOrderItem {
        FixOrderItem {
            order_id: self.order_id.clone(),
            cl_ord_id: self.cl_ord_id.clone(),
            symbol: self.symbol.clone(),
            side: self.side.clone(),
            order_type: self.order_type.clone(),
            qty: self.qty,
            filled_qty: self.filled_qty,
            avg_price: self.avg_price,
            status: self.status.clone(),
            created_at: self.created_at,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Deterministic Mock Pricing
// ─────────────────────────────────────────────────────────────────────────────

/// Computes a deterministic mock asset price for a given ticker symbol.
///
/// Formula: `price = 50.0 + (hash(symbol.to_uppercase()) % 5000) / 100.0`
/// Yields a consistent deterministic price in the range `[50.00, 99.99]`.
pub fn compute_mock_price(symbol: &str) -> f64 {
    let clean_symbol = symbol.trim().to_uppercase();
    let mut hasher = DefaultHasher::new();
    clean_symbol.hash(&mut hasher);
    let h = hasher.finish();
    let offset = (h % 5000) as f64 / 100.0;
    let price = 50.0 + offset;
    (price * 100.0).round() / 100.0
}

// ─────────────────────────────────────────────────────────────────────────────
// FIX Message Parsing & Serializing Helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Parses a pipe-delimited or SOH-delimited FIX 4.4 string into a `HashMap<i32, String>`.
pub fn parse_fix_message(raw: &str) -> Result<HashMap<i32, String>, String> {
    let raw_trimmed = raw.trim();
    if raw_trimmed.is_empty() {
        return Err("FIX message is empty".to_string());
    }

    let mut tags = HashMap::new();

    // Support both pipe `|` and ASCII SOH `\x01` delimiters
    let tokens: Vec<&str> = if raw_trimmed.contains('\x01') {
        raw_trimmed.split('\x01').collect()
    } else {
        raw_trimmed.split('|').collect()
    };

    for token in tokens {
        let t = token.trim();
        if t.is_empty() {
            continue;
        }

        if let Some((tag_str, val_str)) = t.split_once('=') {
            let tag_num: i32 = tag_str
                .trim()
                .parse()
                .map_err(|_| format!("Invalid FIX tag number '{}'", tag_str))?;
            tags.insert(tag_num, val_str.trim().to_string());
        } else {
            return Err(format!("Malformed FIX field token '{}'", t));
        }
    }

    if tags.is_empty() {
        return Err("No valid FIX tag-value pairs found".to_string());
    }

    Ok(tags)
}

/// Builds a standard FIX 4.4 Execution Report (35=8) pipe-delimited message string.
pub fn build_execution_report(
    order_id: &str,
    exec_id: &str,
    exec_type: &str,
    ord_status: &str,
    cl_ord_id: &str,
    symbol: &str,
    side: &str,
    order_qty: f64,
    last_qty: f64,
    last_px: f64,
    cum_qty: f64,
    leaves_qty: f64,
    avg_px: Option<f64>,
) -> String {
    let avg_px_val = avg_px.unwrap_or(0.0);
    let body = format!(
        "35=8|37={}|17={}|150={}|39={}|11={}|55={}|54={}|38={:.2}|32={:.2}|31={:.2}|14={:.2}|151={:.2}|6={:.2}|",
        order_id,
        exec_id,
        exec_type,
        ord_status,
        cl_ord_id,
        symbol,
        side,
        order_qty,
        last_qty,
        last_px,
        cum_qty,
        leaves_qty,
        avg_px_val
    );

    let body_len = body.len();
    format!("8=FIX.4.4|9={}|{}10=000|", body_len, body)
}

/// Builds a standard FIX 4.4 Reject (35=3) or OrderCancelReject (35=9) pipe-delimited message string.
pub fn build_reject_message(
    msg_type: &str,
    cl_ord_id: &str,
    ref_tag: Option<i32>,
    reason: &str,
) -> String {
    let mut body = format!("35={}|11={}|58={}|", msg_type, cl_ord_id, reason);
    if let Some(tag) = ref_tag {
        body = format!("35={}|11={}|45={}|58={}|", msg_type, cl_ord_id, tag, reason);
    }
    let body_len = body.len();
    format!("8=FIX.4.4|9={}|{}10=000|", body_len, body)
}

// ─────────────────────────────────────────────────────────────────────────────
// FIX Order Registry (In-Memory + Database)
// ─────────────────────────────────────────────────────────────────────────────

/// High-performance concurrent registry for FIX orders backed by PostgreSQL.
#[derive(Debug, Clone)]
pub struct FixOrderRegistry {
    orders: Arc<DashMap<Uuid, FixOrder>>,
}

impl Default for FixOrderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl FixOrderRegistry {
    /// Creates a new, empty in-memory FIX order registry.
    pub fn new() -> Self {
        Self {
            orders: Arc::new(DashMap::new()),
        }
    }

    /// Appends or updates an order in the registry.
    pub fn insert(&self, order: FixOrder) {
        self.orders.insert(order.id, order);
    }

    /// Retrieves an order by its primary UUID.
    pub fn get_by_id(&self, id: &Uuid) -> Option<FixOrder> {
        self.orders.get(id).map(|r| r.value().clone())
    }

    /// Finds an order by client order ID (`cl_ord_id`) for a specific user.
    pub fn find_by_cl_ord_id(&self, user_id: &str, cl_ord_id: &str) -> Option<FixOrder> {
        self.orders
            .iter()
            .find(|r| r.user_id == user_id && r.cl_ord_id == cl_ord_id)
            .map(|r| r.value().clone())
    }

    /// Finds an order by either `order_id` or `cl_ord_id` for a specific user.
    pub fn find_user_order(&self, user_id: &str, identifier: &str) -> Option<FixOrder> {
        self.orders
            .iter()
            .find(|r| {
                r.user_id == user_id && (r.cl_ord_id == identifier || r.order_id == identifier)
            })
            .map(|r| r.value().clone())
    }

    /// Updates order status, filled quantity, and average execution price.
    pub fn update_status(
        &self,
        id: &Uuid,
        status: &str,
        filled_qty: f64,
        avg_price: Option<f64>,
    ) -> Option<FixOrder> {
        if let Some(mut entry) = self.orders.get_mut(id) {
            entry.status = status.to_string();
            entry.filled_qty = filled_qty;
            entry.avg_price = avg_price;
            entry.updated_at = Utc::now();
            Some(entry.clone())
        } else {
            None
        }
    }

    /// Queries user orders with status filtering, date ordering, and pagination.
    pub fn query(
        &self,
        user_id: &str,
        status: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> (Vec<FixOrderItem>, usize) {
        let limit = limit.clamp(1, 1000);

        let mut matched: Vec<FixOrder> = self
            .orders
            .iter()
            .filter(|r| {
                if r.user_id != user_id {
                    return false;
                }
                if let Some(s) = status {
                    let s_norm = s.trim().to_lowercase();
                    if !s_norm.is_empty() && r.status.to_lowercase() != s_norm {
                        return false;
                    }
                }
                true
            })
            .map(|r| r.value().clone())
            .collect();

        // Sort descending by created_at (newest first)
        matched.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        let total = matched.len();
        let paginated = matched
            .into_iter()
            .skip(offset)
            .take(limit)
            .map(|o| o.to_summary())
            .collect();

        (paginated, total)
    }

    /// Loads existing orders from PostgreSQL into the in-memory registry.
    pub async fn load_from_db(&self, pool: &PgPool) -> Result<usize, sqlx::Error> {
        use sqlx::Row;
        let rows = sqlx::query(
            "SELECT id, user_id, cl_ord_id, order_id, symbol, side, order_type, qty, filled_qty, avg_price, status, created_at, updated_at FROM fix_orders"
        )
        .fetch_all(pool)
        .await?;

        let count = rows.len();
        for r in rows {
            let order = FixOrder {
                id: r.get("id"),
                user_id: r.get("user_id"),
                cl_ord_id: r.get("cl_ord_id"),
                order_id: r.get("order_id"),
                symbol: r.get("symbol"),
                side: r.get("side"),
                order_type: r.get("order_type"),
                price: None,
                qty: r.get("qty"),
                filled_qty: r.get("filled_qty"),
                avg_price: r.get("avg_price"),
                status: r.get("status"),
                created_at: r.get("created_at"),
                updated_at: r.get("updated_at"),
            };
            self.orders.insert(order.id, order);
        }

        info!("[FIX Registry] Loaded {} FIX orders from PostgreSQL", count);
        Ok(count)
    }

    /// Initializes PostgreSQL table `fix_orders` idempotently.
    pub async fn init_db(pool: &PgPool) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS fix_orders (
                id UUID PRIMARY KEY,
                user_id TEXT NOT NULL,
                cl_ord_id TEXT NOT NULL,
                order_id TEXT NOT NULL,
                symbol TEXT NOT NULL,
                side TEXT NOT NULL,
                order_type TEXT NOT NULL,
                qty DOUBLE PRECISION NOT NULL,
                filled_qty DOUBLE PRECISION NOT NULL DEFAULT 0,
                avg_price DOUBLE PRECISION,
                status TEXT NOT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );
            CREATE INDEX IF NOT EXISTS idx_fix_orders_user_time ON fix_orders(user_id, created_at DESC);
            CREATE INDEX IF NOT EXISTS idx_fix_orders_cl_ord_id ON fix_orders(cl_ord_id);
            CREATE INDEX IF NOT EXISTS idx_fix_orders_order_id ON fix_orders(order_id);
            CREATE INDEX IF NOT EXISTS idx_fix_orders_status ON fix_orders(status);
            "#,
        )
        .execute(pool)
        .await?;

        info!("[FIX Protocol Engine] PostgreSQL 'fix_orders' schema and indexes verified.");
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_fix_message_pipe() {
        let fix_msg = "8=FIX.4.4|9=60|35=D|11=ORD123|55=AAPL|54=1|38=100|40=1|10=000|";
        let tags = parse_fix_message(fix_msg).unwrap();
        assert_eq!(tags.get(&8).unwrap(), "FIX.4.4");
        assert_eq!(tags.get(&35).unwrap(), "D");
        assert_eq!(tags.get(&11).unwrap(), "ORD123");
        assert_eq!(tags.get(&55).unwrap(), "AAPL");
        assert_eq!(tags.get(&54).unwrap(), "1");
        assert_eq!(tags.get(&38).unwrap(), "100");
        assert_eq!(tags.get(&40).unwrap(), "1");
    }

    #[test]
    fn test_parse_fix_message_soh() {
        let fix_msg =
            "8=FIX.4.4\x0135=D\x0111=ORD456\x0155=MSFT\x0154=2\x0138=50\x0140=2\x0144=350.00\x01";
        let tags = parse_fix_message(fix_msg).unwrap();
        assert_eq!(tags.get(&35).unwrap(), "D");
        assert_eq!(tags.get(&11).unwrap(), "ORD456");
        assert_eq!(tags.get(&55).unwrap(), "MSFT");
        assert_eq!(tags.get(&44).unwrap(), "350.00");
    }

    #[test]
    fn test_compute_mock_price_deterministic() {
        let p1 = compute_mock_price("AAPL");
        let p2 = compute_mock_price("AAPL");
        let p3 = compute_mock_price("aapl");
        assert_eq!(p1, p2);
        assert_eq!(p1, p3);
        assert!(p1 >= 50.0 && p1 <= 100.0);

        let p_msft = compute_mock_price("MSFT");
        assert!(p_msft >= 50.0 && p_msft <= 100.0);
    }

    #[test]
    fn test_build_execution_report_and_reject() {
        let exec_rpt = build_execution_report(
            "ORD-001",
            "EXEC-001",
            FIX_EXEC_FILL,
            FIX_ORD_STATUS_FILLED,
            "CL-001",
            "AAPL",
            FIX_SIDE_BUY,
            100.0,
            100.0,
            150.25,
            100.0,
            0.0,
            Some(150.25),
        );
        assert!(exec_rpt.starts_with("8=FIX.4.4|9="));
        assert!(exec_rpt.contains("35=8|"));
        assert!(exec_rpt.contains("37=ORD-001|"));
        assert!(exec_rpt.contains("150=2|39=2|"));
        assert!(exec_rpt.ends_with("10=000|"));

        let reject = build_reject_message(FIX_MSG_REJECT, "CL-001", Some(55), "Symbol missing");
        assert!(reject.contains("35=3|"));
        assert!(reject.contains("45=55|"));
        assert!(reject.contains("58=Symbol missing|"));
    }

    #[test]
    fn test_fix_order_registry_crud_and_query() {
        let registry = FixOrderRegistry::new();
        let user = "quant_trader_1";
        let order = FixOrder {
            id: Uuid::new_v4(),
            user_id: user.to_string(),
            cl_ord_id: "CL-100".to_string(),
            order_id: "ORD-100".to_string(),
            symbol: "NVDA".to_string(),
            side: "1".to_string(),
            order_type: "1".to_string(),
            price: None,
            qty: 200.0,
            filled_qty: 200.0,
            avg_price: Some(85.50),
            status: "filled".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        registry.insert(order.clone());

        assert_eq!(registry.get_by_id(&order.id), Some(order.clone()));
        assert_eq!(
            registry.find_by_cl_ord_id(user, "CL-100"),
            Some(order.clone())
        );

        let (items, total) = registry.query(user, Some("filled"), 10, 0);
        assert_eq!(total, 1);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].symbol, "NVDA");

        let (empty_items, empty_total) = registry.query(user, Some("open"), 10, 0);
        assert_eq!(empty_total, 0);
        assert_eq!(empty_items.len(), 0);
    }
}
