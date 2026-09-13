//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Native Rust ONNX Runtime Sentiment Engine
//! ═══════════════════════════════════════════════════════════════════════════════
//!
//! Ultra-low-latency FinBERT sentiment scoring (FinBERT INT8 primary with base FinBERT fallback),
//! TensorRT INT8 / FP16 GPU acceleration, and multi-threaded CPU fallback.
//! ═══════════════════════════════════════════════════════════════════════════════

use once_cell::sync::Lazy;
use ort::ep::{TensorRT, CUDA};
use ort::session::builder::GraphOptimizationLevel;
use ort::session::Session;
use ort::value::Value;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tokenizers::Tokenizer;
use tracing::{info, warn};

/// 20 representative financial calibration sentences across earnings, M&A, macro, and SEC filings.
pub const INT8_CALIBRATION_CORPUS: &[&str] = &[
    "Apple Inc. reported quarterly revenue of $94.9 billion, up 6 percent year over year.",
    "Company files for Chapter 11 bankruptcy as revenue collapses amid massive debt crisis.",
    "Federal Reserve holds interest rates steady as inflation approaches target 2 percent.",
    "NVIDIA surges 8% on record data center revenue and exceptional Blackwell GPU demand.",
    "Microsoft Cloud revenue exceeds expectations driven by enterprise Azure AI adoption.",
    "Tesla gross margins decline following aggressive electric vehicle price cuts across global markets.",
    "Amazon reports strong holiday operating income growth and accelerated AWS expansion.",
    "JPMorgan Chase delivers record net interest income despite macroeconomic volatility.",
    "Alphabet announces quarterly dividend program and expanded $70 billion share repurchase authorization.",
    "Pfizer reduces annual revenue guidance due to sharply lower demand for vaccine portfolio.",
    "Saudi Aramco announces strategic multibillion investment in downstream petrochemical refining.",
    "Goldman Sachs investment banking fees surge 21% following recovery in debt underwriting.",
    "Boeing halts aircraft deliveries pending federal aviation regulatory safety investigation.",
    "Meta Platforms increases full-year capital expenditure forecast for advanced AI infrastructure.",
    "ExxonMobil completes $60 billion acquisition of Pioneer Natural Resources expanding Permian footprint.",
    "Broadcom raises annual AI revenue guidance to $11 billion on custom silicon demand.",
    "Taiwan Semiconductor reports 30% jump in monthly sales on advanced node semiconductor orders.",
    "Berkshire Hathaway cash reserves climb to record $277 billion as Warren Buffett trims Apple stake.",
    "Eli Lilly raises full-year sales outlook by $3 billion powered by blockbuster diabetes treatments.",
    "Walmart raises fiscal year net sales outlook citing strong consumer spending across grocery and digital.",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SentimentOutput {
    pub sentiment_score: f64,
    pub sentiment_label: String,
    pub prob_positive: f64,
    pub prob_negative: f64,
    pub prob_neutral: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chunk_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chunk_scores: Option<Vec<f64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aggregation_method: Option<String>,
}

impl Default for SentimentOutput {
    fn default() -> Self {
        Self {
            sentiment_score: 0.0,
            sentiment_label: "NEUTRAL".to_string(),
            prob_positive: 0.33,
            prob_negative: 0.33,
            prob_neutral: 0.34,
            chunk_count: None,
            chunk_scores: None,
            aggregation_method: None,
        }
    }
}

impl SentimentOutput {
    pub fn new(
        sentiment_score: f64,
        sentiment_label: String,
        prob_positive: f64,
        prob_negative: f64,
        prob_neutral: f64,
    ) -> Self {
        Self {
            sentiment_score,
            sentiment_label,
            prob_positive,
            prob_negative,
            prob_neutral,
            chunk_count: None,
            chunk_scores: None,
            aggregation_method: None,
        }
    }

    pub fn with_chunks(
        sentiment_score: f64,
        sentiment_label: String,
        prob_positive: f64,
        prob_negative: f64,
        prob_neutral: f64,
        chunk_count: usize,
        chunk_scores: Vec<f64>,
        aggregation_method: String,
    ) -> Self {
        Self {
            sentiment_score,
            sentiment_label,
            prob_positive,
            prob_negative,
            prob_neutral,
            chunk_count: Some(chunk_count),
            chunk_scores: Some(chunk_scores),
            aggregation_method: Some(aggregation_method),
        }
    }

    /// Access the model confidence (maximum class probability).
    pub fn confidence(&self) -> f64 {
        self.prob_positive
            .max(self.prob_negative)
            .max(self.prob_neutral)
    }
}

/// Helper function to pad/truncate token IDs and attention mask vectors to an exact fixed length with specific pad ID.
pub fn pad_to_fixed_len(ids: &mut Vec<i64>, mask: &mut Vec<i64>, pad_id: i64, fixed_len: usize) {
    if ids.len() < fixed_len {
        ids.resize(fixed_len, pad_id);
    } else if ids.len() > fixed_len {
        ids.truncate(fixed_len);
    }
    if mask.len() < fixed_len {
        mask.resize(fixed_len, 0); // 0 = masked attention for padding
    } else if mask.len() > fixed_len {
        mask.truncate(fixed_len);
    }
}

/// Helper function to pad/truncate token IDs and attention mask vectors to exactly 128 elements with specific pad ID.
pub fn pad_to_128_with_token(ids: &mut Vec<i64>, mask: &mut Vec<i64>, pad_id: i64) {
    pad_to_fixed_len(ids, mask, pad_id, 128);
}

/// Helper function to pad/truncate token IDs and attention mask vectors to exactly 128 elements (default pad 0).
pub fn pad_to_128(ids: &mut Vec<i64>, mask: &mut Vec<i64>) {
    pad_to_fixed_len(ids, mask, 0, 128);
}

pub struct OnnxSentimentPipeline {
    session: Mutex<Session>,
    tokenizer: Tokenizer,
    provider: String,
    pad_token_id: i64,
    seq_len: usize,
    has_token_type_ids: bool,
}

impl OnnxSentimentPipeline {
    /// Access the active execution provider name ("TensorRT", "CUDA", or "CPU").
    pub fn provider(&self) -> &str {
        &self.provider
    }

    /// Access the active sequence length.
    pub fn seq_len(&self) -> usize {
        self.seq_len
    }

    /// Whether the model expects token_type_ids input.
    pub fn has_token_type_ids(&self) -> bool {
        self.has_token_type_ids
    }

    /// Load the FinBERT model with tiered Execution Provider fallback (TensorRT INT8/FP16 -> CUDA -> CPU).
    pub fn load_from_paths(model_path: &Path, tokenizer_path: &Path) -> Result<Self, String> {
        if let Ok(dll_path) = find_onnxruntime_dll() {
            info!(
                "Initializing ONNX Runtime from dynamic library: {:?}",
                dll_path
            );
            if let Ok(builder) = ort::init_from(dll_path.to_string_lossy().to_string()) {
                let _ = builder.commit();
            }
        } else {
            let _ = ort::init().commit();
        }

        info!("Loading native ONNX sentiment model from: {:?}", model_path);
        let (session, provider) = create_session_with_fallbacks(model_path)?;
        info!("[ONNX Runtime] Active execution provider: {}", provider);

        let actual_tokenizer_path = if tokenizer_path.is_dir() {
            tokenizer_path.join("tokenizer.json")
        } else {
            tokenizer_path.to_path_buf()
        };

        info!("Loading tokenizer from: {:?}", actual_tokenizer_path);
        let mut tokenizer = Tokenizer::from_file(&actual_tokenizer_path).map_err(|e| {
            format!(
                "Failed to load tokenizer from {:?}: {}",
                actual_tokenizer_path, e
            )
        })?;

        // Determine correct pad token id (<pad> for RoBERTa = 1, [PAD] for BERT = 0)
        let pad_token_id: i64 = tokenizer
            .token_to_id("<pad>")
            .or_else(|| tokenizer.token_to_id("[PAD]"))
            .map(|id| id as i64)
            .unwrap_or(0);

        // Detect sequence length: 512 for FinBERT
        let seq_len: usize = if let Some(custom_seq) = env::var("SENTIMENT_MAX_TOKENS")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
        {
            custom_seq
        } else {
            512
        };

        let has_token_type_ids = session
            .inputs()
            .iter()
            .any(|i| i.name() == "token_type_ids");

        // Disable hard tokenizer truncation & padding so the tokenizer encodes the full document cleanly for sliding window chunking
        let _ = tokenizer.with_padding(None);
        let _ = tokenizer.with_truncation(None);

        info!(
            "[ONNX Runtime] Configured model sequence length: {} tokens (pad_token_id={}, token_type_ids={})",
            seq_len, pad_token_id, has_token_type_ids
        );

        Ok(Self {
            session: Mutex::new(session),
            tokenizer,
            provider,
            pad_token_id,
            seq_len,
            has_token_type_ids,
        })
    }

    /// Helper to run ONNX inference on a slice of token IDs using an already locked session.
    fn run_inference_on_slice(
        session: &mut Session,
        input_ids_raw: &mut Vec<i64>,
        attention_mask_raw: &mut Vec<i64>,
        pad_token_id: i64,
        seq_len: usize,
        has_token_type_ids: bool,
    ) -> Result<(f64, f64, f64, f64, String), String> {
        pad_to_fixed_len(input_ids_raw, attention_mask_raw, pad_token_id, seq_len);

        let input_ids_val = Value::from_array(([1, seq_len], input_ids_raw.clone()))
            .map_err(|e| format!("Failed to create input_ids Value: {}", e))?;
        let attention_mask_val = Value::from_array(([1, seq_len], attention_mask_raw.clone()))
            .map_err(|e| format!("Failed to create attention_mask Value: {}", e))?;

        let outputs = if has_token_type_ids {
            let token_type_ids_val = Value::from_array(([1, seq_len], vec![0i64; seq_len]))
                .map_err(|e| format!("Failed to create token_type_ids Value: {}", e))?;
            let inputs = ort::inputs![
                "input_ids" => input_ids_val,
                "attention_mask" => attention_mask_val,
                "token_type_ids" => token_type_ids_val,
            ];
            session.run(inputs)
        } else {
            let inputs = ort::inputs![
                "input_ids" => input_ids_val,
                "attention_mask" => attention_mask_val,
            ];
            session.run(inputs)
        }
        .map_err(|e| format!("ONNX inference run error: {}", e))?;

        let logits_val = outputs
            .get("logits")
            .ok_or_else(|| "Missing 'logits' output in ONNX model".to_string())?;

        let (shape, logits_data) = logits_val
            .try_extract_tensor::<f32>()
            .map_err(|e| format!("Failed to extract logits tensor: {}", e))?;

        if logits_data.len() < 3 {
            return Err(format!(
                "Expected >= 3 logits for [pos, neg, neu], got shape: {:?}",
                shape
            ));
        }

        let logits = [logits_data[0], logits_data[1], logits_data[2]];

        // Numerically stable Softmax
        let max_logit = logits[0].max(logits[1]).max(logits[2]);
        let exp_pos = ((logits[0] - max_logit) as f64).exp();
        let exp_neg = ((logits[1] - max_logit) as f64).exp();
        let exp_neu = ((logits[2] - max_logit) as f64).exp();
        let sum_exp = exp_pos + exp_neg + exp_neu;

        let prob_positive = (exp_pos / sum_exp * 10000.0).round() / 10000.0;
        let prob_negative = (exp_neg / sum_exp * 10000.0).round() / 10000.0;
        let prob_neutral = (exp_neu / sum_exp * 10000.0).round() / 10000.0;

        let sentiment_score = ((prob_positive - prob_negative) * 10000.0).round() / 10000.0;

        let sentiment_label = if prob_positive > prob_negative && prob_positive > prob_neutral {
            "POSITIVE".to_string()
        } else if prob_negative > prob_positive && prob_negative > prob_neutral {
            "NEGATIVE".to_string()
        } else {
            "NEUTRAL".to_string()
        };

        Ok((
            sentiment_score,
            prob_positive,
            prob_negative,
            prob_neutral,
            sentiment_label,
        ))
    }

    /// Computes sentiment classification for the provided text.
    /// Supports both single-sequence fast path (<= seq_len tokens) and sliding window
    /// chunking for long documents (> seq_len tokens) with confidence-weighted aggregation.
    pub fn analyze(&self, text: &str) -> Result<SentimentOutput, String> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Err("Cannot compute sentiment for empty or whitespace-only text.".to_string());
        }

        // 1. Tokenize input text without truncation
        let encoding = self
            .tokenizer
            .encode(trimmed, true)
            .map_err(|e| format!("Tokenization error: {}", e))?;

        let raw_ids = encoding.get_ids();
        let raw_mask = encoding.get_attention_mask();

        let mut all_ids: Vec<i64> = Vec::with_capacity(raw_ids.len());
        let mut all_mask: Vec<i64> = Vec::with_capacity(raw_ids.len());

        for (i, &id) in raw_ids.iter().enumerate() {
            let mask = raw_mask.get(i).copied().unwrap_or(1);
            if mask != 0 {
                all_ids.push(id as i64);
                all_mask.push(mask as i64);
            }
        }

        if all_ids.is_empty() {
            return Err("Tokenization yielded 0 tokens.".to_string());
        }

        let total_tokens = all_ids.len();

        let chunking_enabled = env::var("SENTIMENT_CHUNKING_ENABLED")
            .map(|v| v != "0" && v.to_lowercase() != "false")
            .unwrap_or(true);

        let mut session_guard = self.session.lock().map_err(|e| e.to_string())?;

        // Fast path for short texts (<= seq_len) or if chunking is disabled
        if total_tokens <= self.seq_len || !chunking_enabled {
            let mut input_ids_raw: Vec<i64> = all_ids.into_iter().take(self.seq_len).collect();
            let mut attention_mask_raw: Vec<i64> =
                all_mask.into_iter().take(self.seq_len).collect();

            let (sentiment_score, prob_positive, prob_negative, prob_neutral, sentiment_label) =
                Self::run_inference_on_slice(
                    &mut session_guard,
                    &mut input_ids_raw,
                    &mut attention_mask_raw,
                    self.pad_token_id,
                    self.seq_len,
                    self.has_token_type_ids,
                )?;

            return Ok(SentimentOutput {
                sentiment_score,
                sentiment_label,
                prob_positive,
                prob_negative,
                prob_neutral,
                chunk_count: None,
                chunk_scores: None,
                aggregation_method: None,
            });
        }

        // Sliding window chunking path for long documents (> seq_len tokens)
        let overlap = env::var("SENTIMENT_CHUNK_OVERLAP")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(8)
            .min(self.seq_len.saturating_sub(1));

        let max_tokens = self.seq_len;
        let stride = max_tokens.saturating_sub(overlap).max(1);

        let mut chunk_scores = Vec::new();
        let mut chunk_weights = Vec::new();
        let mut chunk_probs_pos = Vec::new();
        let mut chunk_probs_neg = Vec::new();
        let mut chunk_probs_neu = Vec::new();

        let mut start = 0;
        while start < total_tokens {
            let end = (start + max_tokens).min(total_tokens);
            let slice = &all_ids[start..end];

            let mut chunk_ids = slice.to_vec();
            let mut chunk_mask = vec![1i64; chunk_ids.len()];

            let (score, p_pos, p_neg, p_neu, _) = Self::run_inference_on_slice(
                &mut session_guard,
                &mut chunk_ids,
                &mut chunk_mask,
                self.pad_token_id,
                self.seq_len,
                self.has_token_type_ids,
            )?;

            // Weight chunk by the confidence of its primary prediction
            let confidence = p_pos.max(p_neg).max(p_neu).max(0.01);

            chunk_scores.push(score);
            chunk_weights.push(confidence);
            chunk_probs_pos.push(p_pos);
            chunk_probs_neg.push(p_neg);
            chunk_probs_neu.push(p_neu);

            if end >= total_tokens {
                break;
            }
            start += stride;
        }

        if chunk_scores.is_empty() {
            return Err("No chunks were processed.".to_string());
        }

        let chunk_count = chunk_scores.len();
        let total_weight: f64 = chunk_weights.iter().sum();

        let (aggregated_score, agg_pos, agg_neg, agg_neu) = if total_weight > 0.0 {
            let w_score: f64 = chunk_scores
                .iter()
                .zip(chunk_weights.iter())
                .map(|(&s, &w)| s * w)
                .sum::<f64>()
                / total_weight;

            let w_pos: f64 = chunk_probs_pos
                .iter()
                .zip(chunk_weights.iter())
                .map(|(&p, &w)| p * w)
                .sum::<f64>()
                / total_weight;

            let w_neg: f64 = chunk_probs_neg
                .iter()
                .zip(chunk_weights.iter())
                .map(|(&p, &w)| p * w)
                .sum::<f64>()
                / total_weight;

            let w_neu: f64 = chunk_probs_neu
                .iter()
                .zip(chunk_weights.iter())
                .map(|(&p, &w)| p * w)
                .sum::<f64>()
                / total_weight;

            (
                (w_score * 10000.0).round() / 10000.0,
                (w_pos * 10000.0).round() / 10000.0,
                (w_neg * 10000.0).round() / 10000.0,
                (w_neu * 10000.0).round() / 10000.0,
            )
        } else {
            let avg_score = chunk_scores.iter().sum::<f64>() / (chunk_count as f64);
            let avg_pos = chunk_probs_pos.iter().sum::<f64>() / (chunk_count as f64);
            let avg_neg = chunk_probs_neg.iter().sum::<f64>() / (chunk_count as f64);
            let avg_neu = chunk_probs_neu.iter().sum::<f64>() / (chunk_count as f64);
            (
                (avg_score * 10000.0).round() / 10000.0,
                (avg_pos * 10000.0).round() / 10000.0,
                (avg_neg * 10000.0).round() / 10000.0,
                (avg_neu * 10000.0).round() / 10000.0,
            )
        };

        let sentiment_label = if aggregated_score > 0.10 {
            "POSITIVE".to_string()
        } else if aggregated_score < -0.10 {
            "NEGATIVE".to_string()
        } else {
            "NEUTRAL".to_string()
        };

        Ok(SentimentOutput {
            sentiment_score: aggregated_score,
            sentiment_label,
            prob_positive: agg_pos,
            prob_negative: agg_neg,
            prob_neutral: agg_neu,
            chunk_count: Some(chunk_count),
            chunk_scores: Some(chunk_scores),
            aggregation_method: Some("sliding_window_confidence_weighted".to_string()),
        })
    }
}

