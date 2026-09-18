use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

/// Model Governance, Version Tagging, and Data Provenance Lineage Metadata.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct ModelMetadata {
    /// Transformer / Deep Learning inference model version tag
    #[schema(example = "finbert-v3.1.0")]
    pub model_version: String,
    /// Quantitative feature engineering & processing pipeline semantic version
    #[schema(example = "2.0.0")]
    pub pipeline_version: String,
    /// Upstream data providers and source lineage collection
    #[schema(example = json!(["SEC EDGAR", "Finnhub", "Polygon"]))]
    pub data_provenance: Vec<String>,
    /// UTC timestamp of the most recent model retraining
    #[schema(example = "2026-08-31T12:00:00Z")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_trained_at: Option<DateTime<Utc>>,
}

pub const DEFAULT_MODEL_VERSION: &str = "finbert-v3.1.0";
pub const DEFAULT_PIPELINE_VERSION: &str = "2.0.0";
pub const DEFAULT_DATA_PROVENANCE: &[&str] = &["SEC EDGAR", "Finnhub", "Polygon"];

impl Default for ModelMetadata {
    fn default() -> Self {
        Self {
            model_version: DEFAULT_MODEL_VERSION.to_string(),
            pipeline_version: DEFAULT_PIPELINE_VERSION.to_string(),
            data_provenance: DEFAULT_DATA_PROVENANCE
                .iter()
                .map(|s| s.to_string())
                .collect(),
            last_trained_at: None,
        }
    }
}

impl ModelMetadata {
    pub fn from_env_or_config() -> Self {
        // 1. Try reading config.yaml if present
        let (cfg_mv, cfg_pv, cfg_dp) = if let Ok(content) = std::fs::read_to_string("config.yaml") {
            parse_config_yaml_model_metadata(&content)
        } else {
            (None, None, None)
        };

        // 2. Env vars override config.yaml and defaults
        let model_version = std::env::var("MODEL_VERSION")
            .ok()
            .or(cfg_mv)
            .unwrap_or_else(|| DEFAULT_MODEL_VERSION.to_string());

        let pipeline_version = std::env::var("PIPELINE_VERSION")
            .ok()
            .or(cfg_pv)
            .unwrap_or_else(|| DEFAULT_PIPELINE_VERSION.to_string());

        let data_provenance = std::env::var("DATA_PROVENANCE")
            .ok()
            .map(|s| {
                s.split(',')
                    .map(|x| x.trim().to_string())
                    .filter(|x| !x.is_empty())
                    .collect()
            })
            .or(cfg_dp)
            .unwrap_or_else(|| {
                DEFAULT_DATA_PROVENANCE
                    .iter()
                    .map(|s| s.to_string())
                    .collect()
            });

        Self {
            model_version,
            pipeline_version,
            data_provenance,
            last_trained_at: None,
        }
    }

    /// Increments model version tag and stamps the current UTC time as `last_trained_at`.
    pub fn increment_version(&mut self) {
        self.last_trained_at = Some(Utc::now());

        if let Some(pos) = self.model_version.rfind('.') {
            let prefix = &self.model_version[..pos];
            let suffix = &self.model_version[pos + 1..];
            if let Ok(num) = suffix.parse::<u32>() {
                self.model_version = format!("{}.{}", prefix, num + 1);
                return;
            }
        }
        self.model_version = format!("{}.1", self.model_version);
    }
}

fn parse_config_yaml_model_metadata(
    content: &str,
) -> (Option<String>, Option<String>, Option<Vec<String>>) {
    let mut in_section = false;
    let mut model_version = None;
    let mut pipeline_version = None;
    let mut data_provenance = Vec::new();
    let mut in_provenance_list = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("model_metadata:") {
            in_section = true;
            continue;
        }
        if in_section {
            if !line.starts_with(' ') && !line.starts_with('\t') && !trimmed.is_empty() {
                break;
            }
            if trimmed.starts_with("model_version:") {
                in_provenance_list = false;
                let val = trimmed
                    .trim_start_matches("model_version:")
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'');
                model_version = Some(val.to_string());
            } else if trimmed.starts_with("pipeline_version:") {
                in_provenance_list = false;
                let val = trimmed
                    .trim_start_matches("pipeline_version:")
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'');
                pipeline_version = Some(val.to_string());
            } else if trimmed.starts_with("data_provenance:") {
                let rest = trimmed.trim_start_matches("data_provenance:").trim();
                if rest.starts_with('[') && rest.ends_with(']') {
                    in_provenance_list = false;
                    let inner = &rest[1..rest.len() - 1];
                    for item in inner.split(',') {
                        let item_clean = item.trim().trim_matches('"').trim_matches('\'');
                        if !item_clean.is_empty() {
                            data_provenance.push(item_clean.to_string());
                        }
                    }
                } else {
                    in_provenance_list = true;
                }
            } else if in_provenance_list && trimmed.starts_with('-') {
                let item = trimmed
                    .trim_start_matches('-')
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'');
                if !item.is_empty() {
                    data_provenance.push(item.to_string());
                }
            }
        }
    }

    let prov = if data_provenance.is_empty() {
        None
    } else {
        Some(data_provenance)
    };
    (model_version, pipeline_version, prov)
}

#[derive(Debug, Clone, Deserialize)]
pub struct SentimentQuery {
    pub ticker: String,
    pub date: Option<String>,
    pub language: Option<String>,
    pub as_of_utc: Option<String>,
}

