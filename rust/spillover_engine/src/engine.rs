//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Spillover Engine Execution & Scheduling Core
//! ═══════════════════════════════════════════════════════════════════════════════

use crate::config::SpilloverConfig;
use crate::correlation::{compute_cross_correlation, SpilloverResult};
use crate::questdb::{fetch_sentiment_history, write_spillovers_ilp};
use crate::timeseries::build_synchronized_matrix;
use reqwest::Client;
use std::time::{Duration, Instant};
use tokio::time::interval;
use tracing::{error, info};

pub struct SpilloverEngine {
    config: SpilloverConfig,
    client: Client,
}

impl SpilloverEngine {
    pub fn new(config: SpilloverConfig) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_millis(config.timeout_ms))
            .pool_idle_timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| Client::new());

        Self { config, client }
    }

    pub fn config(&self) -> &SpilloverConfig {
        &self.config
    }

    /// Run a single cycle of the cross-asset spillover calculation and persistence pipeline.
    pub async fn run_once(&self) -> Result<usize, String> {
        let t0 = Instant::now();
        info!(
            "Starting cross-asset spillover calculation (Lookback: {}d, Max Lag: ±{}h, Threshold: |r| >= {:.2})...",
            self.config.lookback_days, self.config.max_lag_hours, self.config.min_correlation_threshold
        );

        // 1. Ingest historical sentiment records from QuestDB
        let events = fetch_sentiment_history(
            &self.client,
            &self.config.questdb_url,
            self.config.lookback_days,
            self.config.mock_mode,
        )
        .await?;

        if events.is_empty() {
            info!("No historical sentiment records returned from QuestDB. Skipping cycle.");
            return Ok(0);
        }

        info!(
            "Fetched {} sentiment events. Building hourly time series matrix (min_active_hours={}, max_tickers={})...",
            events.len(), self.config.min_active_hours, self.config.max_tickers
        );

        // 2. Synchronize and align hourly series across qualifying tickers
        let (timeline, matrix) = build_synchronized_matrix(
            &events,
            self.config.min_active_hours,
            self.config.max_tickers,
        );

        let num_tickers = matrix.len();
        if num_tickers < 2 {
            info!(
                "Fewer than 2 qualifying tickers with >= {} active hours. Skipping cycle.",
                self.config.min_active_hours
            );
            return Ok(0);
        }

        info!(
            "Constructed aligned matrix across {} discrete hours for {} active tickers. Evaluating {} pairwise combinations...",
            timeline.len(),
            num_tickers,
            (num_tickers * (num_tickers - 1)) / 2
        );

        // 3. Pairwise Cross-Correlation & Lead-Lag Computation
        let ticker_names: Vec<String> = {
            let mut names: Vec<String> = matrix.keys().cloned().collect();
            names.sort();
            names
        };

        let mut spillovers: Vec<SpilloverResult> = Vec::new();

        for i in 0..ticker_names.len() {
            let ticker_a = &ticker_names[i];
            let series_a = &matrix[ticker_a];

            for ticker_b in ticker_names.iter().skip(i + 1) {
                let series_b = &matrix[ticker_b];

                let (lag, corr, obs) =
                    compute_cross_correlation(series_a, series_b, self.config.max_lag_hours);

                if corr.abs() >= self.config.min_correlation_threshold && obs >= 3 {
                    spillovers.push(SpilloverResult {
                        ticker_a: ticker_a.clone(),
                        ticker_b: ticker_b.clone(),
                        lag_hours: lag,
                        correlation: corr,
                        num_observations: obs,
                    });
                }
            }
        }

        info!(
            "Identified {} significant cross-asset lead-lag relationships with |r| >= {:.2}.",
            spillovers.len(),
            self.config.min_correlation_threshold
        );

        // 4. Ingest calculated spillovers to QuestDB via ILP
        let written_count = write_spillovers_ilp(
            &self.client,
            &self.config.questdb_url,
            &spillovers,
            self.config.max_retries,
            self.config.mock_mode,
        )
        .await?;

        let elapsed = t0.elapsed();
        info!(
            "Cross-asset spillover cycle completed in {:.2}s. {} records persisted to QuestDB.",
            elapsed.as_secs_f64(),
            written_count
        );

        Ok(written_count)
    }

    /// Background execution loop running every `interval_secs`.
    pub async fn run_loop(&self) {
        let mut interval_timer = interval(Duration::from_secs(self.config.interval_secs));
        info!(
            "Spillover Engine loop active. Running cycle every {} seconds.",
            self.config.interval_secs
        );

        loop {
            interval_timer.tick().await;

            if let Err(e) = self.run_once().await {
                error!("Error executing spillover engine cycle: {}", e);
            }
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[tokio::test]
    async fn test_engine_run_once_mock_mode() {
        let mut config = SpilloverConfig::default();
        config.mock_mode = true;
        config.min_active_hours = 1;
        config.min_correlation_threshold = 0.2;

        let engine = SpilloverEngine::new(config);
        let result = engine.run_once().await;

        assert!(result.is_ok(), "Engine run_once failed: {:?}", result);
        let count = result.unwrap();
        assert!(
            count > 0,
            "Expected mock mode to produce > 0 spillovers, got {}",
            count
        );
    }
}
