//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Acoustic Feature Extraction & Vocal Stress Analysis
//! ═══════════════════════════════════════════════════════════════════════════════

use serde::{Deserialize, Serialize};

/// Acoustic features extracted from raw audio waveforms.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AudioFeatures {
    /// Fundamental frequency (F0) mean in Hz (e.g., 185.24 Hz)
    pub pitch_mean: f64,
    /// Fundamental frequency standard deviation in Hz (higher = increased vocal stress/inflection)
    pub pitch_std: f64,
    /// Root-Mean-Square (RMS) amplitude energy mean across frames
    pub energy_mean: f64,
    /// RMS energy standard deviation (volume dynamics and volatility)
    pub energy_std: f64,
    /// Ratio of silent frames (< 5% max RMS) to total duration (hesitation/silence ratio: 0.0 to 1.0)
    pub pause_ratio: f64,
    /// Speaking rate measured as distinct voiced segments per second
    pub speech_rate: f64,
}

impl Default for AudioFeatures {
    fn default() -> Self {
        Self {
            pitch_mean: 0.0,
            pitch_std: 0.0,
            energy_mean: 0.0,
            energy_std: 0.0,
            pause_ratio: 0.0,
            speech_rate: 0.0,
        }
    }
}

/// Helper function to round a floating-point value to 4 decimal places.
#[inline]
fn round_4(val: f64) -> f64 {
    (val * 10_000.0).round() / 10_000.0
}