/// Probability distribution across the 3 fundamental sentiment classes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct SentimentProbabilities {
    /// Probability of positive sentiment class (0.0 to 1.0)
    #[schema(example = 0.85)]
    pub positive: f32,
    /// Probability of neutral sentiment class (0.0 to 1.0)
    #[schema(example = 0.10)]
    pub neutral: f32,
    /// Probability of negative sentiment class (0.0 to 1.0)
    #[schema(example = 0.05)]
    pub negative: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct SentimentResponse {
    /// Stock ticker symbol (e.g., "AAPL", "MSFT")
    #[schema(example = "AAPL")]
    pub ticker: String,
    /// Query date or "LATEST"
    #[schema(example = "2026-08-25")]
    pub date: String,
    /// Normalized sentiment score between -1.0 (extremely bearish) and +1.0 (extremely bullish)
    #[schema(example = 0.25)]
    pub sentiment_score: f64,
    /// Classification label ("BULLISH", "BEARISH", "NEUTRAL" or "POSITIVE", "NEGATIVE")
    #[schema(example = "BULLISH")]
    pub sentiment_label: String,
    /// Model prediction confidence score between 0.0 and 1.0
    #[schema(example = 0.85)]
    pub confidence: f32,
    /// Detailed probability distribution across classes (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub probabilities: Option<SentimentProbabilities>,
    /// Point-in-time timestamp (microseconds since Unix epoch) when the signal was published
    #[schema(example = 1787940389786186u64)]
    pub signal_available_ts_us: u64,
    /// Quantitative data quality score between 0.0 and 1.0
    #[serde(default = "default_data_quality_score")]
    #[schema(example = 0.88)]
    pub data_quality_score: f32,
    /// Status or diagnostic message
    #[schema(example = "Point-in-time sentiment signal retrieved")]
    pub message: String,
    /// Model version identifier used for inference (e.g., 'finbert-v3.1.0')
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = "finbert-v3.1.0")]
    pub model_version: Option<String>,
    /// Quantitative feature engineering & processing pipeline semantic version
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = "2.0.0")]
    pub pipeline_version: Option<String>,
    /// Upstream data source lineage and provenance array
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = json!(["SEC EDGAR", "Finnhub", "Polygon"]))]
    pub data_provenance: Option<Vec<String>>,
    /// Detected or declared language of the analyzed article/text (e.g., 'english', 'spanish', 'german', 'french', 'japanese')
    #[serde(default = "default_language")]
    #[schema(example = "english")]
    pub language: String,
    /// Number of token chunks evaluated for long document inference
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = 4)]
    pub chunk_count: Option<usize>,
    /// Array of individual chunk sentiment scores
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = json!([0.25, 0.40, 0.15, 0.30]))]
    pub chunk_scores: Option<Vec<f64>>,
    /// Aggregation strategy used for multi-chunk document scoring
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = "sliding_window_confidence_weighted")]
    pub aggregation_method: Option<String>,
    /// Original publication timestamp in ISO-8601 UTC
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = "2026-08-25T14:30:00.000000Z")]
    pub published_utc: Option<String>,
    /// System ingestion timestamp in ISO-8601 UTC
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = "2026-08-25T14:30:00.050000Z")]
    pub ingested_utc: Option<String>,
    /// Database commit timestamp in ISO-8601 UTC
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = "2026-08-25T14:30:00.100000Z")]
    pub db_commit_utc: Option<String>,
    /// Point-in-time validity start timestamp in ISO-8601 UTC (SCD Type 2)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = "2026-08-25T14:30:00.100000Z")]
    pub valid_from: Option<String>,
    /// Point-in-time validity end timestamp in ISO-8601 UTC (None if current version) (SCD Type 2)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = "2026-08-26T09:15:00.000000Z")]
    pub valid_to: Option<String>,
    /// Slowly Changing Dimension revision number (starts at 1)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = 1)]
    pub revision_number: Option<i32>,
    /// True if this record represents the latest active revision
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = true)]
    pub is_current: Option<bool>,
    /// Degraded service flag indicating fallback store was queried
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = true)]
    pub degraded: Option<bool>,
    /// Underlying storage engine queried for this response ("questdb-hot", "timescale-primary", "in-memory-fallback")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = "timescale-primary")]
    pub storage: Option<String>,
    /// Diagnostic warning message if running in degraded fallback mode
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = "QuestDB unreachable, serving from TimescaleDB primary - degraded mode")]
    pub warning: Option<String>,
}

impl Default for SentimentResponse {
    fn default() -> Self {
        Self {
            ticker: "UNKNOWN".to_string(),
            date: "LATEST".to_string(),
            sentiment_score: 0.0,
            sentiment_label: "NEUTRAL".to_string(),
            confidence: 0.0,
            probabilities: None,
            signal_available_ts_us: 0,
            data_quality_score: 0.80,
            message: String::new(),
            model_version: None,
            pipeline_version: None,
            data_provenance: None,
            language: default_language(),
            chunk_count: None,
            chunk_scores: None,
            aggregation_method: None,
            published_utc: None,
            ingested_utc: None,
            db_commit_utc: None,
            valid_from: None,
            valid_to: None,
            revision_number: None,
            is_current: None,
            degraded: None,
            storage: None,
            warning: None,
        }
    }
}

pub fn default_language() -> String {
    "english".to_string()
}

fn default_data_quality_score() -> f32 {
    0.80
}

