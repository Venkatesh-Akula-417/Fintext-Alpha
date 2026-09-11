//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Streaming Kafka Topic Access Engine & Registry
//! ═══════════════════════════════════════════════════════════════════════════════
//!
//! Manages short-lived Kafka/Redpanda consumer credentials, topic schemas,
//! and automated lifecycle cleanup for institutional real-time stream consumers.
//! ═══════════════════════════════════════════════════════════════════════════════

use chrono::{DateTime, Duration as ChronoDuration, Utc};
use dashmap::DashMap;
use rand::Rng;
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinHandle;
use tracing::{info, warn};
use uuid::Uuid;

use crate::models::{KafkaCredentials, KafkaTopicInfo, StoredKafkaCredential};

pub const DEFAULT_KAFKA_BOOTSTRAP_SERVERS: &str = "127.0.0.1:9092";
pub const DEFAULT_KAFKA_TTL_MINUTES: i64 = 60;
pub const MIN_KAFKA_TTL_MINUTES: i64 = 1;
pub const MAX_KAFKA_TTL_MINUTES: i64 = 1440; // 24 hours
pub const DEFAULT_CLEANUP_INTERVAL_SECS: u64 = 600; // 10 minutes

// ─────────────────────────────────────────────────────────────────────────────
// Available Streaming Topics Metadata
// ─────────────────────────────────────────────────────────────────────────────

/// Returns metadata and schemas for all institutional Kafka topics.
pub fn get_available_kafka_topics() -> Vec<KafkaTopicInfo> {
    vec![
        KafkaTopicInfo {
            topic: "sentiment-events".to_string(),
            description: "Real-time institutional asset sentiment scores, classification signals, and model confidence".to_string(),
            schema_description: "JSON events with ticker, sentiment_score, sentiment_label, confidence, published_utc".to_string(),
            example_payload: json!({
                "ticker": "AAPL",
                "sentiment_score": 0.85,
                "sentiment_label": "BULLISH",
                "confidence": 0.94,
                "model_version": "v2.4.0",
                "pipeline_version": "2026.1",
                "data_provenance": "PROPRIETARY_REALTIME_PIPELINE",
                "published_utc": "2026-08-31T10:00:00Z"
            }),
            partitions: 12,
            retention_hours: 168,
        },
        KafkaTopicInfo {
            topic: "news-events".to_string(),
            description: "Curated institutional full-text financial news headlines, source attribution, and snippet vectors".to_string(),
            schema_description: "JSON events with ticker, title, source, url, sentiment_score, published_utc".to_string(),
            example_payload: json!({
                "ticker": "NVDA",
                "title": "Blackwell Hyperscaler GPU Deployments Accelerate Across Enterprise Data Centers",
                "source": "Institutional Wire",
                "url": "https://fintext.io/news/article-789",
                "sentiment_score": 0.92,
                "published_utc": "2026-08-31T09:45:00Z"
            }),
            partitions: 8,
            retention_hours: 168,
        },
        KafkaTopicInfo {
            topic: "options-events".to_string(),
            description: "Real-time Black-Scholes implied volatility (IV), Greeks surface dynamics, and unusual options order flow".to_string(),
            schema_description: "JSON events with ticker, strike, option_type, volume, open_interest, iv, delta, gamma".to_string(),
            example_payload: json!({
                "ticker": "MSFT",
                "strike": 450.0,
                "option_type": "CALL",
                "expiration": "2026-09-18",
                "volume": 14200,
                "open_interest": 45000,
                "iv": 0.284,
                "delta": 0.542,
                "gamma": 0.031,
                "unusual_score": 0.88,
                "timestamp": "2026-08-31T09:59:30Z"
            }),
            partitions: 16,
            retention_hours: 72,
        },
    ]
}

/// Checks whether a given topic string is supported.
pub fn is_valid_kafka_topic(topic: &str) -> bool {
    let clean = topic.trim().to_lowercase();
    get_available_kafka_topics()
        .iter()
        .any(|t| t.topic == clean)
}

/// Hashes a plaintext password string using SHA-256.
pub fn hash_password_sha256(password: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(password.as_bytes());
    hex::encode(hasher.finalize())
}

