//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Raw Data Archive Storage Adapter
//! Apache Parquet Writer & S3/MinIO Object Storage Integration
//! ═══════════════════════════════════════════════════════════════════════════════

use arrow::array::{ArrayRef, Float64Builder, StringBuilder};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use chrono::{DateTime, Datelike, Timelike, Utc};
use parquet::arrow::ArrowWriter;
use parquet::basic::Compression;
use parquet::file::properties::WriterProperties;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Configuration for the raw document archiving system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawArchiveConfig {
    /// Whether raw document archiving is enabled (default: false).
    pub enabled: bool,
    /// Storage provider: "local", "s3", or "minio" (default: "local").
    pub provider: String,
    /// Destination bucket name for S3 / MinIO (default: "fintext-raw-archive").
    pub bucket: String,
    /// Key prefix inside bucket (default: "raw").
    pub prefix: String,
    /// AWS region (default: "us-east-1").
    pub region: String,
    /// Custom S3 endpoint URL (required for MinIO, e.g. "http://localhost:9000").
    pub endpoint: Option<String>,
    /// S3 / MinIO access key id.
    pub access_key: Option<String>,
    /// S3 / MinIO secret access key.
    pub secret_key: Option<String>,
    /// Number of records to accumulate before flushing a Parquet file (default: 1000).
    pub batch_size: usize,
    /// Maximum seconds to wait before flushing buffered records (default: 60).
    pub flush_interval_secs: u64,
    /// Local directory for file staging and fallback (default: "data/archive").
    pub local_path: String,
    /// Mock mode flag for offline testing (default: false).
    pub mock_mode: bool,
    /// Number of days before Parquet objects transition to GLACIER storage class (default: 30).
    /// If <= 0, transition rule is disabled.
    pub lifecycle_transition_days: i32,
    /// Number of days before Parquet objects expire and are automatically deleted (default: 3650 / 10 years).
    /// If <= 0, expiration rule is disabled.
    pub lifecycle_expiration_days: i32,
    /// Whether to perform HeadObject verification after PutObject upload (default: false).
    pub verify_upload: bool,
}

impl Default for RawArchiveConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: "local".to_string(),
            bucket: "fintext-raw-archive".to_string(),
            prefix: "raw".to_string(),
            region: "us-east-1".to_string(),
            endpoint: None,
            access_key: None,
            secret_key: None,
            batch_size: 1000,
            flush_interval_secs: 60,
            local_path: "data/archive".to_string(),
            mock_mode: false,
            lifecycle_transition_days: 30,
            lifecycle_expiration_days: 3650,
            verify_upload: false,
        }
    }
}

impl RawArchiveConfig {
    /// Load configuration with priority: environment variables -> config/config.yaml -> defaults.
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
                let mut in_raw_archive = false;