/// Compute confidence and class probability distribution from raw probabilities or heuristic score.
pub fn compute_confidence_and_probabilities(
    sentiment_score: f64,
    sentiment_label: &str,
    raw_pos: Option<f64>,
    raw_neg: Option<f64>,
    raw_neu: Option<f64>,
) -> (f32, Option<SentimentProbabilities>) {
    if let (Some(p), Some(n), Some(u)) = (raw_pos, raw_neg, raw_neu) {
        let sum = p + n + u;
        if sum > 0.01 {
            let pos = (p / sum) as f32;
            let neg = (n / sum) as f32;
            let neu = (u / sum) as f32;
            let conf = pos.max(neg).max(neu).clamp(0.0, 1.0);
            return (
                conf,
                Some(SentimentProbabilities {
                    positive: (pos * 10000.0).round() / 10000.0,
                    neutral: (neu * 10000.0).round() / 10000.0,
                    negative: (neg * 10000.0).round() / 10000.0,
                }),
            );
        }
    }

    // Heuristic fallback derived from sentiment_score and sentiment_label
    let label_upper = sentiment_label.trim().to_uppercase();
    if label_upper == "BULLISH" || label_upper == "POSITIVE" {
        let conf = (0.5 + sentiment_score.abs().min(1.0) * 0.5) as f32;
        let pos = conf;
        let neg = (1.0 - pos) * 0.3;
        let neu = 1.0 - pos - neg;
        (
            conf,
            Some(SentimentProbabilities {
                positive: (pos * 10000.0).round() / 10000.0,
                neutral: (neu * 10000.0).round() / 10000.0,
                negative: (neg * 10000.0).round() / 10000.0,
            }),
        )
    } else if label_upper == "BEARISH" || label_upper == "NEGATIVE" {
        let conf = (0.5 + sentiment_score.abs().min(1.0) * 0.5) as f32;
        let neg = conf;
        let pos = (1.0 - neg) * 0.3;
        let neu = 1.0 - neg - pos;
        (
            conf,
            Some(SentimentProbabilities {
                positive: (pos * 10000.0).round() / 10000.0,
                neutral: (neu * 10000.0).round() / 10000.0,
                negative: (neg * 10000.0).round() / 10000.0,
            }),
        )
    } else {
        let conf = (1.0 - sentiment_score.abs().min(1.0) * 0.5) as f32;
        let neu = conf.clamp(0.4, 0.95);
        let rem = (1.0 - neu) / 2.0;
        (
            conf,
            Some(SentimentProbabilities {
                positive: (rem * 10000.0).round() / 10000.0,
                neutral: (neu * 10000.0).round() / 10000.0,
                negative: (rem * 10000.0).round() / 10000.0,
            }),
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct HealthResponse {
    /// Service health status indicator
    #[schema(example = "ok")]
    pub status: String,
    /// Production version tag
    #[schema(example = "2.0.0-institutional")]
    pub version: String,
    /// Server timestamp (microseconds since Unix epoch)
    #[schema(example = 1787940389739799u64)]
    pub timestamp_us: u64,
}

/// Request query parameters for historical sentiment time series (`GET /sentiment/history`).
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct SentimentHistoryParams {
    /// Target stock ticker symbol (e.g., 'AAPL', 'NVDA', 'MSFT')
    #[schema(example = "AAPL")]
    pub ticker: String,
    /// Start date in ISO format YYYY-MM-DD
    #[schema(example = "2025-01-01")]
    pub start_date: String,
    /// End date in ISO format YYYY-MM-DD
    #[schema(example = "2025-03-31")]
    pub end_date: String,
    /// Number of records per page (default: 100, min: 1, max: 1000)
    #[schema(example = 100)]
    pub limit: Option<u32>,
    /// Number of records to skip for pagination (default: 0, min: 0)
    #[schema(example = 0)]
    pub offset: Option<u32>,
    /// Sort order by timestamp: 'asc' or 'desc' (default: 'asc')
    #[schema(example = "asc")]
    pub sort: Option<String>,
    /// Minimum data quality score filter threshold between 0.0 and 1.0 (default: 0.0)
    #[schema(example = 0.70)]
    pub min_quality: Option<f32>,
    /// Optional point-in-time as-of timestamp in ISO-8601 UTC for SCD2 historical revision filtering
    #[serde(default)]
    #[schema(example = "2025-01-15T14:30:00Z")]
    pub as_of_utc: Option<String>,
}

/// Single point-in-time historical sentiment record with microstructure metrics and data quality scoring.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct SentimentRecord {
    /// Publication timestamp in ISO-8601 UTC
    #[schema(example = "2025-01-01T14:30:00.000000Z")]
    pub published_utc: String,
    /// Stock ticker symbol
    #[schema(example = "AAPL")]
    pub ticker: String,
    /// News or filing data source
    #[schema(example = "Institutional Wire")]
    pub source: String,
    /// Headline title or filing summary
    #[schema(example = "AAPL Market Sentiment and Flow Analysis")]
    pub title: String,
    /// Point-in-time sentiment score between -1.0 and +1.0
    #[schema(example = 0.25)]
    pub sentiment_score: f64,
    /// Volume-Synchronized Probability of Toxicity (VPIN)
    #[schema(example = 0.55)]
    pub vpin: f64,
    /// Gamma Exposure (GEX) metric
    #[schema(example = 0.0)]
    pub gamma_exposure: f64,
    /// Quantitative data quality score between 0.0 and 1.0
    #[serde(default = "default_data_quality_score")]
    #[schema(example = 0.88)]
    pub data_quality_score: f32,
    /// Model version identifier used for inference
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = "finbert-v3.1.0")]
    pub model_version: Option<String>,
    /// Pipeline version identifier
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = "2.0.0")]
    pub pipeline_version: Option<String>,
    /// Upstream data source lineage
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = json!(["SEC EDGAR", "Finnhub", "Polygon"]))]
    pub data_provenance: Option<Vec<String>>,
    /// Detected language of the historical sentiment record
    #[serde(default = "default_language")]
    #[schema(example = "english")]
    pub language: String,
    /// System ingestion timestamp in ISO-8601 UTC
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = "2025-01-01T14:30:00.050000Z")]
    pub ingested_utc: Option<String>,
    /// Database commit timestamp in ISO-8601 UTC
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = "2025-01-01T14:30:00.100000Z")]
    pub db_commit_utc: Option<String>,
    /// Point-in-time validity start timestamp in ISO-8601 UTC (SCD Type 2)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = "2025-01-01T14:30:00.100000Z")]
    pub valid_from: Option<String>,
    /// Point-in-time validity end timestamp in ISO-8601 UTC (None if current version) (SCD Type 2)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = "2025-01-02T10:00:00.000000Z")]
    pub valid_to: Option<String>,
    /// Slowly Changing Dimension revision number (starts at 1)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = 1)]
    pub revision_number: Option<i32>,
    /// True if this record represents the latest active revision
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = true)]
    pub is_current: Option<bool>,
}

impl Default for SentimentRecord {
    fn default() -> Self {
        Self {
            published_utc: String::new(),
            ticker: String::new(),
            source: "Institutional Wire".to_string(),
            title: String::new(),
            sentiment_score: 0.0,
            vpin: 0.0,
            gamma_exposure: 0.0,
            data_quality_score: 0.80,
            model_version: None,
            pipeline_version: None,
            data_provenance: None,
            language: default_language(),
            ingested_utc: None,
            db_commit_utc: None,
            valid_from: None,
            valid_to: None,
            revision_number: None,
            is_current: None,
        }
    }
}

