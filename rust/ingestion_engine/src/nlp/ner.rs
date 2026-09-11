//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Native Rust ONNX Named Entity Recognition (NER)
//! ═══════════════════════════════════════════════════════════════════════════════
//!
//! Extracts financial named entities (ORG, PER, LOC, MISC) from news and SEC filings
//! using static [1, 64] BERT token classification, IOB2 decoding, and character offset mapping.
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
use tracing::info;

/// IOB2 label map for dslim/bert-base-NER
pub const NER_LABELS: &[&str] = &[
    "O",      // 0
    "B-MISC", // 1
    "I-MISC", // 2
    "B-PER",  // 3
    "I-PER",  // 4
    "B-ORG",  // 5
    "I-ORG",  // 6
    "B-LOC",  // 7
    "I-LOC",  // 8
];

pub const NER_SEQ_LEN: usize = 64;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Entity {
    pub text: String,
    pub label: String,
    pub start: usize,
    pub end: usize,
}

/// Helper function to pad/truncate token IDs and attention mask vectors to a fixed length.
pub fn pad_to_fixed_len(ids: &mut Vec<i64>, mask: &mut Vec<i64>, pad_id: i64, fixed_len: usize) {
    if ids.len() < fixed_len {
        ids.resize(fixed_len, pad_id);
    } else if ids.len() > fixed_len {
        ids.truncate(fixed_len);
    }
    if mask.len() < fixed_len {
        mask.resize(fixed_len, 0);
    } else if mask.len() > fixed_len {
        mask.truncate(fixed_len);
    }
}

pub struct OnnxNerPipeline {
    session: Mutex<Session>,
    tokenizer: Tokenizer,
    provider: String,
    pad_token_id: i64,
}

impl OnnxNerPipeline {
    /// Access the active execution provider name ("CPU", "TensorRT", or "CUDA").
    pub fn provider(&self) -> &str {
        &self.provider
    }

    /// Load the NER model and tokenizer with resilient CPU/GPU execution fallback.
    pub fn load_from_paths(model_path: &Path, tokenizer_path: &Path) -> Result<Self, String> {
        if let Ok(dll_path) = find_onnxruntime_dll() {
            info!(
                "[NER] Initializing ONNX Runtime from dynamic library: {:?}",
                dll_path
            );
            if let Ok(builder) = ort::init_from(dll_path.to_string_lossy().to_string()) {
                let _ = builder.commit();
            }
        } else {
            let _ = ort::init().commit();
        }

        info!("Loading native ONNX NER model from: {:?}", model_path);
        let (session, provider) = create_ner_session_with_fallbacks(model_path)?;
        info!("[ONNX Runtime NER] Active execution provider: {}", provider);

        info!("Loading NER tokenizer from: {:?}", tokenizer_path);
        let mut tokenizer = Tokenizer::from_file(tokenizer_path).map_err(|e| {
            format!(
                "Failed to load NER tokenizer from {:?}: {}",
                tokenizer_path, e
            )
        })?;

        let pad_token_id: i64 = tokenizer
            .token_to_id("[PAD]")
            .or_else(|| tokenizer.token_to_id("<pad>"))
            .map(|id| id as i64)
            .unwrap_or(0);

        let _ = tokenizer.with_truncation(Some(tokenizers::TruncationParams {
            max_length: NER_SEQ_LEN,
            strategy: tokenizers::TruncationStrategy::LongestFirst,
            stride: 0,
            direction: tokenizers::TruncationDirection::Right,
        }));

        Ok(Self {
            session: Mutex::new(session),
            tokenizer,
            provider,
            pad_token_id,
        })
    }