                for line in contents.lines() {
                    let trimmed = line.trim();
                    if trimmed.starts_with("raw_archive:") {
                        in_raw_archive = true;
                        continue;
                    }
                    if in_raw_archive
                        && !line.starts_with(' ')
                        && !line.starts_with('\t')
                        && !trimmed.is_empty()
                    {
                        in_raw_archive = false;
                    }
                    if in_raw_archive {
                        if let Some((k, v)) = trimmed.split_once(':') {
                            let key = k.trim();
                            let val = v.split('#').next().unwrap_or("").trim().trim_matches('"');
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
                                "batch_size" => {
                                    if let Ok(n) = val.parse::<usize>() {
                                        cfg.batch_size = n;
                                    }
                                }
                                "flush_interval_secs" => {
                                    if let Ok(n) = val.parse::<u64>() {
                                        cfg.flush_interval_secs = n;
                                    }
                                }
                                "local_path" => {
                                    if !val.is_empty() {
                                        cfg.local_path = val.to_string();
                                    }
                                }
                                "lifecycle_transition_days" => {
                                    if let Ok(n) = val.parse::<i32>() {
                                        cfg.lifecycle_transition_days = n;
                                    }
                                }
                                "lifecycle_expiration_days" => {
                                    if let Ok(n) = val.parse::<i32>() {
                                        cfg.lifecycle_expiration_days = n;
                                    }
                                }
                                "verify_upload" => {
                                    if val == "true" || val == "1" {
                                        cfg.verify_upload = true;
                                    } else if val == "false" || val == "0" {
                                        cfg.verify_upload = false;
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
                break;
            }
        }

        // Environment overrides
        if let Ok(val) = env::var("RAW_ARCHIVE_ENABLED").or_else(|_| env::var("ENABLE_RAW_ARCHIVE"))
        {
            cfg.enabled = val == "1" || val.to_lowercase() == "true";
        }
        if let Ok(val) = env::var("RAW_ARCHIVE_PROVIDER") {
            if !val.trim().is_empty() {
                cfg.provider = val.trim().to_lowercase();
            }
        }
        if let Ok(val) = env::var("RAW_ARCHIVE_BUCKET") {
            if !val.trim().is_empty() {
                cfg.bucket = val.trim().to_string();
            }
        }
        if let Ok(val) = env::var("RAW_ARCHIVE_PREFIX") {
            if !val.trim().is_empty() {
                cfg.prefix = val.trim().to_string();
            }
        }
        if let Ok(val) = env::var("RAW_ARCHIVE_REGION").or_else(|_| env::var("AWS_REGION")) {
            if !val.trim().is_empty() {
                cfg.region = val.trim().to_string();
            }
        }
        if let Ok(val) = env::var("RAW_ARCHIVE_ENDPOINT") {
            if !val.trim().is_empty() {
                cfg.endpoint = Some(val.trim().to_string());
            }
        }
        if let Ok(val) =
            env::var("RAW_ARCHIVE_ACCESS_KEY").or_else(|_| env::var("AWS_ACCESS_KEY_ID"))
        {
            if !val.trim().is_empty() {
                cfg.access_key = Some(val.trim().to_string());
            }
        }
        if let Ok(val) =
            env::var("RAW_ARCHIVE_SECRET_KEY").or_else(|_| env::var("AWS_SECRET_ACCESS_KEY"))
        {
            if !val.trim().is_empty() {
                cfg.secret_key = Some(val.trim().to_string());
            }
        }
        if let Ok(val) = env::var("RAW_ARCHIVE_BATCH_SIZE") {
            if let Ok(n) = val.parse::<usize>() {
                cfg.batch_size = n;
            }
        }
        if let Ok(val) = env::var("RAW_ARCHIVE_FLUSH_INTERVAL_SECS") {
            if let Ok(n) = val.parse::<u64>() {
                cfg.flush_interval_secs = n;
            }
        }
        if let Ok(val) = env::var("RAW_ARCHIVE_LOCAL_PATH") {
            if !val.trim().is_empty() {
                cfg.local_path = val.trim().to_string();
            }
        }
        if let Ok(val) = env::var("RAW_ARCHIVE_MOCK") {
            cfg.mock_mode = val == "1" || val.to_lowercase() == "true";
        }
        if let Ok(val) = env::var("RAW_ARCHIVE_LIFECYCLE_TRANSITION_DAYS") {
            if let Ok(n) = val.parse::<i32>() {
                cfg.lifecycle_transition_days = n;
            }
        }
        if let Ok(val) = env::var("RAW_ARCHIVE_LIFECYCLE_EXPIRATION_DAYS") {
            if let Ok(n) = val.parse::<i32>() {
                cfg.lifecycle_expiration_days = n;
            }
        }
        if let Ok(val) = env::var("RAW_ARCHIVE_VERIFY_UPLOAD") {
            cfg.verify_upload = val == "1" || val.to_lowercase() == "true";
        }

        if cfg.provider == "local" {
            cfg.mock_mode = true;
        }

        cfg
    }

    /// Construct S3/MinIO BucketLifecycleConfiguration according to configured retention days.
    /// Returns None if both transition and expiration rules are disabled (<= 0).
    pub fn build_lifecycle_configuration(
        &self,
    ) -> Option<aws_sdk_s3::types::BucketLifecycleConfiguration> {
        use aws_sdk_s3::types::{
            BucketLifecycleConfiguration, ExpirationStatus, LifecycleExpiration, LifecycleRule,
            LifecycleRuleFilter, Transition, TransitionStorageClass,
        };

        if self.lifecycle_transition_days <= 0 && self.lifecycle_expiration_days <= 0 {
            return None;
        }

        let prefix_filter = if self.prefix.is_empty() {
            "raw/".to_string()
        } else {
            let clean = self.prefix.trim_matches('/');
            if clean.is_empty() {
                "raw/".to_string()
            } else {
                format!("{}/", clean)
            }
        };

        let mut rules = Vec::new();

        // Rule 1: Transition objects with prefix to GLACIER storage class after lifecycle_transition_days
        if self.lifecycle_transition_days > 0 {
            let transition = Transition::builder()
                .days(self.lifecycle_transition_days)
                .storage_class(TransitionStorageClass::Glacier)
                .build();

            if let Ok(rule) = LifecycleRule::builder()
                .id("raw-archive-glacier-transition")
                .filter(LifecycleRuleFilter::Prefix(prefix_filter.clone()))
                .status(ExpirationStatus::Enabled)
                .transitions(transition)
                .build()
            {
                rules.push(rule);
            }
        }

        // Rule 2: Expire objects with prefix after lifecycle_expiration_days
        if self.lifecycle_expiration_days > 0 {
            let expiration = LifecycleExpiration::builder()
                .days(self.lifecycle_expiration_days)
                .build();

            if let Ok(rule) = LifecycleRule::builder()
                .id("raw-archive-expiration")
                .filter(LifecycleRuleFilter::Prefix(prefix_filter))
                .status(ExpirationStatus::Enabled)
                .expiration(expiration)
                .build()
            {
                rules.push(rule);
            }
        }

        if rules.is_empty() {
            return None;
        }

        let mut config_builder = BucketLifecycleConfiguration::builder();
        for r in rules {
            config_builder = config_builder.rules(r);
        }
        config_builder.build().ok()
    }
}

/// A raw financial document record queued for Parquet serialization and archive storage.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RawArchiveRecord {
    pub published_utc: String,
    pub ticker: String,
    pub source: String,
    pub title: String,
    pub raw_content: String,
    pub ingested_utc: String,
    pub db_commit_utc: Option<String>,
    pub data_quality_score: Option<f64>,
    pub event_type: Option<String>,
}

/// Parquet Schema and Serialization Engine.
pub struct RawArchiveWriter;

impl RawArchiveWriter {
    /// Return the canonical Apache Arrow schema for raw archive records.
    pub fn schema() -> Arc<Schema> {
        Arc::new(Schema::new(vec![
            Field::new("published_utc", DataType::Utf8, false),
            Field::new("ticker", DataType::Utf8, false),
            Field::new("source", DataType::Utf8, false),
            Field::new("title", DataType::Utf8, false),
            Field::new("raw_content", DataType::Utf8, false),
            Field::new("ingested_utc", DataType::Utf8, false),
            Field::new("db_commit_utc", DataType::Utf8, true),
            Field::new("data_quality_score", DataType::Float64, true),
            Field::new("event_type", DataType::Utf8, true),
        ]))
    }

