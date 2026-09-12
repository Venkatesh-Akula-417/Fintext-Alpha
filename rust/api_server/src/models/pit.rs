//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Point-in-Time (PIT) Replay DTO Models
//! ═══════════════════════════════════════════════════════════════════════════════

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Query parameters for Point-in-Time Data Replay (`GET /pit/replay`).
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct PITReplayParams {
    /// Target stock ticker symbol (e.g., 'AAPL', 'NVDA', 'MSFT').
    #[schema(example = "AAPL")]
    pub ticker: String,

    /// Historical as-of point-in-time timestamp in RFC3339 format (e.g. '2025-06-15T14:30:00Z').
    #[schema(example = "2025-06-15T14:30:00Z")]
    pub as_of_utc: String,

    /// Whether to include news articles committed as of timestamp (default: true).
    #[schema(example = true)]
    pub include_news: Option<bool>,

    /// Whether to include SEC regulatory filings committed as of timestamp (default: true).
    #[schema(example = true)]
    pub include_filings: Option<bool>,

    /// Whether to include corporate/market events committed as of timestamp (default: true).
    #[schema(example = true)]
    pub include_events: Option<bool>,

    /// Whether to include sentiment analysis records committed as of timestamp (default: true).
    #[schema(example = true)]
    pub include_sentiment: Option<bool>,

    /// Maximum number of records per category to return (default: 50, max: 200).
    #[schema(example = 50)]
    pub limit: Option<u32>,
}

/// Point-in-Time News Article Snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct PITReplayNewsItem {
    /// Unique article identifier.
    #[schema(example = "news-aapl-20250615-001")]
    pub id: String,

    /// Headline title.
    #[schema(example = "Apple Expands Silicon AI Architecture Roadmap")]
    pub title: String,

    /// Originating news source or agency.
    #[schema(example = "Institutional Wire")]
    pub source: String,

    /// Source event publication timestamp (ISO-8601 UTC).
    #[schema(example = "2025-06-15T14:20:00.000000Z")]
    pub published_utc: String,

    /// Platform ingestion timestamp (ISO-8601 UTC).
    #[schema(example = "2025-06-15T14:20:00.045000Z")]
    pub ingested_utc: String,

    /// Database commit timestamp (ISO-8601 UTC).
    #[schema(example = "2025-06-15T14:20:00.090000Z")]
    pub db_commit_utc: String,

    /// Associated sentiment score (-1.0 to 1.0).
    #[schema(example = 0.65)]
    pub sentiment_score: f64,
}

/// Point-in-Time Regulatory Filing Snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct PITReplayFilingItem {
    /// Filing unique record identifier.
    #[schema(example = "filing-aapl-8k-20250614")]
    pub id: String,

    /// SEC Form type (e.g., '8-K', '10-Q', '10-K', 'FORM 4').
    #[schema(example = "8-K")]
    pub form_type: String,

    /// SEC filing date (YYYY-MM-DD).
    #[schema(example = "2025-06-14")]
    pub filing_date: String,

    /// SEC EDGAR accession number.
    #[schema(example = "0000320193-25-000050")]
    pub accession_number: String,

    /// Material event category classification.
    #[schema(example = "Item 2.02 Results of Operations")]
    pub event_category: String,

    /// Public release timestamp (ISO-8601 UTC).
    #[schema(example = "2025-06-14T20:05:00.000000Z")]
    pub published_utc: String,

    /// System ingestion timestamp (ISO-8601 UTC).
    #[schema(example = "2025-06-14T20:05:01.200000Z")]
    pub ingested_utc: String,
}