    /// Extracts named entities (ORG, PER, LOC, MISC) from input text.
    pub fn extract(&self, text: &str) -> Result<Vec<Entity>, String> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Ok(Vec::new());
        }

        // 1. Tokenize input text (capped at 64 tokens for low-latency in-process extraction)
        let encoding = self
            .tokenizer
            .encode(trimmed, true)
            .map_err(|e| format!("NER tokenization error: {}", e))?;

        let offsets: Vec<(usize, usize)> = encoding.get_offsets().to_vec();
        let mut input_ids_raw: Vec<i64> = encoding
            .get_ids()
            .iter()
            .take(NER_SEQ_LEN)
            .map(|&id| id as i64)
            .collect();
        let mut attention_mask_raw: Vec<i64> = encoding
            .get_attention_mask()
            .iter()
            .take(NER_SEQ_LEN)
            .map(|&m| m as i64)
            .collect();

        if input_ids_raw.is_empty() {
            return Ok(Vec::new());
        }

        let num_actual_tokens = input_ids_raw.len();

        // 2. Pad to static dimension [1, 64]
        pad_to_fixed_len(
            &mut input_ids_raw,
            &mut attention_mask_raw,
            self.pad_token_id,
            NER_SEQ_LEN,
        );

        let input_ids_val = Value::from_array(([1, NER_SEQ_LEN], input_ids_raw))
            .map_err(|e| format!("Failed to create input_ids Value: {}", e))?;
        let attention_mask_val = Value::from_array(([1, NER_SEQ_LEN], attention_mask_raw))
            .map_err(|e| format!("Failed to create attention_mask Value: {}", e))?;

        let inputs = ort::inputs![
            "input_ids" => input_ids_val,
            "attention_mask" => attention_mask_val,
        ];

        let mut session_guard = self.session.lock().map_err(|e| e.to_string())?;
        let outputs = session_guard
            .run(inputs)
            .map_err(|e| format!("ONNX NER inference run error: {}", e))?;

        let logits_val = outputs
            .get("logits")
            .or_else(|| outputs.get("output"))
            .ok_or_else(|| "Missing 'logits' tensor in NER ONNX model".to_string())?;

        let (shape, logits_data) = logits_val
            .try_extract_tensor::<f32>()
            .map_err(|e| format!("Failed to extract NER logits tensor: {}", e))?;

        if logits_data.len() < NER_SEQ_LEN * 9 {
            return Err(format!("Expected shape [1, 64, 9], got shape: {:?}", shape));
        }

        // 3. IOB2 Tag Decoding & Entity Span Merging
        let mut entities = Vec::new();
        let mut current_entity: Option<(String, usize, usize)> = None;

        for i in 0..num_actual_tokens {
            let (start_offset, end_offset) = if i < offsets.len() {
                offsets[i]
            } else {
                (0, 0)
            };

            // Skip special tokens [CLS], [SEP], [PAD] with zero span
            if start_offset == end_offset {
                if let Some((label, start, end)) = current_entity.take() {
                    if let Some(span) = trimmed.get(start..end) {
                        let text_val = span.trim().to_string();
                        if !text_val.is_empty() {
                            entities.push(Entity {
                                text: text_val,
                                label,
                                start,
                                end,
                            });
                        }
                    }
                }
                continue;
            }

            // Argmax over 9 classes
            let slice = &logits_data[i * 9..(i + 1) * 9];
            let mut max_idx = 0;
            let mut max_score = slice[0];
            for (idx, &score) in slice.iter().enumerate().skip(1) {
                if score > max_score {
                    max_score = score;
                    max_idx = idx;
                }
            }

            let tag = NER_LABELS.get(max_idx).copied().unwrap_or("O");

            if tag == "O" {
                if let Some((label, start, end)) = current_entity.take() {
                    if let Some(span) = trimmed.get(start..end) {
                        let text_val = span.trim().to_string();
                        if !text_val.is_empty() {
                            entities.push(Entity {
                                text: text_val,
                                label,
                                start,
                                end,
                            });
                        }
                    }
                }
            } else if let Some(cat) = tag.strip_prefix("B-") {
                if let Some((label, start, end)) = current_entity.take() {
                    if let Some(span) = trimmed.get(start..end) {
                        let text_val = span.trim().to_string();
                        if !text_val.is_empty() {
                            entities.push(Entity {
                                text: text_val,
                                label,
                                start,
                                end,
                            });
                        }
                    }
                }
                current_entity = Some((cat.to_string(), start_offset, end_offset));
            } else if let Some(cat) = tag.strip_prefix("I-") {
                if let Some((ref curr_cat, _, ref mut curr_end)) = current_entity {
                    if curr_cat == cat {
                        *curr_end = end_offset;
                    } else {
                        if let Some((label, start, end)) = current_entity.take() {
                            if let Some(span) = trimmed.get(start..end) {
                                let text_val = span.trim().to_string();
                                if !text_val.is_empty() {
                                    entities.push(Entity {
                                        text: text_val,
                                        label,
                                        start,
                                        end,
                                    });
                                }
                            }
                        }
                        current_entity = Some((cat.to_string(), start_offset, end_offset));
                    }
                } else {
                    current_entity = Some((cat.to_string(), start_offset, end_offset));
                }
            }
        }

        if let Some((label, start, end)) = current_entity.take() {
            if let Some(span) = trimmed.get(start..end) {
                let text_val = span.trim().to_string();
                if !text_val.is_empty() {
                    entities.push(Entity {
                        text: text_val,
                        label,
                        start,
                        end,
                    });
                }
            }
        }

        Ok(entities)
    }
}

