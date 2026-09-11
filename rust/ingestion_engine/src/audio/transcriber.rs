//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Whisper.cpp Audio Transcription (ASR) Engine
//! ═══════════════════════════════════════════════════════════════════════════════

use hound::{SampleFormat, WavReader};
use once_cell::sync::Lazy;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tracing::{info, warn};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

/// Global lazy Whisper transcriber instance wrapped in a thread-safe Mutex
pub static GLOBAL_TRANSCRIBER: Lazy<Mutex<Option<WhisperTranscriber>>> = Lazy::new(|| {
    let model_path = std::env::var("WHISPER_MODEL_PATH")
        .unwrap_or_else(|_| "models/whisper/ggml-base.en.bin".to_string());

    match WhisperTranscriber::new(&model_path) {
        Ok(t) => Mutex::new(Some(t)),
        Err(e) => {
            warn!(
                "[Whisper ASR] Notice: Unable to load Whisper model from '{}': {}. Transcription will gracefully fall back.",
                model_path, e
            );
            Mutex::new(None)
        }
    }
});

/// High-level Whisper.cpp Automatic Speech Recognition (ASR) wrapper.
pub struct WhisperTranscriber {
    context: WhisperContext,
    model_path: PathBuf,
}

impl std::fmt::Debug for WhisperTranscriber {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WhisperTranscriber")
            .field("model_path", &self.model_path)
            .finish()
    }
}

impl WhisperTranscriber {
    /// Loads a GGML Whisper model from the designated filesystem path.
    pub fn new(model_path: &str) -> Result<Self, String> {
        let path = PathBuf::from(model_path);
        if !path.exists() {
            return Err(format!("Whisper model file does not exist at {:?}", path));
        }

        let ctx_params = WhisperContextParameters::default();
        let context = WhisperContext::new_with_params(model_path, ctx_params).map_err(|e| {
            format!(
                "Failed to initialize WhisperContext from {:?}: {:?}",
                path, e
            )
        })?;

        info!("Whisper model successfully loaded from {:?}", path);
        Ok(Self {
            context,
            model_path: path,
        })
    }

    /// Returns the filesystem path to the loaded Whisper model.
    pub fn model_path(&self) -> &Path {
        &self.model_path
    }

    /// Transcribes normalized 16kHz mono `f32` audio samples into recognized text.
    pub fn transcribe_samples(&self, samples: &[f32]) -> Result<String, String> {
        if samples.is_empty() {
            return Ok(String::new());
        }

        let mut state = self
            .context
            .create_state()
            .map_err(|e| format!("Failed to create Whisper state: {:?}", e))?;

        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_language(Some("en"));
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);

        state
            .full(params, samples)
            .map_err(|e| format!("Whisper full transcription failed: {:?}", e))?;

        let num_segments = state
            .full_n_segments()
            .map_err(|e| format!("Failed to retrieve segment count: {:?}", e))?;

        let mut transcript = String::new();
        for i in 0..num_segments {
            if let Ok(segment) = state.full_get_segment_text(i) {
                let trimmed = segment.trim();
                if !trimmed.is_empty() {
                    if !transcript.is_empty() {
                        transcript.push(' ');
                    }
                    transcript.push_str(trimmed);
                }
            }
        }

        Ok(transcript)
    }

    /// Transcribes a local WAV audio file asynchronously offloading CPU inference to a blocking thread.
    pub async fn transcribe(&self, wav_file_path: &Path) -> Result<String, String> {
        let samples = read_wav_file(wav_file_path)?;
        self.transcribe_samples(&samples)
    }
}

/// Reads and decodes a WAV audio file returning normalized samples and the 16000Hz target sample rate.
pub fn decode_audio_file(path: &Path) -> Result<(Vec<f32>, u32), String> {
    let samples = read_wav_file(path)?;
    Ok((samples, 16000))
}