/// Point-in-Time Corporate/Market Event Snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct PITReplayEventItem {
    /// Event unique record identifier.
    #[schema(example = "evt-aapl-earnings-20250614")]
    pub id: String,

    /// Material corporate event type.
    #[schema(example = "EARNINGS_ANNOUNCEMENT")]
    pub event_type: String,

    /// Target stock ticker symbol.
    #[schema(example = "AAPL")]
    pub ticker: String,

    /// Corporate event occurrence date (YYYY-MM-DD).
    #[schema(example = "2025-06-14")]
    pub event_date: String,

    /// Event announcement timestamp (ISO-8601 UTC).
    #[schema(example = "2025-06-14T20:00:00.000000Z")]
    pub published_utc: String,

    /// Ingestion timestamp (ISO-8601 UTC).
    #[schema(example = "2025-06-14T20:00:00.120000Z")]
    pub ingested_utc: String,
}

/// Point-in-Time Sentiment Record Snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct PITReplaySentimentItem {
    /// Unique sentiment scoring record identifier.
    #[schema(example = "sent-aapl-20250615-142000")]
    pub id: String,

    /// Target stock ticker symbol.
    #[schema(example = "AAPL")]
    pub ticker: String,

    /// Source event publication timestamp (ISO-8601 UTC).
    #[schema(example = "2025-06-15T14:20:00.000000Z")]
    pub published_utc: String,

    /// Platform ingestion timestamp (ISO-8601 UTC).
    #[schema(example = "2025-06-15T14:20:00.045000Z")]
    pub ingested_utc: String,

    /// Database commit timestamp (ISO-8601 UTC).
    #[schema(example = "2025-06-15T14:20:00.090000Z")]
    pub db_commit_utc: String,

    /// Quantized sentiment score (-1.0 to 1.0).
    #[schema(example = 0.65)]
    pub sentiment_score: f64,

    /// Sentiment classification label (e.g. POSITIVE, NEGATIVE, NEUTRAL).
    #[schema(example = "POSITIVE")]
    pub sentiment_label: String,

    /// Classification prediction confidence score (0.0 to 1.0).
    #[schema(example = 0.92)]
    pub confidence: f64,

    /// Point-in-time validity start timestamp in ISO-8601 UTC (SCD Type 2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = "2025-06-15T14:20:00.090000Z")]
    pub valid_from: Option<String>,

    /// Point-in-time validity end timestamp in ISO-8601 UTC (None if current version) (SCD Type 2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = "2025-06-16T09:15:00.000000Z")]
    pub valid_to: Option<String>,

    /// Slowly Changing Dimension revision number (starts at 1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = 1)]
    pub revision_number: Option<i32>,

    /// True if this record represents the latest active revision.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = true)]
    pub is_current: Option<bool>,
}

impl Default for PITReplaySentimentItem {
    fn default() -> Self {
        Self {
            id: String::new(),
            ticker: String::new(),
            published_utc: String::new(),
            ingested_utc: String::new(),
            db_commit_utc: String::new(),
            sentiment_score: 0.0,
            sentiment_label: "NEUTRAL".to_string(),
            confidence: 0.0,
            valid_from: None,
            valid_to: None,
            revision_number: None,
            is_current: None,
        }
    }
}

/// Point-in-Time Category Record Counts and Summary.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct PITReplaySummary {
    /// Number of point-in-time news articles returned.
    #[schema(example = 5)]
    pub news_count: usize,

    /// Number of point-in-time regulatory filings returned.
    #[schema(example = 2)]
    pub filings_count: usize,

    /// Number of point-in-time corporate events returned.
    #[schema(example = 1)]
    pub events_count: usize,

    /// Number of point-in-time sentiment records returned.
    #[schema(example = 8)]
    pub sentiment_count: usize,

    /// Total count of all point-in-time records across categories.
    #[schema(example = 16)]
    pub total_records: usize,
}