/// Creates an ONNX Runtime session for NER with multi-threaded CPU by default.
fn create_ner_session_with_fallbacks(model_path: &Path) -> Result<(Session, String), String> {
    let trt_cache_path = env::var("NER_TENSORRT_CACHE_PATH").unwrap_or_else(|_| {
        if let Some(parent) = model_path.parent() {
            parent
                .join("trt_cache_static")
                .to_string_lossy()
                .to_string()
        } else {
            "models/ner/trt_cache_static".to_string()
        }
    });
    let _ = fs::create_dir_all(&trt_cache_path);

    let force_ep = env::var("NER_EXECUTION_PROVIDER")
        .or_else(|_| env::var("ORT_EXECUTION_PROVIDER"))
        .unwrap_or_else(|_| "cpu".to_string()) // Default to CPU for NER to prevent RTX 2050 GPU VRAM contention
        .to_lowercase();

    let device_id = env::var("ORT_DEVICE_ID")
        .ok()
        .and_then(|v| v.parse::<i32>().ok())
        .unwrap_or(0);

    let cpu_threads = env::var("NER_CPU_THREADS")
        .or_else(|_| env::var("ORT_CPU_THREADS"))
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(|p| p.get().min(6))
                .unwrap_or(4)
        });

    // ── Tier 1: Explicit TensorRT Execution Provider ─────────────────────────
    if force_ep == "tensorrt" {
        info!("[NER] Attempting TensorRT Execution Provider initialization");
        let trt = TensorRT::default()
            .with_device_id(device_id)
            .with_max_workspace_size(256 * 1024 * 1024)
            .with_engine_cache(true)
            .with_engine_cache_path(&trt_cache_path)
            .with_fp16(true);

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
            .and_then(|b| b.with_intra_threads(cpu_threads).map_err(|e| e.to_string()))
            .and_then(|mut b| b.commit_from_file(model_path).map_err(|e| e.to_string()));

        if let Ok(session) = trt_res {
            info!("[NER] TensorRT Execution Provider active.");
            return Ok((session, "TensorRT".to_string()));
        }
    }

    // ── Tier 2: CUDA Execution Provider ──────────────────────────────────────
    if force_ep == "cuda" {
        info!("[NER] Attempting CUDA Execution Provider initialization");
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
            .and_then(|b| b.with_intra_threads(cpu_threads).map_err(|e| e.to_string()))
            .and_then(|mut b| b.commit_from_file(model_path).map_err(|e| e.to_string()));

        if let Ok(session) = cuda_res {
            info!("[NER] CUDA Execution Provider active.");
            return Ok((session, "CUDA".to_string()));
        }
    }

    // ── Tier 3: Multi-Threaded CPU Execution Provider (Default) ──────────────
    info!(
        "[NER] Initializing optimized multi-threaded CPU Execution Provider (Level3, {} intra-threads, 1 inter-thread)...",
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
                "Failed to load ONNX NER model on CPU from {:?}: {}",
                model_path, e
            )
        })?;

    info!(
        "[NER] CPU Execution Provider active (intra_threads={}).",
        cpu_threads
    );
    Ok((session, "CPU".to_string()))
}

static GLOBAL_NER_PIPELINE: Lazy<Mutex<Option<OnnxNerPipeline>>> = Lazy::new(|| Mutex::new(None));

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

