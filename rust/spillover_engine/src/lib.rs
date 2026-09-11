//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Cross-Asset Sentiment Spillover & Lead-Lag Engine
//! ═══════════════════════════════════════════════════════════════════════════════

pub mod config;
pub mod correlation;
pub mod engine;
pub mod questdb;
pub mod timeseries;

pub use config::SpilloverConfig;
pub use correlation::{compute_cross_correlation, pearson_correlation, SpilloverResult};
pub use engine::SpilloverEngine;
pub use questdb::{fetch_sentiment_history, format_spillovers_ilp, write_spillovers_ilp};
pub use timeseries::{
    build_synchronized_matrix, epoch_hour_to_timestamp, timestamp_to_epoch_hour, SentimentEvent,
};