/// Point-in-Time Temporal Consistency and Look-Ahead-Bias Verification.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct PITReplayConsistency {
    /// Whether all returned records strictly satisfy published_utc <= ingested_utc <= db_commit_utc <= as_of_utc.
    #[schema(example = true)]
    pub all_records_consistent: bool,

    /// Number of temporal order or look-ahead violations detected (should always be 0).
    #[schema(example = 0)]
    pub violations_count: usize,

    /// Strict temporal invariant evaluated.
    #[schema(example = "published_utc <= ingested_utc <= db_commit_utc <= as_of_utc")]
    pub invariant: String,

    /// Consistency evaluation summary message.
    #[schema(example = "All records verified point-in-time consistent with zero look-ahead bias.")]
    pub message: String,
}

/// Response payload containing reconstructed historical state and look-ahead audit validation.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct PITReplayResponse {
    /// Requested target stock ticker symbol.
    #[schema(example = "AAPL")]
    pub ticker: String,

    /// Historical as-of point-in-time timestamp evaluated (ISO-8601 RFC3339 UTC).
    #[schema(example = "2025-06-15T14:30:00Z")]
    pub as_of_utc: String,

    /// News articles visible to the model as of the timestamp.
    pub news_articles: Vec<PITReplayNewsItem>,

    /// Regulatory filings visible to the model as of the timestamp.
    pub filings: Vec<PITReplayFilingItem>,

    /// Corporate and market events visible to the model as of the timestamp.
    pub events: Vec<PITReplayEventItem>,

    /// Sentiment scoring records visible to the model as of the timestamp.
    pub sentiment_records: Vec<PITReplaySentimentItem>,

    /// Category and total record counts summary.
    pub summary: PITReplaySummary,

    /// Look-ahead-bias and temporal consistency verification report.
    pub replay_consistency: PITReplayConsistency,

    /// Maximum db_commit_utc timestamp across all visible records (exact model state timestamp).
    #[schema(example = "2025-06-15T14:20:00.090000Z")]
    pub model_generated_at: String,

    /// Diagnostic and status message.
    #[schema(
        example = "Point-in-time historical state successfully reconstructed for AAPL as of 2025-06-15T14:30:00Z"
    )]
    pub message: String,
}

/// Query parameters for Point-in-Time Data Certification (`GET /pit/certificate`).
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct PITCertificateParams {
    /// Dataset or signal processing pipeline version to certify (default: 'current' or '2.1.0').
    #[schema(example = "2.1.0")]
    pub dataset_version: Option<String>,

    /// Target stock ticker universe to audit ('all', 'sp500', or comma-separated tickers, default: 'all').
    #[schema(example = "all")]
    pub universe: Option<String>,

    /// Historical audit start date in ISO YYYY-MM-DD format (default: 90 days ago).
    #[schema(example = "2025-06-01")]
    pub start_date: Option<String>,

    /// Historical audit end date in ISO YYYY-MM-DD format (default: today).
    #[schema(example = "2025-08-31")]
    pub end_date: Option<String>,
}

/// Single point-in-time audit test evaluation result.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct PITTestResult {
    /// Number of detected look-ahead or rule violations.
    #[schema(example = 0)]
    pub violations: usize,

    /// Total number of records evaluated during the audit.
    #[schema(example = 15000)]
    pub total_checked: usize,

    /// Test execution status ('pass', 'conditional_pass', 'fail').
    #[schema(example = "pass")]
    pub status: String,
}

/// Duplicate event test evaluation result.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct PITDuplicateTestResult {
    /// Percentage of duplicate records detected across the dataset.
    #[schema(example = 0.2)]
    pub duplicate_rate_pct: f64,

    /// Total number of records evaluated.
    #[schema(example = 15000)]
    pub total_checked: usize,

    /// Test execution status ('pass', 'fail').
    #[schema(example = "pass")]
    pub status: String,
}

/// Backfill consistency test evaluation result.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct PITBackfillTestResult {
    /// Count of records backfilled into the historical stream.
    #[schema(example = 12)]
    pub backfill_count: usize,

    /// Platform data backfill policy applied.
    #[schema(example = "within_7_days")]
    pub policy: String,

    /// Test execution status ('pass', 'conditional_pass', 'fail').
    #[schema(example = "pass")]
    pub status: String,
}

