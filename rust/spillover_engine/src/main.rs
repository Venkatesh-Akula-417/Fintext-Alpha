//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Cross-Asset Spillover Engine Service Binary
//! ═══════════════════════════════════════════════════════════════════════════════

use fintext_spillover_engine::{SpilloverConfig, SpilloverEngine};
use std::sync::Arc;
use tokio::signal;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Initialize structured logging
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    info!("═══════════════════════════════════════════════════════════════════════════");
    info!(" FinText-Alpha-Vectorizer — Cross-Asset Lead-Lag Spillover Analytics Engine");
    info!("═══════════════════════════════════════════════════════════════════════════");

    // 2. Load configuration from environment
    let config = SpilloverConfig::from_env();
    info!(
        " [QuestDB URL] '{}' | [Interval] {}s | [Lookback] {}d | [Max Lag] ±{}h | [Threshold] |r| >= {:.2}",
        config.questdb_url,
        config.interval_secs,
        config.lookback_days,
        config.max_lag_hours,
        config.min_correlation_threshold
    );

    let engine = Arc::new(SpilloverEngine::new(config));

    // 3. Spawn background execution loop
    let engine_ref = engine.clone();
    let runner_handle = tokio::spawn(async move {
        engine_ref.run_loop().await;
    });

    // 4. Wait for graceful shutdown signal (Ctrl+C / SIGINT)
    info!("Spillover engine running. Press Ctrl+C to initiate graceful shutdown.");
    signal::ctrl_c().await?;
    info!("Shutdown signal received. Commencing graceful teardown...");

    runner_handle.abort();
    info!("Spillover engine teardown complete. Goodbye.");

    Ok(())
}