/// Creates an ONNX Runtime session with tiered execution provider fallback.
fn create_session_with_fallbacks(model_path: &Path) -> Result<(Session, String), String> {
    let trt_cache_path = env::var("ORT_TENSORRT_CACHE_PATH").unwrap_or_else(|_| {
        if let Some(parent) = model_path.parent() {
            parent
                .join("trt_cache_static")
                .to_string_lossy()
                .to_string()
        } else {
            "models/finbert-finetuned/trt_cache_static".to_string()
        }
    });
    let _ = fs::create_dir_all(&trt_cache_path);

    let calib_table_path = env::var("ORT_TENSORRT_CALIBRATION_TABLE").unwrap_or_else(|_| {
        if let Some(parent) = model_path.parent() {
            parent
                .join("trt_calibration.cache")
                .to_string_lossy()
                .to_string()
        } else {
            "models/finbert-finetuned/trt_calibration.cache".to_string()
        }
    });

    let workspace_size_mb = env::var("ORT_TENSORRT_WORKSPACE_SIZE_MB")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(512); // Safe 512MB workspace budget for 4GB RTX 2050

    let device_id = env::var("ORT_DEVICE_ID")
        .ok()
        .and_then(|v| v.parse::<i32>().ok())
        .unwrap_or(0);

    let force_ep = env::var("ORT_EXECUTION_PROVIDER")
        .unwrap_or_else(|_| "auto".to_string())
        .to_lowercase();

    let int8_enabled = env::var("ORT_INT8_ENABLE")
        .map(|v| v != "0" && v.to_lowercase() != "false")
        .unwrap_or(false); // Clean FP16 precision mode for sub-millisecond inference

    // ── Tier 1: TensorRT Execution Provider with Static Graph Acceleration ───
    if force_ep == "auto" || force_ep == "tensorrt" {
        info!(
            "[ONNX Runtime] Attempting TensorRT Execution Provider initialization (device_id={}, workspace={}MB, fp16=true, int8={}, cache='{}')",
            device_id, workspace_size_mb, int8_enabled, trt_cache_path
        );

        let mut trt = TensorRT::default()
            .with_device_id(device_id)
            .with_max_workspace_size(workspace_size_mb * 1024 * 1024)
            .with_engine_cache(true)
            .with_engine_cache_path(&trt_cache_path)
            .with_fp16(true);

        if int8_enabled {
            trt = trt
                .with_int8(true)
                .with_int8_calibration_table_name(&calib_table_path)
                .with_int8_use_native_calibration_table(true);
        }

        let cuda = CUDA::default().with_device_id(device_id);

        let trt_res = Session::builder()
            .map_err(|e| e.to_string())
            .and_then(|b| {
                b.with_execution_providers([trt.build(), cuda.build()])
                    .map_err(|e| e.to_string())
            })
            .and_then(|b| {
                b.with_optimization_level(GraphOptimizationLevel::Level3)
                    .map_err(|e| e.to_string())
            })
            .and_then(|b| b.with_intra_threads(4).map_err(|e| e.to_string()))
            .and_then(|mut b| b.commit_from_file(model_path).map_err(|e| e.to_string()));

        match trt_res {
            Ok(session) => {
                info!(
                    "[ONNX Runtime] TensorRT Execution Provider active (FinBERT Static graph + GPU hardware acceleration + FP16 enabled, int8={}).",
                    int8_enabled
                );
                return Ok((session, "TensorRT".to_string()));
            }
            Err(e) => {
                warn!(
                    "[ONNX Runtime] TensorRT EP initialization notice: {}. Attempting CUDA EP fallback...",
                    e
                );
            }
        }
    }

    // ── Tier 2: CUDA Execution Provider ──────────────────────────────────────
    if force_ep == "auto" || force_ep == "cuda" {
        info!(
            "[ONNX Runtime] Attempting CUDA Execution Provider initialization (device_id={})",
            device_id
        );

        let cuda = CUDA::default().with_device_id(device_id);

        let cuda_res = Session::builder()
            .map_err(|e| e.to_string())
            .and_then(|b| {
                b.with_execution_providers([cuda.build()])
                    .map_err(|e| e.to_string())
            })
            .and_then(|b| {
                b.with_optimization_level(GraphOptimizationLevel::Level3)
                    .map_err(|e| e.to_string())
            })
            .and_then(|b| b.with_intra_threads(4).map_err(|e| e.to_string()))
            .and_then(|mut b| b.commit_from_file(model_path).map_err(|e| e.to_string()));

        match cuda_res {
            Ok(session) => {
                info!("[ONNX Runtime] CUDA Execution Provider active.");
                return Ok((session, "CUDA".to_string()));
            }
            Err(e) => {
                warn!(
                    "[ONNX Runtime] CUDA EP initialization notice: {}. Falling back to multi-threaded CPU EP.",
                    e
                );
            }
        }
    }

    let cpu_threads = env::var("ORT_CPU_THREADS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(|p| p.get())
                .unwrap_or(6)
        });

    // ── Tier 3: CPU Execution Provider (Resilient High-Throughput Fallback) ───
    info!(
        "[ONNX Runtime] Initializing optimized multi-threaded CPU Execution Provider (Level3 Optimization, {} intra-threads, 1 inter-thread)...",
        cpu_threads
    );
    let session = Session::builder()
        .map_err(|e| format!("Failed to create SessionBuilder: {}", e))?
        .with_optimization_level(GraphOptimizationLevel::Level3)
        .map_err(|e| format!("Failed to set optimization level: {}", e))?
        .with_intra_threads(cpu_threads)
        .map_err(|e| format!("Failed to set intra threads: {}", e))?
        .with_inter_threads(1)
        .map_err(|e| format!("Failed to set inter threads: {}", e))?
        .commit_from_file(model_path)
        .map_err(|e| {
            format!(
                "Failed to load ONNX model on CPU from {:?}: {}",
                model_path, e
            )
        })?;

    info!(
        "[ONNX Runtime] CPU Execution Provider active (intra_threads={}, inter_threads=1).",
        cpu_threads
    );
    Ok((session, "CPU".to_string()))
}