/// Response payload for historical sentiment time series query (`GET /sentiment/history`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct SentimentHistoryResponse {
    /// Target ticker
    #[schema(example = "AAPL")]
    pub ticker: String,
    /// Start date of queried range (YYYY-MM-DD)
    #[schema(example = "2025-01-01")]
    pub start_date: String,
    /// End date of queried range (YYYY-MM-DD)
    #[schema(example = "2025-03-31")]
    pub end_date: String,
    /// Number of returned records in the current page
    #[schema(example = 100)]
    pub count: usize,
    /// Total number of available records matching query filters
    #[schema(example = 150)]
    pub total: usize,
    /// Page limit applied
    #[schema(example = 100)]
    pub limit: u32,
    /// Page offset applied
    #[schema(example = 0)]
    pub offset: u32,
    /// Applied sort order ('asc' or 'desc')
    #[schema(example = "asc")]
    pub sort: String,
    /// Array of historical sentiment records
    pub records: Vec<SentimentRecord>,
    /// Model version identifier
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = "finbert-v3.1.0")]
    pub model_version: Option<String>,
    /// Pipeline version identifier
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = "2.0.0")]
    pub pipeline_version: Option<String>,
    /// Upstream data source lineage
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = json!(["SEC EDGAR", "Finnhub", "Polygon"]))]
    pub data_provenance: Option<Vec<String>>,
}

/// Request query parameters for sector sentiment aggregation (`GET /sentiment/sector`).
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct SectorSentimentParams {
    /// GICS Sector Name (e.g., 'Technology', 'Financials', 'Healthcare', 'Energy')
    #[schema(example = "Technology")]
    pub sector: String,
    /// Start date in ISO format YYYY-MM-DD
    #[schema(example = "2025-01-01")]
    pub start_date: String,
    /// End date in ISO format YYYY-MM-DD
    #[schema(example = "2025-03-31")]
    pub end_date: String,
    /// Aggregation operator: 'average', 'sum', 'count', 'median', 'weighted_average' (default: 'average')
    #[schema(example = "average")]
    pub aggregation: Option<String>,
    /// Minimum confidence filter threshold between 0.0 and 1.0 (default: 0.0)
    #[schema(example = 0.5)]
    pub min_confidence: Option<f64>,
    /// Optional point-in-time as-of timestamp in ISO-8601 UTC for SCD2 historical revision filtering
    #[serde(default)]
    #[schema(example = "2025-03-31T23:59:59Z")]
    pub as_of_utc: Option<String>,
}

/// Response payload for sector sentiment aggregation query (`GET /sentiment/sector`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct SectorSentimentResponse {
    /// Target evaluated GICS sector
    #[schema(example = "Technology")]
    pub sector: String,
    /// Start date of queried range (YYYY-MM-DD)
    #[schema(example = "2025-01-01")]
    pub start_date: String,
    /// End date of queried range (YYYY-MM-DD)
    #[schema(example = "2025-03-31")]
    pub end_date: String,
    /// Applied aggregation operator
    #[schema(example = "average")]
    pub aggregation: String,
    /// Minimum confidence threshold applied
    #[schema(example = 0.5)]
    pub min_confidence: f64,
    /// Aggregated sentiment metric value
    #[schema(example = 0.42)]
    pub value: f64,
    /// Total number of sentiment records evaluated in aggregate
    #[schema(example = 1500)]
    pub record_count: usize,
    /// Total count of distinct tickers included in aggregate
    #[schema(example = 22)]
    pub tickers_included: usize,
    /// UTC timestamp of generation
    #[schema(example = "2026-08-29T12:00:00Z")]
    pub generated_at: String,
    /// Model version identifier
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = "finbert-v3.1.0")]
    pub model_version: Option<String>,
    /// Pipeline version identifier
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = "2.0.0")]
    pub pipeline_version: Option<String>,
    /// Upstream data source lineage
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = json!(["SEC EDGAR", "Finnhub", "Polygon"]))]
    pub data_provenance: Option<Vec<String>>,
}

/// Request query parameters for batch multi-ticker sentiment lookup (`GET /sentiment/batch`).
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct BatchSentimentParams {
    /// Optional comma-separated list of stock asset tickers (max 50, e.g., 'AAPL,MSFT,NVDA')
    #[schema(example = "AAPL,MSFT,NVDA")]
    pub tickers: Option<String>,
    /// Optional custom universe UUID to retrieve tickers from
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub universe_id: Option<Uuid>,
    /// Optional historical date formatted as YYYY-MM-DD or 'LATEST'
    #[schema(example = "2026-08-25")]
    pub date: Option<String>,
    /// Optional point-in-time as-of timestamp in ISO-8601 UTC for SCD2 historical revision filtering
    #[serde(default)]
    #[schema(example = "2026-08-25T14:30:00Z")]
    pub as_of_utc: Option<String>,
}

/// Response payload for batch multi-ticker sentiment lookup (`GET /sentiment/batch`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct BatchSentimentResponse {
    /// Total count of sentiment records returned
    #[schema(example = 3)]
    pub count: usize,
    /// Array of individual asset sentiment signals
    pub results: Vec<SentimentResponse>,
    /// Model version identifier
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = "finbert-v3.1.0")]
    pub model_version: Option<String>,
    /// Pipeline version identifier
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = "2.0.0")]
    pub pipeline_version: Option<String>,
    /// Upstream data source lineage
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = json!(["SEC EDGAR", "Finnhub", "Polygon"]))]
    pub data_provenance: Option<Vec<String>>,
}

