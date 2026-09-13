//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Model Card & Lineage Governance Models
//! ═══════════════════════════════════════════════════════════════════════════════

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Hardware infrastructure specifications for optimal model execution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct HardwareRequirements {
    /// Recommended vCPU count for concurrent thread pooling
    #[schema(example = "8 vCPU")]
    pub cpu: String,
    /// Minimum system memory in gigabytes
    #[schema(example = 4)]
    pub memory_gb: u32,
    /// Optional GPU acceleration tier
    #[schema(example = "optional_cuda_tensorrt")]
    pub gpu: String,
}

/// Release version and change log entry for model lineage tracking.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct VersionHistoryItem {
    /// Semantic version of the model artifact
    #[schema(example = "2.1.0")]
    pub version: String,
    /// Release date in ISO-8601 / YYYY-MM-DD format
    #[schema(example = "2025-08-01")]
    pub release_date: String,
    /// Architectural or training delta summary
    #[schema(example = "Sliding window chunking added")]
    pub changes: String,
}

/// Model licensing and proprietary training corpus rights information.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct LicensingInfo {
    /// Intellectual property and usage license terms
    #[schema(example = "internal_proprietary")]
    pub model_license: String,
    /// Data rights verification status
    #[schema(example = "verified_internal_use")]
    pub training_data_rights: String,
}

/// Comprehensive Model Card and Lineage Governance Report.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct ModelCardResponse {
    /// Canonical model identifier
    #[schema(example = "fintext-sentiment-finbert")]
    pub model_id: String,
    /// Human-readable model title and task description
    #[schema(example = "FinBERT (ProsusAI) Fine-Tuned for Financial Sentiment")]
    pub model_name: String,
    /// Neural network architecture family
    #[schema(example = "bert")]
    pub architecture: String,
    /// Upstream foundation model checkpoint
    #[schema(example = "ProsusAI/finbert")]
    pub base_model: String,
    /// Dataset used for domain-specific fine-tuning
    #[schema(example = "financial_phrasebank_and_fiqa")]
    pub fine_tuning_dataset: String,
    /// Downstream quantitative NLP task
    #[schema(example = "financial_sentiment_classification")]
    pub task: String,
    /// Floating-point inference precision
    #[schema(example = "FP16")]
    pub precision: String,
    /// Model weight quantization strategy
    #[schema(example = "INT8_dynamic")]
    pub quantization: String,
    /// Maximum sequence length / context window in tokens
    #[schema(example = 512)]
    pub sequence_length: u32,
    /// Document chunking and sliding window strategy
    #[schema(example = "sliding_window_overlap_16")]
    pub chunking_strategy: String,
    /// Arithmetic mean inference latency in milliseconds
    #[schema(example = 4.8)]
    pub mean_latency_ms: f64,
    /// 95th percentile inference latency in milliseconds
    #[schema(example = 8.5)]
    pub p95_latency_ms: f64,
    /// 99th percentile inference latency in milliseconds
    #[schema(example = 14.2)]
    pub p99_latency_ms: f64,
    /// Minimum and recommended hardware infrastructure
    pub hardware_requirements: HardwareRequirements,
    /// Chronological list of model versions and evolution
    pub version_history: Vec<VersionHistoryItem>,
    /// Licensing and data rights compliance terms
    pub licensing: LicensingInfo,
}