    /// Convert a slice of RawArchiveRecord into an Apache Arrow RecordBatch.
    pub fn records_to_record_batch(records: &[RawArchiveRecord]) -> Result<RecordBatch, String> {
        let schema = Self::schema();
        let num_rows = records.len();

        let mut published_builder = StringBuilder::with_capacity(num_rows, num_rows * 30);
        let mut ticker_builder = StringBuilder::with_capacity(num_rows, num_rows * 8);
        let mut source_builder = StringBuilder::with_capacity(num_rows, num_rows * 16);
        let mut title_builder = StringBuilder::with_capacity(num_rows, num_rows * 64);
        let mut content_builder = StringBuilder::with_capacity(num_rows, num_rows * 512);
        let mut ingested_builder = StringBuilder::with_capacity(num_rows, num_rows * 30);
        let mut commit_builder = StringBuilder::with_capacity(num_rows, num_rows * 30);
        let mut quality_builder = Float64Builder::with_capacity(num_rows);
        let mut event_builder = StringBuilder::with_capacity(num_rows, num_rows * 20);

        for rec in records {
            published_builder.append_value(&rec.published_utc);
            ticker_builder.append_value(&rec.ticker);
            source_builder.append_value(&rec.source);
            title_builder.append_value(&rec.title);
            content_builder.append_value(&rec.raw_content);
            ingested_builder.append_value(&rec.ingested_utc);

            if let Some(ref c) = rec.db_commit_utc {
                commit_builder.append_value(c);
            } else {
                commit_builder.append_null();
            }

            if let Some(q) = rec.data_quality_score {
                quality_builder.append_value(q);
            } else {
                quality_builder.append_null();
            }

            if let Some(ref e) = rec.event_type {
                event_builder.append_value(e);
            } else {
                event_builder.append_null();
            }
        }

        let columns: Vec<ArrayRef> = vec![
            Arc::new(published_builder.finish()),
            Arc::new(ticker_builder.finish()),
            Arc::new(source_builder.finish()),
            Arc::new(title_builder.finish()),
            Arc::new(content_builder.finish()),
            Arc::new(ingested_builder.finish()),
            Arc::new(commit_builder.finish()),
            Arc::new(quality_builder.finish()),
            Arc::new(event_builder.finish()),
        ];

        RecordBatch::try_new(schema, columns)
            .map_err(|e| format!("Failed to create RecordBatch: {}", e))
    }

    /// Generate target partition file path: `data/archive/year=YYYY/month=MM/day=DD/raw_YYYYMMDD_HHMMSS_{uid}.parquet`.
    pub fn generate_partitioned_path(base_dir: &str, timestamp_opt: Option<&str>) -> PathBuf {
        let dt: DateTime<Utc> = timestamp_opt
            .and_then(|ts| DateTime::parse_from_rfc3339(ts).ok())
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(Utc::now);

        let year = dt.year();
        let month = dt.month();
        let day = dt.day();

        let uid = Uuid::new_v4().to_string();
        let short_id = &uid[..8];
        let filename = format!(
            "raw_{:04}{:02}{:02}_{:02}{:02}{:02}_{}.parquet",
            year,
            month,
            day,
            dt.hour(),
            dt.minute(),
            dt.second(),
            short_id
        );

        Path::new(base_dir)
            .join(format!("year={:04}", year))
            .join(format!("month={:02}", month))
            .join(format!("day={:02}", day))
            .join(filename)
    }

