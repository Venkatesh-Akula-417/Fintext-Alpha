//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Email Digest Models & DTOs
//! ═══════════════════════════════════════════════════════════════════════════════

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// Stored email digest subscription configuration.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct DigestSubscription {
    /// Unique subscription identifier UUID
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: Uuid,
    /// Owner user ID (or username / email identifier)
    #[schema(example = "quant_trader_01")]
    pub user_id: String,
    /// Delivery frequency: "daily" or "weekly"
    #[schema(example = "daily")]
    pub frequency: String,
    /// Subscribed ticker symbols (e.g. ["AAPL", "MSFT", "NVDA"])
    #[schema(example = json!(["AAPL", "MSFT", "NVDA"]))]
    pub tickers: Vec<String>,
    /// Subscribed GICS sector names (e.g. ["Technology", "Financials"])
    #[schema(example = json!(["Technology", "Financials"]))]
    pub sectors: Vec<String>,
    /// Subscribed catalyst event categories (e.g. ["earnings", "insider", "ma", "8k", "news"])
    #[schema(example = json!(["earnings", "insider", "8k", "news"]))]
    pub event_types: Vec<String>,
    /// Whether the subscription is actively delivering emails
    #[schema(example = true)]
    pub is_active: bool,
    /// Creation timestamp (UTC)
    pub created_at: DateTime<Utc>,
    /// Last update timestamp (UTC)
    pub updated_at: DateTime<Utc>,
}

/// Request payload for creating or updating an email digest subscription (`POST /digest/subscription`).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateDigestRequest {
    /// Delivery frequency: "daily" (default) or "weekly"
    #[serde(default = "default_frequency")]
    #[schema(example = "daily")]
    pub frequency: String,
    /// List of ticker symbols to track in the digest
    #[serde(default)]
    #[schema(example = json!(["AAPL", "MSFT", "NVDA"]))]
    pub tickers: Vec<String>,
    /// List of GICS sector names to track in the digest
    #[serde(default)]
    #[schema(example = json!(["Technology"]))]
    pub sectors: Vec<String>,
    /// List of catalyst event categories to include (e.g. "earnings", "insider", "ma", "8k", "news", "sentiment")
    #[serde(default = "default_event_types")]
    #[schema(example = json!(["earnings", "insider", "8k", "news"]))]
    pub event_types: Vec<String>,
    /// Whether the subscription is active (default: true)
    #[serde(default = "default_is_active")]
    #[schema(example = true)]
    pub is_active: bool,
}

fn default_frequency() -> String {
    "daily".to_string()
}

fn default_event_types() -> Vec<String> {
    vec![
        "earnings".to_string(),
        "insider".to_string(),
        "8k".to_string(),
        "news".to_string(),
    ]
}

fn default_is_active() -> bool {
    true
}

/// Response payload for retrieving a digest subscription (`GET /digest/subscription`).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct DigestSubscriptionResponse {
    /// Status code indicator
    #[schema(example = "ok")]
    pub status: String,
    /// Human-readable status message
    #[schema(example = "Email digest subscription retrieved successfully")]
    pub message: String,
    /// Active subscription configuration
    pub subscription: DigestSubscription,
}

/// Response payload for deleting/deactivating a subscription (`DELETE /digest/subscription`).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct DeleteDigestResponse {
    /// Status code indicator
    #[schema(example = "deleted")]
    pub status: String,
    /// Confirmation message
    #[schema(example = "Email digest subscription deleted successfully")]
    pub message: String,
}

/// Request payload for manually triggering digest compilation & delivery (`POST /digest/trigger`).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Default)]
pub struct TriggerDigestRequest {
    /// Optional override recipient email address (defaults to user's registered email)
    #[schema(example = "quant.trader@fund.com")]
    pub recipient_email: Option<String>,
    /// Preferred format: "html" or "text" (default: "html")
    #[schema(example = "html")]
    pub format: Option<String>,
}

/// Summary counts of items included in the generated email digest.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Default)]
pub struct DigestItemCounts {
    /// Count of ticker/sector sentiment summaries included
    #[schema(example = 3)]
    pub sentiment_count: usize,
    /// Count of corporate catalyst events (8-K, insider trades, M&A) included
    #[schema(example = 4)]
    pub events_count: usize,
    /// Count of news articles included
    #[schema(example = 5)]
    pub news_count: usize,
}

/// Response payload from triggering an email digest (`POST /digest/trigger`).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct TriggerDigestResponse {
    /// Dispatch status: "sent" or "mock_dispatched"
    #[schema(example = "sent")]
    pub status: String,
    /// Target recipient email address
    #[schema(example = "quant.trader@fund.com")]
    pub recipient: String,
    /// Email subject line
    #[schema(example = "FinText Alpha Daily Market Digest — 2026-08-31")]
    pub subject: String,
    /// HTML formatted digest body preview
    pub preview_html: String,
    /// Plain-text formatted digest body preview
    pub preview_text: String,
    /// Breakdown of summary metrics included in the digest
    pub item_counts: DigestItemCounts,
    /// Dispatch timestamp (UTC)
    pub sent_at: DateTime<Utc>,
}