/// Request query parameters for aggregated news sentiment feed (`GET /sentiment/feed`).
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct SentimentFeedParams {
    /// Optional GICS Sector filter (e.g. 'Technology', 'Financials', 'Healthcare')
    #[serde(default)]
    #[schema(example = "Technology")]
    pub sector: Option<String>,
    /// Earliest timestamp in ISO format YYYY-MM-DD or RFC3339 (default: now - 24 hours)
    #[serde(default, alias = "start_time")]
    #[schema(example = "2025-01-01T00:00:00Z")]
    pub start_date: Option<String>,
    /// Latest timestamp in ISO format YYYY-MM-DD or RFC3339 (default: now)
    #[serde(default, alias = "end_time")]
    #[schema(example = "2025-01-02T00:00:00Z")]
    pub end_date: Option<String>,
    /// Minimum prediction confidence filter threshold between 0.0 and 1.0 (default: 0.0)
    #[serde(default)]
    #[schema(example = 0.60)]
    pub min_confidence: Option<f32>,
    /// Minimum quantitative data quality score filter between 0.0 and 1.0 (default: 0.0)
    #[serde(default)]
    #[schema(example = 0.70)]
    pub min_quality: Option<f32>,
    /// Maximum number of records per page (default: 100, min: 1, max: 1000)
    #[serde(default)]
    #[schema(example = 100)]
    pub limit: Option<u32>,
    /// Number of records to skip for pagination (default: 0, min: 0) [Deprecated: use `cursor` instead]
    #[serde(default)]
    #[schema(example = 0, deprecated)]
    pub offset: Option<u32>,
    /// Optional cursor for keyset/cursor-based pagination (RFC3339 timestamp)
    #[serde(default)]
    #[schema(example = "2026-08-29T14:30:00Z")]
    pub cursor: Option<String>,
    /// Sort order by timestamp: 'asc' or 'desc' (default: 'desc')
    #[serde(default)]
    #[schema(example = "desc")]
    pub sort: Option<String>,
    /// Optional point-in-time as-of timestamp in ISO-8601 UTC for SCD2 historical revision filtering
    #[serde(default)]
    #[schema(example = "2026-08-29T14:30:00Z")]
    pub as_of_utc: Option<String>,
}

/// Consolidated real-time news sentiment signal feed item with microstructure metrics and data quality scoring.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct SentimentFeedItem {
    /// Publication timestamp in ISO-8601 UTC
    #[schema(example = "2026-08-29T14:30:00.000000Z")]
    pub published_utc: String,
    /// Stock asset ticker symbol
    #[schema(example = "AAPL")]
    pub ticker: String,
    /// News or corporate disclosure source
    #[schema(example = "Institutional Wire")]
    pub source: String,
    /// Headline title or filing summary
    #[schema(example = "AAPL Market Sentiment and Microstructure Flow Analysis Report")]
    pub title: String,
    /// Point-in-time sentiment score between -1.0 and +1.0
    #[schema(example = 0.35)]
    pub sentiment_score: f64,
    /// Classification label ("BULLISH", "BEARISH", "NEUTRAL")
    #[schema(example = "BULLISH")]
    pub sentiment_label: String,
    /// Model prediction confidence score between 0.0 and 1.0
    #[schema(example = 0.85)]
    pub confidence: f32,
    /// Quantitative data quality score between 0.0 and 1.0
    #[serde(default = "default_data_quality_score")]
    #[schema(example = 0.88)]
    pub data_quality_score: f32,
    /// Volume-Synchronized Probability of Toxicity (VPIN)
    #[schema(example = 0.52)]
    pub vpin: f64,
    /// Gamma Exposure (GEX) metric
    #[schema(example = 125000.0)]
    pub gamma_exposure: f64,
    /// Model version identifier used for inference
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = "finbert-v3.1.0")]
    pub model_version: Option<String>,
    /// Pipeline version identifier
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = "2.0.0")]
    pub pipeline_version: Option<String>,
    /// Upstream data source lineage
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = json!(["SEC EDGAR", "Finnhub", "Polygon"]))]
    pub data_provenance: Option<Vec<String>>,
    /// Detected language of the feed item
    #[serde(default = "default_language")]
    #[schema(example = "english")]
    pub language: String,
    /// System ingestion timestamp in ISO-8601 UTC
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = "2026-08-29T14:30:00.050000Z")]
    pub ingested_utc: Option<String>,
    /// Database commit timestamp in ISO-8601 UTC
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = "2026-08-29T14:30:00.100000Z")]
    pub db_commit_utc: Option<String>,
    /// Point-in-time validity start timestamp in ISO-8601 UTC (SCD Type 2)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = "2026-08-29T14:30:00.100000Z")]
    pub valid_from: Option<String>,
    /// Point-in-time validity end timestamp in ISO-8601 UTC (None if current version) (SCD Type 2)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = "2026-08-30T09:00:00.000000Z")]
    pub valid_to: Option<String>,
    /// Slowly Changing Dimension revision number (starts at 1)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = 1)]
    pub revision_number: Option<i32>,
    /// True if this record represents the latest active revision
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = true)]
    pub is_current: Option<bool>,
}

impl Default for SentimentFeedItem {
    fn default() -> Self {
        Self {
            published_utc: String::new(),
            ticker: String::new(),
            source: "Institutional Wire".to_string(),
            title: String::new(),
            sentiment_score: 0.0,
            sentiment_label: "NEUTRAL".to_string(),
            confidence: 0.0,
            data_quality_score: 0.80,
            vpin: 0.0,
            gamma_exposure: 0.0,
            model_version: None,
            pipeline_version: None,
            data_provenance: None,
            language: default_language(),
            ingested_utc: None,
            db_commit_utc: None,
            valid_from: None,
            valid_to: None,
            revision_number: None,
            is_current: None,
        }
    }
}

/// Response payload for consolidated news sentiment aggregated feed query (`GET /sentiment/feed`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct SentimentFeedResponse {
    /// Number of returned records in the current page
    #[schema(example = 100)]
    pub count: usize,
    /// Total number of available records matching query filters
    #[schema(example = 345)]
    pub total: usize,
    /// Page limit applied
    #[schema(example = 100)]
    pub limit: u32,
    /// Page offset applied
    #[schema(example = 0)]
    pub offset: u32,
    /// Next page cursor (RFC3339 timestamp of the last record in the current page), or None if end of data
    #[serde(default)]
    #[schema(example = "2026-08-29T14:30:00.000000Z")]
    pub next_cursor: Option<String>,
    /// Applied sort order ('asc' or 'desc')
    #[schema(example = "desc")]
    pub sort: String,
    /// Optional filtered GICS sector
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "Technology")]
    pub sector: Option<String>,
    /// Start time boundary of the feed window in ISO-8601 UTC
    #[schema(example = "2026-08-28T14:30:00.000000Z")]
    pub start_time: String,
    /// End time boundary of the feed window in ISO-8601 UTC
    #[schema(example = "2026-08-29T14:30:00.000000Z")]
    pub end_time: String,
    /// Minimum confidence threshold applied
    #[schema(example = 0.0)]
    pub min_confidence: f32,
    /// Minimum data quality threshold applied
    #[schema(example = 0.0)]
    pub min_quality: f32,
    /// Array of consolidated sentiment feed items
    pub records: Vec<SentimentFeedItem>,
    /// Model version identifier
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = "finbert-v3.1.0")]
    pub model_version: Option<String>,
    /// Pipeline version identifier
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = "2.0.0")]
    pub pipeline_version: Option<String>,
    /// Upstream data source lineage
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = json!(["SEC EDGAR", "Finnhub", "Polygon"]))]
    pub data_provenance: Option<Vec<String>>,
}

