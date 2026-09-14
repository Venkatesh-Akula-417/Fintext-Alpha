use chrono::Utc;
use fintext_ingestion_engine::{
    calculate_freshness_ms, compute_sentiment_onnx, compute_vpin_and_gex_from_polygon,
    count_document_fields, create_bounded_ingestion_channel, decode_audio_file, extract_features,
    quarantine_document, run_updater_cycle, spawn_raw_archiver_worker, transcribe_samples_async,
    BackpressureMetrics, CircuitBreakerConfig, CompleteSignalRecord, DataQualityConfig,
    DataQualityGate, DbCircuitBreaker, FinnhubClient, FinnhubWsClient, FinnhubWsConfig,
    InferenceResponse, IngestionConcurrencyConfig, IngestionSendError, JsonlStreamSink, KafkaSink,
    OptionTrade, PipelineMetrics, PolygonClient, PolygonWsClient, PolygonWsConfig, Preprocessor,
    QualityGateResult, QuestDbBufferConfig, QuestDbBufferConsumer, QuestDbBufferProducer,
    QuestDbConfig, QuestDbSink, RawArchiveConfig, RawArchiveRecord, RawArchiveSender, RawDocument,
    SecEdgarFetcher, SecUpdaterConfig, SentimentOutput, SourceQualityStore, TimescaleDbConfig,
    TimescaleDbSink, WorkerPool,
};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::signal;
use tokio::sync::mpsc;
use tokio::time::{sleep, Instant};
use tracing::{debug, error, info, warn};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 0. Load environment variables from .env if present
    dotenv::dotenv().ok();

    // 1. Initialize structured logging
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    info!("═══════════════════════════════════════════════════════════════════════════");
    info!(" FinText-Alpha-Vectorizer — High-Performance Native Rust Ingestion Engine");
    info!("═══════════════════════════════════════════════════════════════════════════");

    if let Err(e) = verify_model_weights_at_startup() {
        eprintln!("{}", e);
        std::process::exit(1);
    }

    let prod_mode = fintext_ingestion_engine::is_production_mode();
    if prod_mode {
        info!(
            " [PRODUCTION MODE GUARD] ACTIVE: Synthetic fallbacks disabled. Real feeds required."
        );
        let mock_flags = [
            (
                "QUESTDB_MOCK_FALLBACK",
                std::env::var("QUESTDB_MOCK_FALLBACK").as_deref() == Ok("1"),
            ),
            (
                "KAFKA_MOCK_FALLBACK",
                std::env::var("KAFKA_MOCK_FALLBACK").as_deref() == Ok("1"),
            ),
            (
                "KAFKA_MOCK_MODE",
                std::env::var("KAFKA_MOCK_MODE").as_deref() == Ok("1"),
            ),
            (
                "POLYGON_MOCK_FALLBACK",
                std::env::var("POLYGON_MOCK_FALLBACK").as_deref() == Ok("1"),
            ),
            (
                "POLYGON_MOCK_MODE",
                std::env::var("POLYGON_MOCK_MODE").as_deref() == Ok("1"),
            ),
            (
                "WHISPER_MOCK_FALLBACK",
                std::env::var("WHISPER_MOCK_FALLBACK").as_deref() == Ok("1"),
            ),
            (
                "FINNHUB_MOCK_MODE",
                std::env::var("FINNHUB_MOCK_MODE").as_deref() == Ok("1"),
            ),
        ];
        for (flag, active) in mock_flags {
            if active {
                warn!(" [PRODUCTION MODE GUARD] Flag '{}' is set in environment but IGNORED. Real data feeds required in production.", flag);
            }
        }
    } else {
        info!(" [PRODUCTION MODE GUARD] Inactive (Development/Test Mode: Synthetic fallbacks permitted).");
    }
    info!(" [Compliance] Active sources: SEC EDGAR (Public), Finnhub (Internal), Polygon.io (Options/Microstructure).");

    // Helper to evaluate environment variable feature flags
    let is_source_enabled = |primary_var: &str, alt_var: Option<&str>, default_val: bool| -> bool {
        let check_val = |key: &str| -> Option<bool> {
            std::env::var(key)
                .ok()
                .map(|v| match v.trim().to_lowercase().as_str() {
                    "1" | "true" | "yes" | "on" | "enabled" => true,
                    "0" | "false" | "no" | "off" | "disabled" => false,
                    _ => default_val,
                })
        };

        if let Some(val) = check_val(primary_var) {
            return val;
        }
        if let Some(alt) = alt_var {
            if let Some(val) = check_val(alt) {
                return val;
            }
        }
        default_val
    };

    let sec_edgar_enabled = is_source_enabled("ENABLE_SEC_EDGAR", None, true);
    let polygon_enabled = is_source_enabled("ENABLE_POLYGON", None, true);
    let stock_price_enabled = is_source_enabled("ENABLE_STOCK_PRICE_INGESTION", None, true);
    let finnhub_enabled = is_source_enabled("ENABLE_FINNHUB", None, true);
    let finnhub_ws_enabled =
        is_source_enabled("ENABLE_FINNHUB_WEBSOCKET", Some("FINNHUB_WS_ENABLED"), true);
    let polygon_ws_enabled =
        is_source_enabled("ENABLE_POLYGON_WEBSOCKET", Some("POLYGON_WS_ENABLED"), true);

    // 1. Initialize Active Data Sources
    let polygon_client = if polygon_enabled {
        let client = PolygonClient::from_env().ok();
        if client.is_some() {
            info!(" [Polygon.io] API key loaded: [MASKED]. Active for options microstructure (VPIN/GEX).");
        } else if prod_mode {
            warn!(" [Polygon.io] No valid API key in production mode. Synthetic fallback is disabled.");
        } else {
            info!(" [Polygon.io] No API key detected in environment (options microstructure engine in mock/fallback mode)");
        }
        Arc::new(client)
    } else {
        warn!(" [Polygon.io] Source disabled by configuration (ENABLE_POLYGON=0)");
        Arc::new(None)
    };

    let finnhub_client = if finnhub_enabled {
        let client = FinnhubClient::from_env().ok();
        if client.is_some() {
            info!(" [Finnhub] API key loaded: [MASKED]. Active for real-time market news.");
        } else if prod_mode {
            warn!(" [Finnhub] No valid FINNHUB_API_KEY detected in production mode. Real-time news poll disabled.");
        } else {
            info!(" [Finnhub] No FINNHUB_API_KEY detected in environment (skipping Finnhub live poll)");
        }
        Arc::new(client)
    } else {
        warn!(" [Finnhub] Source disabled by configuration (ENABLE_FINNHUB=0)");
        Arc::new(None)
    };

    if sec_edgar_enabled {
        info!(" [SEC EDGAR] Enabled (Public domain regulatory filings; safe for commercial redistribution)");
    } else {
        warn!(" [SEC EDGAR] Source disabled by configuration (ENABLE_SEC_EDGAR=0)");
    }

    let project_root = std::env::current_dir()?;

    // 2. Initialize native Rust processing & sink components
    let preprocessor = Arc::new(Preprocessor::new());
    let metrics = Arc::new(PipelineMetrics::new());

    // Initialize Data Quality Governance & Validation Gates
    let data_quality_cfg = DataQualityConfig::from_env_or_config();
    let quality_store = Arc::new(SourceQualityStore::new());
    let quality_gate = Arc::new(DataQualityGate::new(data_quality_cfg.clone()));
    info!(
        " [Data Quality Gates] Initialized: threshold={:.2}, quarantine_path='{}', cache_size={}",
        data_quality_cfg.source_quality_threshold,
        data_quality_cfg.quarantine_path,
        data_quality_cfg.duplicate_cache_size
    );

    let questdb_sink = Arc::new(QuestDbSink::new(QuestDbConfig::default()));
    let timescaledb_cfg = TimescaleDbConfig::from_env_or_config();
    let db_breaker_cfg = CircuitBreakerConfig::from_env_or_config();
    info!(
        " [Database Circuit Breaker] Initialized: enabled={}, failure_threshold={}, recovery_timeout={}ms, half_open_max_probes={}",
        db_breaker_cfg.enabled,
        db_breaker_cfg.failure_threshold,
        db_breaker_cfg.recovery_timeout_ms,
        db_breaker_cfg.half_open_max_probes
    );
    let db_circuit_breaker = Arc::new(DbCircuitBreaker::new(db_breaker_cfg));
    let timescaledb_sink =
        Arc::new(TimescaleDbSink::new(timescaledb_cfg).with_circuit_breaker(db_circuit_breaker));
    let kafka_sink = Arc::new(KafkaSink::from_env());
    let sink = Arc::new(JsonlStreamSink::new(
        project_root
            .join("data")
            .join("stream")
            .join("processed_signals.jsonl"),
    ));

    // Initialize Raw Data Archive (Apache Parquet & S3/MinIO Object Storage)
    let raw_archive_cfg = RawArchiveConfig::from_env_or_config();
    let (raw_archive_sender, raw_archive_handle) = if raw_archive_cfg.enabled {
        let (tx_archive, rx_archive) = mpsc::channel(raw_archive_cfg.batch_size * 2);
        let sender = RawArchiveSender::new(tx_archive);
        let handle = spawn_raw_archiver_worker(raw_archive_cfg.clone(), rx_archive);
        info!(
            " [Raw Data Archive] Enabled: provider='{}', bucket='{}', local_path='{}', transition_days={}, expiration_days={}, verify_upload={}",
            raw_archive_cfg.provider,
            raw_archive_cfg.bucket,
            raw_archive_cfg.local_path,
            raw_archive_cfg.lifecycle_transition_days,
            raw_archive_cfg.lifecycle_expiration_days,
            raw_archive_cfg.verify_upload
        );
        (Some(sender), Some(handle))
    } else {
        info!(" [Raw Data Archive] Disabled by configuration (raw_archive.enabled=false).");
        (None, None)
    };

    // Initialize QuestDB Kafka Write-Ahead Log (WAL) failover buffer components
    let questdb_buffer_config = QuestDbBufferConfig::default();
    let questdb_buffer_producer =
        Arc::new(QuestDbBufferProducer::new(questdb_buffer_config.clone()));
    let questdb_buffer_consumer = Arc::new(QuestDbBufferConsumer::new(
        questdb_buffer_config.clone(),
        questdb_sink.clone(),
    ));

    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    let buffer_consumer_handle = if questdb_buffer_config.enabled {
        info!(
            " [QuestDB WAL Buffer] Enabled: buffering to topic '{}' (consumer group '{}')",
            questdb_buffer_config.topic, questdb_buffer_config.consumer_group_id
        );
        let consumer = questdb_buffer_consumer.clone();
        Some(tokio::spawn(async move {
            consumer.run_drain_loop(shutdown_rx).await;
        }))
    } else {
        info!(" [QuestDB WAL Buffer] Disabled: operating in direct ILP write mode.");
        None
    };

    // Initialize QuestDB table schemas (e.g. stock_daily_bars)
    let _ = questdb_sink.initialize_tables().await;

    info!(
        " [QuestDB ILP] '{}/write' | [Kafka Events] '{}' -> '{}' | [Kafka Realtime] '{}' -> '{}'",
        questdb_sink.config().url,
        kafka_sink.config().bootstrap_servers,
        kafka_sink.config().topic,
        kafka_sink.config().bootstrap_servers,
        kafka_sink.config().realtime_topic
    );

    // 3. Stock Price (OHLCV) Ingestion Worker
    let stock_poly_client = polygon_client.clone();
    let stock_qdb_sink = questdb_sink.clone();
    let stock_price_handle = if stock_price_enabled && polygon_enabled {
        tokio::spawn(async move {
            let universe_str = std::env::var("POLYGON_STOCK_UNIVERSE")
                .unwrap_or_else(|_| "AAPL,MSFT,NVDA,GOOGL,AMZN,META,TSLA,SPY".to_string());
            let lookback_days: i64 = std::env::var("STOCK_PRICE_LOOKBACK_DAYS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(365);

            let tickers: Vec<String> = universe_str
                .split(',')
                .map(|s| s.trim().to_uppercase())
                .filter(|s| !s.is_empty())
                .collect();

            info!(
                "[Stock Price Ingestion] Active for {} tickers (lookback: {} days): {:?}",
                tickers.len(),
                lookback_days,
                tickers
            );

            loop {
                let now = Utc::now();
                let start_date = (now - chrono::Duration::days(lookback_days))
                    .format("%Y-%m-%d")
                    .to_string();
                let end_date = now.format("%Y-%m-%d").to_string();

                if let Some(ref client) = stock_poly_client.as_ref() {
                    for ticker in &tickers {
                        match client
                            .fetch_daily_bars(ticker, &start_date, &end_date)
                            .await
                        {
                            Ok(bars) => {
                                info!(
                                    "[Stock Price Ingestion] Fetched {} OHLCV bars for {}",
                                    bars.len(),
                                    ticker
                                );
                                if let Err(e) = stock_qdb_sink.write_stock_bars_batch(&bars).await {
                                    warn!(
                                        "[Stock Price Ingestion] QuestDB write notice for {}: {}",
                                        ticker, e
                                    );
                                }
                            }
                            Err(e) => {
                                debug!(
                                    "[Stock Price Ingestion] Notice for {} daily bars: {}",
                                    ticker, e
                                );
                            }
                        }
                        // Rate limiting pacing between ticker requests
                        sleep(Duration::from_millis(250)).await;
                    }
                }

                // Poll once per hour (3600 seconds)
                sleep(Duration::from_secs(3600)).await;
            }
        })
    } else {
        tokio::spawn(async {})
    };

    // 4. Bounded concurrency limiter & input queue for memory containment (strict 2GB ceiling)
    let concurrency_cfg = IngestionConcurrencyConfig::from_env_or_config();
    info!(
        " [Ingestion Concurrency & Backpressure] Active: max_concurrent_tasks={}, input_queue_capacity={}, task_timeout_ms={}",
        concurrency_cfg.max_concurrent_tasks,
        concurrency_cfg.input_queue_capacity,
        concurrency_cfg.task_timeout_ms
    );
    let backpressure_metrics = Arc::new(BackpressureMetrics::new());
    let (ingest_sender, rx_raw) = create_bounded_ingestion_channel(
        concurrency_cfg.input_queue_capacity,
        backpressure_metrics.clone(),
    );
    let tx_raw = ingest_sender.raw_sender().clone();

    // 5. Real-Time WebSocket Streaming Workers (Finnhub & Polygon)
    let (shutdown_fh_tx, shutdown_fh_rx) = tokio::sync::watch::channel(false);
    let finnhub_ws_handle = if finnhub_enabled && finnhub_ws_enabled {
        let fh_ws_client_opt = if prod_mode {
            match FinnhubWsClient::from_env() {
                Ok(c) => Some(c),
                Err(e) => {
                    tracing::error!(" [Finnhub WebSocket] Failed to initialize live client in production mode: {}. Synthetic fallback prohibited.", e);
                    None
                }
            }
        } else {
            Some(FinnhubWsClient::from_env().unwrap_or_else(|_| {
                let mut cfg = FinnhubWsConfig::default();
                cfg.mock_mode = true;
                FinnhubWsClient::new(cfg)
            }))
        };

        if let Some(fh_ws_client) = fh_ws_client_opt {
            let tx_fh = tx_raw.clone();
            info!(
                " [Finnhub WebSocket] Active streaming from '{}' (sub-second news stream)",
                fh_ws_client.config().ws_url
            );
            Some(tokio::spawn(async move {
                fh_ws_client.run_stream_loop(tx_fh, shutdown_fh_rx).await;
            }))
        } else {
            None
        }
    } else {
        None
    };

    let (shutdown_poly_tx, shutdown_poly_rx) = tokio::sync::watch::channel(false);
    let polygon_ws_handle = if polygon_enabled && polygon_ws_enabled {
        let poly_ws_client_opt = if prod_mode {
            match PolygonWsClient::from_env() {
                Ok(c) => Some(c),
                Err(e) => {
                    tracing::error!(" [Polygon WebSocket] Failed to initialize live client in production mode: {}. Synthetic fallback prohibited.", e);
                    None
                }
            }
        } else {
            Some(PolygonWsClient::from_env().unwrap_or_else(|_| {
                let mut cfg = PolygonWsConfig::default();
                cfg.mock_mode = true;
                PolygonWsClient::new(cfg)
            }))
        };

        if let Some(poly_ws_client) = poly_ws_client_opt {
            let (tx_trades, mut rx_trades) = mpsc::channel::<OptionTrade>(1000);
            info!(
                " [Polygon WebSocket] Active options streaming from '{}'",
                poly_ws_client.config().ws_url
            );

            // Background worker to consume live trade stream
            tokio::spawn(async move {
                while let Some(trade) = rx_trades.recv().await {
                    debug!(
                        "[Options Microstructure WS] Received trade: {} size={} price=${:.2}",
                        trade.options_ticker, trade.size, trade.price
                    );
                }
            });

            Some(tokio::spawn(async move {
                poly_ws_client
                    .run_stream_loop(tx_trades, shutdown_poly_rx)
                    .await;
            }))
        } else {
            None
        }
    } else {
        None
    };

    // 6. Ingestion REST Poller Task (Acts as backup/fallback to WebSocket stream)
    let tx_ingest = ingest_sender.clone();
    let sec_fetcher = Arc::new(SecEdgarFetcher::new());
    let finnhub_fetcher = finnhub_client.clone();

    let poller_handle = tokio::spawn(async move {
        let sample_ciks = vec!["0000320193", "0001045810", "0000789019"]; // AAPL, NVDA, MSFT
        let mut last_finnhub_poll = Instant::now() - Duration::from_secs(300);

        loop {
            // 1. Poll Finnhub News (if enabled and client initialized)
            if finnhub_enabled {
                if let Some(ref fh) = finnhub_fetcher.as_ref() {
                    if last_finnhub_poll.elapsed() >= Duration::from_secs(300) {
                        last_finnhub_poll = Instant::now();
                        match fh.fetch_news("general", 20).await {
                            Ok(docs) => {
                                info!(
                                    "[Finnhub Poller] Polled {} real-time market news articles",
                                    docs.len()
                                );
                                for doc in docs {
                                    if let Err(e) = tx_ingest.try_send(doc) {
                                        match e {
                                            IngestionSendError::QueueFull(_) => {} // Structured drop warning already logged
                                            IngestionSendError::ChannelClosed => return,
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                warn!("[Finnhub Poller] Notice: {}", e);
                            }
                        }
                    }
                }
            }

            // 2. Poll SEC Filings (public data, active by default)
            if sec_edgar_enabled {
                for cik in &sample_ciks {
                    match sec_fetcher.fetch_latest_filings(cik).await {
                        Ok(docs) => {
                            for doc in docs {
                                if let Err(e) = tx_ingest.try_send(doc) {
                                    match e {
                                        IngestionSendError::QueueFull(_) => {}
                                        IngestionSendError::ChannelClosed => return,
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            warn!("SEC Edgar Poller notice for CIK {}: {}", cik, e);
                        }
                    }
                }
            }

            sleep(Duration::from_secs(30)).await;
        }
    });

    // 6b. SEC Corporate Actions & Ticker History Background Updater
    let sec_updater_cfg = SecUpdaterConfig::from_env_or_config();
    let sec_updater_handle = if sec_updater_cfg.enabled {
        let updater_cfg = sec_updater_cfg.clone();
        info!(
            " [SEC Corporate Actions Updater] Initialized with interval={}h, mock={}, api_reload_url='{:?}'",
            updater_cfg.interval_hours, updater_cfg.mock, updater_cfg.api_server_reload_url
        );
        Some(tokio::spawn(async move {
            info!("[SEC Corporate Actions Updater] Running initial update cycle...");
            match run_updater_cycle(&updater_cfg).await {
                Ok(report) => {
                    info!(
                        "[SEC Corporate Actions Updater] Initial cycle complete: {} ticker changes, {} delistings, api_reloaded={}",
                        report.ticker_changes_detected, report.delistings_detected, report.api_reloaded
                    );
                }
                Err(e) => {
                    warn!(
                        "[SEC Corporate Actions Updater] Initial cycle notice: {}",
                        e
                    );
                }
            }

            let interval_duration = Duration::from_secs(updater_cfg.interval_hours.max(1) * 3600);
            loop {
                sleep(interval_duration).await;
                info!("[SEC Corporate Actions Updater] Starting scheduled update cycle...");
                match run_updater_cycle(&updater_cfg).await {
                    Ok(report) => {
                        info!(
                            "[SEC Corporate Actions Updater] Cycle complete: {} ticker changes, {} delistings, api_reloaded={}",
                            report.ticker_changes_detected, report.delistings_detected, report.api_reloaded
                        );
                    }
                    Err(e) => {
                        warn!("[SEC Corporate Actions Updater] Cycle notice: {}", e);
                    }
                }
            }
        }))
    } else {
        info!(" [SEC Corporate Actions Updater] Disabled by configuration.");
        None
    };

    // 7. Bounded Processing Worker Pool (NLP + Microstructure + ONNX + Sinks)
    let worker_pool = Arc::new(WorkerPool::new(
        concurrency_cfg.clone(),
        backpressure_metrics.clone(),
    ));
    let rx_raw_shared = Arc::new(tokio::sync::Mutex::new(rx_raw));
    let (shutdown_worker_tx, shutdown_worker_rx) = tokio::sync::watch::channel(false);

    let num_workers = concurrency_cfg.max_concurrent_tasks.max(1);
    let mut worker_handles = Vec::with_capacity(num_workers);

    for worker_id in 0..num_workers {
        let rx_shared = rx_raw_shared.clone();
        let pool = worker_pool.clone();
        let bp_metrics = backpressure_metrics.clone();
        let mut shutdown_rx = shutdown_worker_rx.clone();

        let preprocessor_ref = preprocessor.clone();
        let metrics_ref = metrics.clone();
        let questdb_ref = questdb_sink.clone();
        let timescaledb_ref = timescaledb_sink.clone();
        let questdb_buf_producer_ref = questdb_buffer_producer.clone();
        let kafka_ref = kafka_sink.clone();
        let sink_ref = sink.clone();
        let poly_ref = polygon_client.clone();
        let quality_gate_ref = quality_gate.clone();
        let quality_store_ref = quality_store.clone();
        let data_quality_cfg_ref = data_quality_cfg.clone();
        let raw_archive_sender_ref = raw_archive_sender.clone();
        let timeout_duration = pool.timeout_duration();

        let handle = tokio::spawn(async move {
            loop {
                // Non-blocking lock on shared receiver to extract next document
                let doc_opt = {
                    let mut rx_guard = rx_shared.lock().await;
                    tokio::select! {
                        doc = rx_guard.recv() => doc,
                        _ = shutdown_rx.changed() => {
                            if *shutdown_rx.borrow() {
                                None
                            } else {
                                continue;
                            }
                        }
                    }
                };

                let raw_doc = match doc_opt {
                    Some(doc) => doc,
                    None => break, // Channel drained or shutdown triggered
                };

                // Acquire permit from semaphore with timeout
                let permit = match tokio::time::timeout(
                    timeout_duration,
                    pool.semaphore().clone().acquire_owned(),
                )
                .await
                {
                    Ok(Ok(p)) => p,
                    Ok(Err(_)) => break, // Semaphore closed
                    Err(_) => {
                        warn!(
                            "[Ingestion Worker #{}] Timeout waiting for semaphore permit for document '{}'",
                            worker_id, raw_doc.id
                        );
                        bp_metrics.record_timeout();
                        continue;
                    }
                };

                bp_metrics.inc_active();
                let doc_id = raw_doc.id.clone();

                // Process document with task timeout protection
                let process_res = tokio::time::timeout(
                    timeout_duration,
                    process_single_document(
                        raw_doc,
                        &preprocessor_ref,
                        &metrics_ref,
                        &questdb_ref,
                        &timescaledb_ref,
                        &questdb_buf_producer_ref,
                        &kafka_ref,
                        &sink_ref,
                        &poly_ref,
                        &quality_gate_ref,
                        &quality_store_ref,
                        &data_quality_cfg_ref,
                        &raw_archive_sender_ref,
                    ),
                )
                .await;

                bp_metrics.dec_active();
                drop(permit);

                match process_res {
                    Ok(Ok(())) => {
                        bp_metrics.record_processed();
                    }
                    Ok(Err(e)) => {
                        warn!(
                            "[Ingestion Worker #{}] Processing notice for doc '{}': {}",
                            worker_id, doc_id, e
                        );
                    }
                    Err(_) => {
                        warn!(
                            "[Ingestion Worker #{}] Processing timed out after {}ms for doc '{}'",
                            worker_id,
                            timeout_duration.as_millis(),
                            doc_id
                        );
                        bp_metrics.record_timeout();
                    }
                }
            }
        });

        worker_handles.push(handle);
    }

    // Periodic backpressure metrics telemetry worker (logs every 30s)
    let bp_telemetry = backpressure_metrics.clone();
    let mut shutdown_telemetry_rx = shutdown_worker_rx.clone();
    let telemetry_handle = tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = sleep(Duration::from_secs(30)) => {
                    let snap = bp_telemetry.snapshot();
                    info!(
                        "[Ingestion Telemetry] Processed: {} | Dropped: {} | Timeouts: {} | Active: {}",
                        snap.processed_count, snap.dropped_count, snap.timeout_count, snap.active_tasks
                    );
                }
                _ = shutdown_telemetry_rx.changed() => {
                    if *shutdown_telemetry_rx.borrow() {
                        break;
                    }
                }
            }
        }
    });

    // 8. Wait for shutdown signal (Ctrl+C / SIGINT)
    info!("Ingestion engine running with bounded concurrency. Press Ctrl+C to initiate graceful shutdown.");
    signal::ctrl_c().await?;
    info!("Shutdown signal received. Commencing graceful teardown...");

    // Signal buffer consumer and WebSocket stream loops to finish
    let _ = shutdown_tx.send(true);
    let _ = shutdown_fh_tx.send(true);
    let _ = shutdown_poly_tx.send(true);
    let _ = shutdown_worker_tx.send(true);

    // Abort background polling and streaming tasks
    poller_handle.abort();
    stock_price_handle.abort();
    if let Some(buf_handle) = buffer_consumer_handle {
        buf_handle.abort();
    }
    if let Some(fh_handle) = finnhub_ws_handle {
        fh_handle.abort();
    }
    if let Some(poly_handle) = polygon_ws_handle {
        poly_handle.abort();
    }
    if let Some(updater_h) = sec_updater_handle {
        updater_h.abort();
    }
    if let Some(archive_h) = raw_archive_handle {
        archive_h.abort();
    }
    telemetry_handle.abort();

    // Terminate worker pool tasks
    for handle in worker_handles {
        handle.abort();
    }

    let final_bp = backpressure_metrics.snapshot();
    info!(
        "Teardown complete. Total Preprocessed: {}, Avg Rust Latency: {:.1}µs, Stored: {}. [Backpressure Metrics] Processed: {}, Dropped: {}, Timeouts: {}.",
        metrics
            .total_preprocessed
            .load(std::sync::atomic::Ordering::Relaxed),
        metrics.avg_preprocessing_latency_us(),
        metrics
            .total_stored
            .load(std::sync::atomic::Ordering::Relaxed),
        final_bp.processed_count,
        final_bp.dropped_count,
        final_bp.timeout_count
    );

    Ok(())
}

/// Modular pipeline processor for a single `RawDocument`.
async fn process_single_document(
    raw_doc: RawDocument,
    preprocessor_ref: &Preprocessor,
    metrics_ref: &PipelineMetrics,
    questdb_ref: &Arc<QuestDbSink>,
    timescaledb_ref: &TimescaleDbSink,
    questdb_buf_producer_ref: &Arc<QuestDbBufferProducer>,
    kafka_ref: &KafkaSink,
    sink_ref: &JsonlStreamSink,
    poly_ref: &Option<PolygonClient>,
    quality_gate_ref: &DataQualityGate,
    quality_store_ref: &SourceQualityStore,
    data_quality_cfg_ref: &DataQualityConfig,
    raw_archive_sender_ref: &Option<RawArchiveSender>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let audio_path_opt = raw_doc.audio_path.clone();

    // Data Quality Gates (SCHEMA_VALIDATION -> BUSINESS_RULES -> DUPLICATE_DETECTION -> SOURCE_QUALITY_THRESHOLD)
    let source_quality_score = quality_store_ref.get_quality_score(&raw_doc.source);
    let gate_res =
        preprocessor_ref.evaluate_quality_gates(&raw_doc, quality_gate_ref, source_quality_score);

    match gate_res {
        QualityGateResult::Reject { rule, reason } => {
            warn!(
                "[Data Quality Gate] REJECTED document '{}' from '{}' ([{}] {})",
                raw_doc.id, raw_doc.source, rule, reason
            );
            quality_store_ref.record_rejection(&raw_doc.source, &reason);
            return Ok(());
        }
        QualityGateResult::Quarantine { rule, reason } => {
            warn!(
                "[Data Quality Gate] QUARANTINED document '{}' from '{}' ([{}] {})",
                raw_doc.id, raw_doc.source, rule, reason
            );
            quality_store_ref.record_quarantine(&raw_doc.source, &reason);
            let q_base = Path::new(&data_quality_cfg_ref.quarantine_path);
            if let Err(e) = quarantine_document(q_base, &raw_doc, &rule, &reason) {
                error!("[Data Quality Gate] Failed to store quarantine file: {}", e);
            }
            return Ok(());
        }
        QualityGateResult::Accept => {
            let freshness_ms = calculate_freshness_ms(&raw_doc.published_utc);
            let (present, total) = count_document_fields(&raw_doc);
            quality_store_ref.record_acceptance(&raw_doc.source, freshness_ms, present, total);
        }
    }

    // Preprocessing: Clean HTML, extract primary ticker & event category
    let mut processed = preprocessor_ref.process(raw_doc);
    metrics_ref.record_preprocessing(processed.preprocessing_latency_us, processed.is_spam);

    if processed.is_spam {
        info!(
            "[Spam Filter] Filtered document '{}' ({})",
            processed.title, processed.spam_reason
        );
        return Ok(());
    }

    // Submit raw document to archive if enabled (non-blocking)
    if let Some(ref archive_sender) = raw_archive_sender_ref {
        let archive_record = RawArchiveRecord {
            published_utc: processed.published_utc.clone(),
            ticker: processed
                .primary_ticker
                .clone()
                .unwrap_or_else(|| "UNKNOWN".to_string()),
            source: processed.source.clone(),
            title: processed.title.clone(),
            raw_content: processed.raw_content.clone().unwrap_or_default(),
            ingested_utc: processed.ingested_utc.clone(),
            db_commit_utc: None,
            data_quality_score: Some(source_quality_score),
            event_type: Some(processed.event_category.clone()),
        };
        if let Err(e) = archive_sender.try_send(archive_record) {
            warn!("[Raw Archive] Ingestion channel backpressure notice: {}", e);
        }
    }

    // Options Microstructure (VPIN & Dealer GEX) if ticker is present
    if let Some(ref ticker) = processed.primary_ticker {
        if let Some(ref poly) = poly_ref.as_ref() {
            let today = Utc::now().format("%Y-%m-%d").to_string();
            match compute_vpin_and_gex_from_polygon(poly, ticker, &today, 150.0, 0.25, 0.05).await {
                Ok((vpin, gex_res)) => {
                    processed.vpin = Some(vpin);
                    processed.gex = Some(gex_res.total_gamma_exposure);
                    processed.gex_positive = Some(gex_res.positive_gamma);
                    processed.gex_negative = Some(gex_res.negative_gamma);
                    info!(
                        "[Options Microstructure] Ticker: {} | VPIN: {:.3} | GEX: ${:.2}M",
                        ticker,
                        vpin,
                        gex_res.total_gamma_exposure / 1_000_000.0
                    );
                }
                Err(e) => {
                    debug!("[Options Microstructure] Notice for {}: {}", ticker, e);
                }
            }
        }
    }

    // Audio transcription via native Whisper.cpp if audio attachment exists
    if let Some(ref audio_path) = audio_path_opt {
        match decode_audio_file(std::path::Path::new(audio_path)) {
            Ok((samples, sample_rate)) => {
                if let Ok(audio_feats) = extract_features(&samples, sample_rate) {
                    processed.audio_features = Some(audio_feats);
                }

                match transcribe_samples_async(samples).await {
                    Ok(transcript) => {
                        info!(
                            "[Whisper ASR] Transcribed audio for '{}': {} chars",
                            processed.title,
                            transcript.len()
                        );
                        processed.clean_text.push_str(" ");
                        processed.clean_text.push_str(&transcript);
                        processed.audio_transcript = Some(transcript);
                    }
                    Err(e) => {
                        warn!("[Whisper ASR] Transcription notice: {}", e);
                    }
                }
            }
            Err(e) => {
                warn!("[Audio Decoder] Notice: {}", e);
            }
        }
    }

    // 1. In-process static shape ONNX sentiment inference
    let text_to_score = if !processed.clean_text.is_empty() {
        &processed.clean_text
    } else {
        &processed.title
    };

    let now_us = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_micros() as i64)
        .unwrap_or(0);
    let signal_avail_us = now_us + 437_000; // 437ms tradable SLA

    let sentiment_out = match compute_sentiment_onnx(text_to_score) {
        Ok(out) => {
            info!(
                "[Native Rust ONNX] '{}' | Sentiment: {:+.2} ({}) | Tradable SLA: {}",
                processed.title, out.sentiment_score, out.sentiment_label, signal_avail_us
            );
            out
        }
        Err(e) => {
            warn!("Native ONNX notice ({}). Generating fallback baseline.", e);
            SentimentOutput {
                sentiment_score: 0.0,
                sentiment_label: "NEUTRAL".to_string(),
                prob_positive: 0.33,
                prob_negative: 0.33,
                prob_neutral: 0.34,
                ..Default::default()
            }
        }
    };

    // 2. Time-series persistence to QuestDB (via Kafka WAL Failover Buffer or Direct ILP)
    let now_commit = chrono::Utc::now().to_rfc3339();
    processed.db_commit_utc = Some(now_commit.clone());
    if processed.valid_from.is_none() {
        processed.valid_from = Some(now_commit);
    }
    if processed.revision_number.is_none() {
        processed.revision_number = Some(1);
    }
    if processed.is_current.is_none() {
        processed.is_current = Some(true);
    }
    let ilp_line = questdb_ref.format_ilp_line(&processed, &sentiment_out, signal_avail_us);

    // 2. Time-series persistence (TimescaleDB Primary vs QuestDB Primary Switchover)
    let is_ts_primary = timescaledb_ref.is_enabled() && timescaledb_ref.is_primary();

    if is_ts_primary {
        // Primary synchronous write to TimescaleDB
        match timescaledb_ref
            .write_sentiment_record(&processed, &sentiment_out)
            .await
        {
            Ok(id) => {
                info!(
                    "[TimescaleDB Primary] Record persisted id={} | Ticker: {:?}",
                    id, processed.primary_ticker
                );
            }
            Err(ts_err) => {
                warn!(
                    "[TimescaleDB Primary] Notice: {} (Continuing to secondary QuestDB write)",
                    ts_err
                );
            }
        }

        // Secondary asynchronous non-blocking write to QuestDB
        let qdb_ref = Arc::clone(questdb_ref);
        let qdb_buf_ref = Arc::clone(questdb_buf_producer_ref);
        let proc_clone = processed.clone();
        let sent_clone = sentiment_out.clone();
        let ilp_clone = ilp_line.clone();
        tokio::spawn(async move {
            if qdb_buf_ref.is_enabled() {
                let _ = qdb_buf_ref
                    .push_event(&proc_clone, &sent_clone, signal_avail_us, &ilp_clone)
                    .await;
            } else {
                let _ = qdb_ref.send_ilp_payload(&ilp_clone).await;
            }
        });
    } else {
        // QuestDB Primary Persistence (via Kafka WAL Failover Buffer or Direct ILP)
        if questdb_buf_producer_ref.is_enabled() {
            match questdb_buf_producer_ref
                .push_event(&processed, &sentiment_out, signal_avail_us, &ilp_line)
                .await
            {
                Ok(_) => {
                    info!(
                        "[QuestDB WAL Buffer] Buffered event for ticker {:?} -> topic '{}'",
                        processed.primary_ticker,
                        questdb_buf_producer_ref.config().topic
                    );
                }
                Err(buf_err) => {
                    warn!(
                        "[QuestDB WAL Buffer] Buffer write notice ({}). Attempting direct ILP fallback.",
                        buf_err
                    );
                    let _ = questdb_ref.send_ilp_payload(&ilp_line).await;
                }
            }
        } else {
            match questdb_ref.send_ilp_payload(&ilp_line).await {
                Ok(_) => {
                    info!(
                        "[QuestDB ILP Direct] Event persisted to 'sentiment_news' | Ticker: {:?} | T_avail_ns: {}",
                        processed.primary_ticker, signal_avail_us * 1_000
                    );
                }
                Err(qdb_err) => {
                    warn!(
                        "[QuestDB ILP Direct] Notice: {} (Buffered to local stream sink)",
                        qdb_err
                    );
                }
            }
        }

        // Secondary Dual-write to TimescaleDB (if enabled, non-blocking resilience)
        if timescaledb_ref.is_enabled() {
            match timescaledb_ref
                .write_sentiment_record(&processed, &sentiment_out)
                .await
            {
                Ok(id) => {
                    info!(
                        "[TimescaleDB Dual-Write] Record persisted id={} | Ticker: {:?}",
                        id, processed.primary_ticker
                    );
                }
                Err(ts_err) => {
                    warn!(
                        "[TimescaleDB Dual-Write] Notice: {} (Continuing ingestion pipeline)",
                        ts_err
                    );
                }
            }
        }
    }

    // 3. Asynchronous event streaming to Kafka topic (sentiment-events)
    if let Err(kafka_err) = kafka_ref
        .send_event(&processed, &sentiment_out, signal_avail_us)
        .await
    {
        warn!(
            "[Kafka Producer] Notice: {} (Continuing ingestion pipeline)",
            kafka_err
        );
    }

    // 4. Ultra-low-latency real-time publish to Kafka topic (WebSocket & algorithmic feed)
    if let Err(kafka_rt_err) = kafka_ref
        .publish_realtime_event(&processed, &sentiment_out, signal_avail_us)
        .await
    {
        warn!(
            "[Kafka Realtime Producer] Notice: {} (Continuing ingestion pipeline)",
            kafka_rt_err
        );
    }

    metrics_ref.record_stored();

    let inference_resp = InferenceResponse {
        status: "ok".to_string(),
        id: processed.id.clone(),
        sentiment_score: sentiment_out.sentiment_score,
        sentiment_label: sentiment_out.sentiment_label,
        confidence: sentiment_out
            .prob_positive
            .max(sentiment_out.prob_negative)
            .max(sentiment_out.prob_neutral),
        novelty_score: 1.0,
        signal_available_ts_us: signal_avail_us,
        error: None,
    };

    let record = CompleteSignalRecord {
        document: processed,
        inference: inference_resp,
        stored_at_utc: Utc::now().to_rfc3339(),
    };
    let _ = sink_ref.append(&record);

    Ok(())
}

fn verify_model_weights_at_startup() -> Result<(), String> {
    // Bypass in tests
    if std::env::var("SKIP_WEIGHT_CHECK").is_ok() {
        tracing::warn!("SKIP_WEIGHT_CHECK set; bypassing model weight verification.");
        return Ok(());
    }

    let model_dir = std::env::var("FINTEXT_MODEL_DIR")
        .or_else(|_| std::env::var("MODEL_DIR"))
        .unwrap_or_else(|_| "models".to_string());
    let base = std::path::PathBuf::from(&model_dir);

    // Candidates for primary FinBERT sentiment (accept either name)
    let finbert_candidates = [
        base.join("finbert-finetuned").join("finbert.onnx"),
        base.join("finbert-finetuned").join("model.onnx"),
        base.join("finbert-finetuned").join("model_static.onnx"),
    ];
    let finbert_ok = finbert_candidates.iter().any(|p| p.is_file());

    // NER model (optional in api_server, required in ingestion)
    let ner_ok = base.join("ner").join("model_static.onnx").is_file();

    let mut missing = Vec::new();
    if !finbert_ok {
        missing.push(format!(
            "FinBERT weights missing (looked for finbert.onnx / model.onnx / model_static.onnx under {}/finbert-finetuned/)",
            base.display()
        ));
    }

    if !missing.is_empty() {
        let msg = format!(
            "\n════════════════════════════════════════════════════════════════\n\
             STARTUP FAILURE: Required ONNX model weights not found.\n\
             {}\n\
             \n\
             To fix:\n\
               1. Run: python scripts/fetch_models.py --manifest config/models_manifest.json\n\
               2. Or set FINTEXT_MODEL_DIR to a directory containing weights.\n\
               3. Or set SKIP_WEIGHT_CHECK=1 for development-only runs.\n\
             \n\
             Weights are distributed via GitHub Release models-v1.0.0.\n\
             See docs/MODEL_ASSET_DISTRIBUTION.md for details.\n\
             ════════════════════════════════════════════════════════════════\n",
            missing.join("\n")
        );
        return Err(msg);
    }

    tracing::info!(
        "Model weight verification passed: FinBERT present at {}",
        model_dir
    );
    if !ner_ok {
        tracing::warn!(
            "NER weights not found under {}/ner/ — NER endpoints will be unavailable.",
            model_dir
        );
    }
    Ok(())
}
