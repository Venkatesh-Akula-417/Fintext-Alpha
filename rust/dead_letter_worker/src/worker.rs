//! Main DLQ Worker implementation with Kafka subscription, retry orchestrator, and quarantine routing.

use futures::StreamExt;
use rdkafka::config::ClientConfig;
use rdkafka::consumer::stream_consumer::StreamConsumer;
use rdkafka::consumer::Consumer;
use rdkafka::message::Message;
use rdkafka::producer::{FutureProducer, FutureRecord};
use rdkafka::util::Timeout;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, info, warn};

use crate::config::DlqConfig;
use crate::quarantine::QuarantineManager;
use crate::retry::{execute_with_retry, RetryOutcome};

pub struct DeadLetterWorker {
    pub config: DlqConfig,
    pub quarantine_mgr: Arc<QuarantineManager>,
    pub in_memory_events: Arc<dashmap::DashMap<uuid::Uuid, serde_json::Value>>,
    pub db_pool: Option<sqlx::PgPool>,
    kafka_producer: Option<Arc<FutureProducer>>,
}

impl DeadLetterWorker {
    pub async fn new(config: DlqConfig) -> Result<Self, String> {
        let quarantine_mgr = Arc::new(
            QuarantineManager::new(
                config.s3_quarantine_bucket.clone(),
                config.s3_quarantine_prefix.clone(),
                config.local_quarantine_dir.clone(),
                config.mock_mode,
            )
            .await,
        );

        let in_memory_events = Arc::new(dashmap::DashMap::new());
        let db_pool = None;

        let kafka_producer = if config.mock_mode {
            info!("Running DeadLetterWorker in MOCK mode (skipping external Kafka connection).");
            None
        } else {
            let producer_res: Result<FutureProducer, _> = ClientConfig::new()
                .set("bootstrap.servers", &config.kafka_bootstrap_servers)
                .set("message.timeout.ms", "5000")
                .set("acks", "1")
                .create();

            match producer_res {
                Ok(producer) => {
                    info!(
                        bootstrap_servers = %config.kafka_bootstrap_servers,
                        dlq_topic = %config.dlq_topic,
                        reprocess_topic = %config.reprocess_topic,
                        "Successfully initialized Kafka producer for DLQ worker."
                    );
                    Some(Arc::new(producer))
                }
                Err(e) => {
                    warn!(
                        bootstrap_servers = %config.kafka_bootstrap_servers,
                        error = %e,
                        "Failed initial Kafka producer creation; worker will operate in fallback mode."
                    );
                    None
                }
            }
        };

        Ok(Self {
            config,
            quarantine_mgr,
            in_memory_events,
            db_pool,
            kafka_producer,
        })
    }

    /// Process an individual message payload with exponential backoff and quarantine routing.
    pub async fn handle_message(&self, raw_payload: &str) -> Result<RetryOutcome, String> {
        let config = &self.config;
        let quarantine_mgr = Arc::clone(&self.quarantine_mgr);

        let outcome = execute_with_retry(
            config.max_retries,
            config.initial_backoff_ms,
            config.max_backoff_ms,
            raw_payload,
            |payload, attempt| {
                let producer_opt = self.kafka_producer.clone();
                let reprocess_topic = config.reprocess_topic.clone();
                let payload_owned = payload.to_string();
                async move {
                    Self::process_payload(&payload_owned, attempt, producer_opt, &reprocess_topic)
                        .await
                }
            },
        )
        .await;

        match &outcome {
            RetryOutcome::Success { attempts } => {
                info!(
                    attempts = attempts,
                    "DLQ message successfully recovered and re-routed."
                );
            }
            RetryOutcome::Exhausted {
                attempts,
                last_error,
            } => {
                error!(
                    attempts = attempts,
                    error = %last_error,
                    "Exhausted retries for DLQ message. Commencing quarantine procedure..."
                );
                let dest = quarantine_mgr
                    .quarantine(raw_payload, last_error, *attempts)
                    .await?;
                info!(destination = %dest, "Quarantine completed successfully.");

                // Store in memory registry
                let id = uuid::Uuid::new_v4();
                let parsed_val: serde_json::Value = serde_json::from_str(raw_payload)
                    .unwrap_or_else(|_| serde_json::json!({ "raw": raw_payload }));
                let event_id = parsed_val
                    .get("event_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown_event")
                    .to_string();

                let record = serde_json::json!({
                    "id": id.to_string(),
                    "event_id": event_id,
                    "source": "sentiment",
                    "error_type": "RetriesExhausted",
                    "error_message": last_error,
                    "payload": parsed_val,
                    "retry_count": attempts,
                    "status": "failed",
                    "quarantine_dest": dest
                });
                self.in_memory_events.insert(id, record);

                // If DB pool connected, insert into dlq_events table
                if let Some(ref pool) = self.db_pool {
                    let _ = sqlx::query(
                        r#"
                        INSERT INTO dlq_events (id, event_id, source, error_type, error_message, payload, failed_at, retry_count, status, created_at, updated_at)
                        VALUES ($1, $2, $3, $4, $5, $6, NOW(), $7, 'failed', NOW(), NOW())
                        ON CONFLICT (id) DO NOTHING
                        "#
                    )
                    .bind(id)
                    .bind(&event_id)
                    .bind("sentiment")
                    .bind("RetriesExhausted")
                    .bind(last_error)
                    .bind(&parsed_val)
                    .bind(*attempts as i32)
                    .execute(pool)
                    .await;
                }
            }
        }

        Ok(outcome)
    }