/// Generates a random alphanumeric secret string for temporary SASL passwords.
pub fn generate_random_secret(len: usize) -> String {
    const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let mut rng = rand::thread_rng();
    let random_part: String = (0..len)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect();
    format!("sec_{}", random_part)
}

// ─────────────────────────────────────────────────────────────────────────────
// Kafka Credentials Registry
// ─────────────────────────────────────────────────────────────────────────────

/// In-memory and PostgreSQL synchronized temporary Kafka credentials manager.
#[derive(Debug, Clone, Default)]
pub struct KafkaCredentialsRegistry {
    entries: Arc<DashMap<Uuid, StoredKafkaCredential>>,
}

impl KafkaCredentialsRegistry {
    /// Creates a new empty `KafkaCredentialsRegistry`.
    pub fn new() -> Self {
        Self {
            entries: Arc::new(DashMap::new()),
        }
    }

    /// Initializes PostgreSQL table schema and indexes.
    pub async fn init_db(pool: &PgPool) -> Result<(), sqlx::Error> {
        let sql = r#"
            CREATE TABLE IF NOT EXISTS kafka_credentials (
                id UUID PRIMARY KEY,
                user_id TEXT NOT NULL,
                topic TEXT NOT NULL,
                consumer_group TEXT NOT NULL,
                username TEXT NOT NULL,
                password_hash TEXT NOT NULL,
                broker_address TEXT NOT NULL,
                issued_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                expires_at TIMESTAMPTZ NOT NULL,
                revoked_at TIMESTAMPTZ NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );
            CREATE INDEX IF NOT EXISTS idx_kafka_creds_user ON kafka_credentials(user_id);
            CREATE INDEX IF NOT EXISTS idx_kafka_creds_expires ON kafka_credentials(expires_at);
        "#;
        sqlx::query(sql).execute(pool).await?;
        info!("[Kafka Streaming] PostgreSQL 'kafka_credentials' table verified and indexed");
        Ok(())
    }

    /// Loads active, non-expired credentials from PostgreSQL.
    pub async fn load_from_db(&self, pool: &PgPool) -> Result<usize, sqlx::Error> {
        let sql = r#"
            SELECT id, user_id, topic, consumer_group, username, password_hash,
                   broker_address, issued_at, expires_at, revoked_at, created_at
            FROM kafka_credentials
            WHERE (revoked_at IS NULL) AND (expires_at > NOW())
        "#;
        let rows = sqlx::query(sql).fetch_all(pool).await?;
        let count = rows.len();

        for row in rows {
            let id: Uuid = row.try_get("id")?;
            let user_id: String = row.try_get("user_id")?;
            let topic: String = row.try_get("topic")?;
            let consumer_group: String = row.try_get("consumer_group")?;
            let username: String = row.try_get("username")?;
            let password_hash: String = row.try_get("password_hash")?;
            let broker_address: String = row.try_get("broker_address")?;
            let issued_at: DateTime<Utc> = row.try_get("issued_at")?;
            let expires_at: DateTime<Utc> = row.try_get("expires_at")?;
            let revoked_at: Option<DateTime<Utc>> = row.try_get("revoked_at")?;
            let created_at: DateTime<Utc> = row.try_get("created_at")?;

            self.entries.insert(
                id,
                StoredKafkaCredential {
                    id,
                    user_id,
                    topic,
                    consumer_group,
                    username,
                    password_hash,
                    broker_address,
                    issued_at,
                    expires_at,
                    revoked_at,
                    created_at,
                },
            );
        }

        info!(
            "[Kafka Streaming] Loaded {} active Kafka credentials into memory",
            count
        );
        Ok(count)
    }