/// Request query parameters for sentiment anomaly detection (`GET /sentiment/anomalies`).
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct SentimentAnomaliesParams {
    /// Optional GICS Sector filter (e.g., 'Technology', 'Financials', 'Healthcare')
    #[serde(default)]
    #[schema(example = "Technology")]
    pub sector: Option<String>,
    /// Number of days to compute baseline statistics (default: 30, min: 1, max: 90)
    #[serde(default)]
    #[schema(example = 30)]
    pub lookback_days: Option<u32>,
    /// Minimum absolute z-score required to flag an anomaly (default: 2.0, min: 1.0, max: 5.0)
    #[serde(default)]
    #[schema(example = 2.0)]
    pub zscore_threshold: Option<f64>,
    /// Minimum historical sentiment records required per ticker to compute baseline (default: 20, min: 1, max: 1000)
    #[serde(default)]
    #[schema(example = 20)]
    pub min_records: Option<usize>,
    /// Maximum number of anomaly items to return (default: 20, min: 1, max: 100)
    #[serde(default)]
    #[schema(example = 20)]
    pub limit: Option<usize>,
    /// Optional point-in-time as-of timestamp in ISO-8601 UTC for SCD2 revision filtering
    #[serde(default)]
    #[schema(example = "2026-08-29T14:30:00Z")]
    pub as_of_utc: Option<String>,
}

/// Statistically significant sentiment anomaly record for a stock asset.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct SentimentAnomalyItem {
    /// Stock asset ticker symbol
    #[schema(example = "AAPL")]
    pub ticker: String,
    /// Most recent point-in-time sentiment score (-1.0 to +1.0)
    #[schema(example = 0.85)]
    pub latest_score: f64,
    /// Baseline historical mean sentiment score
    #[schema(example = 0.22)]
    pub mean_score: f64,
    /// Baseline historical sample standard deviation
    #[schema(example = 0.21)]
    pub stddev: f64,
    /// Computed z-score deviation relative to baseline
    #[schema(example = 3.0)]
    pub zscore: f64,
    /// Anomaly classification direction: 'bullish' (z >= threshold) or 'bearish' (z <= -threshold)
    #[schema(example = "bullish")]
    pub direction: String,
    /// Publication timestamp of the most recent sentiment observation in ISO-8601 UTC
    #[schema(example = "2026-08-29T14:30:00.000000Z")]
    pub latest_timestamp: String,
    /// Total number of historical observations evaluated in baseline
    #[schema(example = 45)]
    pub record_count: usize,
    /// Model version identifier
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = "finbert-v3.1.0")]
    pub model_version: Option<String>,
    /// Pipeline version identifier
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = "2.0.0")]
    pub pipeline_version: Option<String>,
    /// Upstream data source lineage
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = json!(["SEC EDGAR", "Finnhub", "Polygon"]))]
    pub data_provenance: Option<Vec<String>>,
}

/// Response payload for sentiment anomaly detection scan (`GET /sentiment/anomalies`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct SentimentAnomaliesResponse {
    /// Number of returned anomaly items in current response
    #[schema(example = 5)]
    pub count: usize,
    /// Total number of detected anomalies matching criteria before limit
    #[schema(example = 8)]
    pub total_anomalies_detected: usize,
    /// Evaluated historical lookback window in calendar days
    #[schema(example = 30)]
    pub lookback_days: u32,
    /// Applied z-score threshold
    #[schema(example = 2.0)]
    pub zscore_threshold: f64,
    /// Applied minimum records filter threshold
    #[schema(example = 20)]
    pub min_records: usize,
    /// Optional filtered GICS sector
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "Technology")]
    pub sector: Option<String>,
    /// Total number of active tickers scanned
    #[schema(example = 22)]
    pub scanned_tickers: usize,
    /// Array of detected sentiment anomaly items ranked by absolute z-score descending
    pub items: Vec<SentimentAnomalyItem>,
    /// UTC timestamp of calculation
    #[schema(example = "2026-08-29T14:30:00.000000Z")]
    pub generated_at: String,
    /// Operational summary message
    #[schema(example = "Sentiment anomaly scan completed across 22 tickers")]
    pub message: String,
    /// Model version identifier
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = "finbert-v3.1.0")]
    pub model_version: Option<String>,
    /// Pipeline version identifier
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = "2.0.0")]
    pub pipeline_version: Option<String>,
    /// Upstream data source lineage
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = json!(["SEC EDGAR", "Finnhub", "Polygon"]))]
    pub data_provenance: Option<Vec<String>>,
}

/// Query parameters for the Sentiment Disagreement Index endpoint (`GET /sentiment/disagreement`).
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::IntoParams)]
pub struct SentimentDisagreementParams {
    /// Target equity ticker symbol (required, e.g. "AAPL", "NVDA").
    pub ticker: String,
    /// Start date of observation window in ISO YYYY-MM-DD format.
    pub start_date: String,
    /// End date of observation window in ISO YYYY-MM-DD format.
    pub end_date: String,
    /// Optional news/filing source filter (e.g. "SEC EDGAR", "Finnhub"). If omitted, includes all sources.
    pub source: Option<String>,
    /// Minimum number of sentiment records required to compute dispersion (min: 5, default: 10).
    pub min_records: Option<usize>,
    /// Statistical dispersion aggregation method ("stddev", "iqr", "mad", default: "stddev").
    pub aggregation: Option<String>,
}