    /// Default reprocessing logic:
    /// - Checks for intentional simulated error flags (`should_fail`, `fail_until_attempt`)
    /// - If successful, publishes recovered payload back to Kafka reprocess topic
    async fn process_payload(
        payload: &str,
        attempt: usize,
        kafka_producer: Option<Arc<FutureProducer>>,
        reprocess_topic: &str,
    ) -> Result<(), String> {
        let parsed: serde_json::Value =
            serde_json::from_str(payload).map_err(|e| format!("Malformed JSON payload: {}", e))?;

        // Support simulated failures for testing
        if let Some(should_fail) = parsed.get("should_fail").and_then(|v| v.as_bool()) {
            if should_fail {
                return Err(
                    "Explicit permanent failure flag ('should_fail: true') detected".to_string(),
                );
            }
        }

        if let Some(fail_until) = parsed.get("fail_until_attempt").and_then(|v| v.as_u64()) {
            if (attempt as u64) <= fail_until {
                return Err(format!(
                    "Simulated transient error on attempt {} (configured to fail until attempt {})",
                    attempt, fail_until
                ));
            }
        }

        // If real Kafka producer is active, publish back to reprocess topic
        if let Some(producer) = kafka_producer {
            let record = FutureRecord::to(reprocess_topic).payload(payload).key("");
            producer
                .send(record, Timeout::After(Duration::from_millis(5000)))
                .await
                .map_err(|(e, _)| {
                    format!(
                        "Failed to republish message to Kafka topic '{}': {}",
                        reprocess_topic, e
                    )
                })?;
        }

        Ok(())
    }

    /// Continuous worker loop listening for DLQ messages from Kafka or mock generator.
    pub async fn run(&mut self) -> Result<(), anyhow::Error> {
        info!(
            dlq_topic = %self.config.dlq_topic,
            reprocess_topic = %self.config.reprocess_topic,
            group_id = %self.config.group_id,
            max_retries = self.config.max_retries,
            mock_mode = self.config.mock_mode,
            "FinText Dead Letter Queue (DLQ) Auto-Reprocessing Worker started."
        );

        if self.config.mock_mode {
            return self.run_mock_loop().await;
        }

        // Production Kafka subscription loop with auto-reconnect
        loop {
            let consumer_res: Result<StreamConsumer, _> = ClientConfig::new()
                .set("bootstrap.servers", &self.config.kafka_bootstrap_servers)
                .set("group.id", &self.config.group_id)
                .set("enable.auto.commit", "true")
                .set("auto.offset.reset", "earliest")
                .set("session.timeout.ms", "6000")
                .create();

            let consumer = match consumer_res {
                Ok(c) => {
                    info!(
                        bootstrap_servers = %self.config.kafka_bootstrap_servers,
                        group_id = %self.config.group_id,
                        "Connected to Kafka broker for DLQ consumption."
                    );
                    c
                }
                Err(e) => {
                    error!(
                        error = %e,
                        "Kafka DLQ consumer creation failed. Retrying in 5 seconds..."
                    );
                    sleep(Duration::from_secs(5)).await;
                    continue;
                }
            };

            if let Err(e) = consumer.subscribe(&[&self.config.dlq_topic]) {
                error!(
                    error = %e,
                    topic = %self.config.dlq_topic,
                    "Failed to subscribe to DLQ topic. Reconnecting in 5s..."
                );
                sleep(Duration::from_secs(5)).await;
                continue;
            }

            info!(
                topic = %self.config.dlq_topic,
                "Listening for DLQ messages from Kafka..."
            );

            let mut message_stream = consumer.stream();
            while let Some(msg_res) = message_stream.next().await {
                match msg_res {
                    Ok(msg) => {
                        if let Some(payload_bytes) = msg.payload() {
                            let payload_str = match std::str::from_utf8(payload_bytes) {
                                Ok(s) => s,
                                Err(e) => {
                                    error!(error = %e, "Received non-UTF8 payload on DLQ topic.");
                                    continue;
                                }
                            };

                            if let Err(e) = self.handle_message(payload_str).await {
                                error!(error = %e, "Unhandled error during DLQ message processing.");
                            }
                        }
                    }
                    Err(e) => {
                        error!(error = %e, "Kafka DLQ stream error encountered.");
                    }
                }
            }

            warn!("Kafka DLQ stream consumption loop terminated. Attempting reconnect in 1s...");
            sleep(Duration::from_secs(1)).await;
        }
    }

    /// Mock execution cycle for offline tests and validation.
    async fn run_mock_loop(&self) -> Result<(), anyhow::Error> {
        info!("Executing Mock DLQ Processing Cycle...");

        // 1. Recoverable message test
        let recoverable_payload = serde_json::json!({
            "event_id": "test-recoverable-101",
            "ticker": "AAPL",
            "sentiment_score": 0.85,
            "fail_until_attempt": 2
        })
        .to_string();

        info!("Sending recoverable test payload (fails on attempts 1-2, succeeds on 3)...");
        let res1 = self.handle_message(&recoverable_payload).await.unwrap();
        assert_eq!(res1, RetryOutcome::Success { attempts: 3 });

        // 2. Unrecoverable message test (quarantine)
        let unrecoverable_payload = serde_json::json!({
            "event_id": "test-unrecoverable-500",
            "ticker": "TSLA",
            "sentiment_score": -0.92,
            "should_fail": true
        })
        .to_string();

        info!("Sending unrecoverable test payload (fails all retries -> quarantine)...");
        let res2 = self.handle_message(&unrecoverable_payload).await.unwrap();
        match res2 {
            RetryOutcome::Exhausted { attempts, .. } => {
                assert_eq!(attempts, self.config.max_retries);
            }
            _ => panic!("Expected Exhausted outcome for unrecoverable payload"),
        }

        info!("Mock DLQ Processing Cycle Completed Successfully. All test invariants verified!");
        Ok(())
    }
}
