//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — PIT Certificate Cryptographic Proof Archival
//! Deterministic JSON Canonicalization, SHA-256 Pre-Image & S3/MinIO/Local Storage
//! ═══════════════════════════════════════════════════════════════════════════════

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;
use tracing::{info, warn};
use uuid::Uuid;

use crate::models::pit::{PITCertificatePolicies, PITCertificateTests};

/// Configuration for PIT Certificate Cryptographic Proof Archival.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PitCertArchiveConfig {
    /// Whether cryptographic proof archival is enabled (default: true).
    pub enabled: bool,
    /// Storage provider: "local", "s3", or "minio" (default: "local").
    pub provider: String,
    /// Target bucket name for S3 / MinIO (default: "fintext-pit-cert-archive").
    pub bucket: String,
    /// Storage key prefix inside bucket (default: "pit-cert").
    pub prefix: String,
    /// AWS region (default: "us-east-1").
    pub region: String,
    /// Custom S3 / MinIO endpoint URL (optional).
    pub endpoint: Option<String>,
    /// Access key ID for S3 / MinIO.
    pub access_key: Option<String>,
    /// Secret access key for S3 / MinIO.
    pub secret_key: Option<String>,
    /// Local staging / storage directory path (default: "data/pit-cert-archive").
    pub local_path: String,
}

impl Default for PitCertArchiveConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            provider: "local".to_string(),
            bucket: "fintext-pit-cert-archive".to_string(),
            prefix: "pit-cert".to_string(),
            region: "us-east-1".to_string(),
            endpoint: None,
            access_key: None,
            secret_key: None,
            local_path: "data/pit-cert-archive".to_string(),
        }
    }
}

