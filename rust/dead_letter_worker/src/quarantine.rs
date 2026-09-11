//! Quarantine management for unrecoverable DLQ messages.
//! Handles S3 bucket storage with automatic local filesystem fallback and structured audit logging.

use chrono::{DateTime, Datelike, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::{error, warn};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuarantinedEnvelope {
    pub quarantine_id: String,
    pub timestamp_utc: String,
    pub attempts_exhausted: usize,
    pub failure_reason: String,
    pub original_payload: serde_json::Value,
}

/// Helper to generate partitioned S3 keys: `dlq/YYYY/MM/DD/message-<ts_ms>-<uuid>.json`
pub fn generate_s3_key(prefix: &str, timestamp: DateTime<Utc>, id: &str) -> String {
    let clean_prefix = if prefix.is_empty() || prefix.ends_with('/') {
        prefix.to_string()
    } else {
        format!("{}/", prefix)
    };

    format!(
        "{}{:04}/{:02}/{:02}/message-{}-{}.json",
        clean_prefix,
        timestamp.year(),
        timestamp.month(),
        timestamp.day(),
        timestamp.timestamp_millis(),
        id
    )
}

/// Helper to generate local quarantine file paths: `data/quarantine/message-<ts_ms>-<uuid>.json`
pub fn generate_local_path(dir: &str, timestamp: DateTime<Utc>, id: &str) -> PathBuf {
    let clean_dir = Path::new(dir);
    clean_dir.join(format!(
        "message-{}-{}.json",
        timestamp.timestamp_millis(),
        id
    ))
}

pub struct QuarantineManager {
    s3_client: Option<aws_sdk_s3::Client>,
    s3_bucket: String,
    s3_prefix: String,
    local_dir: String,
    pub mock_mode: bool,
}

impl QuarantineManager {
    pub async fn new(
        s3_bucket: String,
        s3_prefix: String,
        local_dir: String,
        mock_mode: bool,
    ) -> Self {
        let s3_client = if mock_mode {
            None
        } else {
            // Attempt to load AWS configuration from environment
            let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
            Some(aws_sdk_s3::Client::new(&config))
        };

        Self {
            s3_client,
            s3_bucket,
            s3_prefix,
            local_dir,
            mock_mode,
        }
    }

    /// Quarantine a permanently failed payload by uploading to S3 or writing to local fallback.
    pub async fn quarantine(
        &self,
        raw_payload: &str,
        reason: &str,
        attempts: usize,
    ) -> Result<String, String> {
        let now = Utc::now();
        let unique_id = Uuid::new_v4().to_string();

        let parsed_json: serde_json::Value = serde_json::from_str(raw_payload)
            .unwrap_or_else(|_| serde_json::json!({ "raw_text": raw_payload }));

        let envelope = QuarantinedEnvelope {
            quarantine_id: unique_id.clone(),
            timestamp_utc: now.to_rfc3339(),
            attempts_exhausted: attempts,
            failure_reason: reason.to_string(),
            original_payload: parsed_json,
        };

        let serialized = serde_json::to_string_pretty(&envelope)
            .map_err(|e| format!("Failed to serialize quarantine envelope: {}", e))?;

        // 1. Try S3 upload if client is available
        if let Some(ref client) = self.s3_client {
            let key = generate_s3_key(&self.s3_prefix, now, &unique_id);
            let body = aws_sdk_s3::primitives::ByteStream::from(serialized.as_bytes().to_vec());

            match client
                .put_object()
                .bucket(&self.s3_bucket)
                .key(&key)
                .content_type("application/json")
                .body(body)
                .send()
                .await
            {
                Ok(_) => {
                    error!(
                        quarantine_id = %unique_id,
                        bucket = %self.s3_bucket,
                        s3_key = %key,
                        attempts = attempts,
                        reason = %reason,
                        "CRITICAL: DLQ message exhausted max retries and was quarantined to S3."
                    );
                    return Ok(format!("s3://{}/{}", self.s3_bucket, key));
                }
                Err(e) => {
                    warn!(
                        error = %e,
                        "S3 quarantine upload failed. Falling back to local disk storage."
                    );
                }
            }
        }

        // 2. Local disk fallback
        self.write_local_quarantine(&serialized, now, &unique_id, reason, attempts)
            .await
    }

    async fn write_local_quarantine(
        &self,
        content: &str,
        timestamp: DateTime<Utc>,
        id: &str,
        reason: &str,
        attempts: usize,
    ) -> Result<String, String> {
        let path = generate_local_path(&self.local_dir, timestamp, id);

        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| format!("Failed to create local quarantine dir: {}", e))?;
        }

        tokio::fs::write(&path, content)
            .await
            .map_err(|e| format!("Failed to write local quarantine file: {}", e))?;

        let path_str = path.to_string_lossy().to_string();
        error!(
            quarantine_id = %id,
            local_path = %path_str,
            attempts = attempts,
            reason = %reason,
            "CRITICAL: DLQ message exhausted max retries and was written to local quarantine fallback."
        );

        Ok(path_str)
    }
}
