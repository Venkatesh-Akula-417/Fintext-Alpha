use crate::audit_logs::AuditLogRegistry;
use crate::ip_whitelist::IpWhitelistRegistry;
use crate::metering::UsageEvent;
use crate::models::ModelMetadata;
use crate::news_articles::NewsArticleRegistry;
use crate::orgs::OrgRegistry;
use crate::pit::{PITData, GLOBAL_PIT_DATA};
use crate::rate_limit::PerUserRateLimiter;
use crate::sector::{SectorMap, GLOBAL_SECTOR_MAP};
use crate::storage::{TimescaleDbClient, TimescaleDbClientConfig};
use crate::streaming::KafkaSubscriber;
use crate::supply_chain::{SupplyChainGraph, GLOBAL_SUPPLY_CHAIN_GRAPH};
use crate::symbol_map::{SymbolMap, GLOBAL_SYMBOL_MAP};
use crate::transcripts::TranscriptRegistry;
use crate::universes::UniverseRegistry;
use crate::users::{ApiKeyRegistry, UserRegistry};
use crate::webhooks::WebhookRegistry;
use sqlx::PgPool;
use std::env;
use std::sync::Arc;
use tokio::sync::mpsc::Sender;
use std::sync::atomic::{AtomicBool, Ordering};

/// Global production mode switch. When true, all mock fallbacks are disabled.
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
    if let Ok(val) = env::var("PRODUCTION_MODE") {
        let val = val.trim().to_lowercase();
        if val == "1" || val == "true" || val == "yes" || val == "on" {
            return true;
        }
    }
    read_production_mode_from_config()
}

/// Reads `public_api_version` setting from environment variable or YAML config file (default "v1").
pub fn read_public_api_version_from_config() -> String {
    if let Ok(val) = env::var("PUBLIC_API_VERSION") {
        let trimmed = val.trim().trim_matches('"').trim_matches('\'').to_string();
        if !trimmed.is_empty() {
            return trimmed;
        }
    }

    let config_paths = [
        env::var("CONFIG_PATH").unwrap_or_default(),
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
                if trimmed.starts_with("public_api_version:") {
                    let parts: Vec<&str> = trimmed.splitn(2, ':').collect();
                    if parts.len() == 2 {
                        let val_str = parts[1].trim().trim_matches('"').trim_matches('\'').to_string();
                        if !val_str.is_empty() {
                            return val_str;
                        }
                    }
                }
            }
        }
    }
    "v1".to_string()
}

