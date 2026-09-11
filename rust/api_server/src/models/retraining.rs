//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Model Retraining Pipeline Models & DTOs
//! ═══════════════════════════════════════════════════════════════════════════════

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

/// Model retraining job record.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct RetrainingJob {
    /// Unique identifier of the retraining job (UUID)
    #[schema(example = "a1b2c3d4-e5f6-7890-abcd-ef1234567890")]
    pub id: Uuid,
    /// Optional organization identifier associated with the job
    #[schema(example = "550e8400-e29b-41d4-a716-446655440001")]
    pub org_id: Option<Uuid>,
    /// User identifier who created or triggered the job
    #[schema(example = "quant_macro_fund")]
    pub user_id: String,
    /// Target model type (e.g. `sentiment`, `finbert`, `minilm`)
    #[schema(example = "sentiment")]
    pub model_type: String,
    /// Trigger classification (`manual` or `scheduled`)
    #[schema(example = "manual")]
    pub trigger_type: String,
    /// Current job status (`pending`, `running`, `completed`, `failed`, `cancelled`)
    #[schema(example = "completed")]
    pub status: String,
    /// Hyperparameter and training dataset configuration payload
    pub config: serde_json::Value,
    /// UTC timestamp of job creation
    pub created_at: DateTime<Utc>,
    /// UTC timestamp when training began
    pub started_at: Option<DateTime<Utc>>,
    /// UTC timestamp when training completed, failed, or was cancelled
    pub completed_at: Option<DateTime<Utc>>,
    /// Evaluation and training performance metrics (e.g. loss, accuracy, f1_score)
    #[schema(example = json!({"loss": 0.0412, "accuracy": 0.965, "f1_score": 0.958}))]
    pub metrics: Option<serde_json::Value>,
    /// Error message description if job failed or cancellation reason
    pub error_message: Option<String>,
}

/// Request payload for creating a new model retraining job (`POST /retraining/jobs`).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateRetrainingJobRequest {
    /// Target model type (default: `sentiment`)
    #[schema(example = "sentiment")]
    pub model_type: Option<String>,
    /// Retraining trigger type (`manual` or `scheduled`, default: `manual`)
    #[schema(example = "manual")]
    pub trigger_type: Option<String>,
    /// Dataset path for fine-tuning
    #[schema(example = "data/training/sentiment_v2.jsonl")]
    pub dataset_path: Option<String>,
    /// Number of training epochs (1..=100)
    #[schema(example = 3)]
    pub epochs: Option<u32>,
    /// Training batch size (1..=512)
    #[schema(example = 32)]
    pub batch_size: Option<usize>,
    /// Learning rate (e.g. 0.00002)
    #[schema(example = 0.00002)]
    pub learning_rate: Option<f64>,
    /// Hyperparameter training configuration (e.g. learning rate, epochs, batch size)
    #[schema(example = json!({"learning_rate": 0.00002, "epochs": 3, "batch_size": 32}))]
    pub config: Option<serde_json::Value>,
}

/// Single retraining job response payload.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RetrainingJobResponse {
    /// The retraining job entity
    pub job: RetrainingJob,
    /// Status description or action confirmation message
    #[schema(example = "Retraining job created successfully and queued for execution")]
    pub message: String,
}

/// Paginated list response for model retraining jobs (`GET /retraining/jobs`).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ListRetrainingJobsResponse {
    /// List of retraining job records matching the query filters
    pub jobs: Vec<RetrainingJob>,
    /// Total count of matching retraining jobs across all pages
    #[schema(example = 12)]
    pub total: usize,
    /// Page size limit
    #[schema(example = 50)]
    pub limit: usize,
    /// Page offset
    #[schema(example = 0)]
    pub offset: usize,
}

/// Query parameters for listing retraining jobs (`GET /retraining/jobs`).
#[derive(Debug, Clone, Deserialize, IntoParams, ToSchema)]
pub struct ListRetrainingJobsQuery {
    /// Filter jobs by status (`pending`, `running`, `completed`, `failed`, `cancelled`)
    #[schema(example = "completed")]
    pub status: Option<String>,
    /// Filter jobs by model type (`sentiment`, `finbert`, `minilm`)
    #[schema(example = "sentiment")]
    pub model_type: Option<String>,
    /// Filter jobs by organization UUID (restricted to org admins or owners)
    pub org_id: Option<Uuid>,
    /// Maximum number of records to return (1-100, default: 50)
    #[schema(example = 50)]
    pub limit: Option<usize>,
    /// Number of records to skip (default: 0)
    #[schema(example = 0)]
    pub offset: Option<usize>,
}
