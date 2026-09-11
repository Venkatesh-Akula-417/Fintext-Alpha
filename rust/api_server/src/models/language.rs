use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

/// Query parameters for language detection endpoint (`GET /language/detect`).
#[derive(Debug, Clone, Deserialize, IntoParams, ToSchema)]
pub struct LanguageDetectionQuery {
    /// Raw text, headline, or document excerpt to analyze (up to 5000 chars)
    #[schema(
        example = "El mercado de valores muestra un fuerte crecimiento en las acciones de tecnología"
    )]
    pub text: String,
}

/// Result payload for language detection and model routing.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct LanguageDetectionResponse {
    /// Detected language name ("english", "spanish", "german", "french", "japanese")
    #[schema(example = "spanish")]
    pub language: String,
    /// ISO 639-1 two-letter language code ("en", "es", "de", "fr", "ja")
    #[schema(example = "es")]
    pub language_code: String,
    /// Detection confidence score between 0.0 and 1.0
    #[schema(example = 0.92)]
    pub confidence: f64,
    /// Total character count analyzed (capped at 500 chars)
    #[schema(example = 85)]
    pub analyzed_chars: usize,
    /// Whether a multilingual model pipeline is applied for sentiment inference
    #[schema(example = true)]
    pub is_multilingual_model_applied: bool,
    /// Inference model identifier selected for this language
    #[schema(example = "multilingual-minilm-v1.0")]
    pub model_version: String,
    /// Status or diagnostic message
    #[schema(example = "Language detected successfully")]
    pub message: String,
}