    /// Write raw archive records into a Snappy-compressed Apache Parquet file.
    pub fn write_parquet_file(
        records: &[RawArchiveRecord],
        dest_path: &Path,
    ) -> Result<(PathBuf, usize), String> {
        if records.is_empty() {
            return Err("Cannot write empty record batch to Parquet".to_string());
        }

        // Ensure parent partition directory exists
        if let Some(parent) = dest_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory {}: {}", parent.display(), e))?;
        }

        let batch = Self::records_to_record_batch(records)?;
        let file = File::create(dest_path)
            .map_err(|e| format!("Failed to create file {}: {}", dest_path.display(), e))?;

        let props = WriterProperties::builder()
            .set_compression(Compression::SNAPPY)
            .build();

        let mut writer = ArrowWriter::try_new(file, batch.schema(), Some(props))
            .map_err(|e| format!("Failed to initialize Parquet ArrowWriter: {}", e))?;

        writer
            .write(&batch)
            .map_err(|e| format!("Failed to write RecordBatch to Parquet: {}", e))?;

        let file_metadata = writer
            .close()
            .map_err(|e| format!("Failed to close Parquet writer: {}", e))?;

        let num_rows = file_metadata.num_rows as usize;
        Ok((dest_path.to_path_buf(), num_rows))
    }
}

/// Uploader for archiving Parquet files to AWS S3 or MinIO.
pub struct ArchiveUploader {
    config: RawArchiveConfig,
    s3_client: Option<aws_sdk_s3::Client>,
}

impl ArchiveUploader {
    /// Create a new ArchiveUploader from configuration.
    pub async fn new(config: RawArchiveConfig) -> Self {
        let s3_client =
            if (config.provider == "s3" || config.provider == "minio") && !config.mock_mode {
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
                        "static_archive_credentials",
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

        let uploader = Self { config, s3_client };
        let _ = uploader.apply_lifecycle_policy().await;
        uploader
    }

    /// Access the underlying RawArchiveConfig.
    pub fn config(&self) -> &RawArchiveConfig {
        &self.config
    }

    /// Apply bucket lifecycle configuration on S3/MinIO in a best-effort manner.
    pub async fn apply_lifecycle_policy(&self) -> Result<(), String> {
        if let Some(ref client) = self.s3_client {
            if let Some(lifecycle_config) = self.config.build_lifecycle_configuration() {
                info!(
                    "[Raw Archive] Applying S3/MinIO lifecycle configuration for bucket '{}' (transition: {}d -> GLACIER, expiration: {}d, prefix: '{}/')...",
                    self.config.bucket,
                    self.config.lifecycle_transition_days,
                    self.config.lifecycle_expiration_days,
                    self.config.prefix.trim_matches('/')
                );
                match client
                    .put_bucket_lifecycle_configuration()
                    .bucket(&self.config.bucket)
                    .lifecycle_configuration(lifecycle_config)
                    .send()
                    .await
                {
                    Ok(_) => {
                        info!(
                            "[Raw Archive] Successfully configured bucket lifecycle policy for '{}'",
                            self.config.bucket
                        );
                        Ok(())
                    }
                    Err(e) => {
                        warn!(
                            "[Raw Archive] Failed to apply bucket lifecycle policy on '{}': {}. Continuing in best-effort mode.",
                            self.config.bucket, e
                        );
                        Err(format!("Failed to apply bucket lifecycle policy: {}", e))
                    }
                }
            } else {
                debug!("[Raw Archive] S3 lifecycle configuration skipped: both transition and expiration disabled (<= 0).");
                Ok(())
            }
        } else {
            Ok(())
        }
    }

    /// Upload a locally staged Parquet file to S3/MinIO.
    /// In local/mock mode, this logs and returns the local file URI immediately.
    pub async fn upload_file(
        &self,
        local_file_path: &Path,
        s3_key: &str,
    ) -> Result<String, String> {
        if self.config.provider == "local" || self.config.mock_mode {
            if self.config.verify_upload {
                if !local_file_path.exists() {
                    let err = format!(
                        "Upload verification failed: local file '{}' not found",
                        local_file_path.display()
                    );
                    error!("[Raw Archive] {}", err);
                    return Err(err);
                }
                debug!(
                    "[Raw Archive] Local provider: upload verified for '{}'",
                    local_file_path.display()
                );
            }
            debug!(
                "[Raw Archive] Local provider: retained at '{}'",
                local_file_path.display()
            );
            return Ok(format!("file://{}", local_file_path.display()));
        }

        if let Some(ref client) = self.s3_client {
            let byte_stream =
                match aws_sdk_s3::primitives::ByteStream::from_path(local_file_path).await {
                    Ok(bs) => bs,
                    Err(e) => {
                        let err = format!(
                            "Failed to read parquet file for upload '{}': {}",
                            local_file_path.display(),
                            e
                        );
                        error!("[Raw Archive] {}", err);
                        return Err(err);
                    }
                };

            let send_future = client
                .put_object()
                .bucket(&self.config.bucket)
                .key(s3_key)
                .body(byte_stream)
                .send();

            match tokio::time::timeout(Duration::from_secs(30), send_future).await {
                Ok(Ok(_)) => {
                    // HeadObject upload verification
                    if self.config.verify_upload {
                        let head_future = client
                            .head_object()
                            .bucket(&self.config.bucket)
                            .key(s3_key)
                            .send();

                        match tokio::time::timeout(Duration::from_secs(10), head_future).await {
                            Ok(Ok(head_output)) => {
                                let remote_size = head_output.content_length().unwrap_or(-1);
                                let local_size = fs::metadata(local_file_path)
                                    .map(|m| m.len() as i64)
                                    .unwrap_or(-2);

                                if remote_size > 0 && local_size > 0 && remote_size != local_size {
                                    let err = format!(
                                        "Upload verification size mismatch for key '{}': remote={} bytes, local={} bytes. Local file retained at '{}'",
                                        s3_key, remote_size, local_size, local_file_path.display()
                                    );
                                    warn!("[Raw Archive] {}", err);
                                    return Err(err);
                                }

                                info!(
                                    "[Raw Archive] Upload verified: object '{}' confirmed in bucket '{}' (size: {} bytes).",
                                    s3_key, self.config.bucket, remote_size
                                );
                            }
                            Ok(Err(e)) => {
                                let err = format!(
                                    "Upload verification failed: HeadObject returned error for key '{}': {}. Local file retained at '{}'",
                                    s3_key, e, local_file_path.display()
                                );
                                warn!("[Raw Archive] {}", err);
                                return Err(err);
                            }
                            Err(_) => {
                                let err = format!(
                                    "Upload verification timed out for key '{}' after 10s. Local file retained at '{}'",
                                    s3_key, local_file_path.display()
                                );
                                warn!("[Raw Archive] {}", err);
                                return Err(err);
                            }
                        }
                    }

                    let s3_uri = format!("s3://{}/{}", self.config.bucket, s3_key);
                    info!("[Raw Archive] Successfully uploaded to {}", s3_uri);
                    Ok(s3_uri)
                }
                Ok(Err(e)) => {
                    warn!(
                        "[Raw Archive] S3/MinIO upload error: {}. Local file retained at '{}'",
                        e,
                        local_file_path.display()
                    );
                    Err(format!("S3 upload failed: {}", e))
                }
                Err(_) => {
                    warn!(
                        "[Raw Archive] S3 upload timed out after 30s. Local file retained at '{}'",
                        local_file_path.display()
                    );
                    Err("S3 upload timed out".to_string())
                }
            }
        } else {
            Ok(format!("file://{}", local_file_path.display()))
        }
    }
}

/// Producer channel handle for submitting raw documents into the archiver.
#[derive(Clone)]
pub struct RawArchiveSender {
    tx: tokio::sync::mpsc::Sender<RawArchiveRecord>,
}

impl RawArchiveSender {
    pub fn new(tx: tokio::sync::mpsc::Sender<RawArchiveRecord>) -> Self {
        Self { tx }
    }