impl Default for ModelCardResponse {
    fn default() -> Self {
        Self {
            model_id: "fintext-sentiment-finbert".to_string(),
            model_name: "FinBERT (ProsusAI) Fine-Tuned for Financial Sentiment".to_string(),
            architecture: "bert".to_string(),
            base_model: "ProsusAI/finbert".to_string(),
            fine_tuning_dataset: "financial_phrasebank_and_fiqa".to_string(),
            task: "financial_sentiment_classification".to_string(),
            precision: "FP16".to_string(),
            quantization: "INT8_dynamic".to_string(),
            sequence_length: 512,
            chunking_strategy: "sliding_window_overlap_16".to_string(),
            mean_latency_ms: 4.8,
            p95_latency_ms: 8.5,
            p99_latency_ms: 14.2,
            hardware_requirements: HardwareRequirements {
                cpu: "8 vCPU".to_string(),
                memory_gb: 4,
                gpu: "optional_cuda_tensorrt".to_string(),
            },
            version_history: vec![
                VersionHistoryItem {
                    version: "3.0.0".to_string(),
                    release_date: "2026-09-05".to_string(),
                    changes: "FinBERT domain-specific model integration with INT8 quantization"
                        .to_string(),
                },
                VersionHistoryItem {
                    version: "2.1.0".to_string(),
                    release_date: "2025-08-01".to_string(),
                    changes: "Sliding window chunking added".to_string(),
                },
                VersionHistoryItem {
                    version: "2.0.0".to_string(),
                    release_date: "2025-06-15".to_string(),
                    changes: "Base MiniLM-L6-v2 fine-tuned for sentiment".to_string(),
                },
            ],
            licensing: LicensingInfo {
                model_license: "apache_2.0".to_string(),
                training_data_rights: "prosusai_finbert_open_access".to_string(),
            },
        }
    }
}

impl ModelCardResponse {
    /// Load ModelCardResponse from `config/config.yaml`, `config.yaml`, or env overrides, falling back to defaults.
    pub fn from_config_or_defaults() -> Self {
        let mut card = Self::default();

        // 1. Try reading config file if available
        let config_candidates = [
            std::env::var("CONFIG_PATH").unwrap_or_default(),
            "config/config.yaml".to_string(),
            "config.yaml".to_string(),
            "../config/config.yaml".to_string(),
        ];

        for path in &config_candidates {
            if path.is_empty() {
                continue;
            }
            if let Ok(content) = std::fs::read_to_string(path) {
                if let Some(parsed) = parse_model_card_from_yaml(&content) {
                    card = parsed;
                    break;
                }
            }
        }

        // 2. Dynamic check for fine-tuned model presence in models/finbert-finetuned
        if is_finetuned_model_present() {
            card.model_id = "fintext-sentiment-finbert-finetuned".to_string();
            card.model_name = "FinBERT Fine-Tuned for FinText Financial Sentiment".to_string();
            card.fine_tuning_dataset = "financial_phrasebank_fiqa_and_fintext_corpus".to_string();
            let ft_version = VersionHistoryItem {
                version: "3.1.0".to_string(),
                release_date: "2026-09-05".to_string(),
                changes: "Domain-specific fine-tuning on Financial PhraseBank, FiQA, and FinText SEC/earnings corpus with dynamic INT8 quantization".to_string(),
            };
            if !card.version_history.iter().any(|v| v.version == "3.1.0") {
                card.version_history.insert(0, ft_version);
            }
        }

        // 3. Allow environment variable overrides
        if let Ok(val) = std::env::var("MODEL_CARD_ID") {
            card.model_id = val;
        }
        if let Ok(val) = std::env::var("MODEL_CARD_NAME") {
            card.model_name = val;
        }
        if let Ok(val) = std::env::var("MODEL_CARD_PRECISION") {
            card.precision = val;
        }
        if let Ok(val) = std::env::var("MODEL_CARD_QUANTIZATION") {
            card.quantization = val;
        }

        card
    }
}

/// Helper function to detect presence of fine-tuned FinBERT ONNX model in workspace.
pub fn is_finetuned_model_present() -> bool {
    if std::env::var("FINBERT_FINETUNED")
        .map(|v| v == "1" || v.to_lowercase() == "true")
        .unwrap_or(false)
    {
        return true;
    }
    if let Ok(p) = std::env::var("FINBERT_MODEL_DIR") {
        let pb = std::path::PathBuf::from(p);
        if pb.join("finbert.onnx").exists() || pb.join("model.onnx").exists() {
            return true;
        }
    }
    let candidates = [
        "models/finbert-finetuned/finbert.onnx",
        "../models/finbert-finetuned/finbert.onnx",
        "../../models/finbert-finetuned/finbert.onnx",
        "models/finbert-finetuned/model.onnx",
        "models/finbert-finetuned/model_static.onnx",
    ];
    candidates.iter().any(|p| std::path::Path::new(p).exists())
}

