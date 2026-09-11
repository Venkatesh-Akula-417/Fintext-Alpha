//! Main entry point for the FinText AI Anomaly Detector daemon.

use dotenv::dotenv;
use fintext_observability_anomaly::{
    AnomalyConfig, ObservabilityClient, RollingWindowAnomalyDetector,
};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    dotenv().ok();

    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("================================================================================");
    info!(" FinText Alpha Vectorizer — AI Latency & Performance Anomaly Detector");
    info!("================================================================================");

    let config = AnomalyConfig::from_env();
    info!(
        prometheus_url = %config.prometheus_url,
        query = %config.query,
        threshold_zscore = config.threshold_zscore,
        window_size = config.window_size,
        check_interval_secs = config.check_interval_secs,
        mock_mode = config.mock_mode,
        "Configuration initialized."
    );

    let client =
        ObservabilityClient::new(config.prometheus_url.clone(), config.webhook_url.clone());
    let mut detector = RollingWindowAnomalyDetector::new(
        config.window_size,
        config.threshold_zscore,
        config.min_std_dev,
    );

    if config.mock_mode {
        return run_mock_detection_cycle(&mut detector, &client).await;
    }

    info!("Starting continuous Prometheus latency monitoring loop...");

    loop {
        tokio::select! {
            _ = sleep(Duration::from_secs(config.check_interval_secs)) => {
                match client.query_metric(&config.query).await {
                    Ok(val) => {
                        let report = detector.evaluate_sample(val);
                        if report.is_anomaly {
                            error!(
                                current_latency_sec = %report.current_value,
                                rolling_mean_sec = %report.rolling_mean,
                                std_dev = %report.rolling_std_dev,
                                z_score = %report.z_score,
                                threshold = %report.threshold,
                                "CRITICAL: Latency anomaly detected! Triggering auto-remediation..."
                            );
                            let _ = client.trigger_webhook(&report).await;
                        } else {
                            info!(
                                current_latency_sec = %report.current_value,
                                rolling_mean_sec = %report.rolling_mean,
                                z_score = %report.z_score,
                                samples = report.sample_count,
                                "Latency sample nominal."
                            );
                        }
                    }
                    Err(e) => {
                        warn!(error = %e, "Failed to query Prometheus. Retrying next interval...");
                    }
                }
            }
            _ = tokio::signal::ctrl_c() => {
                info!("Received SIGINT/Ctrl+C. Shutting down anomaly detector daemon gracefully.");
                break;
            }
        }
    }

    info!("Anomaly detector terminated cleanly.");
    Ok(())
}

async fn run_mock_detection_cycle(
    detector: &mut RollingWindowAnomalyDetector,
    client: &ObservabilityClient,
) -> Result<(), anyhow::Error> {
    info!("Executing Mock Latency Anomaly Cycle...");

    // 1. Establish baseline (~0.0025s / 2.5ms)
    info!("Feeding 10 baseline samples (~2.5ms latency)...");
    for &s in &[
        0.0024, 0.0026, 0.0025, 0.0023, 0.0027, 0.0025, 0.0024, 0.0026, 0.0025, 0.0025,
    ] {
        let rep = detector.evaluate_sample(s);
        assert!(!rep.is_anomaly);
    }
    let (mean, std_dev) = detector.compute_statistics();
    info!(baseline_mean_ms = %(mean * 1000.0), baseline_std_ms = %(std_dev * 1000.0), "Baseline established.");

    // 2. Inject anomalous latency spike (150ms / 0.150s)
    info!("Injecting severe latency spike to 150ms (0.150s)...");
    let spike_sample = 0.150;
    let spike_report = detector.evaluate_sample(spike_sample);

    if spike_report.is_anomaly {
        error!(
            spike_latency_ms = %(spike_sample * 1000.0),
            z_score = %spike_report.z_score,
            threshold = %spike_report.threshold,
            "SUCCESS: Anomaly correctly flagged by statistical autoencoder-proxy!"
        );
        let _ = client.trigger_webhook(&spike_report).await;
    } else {
        panic!("FAILED: Spike was not flagged as an anomaly!");
    }

    info!("Mock Anomaly Cycle Completed Successfully!");
    Ok(())
}
