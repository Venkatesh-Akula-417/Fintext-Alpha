//! Entry point binary for the Dead Letter Queue (DLQ) Auto-Reprocessing & Quarantine Worker.

use dotenv::dotenv;
use fintext_dead_letter_worker::{DeadLetterWorker, DlqConfig};
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    dotenv().ok();

    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("================================================================================");
    info!(" FinText Alpha Vectorizer — Dead Letter Queue (DLQ) Auto-Reprocessing Worker");
    info!("================================================================================");

    let config = DlqConfig::from_env();
    info!(
        bootstrap_servers = %config.kafka_bootstrap_servers,
        dlq_topic = %config.dlq_topic,
        reprocess_topic = %config.reprocess_topic,
        group_id = %config.group_id,
        s3_bucket = %config.s3_quarantine_bucket,
        local_quarantine = %config.local_quarantine_dir,
        max_retries = config.max_retries,
        mock_mode = config.mock_mode,
        "Configuration loaded."
    );

    let mut worker = DeadLetterWorker::new(config).await.map_err(|e| {
        error!(error = %e, "Failed to initialize DeadLetterWorker.");
        anyhow::anyhow!(e)
    })?;

    tokio::select! {
        res = worker.run() => {
            if let Err(e) = res {
                error!(error = %e, "DeadLetterWorker encountered fatal error.");
                return Err(e);
            }
        }
        _ = tokio::signal::ctrl_c() => {
            info!("Received SIGINT/Ctrl+C. Initiating graceful worker shutdown...");
        }
    }

    info!("DeadLetterWorker shut down cleanly.");
    Ok(())
}