    /// Issues new temporary credentials for a topic and user.
    pub async fn issue_credentials(
        &self,
        user_id: &str,
        topic: &str,
        ttl_minutes: i64,
        custom_consumer_group: Option<String>,
        broker_address: &str,
        pool: Option<&PgPool>,
    ) -> Result<KafkaCredentials, String> {
        let id = Uuid::new_v4();
        let username = format!("user_{}", Uuid::new_v4().simple());
        let plaintext_password = generate_random_secret(28);
        let password_hash = hash_password_sha256(&plaintext_password);

        let clean_user = user_id.replace(|c: char| !c.is_alphanumeric(), "_");
        let consumer_group = custom_consumer_group.unwrap_or_else(|| {
            format!(
                "cg-{}-{}-{}",
                clean_user,
                topic.replace('-', "_"),
                &Uuid::new_v4().simple().to_string()[..6]
            )
        });

        let now = Utc::now();
        let expires_at = now + ChronoDuration::minutes(ttl_minutes);

        let stored = StoredKafkaCredential {
            id,
            user_id: user_id.to_string(),
            topic: topic.to_string(),
            consumer_group: consumer_group.clone(),
            username: username.clone(),
            password_hash,
            broker_address: broker_address.to_string(),
            issued_at: now,
            expires_at,
            revoked_at: None,
            created_at: now,
        };

        // 1. Insert into memory
        self.entries.insert(id, stored.clone());

        // 2. Persist to DB
        if let Some(p) = pool {
            let sql = r#"
                INSERT INTO kafka_credentials
                    (id, user_id, topic, consumer_group, username, password_hash, broker_address, issued_at, expires_at, revoked_at, created_at)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, NULL, $10)
            "#;
            if let Err(e) = sqlx::query(sql)
                .bind(stored.id)
                .bind(&stored.user_id)
                .bind(&stored.topic)
                .bind(&stored.consumer_group)
                .bind(&stored.username)
                .bind(&stored.password_hash)
                .bind(&stored.broker_address)
                .bind(stored.issued_at)
                .bind(stored.expires_at)
                .bind(stored.created_at)
                .execute(p)
                .await
            {
                warn!(
                    "[Kafka Streaming] Failed to persist credentials to DB: {}",
                    e
                );
            }
        }

        info!(
            "[Kafka Streaming] Issued credentials id='{}' user='{}' topic='{}' ttl={}m expires='{}'",
            id, user_id, topic, ttl_minutes, expires_at
        );