/// Helper function to parse model_card block from raw YAML text
fn parse_model_card_from_yaml(content: &str) -> Option<ModelCardResponse> {
    let mut in_model_card = false;
    let mut in_hw = false;
    let mut in_licensing = false;
    let mut in_version_history = false;
    let mut current_version_item: Option<VersionHistoryItem> = None;
    let mut version_history = Vec::new();

    let mut card = ModelCardResponse::default();
    let mut found_card_block = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') || trimmed.is_empty() {
            continue;
        }

        let indent = line.len() - line.trim_start().len();

        if indent == 0 {
            if trimmed.starts_with("model_card:") {
                in_model_card = true;
                found_card_block = true;
                in_hw = false;
                in_licensing = false;
                in_version_history = false;
            } else {
                in_model_card = false;
                in_hw = false;
                in_licensing = false;
                in_version_history = false;
            }
            continue;
        }

        if !in_model_card {
            continue;
        }

        if indent == 2 {
            if let Some(item) = current_version_item.take() {
                version_history.push(item);
            }
            in_version_history = false;
            in_hw = false;
            in_licensing = false;

            if trimmed.starts_with("hardware_requirements:") {
                in_hw = true;
                continue;
            }
            if trimmed.starts_with("version_history:") {
                in_version_history = true;
                version_history.clear();
                continue;
            }
            if trimmed.starts_with("licensing:") {
                in_licensing = true;
                continue;
            }

            if let Some((k, v)) = parse_yaml_kv(trimmed) {
                match k {
                    "model_id" => card.model_id = v.to_string(),
                    "model_name" => card.model_name = v.to_string(),
                    "architecture" => card.architecture = v.to_string(),
                    "base_model" => card.base_model = v.to_string(),
                    "fine_tuning_dataset" => card.fine_tuning_dataset = v.to_string(),
                    "task" => card.task = v.to_string(),
                    "precision" => card.precision = v.to_string(),
                    "quantization" => card.quantization = v.to_string(),
                    "sequence_length" => {
                        if let Ok(seq) = v.parse::<u32>() {
                            card.sequence_length = seq;
                        }
                    }
                    "chunking_strategy" => card.chunking_strategy = v.to_string(),
                    "mean_latency_ms" => {
                        if let Ok(lat) = v.parse::<f64>() {
                            card.mean_latency_ms = lat;
                        }
                    }
                    "p95_latency_ms" => {
                        if let Ok(lat) = v.parse::<f64>() {
                            card.p95_latency_ms = lat;
                        }
                    }
                    "p99_latency_ms" => {
                        if let Ok(lat) = v.parse::<f64>() {
                            card.p99_latency_ms = lat;
                        }
                    }
                    _ => {}
                }
            }
        } else if indent == 4 {
            if in_hw {
                if let Some((k, v)) = parse_yaml_kv(trimmed) {
                    match k {
                        "cpu" => card.hardware_requirements.cpu = v.to_string(),
                        "memory_gb" => {
                            if let Ok(mem) = v.parse::<u32>() {
                                card.hardware_requirements.memory_gb = mem;
                            }
                        }
                        "gpu" => card.hardware_requirements.gpu = v.to_string(),
                        _ => {}
                    }
                }
            } else if in_licensing {
                if let Some((k, v)) = parse_yaml_kv(trimmed) {
                    match k {
                        "model_license" => card.licensing.model_license = v.to_string(),
                        "training_data_rights" => {
                            card.licensing.training_data_rights = v.to_string()
                        }
                        _ => {}
                    }
                }
            } else if in_version_history {
                if trimmed.starts_with("- ") {
                    if let Some(item) = current_version_item.take() {
                        version_history.push(item);
                    }
                    let item_str = trimmed.trim_start_matches("- ").trim();
                    let mut new_item = VersionHistoryItem {
                        version: String::new(),
                        release_date: String::new(),
                        changes: String::new(),
                    };
                    if let Some((k, v)) = parse_yaml_kv(item_str) {
                        match k {
                            "version" => new_item.version = v.to_string(),
                            "release_date" => new_item.release_date = v.to_string(),
                            "changes" => new_item.changes = v.to_string(),
                            _ => {}
                        }
                    }
                    current_version_item = Some(new_item);
                }
            }
        } else if indent == 6 && in_version_history {
            if let Some(ref mut item) = current_version_item {
                if let Some((k, v)) = parse_yaml_kv(trimmed) {
                    match k {
                        "version" => item.version = v.to_string(),
                        "release_date" => item.release_date = v.to_string(),
                        "changes" => item.changes = v.to_string(),
                        _ => {}
                    }
                }
            }
        }
    }

    if let Some(item) = current_version_item.take() {
        version_history.push(item);
    }
    if !version_history.is_empty() {
        card.version_history = version_history;
    }

    if found_card_block {
        Some(card)
    } else {
        None
    }
}

