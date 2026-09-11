//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — High-Performance Native Ingestion Engine
//! ═══════════════════════════════════════════════════════════════════════════════

pub mod alpha;
pub mod audio;
pub mod data;
pub mod ipc;
pub mod nlp;
pub mod pipeline;
pub mod quality;
pub mod sources;
pub mod storage;
pub mod streaming;
pub mod telemetry;

use std::sync::atomic::{AtomicBool, Ordering};

/// Global production mode switch for ingestion engine. When true, all mock fallbacks are disabled.
pub static PRODUCTION_MODE_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Explicitly configure the global production mode state.
pub fn set_production_mode(active: bool) {
    PRODUCTION_MODE_ACTIVE.store(active, Ordering::SeqCst);
}

/// Checks whether production mode is active via atomic flag, env var, or config file.
pub fn is_production_mode() -> bool {
    if PRODUCTION_MODE_ACTIVE.load(Ordering::Relaxed) {
        return true;
    }
    if let Ok(val) = std::env::var("PRODUCTION_MODE") {
        let val = val.trim().to_lowercase();
        if val == "1" || val == "true" || val == "yes" || val == "on" {
            return true;
        }
    }
    read_production_mode_from_config()
}

/// Helper function to check if QuestDB mock fallback is allowed.
pub fn is_questdb_mock_fallback_enabled() -> bool {
    if is_production_mode() {
        false
    } else {
        std::env::var("QUESTDB_MOCK_FALLBACK").as_deref() == Ok("1")
    }
}

/// Helper function to check if Kafka mock fallback is allowed.
pub fn is_kafka_mock_fallback_enabled() -> bool {
    if is_production_mode() {
        false
    } else {
        std::env::var("KAFKA_MOCK_FALLBACK").as_deref() == Ok("1")
            || std::env::var("KAFKA_MOCK_MODE").as_deref() == Ok("1")
    }
}

/// Helper function to check if Polygon mock fallback is allowed.
pub fn is_polygon_mock_fallback_enabled() -> bool {
    if is_production_mode() {
        false
    } else {
        std::env::var("POLYGON_MOCK_FALLBACK").as_deref() == Ok("1")
            || std::env::var("POLYGON_MOCK_MODE").as_deref() == Ok("1")
    }
}

/// Helper function to check if Whisper mock fallback is allowed.
pub fn is_whisper_mock_fallback_enabled() -> bool {
    if is_production_mode() {
        false
    } else {
        std::env::var("WHISPER_MOCK_FALLBACK").as_deref() == Ok("1")
    }
}

/// Helper function to check if TimescaleDB mock fallback is allowed.
pub fn is_timescale_mock_fallback_enabled() -> bool {
    if is_production_mode() {
        false
    } else {
        std::env::var("TIMESCALE_MOCK_FALLBACK").as_deref() == Ok("1")
            || std::env::var("TIMESCALE_MOCK_MODE").as_deref() == Ok("1")
            || true // default mock fallback in non-production
    }
}

/// Helper function to check if Finnhub mock fallback is allowed.
pub fn is_finnhub_mock_fallback_enabled() -> bool {
    if is_production_mode() {
        false
    } else {
        std::env::var("FINNHUB_MOCK_MODE").as_deref() == Ok("1")
    }
}

/// Reads `production_mode` setting from YAML configuration file.
pub fn read_production_mode_from_config() -> bool {
    let config_paths = [
        std::env::var("CONFIG_PATH").unwrap_or_default(),
        "config/config.yaml".to_string(),
        "config.yaml".to_string(),
        "../config/config.yaml".to_string(),
        "../../config/config.yaml".to_string(),
    ];

    for path in &config_paths {
        if path.is_empty() {
            continue;
        }
        if let Ok(content) = std::fs::read_to_string(path) {
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with('#') || trimmed.is_empty() {
                    continue;
                }
                if trimmed.starts_with("production_mode:") {
                    let parts: Vec<&str> = trimmed.splitn(2, ':').collect();
                    if parts.len() == 2 {
                        let val_str = parts[1].trim().trim_matches('"').trim_matches('\'').to_lowercase();
                        return val_str == "true" || val_str == "1" || val_str == "yes";
                    }
                }
            }
        }
    }
    false
}

