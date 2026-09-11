use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

/// Query parameters for the Supply Chain Risk Alerts endpoint (`GET /events/supply-chain-risk`).
#[derive(Debug, Clone, Serialize, Deserialize, IntoParams, ToSchema)]
pub struct SupplyChainRiskParams {
    /// Focal company ticker symbol (e.g. "AAPL", "NVDA").
    pub ticker: String,
    /// Graph traversal depth: 1 = direct tier-1 suppliers/customers, 2 = tier-2, 3 = tier-3 (1..=3, default: 1).
    pub depth: Option<u32>,
    /// Lookback window in calendar days for negative corporate events (1..=90, default: 30).
    pub event_days: Option<u32>,
    /// Minimum risk score threshold to filter alerts (0.0..=1.0, default: 0.5).
    pub min_risk_score: Option<f64>,
    /// Maximum number of risk alerts to return (1..=100, default: 20).
    pub limit: Option<usize>,
}

/// A single supply chain risk alert for a related company.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct SupplyChainRiskItem {
    /// Focal company ticker queried (e.g., "AAPL").
    pub focal_ticker: String,
    /// Related supplier or customer company ticker (e.g., "TSM", "ASML").
    pub related_ticker: String,
    /// Relationship type relative to the focal company ("supplier", "customer", "partner").
    pub relationship_type: String,
    /// Graph traversal depth / tier level (1 = direct Tier-1, 2 = Tier-2, 3 = Tier-3).
    pub depth: u32,
    /// Classified negative corporate event category (e.g., "Earnings Warning", "Bankruptcy", "CEO Change", "Regulatory Investigation").
    pub event_type: String,
    /// Description of the corporate event causing supply chain risk.
    pub event_description: String,
    /// Event publication date in ISO YYYY-MM-DD format (e.g., "2026-08-25").
    pub event_date: String,
    /// Quantitative risk propagation score clamped to [0.0, 1.0].
    pub risk_score: f64,
}

/// Response payload containing supply chain risk alerts and graph metrics.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct SupplyChainRiskResponse {
    /// Focal company ticker analyzed.
    pub focal_ticker: String,
    /// Traversal depth level applied.
    pub depth: u32,
    /// Lookback period in days applied.
    pub event_days: u32,
    /// Minimum risk score cutoff threshold.
    pub min_risk_score: f64,
    /// Total count of matching risk alert items returned.
    pub count: usize,
    /// Ranked array of supply chain risk alerts.
    pub alerts: Vec<SupplyChainRiskItem>,
    /// Summary explanation of graph traversal and scoring methodology.
    pub message: String,
}