static GLOBAL_PIPELINE: Lazy<Mutex<Option<OnnxSentimentPipeline>>> = Lazy::new(|| Mutex::new(None));

/// Search for onnxruntime.dll in workspace
fn find_onnxruntime_dll() -> Result<PathBuf, String> {
    let mut candidates = vec![
        env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        PathBuf::from("."),
        PathBuf::from(".."),
        PathBuf::from("../.."),
    ];

    if let Ok(manifest) = env::var("CARGO_MANIFEST_DIR") {
        let p = PathBuf::from(manifest);
        if let Some(parent) = p.parent() {
            if let Some(grandparent) = parent.parent() {
                candidates.push(grandparent.to_path_buf());
            }
            candidates.push(parent.to_path_buf());
        }
        candidates.push(p);
    }

    for candidate in candidates {
        let p1 = candidate
            .join("venv")
            .join("Lib")
            .join("site-packages")
            .join("onnxruntime")
            .join("capi")
            .join("onnxruntime.dll");
        if p1.exists() {
            return Ok(p1);
        }
        let p2 = candidate.join("onnxruntime.dll");
        if p2.exists() {
            return Ok(p2);
        }
    }

    Err("Could not find onnxruntime.dll".to_string())
}

/// Helper function to check if the fine-tuned FinBERT ONNX model path exists in workspace.
pub fn find_finetuned_model_path() -> Option<PathBuf> {
    let mut candidates = vec![
        env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        PathBuf::from("."),
        PathBuf::from(".."),
        PathBuf::from("../.."),
    ];

    if let Ok(manifest) = env::var("CARGO_MANIFEST_DIR") {
        let p = PathBuf::from(manifest);
        if let Some(parent) = p.parent() {
            if let Some(grandparent) = parent.parent() {
                candidates.push(grandparent.to_path_buf());
            }
            candidates.push(parent.to_path_buf());
        }
        candidates.push(p);
    }

    for candidate in candidates {
        let p = candidate
            .join("models")
            .join("finbert-finetuned")
            .join("finbert.onnx");
        if p.exists() {
            return Some(p);
        }
        let p_static = candidate
            .join("models")
            .join("finbert-finetuned")
            .join("model_static.onnx");
        if p_static.exists() {
            return Some(p_static);
        }
        let p_dyn = candidate
            .join("models")
            .join("finbert-finetuned")
            .join("model.onnx");
        if p_dyn.exists() {
            return Some(p_dyn);
        }
    }

    None
}