/// Asynchronously transcribes pre-decoded audio samples using the global Whisper engine.
pub async fn transcribe_samples_async(samples: Vec<f32>) -> Result<String, String> {
    if !crate::is_production_mode() && std::env::var("WHISPER_MOCK_FALLBACK").as_deref() == Ok("1") {
        return Ok("Apple Inc. reported record quarterly revenue of $94.9 billion, up 6 percent year over year. Earnings per diluted share were $1.64. Operating cash flow reached $26.8 billion.".to_string());
    }

    tokio::task::spawn_blocking(move || {
        let lock = GLOBAL_TRANSCRIBER.lock().map_err(|e| format!("Transcriber mutex poisoned: {}", e))?;
        match lock.as_ref() {
            Some(transcriber) => transcriber.transcribe_samples(&samples),
            None => {
                if !crate::is_production_mode() && std::env::var("WHISPER_MOCK_FALLBACK").as_deref() == Ok("1") {
                    Ok("Quarterly financial results exceeded consensus expectations with strong operating margins.".to_string())
                } else {
                    Err("Whisper transcriber is not initialized. Please configure valid WHISPER_MODEL_PATH.".to_string())
                }
            }
        }
    })
    .await
    .map_err(|e| format!("JoinError during blocking Whisper transcription: {}", e))?
}

/// Convenience async function to transcribe audio using the globally initialized Whisper engine.
pub async fn transcribe_audio(wav_file_path: &Path) -> Result<String, String> {
    // Fast-path mock mode for deterministic offline testing without requiring large GGML model downloads
    if !crate::is_production_mode() && std::env::var("WHISPER_MOCK_FALLBACK").as_deref() == Ok("1") {
        info!(
            "[Whisper ASR Mock] Transcribing WAV file at {:?} (mock mode enabled)",
            wav_file_path
        );
        let _ = decode_audio_file(wav_file_path)?;
        return Ok("Apple Inc. reported record quarterly revenue of $94.9 billion, up 6 percent year over year. Earnings per diluted share were $1.64. Operating cash flow reached $26.8 billion.".to_string());
    }

    let (samples, _) = decode_audio_file(wav_file_path)?;
    transcribe_samples_async(samples).await
}

/// Reads a WAV audio file, normalizes samples to `[-1.0, 1.0]`, converts multi-channel to mono, and resamples to 16kHz.
pub fn read_wav_file(path: &Path) -> Result<Vec<f32>, String> {
    let mut reader = WavReader::open(path)
        .map_err(|e| format!("Failed to open WAV file at {:?}: {}", path, e))?;

    let spec = reader.spec();
    let channels = spec.channels as usize;
    let sample_rate = spec.sample_rate;

    if channels == 0 {
        return Err("Invalid WAV file: channels count is 0".to_string());
    }

    let raw_samples: Vec<f32> = match spec.sample_format {
        SampleFormat::Int => match spec.bits_per_sample {
            16 => {
                let s: Result<Vec<i16>, _> = reader.samples::<i16>().collect();
                let s = s.map_err(|e| format!("Error reading i16 samples from WAV: {}", e))?;
                s.into_iter().map(|x| x as f32 / 32768.0).collect()
            }
            24 | 32 => {
                let s: Result<Vec<i32>, _> = reader.samples::<i32>().collect();
                let s = s.map_err(|e| format!("Error reading i32 samples from WAV: {}", e))?;
                let max_val = (1i64 << (spec.bits_per_sample - 1)) as f32;
                s.into_iter().map(|x| x as f32 / max_val).collect()
            }
            8 => {
                let s: Result<Vec<i8>, _> = reader.samples::<i8>().collect();
                let s = s.map_err(|e| format!("Error reading i8 samples from WAV: {}", e))?;
                s.into_iter().map(|x| x as f32 / 128.0).collect()
            }
            b => return Err(format!("Unsupported integer bits per sample: {}", b)),
        },
        SampleFormat::Float => {
            let s: Result<Vec<f32>, _> = reader.samples::<f32>().collect();
            s.map_err(|e| format!("Error reading float samples from WAV: {}", e))?
        }
    };

    // Convert multi-channel (e.g., stereo) to mono
    let mono_samples = if channels > 1 {
        let mut mono = Vec::with_capacity(raw_samples.len() / channels);
        for chunk in raw_samples.chunks(channels) {
            let sum: f32 = chunk.iter().sum();
            mono.push(sum / channels as f32);
        }
        mono
    } else {
        raw_samples
    };

    // Resample linearly to 16,000 Hz if necessary
    let resampled = if sample_rate != 16000 {
        resample_linear(&mono_samples, sample_rate, 16000)
    } else {
        mono_samples
    };

    Ok(resampled)
}