fn parse_yaml_kv(line: &str) -> Option<(&str, &str)> {
    if let Some(idx) = line.find(':') {
        let k = line[..idx].trim();
        let mut v = line[idx + 1..].trim();
        if (v.starts_with('"') && v.ends_with('"')) || (v.starts_with('\'') && v.ends_with('\'')) {
            if v.len() >= 2 {
                v = &v[1..v.len() - 1];
            }
        }
        Some((k, v))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_card_default_values() {
        let card = ModelCardResponse::default();
        assert_eq!(card.model_id, "fintext-sentiment-finbert");
        assert_eq!(
            card.model_name,
            "FinBERT (ProsusAI) Fine-Tuned for Financial Sentiment"
        );
        assert_eq!(card.architecture, "bert");
        assert_eq!(card.base_model, "ProsusAI/finbert");
        assert_eq!(card.precision, "FP16");
        assert_eq!(card.quantization, "INT8_dynamic");
        assert_eq!(card.sequence_length, 512);
        assert_eq!(card.chunking_strategy, "sliding_window_overlap_16");
        assert_eq!(card.mean_latency_ms, 4.8);
        assert_eq!(card.hardware_requirements.cpu, "8 vCPU");
        assert_eq!(card.hardware_requirements.memory_gb, 4);
        assert_eq!(card.version_history.len(), 3);
        assert_eq!(card.licensing.model_license, "apache_2.0");
    }

    #[test]
    fn test_parse_model_card_from_yaml() {
        let yaml_sample = r#"
model_card:
  model_id: "custom-sentiment-v3"
  model_name: "Custom Model"
  architecture: "transformer_encoder"
  base_model: "ProsusAI/finbert"
  fine_tuning_dataset: "custom_dataset"
  task: "financial_sentiment_classification"
  precision: "FP32"
  quantization: "none"
  sequence_length: 64
  chunking_strategy: "sliding_window_overlap_16"
  mean_latency_ms: 1.15
  p95_latency_ms: 1.8
  p99_latency_ms: 2.2
  hardware_requirements:
    cpu: "16 vCPU"
    memory_gb: 8
    gpu: "cuda_rtx_4090"
  version_history:
    - version: "3.0.0"
      release_date: "2026-01-01"
      changes: "Major version 3 release"
  licensing:
    model_license: "mit"
    training_data_rights: "public_domain"
"#;

        let parsed =
            parse_model_card_from_yaml(yaml_sample).expect("Should parse YAML successfully");
        assert_eq!(parsed.model_id, "custom-sentiment-v3");
        assert_eq!(parsed.model_name, "Custom Model");
        assert_eq!(parsed.precision, "FP32");
        assert_eq!(parsed.quantization, "none");
        assert_eq!(parsed.sequence_length, 64);
        assert_eq!(parsed.chunking_strategy, "sliding_window_overlap_16");
        assert_eq!(parsed.mean_latency_ms, 1.15);
        assert_eq!(parsed.hardware_requirements.cpu, "16 vCPU");
        assert_eq!(parsed.hardware_requirements.memory_gb, 8);
        assert_eq!(parsed.hardware_requirements.gpu, "cuda_rtx_4090");
        assert_eq!(parsed.version_history.len(), 1);
        assert_eq!(parsed.version_history[0].version, "3.0.0");
        assert_eq!(parsed.licensing.model_license, "mit");
    }
}