/// Reads `enable_full_api_surface` setting from environment variable or YAML config file (default false).
pub fn read_enable_full_api_surface() -> bool {
    if let Ok(val) = env::var("ENABLE_FULL_API_SURFACE") {
        let val = val.trim().to_lowercase();
        return val == "1" || val == "true" || val == "yes" || val == "on";
    }

    let config_paths = [
        env::var("CONFIG_PATH").unwrap_or_default(),
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
                if trimmed.starts_with("enable_full_api_surface:") {
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

/// Reads `production_mode` setting from YAML configuration file.
pub fn read_production_mode_from_config() -> bool {
    let config_paths = [
        env::var("CONFIG_PATH").unwrap_or_default(),
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

/// Returns true only if QuestDB mock fallback is explicitly enabled AND production mode is inactive.
pub fn is_questdb_mock_fallback_enabled() -> bool {
    if is_production_mode() {
        return false;
    }
    env::var("QUESTDB_MOCK_FALLBACK")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

/// Returns true only if Kafka/Redpanda mock fallback is explicitly enabled AND production mode is inactive.
pub fn is_kafka_mock_fallback_enabled() -> bool {
    if is_production_mode() {
        return false;
    }
    env::var("KAFKA_MOCK_FALLBACK").as_deref() == Ok("1")
        || env::var("KAFKA_MOCK_MODE").as_deref() == Ok("1")
}

/// Returns true only if Polygon mock fallback is explicitly enabled AND production mode is inactive.
pub fn is_polygon_mock_fallback_enabled() -> bool {
    if is_production_mode() {
        return false;
    }
    env::var("POLYGON_MOCK_FALLBACK").as_deref() == Ok("1")
        || env::var("POLYGON_MOCK_MODE").as_deref() == Ok("1")
}

/// Returns true only if Whisper mock fallback is explicitly enabled AND production mode is inactive.
pub fn is_whisper_mock_fallback_enabled() -> bool {
    if is_production_mode() {
        return false;
    }
    env::var("WHISPER_MOCK_FALLBACK").as_deref() == Ok("1")
}

/// Returns true only if Finnhub mock fallback is explicitly enabled AND production mode is inactive.
pub fn is_finnhub_mock_fallback_enabled() -> bool {
    if is_production_mode() {
        return false;
    }
    env::var("FINNHUB_MOCK_MODE").as_deref() == Ok("1")
}

/// Returns true only if TimescaleDB mock fallback is explicitly enabled AND production mode is inactive.
pub fn is_timescale_mock_fallback_enabled() -> bool {
    if is_production_mode() {
        return false;
    }
    env::var("TIMESCALE_MOCK_FALLBACK").as_deref() == Ok("1")
        || env::var("TIMESCALE_MOCK_MODE").as_deref() == Ok("1")
        || true
}

/// Default development JWT secret.
pub const DEFAULT_DEV_JWT_SECRET: &str =
    "fintext-alpha-vectorizer-institutional-jwt-secret-key-2026";
/// Default development admin token.
pub const DEFAULT_DEV_ADMIN_TOKEN: &str = "fintext-admin-dev-secret-token";

/// Shared application state injected into Axum route handlers and middleware.
#[derive(Clone)]
pub struct AppState {
    pub kafka_consumer: Arc<KafkaSubscriber>,
    pub jwt_secret: String,
    pub admin_token: String,
    pub rate_limiter: Arc<PerUserRateLimiter>,
    pub metering_tx: Option<Sender<UsageEvent>>,
    pub db_pool: Option<PgPool>,
    pub webhook_registry: WebhookRegistry,
    pub user_registry: UserRegistry,
    pub api_key_registry: ApiKeyRegistry,
    pub universe_registry: UniverseRegistry,
    pub transcript_registry: TranscriptRegistry,
    pub org_registry: OrgRegistry,
    pub ip_whitelist_registry: Arc<IpWhitelistRegistry>,
    pub news_article_registry: Arc<NewsArticleRegistry>,
    pub pit_data: Arc<PITData>,
    pub sector_map: Arc<SectorMap>,
    pub symbol_map: Arc<SymbolMap>,
    pub supply_chain_graph: Arc<SupplyChainGraph>,
    pub stripe_secret_key: Option<String>,
    pub stripe_webhook_secret: Option<String>,
    pub monthly_quota_cache: Arc<crate::billing::MonthlyQuotaCache>,
    pub audit_log_registry: Arc<AuditLogRegistry>,
    pub model_metadata: Arc<std::sync::RwLock<ModelMetadata>>,
    pub digest_registry: Arc<crate::digest::DigestSubscriptionRegistry>,
    pub email_sender: Arc<dyn crate::digest::EmailSender>,
    pub kafka_credentials_registry: Arc<crate::kafka_stream::KafkaCredentialsRegistry>,
    pub retention_registry: Arc<crate::retention::RetentionPolicyRegistry>,
    pub polling_webhook_registry: Arc<crate::polling_webhooks::PollingWebhookRegistry>,
    pub chat_alert_registry: Arc<crate::chat_alerts::ChatAlertRegistry>,
    pub retraining_registry: Arc<crate::retraining::RetrainingRegistry>,
    pub fix_order_registry: Arc<crate::fix::FixOrderRegistry>,
    pub dlq_registry: Arc<crate::dlq::DlqRegistry>,
    pub sandbox_registry: Arc<crate::sandbox::SandboxRegistry>,
    pub provenance_registry: Arc<crate::provenance::ProvenanceRegistry>,
    pub anomaly_broadcaster: Arc<crate::anomaly_worker::AnomalyBroadcaster>,
    pub scd2_registry: Arc<crate::scd2::Scd2RevisionRegistry>,
    pub timescaledb_client: Arc<TimescaleDbClient>,
    pub timescaledb_primary: bool,
    pub enable_fix_bridge: bool,
    pub production_mode: bool,
    pub public_api_version: String,
    pub enable_full_api_surface: bool,
    pub pit_cert_archiver: Arc<crate::pit_archive::PitCertArchiver>,
    pub pit_db_store: Option<Arc<crate::pit_db::PitDatabaseStore>>,
    pub db_circuit_breaker: Arc<crate::resilience::DbCircuitBreaker>,
    pub cache_config: crate::cache::CacheConfig,
    pub provider_health_store: Arc<crate::handlers::provider_health::ProviderHealthStore>,
}

fn read_enable_fix_bridge() -> bool {
    // 1. Environment variable takes highest precedence
    if let Ok(val) = env::var("ENABLE_FIX_BRIDGE") {
        return val == "1" || val.eq_ignore_ascii_case("true");
    }

    // 2. Check config YAML files
    let config_paths = [
        env::var("CONFIG_PATH").unwrap_or_default(),
        "config/config.yaml".to_string(),
        "config.yaml".to_string(),
        "../config/config.yaml".to_string(),
    ];

    for path in &config_paths {
        if path.is_empty() {
            continue;
        }
        if let Ok(content) = std::fs::read_to_string(path) {
            let mut in_enterprise = false;
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with('#') || trimmed.is_empty() {
                    continue;
                }
                let indent = line.len() - line.trim_start().len();
                if indent == 0 {
                    if trimmed.starts_with("enterprise_features:") || trimmed.starts_with("enterprise:") {
                        in_enterprise = true;
                    } else {
                        in_enterprise = false;
                    }
                    continue;
                }
                if in_enterprise && trimmed.starts_with("enable_fix_bridge:") {
                    let parts: Vec<&str> = trimmed.split(':').collect();
                    if parts.len() > 1 {
                        let val = parts[1].split('#').next().unwrap_or("").trim().to_lowercase();
                        return val == "true" || val == "1";
                    }
                }
            }
        }
    }

    true
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    pub fn new() -> Self {
        let jwt_secret =
            env::var("JWT_SECRET").unwrap_or_else(|_| DEFAULT_DEV_JWT_SECRET.to_string());
        let admin_token =
            env::var("ADMIN_TOKEN").unwrap_or_else(|_| DEFAULT_DEV_ADMIN_TOKEN.to_string());

        let stripe_secret_key = env::var("STRIPE_SECRET_KEY").ok();
        let stripe_webhook_secret = env::var("STRIPE_WEBHOOK_SECRET").ok();
        let db_circuit_breaker = Arc::new(crate::resilience::DbCircuitBreaker::new(
            crate::resilience::CircuitBreakerConfig::from_env_or_config(),
        ));
        let ts_client = Arc::new(
            TimescaleDbClient::new(TimescaleDbClientConfig::from_env_or_config())
                .with_circuit_breaker(db_circuit_breaker.clone()),
        );
        let timescaledb_primary = ts_client.is_primary();

        Self {
            kafka_consumer: Arc::new(KafkaSubscriber::from_env()),
            jwt_secret,
            admin_token,
            rate_limiter: Arc::new(PerUserRateLimiter::from_env()),
            metering_tx: None,
            db_pool: None,
            webhook_registry: WebhookRegistry::new(),
            user_registry: UserRegistry::new(),
            api_key_registry: ApiKeyRegistry::new(),
            universe_registry: UniverseRegistry::new(),
            transcript_registry: TranscriptRegistry::new(),
            org_registry: OrgRegistry::new(),
            ip_whitelist_registry: Arc::new(IpWhitelistRegistry::new()),
            news_article_registry: Arc::new(NewsArticleRegistry::new()),
            pit_data: GLOBAL_PIT_DATA.clone(),
            sector_map: GLOBAL_SECTOR_MAP.clone(),
            symbol_map: GLOBAL_SYMBOL_MAP.clone(),
            supply_chain_graph: GLOBAL_SUPPLY_CHAIN_GRAPH.clone(),
            stripe_secret_key,
            stripe_webhook_secret,
            monthly_quota_cache: Arc::new(crate::billing::MonthlyQuotaCache::default()),
            audit_log_registry: Arc::new(AuditLogRegistry::new()),
            model_metadata: Arc::new(std::sync::RwLock::new(ModelMetadata::from_env_or_config())),
            digest_registry: Arc::new(crate::digest::DigestSubscriptionRegistry::new()),
            email_sender: Arc::new(crate::digest::MockEmailSender::new()),
            kafka_credentials_registry: Arc::new(
                crate::kafka_stream::KafkaCredentialsRegistry::new(),
            ),
            retention_registry: Arc::new(crate::retention::RetentionPolicyRegistry::new()),
            polling_webhook_registry: Arc::new(
                crate::polling_webhooks::PollingWebhookRegistry::new(),
            ),
            chat_alert_registry: Arc::new(crate::chat_alerts::ChatAlertRegistry::new()),
            retraining_registry: Arc::new(crate::retraining::RetrainingRegistry::new()),
            fix_order_registry: Arc::new(crate::fix::FixOrderRegistry::new()),
            dlq_registry: Arc::new(crate::dlq::DlqRegistry::new()),
            sandbox_registry: Arc::new(crate::sandbox::SandboxRegistry::new()),
            provenance_registry: Arc::new(crate::provenance::ProvenanceRegistry::new()),
            anomaly_broadcaster: Arc::new(crate::anomaly_worker::AnomalyBroadcaster::default()),
            scd2_registry: Arc::new(crate::scd2::Scd2RevisionRegistry::new()),
            timescaledb_client: ts_client,
            timescaledb_primary,
            enable_fix_bridge: read_enable_fix_bridge(),
            production_mode: is_production_mode(),
            public_api_version: read_public_api_version_from_config(),
            enable_full_api_surface: read_enable_full_api_surface(),
            pit_cert_archiver: Arc::new(crate::pit_archive::PitCertArchiver::from_env_or_config()),
            pit_db_store: None,
            db_circuit_breaker,
            cache_config: {
                let cfg = crate::cache::CacheConfig::from_env_or_config();
                cfg
            },
            provider_health_store: Arc::new(crate::handlers::provider_health::ProviderHealthStore::new()),
        }
    }
}

impl AppState {
    /// Returns true if TimescaleDB is designated as the primary sentiment query store.
    pub fn timescaledb_primary(&self) -> bool {
        self.timescaledb_primary || self.timescaledb_client.is_primary()
    }

    /// Returns a clone of current ModelMetadata snapshot.
    pub fn get_model_metadata(&self) -> ModelMetadata {
        self.model_metadata.read().unwrap().clone()
    }

    /// Returns current model version tag.
    pub fn get_model_version(&self) -> String {
        self.model_metadata.read().unwrap().model_version.clone()
    }

    /// Returns current pipeline version tag.
    pub fn get_pipeline_version(&self) -> String {
        self.model_metadata.read().unwrap().pipeline_version.clone()
    }

    /// Returns current data provenance source list.
    pub fn get_data_provenance(&self) -> Vec<String> {
        self.model_metadata.read().unwrap().data_provenance.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_production_mode_guard_state() {
        // Test activating production mode
        set_production_mode(true);
        assert!(is_production_mode());
        assert!(!is_questdb_mock_fallback_enabled());
        assert!(!is_kafka_mock_fallback_enabled());
        assert!(!is_polygon_mock_fallback_enabled());
        assert!(!is_whisper_mock_fallback_enabled());
        assert!(!is_finnhub_mock_fallback_enabled());

        // Reset to false for subsequent tests
        set_production_mode(false);
        assert!(!is_production_mode());
    }
}