/// Find NER model and tokenizer paths in workspace.
pub fn find_ner_assets() -> Result<(PathBuf, PathBuf), String> {
    if let (Ok(m), Ok(t)) = (env::var("NER_MODEL_PATH"), env::var("NER_TOKENIZER_PATH")) {
        let mp = PathBuf::from(m);
        let tp = PathBuf::from(t);
        if mp.exists() && tp.exists() {
            return Ok((mp, tp));
        }
    }

    let model_dir_env = env::var("NER_MODEL_DIR").ok();

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

    if let Some(custom_dir) = &model_dir_env {
        let p = PathBuf::from(custom_dir);
        let static_model = p.join("model_static.onnx");
        let tokenizer = p.join("tokenizer.json");
        if static_model.exists() && tokenizer.exists() {
            return Ok((static_model, tokenizer));
        }
    }

    for candidate in &candidates {
        let static_model = candidate
            .join("models")
            .join("ner")
            .join("model_static.onnx");
        let tokenizer = candidate.join("models").join("ner").join("tokenizer.json");
        if static_model.exists() && tokenizer.exists() {
            return Ok((static_model, tokenizer));
        }
    }

    Err("Could not locate models/ner assets in workspace.".to_string())
}

/// Extract named entities from text using the native in-process ONNX NER pipeline.
pub fn extract_entities(text: &str) -> Result<Vec<Entity>, String> {
    let mut guard = GLOBAL_NER_PIPELINE.lock().map_err(|e| e.to_string())?;
    if guard.is_none() {
        let (model_path, tokenizer_path) = find_ner_assets()?;
        let pipeline = OnnxNerPipeline::load_from_paths(&model_path, &tokenizer_path)?;
        *guard = Some(pipeline);
    }

    guard.as_ref().unwrap().extract(text)
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

    #[test]
    fn test_ner_entity_extraction_sample() {
        init_test_tracing();
        let text = "Apple Inc. is based in Cupertino, California.";
        let res = extract_entities(text);
        assert!(res.is_ok(), "NER extraction failed: {:?}", res);
        let entities = res.unwrap();
        assert!(!entities.is_empty(), "Entities should not be empty");

        let labels: Vec<&str> = entities.iter().map(|e| e.label.as_str()).collect();
        let texts: Vec<&str> = entities.iter().map(|e| e.text.as_str()).collect();

        info!("[NER Test] Extracted entities: {:?}", entities);
        assert!(labels.contains(&"ORG"), "Should detect ORG label");
        assert!(labels.contains(&"LOC"), "Should detect LOC label");
        assert!(
            texts.iter().any(|t| t.contains("Apple")),
            "Should extract Apple Inc."
        );
        assert!(
            texts
                .iter()
                .any(|t| t.contains("Cupertino") || t.contains("California")),
            "Should extract location"
        );
    }

    #[test]
    fn test_ner_empty_text_returns_empty_vec() {
        init_test_tracing();
        let res = extract_entities("   ");
        assert!(res.is_ok());
        assert!(res.unwrap().is_empty());
    }

    #[test]
    fn test_ner_benchmark_latency() {
        init_test_tracing();
        let (model_path, tokenizer_path) = find_ner_assets().expect("NER assets must be present");
        let pipeline = OnnxNerPipeline::load_from_paths(&model_path, &tokenizer_path)
            .expect("NER pipeline must load successfully");

        let sample =
            "NVIDIA CEO Jensen Huang announced new Blackwell GPU architectures in Santa Clara.";

        // Warmup
        let _ = pipeline.extract(sample);

        let iterations = 10;
        let mut total_duration = std::time::Duration::ZERO;

        for _ in 0..iterations {
            let t0 = Instant::now();
            let _ = pipeline.extract(sample);
            total_duration += t0.elapsed();
        }

        let avg_ms = total_duration.as_secs_f64() * 1000.0 / iterations as f64;
        let avg_us = avg_ms * 1000.0;

        println!(
            "\n═══════════════════════════════════════════════════════════════════════════════\n\
            [Benchmark] Model: BERT-Base-NER (Seq: 64) | Provider: {} | Average Latency: {:.2} ms ({:.0} us) over {} iterations\n\
            ═══════════════════════════════════════════════════════════════════════════════\n",
            pipeline.provider(), avg_ms, avg_us, iterations
        );
    }
}
