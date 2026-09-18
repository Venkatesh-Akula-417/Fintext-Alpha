//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Axum HTTP & WebSocket Server Entrypoint
//! ═══════════════════════════════════════════════════════════════════════════════

use fintext_api_server::{create_app_with_state, AppState, KafkaSubscriber};
use std::env;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::signal;
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Build multi-threaded Tokio runtime bounded to 4 worker threads (hardware containment)
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()?;

    runtime.block_on(async_main())
}

async fn async_main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Initialize structured logging
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    info!("═══════════════════════════════════════════════════════════════════════════");
    info!(" FinText-Alpha-Vectorizer — Native Rust Axum API Gateway & WebSocket Push");
    info!("═══════════════════════════════════════════════════════════════════════════");

    if let Err(e) = verify_model_weights_at_startup() {
        eprintln!("{}", e);
        std::process::exit(1);
    }

    let prod_mode = fintext_api_server::is_production_mode();
    if prod_mode {
        info!(" [Production Mode Guard] ACTIVE (PRODUCTION_MODE=true). All synthetic/mock fallbacks DISABLED.");
        let mock_flags = [
            (
                "QUESTDB_MOCK_FALLBACK",
                env::var("QUESTDB_MOCK_FALLBACK").as_deref() == Ok("1"),
            ),
            (
                "KAFKA_MOCK_FALLBACK",
                env::var("KAFKA_MOCK_FALLBACK").as_deref() == Ok("1"),
            ),
            (
                "KAFKA_MOCK_MODE",
                env::var("KAFKA_MOCK_MODE").as_deref() == Ok("1"),
            ),
            (
                "POLYGON_MOCK_FALLBACK",
                env::var("POLYGON_MOCK_FALLBACK").as_deref() == Ok("1"),
            ),
            (
                "POLYGON_MOCK_MODE",
                env::var("POLYGON_MOCK_MODE").as_deref() == Ok("1"),
            ),
            (
                "WHISPER_MOCK_FALLBACK",
                env::var("WHISPER_MOCK_FALLBACK").as_deref() == Ok("1"),
            ),
            (
                "FINNHUB_MOCK_MODE",
                env::var("FINNHUB_MOCK_MODE").as_deref() == Ok("1"),
            ),
        ];
        for (flag, active) in mock_flags {
            if active {
                warn!(" [Production Mode Guard] Flag '{}' is set in environment but IGNORED. Synthetic data is prohibited in production.", flag);
            }
        }
    } else {
        info!(" [Production Mode Guard] INACTIVE (development/testing mode). Mocks and fallbacks enabled.");
    }

    // 1.1 Initialize Secrets Provider (Local Environment vs AWS Secrets Manager)
    let secrets_cfg = fintext_api_server::SecretsConfig::from_env_or_config();
    info!(
        "[Secrets Manager] Provider: '{}' (Region: '{}', ARN: '{}')",
        secrets_cfg.provider,
        secrets_cfg.aws_region,
        if secrets_cfg.aws_secret_arn.is_empty() {
            "[NONE]"
        } else {
            &secrets_cfg.aws_secret_arn
        }
    );

    let secrets_provider = match fintext_api_server::init_secrets_provider(&secrets_cfg).await {
        Ok(p) => p,
        Err(e) => {
            tracing::error!(
                "[Secrets Manager] CRITICAL: Failed to initialize secrets provider: {}",
                e
            );
            std::process::exit(1);
        }
    };

    let required_secrets = match fintext_api_server::validate_required_secrets(
        secrets_provider.as_ref(),
        &secrets_cfg.provider,
        prod_mode,
    ) {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(
                "[Secrets Manager] CRITICAL: Required secrets validation failed: {}",
                e
            );
            std::process::exit(1);
        }
    };

    // 2. Initialize Kafka real-time subscriber and spawn background task
    let kafka_consumer = Arc::new(KafkaSubscriber::from_env());
    let _sub_handle = kafka_consumer.clone().start_subscription();

    info!(
        "[Kafka Subscriber] Subscribed to broker='{}', topic='{}', group='{}' (WS capacity: {})",
        kafka_consumer.config().bootstrap_servers,
        kafka_consumer.config().topic,
        kafka_consumer.config().group_id,
        kafka_consumer.config().channel_capacity
    );

    // 3. Initialize PostgreSQL Connection Pool and Usage Metering Worker
    let db_url = required_secrets.database_url.clone();

    let (db_pool, is_db_connected) = match sqlx::postgres::PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(std::time::Duration::from_millis(1500))
        .connect(&db_url)
        .await
    {
        Ok(pool) => {
            if let Err(e) = fintext_api_server::metering::init_db(&pool).await {
                warn!(
                    "[Usage Metering] Failed to verify/init 'usage_events' table schema: {}",
                    e
                );
            }
            if let Err(e) = fintext_api_server::webhooks::init_webhooks_db(&pool).await {
                warn!(
                    "[Webhooks] Failed to verify/init 'webhooks' table schema: {}",
                    e
                );
            }
            if let Err(e) = fintext_api_server::users::init_users_db(&pool).await {
                warn!(
                    "[Users & API Keys] Failed to verify/init 'users' and 'api_keys' schema: {}",
                    e
                );
            }
            if let Err(e) = fintext_api_server::universes::init_universes_db(&pool).await {
                warn!(
                    "[Universes] Failed to verify/init 'universes' schema: {}",
                    e
                );
            }
            if let Err(e) = fintext_api_server::TranscriptRegistry::init_db(&pool).await {
                warn!(
                    "[Transcripts] Failed to verify/init 'earnings_call_transcripts' schema: {}",
                    e
                );
            }
            if let Err(e) = fintext_api_server::ip_whitelist::init_ip_whitelist_db(&pool).await {
                warn!(
                    "[IP Whitelist] Failed to verify/init 'ip_whitelist' schema: {}",
                    e
                );
            }
            if let Err(e) =
                fintext_api_server::news_articles::NewsArticleRegistry::init_db(&pool).await
            {
                warn!(
                    "[News Articles] Failed to verify/init 'news_articles' schema: {}",
                    e
                );
            }
            if let Err(e) = fintext_api_server::init_audit_logs_table(&pool).await {
                warn!(
                    "[Audit Logs] Failed to verify/init 'audit_logs' schema: {}",
                    e
                );
            }
            if let Err(e) = fintext_api_server::DigestSubscriptionRegistry::init_db(&pool).await {
                warn!(
                    "[Email Digest] Failed to verify/init 'email_digest_subscriptions' schema: {}",
                    e
                );
            }
            if let Err(e) = fintext_api_server::KafkaCredentialsRegistry::init_db(&pool).await {
                warn!(
                    "[Kafka Streaming] Failed to verify/init 'kafka_credentials' schema: {}",
                    e
                );
            }
            if let Err(e) = fintext_api_server::RetentionPolicyRegistry::init_db(&pool).await {
                warn!(
                    "[Data Retention] Failed to verify/init 'data_retention_policies' schema: {}",
                    e
                );
            }
            if let Err(e) = fintext_api_server::PollingWebhookRegistry::init_db(&pool).await {
                warn!(
                    "[Polling Webhooks] Failed to verify/init 'polling_webhooks' schema: {}",
                    e
                );
            }
            if let Err(e) = fintext_api_server::ChatAlertRegistry::init_db(&pool).await {
                warn!(
                    "[Chat Alerts] Failed to verify/init 'chat_alert_subscriptions' schema: {}",
                    e
                );
            }
            if let Err(e) = fintext_api_server::FixOrderRegistry::init_db(&pool).await {
                warn!(
                    "[FIX Protocol] Failed to verify/init 'fix_orders' schema: {}",
                    e
                );
            }
            if let Err(e) = fintext_api_server::DlqRegistry::init_db(&pool).await {
                warn!(
                    "[DLQ Registry] Failed to verify/init 'dlq_events' schema: {}",
                    e
                );
            }
            if let Err(e) = fintext_api_server::provenance::init_data_provenance_table(&pool).await
            {
                warn!(
                    "[Data Provenance] Failed to verify/init 'data_provenance' schema: {}",
                    e
                );
            }
            info!("[Auth, Metering, Webhooks, Universes, Transcripts, IP Whitelist, News, Digest, Kafka, Retention, Polling Webhooks, Chat Alerts, FIX, DLQ, Provenance & Audit Logs] PostgreSQL database connected successfully");
            (Some(pool), true)
        }
        Err(e) => {
            warn!(
                "[Auth, Metering, Webhooks, Universes, Transcripts, IP Whitelist, News, Digest, Kafka, FIX, DLQ, Provenance & Audit Logs] PostgreSQL not available at '{}': {}. Events will be tracked in-memory/fallback mode.",
                db_url.split('@').next().unwrap_or("postgres://..."),
                e
            );
            (None, false)
        }
    };

    let (metering_tx, metering_rx) = fintext_api_server::metering::create_metering_channel(
        fintext_api_server::DEFAULT_METERING_CHANNEL_CAPACITY,
    );
    let _metering_worker_handle = fintext_api_server::metering::spawn_metering_worker(
        metering_rx,
        db_pool.clone(),
        fintext_api_server::metering::MeteringWorkerConfig::default(),
    );

    // 4. Build Axum application router with shared state (including JWT, Rate Limiting, Metering, Webhooks, Universes, Transcripts, IP Whitelist, News Articles, Email Digests & Kafka Streaming)
    let jwt_secret = required_secrets.jwt_secret.clone();
    let admin_token = required_secrets.admin_token.clone();

    let rate_limiter = Arc::new(fintext_api_server::PerUserRateLimiter::from_env());
    let rate_cfg = rate_limiter.config();

    let webhook_registry = fintext_api_server::WebhookRegistry::new();
    let user_registry = fintext_api_server::UserRegistry::new();
    let api_key_registry = fintext_api_server::ApiKeyRegistry::new();
    let universe_registry = fintext_api_server::UniverseRegistry::new();
    let transcript_registry = fintext_api_server::TranscriptRegistry::new();
    let news_article_registry =
        Arc::new(fintext_api_server::news_articles::NewsArticleRegistry::new());
    let audit_log_registry = Arc::new(fintext_api_server::AuditLogRegistry::new());
    let digest_registry = Arc::new(fintext_api_server::DigestSubscriptionRegistry::new());
    let email_sender: Arc<dyn fintext_api_server::EmailSender> =
        Arc::new(fintext_api_server::MockEmailSender::new());
    let kafka_credentials_registry = Arc::new(fintext_api_server::KafkaCredentialsRegistry::new());
    let retention_registry = Arc::new(fintext_api_server::RetentionPolicyRegistry::new());
    let polling_webhook_registry = Arc::new(fintext_api_server::PollingWebhookRegistry::new());
    let chat_alert_registry = Arc::new(fintext_api_server::ChatAlertRegistry::new());
    let retraining_registry = Arc::new(fintext_api_server::retraining::RetrainingRegistry::new());
    let fix_order_registry = Arc::new(fintext_api_server::FixOrderRegistry::new());
    let dlq_registry = Arc::new(fintext_api_server::DlqRegistry::new());
    let provenance_registry = Arc::new(fintext_api_server::ProvenanceRegistry::new());

    if let Some(ref pool) = db_pool {
        if let Err(e) = news_article_registry.load_from_db(pool).await {
            warn!(
                "[News Articles] Failed to load articles from database: {}",
                e
            );
        }
        if let Err(e) = digest_registry.load_from_db(pool).await {
            warn!(
                "[Email Digest] Failed to load digest subscriptions from database: {}",
                e
            );
        }
        if let Err(e) = kafka_credentials_registry.load_from_db(pool).await {
            warn!(
                "[Kafka Streaming] Failed to load kafka credentials from database: {}",
                e
            );
        }
        if let Err(e) = retention_registry.load_from_db(pool).await {
            warn!(
                "[Data Retention] Failed to load retention policies from database: {}",
                e
            );
        }
        if let Err(e) = polling_webhook_registry.load_from_db(pool).await {
            warn!(
                "[Polling Webhooks] Failed to load polling webhooks from database: {}",
                e
            );
        }
        if let Err(e) = chat_alert_registry.load_from_db(pool).await {
            warn!(
                "[Chat Alerts] Failed to load chat alert subscriptions from database: {}",
                e
            );
        }
        if let Err(e) = retraining_registry.load_from_db(pool).await {
            warn!(
                "[Model Retraining] Failed to load retraining jobs from database: {}",
                e
            );
        }
        if let Err(e) = fix_order_registry.load_from_db(pool).await {
            warn!(
                "[FIX Protocol] Failed to load FIX orders from database: {}",
                e
            );
        }
        if let Err(e) = dlq_registry.load_from_db(pool).await {
            warn!(
                "[DLQ Registry] Failed to load DLQ events from database: {}",
                e
            );
        }
    }

    info!(
        "[Security] JWT Auth, API Keys & Per-User Rate Limiting initialized (Limit: {} reqs / {}s, Secret: [MASKED], Admin Token: [MASKED], DB: {})",
        rate_cfg.requests_per_window,
        rate_cfg.window_seconds,
        if is_db_connected { "ONLINE" } else { "OFFLINE/FALLBACK" }
    );

    let model_metadata = Arc::new(std::sync::RwLock::new(
        fintext_api_server::models::ModelMetadata::from_env_or_config(),
    ));

    // ── Database Circuit Breaker & Retry Policy (Suite #271) ─────────────────
    let db_breaker_cfg = fintext_api_server::CircuitBreakerConfig::from_env_or_config();
    info!(
        "[Database Circuit Breaker] Initialized: enabled={}, failure_threshold={}, recovery_timeout={}ms, half_open_max_probes={}",
        db_breaker_cfg.enabled,
        db_breaker_cfg.failure_threshold,
        db_breaker_cfg.recovery_timeout_ms,
        db_breaker_cfg.half_open_max_probes
    );
    let db_circuit_breaker = Arc::new(fintext_api_server::DbCircuitBreaker::new(db_breaker_cfg));

    // ── In-Memory Cache TTL & Capacity Governance (Suite #272) ───────────────
    let cache_config = fintext_api_server::CacheConfig::from_env_or_config();
    info!(
        "[Cache Governance] Initialized: default_ttl={}s, max_capacity={}, cleanup_interval={}s",
        cache_config.default_ttl_secs,
        cache_config.default_max_capacity,
        cache_config.cleanup_interval_secs
    );
    let provider_health_store = Arc::new(
        fintext_api_server::handlers::provider_health::ProviderHealthStore::with_ttl_secs(
            60,
            cache_config.default_max_capacity,
        ),
    );

    let timescale_cfg = fintext_api_server::storage::TimescaleDbClientConfig::from_env_or_config();
    let timescaledb_primary = timescale_cfg.primary;
    let timescaledb_client = Arc::new(
        fintext_api_server::storage::TimescaleDbClient::new(timescale_cfg)
            .with_circuit_breaker(db_circuit_breaker.clone()),
    );
    if timescaledb_client.config().mock_mode
        || std::env::var("TIMESCALE_MOCK_MODE").as_deref() == Ok("1")
    {
        timescaledb_client.seed_mock_records();
    }

    // ── Point-in-Time (PIT) Database Store (Suite #269) ───────────────────────
    let pit_db_cfg = fintext_api_server::pit_db::PitDatabaseConfig::from_env_or_config();
    let (pit_db_store, active_pit_data) = if pit_db_cfg.enabled {
        match fintext_api_server::pit_db::PitDatabaseStore::connect(pit_db_cfg.clone()).await {
            Ok(store) => {
                info!(
                    "[PIT Database] PostgreSQL PIT store connected successfully at {}",
                    pit_db_cfg.url
                );
                if let Err(e) = store.init_db().await {
                    warn!("[PIT Database] Failed to initialize/verify schema: {}", e);
                }
                let data = match store.load_snapshot(None).await {
                    Ok(snapshot)
                        if !snapshot.ticker_intervals.is_empty()
                            || !snapshot.delisted_tickers.is_empty() =>
                    {
                        info!(
                            "[PIT Database] Loaded Point-in-Time data directly from PostgreSQL: {} ticker intervals, {} delisted securities",
                            snapshot.ticker_intervals.len(), snapshot.delisted_tickers.len()
                        );
                        let _ = fintext_api_server::GLOBAL_PIT_DATA
                            .reload_from_snapshot(snapshot.clone());
                        Arc::new(fintext_api_server::PITData::from_snapshot(snapshot))
                    }
                    Ok(_) => {
                        if pit_db_cfg.fallback_to_json {
                            warn!("[PIT Database] Database tables are empty. Falling back to JSON configuration files.");
                            fintext_api_server::GLOBAL_PIT_DATA.clone()
                        } else {
                            info!("[PIT Database] Database tables are empty, initialized empty snapshot (fallback_to_json=false).");
                            Arc::new(fintext_api_server::PITData::empty())
                        }
                    }
                    Err(e) => {
                        if pit_db_cfg.fallback_to_json {
                            warn!("[PIT Database] Failed loading PIT data from database ({}). Falling back to JSON.", e);
                            fintext_api_server::GLOBAL_PIT_DATA.clone()
                        } else {
                            error!("[PIT Database] Failed loading PIT data from database and fallback_to_json=false: {}", e);
                            panic!("Fatal: Failed to load PIT data from PostgreSQL: {}", e);
                        }
                    }
                };
                (Some(Arc::new(store)), data)
            }
            Err(e) => {
                if pit_db_cfg.fallback_to_json {
                    warn!("[PIT Database] PostgreSQL connection failed ({}). Gracefully falling back to JSON configuration files.", e);
                    (None, fintext_api_server::GLOBAL_PIT_DATA.clone())
                } else {
                    error!("[PIT Database] PostgreSQL connection failed and fallback_to_json=false: {}", e);
                    panic!(
                        "Fatal: Database-backed PIT required but PostgreSQL connection failed: {}",
                        e
                    );
                }
            }
        }
    } else {
        (None, fintext_api_server::GLOBAL_PIT_DATA.clone())
    };

    let state = AppState {
        kafka_consumer: kafka_consumer.clone(),
        jwt_secret,
        admin_token,
        rate_limiter,
        metering_tx: Some(metering_tx),
        db_pool: db_pool.clone(),
        webhook_registry,
        user_registry,
        api_key_registry,
        universe_registry,
        transcript_registry,
        org_registry: fintext_api_server::orgs::OrgRegistry::new(),
        ip_whitelist_registry: Arc::new(
            fintext_api_server::ip_whitelist::IpWhitelistRegistry::new(),
        ),
        news_article_registry: news_article_registry.clone(),
        pit_data: active_pit_data,
        sector_map: fintext_api_server::GLOBAL_SECTOR_MAP.clone(),
        symbol_map: fintext_api_server::GLOBAL_SYMBOL_MAP.clone(),
        supply_chain_graph: fintext_api_server::GLOBAL_SUPPLY_CHAIN_GRAPH.clone(),
        stripe_secret_key: secrets_provider.get("STRIPE_SECRET_KEY"),
        stripe_webhook_secret: secrets_provider.get("STRIPE_WEBHOOK_SECRET"),
        monthly_quota_cache: std::sync::Arc::new(fintext_api_server::MonthlyQuotaCache::default()),
        audit_log_registry,
        model_metadata,
        digest_registry: digest_registry.clone(),
        email_sender: email_sender.clone(),
        kafka_credentials_registry: kafka_credentials_registry.clone(),
        retention_registry: retention_registry.clone(),
        polling_webhook_registry: polling_webhook_registry.clone(),
        chat_alert_registry: chat_alert_registry.clone(),
        retraining_registry: retraining_registry.clone(),
        fix_order_registry: fix_order_registry.clone(),
        dlq_registry: dlq_registry.clone(),
        sandbox_registry: Arc::new(fintext_api_server::SandboxRegistry::new()),
        provenance_registry: provenance_registry.clone(),
        anomaly_broadcaster: Arc::new(
            fintext_api_server::anomaly_worker::AnomalyBroadcaster::default(),
        ),
        enable_fix_bridge: fintext_api_server::state::AppState::default().enable_fix_bridge,
        production_mode: fintext_api_server::state::is_production_mode(),
        public_api_version: fintext_api_server::state::read_public_api_version_from_config(),
        enable_full_api_surface: fintext_api_server::state::read_enable_full_api_surface(),
        scd2_registry: Arc::new(fintext_api_server::scd2::Scd2RevisionRegistry::new()),
        pit_cert_archiver: Arc::new(fintext_api_server::PitCertArchiver::from_env_or_config()),
        pit_db_store,
        timescaledb_primary,
        timescaledb_client,
        db_circuit_breaker,
        cache_config: cache_config.clone(),
        provider_health_store: provider_health_store.clone(),
        timescale_fallback_count: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        questdb_health_up: Arc::new(std::sync::atomic::AtomicBool::new(
            fintext_api_server::state::is_questdb_enabled(),
        )),
    };

    // ── Background Cache Governance Cleanup Task (Suite #272) ────────────────
    let bg_quota_cache = state.monthly_quota_cache.clone();
    let bg_rate_limiter = state.rate_limiter.clone();
    let bg_ph_store = state.provider_health_store.clone();
    let bg_scd2 = state.scd2_registry.clone();
    let bg_interval_secs = cache_config.cleanup_interval_secs.max(1);
    let _cache_cleanup_handle = tokio::spawn(async move {
        let mut ticker = tokio::time::interval(std::time::Duration::from_secs(bg_interval_secs));
        loop {
            ticker.tick().await;
            let p1 = bg_quota_cache.remove_expired();
            let p2 = bg_rate_limiter.remove_expired();
            let p3 = bg_ph_store.remove_expired();
            let p4 = bg_scd2.remove_expired();
            let total = p1 + p2 + p3 + p4;
            if total > 0 {
                tracing::debug!(
                    "[Cache Governance] Background purge removed {} expired entries (quota={}, rate_limit={}, provider_health={}, scd2={})",
                    total, p1, p2, p3, p4
                );
            }
        }
    });

    // 5. Spawn background Webhook Dispatcher, API Key Revocation Worker, Email Digest Worker & Kafka Cleanup Worker
    let _webhook_dispatcher_handle =
        fintext_api_server::webhooks::spawn_webhook_dispatcher(state.clone());
    let _api_key_revocation_handle =
        fintext_api_server::users::spawn_api_key_revocation_worker(state.clone());
    let digest_interval = env::var("DIGEST_WORKER_INTERVAL_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(fintext_api_server::digest::DEFAULT_DIGEST_WORKER_INTERVAL_SECS);
    let _digest_worker_handle = fintext_api_server::spawn_digest_worker(
        digest_registry,
        email_sender,
        news_article_registry,
        db_pool.clone(),
        digest_interval,
    );
    let _kafka_cleanup_handle = fintext_api_server::spawn_kafka_credential_cleanup_worker(
        kafka_credentials_registry,
        db_pool,
        600,
    );
    let retention_interval = env::var("RETENTION_CHECK_INTERVAL_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(fintext_api_server::retention::DEFAULT_RETENTION_CHECK_INTERVAL_SECS);
    let _retention_worker_handle =
        fintext_api_server::spawn_retention_worker(state.clone(), retention_interval);
    let polling_interval = env::var("POLLING_WEBHOOK_CHECK_INTERVAL_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(fintext_api_server::polling_webhooks::DEFAULT_POLLING_CHECK_INTERVAL_SECS);
    let _polling_scheduler_handle =
        fintext_api_server::spawn_polling_webhook_scheduler(state.clone(), polling_interval);
    let chat_alert_interval = env::var("CHAT_ALERT_INTERVAL_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(fintext_api_server::chat_alerts::DEFAULT_CHAT_ALERT_INTERVAL_SECS);
    let _chat_alert_dispatcher_handle =
        fintext_api_server::spawn_chat_alert_dispatcher(state.clone(), chat_alert_interval);
    let retraining_interval = env::var("RETRAINING_WORKER_INTERVAL_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(fintext_api_server::retraining::DEFAULT_RETRAINING_INTERVAL_SECS);
    let _retraining_worker_handle =
        fintext_api_server::retraining::spawn_retraining_worker(state.clone(), retraining_interval);
    let anomaly_scan_interval = env::var("ANOMALY_SCAN_INTERVAL_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(fintext_api_server::anomaly_worker::DEFAULT_ANOMALY_SCAN_INTERVAL_SECS);
    let _anomaly_worker_handle = fintext_api_server::anomaly_worker::spawn_anomaly_detection_worker(
        state.clone(),
        fintext_api_server::anomaly_worker::AnomalyWorkerConfig {
            scan_interval_secs: anomaly_scan_interval,
            ..Default::default()
        },
    );

    let app = create_app_with_state(state);

    // 4. Resolve binding address
    let port = env::var("PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(8000);
    let host = env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());

    let addr: SocketAddr = format!("{}:{}", host, port)
        .parse()
        .unwrap_or_else(|_| SocketAddr::from(([0, 0, 0, 0], port)));

    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!(
        "Server listening on http://{} (REST) and ws://{}/ws (WebSocket)",
        addr, addr
    );

    // 5. Serve with graceful shutdown on SIGINT/Ctrl+C
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    info!("Server shutdown complete.");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C signal handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("Received Ctrl+C shutdown signal. Draining in-flight connections...");
        },
        _ = terminate => {
            info!("Received SIGTERM shutdown signal. Draining in-flight connections...");
        },
    }
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
