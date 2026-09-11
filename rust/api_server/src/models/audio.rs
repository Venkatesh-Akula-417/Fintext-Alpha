//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Audio Transcription & Acoustic Feature Models
//! ═══════════════════════════════════════════════════════════════════════════════

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Acoustic prosody and vocal stress features extracted from spoken audio.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct AcousticFeatures {
    /// Fundamental frequency (F0) mean in Hz (pitch)
    #[schema(example = 120.3)]
    pub pitch_mean_hz: f64,
    /// Root-Mean-Square (RMS) amplitude energy level
    #[schema(example = 0.05)]
    pub energy_rms: f64,
    /// Ratio of silent / hesitation duration to total speech duration (0.0 to 1.0)
    #[schema(example = 0.15)]
    pub pause_ratio: f64,
}

/// Spoken content sentiment classification metrics.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct AudioSentiment {
    /// Quantitative sentiment score ranging from -1.0 (Bearish) to +1.0 (Bullish)
    #[schema(example = 0.35)]
    pub score: f64,
    /// Sentiment categorical classification ('BULLISH', 'BEARISH', 'NEUTRAL')
    #[schema(example = "NEUTRAL")]
    pub label: String,
    /// Model prediction confidence score (0.0 to 1.0)
    #[schema(example = 0.72)]
    pub confidence: f64,
}

/// Response payload for audio transcription and acoustic sentiment analysis.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct AudioTranscriptionResponse {
    /// Transcribed spoken text
    #[schema(
        example = "Apple Inc. reported record quarterly revenue of $94.9 billion, up 6 percent year over year."
    )]
    pub transcription: String,
    /// Total audio recording duration in seconds
    #[schema(example = 120.5)]
    pub duration_seconds: f64,
    /// Extracted vocal acoustic stress and prosody features
    pub acoustic_features: AcousticFeatures,
    /// Financial sentiment analysis of transcribed text
    pub sentiment: AudioSentiment,
    /// Total word count in transcription
    #[schema(example = 1500)]
    pub word_count: usize,
    /// Detected or configured spoken language code
    #[schema(example = "en")]
    pub language: String,
    /// Optional stored transcript ID if store=true was requested
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub transcript_id: Option<uuid::Uuid>,
}