    /// Submit a raw record to the archiver without blocking ingestion.
    /// If channel is saturated, drops record and returns an error message.
    pub fn try_send(&self, record: RawArchiveRecord) -> Result<(), String> {
        self.tx
            .try_send(record)
            .map_err(|e| format!("Raw archive channel full/closed: {}", e))
    }
}

/// Spawns the background archiver task with bounded channel and interval timer.
pub fn spawn_raw_archiver_worker(
    config: RawArchiveConfig,
    mut rx: tokio::sync::mpsc::Receiver<RawArchiveRecord>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        info!(
            "[Raw Archive] Worker started: provider='{}', batch_size={}, flush_interval={}s, local_path='{}'",
            config.provider, config.batch_size, config.flush_interval_secs, config.local_path
        );

        let uploader = Arc::new(ArchiveUploader::new(config.clone()).await);
        let mut buffer: Vec<RawArchiveRecord> = Vec::with_capacity(config.batch_size);
        let mut interval =
            tokio::time::interval(Duration::from_secs(config.flush_interval_secs.max(1)));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            tokio::select! {
                record_opt = rx.recv() => {
                    match record_opt {
                        Some(rec) => {
                            buffer.push(rec);
                            if buffer.len() >= config.batch_size {
                                flush_buffer(&mut buffer, &config, &uploader).await;
                            }
                        }
                        None => {
                            info!("[Raw Archive] Ingestion channel closed. Performing final buffer flush...");
                            break;
                        }
                    }
                }
                _ = interval.tick() => {
                    if !buffer.is_empty() {
                        debug!("[Raw Archive] Flush timer elapsed with {} records in buffer.", buffer.len());
                        flush_buffer(&mut buffer, &config, &uploader).await;
                    }
                }
            }
        }