/// Battery of 8 automated Point-in-Time and Look-Ahead Bias audit tests.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct PITCertificateTests {
    /// Test 1: Triple timestamp ordering (published_utc <= ingested_utc <= db_commit_utc).
    pub signal_availability_ordering: PITTestResult,

    /// Test 2: Ticker rename look-ahead prevention (no records prior to rename effective date).
    pub ticker_rename: PITTestResult,

    /// Test 3: Delisted security survivorship bias check (no records post-delisting).
    pub delisted_security: PITTestResult,

    /// Test 4: Corporate actions effective date alignment (splits, dividends).
    pub corporate_action: PITTestResult,

    /// Test 5: Duplicate event deduplication rate.
    pub duplicate_event: PITDuplicateTestResult,

    /// Test 6: Clock skew / out-of-order event check (ingested_utc >= published_utc).
    pub out_of_order_event: PITTestResult,

    /// Test 7: Timezone and timestamp precision verification (valid RFC3339 UTC).
    pub timestamp_precision: PITTestResult,

    /// Test 8: Backfill latency and SLA consistency check.
    pub backfill_consistency: PITBackfillTestResult,
}

/// Institutional governance, correction, and timestamp policies.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct PITCertificatePolicies {
    /// Ingestion timestamp specification standard.
    #[schema(example = "triple_timestamp_utc")]
    pub timestamp_policy: String,

    /// Historical data correction policy.
    #[schema(example = "append_only_with_new_record")]
    pub correction_policy: String,

    /// Allowed backfill window and auditing requirements.
    #[schema(example = "allowed_within_7_days_with_audit_log")]
    pub backfill_policy: String,

    /// Survivorship bias mitigation policy.
    #[schema(example = "pit_aware_with_delisting")]
    pub universe_policy: String,
}

/// Formal Cryptographically Signed Point-in-Time Audit Certificate.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct PITCertificateResponse {
    /// Unique deterministic certificate identification code.
    #[schema(example = "PIT-CERT-20250902-001")]
    pub certificate_id: String,

    /// Certified dataset or processing pipeline version.
    #[schema(example = "2.1.0")]
    pub dataset_version: String,

    /// Evaluated asset universe filter ('all', 'sp500', or specific constituents).
    #[schema(example = "all")]
    pub universe: String,

    /// Historical audit start date (YYYY-MM-DD).
    #[schema(example = "2025-06-01")]
    pub audit_start_date: String,

    /// Historical audit end date (YYYY-MM-DD).
    #[schema(example = "2025-08-31")]
    pub audit_end_date: String,

    /// Certificate generation timestamp (ISO-8601 UTC).
    #[schema(example = "2025-09-02T10:00:00Z")]
    pub issued_at: String,

    /// Overall certification outcome ('pass', 'conditional_pass', 'fail').
    #[schema(example = "pass")]
    pub overall_result: String,

    /// Comprehensive audit test results breakdown.
    pub tests: PITCertificateTests,

    /// Platform governance and audit policies certified.
    pub policies: PITCertificatePolicies,

    /// Cryptographic SHA-256 digital signature over certificate payload.
    #[schema(example = "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855")]
    pub signature: String,

    /// Certificate active status ('active', 'revoked', 'expired').
    #[schema(example = "active")]
    pub status: String,

    /// Archive object storage key for cryptographic audit proof (null if archival disabled).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(
        example = "data/pit-cert-archive/pit-cert-2.1.0-all-2025-06-01-2025-08-31-0123456789abcdef.json"
    )]
    pub archive_object_key: Option<String>,

    /// Timestamp when cryptographic proof was archived (ISO-8601 UTC, null if archival disabled).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "2025-09-02T10:00:00Z")]
    pub archive_timestamp: Option<String>,
}