        Ok(KafkaCredentials {
            id,
            user_id: user_id.to_string(),
            username,
            password: plaintext_password,
            broker_address: broker_address.to_string(),
            topic: topic.to_string(),
            consumer_group,
            issued_at: now,
            expires_at,
        })
    }

    /// Revokes a credential early.
    pub async fn revoke_credentials(
        &self,
        id: Uuid,
        user_id: &str,
        pool: Option<&PgPool>,
    ) -> Result<Option<StoredKafkaCredential>, String> {
        let mut entry = match self.entries.get_mut(&id) {
            Some(e) => e,
            None => return Ok(None),
        };

        if entry.user_id != user_id {
            return Err("Unauthorized".to_string());
        }

        if entry.revoked_at.is_some() {
            return Ok(Some(entry.clone()));
        }

        let now = Utc::now();
        entry.revoked_at = Some(now);
        let updated = entry.clone();

        if let Some(p) = pool {
            let sql = "UPDATE kafka_credentials SET revoked_at = $1 WHERE id = $2";
            if let Err(e) = sqlx::query(sql).bind(now).bind(id).execute(p).await {
                warn!(
                    "[Kafka Streaming] Failed to mark credential revoked in DB: {}",
                    e
                );
            }
        }

        info!(
            "[Kafka Streaming] User '{}' revoked credentials id='{}'",
            user_id, id
        );
        Ok(Some(updated))
    }

    /// Retrieves an active credential by ID.
    pub fn get_by_id(&self, id: &Uuid) -> Option<StoredKafkaCredential> {
        self.entries.get(id).map(|r| r.value().clone())
    }

    /// Returns all credentials for a given user.
    pub fn list_by_user(&self, user_id: &str) -> Vec<StoredKafkaCredential> {
        self.entries
            .iter()
            .filter(|r| r.value().user_id == user_id)
            .map(|r| r.value().clone())
            .collect()
    }

    /// Total credentials in cache.
    pub fn count(&self) -> usize {
        self.entries.len()
    }

    /// Finds expired non-revoked credentials, marks them revoked, and purges them if necessary.
    pub async fn cleanup_expired(&self, pool: Option<&PgPool>) -> usize {
        let now = Utc::now();
        let mut expired_count = 0;

        for mut item in self.entries.iter_mut() {
            if item.revoked_at.is_none() && item.expires_at <= now {
                item.revoked_at = Some(now);
                expired_count += 1;
            }
        }

        if expired_count > 0 {
            info!(
                "[Kafka Streaming Cleanup] Marked {} expired credentials as revoked in cache",
                expired_count
            );

            if let Some(p) = pool {
                let sql = "UPDATE kafka_credentials SET revoked_at = NOW() WHERE revoked_at IS NULL AND expires_at <= NOW()";
                if let Err(e) = sqlx::query(sql).execute(p).await {
                    warn!(
                        "[Kafka Streaming Cleanup] Failed to update expired credentials in DB: {}",
                        e
                    );
                }
            }
        }

        expired_count
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Background Cleanup Worker
// ─────────────────────────────────────────────────────────────────────────────

/// Spawns the periodic Kafka Credential Expiration Cleanup Tokio task.
pub fn spawn_kafka_credential_cleanup_worker(
    registry: Arc<KafkaCredentialsRegistry>,
    pool: Option<PgPool>,
    interval_secs: u64,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        info!(
            "[Kafka Cleanup Worker] Started background cleanup worker (Interval: {}s)",
            interval_secs
        );

        let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
        interval.tick().await; // skip immediate first tick

        loop {
            interval.tick().await;
            registry.cleanup_expired(pool.as_ref()).await;
        }
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_available_topics() {
        let topics = get_available_kafka_topics();
        assert_eq!(topics.len(), 3);
        assert!(is_valid_kafka_topic("sentiment-events"));
        assert!(is_valid_kafka_topic("news-events"));
        assert!(is_valid_kafka_topic("options-events"));
        assert!(!is_valid_kafka_topic("invalid-topic"));
    }

    #[test]
    fn test_hash_password_sha256() {
        let secret = "test_secret_123";
        let hash1 = hash_password_sha256(secret);
        let hash2 = hash_password_sha256(secret);
        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 64);
    }

    #[tokio::test]
    async fn test_kafka_credentials_registry_lifecycle() {
        let registry = KafkaCredentialsRegistry::new();
        assert_eq!(registry.count(), 0);

        // Issue credentials
        let creds = registry
            .issue_credentials(
                "quant_trader_01",
                "sentiment-events",
                60,
                Some("custom-cg-01".to_string()),
                "127.0.0.1:9092",
                None,
            )
            .await
            .unwrap();

        assert_eq!(creds.user_id, "quant_trader_01");
        assert_eq!(creds.topic, "sentiment-events");
        assert_eq!(creds.consumer_group, "custom-cg-01");
        assert!(creds.password.starts_with("sec_"));
        assert_eq!(registry.count(), 1);

        // Get by ID
        let stored = registry.get_by_id(&creds.id).unwrap();
        assert_eq!(stored.username, creds.username);
        assert_eq!(stored.password_hash, hash_password_sha256(&creds.password));
        assert!(stored.revoked_at.is_none());

        // Revoke credentials
        let revoked = registry
            .revoke_credentials(creds.id, "quant_trader_01", None)
            .await
            .unwrap();
        assert!(revoked.is_some());
        assert!(revoked.unwrap().revoked_at.is_some());

        // Unauthorized revocation attempt
        let err = registry
            .revoke_credentials(creds.id, "other_user", None)
            .await;
        assert!(err.is_err());
    }

    #[tokio::test]
    async fn test_cleanup_expired_credentials() {
        let registry = KafkaCredentialsRegistry::new();
        let creds = registry
            .issue_credentials(
                "expiring_user",
                "news-events",
                -5, // expired 5 minutes ago
                None,
                "127.0.0.1:9092",
                None,
            )
            .await
            .unwrap();

        assert!(registry.get_by_id(&creds.id).unwrap().revoked_at.is_none());
        let cleaned = registry.cleanup_expired(None).await;
        assert_eq!(cleaned, 1);
        assert!(registry.get_by_id(&creds.id).unwrap().revoked_at.is_some());
    }
}
