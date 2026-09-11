//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Chat Alert Bot Subscription Models (Telegram/Discord)
//! ═══════════════════════════════════════════════════════════════════════════════

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// Supported chat notification channels.
pub const CHANNEL_TYPE_TELEGRAM: &str = "telegram";
pub const CHANNEL_TYPE_DISCORD: &str = "discord";

/// Allowed event types for chat alert subscriptions.
pub const ALLOWED_EVENT_TYPES: &[&str] = &["sentiment_anomaly", "8k_filing", "unusual_options"];

/// Stored Telegram / Discord chat alert subscription model.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct ChatAlertSubscription {
    /// Unique Subscription UUID
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: Uuid,
    /// Owner user identifier
    #[schema(example = "quant_fund_01")]
    pub user_id: String,
    /// Channel type ('telegram' or 'discord')
    #[schema(example = "telegram")]
    pub channel_type: String,
    /// Channel target (Telegram chat ID e.g. "123456789" or Discord HTTPS Webhook URL)
    #[schema(example = "123456789")]
    pub channel_target: String,
    /// Subscribed event types (e.g. ["sentiment_anomaly", "8k_filing", "unusual_options"])
    #[schema(example = json!(["sentiment_anomaly", "8k_filing"]))]
    pub event_types: Vec<String>,
    /// Whether this subscription is currently active
    #[schema(example = true)]
    pub is_active: bool,
    /// UTC timestamp of subscription creation
    pub created_at: DateTime<Utc>,
}

/// Request payload for creating a chat alert subscription (`POST /chat-alerts`).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateChatAlertRequest {
    /// Target channel type ('telegram' or 'discord')
    #[schema(example = "telegram")]
    pub channel_type: String,
    /// Channel target: numeric chat ID or @channel for Telegram, or HTTPS webhook URL for Discord
    #[schema(example = "123456789")]
    pub channel_target: String,
    /// List of market event types to subscribe to
    #[schema(example = json!(["sentiment_anomaly", "8k_filing"]))]
    pub event_types: Vec<String>,
}

/// Response payload upon creating or fetching a single chat alert subscription.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct ChatAlertSubscriptionResponse {
    /// Unique Subscription UUID
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: Uuid,
    /// Owner user identifier
    #[schema(example = "quant_fund_01")]
    pub user_id: String,
    /// Channel type ('telegram' or 'discord')
    #[schema(example = "telegram")]
    pub channel_type: String,
    /// Channel target (Telegram chat ID or Discord Webhook URL)
    #[schema(example = "123456789")]
    pub channel_target: String,
    /// Subscribed event types
    pub event_types: Vec<String>,
    /// Whether subscription is active
    #[schema(example = true)]
    pub is_active: bool,
    /// Creation timestamp (UTC)
    pub created_at: DateTime<Utc>,
}

impl From<ChatAlertSubscription> for ChatAlertSubscriptionResponse {
    fn from(sub: ChatAlertSubscription) -> Self {
        Self {
            id: sub.id,
            user_id: sub.user_id,
            channel_type: sub.channel_type,
            channel_target: sub.channel_target,
            event_types: sub.event_types,
            is_active: sub.is_active,
            created_at: sub.created_at,
        }
    }
}

/// Response payload for listing user's chat alert subscriptions (`GET /chat-alerts`).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct ChatAlertsResponse {
    /// List of configured chat alert subscriptions
    pub subscriptions: Vec<ChatAlertSubscriptionResponse>,
    /// Total count of subscriptions
    #[schema(example = 2)]
    pub total: usize,
}

/// Response payload upon deleting a chat alert subscription (`DELETE /chat-alerts/{id}`).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct DeleteChatAlertResponse {
    /// Whether deletion was successful
    #[schema(example = true)]
    pub success: bool,
    /// Deleted subscription UUID
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: Uuid,
    /// Confirmation message
    #[schema(example = "Chat alert subscription deleted successfully")]
    pub message: String,
}