/// Find model and tokenizer paths in workspace (prefers models/finbert-finetuned if present).
fn find_finbert_assets() -> Result<(PathBuf, PathBuf), String> {
    if let (Ok(m), Ok(t)) = (
        env::var("FINBERT_MODEL_PATH"),
        env::var("FINBERT_TOKENIZER_PATH"),
    ) {
        let mp = PathBuf::from(m);
        let tp = PathBuf::from(t);
        let tp_file = if tp.is_dir() {
            tp.join("tokenizer.json")
        } else {
            tp
        };
        if mp.exists() && tp_file.exists() {
            return Ok((mp, tp_file));
        }
    }

    let model_dir_env = env::var("FINBERT_MODEL_DIR").ok();

    let mut candidates = vec![
        env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        PathBuf::from("."),
        PathBuf::from(".."),
        PathBuf::from("../.."),
    ];

    if let Ok(manifest) = env::var("CARGO_MANIFEST_DIR") {
        let p = PathBuf::from(manifest);
        if let Some(parent) = p.parent() {
            if let Some(grandparent) = parent.parent() {
                candidates.push(grandparent.to_path_buf());
            }
            candidates.push(parent.to_path_buf());
        }
        candidates.push(p);
    }

    // 1. Explicit FINBERT_MODEL_DIR override
    if let Some(custom_dir) = &model_dir_env {
        let p = PathBuf::from(custom_dir);
        let finbert_onnx = p.join("finbert.onnx");
        let static_model = p.join("model_static.onnx");
        let dynamic_model = p.join("model.onnx");
        let tokenizer = p.join("tokenizer.json");
        if finbert_onnx.exists() && tokenizer.exists() {
            info!(
                "[ONNX Runtime] Located model from FINBERT_MODEL_DIR: {:?}",
                finbert_onnx
            );
            return Ok((finbert_onnx, tokenizer));
        } else if static_model.exists() && tokenizer.exists() {
            info!(
                "[ONNX Runtime] Located model from FINBERT_MODEL_DIR: {:?}",
                static_model
            );
            return Ok((static_model, tokenizer));
        } else if dynamic_model.exists() && tokenizer.exists() {
            info!(
                "[ONNX Runtime] Located model from FINBERT_MODEL_DIR: {:?}",
                dynamic_model
            );
            return Ok((dynamic_model, tokenizer));
        }
    }

    // 2. Highest Priority: Fine-Tuned FinBERT domain model (models/finbert-finetuned)
    for candidate in &candidates {
        let ft_dir = candidate.join("models").join("finbert-finetuned");
        let ft_onnx = ft_dir.join("finbert.onnx");
        let ft_static = ft_dir.join("model_static.onnx");
        let ft_dynamic = ft_dir.join("model.onnx");
        let ft_tokenizer = ft_dir.join("tokenizer.json");

        if ft_onnx.exists() && ft_tokenizer.exists() {
            info!(
                "[ONNX Runtime] Located fine-tuned FinBERT model at: {:?}",
                ft_onnx
            );
            return Ok((ft_onnx, ft_tokenizer));
        }
        if ft_static.exists() && ft_tokenizer.exists() {
            info!(
                "[ONNX Runtime] Located static fine-tuned FinBERT model at: {:?}",
                ft_static
            );
            return Ok((ft_static, ft_tokenizer));
        }
        if ft_dynamic.exists() && ft_tokenizer.exists() {
            info!(
                "[ONNX Runtime] Located dynamic fine-tuned FinBERT model at: {:?}",
                ft_dynamic
            );
            return Ok((ft_dynamic, ft_tokenizer));
        }
    }

    // 3. Fallback: Base FinBERT domain-specific model (models/finbert)
    for candidate in &candidates {
        let finbert_dir = candidate.join("models").join("finbert");
        let finbert_onnx = finbert_dir.join("finbert.onnx");
        let finbert_static = finbert_dir.join("model_static.onnx");
        let finbert_dynamic = finbert_dir.join("model.onnx");
        let finbert_tokenizer = finbert_dir.join("tokenizer.json");

        if finbert_onnx.exists() && finbert_tokenizer.exists() {
            info!(
                "[ONNX Runtime] Located primary FinBERT model at: {:?}",
                finbert_onnx
            );
            return Ok((finbert_onnx, finbert_tokenizer));
        }
        if finbert_static.exists() && finbert_tokenizer.exists() {
            info!(
                "[ONNX Runtime] Located static FinBERT model at: {:?}",
                finbert_static
            );
            return Ok((finbert_static, finbert_tokenizer));
        }
        if finbert_dynamic.exists() && finbert_tokenizer.exists() {
            info!(
                "[ONNX Runtime] Located dynamic FinBERT model at: {:?}",
                finbert_dynamic
            );
            return Ok((finbert_dynamic, finbert_tokenizer));
        }
    }

    Err("Could not locate models/finbert-finetuned or models/finbert assets in workspace.".to_string())
}