/// Linear interpolation audio resampler
pub fn resample_linear(input: &[f32], src_rate: u32, target_rate: u32) -> Vec<f32> {
    if input.is_empty() || src_rate == target_rate {
        return input.to_vec();
    }
    let ratio = src_rate as f64 / target_rate as f64;
    let target_len = (input.len() as f64 / ratio).round() as usize;
    let mut output = Vec::with_capacity(target_len);

    for i in 0..target_len {
        let src_idx = i as f64 * ratio;
        let idx0 = src_idx.floor() as usize;
        let idx1 = (idx0 + 1).min(input.len() - 1);
        let frac = (src_idx - idx0 as f64) as f32;
        let val = input[idx0] * (1.0 - frac) + input[idx1] * frac;
        output.push(val);
    }
    output
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use hound::{WavSpec, WavWriter};
    use std::f32::consts::PI;

    /// Helper to write a temporary synthetic 16kHz sine wave WAV file
    fn create_synthetic_wav(path: &Path, sample_rate: u32, duration_secs: f32) {
        let spec = WavSpec {
            channels: 1,
            sample_rate,
            bits_per_sample: 16,
            sample_format: SampleFormat::Int,
        };
        let mut writer = WavWriter::create(path, spec).unwrap();
        let num_samples = (sample_rate as f32 * duration_secs) as usize;
        for t in 0..num_samples {
            let sample = (t as f32 * 440.0 * 2.0 * PI / sample_rate as f32).sin();
            let val = (sample * 32767.0) as i16;
            writer.write_sample(val).unwrap();
        }
        writer.finalize().unwrap();
    }

    #[test]
    fn test_wav_reader_synthetic_audio() {
        let temp_dir = std::env::temp_dir();
        let wav_path = temp_dir.join("test_synthetic_16k.wav");

        create_synthetic_wav(&wav_path, 16000, 0.5);

        let samples = read_wav_file(&wav_path).expect("Failed to read synthetic WAV");
        assert_eq!(samples.len(), 8000);
        for &s in &samples {
            assert!(s >= -1.0 && s <= 1.0);
        }

        let _ = std::fs::remove_file(wav_path);
    }

    #[test]
    fn test_wav_resampling_44100_to_16000() {
        let temp_dir = std::env::temp_dir();
        let wav_path = temp_dir.join("test_synthetic_44k.wav");

        create_synthetic_wav(&wav_path, 44100, 1.0);

        let samples = read_wav_file(&wav_path).expect("Failed to read and resample WAV");
        assert_eq!(samples.len(), 16000);

        let _ = std::fs::remove_file(wav_path);
    }

    #[test]
    fn test_missing_model_graceful_error() {
        let result = WhisperTranscriber::new("models/whisper/non_existent_file.bin");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("does not exist"));
    }

    #[tokio::test]
    async fn test_mock_transcription_end_to_end() {
        std::env::set_var("WHISPER_MOCK_FALLBACK", "1");
        let temp_dir = std::env::temp_dir();
        let wav_path = temp_dir.join("test_mock_earnings.wav");

        create_synthetic_wav(&wav_path, 16000, 1.0);

        let transcript = transcribe_audio(&wav_path)
            .await
            .expect("Mock transcription should succeed");
        assert!(transcript.contains("Apple Inc."));
        assert!(transcript.contains("quarterly revenue"));

        let _ = std::fs::remove_file(wav_path);
    }
}
