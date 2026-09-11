//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Streaming Kafka Topic Access Models & DTOs
//! ═══════════════════════════════════════════════════════════════════════════════

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

/// Metadata describing an institutional Kafka streaming topic.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct KafkaTopicInfo {
    /// Kafka topic name (e.g. "sentiment-events")
    #[schema(example = "sentiment-events")]
    pub topic: String,
    /// High-level description of data stream
    #[schema(
        example = "Real-time institutional sentiment scores, categorical signals, and model confidence"
    )]
    pub description: String,
    /// Schema documentation for JSON payloads
    #[schema(
        example = "JSON events with ticker, sentiment_score, sentiment_label, confidence, published_utc"
    )]
    pub schema_description: String,
    /// Example event payload JSON
    #[schema(example = json!({
        "ticker": "AAPL",
        "sentiment_score": 0.85,
        "sentiment_label": "BULLISH",
        "confidence": 0.94,
        "published_utc": "2026-08-31T10:00:00Z"
    }))]
    pub example_payload: serde_json::Value,
    /// Number of configured topic partitions
    #[schema(example = 12)]
    pub partitions: u32,
    /// Message retention duration in hours
    #[schema(example = 168)]
    pub retention_hours: u32,
}

/// Response payload for listing available Kafka streaming topics (`GET /stream/kafka/topics`).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct KafkaTopicsResponse {
    /// Array of available streaming topics
    pub topics: Vec<KafkaTopicInfo>,
    /// Total count of available streaming topics
    #[schema(example = 3)]
    pub total_topics: usize,
}

/// Query parameters for requesting temporary Kafka consumer credentials (`GET /stream/kafka/credentials`).
#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct GetKafkaCredentialsQuery {
    /// Required target topic name to stream (e.g. "sentiment-events", "news-events", "options-events")
    pub topic: String,
    /// Optional credential lifetime in minutes (default: 60, min: 1, max: 1440 = 24h)
    pub ttl_minutes: Option<i64>,
    /// Optional custom consumer group name (defaults to auto-generated user-scoped group)
    pub consumer_group: Option<String>,
}

/// Issued temporary Kafka consumer credentials returned once to the caller.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct KafkaCredentials {
    /// Unique credential identifier UUID
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: Uuid,
    /// User identifier owning these credentials
    #[schema(example = "quant_trader_01")]
    pub user_id: String,
    /// Generated SASL/SCRAM username
    #[schema(example = "user_550e8400e29b41d4a716446655440000")]
    pub username: String,
    /// Generated plaintext password secret (only returned on issuance)
    #[schema(example = "sec_k9x2m4p8q1w7r3t5y8u2i4o6p9a1s3d5")]
    pub password: String,
    /// Kafka cluster bootstrap broker address
    #[schema(example = "127.0.0.1:9092")]
    pub broker_address: String,
    /// Scoped topic allowed for consumption
    #[schema(example = "sentiment-events")]
    pub topic: String,
    /// Scoped consumer group identifier
    #[schema(example = "user-quant_trader_01-sentiment")]
    pub consumer_group: String,
    /// Credential issuance timestamp (UTC)
    pub issued_at: DateTime<Utc>,
    /// Credential expiration timestamp (UTC)
    pub expires_at: DateTime<Utc>,
}

/// Internal stored credential model (with hashed password) for registry & PostgreSQL table.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct StoredKafkaCredential {
    pub id: Uuid,
    pub user_id: String,
    pub topic: String,
    pub consumer_group: String,
    pub username: String,
    pub password_hash: String,
    pub broker_address: String,
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// Response payload for revoking credentials early (`DELETE /stream/kafka/credentials/{id}`).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct RevokeKafkaCredentialsResponse {
    /// Revocation status
    #[schema(example = "revoked")]
    pub status: String,
    /// Confirmation message
    #[schema(example = "Kafka credentials revoked successfully")]
    pub message: String,
    /// Target credential identifier UUID
    pub id: Uuid,
    /// Revocation timestamp (UTC)
    pub revoked_at: DateTime<Utc>,
}
