//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Audio Transcription & Acoustic Stress Analysis Handler
//! ═══════════════════════════════════════════════════════════════════════════════

use axum::extract::{Multipart, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use axum::Extension;
use chrono::Utc;
use serde::Deserialize;
use tracing::{error, info};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::audio::AudioProcessor;
use crate::auth::{AuthErrorResponse, Claims};
#[allow(unused_imports)]
use crate::models::AudioTranscriptionResponse;
use crate::state::AppState;
use crate::transcripts::StoredTranscript;

pub const MAX_AUDIO_BYTES: usize = 50 * 1024 * 1024; // 50 MB limit

/// Supported audio file extensions.
const SUPPORTED_EXTENSIONS: &[&str] = &["wav", "mp3", "flac", "m4a", "aac", "ogg"];

/// Optional query parameters for audio transcription endpoint.
#[derive(Debug, Clone, Deserialize, IntoParams, ToSchema)]
pub struct AudioTranscribeParams {
    /// Automatically store transcription in the transcripts database (default: false)
    #[schema(example = true)]
    pub store: Option<bool>,
    /// Associated ticker symbol for stored transcript
    #[schema(example = "AAPL")]
    pub ticker: Option<String>,
    /// Fiscal quarter (1 to 4)
    #[schema(example = 4)]
    pub quarter: Option<u8>,
    /// Fiscal year (e.g. 2024)
    #[schema(example = 2024)]
    pub year: Option<i32>,
    /// Call date (YYYY-MM-DD)
    #[schema(example = "2024-10-31")]
    pub call_date: Option<String>,
}

/// Check whether the provided filename or content-type is a supported audio format.
fn is_supported_audio_format(filename: &str, content_type: Option<&str>) -> bool {
    let lower_name = filename.to_lowercase();
    for ext in SUPPORTED_EXTENSIONS {
        if lower_name.ends_with(&format!(".{}", ext)) {
            return true;
        }
    }

    if let Some(ct) = content_type {
        let lower_ct = ct.to_lowercase();
        if lower_ct.starts_with("audio/") || lower_ct == "application/octet-stream" {
            return true;
        }
    }

    false
}

/// Upload & Transcribe Spoken Audio with Acoustic Feature Extraction.
///
/// Accepts a `multipart/form-data` payload containing an audio file in field `audio`
/// (WAV, MP3, FLAC, M4A, AAC, OGG up to 50 MB), transcribes spoken earnings / executive audio,
/// extracts acoustic vocal stress metrics (pitch, RMS energy, pause ratio), and classifies
/// content sentiment using FinBERT.
///
/// If `store=true` is specified in query parameters or form fields, the transcribed text and
/// sentiment will be automatically persisted to the earnings call transcript database.
#[utoipa::path(
    post,
    path = "/audio/transcribe",
    tag = "Audio Processing",
    params(
        AudioTranscribeParams
    ),
    request_body(
        content = inline(String),
        description = "Multipart form-data containing 'audio' file upload (WAV, MP3, FLAC, M4A, AAC, OGG up to 50 MB)"
    ),
    responses(
        (status = 200, description = "Audio transcribed and analyzed successfully", body = AudioTranscriptionResponse),
        (status = 400, description = "Invalid audio format or missing audio field", body = AuthErrorResponse),
        (status = 401, description = "Unauthorized (missing or invalid Bearer JWT)", body = AuthErrorResponse),
        (status = 413, description = "Audio file exceeds 50 MB size limit", body = AuthErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::rate_limit::RateLimitErrorResponse)
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn transcribe_audio_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Query(params): Query<AudioTranscribeParams>,
    mut multipart: Multipart,
) -> Response {
    let mut audio_bytes: Option<Vec<u8>> = None;
    let mut audio_filename: String = "upload.wav".to_string();
    let mut audio_content_type: Option<String> = None;

    let mut form_store: bool = false;
    let mut form_ticker: Option<String> = None;
    let mut form_quarter: Option<u8> = None;
    let mut form_year: Option<i32> = None;
    let mut form_call_date: Option<String> = None;

    while let Ok(Some(field)) = multipart.next_field().await {
        let field_name = field.name().unwrap_or("").to_string();
        let filename = field.file_name().map(|f| f.to_string());
        let content_type = field.content_type().map(|c| c.to_string());

        if field_name == "audio" {
            if let Some(f) = filename {
                audio_filename = f;
            }
            audio_content_type = content_type;

            match field.bytes().await {
                Ok(bytes) => {
                    if bytes.len() > MAX_AUDIO_BYTES {
                        let err = AuthErrorResponse {
                            error: "Payload Too Large".to_string(),
                            message: format!(
                                "Audio file exceeds maximum permitted size of 50 MB (uploaded {} bytes)",
                                bytes.len()
                            ),
                        };
                        return (StatusCode::PAYLOAD_TOO_LARGE, Json(err)).into_response();
                    }
                    audio_bytes = Some(bytes.to_vec());
                }
                Err(e) => {
                    error!("Failed to read multipart audio bytes: {}", e);
                    let err = AuthErrorResponse {
                        error: "Bad Request".to_string(),
                        message: format!("Failed to read uploaded audio stream: {}", e),
                    };
                    return (StatusCode::BAD_REQUEST, Json(err)).into_response();
                }
            }
        } else if field_name == "store" {
            if let Ok(text) = field.text().await {
                form_store = text.trim().eq_ignore_ascii_case("true") || text.trim() == "1";
            }
        } else if field_name == "ticker" {
            if let Ok(text) = field.text().await {
                if !text.trim().is_empty() {
                    form_ticker = Some(text.trim().to_uppercase());
                }
            }
        } else if field_name == "quarter" {
            if let Ok(text) = field.text().await {
                form_quarter = text.trim().parse::<u8>().ok();
            }
        } else if field_name == "year" {
            if let Ok(text) = field.text().await {
                form_year = text.trim().parse::<i32>().ok();
            }
        } else if field_name == "call_date" {
            if let Ok(text) = field.text().await {
                if !text.trim().is_empty() {
                    form_call_date = Some(text.trim().to_string());
                }
            }
        }
    }

    let raw_bytes = match audio_bytes {
        Some(b) if !b.is_empty() => b,
        _ => {
            let err = AuthErrorResponse {
                error: "Bad Request".to_string(),
                message: "Missing 'audio' file in multipart/form-data upload".to_string(),
            };
            return (StatusCode::BAD_REQUEST, Json(err)).into_response();
        }
    };

    if !is_supported_audio_format(&audio_filename, audio_content_type.as_deref()) {
        let err = AuthErrorResponse {
            error: "Bad Request".to_string(),
            message: format!(
                "Unsupported audio format for file '{}'. Allowed formats: WAV, MP3, FLAC, M4A, AAC, OGG",
                audio_filename
            ),
        };
        return (StatusCode::BAD_REQUEST, Json(err)).into_response();
    }

    info!(
        "[Audio Transcription] Processing '{}' ({} bytes)",
        audio_filename,
        raw_bytes.len()
    );

    match AudioProcessor::process_audio(&raw_bytes, &audio_filename) {
        Ok(mut response) => {
            let should_store = params.store.unwrap_or(false) || form_store;
            if should_store {
                let ticker = form_ticker
                    .or(params.ticker)
                    .unwrap_or_else(|| "UNKNOWN".to_string())
                    .to_uppercase();
                let quarter = form_quarter.or(params.quarter);
                let year = form_year.or(params.year);
                let call_date = form_call_date.or(params.call_date);

                let stored = StoredTranscript {
                    id: Uuid::new_v4(),
                    user_id: claims.sub.clone(),
                    ticker,
                    quarter,
                    year,
                    call_date,
                    transcript_text: response.transcription.clone(),
                    source: "whisper_asr".to_string(),
                    sentiment_score: Some(response.sentiment.score),
                    sentiment_label: Some(response.sentiment.label.clone()),
                    confidence: Some(response.sentiment.confidence),
                    word_count: response.word_count,
                    created_at: Utc::now(),
                };

                let id = stored.id;
                let _ = state
                    .transcript_registry
                    .insert(stored, state.db_pool.as_ref())
                    .await;
                response.transcript_id = Some(id);
                info!(
                    "[Audio Transcription] Stored transcript {} for user {}",
                    id, claims.sub
                );
            }

            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => {
            error!("Audio processing failed: {}", e);
            let err = AuthErrorResponse {
                error: "Internal Error".to_string(),
                message: format!("Audio analysis failed: {}", e),
            };
            (StatusCode::INTERNAL_SERVER_ERROR, Json(err)).into_response()
        }
    }
}