impl PitCertArchiveConfig {
    /// Reads configuration with priority: environment variables -> config/config.yaml -> defaults.
    pub fn from_env_or_config() -> Self {
        let mut cfg = Self::default();

        let config_paths = [
            env::var("CONFIG_PATH").unwrap_or_default(),
            "config/config.yaml".to_string(),
            "config.yaml".to_string(),
            "../config/config.yaml".to_string(),
            "../../config/config.yaml".to_string(),
        ];

        for path in &config_paths {
            if path.is_empty() {
                continue;
            }
            if let Ok(contents) = fs::read_to_string(path) {
                let mut in_section = false;

                for line in contents.lines() {
                    let trimmed = line.trim();
                    if trimmed.starts_with('#') || trimmed.is_empty() {
                        continue;
                    }

                    if trimmed.starts_with("pit_certificate_archive:") {
                        in_section = true;
                        continue;
                    }

                    if in_section && !line.starts_with(' ') && !line.starts_with('\t') {
                        in_section = false;
                    }

                    if in_section {
                        if let Some((k, v)) = trimmed.split_once(':') {
                            let key = k.trim().trim_start_matches('-').trim();
                            let val = v.split('#').next().unwrap_or("").trim().trim_matches('"').trim_matches('\'');
                            match key {
                                "enabled" => {
                                    if val == "true" || val == "1" {
                                        cfg.enabled = true;
                                    } else if val == "false" || val == "0" {
                                        cfg.enabled = false;
                                    }
                                }
                                "provider" => {
                                    if !val.is_empty() {
                                        cfg.provider = val.to_string();
                                    }
                                }
                                "bucket" => {
                                    if !val.is_empty() {
                                        cfg.bucket = val.to_string();
                                    }
                                }
                                "prefix" => {
                                    if !val.is_empty() {
                                        cfg.prefix = val.to_string();
                                    }
                                }
                                "region" => {
                                    if !val.is_empty() {
                                        cfg.region = val.to_string();
                                    }
                                }
                                "endpoint" => {
                                    if !val.is_empty() {
                                        cfg.endpoint = Some(val.to_string());
                                    }
                                }
                                "access_key" => {
                                    if !val.is_empty() {
                                        cfg.access_key = Some(val.to_string());
                                    }
                                }
                                "secret_key" => {
                                    if !val.is_empty() {
                                        cfg.secret_key = Some(val.to_string());
                                    }
                                }
                                "local_path" => {
                                    if !val.is_empty() {
                                        cfg.local_path = val.to_string();
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }

        // Environment variable overrides take highest priority
        if let Ok(v) = env::var("PIT_CERT_ARCHIVE_ENABLED") {
            let v_lower = v.trim().to_lowercase();
            cfg.enabled = v_lower == "1" || v_lower == "true" || v_lower == "yes";
        }
        if let Ok(v) = env::var("PIT_CERT_ARCHIVE_PROVIDER") {
            if !v.trim().is_empty() {
                cfg.provider = v.trim().to_string();
            }
        }
        if let Ok(v) = env::var("PIT_CERT_ARCHIVE_BUCKET") {
            if !v.trim().is_empty() {
                cfg.bucket = v.trim().to_string();
            }
        }
        if let Ok(v) = env::var("PIT_CERT_ARCHIVE_PREFIX") {
            if !v.trim().is_empty() {
                cfg.prefix = v.trim().to_string();
            }
        }
        if let Ok(v) = env::var("PIT_CERT_ARCHIVE_REGION") {
            if !v.trim().is_empty() {
                cfg.region = v.trim().to_string();
            }
        }
        if let Ok(v) = env::var("PIT_CERT_ARCHIVE_ENDPOINT") {
            if !v.trim().is_empty() {
                cfg.endpoint = Some(v.trim().to_string());
            }
        }
        if let Ok(v) = env::var("PIT_CERT_ARCHIVE_ACCESS_KEY") {
            if !v.trim().is_empty() {
                cfg.access_key = Some(v.trim().to_string());
            }
        }
        if let Ok(v) = env::var("PIT_CERT_ARCHIVE_SECRET_KEY") {
            if !v.trim().is_empty() {
                cfg.secret_key = Some(v.trim().to_string());
            }
        }
        if let Ok(v) = env::var("PIT_CERT_ARCHIVE_LOCAL_PATH") {
            if !v.trim().is_empty() {
                cfg.local_path = v.trim().to_string();
            }
        }

        cfg
    }
}

/// Recursively canonicalizes a JSON value so all objects have their keys sorted alphabetically.
pub fn canonicalize_json(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut sorted = BTreeMap::new();
            for (k, v) in map {
                sorted.insert(k.clone(), canonicalize_json(v));
            }
            Value::Object(sorted.into_iter().collect())
        }
        Value::Array(arr) => {
            let canonical_arr: Vec<_> = arr.iter().map(canonicalize_json).collect();
            Value::Array(canonical_arr)
        }
        other => other.clone(),
    }
}

/// Assembles the canonical JSON pre-image representation of certificate inputs and test results.
/// All object keys at every level are strictly sorted in alphabetical order.
pub fn build_canonical_preimage(
    dataset_version: &str,
    universe: &str,
    audit_start_date: &str,
    audit_end_date: &str,
    overall_result: &str,
    tests: &PITCertificateTests,
    policies: &PITCertificatePolicies,
) -> String {
    let mut root = BTreeMap::new();
    root.insert("audit_end_date".to_string(), Value::String(audit_end_date.to_string()));
    root.insert("audit_start_date".to_string(), Value::String(audit_start_date.to_string()));
    root.insert("dataset_version".to_string(), Value::String(dataset_version.to_string()));
    root.insert("overall_result".to_string(), Value::String(overall_result.to_string()));

    // Serialize policies to canonical JSON
    if let Ok(policies_val) = serde_json::to_value(policies) {
        root.insert("policies".to_string(), canonicalize_json(&policies_val));
    }

    // Serialize tests to canonical JSON
    if let Ok(tests_val) = serde_json::to_value(tests) {
        root.insert("tests".to_string(), canonicalize_json(&tests_val));
    }

    root.insert("universe".to_string(), Value::String(universe.to_string()));

    let canonical_root = Value::Object(root.into_iter().collect());
    serde_json::to_string_pretty(&canonical_root).unwrap_or_default()
}

/// Computes the SHA-256 digital signature hex digest over the exact canonical JSON bytes.
pub fn compute_canonical_signature(canonical_json: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(canonical_json.as_bytes());
    format!("sha256:{:x}", hasher.finalize())
}

/// Cryptographic Proof Archiver handling local atomic storage and S3/MinIO uploads.
#[derive(Debug, Clone)]
pub struct PitCertArchiver {
    config: PitCertArchiveConfig,
    s3_client: Option<aws_sdk_s3::Client>,
}

impl PitCertArchiver {
    /// Creates a new `PitCertArchiver` from configuration.
    pub fn new(config: PitCertArchiveConfig) -> Self {
        let s3_client = if config.provider == "s3" || config.provider == "minio" {
            let region = aws_sdk_s3::config::Region::new(config.region.clone());
            let mut conf_builder = aws_sdk_s3::config::Builder::new()
                .region(region)
                .behavior_version(aws_sdk_s3::config::BehaviorVersion::latest());

            if let (Some(ref ak), Some(ref sk)) = (&config.access_key, &config.secret_key) {
                let creds = aws_sdk_s3::config::Credentials::new(
                    ak.clone(),
                    sk.clone(),
                    None,
                    None,
                    "static_pit_cert_credentials",
                );
                conf_builder = conf_builder.credentials_provider(creds);
            }

            if let Some(ref ep) = config.endpoint {
                conf_builder = conf_builder.endpoint_url(ep).force_path_style(true);
            }

            Some(aws_sdk_s3::Client::from_conf(conf_builder.build()))
        } else {
            None
        };

        Self { config, s3_client }
    }

    /// Creates an archiver by reading configuration from environment or `config.yaml`.
    pub fn from_env_or_config() -> Self {
        Self::new(PitCertArchiveConfig::from_env_or_config())
    }

    /// Access active configuration.
    pub fn config(&self) -> &PitCertArchiveConfig {
        &self.config
    }

    /// Archives the canonical JSON proof file and returns `(object_key, archive_timestamp)`.
    pub async fn archive_proof(
        &self,
        canonical_json: &str,
        dataset_version: &str,
        universe: &str,
        start_date: &str,
        end_date: &str,
    ) -> Result<(String, String), String> {
        if !self.config.enabled {
            return Err("PIT certificate proof archival is disabled in configuration".to_string());
        }

        // Clean universe for safe file naming
        let safe_universe = universe
            .replace(',', "_")
            .replace(' ', "")
            .replace('/', "_")
            .replace('\\', "_");

        let file_uuid = Uuid::new_v4().simple().to_string();
        let filename = format!(
            "pit-cert-{}-{}-{}-{}-{}.json",
            dataset_version, safe_universe, start_date, end_date, file_uuid
        );

        let timestamp = Utc::now().to_rfc3339();

        // 1. Write locally (always staged locally for audit trail and atomicity)
        let local_dir = PathBuf::from(&self.config.local_path);
        tokio::fs::create_dir_all(&local_dir)
            .await
            .map_err(|e| format!("Failed to create local archive directory '{}': {}", local_dir.display(), e))?;

        let target_file = local_dir.join(&filename);
        let temp_file = local_dir.join(format!("{}.tmp.{}", filename, file_uuid));

        tokio::fs::write(&temp_file, canonical_json.as_bytes())
            .await
            .map_err(|e| format!("Failed to write temporary archive file '{}': {}", temp_file.display(), e))?;

        tokio::fs::rename(&temp_file, &target_file)
            .await
            .map_err(|e| format!("Failed to rename archive file to '{}': {}", target_file.display(), e))?;

        // 2. If provider is S3 or MinIO, upload to remote bucket
        if (self.config.provider == "s3" || self.config.provider == "minio") && self.s3_client.is_some() {
            let s3_client = self.s3_client.as_ref().unwrap();
            let s3_key = format!("{}/{}", self.config.prefix.trim_matches('/'), filename);

            let byte_stream = aws_sdk_s3::primitives::ByteStream::from(canonical_json.as_bytes().to_vec());

            let put_future = s3_client
                .put_object()
                .bucket(&self.config.bucket)
                .key(&s3_key)
                .content_type("application/json")
                .body(byte_stream)
                .send();

            match tokio::time::timeout(Duration::from_secs(30), put_future).await {
                Ok(Ok(_)) => {
                    info!(
                        "[PIT Archive] Uploaded cryptographic proof to s3://{}/{}",
                        self.config.bucket, s3_key
                    );
                    Ok((s3_key, timestamp))
                }
                Ok(Err(e)) => {
                    warn!(
                        "[PIT Archive] S3 upload error: {}. Retained local file at '{}'",
                        e,
                        target_file.display()
                    );
                    let local_key = format!("{}/{}", self.config.local_path.trim_end_matches('/'), filename);
                    Ok((local_key, timestamp))
                }
                Err(_) => {
                    warn!(
                        "[PIT Archive] S3 upload timed out. Retained local file at '{}'",
                        target_file.display()
                    );
                    let local_key = format!("{}/{}", self.config.local_path.trim_end_matches('/'), filename);
                    Ok((local_key, timestamp))
                }
            }
        } else {
            // Local provider
            let object_key = format!("{}/{}", self.config.local_path.trim_end_matches('/'), filename);
            info!(
                "[PIT Archive] Archived cryptographic proof locally at '{}'",
                object_key
            );
            Ok((object_key, timestamp))
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::pit::{PITBackfillTestResult, PITDuplicateTestResult, PITTestResult};

    fn sample_tests() -> PITCertificateTests {
        PITCertificateTests {
            signal_availability_ordering: PITTestResult {
                violations: 0,
                total_checked: 1000,
                status: "pass".to_string(),
            },
            ticker_rename: PITTestResult {
                violations: 0,
                total_checked: 50,
                status: "pass".to_string(),
            },
            delisted_security: PITTestResult {
                violations: 0,
                total_checked: 25,
                status: "pass".to_string(),
            },
            corporate_action: PITTestResult {
                violations: 0,
                total_checked: 30,
                status: "pass".to_string(),
            },
            duplicate_event: PITDuplicateTestResult {
                duplicate_rate_pct: 0.05,
                total_checked: 1000,
                status: "pass".to_string(),
            },
            out_of_order_event: PITTestResult {
                violations: 0,
                total_checked: 1000,
                status: "pass".to_string(),
            },
            timestamp_precision: PITTestResult {
                violations: 0,
                total_checked: 1000,
                status: "pass".to_string(),
            },
            backfill_consistency: PITBackfillTestResult {
                backfill_count: 12,
                policy: "within_7_days".to_string(),
                status: "pass".to_string(),
            },
        }
    }

    fn sample_policies() -> PITCertificatePolicies {
        PITCertificatePolicies {
            timestamp_policy: "triple_timestamp_utc".to_string(),
            correction_policy: "append_only_with_new_record".to_string(),
            backfill_policy: "allowed_within_7_days_with_audit_log".to_string(),
            universe_policy: "pit_aware_with_delisting".to_string(),
        }
    }

    #[test]
    fn test_canonicalize_json_sorts_keys_alphabetically() {
        let input = serde_json::json!({
            "zebra": 1,
            "apple": 2,
            "nested": {
                "charlie": "c",
                "bravo": "b",
                "alpha": "a"
            }
        });

        let canonical = canonicalize_json(&input);
        let pretty = serde_json::to_string_pretty(&canonical).unwrap();

        // In pretty output, apple must appear before zebra
        let pos_apple = pretty.find("\"apple\"").unwrap();
        let pos_zebra = pretty.find("\"zebra\"").unwrap();
        assert!(pos_apple < pos_zebra, "apple should precede zebra");

        // In nested output, alpha < bravo < charlie
        let pos_alpha = pretty.find("\"alpha\"").unwrap();
        let pos_bravo = pretty.find("\"bravo\"").unwrap();
        let pos_charlie = pretty.find("\"charlie\"").unwrap();
        assert!(pos_alpha < pos_bravo, "alpha should precede bravo");
        assert!(pos_bravo < pos_charlie, "bravo should precede charlie");
    }

    #[test]
    fn test_build_canonical_preimage_determinism() {
        let tests = sample_tests();
        let policies = sample_policies();

        let json1 = build_canonical_preimage(
            "2.1.0",
            "all",
            "2025-06-01",
            "2025-08-31",
            "pass",
            &tests,
            &policies,
        );

        let json2 = build_canonical_preimage(
            "2.1.0",
            "all",
            "2025-06-01",
            "2025-08-31",
            "pass",
            &tests,
            &policies,
        );

        assert_eq!(json1, json2, "Canonical preimage must be 100% deterministic");

        let sig1 = compute_canonical_signature(&json1);
        let sig2 = compute_canonical_signature(&json2);
        assert_eq!(sig1, sig2);
        assert!(sig1.starts_with("sha256:"));
        assert_eq!(sig1.len(), 7 + 64);
    }

    #[tokio::test]
    async fn test_local_archive_atomic_write_and_hash_match() {
        let tmp_dir = format!("target/test_pit_archive_{}", Uuid::new_v4().simple());
        let config = PitCertArchiveConfig {
            enabled: true,
            provider: "local".to_string(),
            bucket: "test-bucket".to_string(),
            prefix: "test-prefix".to_string(),
            region: "us-east-1".to_string(),
            endpoint: None,
            access_key: None,
            secret_key: None,
            local_path: tmp_dir.clone(),
        };

        let archiver = PitCertArchiver::new(config);
        let tests = sample_tests();
        let policies = sample_policies();

        let canonical_json = build_canonical_preimage(
            "2.1.0",
            "AAPL,MSFT",
            "2025-06-01",
            "2025-08-31",
            "pass",
            &tests,
            &policies,
        );

        let expected_signature = compute_canonical_signature(&canonical_json);

        let (object_key, timestamp) = archiver
            .archive_proof(&canonical_json, "2.1.0", "AAPL,MSFT", "2025-06-01", "2025-08-31")
            .await
            .expect("Local archive write must succeed");

        assert!(object_key.starts_with(&tmp_dir));
        assert!(object_key.contains("pit-cert-2.1.0-AAPL_MSFT-2025-06-01-2025-08-31-"));
        assert!(object_key.ends_with(".json"));
        assert!(!timestamp.is_empty());

        // Verify the file exists on disk
        let file_bytes = tokio::fs::read(&object_key).await.expect("Archived file must exist");
        assert_eq!(
            String::from_utf8(file_bytes.clone()).unwrap(),
            canonical_json,
            "Archived file content must match canonical JSON exactly"
        );

        // Verify recomputed SHA-256 over exact file bytes matches signature
        let mut hasher = Sha256::new();
        hasher.update(&file_bytes);
        let recomputed_sig = format!("sha256:{:x}", hasher.finalize());

        assert_eq!(
            recomputed_sig, expected_signature,
            "SHA-256 over archived file bytes must match certificate signature"
        );

        // Cleanup
        let _ = tokio::fs::remove_dir_all(&tmp_dir).await;
    }

    #[tokio::test]
    async fn test_archive_disabled_returns_error() {
        let config = PitCertArchiveConfig {
            enabled: false,
            provider: "local".to_string(),
            bucket: "test-bucket".to_string(),
            prefix: "test-prefix".to_string(),
            region: "us-east-1".to_string(),
            endpoint: None,
            access_key: None,
            secret_key: None,
            local_path: "target/test_disabled".to_string(),
        };

        let archiver = PitCertArchiver::new(config);
        let result = archiver
            .archive_proof("{}", "2.1.0", "all", "2025-01-01", "2025-02-01")
            .await;

        assert!(result.is_err(), "Archiving when disabled must return Err");
    }
}