/// Breakdown of sentiment statistics for a specific news or filing data provider.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct SourceBreakdown {
    /// Data source provider name (e.g., "SEC EDGAR", "Finnhub", "Bloomberg").
    #[schema(example = "SEC EDGAR")]
    pub source: String,
    /// Arithmetic mean sentiment score across records from this source.
    #[schema(example = 0.15)]
    pub mean: f64,
    /// Number of sentiment records originating from this source.
    #[schema(example = 40)]
    pub count: usize,
}

/// Response payload containing the quantified sentiment disagreement index and multi-source dispersion metrics.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct SentimentDisagreementResponse {
    /// Target equity ticker symbol.
    #[schema(example = "AAPL")]
    pub ticker: String,
    /// Start date of observation window (YYYY-MM-DD).
    #[schema(example = "2025-01-01")]
    pub start_date: String,
    /// End date of observation window (YYYY-MM-DD).
    #[schema(example = "2025-03-31")]
    pub end_date: String,
    /// Applied dispersion aggregation method ("stddev", "iqr", "mad").
    #[schema(example = "stddev")]
    pub aggregation: String,
    /// Quantified sentiment disagreement/dispersion index across sources.
    #[schema(example = 0.18)]
    pub disagreement_index: f64,
    /// Overall arithmetic mean sentiment across all analyzed records.
    #[schema(example = 0.12)]
    pub mean_sentiment: f64,
    /// Overall median sentiment across all analyzed records.
    #[schema(example = 0.10)]
    pub median_sentiment: f64,
    /// Total number of sentiment records analyzed.
    #[schema(example = 150)]
    pub record_count: usize,
    /// Number of distinct news/filing sources represented.
    #[schema(example = 5)]
    pub source_count: usize,
    /// Detailed per-source sentiment breakdown.
    pub sources_breakdown: Vec<SourceBreakdown>,
    /// ISO 8601 UTC timestamp of calculation.
    #[schema(example = "2026-08-30T10:00:00Z")]
    pub generated_at: String,
    /// Model version identifier
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = "finbert-v3.1.0")]
    pub model_version: Option<String>,
    /// Pipeline version identifier
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = "2.0.0")]
    pub pipeline_version: Option<String>,
    /// Upstream data source lineage
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = json!(["SEC EDGAR", "Finnhub", "Bloomberg"]))]
    pub data_provenance: Option<Vec<String>>,
}

/// Query parameters for Entity Sentiment Breakdown endpoint (`GET /sentiment/entities`).
#[derive(Debug, Clone, Deserialize, IntoParams, ToSchema)]
pub struct SentimentEntitiesParams {
    /// Start date filter (YYYY-MM-DD or RFC3339). Defaults to 7 days ago.
    #[schema(example = "2026-08-23")]
    pub start_date: Option<String>,
    /// End date filter (YYYY-MM-DD or RFC3339). Defaults to now.
    #[schema(example = "2026-08-30")]
    pub end_date: Option<String>,
    /// Minimum article mention count required for an entity to be included (default: 5, min: 1).
    #[schema(example = 5)]
    pub min_mentions: Option<usize>,
    /// Filter by entity type ("company", "person", "product", "location", "organization", "all"). Default "all".
    #[schema(example = "all")]
    pub entity_type: Option<String>,
    /// Maximum number of entities to return (1 to 100, default: 20).
    #[schema(example = 20)]
    pub limit: Option<usize>,
    /// Sort criteria ("avg_sentiment", "mentions", "positive_ratio", "negative_ratio"). Default "avg_sentiment".
    #[schema(example = "avg_sentiment")]
    pub sort_by: Option<String>,
}

/// Aggregated sentiment statistics for an extracted named entity.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct EntitySentimentItem {
    /// Name or text representation of the entity (e.g., "Apple", "Tim Cook").
    #[schema(example = "Apple")]
    pub entity_text: String,
    /// Categorical entity type ("company", "person", "product", "location", "organization").
    #[schema(example = "company")]
    pub entity_type: String,
    /// Average sentiment score (-1.0 to 1.0) across all mentioning news articles.
    #[schema(example = 0.42)]
    pub avg_sentiment: f64,
    /// Fraction of mentions with sentiment_score > 0.1 (0.0 to 1.0).
    #[schema(example = 0.75)]
    pub positive_ratio: f64,
    /// Fraction of mentions with sentiment_score < -0.1 (0.0 to 1.0).
    #[schema(example = 0.10)]
    pub negative_ratio: f64,
    /// Total number of news articles mentioning this entity.
    #[schema(example = 25)]
    pub mention_count: usize,
    /// Most recent date on which this entity was mentioned (YYYY-MM-DD).
    #[schema(example = "2026-08-29")]
    pub latest_mention_date: String,
    /// Model version identifier used for entity extraction & sentiment inference
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = "finbert-v3.1.0")]
    pub model_version: Option<String>,
    /// Pipeline version identifier
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = "2.0.0")]
    pub pipeline_version: Option<String>,
    /// Upstream data source lineage
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = json!(["Institutional Wire", "Bloomberg Terminal Feed"]))]
    pub data_provenance: Option<Vec<String>>,
}

/// Response payload for `GET /sentiment/entities`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct EntitySentimentResponse {
    /// Effective start date of the analysis window (YYYY-MM-DD).
    #[schema(example = "2026-08-23")]
    pub start_date: String,
    /// Effective end date of the analysis window (YYYY-MM-DD).
    #[schema(example = "2026-08-30")]
    pub end_date: String,
    /// Entity type filter applied ("all", "company", "person", "product", "location", "organization").
    #[schema(example = "all")]
    pub entity_type: String,
    /// Minimum mentions threshold applied.
    #[schema(example = 5)]
    pub min_mentions: usize,
    /// Number of aggregated entities returned in this response.
    #[schema(example = 10)]
    pub count: usize,
    /// List of aggregated entity sentiment metrics.
    pub entities: Vec<EntitySentimentItem>,
    /// ISO 8601 UTC timestamp of response generation.
    #[schema(example = "2026-08-30T12:00:00Z")]
    pub generated_at: String,
    /// Model version identifier
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = "finbert-v3.1.0")]
    pub model_version: Option<String>,
    /// Pipeline version identifier
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = "2.0.0")]
    pub pipeline_version: Option<String>,
    /// Upstream data source lineage
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = json!(["SEC EDGAR", "Finnhub", "Polygon"]))]
    pub data_provenance: Option<Vec<String>>,
}

