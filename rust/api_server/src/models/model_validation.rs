//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Model Validation & Calibration Report DTOs
//! ═══════════════════════════════════════════════════════════════════════════════

use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

/// Query parameters for GET /model-validation
#[derive(Debug, Clone, Default, Serialize, Deserialize, IntoParams, ToSchema)]
pub struct ModelValidationQuery {
    /// Optional benchmark dataset version tag to evaluate (e.g., '1.0.0')
    pub dataset_version: Option<String>,
    /// Optional flag to trigger on-the-fly dynamic recalibration computation
    pub recalibrate: Option<bool>,
}

/// Evaluation metrics for a single sentiment class (Positive, Negative, Neutral).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct PerClassMetrics {
    /// Precision score: TP / (TP + FP)
    #[schema(example = 0.89)]
    pub precision: f64,
    /// Recall / Sensitivity score: TP / (TP + FN)
    #[schema(example = 0.87)]
    pub recall: f64,
    /// Harmonic mean of precision and recall: 2 * (P * R) / (P + R)
    #[schema(example = 0.88)]
    pub f1_score: f64,
    /// Total ground-truth sample count for this class
    #[schema(example = 45)]
    pub support: usize,
}

/// Prediction distribution for a specific true ground-truth class.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct ClassConfusion {
    /// Count predicted as positive
    #[schema(example = 40)]
    pub predicted_positive: u64,
    /// Count predicted as negative
    #[schema(example = 2)]
    pub predicted_negative: u64,
    /// Count predicted as neutral
    #[schema(example = 3)]
    pub predicted_neutral: u64,
}

/// Full 3x3 Confusion Matrix across sentiment classes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct ConfusionMatrix {
    /// Predictions for ground-truth positive samples
    pub true_positive: ClassConfusion,
    /// Predictions for ground-truth negative samples
    pub true_negative: ClassConfusion,
    /// Predictions for ground-truth neutral samples
    pub true_neutral: ClassConfusion,
}

/// Aggregated multi-class classification metrics.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct ClassificationMetrics {
    /// Overall multi-class classification accuracy (0.0 to 1.0)
    #[schema(example = 0.876)]
    pub accuracy: f64,
    /// Unweighted macro-averaged precision across all 3 classes
    #[schema(example = 0.868)]
    pub macro_precision: f64,
    /// Unweighted macro-averaged recall across all 3 classes
    #[schema(example = 0.872)]
    pub macro_recall: f64,
    /// Unweighted macro-averaged F1 score across all 3 classes
    #[schema(example = 0.870)]
    pub macro_f1: f64,
    /// Per-class performance breakdown for positive sentiment
    pub positive: PerClassMetrics,
    /// Per-class performance breakdown for negative sentiment
    pub negative: PerClassMetrics,
    /// Per-class performance breakdown for neutral sentiment
    pub neutral: PerClassMetrics,
}

/// A single confidence decile bin for probability calibration analysis.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct CalibrationPoint {
    /// Decile bin index (0 to 9)
    #[schema(example = 8)]
    pub bin_index: usize,
    /// Lower confidence bound of the bin [inclusive]
    #[schema(example = 0.80)]
    pub confidence_min: f64,
    /// Upper confidence bound of the bin [exclusive, or 1.0 inclusive]
    #[schema(example = 0.90)]
    pub confidence_max: f64,
    /// Arithmetic mean of predicted model confidence probabilities within the bin
    #[schema(example = 0.854)]
    pub predicted_confidence_mean: f64,
    /// Empirical classification accuracy (fraction of correct predictions) within the bin
    #[schema(example = 0.867)]
    pub accuracy: f64,
    /// Total samples evaluated within this confidence decile
    #[schema(example = 15)]
    pub sample_count: usize,
}

/// Institutional Model Validation & Calibration Report response payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct ModelValidationResponse {
    /// Canonical model identifier
    #[schema(example = "fintext-sentiment-finbert")]
    pub model_id: String,
    /// Current production model artifact release version
    #[schema(example = "3.0.0")]
    pub model_version: String,
    /// Version tag of the ground-truth benchmark evaluation dataset
    #[schema(example = "1.0.0")]
    pub dataset_version: String,
    /// Total number of labeled financial headlines evaluated
    #[schema(example = 105)]
    pub dataset_size: usize,
    /// ISO-8601 UTC timestamp when model validation was executed
    #[schema(example = "2026-09-05T13:30:00Z")]
    pub evaluated_at: String,
    /// Comprehensive classification performance metrics
    pub metrics: ClassificationMetrics,
    /// 3x3 multi-class confusion matrix breakdown
    pub confusion_matrix: ConfusionMatrix,
    /// Decile probability calibration curve points
    pub calibration_curve: Vec<CalibrationPoint>,
    /// Multi-class Brier score evaluating probabilistic prediction accuracy
    #[schema(example = 0.182)]
    pub brier_score: f64,
    /// Expected Calibration Error (ECE) measuring confidence calibration deviation
    #[schema(example = 0.048)]
    pub expected_calibration_error: f64,
    /// Contextual operational notes and institutional validation conclusions
    pub notes: Vec<String>,
}