        // Final flush on shutdown
        if !buffer.is_empty() {
            flush_buffer(&mut buffer, &config, &uploader).await;
        }

        info!("[Raw Archive] Worker shutdown complete.");
    })
}

/// Flush buffered records to a partitioned Parquet file and trigger uploader.
async fn flush_buffer(
    buffer: &mut Vec<RawArchiveRecord>,
    config: &RawArchiveConfig,
    uploader: &ArchiveUploader,
) {
    if buffer.is_empty() {
        return;
    }

    let records: Vec<RawArchiveRecord> = buffer.drain(..).collect();
    let count = records.len();
    let first_ts = records.first().map(|r| r.published_utc.clone());
    let local_path_base = config.local_path.clone();
    let prefix = config.prefix.clone();

    // Offload synchronous Parquet encoding to blocking thread pool
    let write_res = tokio::task::spawn_blocking(move || {
        let dest =
            RawArchiveWriter::generate_partitioned_path(&local_path_base, first_ts.as_deref());
        RawArchiveWriter::write_parquet_file(&records, &dest)
    })
    .await;

    match write_res {
        Ok(Ok((path, rows))) => {
            let file_size_kb = fs::metadata(&path)
                .map(|m| m.len() as f64 / 1024.0)
                .unwrap_or(0.0);

            info!(
                "[Raw Archive] Flushed {} records to '{}' ({:.1} KB)",
                rows,
                path.display(),
                file_size_kb
            );

            // Construct S3 key matching partition hierarchy
            let dt = Utc::now();
            let s3_key = format!(
                "{}/year={:04}/month={:02}/day={:02}/{}",
                prefix,
                dt.year(),
                dt.month(),
                dt.day(),
                path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("raw.parquet")
            );

            let _ = uploader.upload_file(&path, &s3_key).await;
        }
        Ok(Err(e)) => {
            error!(
                "[Raw Archive] Failed to write Parquet file ({} records lost): {}",
                count, e
            );
        }
        Err(join_err) => {
            error!(
                "[Raw Archive] Blocking worker join error during flush: {}",
                join_err
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;

    #[test]
    fn test_raw_archive_schema_definition() {
        let schema = RawArchiveWriter::schema();
        assert_eq!(schema.fields().len(), 9);

        assert_eq!(schema.field(0).name(), "published_utc");
        assert!(!schema.field(0).is_nullable());
        assert_eq!(schema.field(0).data_type(), &DataType::Utf8);

        assert_eq!(schema.field(1).name(), "ticker");
        assert!(!schema.field(1).is_nullable());

        assert_eq!(schema.field(2).name(), "source");
        assert!(!schema.field(2).is_nullable());

        assert_eq!(schema.field(3).name(), "title");
        assert!(!schema.field(3).is_nullable());

        assert_eq!(schema.field(4).name(), "raw_content");
        assert!(!schema.field(4).is_nullable());

        assert_eq!(schema.field(5).name(), "ingested_utc");
        assert!(!schema.field(5).is_nullable());

        assert_eq!(schema.field(6).name(), "db_commit_utc");
        assert!(schema.field(6).is_nullable());

        assert_eq!(schema.field(7).name(), "data_quality_score");
        assert!(schema.field(7).is_nullable());
        assert_eq!(schema.field(7).data_type(), &DataType::Float64);

        assert_eq!(schema.field(8).name(), "event_type");
        assert!(schema.field(8).is_nullable());
    }

    #[test]
    fn test_record_batch_conversion_and_parquet_roundtrip() {
        let temp_dir = env::temp_dir().join(format!("test_archive_{}", Uuid::new_v4()));
        let _ = fs::create_dir_all(&temp_dir);
        let target_file = temp_dir.join("test_roundtrip.parquet");

        let records = vec![
            RawArchiveRecord {
                published_utc: "2026-09-06T10:00:00Z".to_string(),
                ticker: "AAPL".to_string(),
                source: "sec_edgar".to_string(),
                title: "Apple Q3 Services Report".to_string(),
                raw_content: "<html><body>Apple reports record Q3 gross margin.</body></html>"
                    .to_string(),
                ingested_utc: "2026-09-06T10:00:01Z".to_string(),
                db_commit_utc: Some("2026-09-06T10:00:02Z".to_string()),
                data_quality_score: Some(0.95),
                event_type: Some("EARNINGS".to_string()),
            },
            RawArchiveRecord {
                published_utc: "2026-09-06T11:00:00Z".to_string(),
                ticker: "NVDA".to_string(),
                source: "finnhub".to_string(),
                title: "NVIDIA Next-Gen Architecture".to_string(),
                raw_content: "Data center demand continues exponential growth.".to_string(),
                ingested_utc: "2026-09-06T11:00:01Z".to_string(),
                db_commit_utc: None,
                data_quality_score: None,
                event_type: None,
            },
        ];

        // 1. Write Parquet
        let (written_path, rows_written) =
            RawArchiveWriter::write_parquet_file(&records, &target_file).unwrap();
        assert_eq!(rows_written, 2);
        assert!(written_path.exists());

        // 2. Read back via Arrow reader
        let file = File::open(&written_path).unwrap();
        let builder = ParquetRecordBatchReaderBuilder::try_new(file).unwrap();
        let mut reader = builder.build().unwrap();

        let batch = reader.next().unwrap().unwrap();
        assert_eq!(batch.num_rows(), 2);
        assert_eq!(batch.num_columns(), 9);

        // Check values in first column (published_utc)
        let pub_col = batch
            .column(0)
            .as_any()
            .downcast_ref::<arrow::array::StringArray>()
            .unwrap();
        assert_eq!(pub_col.value(0), "2026-09-06T10:00:00Z");
        assert_eq!(pub_col.value(1), "2026-09-06T11:00:00Z");

        // Check tickers
        let ticker_col = batch
            .column(1)
            .as_any()
            .downcast_ref::<arrow::array::StringArray>()
            .unwrap();
        assert_eq!(ticker_col.value(0), "AAPL");
        assert_eq!(ticker_col.value(1), "NVDA");

        // Check raw_content
        let content_col = batch
            .column(4)
            .as_any()
            .downcast_ref::<arrow::array::StringArray>()
            .unwrap();
        assert!(content_col.value(0).contains("Apple reports record"));
        assert!(content_col.value(1).contains("Data center demand"));

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_partition_path_generation() {
        let path = RawArchiveWriter::generate_partitioned_path(
            "data/archive",
            Some("2026-09-06T15:30:45Z"),
        );
        let path_str = path.to_str().unwrap().replace('\\', "/");

        assert!(path_str.contains("data/archive/year=2026/month=09/day=06"));
        assert!(path_str.ends_with(".parquet"));
        assert!(path_str.contains("raw_20260906_153045_"));
    }

    #[test]
    fn test_raw_archive_config_defaults() {
        let cfg = RawArchiveConfig::default();
        assert!(!cfg.enabled);
        assert_eq!(cfg.provider, "local");
        assert_eq!(cfg.batch_size, 1000);
        assert_eq!(cfg.flush_interval_secs, 60);
        assert_eq!(cfg.local_path, "data/archive");
        assert_eq!(cfg.bucket, "fintext-raw-archive");
        assert_eq!(cfg.lifecycle_transition_days, 30);
        assert_eq!(cfg.lifecycle_expiration_days, 3650);
        assert!(!cfg.verify_upload);
    }

    #[test]
    fn test_raw_archive_config_env_overrides() {
        env::set_var("RAW_ARCHIVE_LIFECYCLE_TRANSITION_DAYS", "45");
        env::set_var("RAW_ARCHIVE_LIFECYCLE_EXPIRATION_DAYS", "730");
        env::set_var("RAW_ARCHIVE_VERIFY_UPLOAD", "true");

        let cfg = RawArchiveConfig::from_env_or_config();
        assert_eq!(cfg.lifecycle_transition_days, 45);
        assert_eq!(cfg.lifecycle_expiration_days, 730);
        assert!(cfg.verify_upload);

        env::remove_var("RAW_ARCHIVE_LIFECYCLE_TRANSITION_DAYS");
        env::remove_var("RAW_ARCHIVE_LIFECYCLE_EXPIRATION_DAYS");
        env::remove_var("RAW_ARCHIVE_VERIFY_UPLOAD");
    }

    #[test]
    fn test_lifecycle_rule_construction_default() {
        use aws_sdk_s3::types::TransitionStorageClass;

        let cfg = RawArchiveConfig::default();
        let lifecycle_opt = cfg.build_lifecycle_configuration();
        assert!(lifecycle_opt.is_some());

        let lifecycle = lifecycle_opt.unwrap();
        let rules = lifecycle.rules();
        assert_eq!(rules.len(), 2);

        // Rule 1: Transition
        let r1 = &rules[0];
        assert_eq!(r1.id(), Some("raw-archive-glacier-transition"));
        let transitions = r1.transitions();
        assert_eq!(transitions.len(), 1);
        assert_eq!(transitions[0].days(), Some(30));
        assert_eq!(
            transitions[0].storage_class(),
            Some(&TransitionStorageClass::Glacier)
        );

        // Rule 2: Expiration
        let r2 = &rules[1];
        assert_eq!(r2.id(), Some("raw-archive-expiration"));
        let exp = r2.expiration().expect("Expected expiration in rule 2");
        assert_eq!(exp.days(), Some(3650));
    }

    #[test]
    fn test_lifecycle_rule_partial_or_disabled() {
        // Transition disabled (<= 0), expiration enabled
        let mut cfg = RawArchiveConfig::default();
        cfg.lifecycle_transition_days = 0;
        cfg.lifecycle_expiration_days = 365;
        let lc1 = cfg
            .build_lifecycle_configuration()
            .expect("Expected 1 rule");
        assert_eq!(lc1.rules().len(), 1);
        assert_eq!(lc1.rules()[0].id(), Some("raw-archive-expiration"));

        // Transition enabled, expiration disabled (<= 0)
        cfg.lifecycle_transition_days = 60;
        cfg.lifecycle_expiration_days = -1;
        let lc2 = cfg
            .build_lifecycle_configuration()
            .expect("Expected 1 rule");
        assert_eq!(lc2.rules().len(), 1);
        assert_eq!(lc2.rules()[0].id(), Some("raw-archive-glacier-transition"));

        // Both disabled (<= 0) -> returns None
        cfg.lifecycle_transition_days = 0;
        cfg.lifecycle_expiration_days = 0;
        assert!(cfg.build_lifecycle_configuration().is_none());
    }

    #[tokio::test]
    async fn test_upload_verification_local_mode() {
        let temp_dir = env::temp_dir().join(format!("test_verify_{}", Uuid::new_v4()));
        let _ = fs::create_dir_all(&temp_dir);
        let valid_file = temp_dir.join("existing.parquet");
        fs::write(&valid_file, b"PAR1_test_content").unwrap();
        let missing_file = temp_dir.join("missing.parquet");

        let mut cfg = RawArchiveConfig::default();
        cfg.provider = "local".to_string();
        cfg.verify_upload = true;

        let uploader = ArchiveUploader::new(cfg).await;

        // Existing file passes verification
        let ok_res = uploader.upload_file(&valid_file, "raw/test.parquet").await;
        assert!(ok_res.is_ok());

        // Missing file fails verification
        let err_res = uploader
            .upload_file(&missing_file, "raw/missing.parquet")
            .await;
        assert!(err_res.is_err());
        assert!(err_res.unwrap_err().contains("local file"));

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_raw_archive_channel_non_blocking_behavior() {
        let (tx, mut rx) = tokio::sync::mpsc::channel(2);
        let sender = RawArchiveSender::new(tx);

        let rec = RawArchiveRecord {
            published_utc: "2026-09-06T10:00:00Z".to_string(),
            ticker: "MSFT".to_string(),
            source: "polygon".to_string(),
            title: "MSFT Options Flow".to_string(),
            raw_content: "Options call buying detected".to_string(),
            ingested_utc: "2026-09-06T10:00:01Z".to_string(),
            db_commit_utc: None,
            data_quality_score: Some(0.90),
            event_type: Some("OPTIONS".to_string()),
        };

        // Two successful sends fill the channel
        assert!(sender.try_send(rec.clone()).is_ok());
        assert!(sender.try_send(rec.clone()).is_ok());

        // Third send overflows the channel -> must return Err without blocking
        let overflow_res = sender.try_send(rec.clone());
        assert!(overflow_res.is_err());
        assert!(overflow_res.unwrap_err().contains("channel full"));

        // Draining one item frees capacity
        let drained = rx.recv().await;
        assert!(drained.is_some());
        assert!(sender.try_send(rec).is_ok());
    }

    #[test]
    fn test_s3_lifecycle_types() {
        use aws_sdk_s3::types::{
            BucketLifecycleConfiguration, ExpirationStatus, LifecycleExpiration, LifecycleRule,
            LifecycleRuleFilter, Transition, TransitionStorageClass,
        };

        let transition = Transition::builder()
            .days(30)
            .storage_class(TransitionStorageClass::Glacier)
            .build();

        let expiration = LifecycleExpiration::builder().days(3650).build();

        let rule = LifecycleRule::builder()
            .id("raw-archive-lifecycle-rule")
            .filter(LifecycleRuleFilter::Prefix("raw/".to_string()))
            .status(ExpirationStatus::Enabled)
            .transitions(transition)
            .expiration(expiration)
            .build()
            .expect("Failed to build LifecycleRule");

        let config = BucketLifecycleConfiguration::builder()
            .rules(rule)
            .build()
            .expect("Failed to build BucketLifecycleConfiguration");

        assert_eq!(config.rules().len(), 1);

        let head_out = aws_sdk_s3::operation::head_object::HeadObjectOutput::builder()
            .content_length(1024)
            .build();
        assert_eq!(head_out.content_length(), Some(1024));

        let client = aws_sdk_s3::Client::from_conf(
            aws_sdk_s3::config::Builder::new()
                .region(aws_sdk_s3::config::Region::new("us-east-1"))
                .behavior_version(aws_sdk_s3::config::BehaviorVersion::latest())
                .build(),
        );
        let _req = client
            .put_bucket_lifecycle_configuration()
            .bucket("test-bucket")
            .lifecycle_configuration(config);
    }
}