pub use alpha::{
    build_adjacency_matrix, build_distance_matrix, calculate_black_scholes_gamma, compute_gex,
    compute_vpin, compute_vpin_and_gex_from_polygon, compute_vpin_detailed, GexResult,
    SupplyChainGNN, VpinResult, DEFAULT_GAMMA, MAX_NODES,
};
pub use audio::{
    decode_audio_file, extract_features, read_wav_file, transcribe_audio, transcribe_samples_async,
    AudioFeatures, WhisperTranscriber, GLOBAL_TRANSCRIBER,
};
pub use data::{parse_options_ticker, OptionTrade, ParsedOptionsContract, PolygonClient};
pub use ipc::{InferenceRequest, InferenceResponse, PythonBridge};
pub use nlp::{
    compute_sentiment_onnx, extract_entities, Entity, OnnxNerPipeline, OnnxSentimentPipeline,
    SentimentOutput,
};
pub use pipeline::{
    create_bounded_ingestion_channel, BackpressureMetrics, BackpressureMetricsSnapshot,
    CircuitBreakerConfig, DbCircuitBreaker, DbError,
    IngestionConcurrencyConfig, IngestionSendError, IngestionSender, Preprocessor,
    ProcessedDocument, RawDocument, SignalLatencyMetrics, WorkerPool,
    DEFAULT_INPUT_QUEUE_CAPACITY, DEFAULT_MAX_CONCURRENT_TASKS, DEFAULT_TASK_TIMEOUT_MS,
};
pub use quality::{
    calculate_freshness_ms, count_document_fields, quarantine_document, DataQualityConfig,
    DataQualityGate, DuplicateDetector, QualityGateResult, SourceMetrics, SourceQualityStore,
};
pub use sources::{
    compute_ticker_diff, generate_mock_company_tickers, parse_sec_company_tickers_json,
    run_updater_cycle, DailyBar, DelistingEvent, FinnhubArticle, FinnhubClient, FinnhubWsClient,
    FinnhubWsConfig, FinnhubWsMessage, FinnhubWsNewsItem, PolygonAggsResponse, PolygonWsClient,
    PolygonWsConfig, PolygonWsMessage, PolygonWsRawTrade, SecEdgarFetcher, SecTickerRecord,
    SecUpdaterConfig, TickerChange, TickerDiff, UpdaterRunReport,
};
pub use storage::{
    calculate_buffer_backoff, quarantine_failed_message, spawn_raw_archiver_worker,
    ArchiveUploader, CompleteSignalRecord, JsonlStreamSink, QuestDbBufferConfig,
    QuestDbBufferConsumer, QuestDbBufferMessage, QuestDbBufferProducer, QuestDbConfig,
    QuestDbSink, RawArchiveConfig, RawArchiveRecord, RawArchiveSender, RawArchiveWriter,
    TimescaleDbConfig, TimescaleDbSink, TimescaleSentimentRecord,
};
pub use streaming::{
    KafkaSentimentEvent, KafkaSink, KafkaSinkConfig, RealtimeSentimentEvent,
};
pub use telemetry::PipelineMetrics;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preprocessor_html_and_tickers() {
        let preprocessor = Preprocessor::new();
        let raw = RawDocument {
            id: "test-001".to_string(),
            title: "Apple Beats Estimates with Record iPhone Revenue and $AAPL Rising 5%".to_string(),
            source: "Reuters".to_string(),
            url: "https://reuters.com/apple".to_string(),
            published_utc: "2023-10-27T10:00:00Z".to_string(),
            raw_content: "<p>Apple Inc. announced financial results for its fiscal 2023 fourth quarter. <a href='https://apple.com'>Read more</a></p>".to_string(),
            audio_path: None,
            ingested_utc: "2023-10-27T10:00:01Z".to_string(),
            ..Default::default()
        };

        // Warm-up pass to initialize Lazy static regexes
        let _ = preprocessor.process(raw.clone());

        // Steady-state pass
        let processed = preprocessor.process(raw);
        assert_eq!(
            processed.clean_text,
            "Apple Inc. announced financial results for its fiscal 2023 fourth quarter. Read more"
        );
        assert!(processed.tickers.contains(&"AAPL".to_string()));
        assert_eq!(processed.primary_ticker, Some("AAPL".to_string()));
        assert_eq!(processed.ingested_utc, "2023-10-27T10:00:01Z");
        assert!(!processed.is_spam);
        assert_eq!(processed.event_category, "Earnings_Beat");
        assert!(processed.preprocessing_latency_us < 80_000); // Sub-80ms (<80,000µs SLA)
    }

    #[test]
    fn test_preprocessor_spam_filtering() {
        let preprocessor = Preprocessor::new();
        let raw = RawDocument {
            id: "test-002".to_string(),
            title: "FREE BITCOIN 100x GUARANTEED TO THE MOON 🚀💎".to_string(),
            source: "Reddit".to_string(),
            url: "https://reddit.com/r/wallstreetbets".to_string(),
            published_utc: "2023-10-27T10:05:00Z".to_string(),
            raw_content: "buy now don't miss massive gains!".to_string(),
            audio_path: None,
            ingested_utc: "2023-10-27T10:05:01Z".to_string(),
            ..Default::default()
        };

        let processed = preprocessor.process(raw);
        assert!(processed.is_spam);
        assert_eq!(processed.spam_reason, "promotional_language");
    }
}