/// Extracts acoustic stress, energy, pause, and rate features from 16 kHz mono audio samples.
///
/// # Arguments
/// * `samples` - Normalized `f32` samples in `[-1.0, 1.0]`.
/// * `sample_rate` - Audio sampling frequency in Hz (typically 16000 Hz).
pub fn extract_features(samples: &[f32], sample_rate: u32) -> Result<AudioFeatures, String> {
    if samples.is_empty() {
        return Ok(AudioFeatures::default());
    }

    if sample_rate == 0 {
        return Err("Sample rate cannot be zero".to_string());
    }

    // Bound maximum analysis duration to the first 300 seconds (5 minutes) for strict latency containment
    let max_samples = (sample_rate as usize).saturating_mul(300);
    let analysis_samples = if samples.len() > max_samples {
        &samples[..max_samples]
    } else {
        samples
    };

    // Frame configuration: 25ms frame window, 10ms hop
    let frame_len = ((sample_rate as f64) * 0.025).round() as usize; // e.g. 400 samples at 16k
    let hop_len = ((sample_rate as f64) * 0.010).round() as usize; // e.g. 160 samples at 16k

    if analysis_samples.len() < frame_len {
        return Ok(AudioFeatures::default());
    }

    let total_duration_secs = analysis_samples.len() as f64 / sample_rate as f64;

    // Pitch lag bounds for typical human speech (50 Hz to 400 Hz)
    let lag_min = (sample_rate / 400).max(1) as usize; // e.g. 40 at 16k
    let lag_max = (sample_rate / 50) as usize; // e.g. 320 at 16k

    let mut rms_values: Vec<f64> = Vec::new();
    let mut pitch_values: Vec<f64> = Vec::new();
    let mut voiced_flags: Vec<bool> = Vec::new();

    let mut start_idx = 0;
    while start_idx + frame_len <= analysis_samples.len() {
        let frame = &analysis_samples[start_idx..start_idx + frame_len];

        // 1. RMS Energy Calculation
        let sum_sq: f64 = frame.iter().map(|&x| (x as f64).powi(2)).sum();
        let rms = (sum_sq / frame_len as f64).sqrt();
        rms_values.push(rms);

        // 2. Pitch Detection via Normalized Autocorrelation
        if sum_sq < 1e-7 {
            // Signal energy too low -> unvoiced / silent
            voiced_flags.push(false);
        } else {
            let mut best_corr = 0.0f64;
            let mut best_lag = 0usize;

            let max_lag = lag_max.min(frame_len - 1);
            for lag in lag_min..=max_lag {
                let mut sum_prod = 0.0f64;
                let mut sum_tail_sq = 0.0f64;
                let len = frame_len - lag;

                for i in 0..len {
                    let x0 = frame[i] as f64;
                    let x1 = frame[i + lag] as f64;
                    sum_prod += x0 * x1;
                    sum_tail_sq += x1 * x1;
                }

                let denom = (sum_sq * sum_tail_sq).sqrt();
                if denom > 1e-9 {
                    let norm_corr = sum_prod / denom;
                    if norm_corr > best_corr {
                        best_corr = norm_corr;
                        best_lag = lag;
                    }
                }
            }

            // Voiced threshold check (autocorrelation peak >= 0.30)
            if best_corr >= 0.30 && best_lag > 0 {
                let f0 = (sample_rate as f64) / (best_lag as f64);
                if (50.0..=400.0).contains(&f0) {
                    pitch_values.push(f0);
                    voiced_flags.push(true);
                } else {
                    voiced_flags.push(false);
                }
            } else {
                voiced_flags.push(false);
            }
        }

        start_idx += hop_len;
    }

    if rms_values.is_empty() {
        return Ok(AudioFeatures::default());
    }

    let num_frames = rms_values.len() as f64;

    // 3. Energy Mean & Std Dev
    let energy_mean = rms_values.iter().sum::<f64>() / num_frames;
    let energy_variance = rms_values
        .iter()
        .map(|&r| (r - energy_mean).powi(2))
        .sum::<f64>()
        / num_frames;
    let energy_std = energy_variance.sqrt();

    // 4. Pitch Mean & Std Dev
    let (pitch_mean, pitch_std) = if !pitch_values.is_empty() {
        let n_pitch = pitch_values.len() as f64;
        let p_mean = pitch_values.iter().sum::<f64>() / n_pitch;
        let p_variance = pitch_values
            .iter()
            .map(|&p| (p - p_mean).powi(2))
            .sum::<f64>()
            / n_pitch;
        (p_mean, p_variance.sqrt())
    } else {
        (0.0, 0.0)
    };

    // 5. Pause Ratio (silent if RMS < 5% of max RMS)
    let max_rms = rms_values.iter().cloned().fold(0.0f64, f64::max);
    let pause_ratio = if max_rms < 1e-6 {
        1.0 // entire signal is silent
    } else {
        let silence_threshold = 0.05 * max_rms;
        let silent_count = rms_values
            .iter()
            .filter(|&&r| r < silence_threshold)
            .count();
        silent_count as f64 / num_frames
    };

    // 6. Speech Rate (voiced segments per second)
    let mut num_voiced_segments = 0;
    let mut in_segment = false;

    for &is_voiced in &voiced_flags {
        if is_voiced {
            if !in_segment {
                in_segment = true;
                num_voiced_segments += 1;
            }
        } else {
            in_segment = false;
        }
    }

    let speech_rate = if total_duration_secs > 0.0 {
        num_voiced_segments as f64 / total_duration_secs
    } else {
        0.0
    };

    Ok(AudioFeatures {
        pitch_mean: round_4(pitch_mean),
        pitch_std: round_4(pitch_std),
        energy_mean: round_4(energy_mean),
        energy_std: round_4(energy_std),
        pause_ratio: round_4(pause_ratio),
        speech_rate: round_4(speech_rate),
    })
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::f32::consts::PI;

    /// Generates a synthetic sine wave test signal.
    fn generate_sine_wave(
        freq_hz: f32,
        sample_rate: u32,
        duration_secs: f32,
        amplitude: f32,
    ) -> Vec<f32> {
        let n_samples = (sample_rate as f32 * duration_secs) as usize;
        (0..n_samples)
            .map(|t| amplitude * (2.0 * PI * freq_hz * (t as f32) / (sample_rate as f32)).sin())
            .collect()
    }

    #[test]
    fn test_pitch_detection_sine_wave_200hz() {
        let sample_rate = 16000;
        let samples = generate_sine_wave(200.0, sample_rate, 1.0, 0.8);

        let features = extract_features(&samples, sample_rate).expect("Feature extraction failed");

        // Pitch should be very close to 200.0 Hz (within 10%)
        assert!(
            features.pitch_mean >= 180.0 && features.pitch_mean <= 220.0,
            "Expected pitch ~200Hz, got {}",
            features.pitch_mean
        );
        assert!(
            features.pitch_std < 10.0,
            "Sine wave pitch should be stable"
        );
        assert!(
            features.energy_mean > 0.4,
            "Energy should reflect amplitude 0.8"
        );
        assert!(
            features.pause_ratio < 0.2,
            "Continuous sine wave should have low pause ratio"
        );
    }

    #[test]
    fn test_silence_pause_ratio() {
        let sample_rate = 16000;
        let silence = vec![0.0f32; 16000]; // 1 second of complete silence

        let features =
            extract_features(&silence, sample_rate).expect("Feature extraction on silence failed");

        assert_eq!(features.pitch_mean, 0.0);
        assert_eq!(features.pitch_std, 0.0);
        assert_eq!(features.energy_mean, 0.0);
        assert_eq!(features.pause_ratio, 1.0);
        assert_eq!(features.speech_rate, 0.0);
    }

    #[test]
    fn test_extract_features_mixed_signal() {
        let sample_rate = 16000;
        let mut mixed = generate_sine_wave(150.0, sample_rate, 0.5, 0.7);
        mixed.extend(vec![0.0f32; 8000]); // 0.5s pause
        mixed.extend(generate_sine_wave(250.0, sample_rate, 0.5, 0.7));

        let features = extract_features(&mixed, sample_rate).expect("Feature extraction failed");

        assert!(features.pitch_mean > 100.0 && features.pitch_mean < 300.0);
        assert!(
            features.pitch_std > 20.0,
            "Mixed frequencies should produce non-zero pitch std"
        );
        assert!(
            features.pause_ratio >= 0.25,
            "Pause segment should increase pause_ratio"
        );
        assert!(
            features.speech_rate >= 1.0,
            "Should detect at least 2 voiced bursts"
        );
    }
}