fn default_backfill_limit() -> Option<usize> {
    Some(1000)
}

fn default_backfill_overwrite() -> Option<bool> {
    Some(false)
}

/// Request payload for `POST /sentiment/backfill`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BackfillSentimentRequest {
    /// Target stock ticker symbol (e.g., "AAPL", "MSFT", "NVDA").
    #[schema(example = "AAPL")]
    pub ticker: String,
    /// Historical backfill start date (YYYY-MM-DD).
    #[schema(example = "2025-01-01")]
    pub start_date: String,
    /// Historical backfill end date (YYYY-MM-DD).
    #[schema(example = "2025-03-31")]
    pub end_date: String,
    /// Maximum number of articles to process in this request (default: 1000, max: 10000).
    #[serde(default = "default_backfill_limit")]
    #[schema(example = 1000)]
    pub limit: Option<usize>,
    /// Whether to recompute sentiment for articles that already have scores (default: false).
    #[serde(default = "default_backfill_overwrite")]
    #[schema(example = false)]
    pub overwrite: Option<bool>,
}

/// Response payload for `POST /sentiment/backfill`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct BackfillSentimentResponse {
    /// Target stock ticker symbol.
    #[schema(example = "AAPL")]
    pub ticker: String,
    /// Historical backfill window start date (YYYY-MM-DD).
    #[schema(example = "2025-01-01")]
    pub start_date: String,
    /// Historical backfill window end date (YYYY-MM-DD).
    #[schema(example = "2025-03-31")]
    pub end_date: String,
    /// Whether existing scores were overwritten.
    #[schema(example = false)]
    pub overwrite: bool,
    /// Total number of matching candidate news articles found for ticker and date range.
    #[schema(example = 150)]
    pub total_articles_found: usize,
    /// Number of news articles successfully processed and backfilled with sentiment scores.
    #[schema(example = 150)]
    pub processed_articles: usize,
    /// Number of articles that failed processing.
    #[schema(example = 0)]
    pub failed_articles: usize,
    /// ISO 8601 UTC timestamp of backfill operation completion.
    #[schema(example = "2025-08-31T12:00:00Z")]
    pub generated_at: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_confidence_and_probabilities_with_raw() {
        let (conf, probs) = compute_confidence_and_probabilities(
            0.85,
            "BULLISH",
            Some(0.80),
            Some(0.05),
            Some(0.15),
        );
        assert_eq!(conf, 0.80);
        assert!(probs.is_some());
        let p = probs.unwrap();
        assert_eq!(p.positive, 0.80);
        assert_eq!(p.negative, 0.05);
        assert_eq!(p.neutral, 0.15);
    }

    #[test]
    fn test_compute_confidence_and_probabilities_heuristic() {
        let (conf, probs) = compute_confidence_and_probabilities(0.70, "BULLISH", None, None, None);
        assert!(conf >= 0.80);
        assert!(probs.is_some());
        let p = probs.unwrap();
        assert!((p.positive + p.neutral + p.negative - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_model_metadata_defaults() {
        let meta = ModelMetadata::default();
        assert_eq!(meta.model_version, DEFAULT_MODEL_VERSION);
        assert_eq!(meta.pipeline_version, DEFAULT_PIPELINE_VERSION);
        assert_eq!(
            meta.data_provenance,
            vec!["SEC EDGAR", "Finnhub", "Polygon"]
        );
    }

    #[test]
    fn test_model_metadata_env_overrides() {
        std::env::set_var("MODEL_VERSION", "finbert-v3.0-custom");
        std::env::set_var("PIPELINE_VERSION", "3.1.4");
        std::env::set_var("DATA_PROVENANCE", "Bloomberg, Refinitiv, SEC");

        let meta = ModelMetadata::from_env_or_config();
        assert_eq!(meta.model_version, "finbert-v3.0-custom");
        assert_eq!(meta.pipeline_version, "3.1.4");
        assert_eq!(meta.data_provenance, vec!["Bloomberg", "Refinitiv", "SEC"]);

        // Cleanup
        std::env::remove_var("MODEL_VERSION");
        std::env::remove_var("PIPELINE_VERSION");
        std::env::remove_var("DATA_PROVENANCE");
    }

    #[test]
    fn test_sentiment_response_backward_compatibility() {
        // Old payload without model metadata fields
        let legacy_json = r#"{
            "ticker": "AAPL",
            "date": "2026-08-30",
            "sentiment_score": 0.75,
            "sentiment_label": "BULLISH",
            "confidence": 0.90,
            "signal_available_ts_us": 1725024000000000,
            "data_quality_score": 0.95,
            "message": "Sentiment signal retrieved successfully"
        }"#;

        let resp: SentimentResponse = serde_json::from_str(legacy_json).unwrap();
        assert_eq!(resp.ticker, "AAPL");
        assert_eq!(resp.sentiment_score, 0.75);
        assert!(resp.model_version.is_none());
        assert!(resp.pipeline_version.is_none());
        assert!(resp.data_provenance.is_none());

        // New payload with model metadata fields
        let modern_json = r#"{
            "ticker": "AAPL",
            "date": "2026-08-30",
            "sentiment_score": 0.75,
            "sentiment_label": "BULLISH",
            "confidence": 0.90,
            "signal_available_ts_us": 1725024000000000,
            "data_quality_score": 0.95,
            "message": "Sentiment signal retrieved successfully",
            "model_version": "finbert-v3.1.0",
            "pipeline_version": "2.0.0",
            "data_provenance": ["SEC EDGAR", "Finnhub"]
        }"#;

        let modern_resp: SentimentResponse = serde_json::from_str(modern_json).unwrap();
        assert_eq!(modern_resp.model_version.as_deref(), Some("finbert-v3.1.0"));
        assert_eq!(modern_resp.pipeline_version.as_deref(), Some("2.0.0"));
        assert_eq!(
            modern_resp.data_provenance,
            Some(vec!["SEC EDGAR".to_string(), "Finnhub".to_string()])
        );
    }
}