/// Compute sentiment using the native in-process ONNX Runtime pipeline.
pub fn compute_sentiment_onnx(text: &str) -> Result<SentimentOutput, String> {
    if env::var("SENTIMENT_MOCK_MODE")
        .map(|v| v == "1" || v.to_lowercase() == "true")
        .unwrap_or(false)
    {
        return Ok(generate_mock_sentiment(text));
    }

    let mut guard = GLOBAL_PIPELINE.lock().map_err(|e| e.to_string())?;
    if guard.is_none() {
        match find_finbert_assets() {
            Ok((model_path, tokenizer_path)) => {
                let pipeline =
                    OnnxSentimentPipeline::load_from_paths(&model_path, &tokenizer_path)?;
                *guard = Some(pipeline);
            }
            Err(e) => {
                if env::var("SENTIMENT_MOCK_FALLBACK")
                    .map(|v| v == "1" || v.to_lowercase() == "true")
                    .unwrap_or(false)
                {
                    warn!(
                        "[ONNX Runtime] Sentiment assets not found ({}). Falling back to mock sentiment.",
                        e
                    );
                    return Ok(generate_mock_sentiment(text));
                }
                return Err(e);
            }
        }
    }

    guard.as_ref().unwrap().analyze(text)
}

fn generate_mock_sentiment(text: &str) -> SentimentOutput {
    let lower = text.to_lowercase();
    let pos_words = [
        "record", "surge", "gain", "profit", "beat", "positive", "growth", "high", "raise", "up",
        "bull", "exceed",
    ];
    let neg_words = [
        "loss",
        "fall",
        "drop",
        "collapse",
        "decline",
        "negative",
        "down",
        "bankruptcy",
        "crisis",
        "halt",
        "bear",
        "debt",
    ];

    let pos_count = pos_words.iter().filter(|&&w| lower.contains(w)).count();
    let neg_count = neg_words.iter().filter(|&&w| lower.contains(w)).count();

    if pos_count > neg_count {
        SentimentOutput::new(0.75, "POSITIVE".to_string(), 0.85, 0.05, 0.10)
    } else if neg_count > pos_count {
        SentimentOutput::new(-0.75, "NEGATIVE".to_string(), 0.05, 0.85, 0.10)
    } else {
        SentimentOutput::new(0.0, "NEUTRAL".to_string(), 0.33, 0.33, 0.34)
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::time::Instant;

    fn init_test_tracing() {
        let _ = tracing_subscriber::fmt()
            .with_env_filter("info")
            .with_test_writer()
            .try_init();
    }

    /// Returns true if every provided path exists on disk.
    fn model_assets_present(paths: &[&std::path::Path]) -> bool {
        paths.iter().all(|p| p.exists())
    }

    fn check_finbert_assets() -> (PathBuf, PathBuf) {
        match find_finbert_assets() {
            Ok(p) => p,
            Err(_) => (
                PathBuf::from("models/finbert-finetuned/model_static.onnx"),
                PathBuf::from("models/finbert-finetuned/tokenizer.json"),
            ),
        }
    }

    #[test]
    fn test_onnx_sentiment_positive() {
        let (m, t) = check_finbert_assets();
        if !model_assets_present(&[m.as_path(), t.as_path()]) {
            eprintln!(
                "SKIP: required model assets not present: {:?}. Set MODEL_DIR or run locally with assets.",
                &[m.as_path(), t.as_path()]
            );
            return;
        }

        init_test_tracing();
        let output = compute_sentiment_onnx(
            "Apple reported record quarterly revenue and raised dividend guidance.",
        );
        assert!(output.is_ok(), "Failed ONNX inference: {:?}", output);
        let sent = output.unwrap();
        assert!(sent.sentiment_score > 0.0);
        assert_eq!(sent.sentiment_label, "POSITIVE");
        assert!(sent.prob_positive > sent.prob_negative);
    }

    #[test]
    fn test_onnx_sentiment_negative() {
        let (m, t) = check_finbert_assets();
        if !model_assets_present(&[m.as_path(), t.as_path()]) {
            eprintln!(
                "SKIP: required model assets not present: {:?}. Set MODEL_DIR or run locally with assets.",
                &[m.as_path(), t.as_path()]
            );
            return;
        }

        init_test_tracing();
        let output = compute_sentiment_onnx("Company files for Chapter 11 bankruptcy as revenue collapses amid massive debt crisis.");
        assert!(output.is_ok(), "Failed ONNX inference: {:?}", output);
        let sent = output.unwrap();
        assert!(sent.sentiment_score < 0.0);
        assert_eq!(sent.sentiment_label, "NEGATIVE");
        assert!(sent.prob_negative > sent.prob_positive);
    }

    #[test]
    fn test_onnx_sentiment_empty_error() {
        init_test_tracing();
        let output = compute_sentiment_onnx("   ");
        assert!(output.is_err());
    }

    #[test]
    fn test_onnx_execution_provider_registered() {
        let (m, t) = check_finbert_assets();
        if !model_assets_present(&[m.as_path(), t.as_path()]) {
            eprintln!(
                "SKIP: required model assets not present: {:?}. Set MODEL_DIR or run locally with assets.",
                &[m.as_path(), t.as_path()]
            );
            return;
        }

        init_test_tracing();
        let (model_path, tokenizer_path) = find_finbert_assets().expect("Assets must be present");
        let pipeline = OnnxSentimentPipeline::load_from_paths(&model_path, &tokenizer_path)
            .expect("Pipeline should load with valid EP");
        let ep = pipeline.provider();
        info!(
            "[ONNX Runtime] Verified registered Execution Provider: {}",
            ep
        );
        assert!(
            ep == "TensorRT" || ep == "CUDA" || ep == "CPU",
            "Provider must be TensorRT, CUDA, or CPU fallback, got '{}'",
            ep
        );
    }

    #[test]
    fn test_onnx_pad_to_128_helper() {
        let mut ids = vec![101, 102];
        let mut mask = vec![1, 1];
        pad_to_128(&mut ids, &mut mask);
        assert_eq!(ids.len(), 128);
        assert_eq!(mask.len(), 128);
        assert_eq!(ids[0], 101);
        assert_eq!(ids[1], 102);
        assert_eq!(ids[2], 0);
        assert_eq!(mask[0], 1);
        assert_eq!(mask[1], 1);
        assert_eq!(mask[2], 0);
    }

    #[test]
    fn test_onnx_pad_to_fixed_len_helper() {
        let mut ids = vec![101, 102];
        let mut mask = vec![1, 1];
        pad_to_fixed_len(&mut ids, &mut mask, 0, 32);
        assert_eq!(ids.len(), 32);
        assert_eq!(mask.len(), 32);
        assert_eq!(ids[0], 101);
        assert_eq!(ids[1], 102);
        assert_eq!(ids[2], 0);
        assert_eq!(mask[0], 1);
        assert_eq!(mask[1], 1);
        assert_eq!(mask[2], 0);
    }

    #[test]
    fn test_onnx_sentiment_benchmark_latency() {
        let (m, t) = check_finbert_assets();
        if !model_assets_present(&[m.as_path(), t.as_path()]) {
            eprintln!(
                "SKIP: required model assets not present: {:?}. Set MODEL_DIR or run locally with assets.",
                &[m.as_path(), t.as_path()]
            );
            return;
        }

        init_test_tracing();

        let (model_path, tokenizer_path) = find_finbert_assets().expect("Assets must be present");
        let pipeline = OnnxSentimentPipeline::load_from_paths(&model_path, &tokenizer_path)
            .expect("Pipeline must load successfully");

        let provider = pipeline.provider().to_string();
        let seq_len = pipeline.seq_len();
        info!(
            "[ONNX Runtime] Active execution provider: {} (Model: {:?}, SeqLen: {}, FP16: true)",
            provider,
            model_path.file_name().unwrap_or_default(),
            seq_len
        );

        // 1. Warm-up Phase (1 iteration to build/cache kernels)
        let warmup_start = Instant::now();
        let warmup_res = pipeline.analyze(INT8_CALIBRATION_CORPUS[0]);
        let warmup_duration = warmup_start.elapsed();
        assert!(
            warmup_res.is_ok(),
            "Warmup inference failed: {:?}",
            warmup_res
        );
        info!(
            "[Benchmark] Warm-up inference complete in {:.2}ms",
            warmup_duration.as_secs_f64() * 1000.0
        );

        // 2. Steady-State Benchmark Phase (10 iterations)
        let iterations = 10;
        let mut total_duration = std::time::Duration::ZERO;

        for i in 1..=iterations {
            let sample_text = INT8_CALIBRATION_CORPUS[(i - 1) % INT8_CALIBRATION_CORPUS.len()];
            let iter_start = Instant::now();
            let res = pipeline.analyze(sample_text);
            let iter_elapsed = iter_start.elapsed();
            total_duration += iter_elapsed;
            assert!(res.is_ok(), "Benchmark iteration {} failed: {:?}", i, res);
        }

        let avg_ms = total_duration.as_secs_f64() * 1000.0 / iterations as f64;
        let avg_us = avg_ms * 1000.0;

        println!(
            "\n═══════════════════════════════════════════════════════════════════════════════\n\
            [Benchmark] Model: FinBERT (Seq: {}) | Provider: {} | Average Latency: {:.2} ms ({:.0} us) over {} iterations\n\
            ═══════════════════════════════════════════════════════════════════════════════\n",
            seq_len, provider, avg_ms, avg_us, iterations
        );
    }

    #[test]
    fn test_onnx_sentiment_sliding_window_long_text() {
        let (m, t) = check_finbert_assets();
        if !model_assets_present(&[m.as_path(), t.as_path()]) {
            eprintln!(
                "SKIP: required model assets not present: {:?}. Set MODEL_DIR or run locally with assets.",
                &[m.as_path(), t.as_path()]
            );
            return;
        }

        init_test_tracing();

        // Synthetic long financial filing text (>600 tokens to ensure chunking triggers on both 32-token and 512-token models)
        let paragraph = "Apple Inc. reported exceptional fourth-quarter financial results with record-breaking quarterly revenue of $94.9 billion, representing a 6 percent increase year-over-year. Net income expanded significantly driven by unprecedented demand for iPhone 16 Pro models and double-digit growth in Services revenue including App Store, Apple Pay, and iCloud. Gross margin reached an all-time high of 46.2 percent compared to 45.2 percent in the prior-year quarter. Operating cash flow totaled a remarkable $26.8 billion, allowing the company to return over $29 billion to shareholders through quarterly dividends and accelerated share repurchases. Management raised its guidance for the upcoming holiday quarter, projecting continued margin expansion and double-digit operating leverage. ";
        let long_sec_filing = paragraph.repeat(6);

        let output = compute_sentiment_onnx(&long_sec_filing);
        assert!(
            output.is_ok(),
            "Failed chunked ONNX inference: {:?}",
            output
        );
        let sent = output.unwrap();

        info!(
            "[Chunking Test] Long Document Sentiment: {:+.4} ({}) | Chunks: {:?} | Scores: {:?}",
            sent.sentiment_score, sent.sentiment_label, sent.chunk_count, sent.chunk_scores
        );

        assert!(
            sent.chunk_count.is_some(),
            "chunk_count must be populated for long text"
        );
        let count = sent.chunk_count.unwrap();
        assert!(
            count > 1,
            "Expected multiple chunks for >100 tokens, got {}",
            count
        );

        assert!(
            sent.chunk_scores.is_some(),
            "chunk_scores array must be populated"
        );
        let scores = sent.chunk_scores.unwrap();
        assert_eq!(scores.len(), count);

        assert_eq!(
            sent.aggregation_method.as_deref(),
            Some("sliding_window_confidence_weighted")
        );

        assert!(sent.sentiment_score >= -1.0 && sent.sentiment_score <= 1.0);
        assert!(sent.prob_positive >= 0.0 && sent.prob_positive <= 1.0);
        assert!(sent.prob_negative >= 0.0 && sent.prob_negative <= 1.0);
        assert!(sent.prob_neutral >= 0.0 && sent.prob_neutral <= 1.0);

        let sum_probs = sent.prob_positive + sent.prob_negative + sent.prob_neutral;
        assert!(
            (sum_probs - 1.0).abs() < 0.01,
            "Probabilities should sum to ~1.0"
        );

        assert!(
            sent.sentiment_score > 0.0,
            "Positive long text should yield positive aggregate score"
        );
        assert_eq!(sent.sentiment_label, "POSITIVE");
    }

    #[test]
    fn test_onnx_sentiment_short_text_no_chunking() {
        let (m, t) = check_finbert_assets();
        if !model_assets_present(&[m.as_path(), t.as_path()]) {
            eprintln!(
                "SKIP: required model assets not present: {:?}. Set MODEL_DIR or run locally with assets.",
                &[m.as_path(), t.as_path()]
            );
            return;
        }

        init_test_tracing();

        let short_text = "NVIDIA beats earnings.";
        let output = compute_sentiment_onnx(short_text);
        assert!(output.is_ok());
        let sent = output.unwrap();

        assert!(
            sent.chunk_count.is_none(),
            "Short text should not have chunk_count set"
        );
        assert!(
            sent.chunk_scores.is_none(),
            "Short text should not have chunk_scores set"
        );
        assert!(
            sent.aggregation_method.is_none(),
            "Short text should not have aggregation_method set"
        );
    }

    #[test]
    fn test_finbert_confidence_and_structure() {
        let (m, t) = check_finbert_assets();
        if !model_assets_present(&[m.as_path(), t.as_path()]) {
            eprintln!(
                "SKIP: required model assets not present: {:?}. Set MODEL_DIR or run locally with assets.",
                &[m.as_path(), t.as_path()]
            );
            return;
        }

        init_test_tracing();
        let output = compute_sentiment_onnx(
            "Apple reported record quarterly revenue and raised dividend guidance.",
        );
        assert!(output.is_ok());
        let sent = output.unwrap();
        assert!(sent.confidence() >= 0.33);
        assert_eq!(sent.sentiment_label, "POSITIVE");
        assert!(sent.prob_positive > sent.prob_negative);
    }

    #[test]
    fn test_finetuned_model_path_resolution() {
        // When models/finbert-finetuned is absent, it returns None
        // When present, it returns Some(PathBuf) pointing to finbert.onnx or model_static.onnx
        let path_opt = find_finetuned_model_path();
        if let Some(p) = path_opt {
            assert!(p.exists());
            assert!(p.to_string_lossy().contains("finbert-finetuned"));
        }
    }
}
