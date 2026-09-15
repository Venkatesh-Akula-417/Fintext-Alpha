//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Axum High-Performance HTTP & WebSocket API Gateway
//! ═══════════════════════════════════════════════════════════════════════════════

pub mod auth;
pub mod billing;
pub mod chat_alerts;
pub mod handlers;
pub mod metering;
pub mod models;
pub mod openapi;
pub mod pit;
pub mod polling_webhooks;
pub mod quality;
pub mod rate_limit;
pub mod sector;
pub mod state;
pub mod storage;
pub mod streaming;
pub mod supply_chain;
pub mod symbol_map;
pub mod universes;
pub mod users;
pub mod webhooks;

pub mod audio;
pub mod audit_logs;
pub mod digest;
pub mod fix;
pub mod ip_whitelist;
pub mod kafka_stream;
pub mod language;
pub mod news_articles;
pub mod orgs;
pub mod retention;
pub mod retraining;
pub mod transcripts;
pub use fix::{compute_mock_price, parse_fix_message, FixOrder, FixOrderRegistry};
pub mod dlq;
pub use dlq::{DlqEvent, DlqRegistry};
pub mod sandbox;
pub use sandbox::{SandboxContext, SandboxInfo, SandboxRegistry};
pub mod provenance;
pub use provenance::{init_data_provenance_table, ProvenanceRegistry};
pub mod anomaly_worker;
pub use anomaly_worker::{
    scan_and_broadcast_anomalies, spawn_anomaly_detection_worker, AnomalyBroadcaster,
    AnomalyWorkerConfig, DEFAULT_ANOMALY_SCAN_INTERVAL_SECS,
};
pub mod scd2;
pub use scd2::*;
pub mod secrets;
pub use secrets::*;
pub mod pit_archive;
pub use pit_archive::*;
pub mod pit_db;
pub use pit_db::*;
pub mod resilience;
pub use resilience::*;
pub mod cache;
pub use cache::*;

pub use audit_logs::{
    init_audit_logs_table, log_audit_event, AuditExportQuery, AuditLogEntry,
    AuditLogExportResponse, AuditLogRegistry, AuditLogsQuery, AuditLogsResponse,
};
pub use auth::{
    auth_middleware, generate_jwt, generate_jwt_with_org, generate_jwt_with_sandbox,
    is_sandbox_request, issue_token_handler, validate_jwt, AuthErrorResponse, Claims,
    IssueTokenRequest, IssueTokenResponse, DEFAULT_JWT_EXPIRY_SECS,
};
use axum::middleware::from_fn_with_state;
use axum::routing::{any, delete, get, patch, post};
use axum::Router;
pub use billing::{
    create_checkout_handler, create_portal_handler, get_subscription_handler, init_billing_db,
    stripe_webhook_handler, BillingWebhookResponse, CheckoutRequest, CheckoutResponse,
    MonthlyQuotaCache, PortalRequest, PortalResponse, SubscriptionResponse,
};
pub use chat_alerts::{
    create_chat_alert_handler, delete_chat_alert_handler, dispatch_chat_alert,
    list_chat_alerts_handler, spawn_chat_alert_dispatcher, ChatAlertRegistry,
};
pub use digest::{
    compose_digest_email, spawn_digest_worker, validate_digest_request, DigestSubscriptionRegistry,
    EmailSender, MockEmailSender, SentEmailRecord,
};
pub use handlers::health::readyz_handler;
pub use handlers::{
    activate_sandbox_handler, backfill_sentiment_handler, backtest_handler,
    cancel_fix_order_handler, cancel_retraining_job_handler, create_digest_subscription_handler,
    create_retention_policy_handler, create_retraining_job_handler, create_transcript_handler,
    deactivate_sandbox_handler, delete_digest_subscription_handler,
    delete_retention_policy_handler, delete_transcript_handler, export_audit_logs_handler,
    export_csv_handler, export_parquet_handler, get_8k_events_handler, get_audit_logs_handler,
    get_bankruptcy_risk_handler, get_batch_sentiment_handler, get_commodity_sentiment_handler,
    get_credit_sentiment_handler, get_crypto_sentiment_handler, get_digest_subscription_handler,
    get_dlq_event_handler, get_earnings_surprise_handler, get_esg_scores_handler,
    get_event_study_handler, get_factor_exposure_handler, get_fx_sentiment_handler,
    get_insider_trading_handler, get_kafka_credentials_handler, get_language_detect_handler,
    get_ma_rumors_handler, get_market_breadth_handler, get_market_regime_handler,
    get_model_card_handler, get_model_validation_handler, get_news_article_handler,
    get_options_iv_handler, get_options_microstructure_handler, get_options_vol_surface_handler,
    get_pit_certificate_handler, get_pit_replay_handler, get_provenance_handler,
    get_provider_health_handler, get_put_call_ratio_handler, get_regulatory_filings_handler,
    get_retraining_job_handler, get_return_correlation_handler, get_sandbox_status_handler,
    get_sector_sentiment_handler, get_sentiment_anomalies_handler,
    get_sentiment_disagreement_handler, get_sentiment_entities_handler, get_sentiment_feed_handler,
    get_sentiment_handler, get_sentiment_history_handler, get_sentiment_revisions_handler,
    get_sla_status_handler, get_spillover_matrix_handler, get_spillovers_handler,
    get_supply_chain_risk_handler, get_symbol_map_handler, get_transcript_handler,
    get_unusual_options_handler, get_usage_stats_handler, health_check_handler,
    list_dlq_events_handler, list_fix_orders_handler, list_kafka_topics_handler,
    list_news_articles_handler, list_retention_policies_handler, list_retraining_jobs_handler,
    list_transcripts_handler, portfolio_factor_exposure_handler, portfolio_optimize_handler,
    post_alpha_report_handler, post_anomaly_scan_handler, post_sentiment_revision_handler,
    post_signal_quality_report_handler, purge_dlq_event_handler, reload_pit_data_handler,
    reprocess_dlq_event_handler, revoke_kafka_credentials_handler, search::get_search_handler,
    sector_rotation::get_sector_rotation_handler, sla_latency_handler, submit_fix_order_handler,
    transcribe_audio_handler, trigger_digest_send_handler, websocket_handler,
};
pub use ip_whitelist::{
    add_ip_whitelist_handler, delete_ip_whitelist_handler, get_ip_whitelist_handler,
    init_ip_whitelist_db, ip_whitelist_middleware, AddIpWhitelistRequest,
    DeleteIpWhitelistResponse, IpWhitelistEntry, IpWhitelistErrorResponse, IpWhitelistRegistry,
    ListIpWhitelistResponse,
};
pub use kafka_stream::{
    get_available_kafka_topics, is_valid_kafka_topic, spawn_kafka_credential_cleanup_worker,
    KafkaCredentialsRegistry,
};
pub use metering::{
    create_metering_channel, init_db, metering_middleware, spawn_metering_worker,
    MeteringWorkerConfig, UsageEvent, DEFAULT_METERING_BATCH_SIZE,
    DEFAULT_METERING_CHANNEL_CAPACITY, DEFAULT_METERING_FLUSH_INTERVAL_MS,
};
pub use models::{
    AbnormalReturnPoint, AcousticFeatures, AnomalyScanResponse, AudioSentiment,
    AudioTranscriptionResponse, BackfillSentimentRequest, BackfillSentimentResponse,
    BacktestRequest, BacktestResponse, BankruptcyComponents, BankruptcyRiskParams,
    BankruptcyRiskResponse, BatchSentimentParams, BatchSentimentResponse, CalibrationPoint,
    ChatAlertSubscription, ChatAlertSubscriptionResponse, ChatAlertsResponse, ClassConfusion,
    ClassificationMetrics, CommoditySentimentArticle, CommoditySentimentParams,
    CommoditySentimentResponse, CommoditySentimentSummary, ConfusionMatrix, CreateChatAlertRequest,
    CreateRetentionPolicyRequest, CreateRetrainingJobRequest, CreateTranscriptRequest,
    CreditSentimentParams, CreditSentimentResponse, CryptoSentimentArticle, CryptoSentimentParams,
    CryptoSentimentResponse, CryptoSentimentSummary, DLQEventDetail, DLQEventItem,
    DLQEventsListResponse, DLQEventsQueryParams, DataProvenanceItem, DataProvenanceResponse,
    DeleteChatAlertResponse, DeleteRetentionPolicyResponse, DeleteTranscriptResponse,
    ESGDimensionScore, ESGDimensions, ESGScoresParams, ESGScoresResponse, EarningsSurpriseItem,
    EarningsSurpriseParams, EarningsSurpriseResponse, EightKFiling, EightKParams, EightKResponse,
    EquityPoint, EventStudyParams, EventStudyResponse, ExportParquetParams, FIXCancelRequest,
    FIXOrderRequest, FIXOrderResponse, FIXOrdersListResponse, FIXOrdersQueryParams,
    FXSentimentArticle, FXSentimentParams, FXSentimentResponse, FXSentimentSummary,
    FactorExposureItem, FactorExposureParams, FactorExposureResponse, FixOrderItem, HealthResponse,
    InsiderTradeItem, InsiderTradingParams, InsiderTradingResponse, LanguageDetectionQuery,
    LanguageDetectionResponse, ListRetrainingJobsQuery, ListRetrainingJobsResponse, MARumorItem,
    MARumorsParams, MARumorsResponse, MarketBreadthParams, MarketBreadthPoint,
    MarketBreadthResponse, MarketRegimeParams, MarketRegimeResponse, MicrostructureParams,
    MicrostructurePoint, MicrostructureResponse, ModelValidationQuery, ModelValidationResponse,
    NewsArticleFull, NewsArticleMetadata, NewsArticlesListResponse, OLSStatistics, OptionContract,
    OptionsIvParams, OptionsIvResponse, OptionsVolSurfaceParams, OptionsVolSurfaceResponse,
    PITBackfillTestResult, PITCertificateParams, PITCertificatePolicies, PITCertificateResponse,
    PITCertificateTests, PITDuplicateTestResult, PITReplayConsistency, PITReplayEventItem,
    PITReplayFilingItem, PITReplayNewsItem, PITReplayParams, PITReplayResponse,
    PITReplaySentimentItem, PITReplaySummary, PITTestResult, PerClassMetrics, PortfolioConstraints,
    PortfolioFactorExposureRequest, PortfolioFactorExposureResponse, PortfolioOptimizeRequest,
    PortfolioOptimizeResponse, PortfolioWeight, ProcessingStep, ProviderHealthItem,
    ProviderHealthQuery, ProviderHealthResponse, PurgeDLQResponse, PutCallRatioParams,
    PutCallRatioPoint, PutCallRatioResponse, RegimeComponents, RegulatoryFilingItem,
    RegulatoryFilingsParams, RegulatoryFilingsResponse, ReprocessDLQResponse,
    RetentionPoliciesResponse, RetentionPolicy, RetrainingJob, RetrainingJobResponse,
    ReturnCorrelationItem, ReturnCorrelationParams, ReturnCorrelationResponse, SLAStatusParams,
    SLAStatusResponse, SandboxStatusResponse, SearchParams, SearchResponse, SearchResultItem,
    SectorRotationItem, SectorRotationParams, SectorRotationResponse, SectorSentimentParams,
    SectorSentimentResponse, SentimentAnomaliesParams, SentimentAnomaliesResponse,
    SentimentAnomalyAlert, SentimentAnomalyItem, SentimentDisagreementParams,
    SentimentDisagreementResponse, SentimentFeedItem, SentimentFeedParams, SentimentFeedResponse,
    SentimentHistoryParams, SentimentHistoryResponse, SentimentProbabilities, SentimentQuery,
    SentimentRecord, SentimentResponse, SourceBreakdown, SpilloverItem, SpilloverMatrixItem,
    SpilloverMatrixParams, SpilloverMatrixResponse, SpilloverQuery, SpilloverResponse,
    SupplyChainRiskItem, SupplyChainRiskParams, SupplyChainRiskResponse, SymbolMapParams,
    SymbolMapResponse, TranscriptListParams, TranscriptListResponse, TranscriptMetadata,
    TranscriptResponse, UnusualOptionItem, UnusualOptionsParams, UnusualOptionsResponse,
    UsageGroupItem, UsageStatsParams, UsageStatsResponse, UsageStatsSummary, VolSurfacePoint,
    DEFAULT_SANDBOX_MOCK_VERSION,
};
pub use news_articles::NewsArticleRegistry;
pub use openapi::{ApiDoc, PublicApiDoc};
pub use orgs::{
    create_org_handler, get_org_handler, init_orgs_db, invite_member_handler, leave_org_handler,
    list_orgs_handler, remove_member_handler, select_org_handler, update_member_role_handler,
    CreateOrgRequest, CreateOrgResponse, InviteMemberRequest, InviteMemberResponse,
    LeaveOrgResponse, ListOrgsResponse, OrgDetailsResponse, OrgRegistry, OrgRole, Organization,
    OrganizationMember, RemoveMemberResponse, SelectOrgResponse, UpdateMemberRoleRequest,
    UpdateMemberRoleResponse,
};
pub use pit::{PITData, GLOBAL_PIT_DATA};
pub use polling_webhooks::{
    create_polling_webhook_handler, delete_polling_webhook_handler, list_polling_webhooks_handler,
    spawn_polling_webhook_scheduler, CreatePollingWebhookRequest, DeletePollingWebhookResponse,
    PollingWebhook, PollingWebhookRegistry, PollingWebhookResponse, PollingWebhooksResponse,
};
pub use rate_limit::{
    rate_limit_middleware, PerUserRateLimiter, RateLimitConfig, RateLimitErrorResponse,
    RateLimitStatus, DEFAULT_RATE_LIMIT_REQUESTS, DEFAULT_RATE_LIMIT_WINDOW_SECS,
    HEADER_RATELIMIT_LIMIT, HEADER_RATELIMIT_REMAINING, HEADER_RATELIMIT_RESET, HEADER_RETRY_AFTER,
};
pub use retention::{
    get_valid_data_categories, is_valid_data_category, spawn_retention_worker,
    RetentionPolicyRegistry,
};
pub use retraining::{
    init_db as init_retraining_db, is_valid_model_type, is_valid_trigger_type,
    process_retraining_job, spawn_retraining_worker, trigger_immediate_job_processing,
    RetrainingRegistry, DEFAULT_RETRAINING_INTERVAL_SECS, MAX_CONCURRENT_RETRAINING_JOBS,
    VALID_MODEL_TYPES, VALID_TRIGGER_TYPES,
};
pub use sector::{SectorMap, GLOBAL_SECTOR_MAP};
pub use state::{
    enable_full_api_surface, is_finnhub_mock_fallback_enabled, is_kafka_mock_fallback_enabled,
    is_polygon_mock_fallback_enabled, is_production_mode, is_questdb_mock_fallback_enabled,
    is_whisper_mock_fallback_enabled, read_production_mode_from_config, set_production_mode,
    AppState, DEFAULT_DEV_ADMIN_TOKEN, DEFAULT_DEV_JWT_SECRET, PRODUCTION_MODE_ACTIVE,
};
use std::time::Duration;
pub use streaming::{KafkaSubscriber, KafkaSubscriberConfig};
pub use supply_chain::{SupplyChainEdge, SupplyChainGraph, GLOBAL_SUPPLY_CHAIN_GRAPH};
pub use symbol_map::{SecurityIdentifiers, SymbolMap, GLOBAL_SYMBOL_MAP};
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;
pub use transcripts::{StoredTranscript, TranscriptRegistry};
pub use universes::{
    create_universe_handler, delete_universe_handler, get_universe_handler, init_universes_db,
    list_universes_handler, update_universe_handler, CreateUniverseRequest, DeleteUniverseResponse,
    ListUniversesParams, ListUniversesResponse, Universe, UniverseRegistry, UpdateUniverseRequest,
};
pub use users::{
    create_api_key_handler, delete_api_key_handler, get_api_key_handler, get_me_handler,
    hash_api_key, hash_password, init_users_db, list_api_keys_handler, login_user_handler,
    register_user_handler, rotate_api_key_handler, spawn_api_key_revocation_worker, validate_email,
    validate_password_strength, verify_password, ApiKeyItem, ApiKeyRegistry, CreateApiKeyRequest,
    CreateApiKeyResponse, DeleteApiKeyResponse, ListApiKeysResponse, LoginRequest, LoginResponse,
    RegisterRequest, RegisterResponse, RotateApiKeyRequest, RotateApiKeyResponse, StoredApiKey,
    StoredUser, UserProfileResponse, UserRegistry,
};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;
pub use webhooks::{
    delete_webhook_handler, init_webhooks_db, list_webhooks_handler, register_webhook_handler,
    spawn_webhook_dispatcher, CreateWebhookRequest, DeleteWebhookResponse, ListWebhooksResponse,
    WebhookPayload, WebhookRegistry, WebhookResponse, WebhookSubscription,
};

pub mod subtle {
    pub trait ConstantTimeEq {
        fn ct_eq(&self, other: &Self) -> bool;
    }

    impl ConstantTimeEq for [u8] {
        #[inline]
        fn ct_eq(&self, other: &[u8]) -> bool {
            if self.len() != other.len() {
                return false;
            }
            let mut diff = 0u8;
            for (&a, &b) in self.iter().zip(other.iter()) {
                diff |= a ^ b;
            }
            diff == 0
        }
    }
}

pub use subtle::ConstantTimeEq;

/// RFC 8594 410 Gone stub handler for deprecated and permanently removed endpoints.
pub async fn gone_handler() -> impl axum::response::IntoResponse {
    let mut headers = axum::http::HeaderMap::new();
    headers.insert(
        axum::http::header::HeaderName::from_static("sunset"),
        axum::http::HeaderValue::from_static("Wed, 11 Nov 2026 00:00:00 GMT"),
    );
    (
        axum::http::StatusCode::GONE,
        headers,
        axum::Json(serde_json::json!({
            "error": "Gone",
            "message": "This endpoint has been permanently removed in v1.0. Consult the documentation for migration guidance."
        })),
    )
}

/// Administrative token validation middleware for /internal/* operational endpoints.
/// Enforces constant-time verification of the X-Admin-Token header.
pub async fn admin_token_middleware(
    axum::extract::State(state): axum::extract::State<AppState>,
    mut req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    use axum::response::IntoResponse;

    let token_header = req
        .headers()
        .get("X-Admin-Token")
        .or_else(|| req.headers().get("x-admin-token"))
        .and_then(|h| h.to_str().ok());

    let expected_token = std::env::var("ADMIN_TOKEN").unwrap_or_else(|_| state.admin_token.clone());

    let is_valid = match token_header {
        Some(token) => {
            let token_bytes = token.as_bytes();
            let expected_bytes = expected_token.as_bytes();
            token_bytes.ct_eq(expected_bytes)
        }
        None => false,
    };

    if !is_valid {
        return (
            axum::http::StatusCode::UNAUTHORIZED,
            axum::Json(serde_json::json!({
                "error": "Unauthorized",
                "message": "Invalid or missing X-Admin-Token header"
            })),
        )
            .into_response();
    }

    // Process Authorization header if present or required by downstream handlers
    if let Some(auth_header) = req
        .headers()
        .get("authorization")
        .and_then(|h| h.to_str().ok())
    {
        let parts: Vec<&str> = auth_header.split_whitespace().collect();
        if parts.len() == 2 && parts[0].eq_ignore_ascii_case("bearer") {
            match validate_jwt(parts[1], state.jwt_secret.as_bytes()) {
                Ok(claims) => {
                    req.extensions_mut().insert(claims);
                }
                Err(_) => {
                    return (
                        axum::http::StatusCode::UNAUTHORIZED,
                        axum::Json(serde_json::json!({
                            "error": "Unauthorized",
                            "message": "Invalid JWT token"
                        })),
                    )
                        .into_response();
                }
            }
        }
    } else {
        let path = req.uri().path();
        if path.contains("/dlq/")
            || path.ends_with("/dlq/events")
            || path.contains("/retraining/")
            || path.contains("/stream/kafka")
        {
            return (
                axum::http::StatusCode::UNAUTHORIZED,
                axum::Json(serde_json::json!({
                    "error": "Unauthorized",
                    "message": "Missing Authorization Bearer token"
                })),
            )
                .into_response();
        }
    }

    next.run(req).await
}

/// Builds the internal operational router gated by ADMIN_TOKEN middleware under `/internal`.
pub fn internal_router(state: AppState) -> Router<AppState> {
    let internal_routes = Router::new()
        .route("/dlq/events", get(list_dlq_events_handler))
        .route(
            "/dlq/events/:id",
            get(get_dlq_event_handler).delete(purge_dlq_event_handler),
        )
        .route(
            "/dlq/events/:id/reprocess",
            post(reprocess_dlq_event_handler),
        )
        .route("/admin/reload-pit-data", post(reload_pit_data_handler))
        .route(
            "/retraining/jobs",
            get(list_retraining_jobs_handler).post(create_retraining_job_handler),
        )
        .route("/retraining/jobs/:id", get(get_retraining_job_handler))
        .route(
            "/retraining/jobs/:id/cancel",
            post(cancel_retraining_job_handler),
        )
        .route("/sentiment/backfill", post(backfill_sentiment_handler))
        .route("/stream/kafka/topics", get(list_kafka_topics_handler))
        .route(
            "/stream/kafka/credentials",
            get(get_kafka_credentials_handler),
        )
        .route(
            "/stream/kafka/credentials/:id",
            delete(revoke_kafka_credentials_handler),
        )
        .layer(from_fn_with_state(state.clone(), admin_token_middleware))
        .layer(TimeoutLayer::new(Duration::from_secs(10)));

    Router::new().nest("/internal", internal_routes)
}

/// Build the Axum application router with default state.
pub fn create_app() -> Router {
    create_app_with_state(AppState::default())
}

/// Build the Axum application router with explicitly injected `AppState`.
pub fn create_app_with_state(state: AppState) -> Router {
    // 1. Protected REST routes (10-second timeout applied)
    let protected_rest = Router::new()
        .route(
            "/transcripts",
            post(create_transcript_handler).get(list_transcripts_handler),
        )
        .route(
            "/transcripts/:id",
            get(get_transcript_handler).delete(delete_transcript_handler),
        )
        .route("/sentiment", get(get_sentiment_handler))
        .route("/sentiment/anomalies", get(get_sentiment_anomalies_handler))
        .route(
            "/sentiment/disagreement",
            get(get_sentiment_disagreement_handler),
        )
        .route("/sentiment/entities", get(get_sentiment_entities_handler))
        .route("/sentiment/feed", get(get_sentiment_feed_handler))
        .route("/sentiment/history", get(get_sentiment_history_handler))
        .route("/sentiment/sector", get(get_sector_sentiment_handler))
        .route("/sentiment/batch", get(get_batch_sentiment_handler))
        .route("/sentiment/revision", post(post_sentiment_revision_handler))
        .route("/sentiment/revisions", get(get_sentiment_revisions_handler))
        .route("/symbols/map", get(get_symbol_map_handler))
        .route(
            "/universes",
            post(create_universe_handler).get(list_universes_handler),
        )
        .route(
            "/universes/:id",
            get(get_universe_handler)
                .put(update_universe_handler)
                .delete(delete_universe_handler),
        )
        .route("/options/iv", get(get_options_iv_handler))
        .route("/options/unusual", get(get_unusual_options_handler))
        .route("/options/vol-surface", get(get_options_vol_surface_handler))
        .route("/options/put-call-ratio", get(get_put_call_ratio_handler))
        .route(
            "/options/microstructure",
            get(get_options_microstructure_handler),
        )
        .route("/usage/stats", get(get_usage_stats_handler))
        .route("/events/8k", get(get_8k_events_handler))
        .route(
            "/events/earnings-surprise",
            get(get_earnings_surprise_handler),
        )
        .route("/events/insider-trading", get(get_insider_trading_handler))
        .route(
            "/events/supply-chain-risk",
            get(get_supply_chain_risk_handler),
        )
        .route(
            "/signals/quality-report",
            post(post_signal_quality_report_handler),
        )
        .route("/pit/replay", get(get_pit_replay_handler))
        .route("/pit/certificate", get(get_pit_certificate_handler))
        .route("/providers/health", get(get_provider_health_handler))
        .route("/model-validation", get(get_model_validation_handler))
        .route("/export/csv", get(export_csv_handler))
        .route("/export/parquet", get(export_parquet_handler))
        .route("/auth/me", get(get_me_handler))
        .route(
            "/auth/api-keys",
            post(create_api_key_handler).get(list_api_keys_handler),
        )
        .route(
            "/auth/api-keys/:id",
            get(get_api_key_handler).delete(delete_api_key_handler),
        )
        .route("/auth/api-keys/:id/rotate", post(rotate_api_key_handler))
        .route(
            "/webhooks",
            post(register_webhook_handler).get(list_webhooks_handler),
        )
        .route("/webhooks/:id", delete(delete_webhook_handler))
        .route("/billing/checkout", post(create_checkout_handler))
        .route("/billing/portal", post(create_portal_handler))
        .route("/billing/subscription", get(get_subscription_handler))
        .route("/orgs", post(create_org_handler).get(list_orgs_handler))
        .route("/orgs/:id", get(get_org_handler))
        .route("/orgs/:id/invites", post(invite_member_handler))
        .route(
            "/orgs/:id/members/:user_id",
            patch(update_member_role_handler).delete(remove_member_handler),
        )
        .route("/orgs/:id/leave", post(leave_org_handler))
        .route("/orgs/:id/select", post(select_org_handler))
        .route(
            "/security/ip-whitelist",
            get(get_ip_whitelist_handler).post(add_ip_whitelist_handler),
        )
        .route(
            "/security/ip-whitelist/:id",
            delete(delete_ip_whitelist_handler),
        )
        .route("/news/articles", get(list_news_articles_handler))
        .route("/audit/logs", get(get_audit_logs_handler))
        .route("/audit/export", get(export_audit_logs_handler))
        .route("/search", get(get_search_handler))
        .route(
            "/digest/subscription",
            get(get_digest_subscription_handler)
                .post(create_digest_subscription_handler)
                .delete(delete_digest_subscription_handler),
        )
        .route(
            "/retention/policies",
            get(list_retention_policies_handler).post(create_retention_policy_handler),
        )
        .route(
            "/retention/policies/:id",
            delete(delete_retention_policy_handler),
        )
        .route("/sla/status", get(get_sla_status_handler))
        .route("/sla/latency", get(sla_latency_handler))
        .route("/sandbox/activate", post(activate_sandbox_handler))
        .route("/sandbox/deactivate", post(deactivate_sandbox_handler))
        .route("/sandbox/status", get(get_sandbox_status_handler))
        .route(
            "/provenance/:record_type/:record_id",
            get(get_provenance_handler),
        )
        .layer(TimeoutLayer::new(Duration::from_secs(10)));

    // 2. Protected WebSocket route (long-lived connection; no request timeout)
    let protected_ws = Router::new().route("/ws", get(websocket_handler));

    // 3. Protected routes layer with per-user rate limiting, usage metering, and JWT/API-Key authentication middleware
    let protected_routes = Router::new()
        .merge(protected_rest)
        .merge(protected_ws)
        .layer(from_fn_with_state(state.clone(), rate_limit_middleware))
        .layer(from_fn_with_state(state.clone(), metering_middleware))
        .layer(from_fn_with_state(state.clone(), ip_whitelist_middleware))
        .layer(from_fn_with_state(state.clone(), auth_middleware));

    // 4. Public routes (/health probe, /auth/token, /auth/login, Swagger UI / OpenAPI docs, and 410 Gone stubs)
    let swagger_router = SwaggerUi::new("/swagger-ui")
        .url("/v1/api-docs/openapi.json", PublicApiDoc::openapi())
        .url("/api-docs/openapi.json", ApiDoc::openapi());

    let public_routes = Router::new()
        .merge(swagger_router)
        .route(
            "/v1/swagger-ui",
            get(|| async { axum::response::Redirect::temporary("/swagger-ui/") }),
        )
        .route(
            "/v1/swagger-ui/",
            get(|| async { axum::response::Redirect::temporary("/swagger-ui/") }),
        )
        .route("/health", get(health_check_handler))
        .route("/readyz", get(readyz_handler))
        .route("/auth/token", post(issue_token_handler))
        .route("/auth/register", any(gone_handler))
        .route("/news/articles/:id", any(gone_handler))
        .route("/audio/transcribe", any(gone_handler))
        .route("/auth/login", post(login_user_handler))
        .route("/billing/webhook", post(stripe_webhook_handler))
        .route("/model-card", get(get_model_card_handler))
        .layer(TimeoutLayer::new(Duration::from_secs(10)));

    // 5. Versioned Public v1 Router (pruned core endpoints)
    let v1_router = public_v1_router(state.clone());

    let mut app = Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .merge(v1_router);

    if enable_full_api_surface() {
        app = app.merge(internal_router(state.clone()));
    }

    app.with_state(state).layer(TraceLayer::new_for_http())
}

/// Builds the versioned public API Gateway router exposing only the core endpoints under `/v1`.
pub fn public_v1_router(state: AppState) -> Router<AppState> {
    let v1_protected = Router::new()
        // Auth / User core endpoints
        .route("/users/me", get(get_me_handler))
        .route("/auth/me", get(get_me_handler))
        .route(
            "/users/api-keys",
            post(create_api_key_handler).get(list_api_keys_handler),
        )
        .route(
            "/auth/api-keys",
            post(create_api_key_handler).get(list_api_keys_handler),
        )
        .route(
            "/users/api-keys/:id",
            get(get_api_key_handler).delete(delete_api_key_handler),
        )
        .route(
            "/auth/api-keys/:id",
            get(get_api_key_handler).delete(delete_api_key_handler),
        )
        // Sentiment / NLP core endpoints
        .route("/sentiment/feed", get(get_sentiment_feed_handler))
        .route("/sentiment/history", get(get_sentiment_history_handler))
        .route(
            "/sentiment/batch",
            post(get_batch_sentiment_handler).get(get_batch_sentiment_handler),
        )
        .route("/sentiment/confidence", get(get_sentiment_handler))
        .route("/sentiment", get(get_sentiment_handler))
        .route(
            "/sentiment/disagreement",
            get(get_sentiment_disagreement_handler),
        )
        .route("/sentiment/entities", get(get_sentiment_entities_handler))
        .route("/sentiment/sector", get(get_sector_sentiment_handler))
        .route("/news/articles", get(list_news_articles_handler))
        // Events / Filings core endpoints
        .route("/events/8k", get(get_8k_events_handler))
        .route(
            "/events/earnings-surprise",
            get(get_earnings_surprise_handler),
        )
        .route(
            "/transcripts",
            get(list_transcripts_handler).post(create_transcript_handler),
        )
        .route(
            "/transcripts/:id",
            get(get_transcript_handler).delete(delete_transcript_handler),
        )
        // PIT / Governance core endpoints
        .route("/pit/certificate", get(get_pit_certificate_handler))
        .route("/pit/replay", get(get_pit_replay_handler))
        .route("/symbols/map", get(get_symbol_map_handler))
        .route(
            "/universes",
            get(list_universes_handler).post(create_universe_handler),
        )
        .route(
            "/universes/:id",
            get(get_universe_handler).delete(delete_universe_handler),
        )
        // Signal Quality / Export core endpoints
        .route(
            "/signals/quality-report",
            post(post_signal_quality_report_handler),
        )
        .route(
            "/export/csv",
            post(export_csv_handler).get(export_csv_handler),
        )
        // Notifications core endpoints
        .route(
            "/webhooks",
            post(register_webhook_handler).get(list_webhooks_handler),
        )
        .route("/webhooks/:id", delete(delete_webhook_handler))
        // Apply institutional security & governance middleware
        .layer(from_fn_with_state(state.clone(), rate_limit_middleware))
        .layer(from_fn_with_state(state.clone(), metering_middleware))
        .layer(from_fn_with_state(state.clone(), ip_whitelist_middleware))
        .layer(from_fn_with_state(state.clone(), auth_middleware));

    let v1_public = Router::new()
        .route("/health", get(health_check_handler))
        .route("/readyz", get(readyz_handler))
        .route("/auth/token", post(issue_token_handler))
        .route("/auth/register", any(gone_handler))
        .route("/news/articles/:id", any(gone_handler))
        .route("/audio/transcribe", any(gone_handler))
        .route("/model-card", get(get_model_card_handler));

    let v1_all = Router::new()
        .merge(v1_public)
        .merge(v1_protected)
        .layer(TimeoutLayer::new(Duration::from_secs(10)));

    Router::new().nest("/v1", v1_all)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{header, Request, StatusCode};
    use chrono::Utc;
    use http_body_util::BodyExt;
    use models::{
        BacktestResponse, HealthResponse, ModelMetadata, OptionsIvResponse, SLALatencyResponse,
        SLAStatusResponse, SentimentFeedResponse, SentimentResponse, SpilloverResponse,
    };
    use std::sync::Arc;
    use tower::ServiceExt;
    use uuid::Uuid;

    /// Helper to generate a valid test Bearer token Authorization header with custom role.
    fn test_auth_header_with_role(
        user_id: &str,
        role: &str,
    ) -> (header::HeaderName, header::HeaderValue) {
        let token = generate_jwt(user_id, 3600, Some(role), DEFAULT_DEV_JWT_SECRET.as_bytes())
            .expect("Should generate test JWT");

        (
            header::AUTHORIZATION,
            header::HeaderValue::from_str(&format!("Bearer {}", token)).unwrap(),
        )
    }

    /// Helper to generate a valid test Bearer token Authorization header for a given user.
    fn test_auth_header_for_user(user_id: &str) -> (header::HeaderName, header::HeaderValue) {
        test_auth_header_with_role(user_id, "institutional")
    }

    /// Helper to generate a default test Bearer token Authorization header.
    fn test_auth_header() -> (header::HeaderName, header::HeaderValue) {
        test_auth_header_for_user("test_quant_fund")
    }

    /// Helper to build the Axum application router with full API surface enabled (internal routes merged).
    fn create_app_with_full_surface() -> Router {
        std::env::set_var("ENABLE_FULL_API_SURFACE", "1");
        let mut state = AppState::default();
        state.enable_full_api_surface = true;
        create_app_with_state(state)
    }

    /// Helper to seed a test user directly in UserRegistry.
    fn seed_test_user(state: &mut AppState, email: &str, password: &str, role: &str) -> StoredUser {
        let user = StoredUser {
            id: Uuid::new_v4(),
            email: email.to_string(),
            password_hash: hash_password(password).unwrap(),
            role: role.to_string(),
            created_at: Utc::now(),
            is_active: true,
        };
        state.user_registry.insert(user.clone());
        user
    }

    #[tokio::test]
    async fn test_openapi_json_spec_endpoint() {
        let app = create_app();

        let req = Request::builder()
            .uri("/api-docs/openapi.json")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body_str = String::from_utf8(body.to_vec()).unwrap();
        let json_val: serde_json::Value = serde_json::from_str(&body_str).unwrap();

        assert_eq!(json_val["info"]["title"], "FinText-Alpha-Vectorizer API");
        assert_eq!(json_val["info"]["version"], "2.0.0-institutional");

        // Verify that core routes are documented in OpenAPI paths
        let paths = json_val["paths"].as_object().expect("Paths must be object");
        assert!(paths.contains_key("/health"));
        assert!(paths.contains_key("/auth/token"));
        assert!(paths.contains_key("/sentiment"));
        // Verify removed routes are NOT present in OpenAPI paths
        assert!(!paths.contains_key("/spillovers"));
        assert!(!paths.contains_key("/backtest"));
        assert!(!paths.contains_key("/fix/order"));
        assert!(!paths.contains_key("/crypto/sentiment"));

        // Verify Bearer JWT Security scheme
        let sec_schemes = &json_val["components"]["securitySchemes"];
        assert!(sec_schemes.get("bearerAuth").is_some());
    }

    #[tokio::test]
    async fn test_swagger_ui_endpoint_accessible() {
        let app = create_app();

        let req = Request::builder()
            .uri("/swagger-ui/")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert!(
            response.status() == StatusCode::OK || response.status().is_redirection(),
            "Swagger UI endpoint should return 200 OK or redirect"
        );
    }

    #[tokio::test]
    async fn test_public_v1_openapi_json_spec_endpoint() {
        let app = create_app();

        let req = Request::builder()
            .uri("/v1/api-docs/openapi.json")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body_str = String::from_utf8(body.to_vec()).unwrap();
        let json_val: serde_json::Value = serde_json::from_str(&body_str).unwrap();

        assert_eq!(
            json_val["info"]["title"],
            "FinText-Alpha-Vectorizer Public API"
        );
        assert_eq!(json_val["info"]["version"], "1.0.0");

        let paths = json_val["paths"].as_object().expect("Paths must be object");
        // Verify only /v1 paths are documented in public spec
        for path_key in paths.keys() {
            assert!(
                path_key.starts_with("/v1/"),
                "Expected public route to start with /v1/, but got: {}",
                path_key
            );
        }

        // Verify core public endpoints are present
        assert!(paths.contains_key("/v1/health"));
        assert!(paths.contains_key("/v1/auth/token"));
        assert!(paths.contains_key("/v1/users/me"));
        assert!(paths.contains_key("/v1/users/api-keys"));
        assert!(paths.contains_key("/v1/sentiment/feed"));
        assert!(paths.contains_key("/v1/sentiment/history"));
        assert!(paths.contains_key("/v1/sentiment/batch"));
        assert!(paths.contains_key("/v1/sentiment/confidence"));
        assert!(paths.contains_key("/v1/sentiment/disagreement"));
        assert!(paths.contains_key("/v1/sentiment/entities"));
        assert!(paths.contains_key("/v1/sentiment/sector"));
        assert!(paths.contains_key("/v1/news/articles"));
        assert!(paths.contains_key("/v1/events/8k"));
        assert!(paths.contains_key("/v1/events/earnings-surprise"));
        assert!(paths.contains_key("/v1/transcripts"));
        assert!(paths.contains_key("/v1/pit/certificate"));
        assert!(paths.contains_key("/v1/pit/replay"));
        assert!(paths.contains_key("/v1/model-card"));
        assert!(paths.contains_key("/v1/symbols/map"));
        assert!(paths.contains_key("/v1/universes"));
        assert!(paths.contains_key("/v1/signals/quality-report"));
        assert!(paths.contains_key("/v1/export/csv"));
        assert!(paths.contains_key("/v1/webhooks"));

        // Ensure non-public/internal routes are NOT in public spec
        assert!(!paths.contains_key("/v1/market/regime"));
        assert!(!paths.contains_key("/v1/spillovers"));
        assert!(!paths.contains_key("/v1/options/iv"));
        assert!(!paths.contains_key("/v1/billing/checkout"));
        assert!(!paths.contains_key("/v1/orgs"));
        assert!(!paths.contains_key("/v1/fix/orders"));
    }

    #[tokio::test]
    async fn test_public_v1_health_endpoint() {
        let app = create_app();

        let req = Request::builder()
            .uri("/v1/health")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_public_v1_auth_token_and_users_me() {
        let app = create_app();

        // 1. POST /v1/auth/token
        let auth_payload = serde_json::json!({
            "user_id": "v1_test_user",
            "tier": "institutional"
        });
        let req = Request::builder()
            .method("POST")
            .uri("/v1/auth/token")
            .header("X-Admin-Token", DEFAULT_DEV_ADMIN_TOKEN)
            .header("Content-Type", "application/json")
            .body(Body::from(serde_json::to_vec(&auth_payload).unwrap()))
            .unwrap();

        let response = app.clone().oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let token_resp: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let token = token_resp["token"]
            .as_str()
            .expect("token should be present");

        // 2. GET /v1/users/me
        let req2 = Request::builder()
            .method("GET")
            .uri("/v1/users/me")
            .header("Authorization", format!("Bearer {}", token))
            .body(Body::empty())
            .unwrap();

        let response2 = app.clone().oneshot(req2).await.unwrap();
        assert_eq!(response2.status(), StatusCode::OK);

        // 3. GET /v1/model-card
        let req3 = Request::builder()
            .method("GET")
            .uri("/v1/model-card")
            .body(Body::empty())
            .unwrap();

        let response3 = app.oneshot(req3).await.unwrap();
        assert_eq!(response3.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_health_endpoint() {
        let app = create_app();

        let req = Request::builder()
            .uri("/health")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body_str = String::from_utf8(body.to_vec()).unwrap();
        let health: HealthResponse = serde_json::from_str(&body_str).unwrap();

        assert_eq!(health.status, "ok");
        assert_eq!(health.version, "2.0.0-institutional");
        assert!(health.timestamp_us > 0);
    }

    #[tokio::test]
    async fn test_auth_token_issuance_valid_admin() {
        let app = create_app();

        let payload = serde_json::json!({
            "user_id": "quant_fund_beta",
            "expires_in_seconds": 7200,
            "role": "institutional"
        });

        let req = Request::builder()
            .method("POST")
            .uri("/auth/token")
            .header(header::CONTENT_TYPE, "application/json")
            .header("X-Admin-Token", DEFAULT_DEV_ADMIN_TOKEN)
            .body(Body::from(payload.to_string()))
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body_str = String::from_utf8(body.to_vec()).unwrap();
        let token_resp: IssueTokenResponse = serde_json::from_str(&body_str).unwrap();

        assert_eq!(token_resp.user_id, "quant_fund_beta");
        assert_eq!(token_resp.token_type, "Bearer");
        assert_eq!(token_resp.expires_in, 7200);
        assert_eq!(token_resp.role, "institutional");
        assert!(!token_resp.token.is_empty());

        // Validate that issued token can be decoded
        let claims = validate_jwt(&token_resp.token, DEFAULT_DEV_JWT_SECRET.as_bytes()).unwrap();
        assert_eq!(claims.sub, "quant_fund_beta");
    }

    #[tokio::test]
    async fn test_auth_token_issuance_invalid_admin() {
        let app = create_app();

        let payload = serde_json::json!({
            "user_id": "unauthorized_user"
        });

        let req = Request::builder()
            .method("POST")
            .uri("/auth/token")
            .header(header::CONTENT_TYPE, "application/json")
            .header("X-Admin-Token", "wrong_admin_token_value")
            .body(Body::from(payload.to_string()))
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_auth_token_issuance_missing_user_id() {
        let app = create_app();

        let payload = serde_json::json!({
            "user_id": ""
        });

        let req = Request::builder()
            .method("POST")
            .uri("/auth/token")
            .header(header::CONTENT_TYPE, "application/json")
            .header("X-Admin-Token", DEFAULT_DEV_ADMIN_TOKEN)
            .body(Body::from(payload.to_string()))
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_admin_reload_pit_data_unauthorized() {
        let app = create_app_with_full_surface();

        let req = Request::builder()
            .method("POST")
            .uri("/internal/admin/reload-pit-data")
            .header(header::CONTENT_TYPE, "application/json")
            .header("X-Admin-Token", "wrong_admin_token")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_admin_reload_pit_data_authorized() {
        let app = create_app_with_full_surface();

        let req = Request::builder()
            .method("POST")
            .uri("/internal/admin/reload-pit-data")
            .header(header::CONTENT_TYPE, "application/json")
            .header("X-Admin-Token", DEFAULT_DEV_ADMIN_TOKEN)
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body_str = String::from_utf8(body.to_vec()).unwrap();
        let reload_resp: serde_json::Value = serde_json::from_str(&body_str).unwrap();

        assert_eq!(reload_resp["status"], "success");
        assert!(reload_resp["ticker_intervals_count"].as_u64().is_some());
    }

    #[tokio::test]
    async fn test_protected_sentiment_without_token_rejected() {
        let app = create_app();

        let req = Request::builder()
            .uri("/sentiment?ticker=AAPL")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body_str = String::from_utf8(body.to_vec()).unwrap();
        let err: AuthErrorResponse = serde_json::from_str(&body_str).unwrap();
        assert_eq!(err.error, "Unauthorized");
    }

    #[tokio::test]
    async fn test_protected_sentiment_with_invalid_token_rejected() {
        let app = create_app();

        let req = Request::builder()
            .uri("/sentiment?ticker=AAPL")
            .header(header::AUTHORIZATION, "Bearer invalid_garbage_token_value")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_protected_sentiment_with_expired_token_rejected() {
        let app = create_app();

        let now = Utc::now().timestamp() as usize;
        let claims = Claims {
            sub: "expired_trader".to_string(),
            exp: now - 30,
            iat: now - 3600,
            role: "institutional".to_string(),
            org_id: None,
            sandbox: None,
        };
        let token = jsonwebtoken::encode(
            &jsonwebtoken::Header::default(),
            &claims,
            &jsonwebtoken::EncodingKey::from_secret(DEFAULT_DEV_JWT_SECRET.as_bytes()),
        )
        .unwrap();

        let req = Request::builder()
            .uri("/sentiment?ticker=AAPL")
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body_str = String::from_utf8(body.to_vec()).unwrap();
        assert!(body_str.contains("expired"));
    }

    #[tokio::test]
    async fn test_sentiment_endpoint_valid() {
        std::env::set_var("QUESTDB_MOCK_FALLBACK", "1");
        let app = create_app();

        let (auth_k, auth_v) = test_auth_header();
        let req = Request::builder()
            .uri("/sentiment?ticker=AAPL&date=2026-08-25")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // Verify Rate Limit headers are attached
        assert!(response.headers().contains_key(&HEADER_RATELIMIT_LIMIT));
        assert!(response.headers().contains_key(&HEADER_RATELIMIT_REMAINING));
        assert!(response.headers().contains_key(&HEADER_RATELIMIT_RESET));

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body_str = String::from_utf8(body.to_vec()).unwrap();
        let sentiment: SentimentResponse = serde_json::from_str(&body_str).unwrap();

        assert_eq!(sentiment.ticker, "AAPL");
        assert_eq!(sentiment.date, "2026-08-25");
        assert!(sentiment.signal_available_ts_us > 0);
    }

    #[tokio::test]
    async fn test_sentiment_endpoint_missing_ticker() {
        let app = create_app();

        let (auth_k, auth_v) = test_auth_header();
        let req = Request::builder()
            .uri("/sentiment?ticker=")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_sentiment_endpoint_invalid_ticker() {
        let app = create_app();

        let (auth_k, auth_v) = test_auth_header();
        let req = Request::builder()
            .uri("/sentiment?ticker=TOOLONGTICKERNAMEEXCEEDSLIMIT")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_spillovers_endpoint_valid() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();
        let req = Request::builder()
            .uri("/spillovers?ticker=AAPL&limit=10")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_spillovers_endpoint_missing_ticker() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();
        let req = Request::builder()
            .uri("/spillovers?ticker=")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_spillovers_endpoint_invalid_ticker() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();
        let req = Request::builder()
            .uri("/spillovers?ticker=TOOLONGTICKERNAMEEXCEEDSLIMIT")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_backtest_endpoint_valid() {
        let app = create_app();
        let payload = serde_json::json!({
            "ticker": "AAPL",
            "start_date": "2025-01-01",
            "end_date": "2025-03-31",
            "long_threshold": 0.2,
            "short_threshold": -0.2,
            "holding_days": 5,
            "initial_capital": 1000000.0
        });

        let (auth_k, auth_v) = test_auth_header();
        let req = Request::builder()
            .method("POST")
            .uri("/backtest")
            .header(header::CONTENT_TYPE, "application/json")
            .header(auth_k, auth_v)
            .body(Body::from(payload.to_string()))
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_backtest_endpoint_missing_ticker() {
        let app = create_app();
        let payload = serde_json::json!({
            "ticker": "",
            "start_date": "2025-01-01",
            "end_date": "2025-03-31"
        });

        let (auth_k, auth_v) = test_auth_header();
        let req = Request::builder()
            .method("POST")
            .uri("/backtest")
            .header(header::CONTENT_TYPE, "application/json")
            .header(auth_k, auth_v)
            .body(Body::from(payload.to_string()))
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_backtest_endpoint_invalid_date_range() {
        let app = create_app();
        let payload = serde_json::json!({
            "ticker": "AAPL",
            "start_date": "2025-06-01",
            "end_date": "2025-01-01"
        });

        let (auth_k, auth_v) = test_auth_header();
        let req = Request::builder()
            .method("POST")
            .uri("/backtest")
            .header(header::CONTENT_TYPE, "application/json")
            .header(auth_k, auth_v)
            .body(Body::from(payload.to_string()))
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_backtest_endpoint_invalid_thresholds() {
        let app = create_app();
        let payload = serde_json::json!({
            "ticker": "AAPL",
            "start_date": "2025-01-01",
            "end_date": "2025-03-31",
            "long_threshold": -0.5,
            "short_threshold": 0.5
        });

        let (auth_k, auth_v) = test_auth_header();
        let req = Request::builder()
            .method("POST")
            .uri("/backtest")
            .header(header::CONTENT_TYPE, "application/json")
            .header(auth_k, auth_v)
            .body(Body::from(payload.to_string()))
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_backtest_endpoint_multi_asset_portfolio() {
        let app = create_app();
        let payload = serde_json::json!({
            "tickers": ["AAPL", "NVDA", "MSFT"],
            "weights": [0.5, 0.3, 0.2],
            "benchmark_ticker": "SPY",
            "transaction_cost_bps": 10.0,
            "start_date": "2025-01-01",
            "end_date": "2025-03-31",
            "long_threshold": 0.2,
            "short_threshold": -0.2,
            "holding_days": 5,
            "initial_capital": 1000000.0
        });

        let (auth_k, auth_v) = test_auth_header();
        let req = Request::builder()
            .method("POST")
            .uri("/backtest")
            .header(header::CONTENT_TYPE, "application/json")
            .header(auth_k, auth_v)
            .body(Body::from(payload.to_string()))
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_backtest_endpoint_invalid_weights() {
        let app = create_app();
        let payload = serde_json::json!({
            "tickers": ["AAPL", "NVDA"],
            "weights": [0.5, 0.1], // sum is 0.6 != 1.0
            "start_date": "2025-01-01",
            "end_date": "2025-03-31"
        });

        let (auth_k, auth_v) = test_auth_header();
        let req = Request::builder()
            .method("POST")
            .uri("/backtest")
            .header(header::CONTENT_TYPE, "application/json")
            .header(auth_k, auth_v)
            .body(Body::from(payload.to_string()))
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_rate_limit_exceeded_returns_429() {
        std::env::set_var("QUESTDB_MOCK_FALLBACK", "1");

        let state = AppState {
            kafka_consumer: Arc::new(KafkaSubscriber::from_env()),
            jwt_secret: DEFAULT_DEV_JWT_SECRET.to_string(),
            admin_token: DEFAULT_DEV_ADMIN_TOKEN.to_string(),
            rate_limiter: Arc::new(PerUserRateLimiter::new(RateLimitConfig {
                requests_per_window: 2,
                window_seconds: 60,
            })),
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
            stripe_secret_key: None,
            stripe_webhook_secret: None,
            monthly_quota_cache: Arc::new(MonthlyQuotaCache::default()),
            audit_log_registry: Arc::new(AuditLogRegistry::new()),
            model_metadata: Arc::new(std::sync::RwLock::new(ModelMetadata::from_env_or_config())),
            digest_registry: Arc::new(DigestSubscriptionRegistry::new()),
            email_sender: Arc::new(MockEmailSender::new()),
            kafka_credentials_registry: Arc::new(KafkaCredentialsRegistry::new()),
            retention_registry: Arc::new(RetentionPolicyRegistry::new()),
            polling_webhook_registry: Arc::new(PollingWebhookRegistry::new()),
            chat_alert_registry: Arc::new(ChatAlertRegistry::new()),
            retraining_registry: Arc::new(crate::retraining::RetrainingRegistry::new()),
            fix_order_registry: Arc::new(FixOrderRegistry::new()),
            dlq_registry: Arc::new(DlqRegistry::new()),
            sandbox_registry: Arc::new(SandboxRegistry::new()),
            provenance_registry: Arc::new(ProvenanceRegistry::new()),
            anomaly_broadcaster: Arc::new(AnomalyBroadcaster::default()),
            enable_fix_bridge: true,
            production_mode: false,
            public_api_version: "v1".to_string(),
            enable_full_api_surface: false,
            scd2_registry: Arc::new(Scd2RevisionRegistry::new()),
            pit_cert_archiver: Arc::new(PitCertArchiver::from_env_or_config()),
            pit_db_store: None,
            timescaledb_primary: false,
            timescaledb_client: Arc::new(crate::storage::TimescaleDbClient::new(
                crate::storage::TimescaleDbClientConfig::default(),
            )),
            db_circuit_breaker: Arc::new(crate::resilience::DbCircuitBreaker::new(
                crate::resilience::CircuitBreakerConfig::default(),
            )),
            cache_config: crate::cache::CacheConfig::default(),
            provider_health_store: Arc::new(
                crate::handlers::provider_health::ProviderHealthStore::default(),
            ),
        };

        let app = create_app_with_state(state);
        let (auth_k, auth_v) = test_auth_header_for_user("heavy_trader");

        // Request 1: Allowed
        let req1 = Request::builder()
            .uri("/sentiment?ticker=AAPL")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp1 = app.clone().oneshot(req1).await.unwrap();
        assert_eq!(resp1.status(), StatusCode::OK);

        // Request 2: Allowed
        let req2 = Request::builder()
            .uri("/sentiment?ticker=AAPL")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp2 = app.clone().oneshot(req2).await.unwrap();
        assert_eq!(resp2.status(), StatusCode::OK);

        // Request 3: Blocked (429)
        let req3 = Request::builder()
            .uri("/sentiment?ticker=AAPL")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();
        let resp3 = app.clone().oneshot(req3).await.unwrap();
        assert_eq!(resp3.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(
            resp3.headers().get(&HEADER_RATELIMIT_REMAINING).unwrap(),
            "0"
        );
        assert!(resp3.headers().contains_key(&HEADER_RETRY_AFTER));
    }

    #[tokio::test]
    async fn test_rate_limit_independent_user_quotas() {
        std::env::set_var("QUESTDB_MOCK_FALLBACK", "1");

        let state = AppState {
            kafka_consumer: Arc::new(KafkaSubscriber::from_env()),
            jwt_secret: DEFAULT_DEV_JWT_SECRET.to_string(),
            admin_token: DEFAULT_DEV_ADMIN_TOKEN.to_string(),
            rate_limiter: Arc::new(PerUserRateLimiter::new(RateLimitConfig {
                requests_per_window: 1,
                window_seconds: 60,
            })),
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
            stripe_secret_key: None,
            stripe_webhook_secret: None,
            monthly_quota_cache: Arc::new(MonthlyQuotaCache::default()),
            audit_log_registry: Arc::new(AuditLogRegistry::new()),
            model_metadata: Arc::new(std::sync::RwLock::new(ModelMetadata::from_env_or_config())),
            digest_registry: Arc::new(DigestSubscriptionRegistry::new()),
            email_sender: Arc::new(MockEmailSender::new()),
            kafka_credentials_registry: Arc::new(KafkaCredentialsRegistry::new()),
            retention_registry: Arc::new(RetentionPolicyRegistry::new()),
            polling_webhook_registry: Arc::new(PollingWebhookRegistry::new()),
            chat_alert_registry: Arc::new(ChatAlertRegistry::new()),
            retraining_registry: Arc::new(crate::retraining::RetrainingRegistry::new()),
            fix_order_registry: Arc::new(FixOrderRegistry::new()),
            dlq_registry: Arc::new(DlqRegistry::new()),
            sandbox_registry: Arc::new(SandboxRegistry::new()),
            provenance_registry: Arc::new(ProvenanceRegistry::new()),
            anomaly_broadcaster: Arc::new(AnomalyBroadcaster::default()),
            enable_fix_bridge: true,
            production_mode: false,
            public_api_version: "v1".to_string(),
            enable_full_api_surface: false,
            scd2_registry: Arc::new(Scd2RevisionRegistry::new()),
            pit_cert_archiver: Arc::new(PitCertArchiver::from_env_or_config()),
            pit_db_store: None,
            timescaledb_primary: false,
            timescaledb_client: Arc::new(crate::storage::TimescaleDbClient::new(
                crate::storage::TimescaleDbClientConfig::default(),
            )),
            db_circuit_breaker: Arc::new(crate::resilience::DbCircuitBreaker::new(
                crate::resilience::CircuitBreakerConfig::default(),
            )),
            cache_config: crate::cache::CacheConfig::default(),
            provider_health_store: Arc::new(
                crate::handlers::provider_health::ProviderHealthStore::default(),
            ),
        };

        let app = create_app_with_state(state);

        let (k_user1, v_user1) = test_auth_header_for_user("trader_one");
        let (k_user2, v_user2) = test_auth_header_for_user("trader_two");

        // User 1 Request 1: OK
        let req1 = Request::builder()
            .uri("/sentiment?ticker=AAPL")
            .header(k_user1.clone(), v_user1.clone())
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            app.clone().oneshot(req1).await.unwrap().status(),
            StatusCode::OK
        );

        // User 1 Request 2: Blocked (429)
        let req2 = Request::builder()
            .uri("/sentiment?ticker=AAPL")
            .header(k_user1, v_user1)
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            app.clone().oneshot(req2).await.unwrap().status(),
            StatusCode::TOO_MANY_REQUESTS
        );

        // User 2 Request 1: Must be Allowed
        let req3 = Request::builder()
            .uri("/sentiment?ticker=AAPL")
            .header(k_user2, v_user2)
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            app.clone().oneshot(req3).await.unwrap().status(),
            StatusCode::OK
        );
    }

    #[tokio::test]
    async fn test_metering_captures_authenticated_request() {
        std::env::set_var("QUESTDB_MOCK_FALLBACK", "1");
        let (tx, mut rx) = create_metering_channel(10);

        let state = AppState {
            kafka_consumer: Arc::new(KafkaSubscriber::from_env()),
            jwt_secret: DEFAULT_DEV_JWT_SECRET.to_string(),
            admin_token: DEFAULT_DEV_ADMIN_TOKEN.to_string(),
            rate_limiter: Arc::new(PerUserRateLimiter::from_env()),
            metering_tx: Some(tx),
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
            stripe_secret_key: None,
            stripe_webhook_secret: None,
            monthly_quota_cache: Arc::new(MonthlyQuotaCache::default()),
            audit_log_registry: Arc::new(AuditLogRegistry::new()),
            model_metadata: Arc::new(std::sync::RwLock::new(ModelMetadata::from_env_or_config())),
            digest_registry: Arc::new(DigestSubscriptionRegistry::new()),
            email_sender: Arc::new(MockEmailSender::new()),
            kafka_credentials_registry: Arc::new(KafkaCredentialsRegistry::new()),
            retention_registry: Arc::new(RetentionPolicyRegistry::new()),
            polling_webhook_registry: Arc::new(PollingWebhookRegistry::new()),
            chat_alert_registry: Arc::new(ChatAlertRegistry::new()),
            retraining_registry: Arc::new(crate::retraining::RetrainingRegistry::new()),
            fix_order_registry: Arc::new(FixOrderRegistry::new()),
            dlq_registry: Arc::new(DlqRegistry::new()),
            sandbox_registry: Arc::new(SandboxRegistry::new()),
            provenance_registry: Arc::new(ProvenanceRegistry::new()),
            anomaly_broadcaster: Arc::new(AnomalyBroadcaster::default()),
            enable_fix_bridge: true,
            production_mode: false,
            public_api_version: "v1".to_string(),
            enable_full_api_surface: false,
            scd2_registry: Arc::new(Scd2RevisionRegistry::new()),
            pit_cert_archiver: Arc::new(PitCertArchiver::from_env_or_config()),
            pit_db_store: None,
            timescaledb_primary: false,
            timescaledb_client: Arc::new(crate::storage::TimescaleDbClient::new(
                crate::storage::TimescaleDbClientConfig::default(),
            )),
            db_circuit_breaker: Arc::new(crate::resilience::DbCircuitBreaker::new(
                crate::resilience::CircuitBreakerConfig::default(),
            )),
            cache_config: crate::cache::CacheConfig::default(),
            provider_health_store: Arc::new(
                crate::handlers::provider_health::ProviderHealthStore::default(),
            ),
        };

        let app = create_app_with_state(state);
        let (auth_k, auth_v) = test_auth_header_for_user("metered_fund_01");

        let req = Request::builder()
            .uri("/sentiment?ticker=AAPL&date=2026-08-25")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let event = rx.recv().await.expect("Expected captured usage event");
        assert_eq!(event.user_id, "metered_fund_01");
        assert_eq!(event.endpoint, "/sentiment");
        assert_eq!(event.method, "GET");
        assert_eq!(event.status_code, 200);
        assert!(event.latency_ms >= 0.0);
    }

    #[tokio::test]
    async fn test_metering_captures_rate_limited_429_request() {
        std::env::set_var("QUESTDB_MOCK_FALLBACK", "1");
        let (tx, mut rx) = create_metering_channel(10);

        let state = AppState {
            kafka_consumer: Arc::new(KafkaSubscriber::from_env()),
            jwt_secret: DEFAULT_DEV_JWT_SECRET.to_string(),
            admin_token: DEFAULT_DEV_ADMIN_TOKEN.to_string(),
            rate_limiter: Arc::new(PerUserRateLimiter::new(RateLimitConfig {
                requests_per_window: 1,
                window_seconds: 60,
            })),
            metering_tx: Some(tx),
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
            stripe_secret_key: None,
            stripe_webhook_secret: None,
            monthly_quota_cache: Arc::new(MonthlyQuotaCache::default()),
            audit_log_registry: Arc::new(AuditLogRegistry::new()),
            model_metadata: Arc::new(std::sync::RwLock::new(ModelMetadata::from_env_or_config())),
            digest_registry: Arc::new(DigestSubscriptionRegistry::new()),
            email_sender: Arc::new(MockEmailSender::new()),
            kafka_credentials_registry: Arc::new(KafkaCredentialsRegistry::new()),
            retention_registry: Arc::new(RetentionPolicyRegistry::new()),
            polling_webhook_registry: Arc::new(PollingWebhookRegistry::new()),
            chat_alert_registry: Arc::new(ChatAlertRegistry::new()),
            retraining_registry: Arc::new(crate::retraining::RetrainingRegistry::new()),
            fix_order_registry: Arc::new(FixOrderRegistry::new()),
            dlq_registry: Arc::new(DlqRegistry::new()),
            sandbox_registry: Arc::new(SandboxRegistry::new()),
            provenance_registry: Arc::new(ProvenanceRegistry::new()),
            anomaly_broadcaster: Arc::new(AnomalyBroadcaster::default()),
            enable_fix_bridge: true,
            production_mode: false,
            public_api_version: "v1".to_string(),
            enable_full_api_surface: false,
            scd2_registry: Arc::new(Scd2RevisionRegistry::new()),
            pit_cert_archiver: Arc::new(PitCertArchiver::from_env_or_config()),
            pit_db_store: None,
            timescaledb_primary: false,
            timescaledb_client: Arc::new(crate::storage::TimescaleDbClient::new(
                crate::storage::TimescaleDbClientConfig::default(),
            )),
            db_circuit_breaker: Arc::new(crate::resilience::DbCircuitBreaker::new(
                crate::resilience::CircuitBreakerConfig::default(),
            )),
            cache_config: crate::cache::CacheConfig::default(),
            provider_health_store: Arc::new(
                crate::handlers::provider_health::ProviderHealthStore::default(),
            ),
        };

        let app = create_app_with_state(state);
        let (auth_k, auth_v) = test_auth_header_for_user("rate_limited_user");

        // Request 1: Allowed (200)
        let req1 = Request::builder()
            .uri("/sentiment?ticker=AAPL")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            app.clone().oneshot(req1).await.unwrap().status(),
            StatusCode::OK
        );
        let ev1 = rx.recv().await.unwrap();
        assert_eq!(ev1.status_code, 200);

        // Request 2: Blocked by rate limiter (429) -> metering must still capture it!
        let req2 = Request::builder()
            .uri("/sentiment?ticker=AAPL")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            app.clone().oneshot(req2).await.unwrap().status(),
            StatusCode::TOO_MANY_REQUESTS
        );
        let ev2 = rx.recv().await.unwrap();
        assert_eq!(ev2.status_code, 429);
        assert_eq!(ev2.user_id, "rate_limited_user");
    }

    #[tokio::test]
    async fn test_websocket_auth_missing_token_rejected() {
        let app = create_app();

        let req = Request::builder().uri("/ws").body(Body::empty()).unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_websocket_auth_via_query_param_token() {
        let subscriber = Arc::new(KafkaSubscriber::new(KafkaSubscriberConfig {
            bootstrap_servers: "127.0.0.1:9092".to_string(),
            topic: "test.sentiment.updates".to_string(),
            group_id: "test-group".to_string(),
            timeout_ms: 500,
            channel_capacity: 32,
            max_connections: 10,
            enabled: true,
            mock_mode: true,
        }));

        let state = AppState {
            kafka_consumer: subscriber.clone(),
            jwt_secret: DEFAULT_DEV_JWT_SECRET.to_string(),
            admin_token: DEFAULT_DEV_ADMIN_TOKEN.to_string(),
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
            stripe_secret_key: None,
            stripe_webhook_secret: None,
            monthly_quota_cache: Arc::new(MonthlyQuotaCache::default()),
            audit_log_registry: Arc::new(AuditLogRegistry::new()),
            model_metadata: Arc::new(std::sync::RwLock::new(ModelMetadata::from_env_or_config())),
            digest_registry: Arc::new(DigestSubscriptionRegistry::new()),
            email_sender: Arc::new(MockEmailSender::new()),
            kafka_credentials_registry: Arc::new(KafkaCredentialsRegistry::new()),
            retention_registry: Arc::new(RetentionPolicyRegistry::new()),
            polling_webhook_registry: Arc::new(PollingWebhookRegistry::new()),
            chat_alert_registry: Arc::new(ChatAlertRegistry::new()),
            retraining_registry: Arc::new(crate::retraining::RetrainingRegistry::new()),
            fix_order_registry: Arc::new(FixOrderRegistry::new()),
            dlq_registry: Arc::new(DlqRegistry::new()),
            sandbox_registry: Arc::new(SandboxRegistry::new()),
            provenance_registry: Arc::new(ProvenanceRegistry::new()),
            anomaly_broadcaster: Arc::new(AnomalyBroadcaster::default()),
            enable_fix_bridge: true,
            production_mode: false,
            public_api_version: "v1".to_string(),
            enable_full_api_surface: false,
            scd2_registry: Arc::new(Scd2RevisionRegistry::new()),
            pit_cert_archiver: Arc::new(PitCertArchiver::from_env_or_config()),
            pit_db_store: None,
            timescaledb_primary: false,
            timescaledb_client: Arc::new(crate::storage::TimescaleDbClient::new(
                crate::storage::TimescaleDbClientConfig::default(),
            )),
            db_circuit_breaker: Arc::new(crate::resilience::DbCircuitBreaker::new(
                crate::resilience::CircuitBreakerConfig::default(),
            )),
            cache_config: crate::cache::CacheConfig::default(),
            provider_health_store: Arc::new(
                crate::handlers::provider_health::ProviderHealthStore::default(),
            ),
        };

        let app = create_app_with_state(state);
        let token =
            generate_jwt("ws_client", 3600, None, DEFAULT_DEV_JWT_SECRET.as_bytes()).unwrap();

        let req = Request::builder()
            .uri(format!("/ws?token={}", token))
            .header(header::HOST, "localhost:8000")
            .header(header::CONNECTION, "Upgrade")
            .header(header::UPGRADE, "websocket")
            .header("Sec-WebSocket-Version", "13")
            .header("Sec-WebSocket-Key", "dGhlIHNhbXBsZSBub25jZQ==")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_ne!(response.status(), StatusCode::UNAUTHORIZED);
        assert!(
            response.status() == StatusCode::SWITCHING_PROTOCOLS
                || response.status() == StatusCode::UPGRADE_REQUIRED
        );
    }

    #[tokio::test]
    async fn test_websocket_route_registered_and_mock_broadcast() {
        let subscriber = Arc::new(KafkaSubscriber::new(KafkaSubscriberConfig {
            bootstrap_servers: "127.0.0.1:9092".to_string(),
            topic: "test.sentiment.updates".to_string(),
            group_id: "test-group".to_string(),
            timeout_ms: 500,
            channel_capacity: 32,
            max_connections: 10,
            enabled: true,
            mock_mode: true,
        }));

        let state = AppState {
            kafka_consumer: subscriber.clone(),
            jwt_secret: DEFAULT_DEV_JWT_SECRET.to_string(),
            admin_token: DEFAULT_DEV_ADMIN_TOKEN.to_string(),
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
            stripe_secret_key: None,
            stripe_webhook_secret: None,
            monthly_quota_cache: Arc::new(MonthlyQuotaCache::default()),
            audit_log_registry: Arc::new(AuditLogRegistry::new()),
            model_metadata: Arc::new(std::sync::RwLock::new(ModelMetadata::from_env_or_config())),
            digest_registry: Arc::new(DigestSubscriptionRegistry::new()),
            email_sender: Arc::new(MockEmailSender::new()),
            kafka_credentials_registry: Arc::new(KafkaCredentialsRegistry::new()),
            retention_registry: Arc::new(RetentionPolicyRegistry::new()),
            polling_webhook_registry: Arc::new(PollingWebhookRegistry::new()),
            chat_alert_registry: Arc::new(ChatAlertRegistry::new()),
            retraining_registry: Arc::new(crate::retraining::RetrainingRegistry::new()),
            fix_order_registry: Arc::new(FixOrderRegistry::new()),
            dlq_registry: Arc::new(DlqRegistry::new()),
            sandbox_registry: Arc::new(SandboxRegistry::new()),
            provenance_registry: Arc::new(ProvenanceRegistry::new()),
            anomaly_broadcaster: Arc::new(AnomalyBroadcaster::default()),
            enable_fix_bridge: true,
            production_mode: false,
            public_api_version: "v1".to_string(),
            enable_full_api_surface: false,
            scd2_registry: Arc::new(Scd2RevisionRegistry::new()),
            pit_cert_archiver: Arc::new(PitCertArchiver::from_env_or_config()),
            pit_db_store: None,
            timescaledb_primary: false,
            timescaledb_client: Arc::new(crate::storage::TimescaleDbClient::new(
                crate::storage::TimescaleDbClientConfig::default(),
            )),
            db_circuit_breaker: Arc::new(crate::resilience::DbCircuitBreaker::new(
                crate::resilience::CircuitBreakerConfig::default(),
            )),
            cache_config: crate::cache::CacheConfig::default(),
            provider_health_store: Arc::new(
                crate::handlers::provider_health::ProviderHealthStore::default(),
            ),
        };

        let _app = create_app_with_state(state);

        let mut rx = subscriber.subscribe_client();
        let delivered = subscriber.broadcast_message(r#"{"ticker":"MSFT","sentiment_score":0.75}"#);
        assert_eq!(delivered, 1);
        let msg = rx.recv().await.unwrap();
        assert!(msg.contains("MSFT"));
    }

    #[tokio::test]
    async fn test_webhook_registration_and_list_flow() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header_for_user("webhook_trader_01");

        let payload = serde_json::json!({
            "url": "https://quant.fund.com/webhooks/sentiment",
            "events": ["sentiment", "spillover"]
        });

        // 1. POST /webhooks
        let req = Request::builder()
            .method("POST")
            .uri("/webhooks")
            .header(header::CONTENT_TYPE, "application/json")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::from(payload.to_string()))
            .unwrap();

        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);

        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let created: WebhookResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(created.user_id, "webhook_trader_01");
        assert_eq!(created.url, "https://quant.fund.com/webhooks/sentiment");
        assert_eq!(created.events.len(), 2);
        assert_eq!(created.secret.len(), 64);

        // 2. GET /webhooks
        let req2 = Request::builder()
            .uri("/webhooks")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();

        let resp2 = app.clone().oneshot(req2).await.unwrap();
        assert_eq!(resp2.status(), StatusCode::OK);

        let body2 = resp2.into_body().collect().await.unwrap().to_bytes();
        let list_resp: ListWebhooksResponse = serde_json::from_slice(&body2).unwrap();
        assert!(list_resp.count >= 1);
        assert!(list_resp.webhooks.iter().any(|w| w.id == created.id));
    }

    #[tokio::test]
    async fn test_webhook_registration_invalid_url_rejected() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        // Insecure HTTP to public domain (must be HTTPS)
        let payload = serde_json::json!({
            "url": "http://insecure.fund.com/webhook",
            "events": ["sentiment"]
        });

        let req = Request::builder()
            .method("POST")
            .uri("/webhooks")
            .header(header::CONTENT_TYPE, "application/json")
            .header(auth_k, auth_v)
            .body(Body::from(payload.to_string()))
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_webhook_registration_empty_events_rejected() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let payload = serde_json::json!({
            "url": "https://quant.fund.com/webhook",
            "events": []
        });

        let req = Request::builder()
            .method("POST")
            .uri("/webhooks")
            .header(header::CONTENT_TYPE, "application/json")
            .header(auth_k, auth_v)
            .body(Body::from(payload.to_string()))
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_webhook_deletion_flow() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header_for_user("del_user");

        // Register
        let payload = serde_json::json!({
            "url": "https://del.fund.com/webhook",
            "events": ["sentiment"]
        });
        let req = Request::builder()
            .method("POST")
            .uri("/webhooks")
            .header(header::CONTENT_TYPE, "application/json")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::from(payload.to_string()))
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let created: WebhookResponse =
            serde_json::from_slice(&resp.into_body().collect().await.unwrap().to_bytes()).unwrap();

        // Delete
        let del_req = Request::builder()
            .method("DELETE")
            .uri(format!("/webhooks/{}", created.id))
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let del_resp = app.clone().oneshot(del_req).await.unwrap();
        assert_eq!(del_resp.status(), StatusCode::OK);

        // Delete again -> 404 Not Found
        let del_req2 = Request::builder()
            .method("DELETE")
            .uri(format!("/webhooks/{}", created.id))
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();
        let del_resp2 = app.clone().oneshot(del_req2).await.unwrap();
        assert_eq!(del_resp2.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_webhook_unauthenticated_rejected() {
        let app = create_app();

        let req = Request::builder()
            .uri("/webhooks")
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_export_csv_endpoint_valid_stream() {
        std::env::set_var("QUESTDB_MOCK_FALLBACK", "1");
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .uri("/export/csv?ticker=AAPL&start_date=2025-01-01&end_date=2025-01-05&limit=10")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // Check Content-Type & Content-Disposition
        assert_eq!(
            resp.headers().get(header::CONTENT_TYPE).unwrap(),
            "text/csv; charset=utf-8"
        );
        let disp = resp
            .headers()
            .get(header::CONTENT_DISPOSITION)
            .unwrap()
            .to_str()
            .unwrap();
        assert!(disp.contains("attachment; filename=\"sentiment_AAPL_2025-01-01_2025-01-05.csv\""));

        // Read streamed body
        let body_bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let csv_text = String::from_utf8(body_bytes.to_vec()).unwrap();

        // Check header row
        assert!(csv_text
            .starts_with("published_utc,ticker,source,title,sentiment_score,vpin,gamma_exposure"));
        // Check data rows
        assert!(csv_text.contains("AAPL"));
        assert!(csv_text.contains("Institutional Wire"));
        assert!(csv_text.contains("2025-01-01T14:30:00.000000Z"));
    }

    #[tokio::test]
    async fn test_export_csv_endpoint_missing_ticker() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .uri("/export/csv?ticker=&start_date=2025-01-01&end_date=2025-01-05")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_export_csv_endpoint_invalid_date_range() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        // start_date after end_date
        let req = Request::builder()
            .uri("/export/csv?ticker=AAPL&start_date=2025-05-01&end_date=2025-01-01")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_export_csv_endpoint_unsupported_format() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .uri("/export/csv?ticker=AAPL&start_date=2025-01-01&end_date=2025-01-05&format=parquet")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_export_csv_unauthenticated_rejected() {
        let app = create_app();

        let req = Request::builder()
            .uri("/export/csv?ticker=AAPL&start_date=2025-01-01&end_date=2025-01-05")
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_user_register_and_login_flow() {
        let mut state = AppState::default();
        seed_test_user(
            &mut state,
            "lead.quant@citadel.com",
            "SuperSecretPass2026!",
            "user",
        );
        let app = create_app_with_state(state);

        // 1. Register -> 410 Gone with RFC 8594 Sunset header
        let reg_payload = serde_json::json!({
            "email": "lead.quant@citadel.com",
            "password": "SuperSecretPass2026!"
        });

        let reg_req = Request::builder()
            .method("POST")
            .uri("/auth/register")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(reg_payload.to_string()))
            .unwrap();

        let reg_resp = app.clone().oneshot(reg_req).await.unwrap();
        assert_eq!(reg_resp.status(), StatusCode::GONE);
        assert_eq!(
            reg_resp
                .headers()
                .get("sunset")
                .and_then(|h| h.to_str().ok()),
            Some("Wed, 11 Nov 2026 00:00:00 GMT")
        );
        let reg_body = reg_resp.into_body().collect().await.unwrap().to_bytes();
        let reg_json: serde_json::Value = serde_json::from_slice(&reg_body).unwrap();
        assert_eq!(reg_json["error"], "Gone");

        // 2. Login with seeded user
        let login_payload = serde_json::json!({
            "email": "lead.quant@citadel.com",
            "password": "SuperSecretPass2026!"
        });

        let login_req = Request::builder()
            .method("POST")
            .uri("/auth/login")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(login_payload.to_string()))
            .unwrap();

        let login_resp = app.clone().oneshot(login_req).await.unwrap();
        assert_eq!(login_resp.status(), StatusCode::OK);

        let login_body = login_resp.into_body().collect().await.unwrap().to_bytes();
        let login_data: LoginResponse = serde_json::from_slice(&login_body).unwrap();
        assert!(!login_data.token.is_empty());
        assert_eq!(login_data.email, "lead.quant@citadel.com");
        assert_eq!(login_data.token_type, "Bearer");

        // 3. Call GET /auth/me with issued JWT
        let me_req = Request::builder()
            .uri("/auth/me")
            .header(
                header::AUTHORIZATION,
                format!("Bearer {}", login_data.token),
            )
            .body(Body::empty())
            .unwrap();

        let me_resp = app.clone().oneshot(me_req).await.unwrap();
        assert_eq!(me_resp.status(), StatusCode::OK);

        let me_body = me_resp.into_body().collect().await.unwrap().to_bytes();
        let me_data: UserProfileResponse = serde_json::from_slice(&me_body).unwrap();
        assert_eq!(me_data.email, "lead.quant@citadel.com");
    }

    #[tokio::test]
    async fn test_user_register_invalid_email_and_password() {
        let app = create_app();

        // Invalid Email -> 410 Gone (endpoint permanently removed in v1.0)
        let bad_email_payload = serde_json::json!({
            "email": "not-an-email",
            "password": "ValidPass2026!"
        });
        let req1 = Request::builder()
            .method("POST")
            .uri("/auth/register")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(bad_email_payload.to_string()))
            .unwrap();
        let resp1 = app.clone().oneshot(req1).await.unwrap();
        assert_eq!(resp1.status(), StatusCode::GONE);
        assert_eq!(
            resp1.headers().get("sunset").and_then(|h| h.to_str().ok()),
            Some("Wed, 11 Nov 2026 00:00:00 GMT")
        );

        // Weak Password -> 410 Gone
        let weak_pass_payload = serde_json::json!({
            "email": "user@quant.com",
            "password": "lowercaseonly123"
        });
        let req2 = Request::builder()
            .method("POST")
            .uri("/auth/register")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(weak_pass_payload.to_string()))
            .unwrap();
        let resp2 = app.clone().oneshot(req2).await.unwrap();
        assert_eq!(resp2.status(), StatusCode::GONE);
    }

    #[tokio::test]
    async fn test_user_register_duplicate_email_conflict() {
        let app = create_app();

        let payload = serde_json::json!({
            "email": "unique@fund.com",
            "password": "Password123!"
        });

        // Register is permanently removed in v1.0 -> 410 Gone
        let req1 = Request::builder()
            .method("POST")
            .uri("/auth/register")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(payload.to_string()))
            .unwrap();
        let resp1 = app.clone().oneshot(req1).await.unwrap();
        assert_eq!(resp1.status(), StatusCode::GONE);

        let req2 = Request::builder()
            .method("POST")
            .uri("/auth/register")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(payload.to_string()))
            .unwrap();
        let resp2 = app.oneshot(req2).await.unwrap();
        assert_eq!(resp2.status(), StatusCode::GONE);
    }

    #[tokio::test]
    async fn test_user_login_invalid_credentials_rejected() {
        let mut state = AppState::default();
        seed_test_user(
            &mut state,
            "registered@fund.com",
            "CorrectPass2026!",
            "user",
        );
        let app = create_app_with_state(state);

        // Non-existent user
        let payload1 = serde_json::json!({
            "email": "nonexistent@fund.com",
            "password": "Password123!"
        });
        let req1 = Request::builder()
            .method("POST")
            .uri("/auth/login")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(payload1.to_string()))
            .unwrap();
        assert_eq!(
            app.clone().oneshot(req1).await.unwrap().status(),
            StatusCode::UNAUTHORIZED
        );

        // Try registered user with wrong password
        let payload2 = serde_json::json!({
            "email": "registered@fund.com",
            "password": "WrongPassword123!"
        });
        let req2 = Request::builder()
            .method("POST")
            .uri("/auth/login")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(payload2.to_string()))
            .unwrap();
        assert_eq!(
            app.oneshot(req2).await.unwrap().status(),
            StatusCode::UNAUTHORIZED
        );
    }

    #[tokio::test]
    async fn test_api_key_lifecycle_and_x_api_key_authentication() {
        std::env::set_var("QUESTDB_MOCK_FALLBACK", "1");
        let mut state = AppState::default();
        seed_test_user(
            &mut state,
            "trader.bot@fund.com",
            "BotPassword2026!",
            "user",
        );
        let app = create_app_with_state(state);

        let login_payload = serde_json::json!({
            "email": "trader.bot@fund.com",
            "password": "BotPassword2026!"
        });
        let login_req = Request::builder()
            .method("POST")
            .uri("/auth/login")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(login_payload.to_string()))
            .unwrap();
        let login_resp = app.clone().oneshot(login_req).await.unwrap();
        let login_data: LoginResponse =
            serde_json::from_slice(&login_resp.into_body().collect().await.unwrap().to_bytes())
                .unwrap();

        // 2. Generate API Key via POST /auth/api-keys
        let create_key_payload = serde_json::json!({
            "name": "Live Algo Trading Key"
        });
        let create_key_req = Request::builder()
            .method("POST")
            .uri("/auth/api-keys")
            .header(header::CONTENT_TYPE, "application/json")
            .header(
                header::AUTHORIZATION,
                format!("Bearer {}", login_data.token),
            )
            .body(Body::from(create_key_payload.to_string()))
            .unwrap();

        let create_key_resp = app.clone().oneshot(create_key_req).await.unwrap();
        assert_eq!(create_key_resp.status(), StatusCode::CREATED);

        let key_data: CreateApiKeyResponse = serde_json::from_slice(
            &create_key_resp
                .into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes(),
        )
        .unwrap();
        assert!(key_data.api_key.starts_with("ft_"));
        assert_eq!(key_data.name, "Live Algo Trading Key");

        // 3. List API Keys via GET /auth/api-keys
        let list_key_req = Request::builder()
            .uri("/auth/api-keys")
            .header(
                header::AUTHORIZATION,
                format!("Bearer {}", login_data.token),
            )
            .body(Body::empty())
            .unwrap();

        let list_key_resp = app.clone().oneshot(list_key_req).await.unwrap();
        assert_eq!(list_key_resp.status(), StatusCode::OK);
        let list_data: ListApiKeysResponse = serde_json::from_slice(
            &list_key_resp
                .into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes(),
        )
        .unwrap();
        assert_eq!(list_data.count, 1);
        assert_eq!(list_data.api_keys[0].id, key_data.id);

        // 4. Access protected endpoint /sentiment using X-API-Key header!
        let api_key_auth_req = Request::builder()
            .uri("/sentiment?ticker=AAPL")
            .header("x-api-key", &key_data.api_key)
            .body(Body::empty())
            .unwrap();

        let api_key_resp = app.clone().oneshot(api_key_auth_req).await.unwrap();
        assert_eq!(api_key_resp.status(), StatusCode::OK);

        // 5. Revoke API Key via DELETE /auth/api-keys/{id}
        let del_key_req = Request::builder()
            .method("DELETE")
            .uri(format!("/auth/api-keys/{}", key_data.id))
            .header(
                header::AUTHORIZATION,
                format!("Bearer {}", login_data.token),
            )
            .body(Body::empty())
            .unwrap();

        let del_key_resp = app.clone().oneshot(del_key_req).await.unwrap();
        assert_eq!(del_key_resp.status(), StatusCode::OK);

        // 6. Access protected endpoint with revoked key -> 401 Unauthorized!
        let revoked_auth_req = Request::builder()
            .uri("/sentiment?ticker=AAPL")
            .header("x-api-key", &key_data.api_key)
            .body(Body::empty())
            .unwrap();

        let revoked_resp = app.oneshot(revoked_auth_req).await.unwrap();
        assert_eq!(revoked_resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_sentiment_history_endpoint_valid_and_pagination() {
        std::env::set_var("QUESTDB_MOCK_FALLBACK", "1");
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        // 1. Page 1: Limit 5, Offset 0, Sort asc
        let req1 = Request::builder()
            .uri("/sentiment/history?ticker=AAPL&start_date=2025-01-01&end_date=2025-01-10&limit=5&offset=0&sort=asc")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();

        let resp1 = app.clone().oneshot(req1).await.unwrap();
        assert_eq!(resp1.status(), StatusCode::OK);

        let body1 = resp1.into_body().collect().await.unwrap().to_bytes();
        let data1: SentimentHistoryResponse = serde_json::from_slice(&body1).unwrap();
        assert_eq!(data1.ticker, "AAPL");
        assert_eq!(data1.count, 5);
        assert_eq!(data1.total, 10);
        assert_eq!(data1.limit, 5);
        assert_eq!(data1.offset, 0);
        assert_eq!(data1.sort, "asc");
        assert_eq!(data1.records.len(), 5);
        assert_eq!(
            data1.records[0].published_utc,
            "2025-01-01T14:30:00.000000Z"
        );

        // 2. Page 2: Limit 5, Offset 5
        let req2 = Request::builder()
            .uri("/sentiment/history?ticker=AAPL&start_date=2025-01-01&end_date=2025-01-10&limit=5&offset=5&sort=asc")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();

        let resp2 = app.clone().oneshot(req2).await.unwrap();
        assert_eq!(resp2.status(), StatusCode::OK);

        let body2 = resp2.into_body().collect().await.unwrap().to_bytes();
        let data2: SentimentHistoryResponse = serde_json::from_slice(&body2).unwrap();
        assert_eq!(data2.count, 5);
        assert_eq!(data2.offset, 5);
        assert_eq!(
            data2.records[0].published_utc,
            "2025-01-06T14:30:00.000000Z"
        );

        // 3. Sort desc
        let req3 = Request::builder()
            .uri("/sentiment/history?ticker=AAPL&start_date=2025-01-01&end_date=2025-01-10&limit=5&offset=0&sort=desc")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();

        let resp3 = app.oneshot(req3).await.unwrap();
        assert_eq!(resp3.status(), StatusCode::OK);
        let data3: SentimentHistoryResponse =
            serde_json::from_slice(&resp3.into_body().collect().await.unwrap().to_bytes()).unwrap();
        assert_eq!(data3.sort, "desc");
        assert_eq!(
            data3.records[0].published_utc,
            "2025-01-10T14:30:00.000000Z"
        );
    }

    #[tokio::test]
    async fn test_sentiment_history_endpoint_missing_ticker() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .uri("/sentiment/history?ticker=&start_date=2025-01-01&end_date=2025-01-05")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_sentiment_history_endpoint_invalid_date_range() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        // start_date after end_date
        let req = Request::builder()
            .uri("/sentiment/history?ticker=AAPL&start_date=2025-05-01&end_date=2025-01-01")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_sentiment_history_endpoint_invalid_limit_and_sort() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        // Limit > 1000
        let req1 = Request::builder()
            .uri("/sentiment/history?ticker=AAPL&start_date=2025-01-01&end_date=2025-01-05&limit=2000")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            app.clone().oneshot(req1).await.unwrap().status(),
            StatusCode::BAD_REQUEST
        );

        // Invalid sort
        let req2 = Request::builder()
            .uri("/sentiment/history?ticker=AAPL&start_date=2025-01-01&end_date=2025-01-05&sort=random")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            app.oneshot(req2).await.unwrap().status(),
            StatusCode::BAD_REQUEST
        );
    }

    #[tokio::test]
    async fn test_sentiment_history_unauthenticated_rejected() {
        let app = create_app();

        let req = Request::builder()
            .uri("/sentiment/history?ticker=AAPL&start_date=2025-01-01&end_date=2025-01-05")
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_spillover_matrix_endpoint_valid() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .uri("/spillovers/matrix?tickers=AAPL,MSFT,NVDA&start_date=2025-01-01&end_date=2025-03-31&min_correlation=0.5&max_lag_hours=24")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_spillover_matrix_endpoint_default_universe() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .uri("/spillovers/matrix?start_date=2025-01-01&end_date=2025-03-31")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_spillover_matrix_endpoint_invalid_bounds() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        // Inverted dates
        let req1 = Request::builder()
            .uri("/spillovers/matrix?tickers=AAPL,MSFT&start_date=2025-05-01&end_date=2025-01-01")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            app.clone().oneshot(req1).await.unwrap().status(),
            StatusCode::NOT_FOUND
        );

        // Invalid min_correlation > 1.0
        let req2 = Request::builder()
            .uri("/spillovers/matrix?tickers=AAPL,MSFT&start_date=2025-01-01&end_date=2025-03-31&min_correlation=2.5")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            app.clone().oneshot(req2).await.unwrap().status(),
            StatusCode::NOT_FOUND
        );

        // Invalid max_lag_hours > 168
        let req3 = Request::builder()
            .uri("/spillovers/matrix?tickers=AAPL,MSFT&start_date=2025-01-01&end_date=2025-03-31&max_lag_hours=500")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            app.oneshot(req3).await.unwrap().status(),
            StatusCode::NOT_FOUND
        );
    }

    #[tokio::test]
    async fn test_spillover_matrix_unauthenticated_rejected() {
        let app = create_app();

        let req = Request::builder()
            .uri("/spillovers/matrix?start_date=2025-01-01&end_date=2025-03-31")
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_pit_sentiment_history_symbol_change_fb_meta() {
        std::env::set_var("QUESTDB_MOCK_FALLBACK", "1");
        std::env::set_var("PIT_DATA_ENABLED", "1");
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        // 1. FB in 2021 (active period before June 2022) -> Valid records returned
        let req_fb = Request::builder()
            .uri("/sentiment/history?ticker=FB&start_date=2021-01-01&end_date=2021-06-01")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();

        let resp_fb = app.clone().oneshot(req_fb).await.unwrap();
        assert_eq!(resp_fb.status(), StatusCode::OK);
        let body_fb = resp_fb.into_body().collect().await.unwrap().to_bytes();
        let data_fb: SentimentHistoryResponse = serde_json::from_slice(&body_fb).unwrap();
        assert_eq!(data_fb.ticker, "FB");
        assert!(data_fb.count > 0);

        // 2. META in 2021 (before rename in June 2022) -> PIT should return 0 records (empty)
        let req_meta = Request::builder()
            .uri("/sentiment/history?ticker=META&start_date=2021-01-01&end_date=2021-06-01")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();

        let resp_meta = app.oneshot(req_meta).await.unwrap();
        assert_eq!(resp_meta.status(), StatusCode::OK);
        let body_meta = resp_meta.into_body().collect().await.unwrap().to_bytes();
        let data_meta: SentimentHistoryResponse = serde_json::from_slice(&body_meta).unwrap();
        assert_eq!(data_meta.ticker, "META");
        assert_eq!(data_meta.count, 0);
        assert_eq!(data_meta.total, 0);
    }

    #[tokio::test]
    async fn test_pit_delisted_security_filtered_twtr() {
        std::env::set_var("QUESTDB_MOCK_FALLBACK", "1");
        std::env::set_var("PIT_DATA_ENABLED", "1");
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        // TWTR in 2024 (delisted in Oct 2022) -> PIT should return 0 records
        let req = Request::builder()
            .uri("/sentiment/history?ticker=TWTR&start_date=2024-01-01&end_date=2024-03-01")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let data: SentimentHistoryResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(data.count, 0);
    }

    #[tokio::test]
    async fn test_pit_spillover_matrix_excludes_invalid_tickers() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .uri(
                "/spillovers/matrix?tickers=AAPL,FB,META&start_date=2021-01-01&end_date=2021-03-31",
            )
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_sector_sentiment_valid_average() {
        std::env::set_var("QUESTDB_MOCK_FALLBACK", "1");
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .uri("/sentiment/sector?sector=Technology&start_date=2025-01-01&end_date=2025-03-31&aggregation=average&min_confidence=0.5")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let data: SectorSentimentResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(data.sector, "Technology");
        assert_eq!(data.aggregation, "average");
        assert!(data.tickers_included > 0);
        assert!(data.record_count > 0);
    }

    #[tokio::test]
    async fn test_sector_sentiment_valid_weighted_average() {
        std::env::set_var("QUESTDB_MOCK_FALLBACK", "1");
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .uri("/sentiment/sector?sector=Financials&start_date=2025-01-01&end_date=2025-03-31&aggregation=weighted_average")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let data: SectorSentimentResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(data.sector, "Financials");
        assert_eq!(data.aggregation, "weighted_average");
    }

    #[tokio::test]
    async fn test_sector_sentiment_invalid_sector_not_found() {
        std::env::set_var("QUESTDB_MOCK_FALLBACK", "1");
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .uri(
                "/sentiment/sector?sector=QuantumPhysics&start_date=2025-01-01&end_date=2025-03-31",
            )
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_sector_sentiment_invalid_aggregation() {
        std::env::set_var("QUESTDB_MOCK_FALLBACK", "1");
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .uri("/sentiment/sector?sector=Technology&start_date=2025-01-01&end_date=2025-03-31&aggregation=invalid_op")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_sector_sentiment_unauthenticated() {
        std::env::set_var("QUESTDB_MOCK_FALLBACK", "1");
        let app = create_app();

        let req = Request::builder()
            .uri("/sentiment/sector?sector=Technology&start_date=2025-01-01&end_date=2025-03-31")
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_batch_sentiment_valid() {
        std::env::set_var("QUESTDB_MOCK_FALLBACK", "1");
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .uri("/sentiment/batch?tickers=AAPL,MSFT,NVDA")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let data: BatchSentimentResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(data.count, 3);
        assert_eq!(data.results.len(), 3);
        assert_eq!(data.results[0].ticker, "AAPL");
        assert_eq!(data.results[1].ticker, "MSFT");
        assert_eq!(data.results[2].ticker, "NVDA");
        assert!(data.results[0].confidence > 0.0);
    }

    #[tokio::test]
    async fn test_batch_sentiment_with_date() {
        std::env::set_var("QUESTDB_MOCK_FALLBACK", "1");
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .uri("/sentiment/batch?tickers=AAPL,GOOGL&date=2025-01-15")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let data: BatchSentimentResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(data.count, 2);
        assert_eq!(data.results[0].date, "2025-01-15");
    }

    #[tokio::test]
    async fn test_batch_sentiment_empty_tickers() {
        std::env::set_var("QUESTDB_MOCK_FALLBACK", "1");
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .uri("/sentiment/batch?tickers=")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_batch_sentiment_too_many_tickers() {
        std::env::set_var("QUESTDB_MOCK_FALLBACK", "1");
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let tickers_55 = (0..55)
            .map(|i| format!("TICK{}", i))
            .collect::<Vec<String>>()
            .join(",");
        let req = Request::builder()
            .uri(format!("/sentiment/batch?tickers={}", tickers_55))
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_batch_sentiment_unauthenticated() {
        std::env::set_var("QUESTDB_MOCK_FALLBACK", "1");
        let app = create_app();

        let req = Request::builder()
            .uri("/sentiment/batch?tickers=AAPL,MSFT")
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_symbol_map_auto_all() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .uri("/symbols/map?identifier=AAPL&output_type=all")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let data: SymbolMapResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(data.input_identifier, "AAPL");
        assert_eq!(data.input_type, "ticker");
        assert_eq!(data.output_type, "all");
        assert_eq!(data.result.ticker.as_deref(), Some("AAPL"));
        assert_eq!(data.result.figi.as_deref(), Some("BBG000B9XRY4"));
        assert_eq!(data.result.cusip.as_deref(), Some("037833100"));
        assert_eq!(data.result.isin.as_deref(), Some("US0378331005"));
    }

    #[tokio::test]
    async fn test_symbol_map_isin_to_ticker() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .uri("/symbols/map?identifier=US0378331005&input_type=isin&output_type=ticker")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let data: SymbolMapResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(data.input_type, "isin");
        assert_eq!(data.output_type, "ticker");
        assert_eq!(data.result.ticker.as_deref(), Some("AAPL"));
        assert_eq!(data.result.figi, None);
        assert_eq!(data.result.cusip, None);
        assert_eq!(data.result.isin, None);
    }

    #[tokio::test]
    async fn test_symbol_map_figi_to_all() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .uri("/symbols/map?identifier=BBG000BBJQV0")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let data: SymbolMapResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(data.input_type, "figi");
        assert_eq!(data.result.ticker.as_deref(), Some("NVDA"));
    }

    #[tokio::test]
    async fn test_symbol_map_not_found() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .uri("/symbols/map?identifier=NONEXISTENT999")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_symbol_map_invalid_input_type() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .uri("/symbols/map?identifier=AAPL&input_type=invalid_type")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_symbol_map_unauthenticated() {
        let app = create_app();

        let req = Request::builder()
            .uri("/symbols/map?identifier=AAPL")
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_universe_crud_lifecycle() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header_for_user("universe_trader_01");

        // 1. Create Universe
        let create_payload = serde_json::json!({
            "name": "Tech Titans",
            "tickers": ["AAPL", "MSFT", "NVDA"]
        });
        let req1 = Request::builder()
            .method("POST")
            .uri("/universes")
            .header(auth_k.clone(), auth_v.clone())
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(create_payload.to_string()))
            .unwrap();

        let resp1 = app.clone().oneshot(req1).await.unwrap();
        assert_eq!(resp1.status(), StatusCode::CREATED);
        let body1 = resp1.into_body().collect().await.unwrap().to_bytes();
        let u1: Universe = serde_json::from_slice(&body1).unwrap();
        assert_eq!(u1.name, "Tech Titans");
        assert_eq!(u1.tickers, vec!["AAPL", "MSFT", "NVDA"]);

        // 2. List Universes
        let req2 = Request::builder()
            .uri("/universes")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp2 = app.clone().oneshot(req2).await.unwrap();
        assert_eq!(resp2.status(), StatusCode::OK);
        let body2 = resp2.into_body().collect().await.unwrap().to_bytes();
        let list_resp: ListUniversesResponse = serde_json::from_slice(&body2).unwrap();
        assert_eq!(list_resp.count, 1);
        assert_eq!(list_resp.universes[0].id, u1.id);

        // 3. Get Universe by ID
        let req3 = Request::builder()
            .uri(format!("/universes/{}", u1.id))
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp3 = app.clone().oneshot(req3).await.unwrap();
        assert_eq!(resp3.status(), StatusCode::OK);

        // 4. Update Universe
        let update_payload = serde_json::json!({
            "name": "Mega Tech Titans",
            "tickers": ["AAPL", "MSFT", "NVDA", "AMZN"]
        });
        let req4 = Request::builder()
            .method("PUT")
            .uri(format!("/universes/{}", u1.id))
            .header(auth_k.clone(), auth_v.clone())
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(update_payload.to_string()))
            .unwrap();
        let resp4 = app.clone().oneshot(req4).await.unwrap();
        assert_eq!(resp4.status(), StatusCode::OK);
        let body4 = resp4.into_body().collect().await.unwrap().to_bytes();
        let u4: Universe = serde_json::from_slice(&body4).unwrap();
        assert_eq!(u4.name, "Mega Tech Titans");
        assert_eq!(u4.tickers.len(), 4);

        // 5. Delete Universe
        let req5 = Request::builder()
            .method("DELETE")
            .uri(format!("/universes/{}", u1.id))
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp5 = app.clone().oneshot(req5).await.unwrap();
        assert_eq!(resp5.status(), StatusCode::OK);

        // 6. Verify 404 after deletion
        let req6 = Request::builder()
            .uri(format!("/universes/{}", u1.id))
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();
        let resp6 = app.oneshot(req6).await.unwrap();
        assert_eq!(resp6.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_universe_creation_validation() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        // Empty Name
        let req1 = Request::builder()
            .method("POST")
            .uri("/universes")
            .header(auth_k.clone(), auth_v.clone())
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                serde_json::json!({"name": "  ", "tickers": ["AAPL"]}).to_string(),
            ))
            .unwrap();
        assert_eq!(
            app.clone().oneshot(req1).await.unwrap().status(),
            StatusCode::BAD_REQUEST
        );

        // Empty Tickers
        let req2 = Request::builder()
            .method("POST")
            .uri("/universes")
            .header(auth_k.clone(), auth_v.clone())
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                serde_json::json!({"name": "Tech", "tickers": []}).to_string(),
            ))
            .unwrap();
        assert_eq!(
            app.clone().oneshot(req2).await.unwrap().status(),
            StatusCode::BAD_REQUEST
        );

        // Invalid Ticker format
        let req3 = Request::builder()
            .method("POST")
            .uri("/universes")
            .header(auth_k, auth_v)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                serde_json::json!({"name": "Tech", "tickers": ["BAD$TICKER!"]}).to_string(),
            ))
            .unwrap();
        assert_eq!(
            app.oneshot(req3).await.unwrap().status(),
            StatusCode::BAD_REQUEST
        );
    }

    #[tokio::test]
    async fn test_universe_ownership_isolation() {
        let app = create_app();
        let (k_user1, v_user1) = test_auth_header_for_user("user_owner");
        let (k_user2, v_user2) = test_auth_header_for_user("user_intruder");

        // User 1 creates universe
        let req1 = Request::builder()
            .method("POST")
            .uri("/universes")
            .header(k_user1, v_user1)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                serde_json::json!({"name": "Secret Fund", "tickers": ["AAPL"]}).to_string(),
            ))
            .unwrap();
        let resp1 = app.clone().oneshot(req1).await.unwrap();
        let u: Universe =
            serde_json::from_slice(&resp1.into_body().collect().await.unwrap().to_bytes()).unwrap();

        // User 2 cannot access User 1's universe
        let req2 = Request::builder()
            .uri(format!("/universes/{}", u.id))
            .header(k_user2.clone(), v_user2.clone())
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            app.clone().oneshot(req2).await.unwrap().status(),
            StatusCode::NOT_FOUND
        );

        // User 2 cannot update User 1's universe
        let req3 = Request::builder()
            .method("PUT")
            .uri(format!("/universes/{}", u.id))
            .header(k_user2.clone(), v_user2.clone())
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                serde_json::json!({"name": "Hacked"}).to_string(),
            ))
            .unwrap();
        assert_eq!(
            app.clone().oneshot(req3).await.unwrap().status(),
            StatusCode::NOT_FOUND
        );

        // User 2 cannot delete User 1's universe
        let req4 = Request::builder()
            .method("DELETE")
            .uri(format!("/universes/{}", u.id))
            .header(k_user2, v_user2)
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            app.oneshot(req4).await.unwrap().status(),
            StatusCode::NOT_FOUND
        );
    }

    #[tokio::test]
    async fn test_batch_sentiment_with_universe_id() {
        std::env::set_var("QUESTDB_MOCK_FALLBACK", "1");
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header_for_user("batch_universe_trader");

        // 1. Create a Universe
        let req1 = Request::builder()
            .method("POST")
            .uri("/universes")
            .header(auth_k.clone(), auth_v.clone())
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                serde_json::json!({"name": "Pair Trades", "tickers": ["AAPL", "MSFT"]}).to_string(),
            ))
            .unwrap();
        let resp1 = app.clone().oneshot(req1).await.unwrap();
        let u: Universe =
            serde_json::from_slice(&resp1.into_body().collect().await.unwrap().to_bytes()).unwrap();

        // 2. Query Batch Sentiment via universe_id
        let req2 = Request::builder()
            .uri(format!("/sentiment/batch?universe_id={}", u.id))
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();
        let resp2 = app.oneshot(req2).await.unwrap();
        assert_eq!(resp2.status(), StatusCode::OK);
        let body2 = resp2.into_body().collect().await.unwrap().to_bytes();
        let batch: BatchSentimentResponse = serde_json::from_slice(&body2).unwrap();
        assert_eq!(batch.count, 2);
        assert_eq!(batch.results[0].ticker, "AAPL");
        assert_eq!(batch.results[1].ticker, "MSFT");
    }

    #[tokio::test]
    async fn test_batch_sentiment_conflicting_params() {
        std::env::set_var("QUESTDB_MOCK_FALLBACK", "1");
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .uri(format!(
                "/sentiment/batch?tickers=AAPL&universe_id={}",
                Uuid::new_v4()
            ))
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_batch_sentiment_missing_both_params() {
        std::env::set_var("QUESTDB_MOCK_FALLBACK", "1");
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .uri("/sentiment/batch")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_sentiment_history_min_quality_filter() {
        std::env::set_var("QUESTDB_MOCK_FALLBACK", "1");
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        // 1. Unfiltered query
        let req1 = Request::builder()
            .uri("/sentiment/history?ticker=AAPL&start_date=2025-01-01&end_date=2025-01-10")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp1 = app.clone().oneshot(req1).await.unwrap();
        assert_eq!(resp1.status(), StatusCode::OK);
        let body1 = resp1.into_body().collect().await.unwrap().to_bytes();
        let res1: SentimentHistoryResponse = serde_json::from_slice(&body1).unwrap();
        assert_eq!(res1.count, 10);
        for rec in &res1.records {
            assert!(rec.data_quality_score > 0.0 && rec.data_quality_score <= 1.0);
        }

        // 2. Filtered with high min_quality
        let req2 = Request::builder()
            .uri("/sentiment/history?ticker=AAPL&start_date=2025-01-01&end_date=2025-01-10&min_quality=0.85")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp2 = app.clone().oneshot(req2).await.unwrap();
        assert_eq!(resp2.status(), StatusCode::OK);
        let body2 = resp2.into_body().collect().await.unwrap().to_bytes();
        let res2: SentimentHistoryResponse = serde_json::from_slice(&body2).unwrap();
        for rec in &res2.records {
            assert!(
                rec.data_quality_score >= 0.85,
                "Record quality {} must be >= 0.85",
                rec.data_quality_score
            );
        }

        // 3. Invalid min_quality (> 1.0)
        let req3 = Request::builder()
            .uri("/sentiment/history?ticker=AAPL&start_date=2025-01-01&end_date=2025-01-10&min_quality=1.5")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();
        let resp3 = app.oneshot(req3).await.unwrap();
        assert_eq!(resp3.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_export_csv_min_quality_and_quality_column() {
        std::env::set_var("QUESTDB_MOCK_FALLBACK", "1");
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .uri("/export/csv?ticker=AAPL&start_date=2025-01-01&end_date=2025-01-05&min_quality=0.80")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body_bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let csv_text = String::from_utf8_lossy(&body_bytes);
        let lines: Vec<&str> = csv_text.lines().collect();
        assert!(lines.len() >= 1);
        assert!(lines[0].contains("data_quality_score"));
    }

    #[tokio::test]
    async fn test_sentiment_endpoints_include_data_quality_score() {
        std::env::set_var("QUESTDB_MOCK_FALLBACK", "1");
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        // 1. Single sentiment endpoint
        let req1 = Request::builder()
            .uri("/sentiment?ticker=AAPL")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp1 = app.clone().oneshot(req1).await.unwrap();
        assert_eq!(resp1.status(), StatusCode::OK);
        let body1 = resp1.into_body().collect().await.unwrap().to_bytes();
        let res1: SentimentResponse = serde_json::from_slice(&body1).unwrap();
        assert!(res1.data_quality_score >= 0.5 && res1.data_quality_score <= 1.0);

        // 2. Batch sentiment endpoint
        let req2 = Request::builder()
            .uri("/sentiment/batch?tickers=AAPL,NVDA")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();
        let resp2 = app.oneshot(req2).await.unwrap();
        assert_eq!(resp2.status(), StatusCode::OK);
        let body2 = resp2.into_body().collect().await.unwrap().to_bytes();
        let res2: BatchSentimentResponse = serde_json::from_slice(&body2).unwrap();
        assert_eq!(res2.count, 2);
        for item in &res2.results {
            assert!(item.data_quality_score >= 0.5 && item.data_quality_score <= 1.0);
        }
    }

    #[tokio::test]
    async fn test_options_iv_endpoint_valid_chain() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .uri("/options/iv?ticker=AAPL&expiration_date=2025-12-19&option_type=all")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body_bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let res: OptionsIvResponse = serde_json::from_slice(&body_bytes).unwrap();
        assert_eq!(res.ticker, "AAPL");
        assert_eq!(res.expiration_date, "2025-12-19");
        assert!(res.underlying_price > 0.0);
        assert!(res.count >= 10);
        assert_eq!(res.contracts.len(), res.count);

        for contract in &res.contracts {
            assert_eq!(contract.underlying_ticker, "AAPL");
            assert!(contract.strike > 0.0);
            assert!(contract.implied_volatility > 0.0);
            assert!(contract.last > 0.0);
            assert!(contract.bid > 0.0);
            assert!(contract.ask >= contract.bid);
        }
    }

    #[tokio::test]
    async fn test_options_iv_endpoint_single_strike_call() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .uri("/options/iv?ticker=NVDA&expiration_date=2025-12-19&option_type=call&strike=130")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body_bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let res: OptionsIvResponse = serde_json::from_slice(&body_bytes).unwrap();
        assert_eq!(res.ticker, "NVDA");
        assert_eq!(res.count, 1);
        assert_eq!(res.contracts.len(), 1);
        assert_eq!(res.contracts[0].strike, 130.0);
        assert_eq!(res.contracts[0].option_type, "CALL");
        assert!(res.contracts[0].delta > 0.0 && res.contracts[0].delta < 1.0);
        assert!(res.contracts[0].gamma > 0.0);
        assert!(res.contracts[0].vega > 0.0);
    }

    #[tokio::test]
    async fn test_options_iv_endpoint_invalid_date_and_parameters() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        // 1. Invalid date format
        let req1 = Request::builder()
            .uri("/options/iv?ticker=AAPL&expiration_date=invalid-date")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp1 = app.clone().oneshot(req1).await.unwrap();
        assert_eq!(resp1.status(), StatusCode::BAD_REQUEST);

        // 2. Negative strike
        let req2 = Request::builder()
            .uri("/options/iv?ticker=AAPL&expiration_date=2025-12-19&strike=-50")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp2 = app.clone().oneshot(req2).await.unwrap();
        assert_eq!(resp2.status(), StatusCode::BAD_REQUEST);

        // 3. Out-of-bounds risk-free rate
        let req3 = Request::builder()
            .uri("/options/iv?ticker=AAPL&expiration_date=2025-12-19&risk_free_rate=0.50")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();
        let resp3 = app.oneshot(req3).await.unwrap();
        assert_eq!(resp3.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_options_iv_endpoint_unauthenticated_rejected() {
        let app = create_app();

        let req = Request::builder()
            .uri("/options/iv?ticker=AAPL&expiration_date=2025-12-19")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_unusual_options_endpoint_all_tickers() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .uri("/options/unusual?min_volume_oi_ratio=2.0&min_volume=100&days=1&limit=10")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body_bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let res: UnusualOptionsResponse = serde_json::from_slice(&body_bytes).unwrap();
        assert_eq!(res.ticker, "ALL");
        assert_eq!(res.min_volume_oi_ratio, 2.0);
        assert_eq!(res.min_volume, 100);
        assert_eq!(res.days, 1);
        assert!(res.count > 0);
        assert_eq!(res.items.len(), res.count);

        for item in &res.items {
            assert!(item.volume >= 100);
            assert!(item.volume_oi_ratio >= 2.0);
            assert!(item.score > 0.0);
            assert!(!item.ticker.is_empty());
        }

        // Verify sorted by score descending
        for pair in res.items.windows(2) {
            assert!(pair[0].score >= pair[1].score);
        }
    }

    #[tokio::test]
    async fn test_unusual_options_endpoint_ticker_filter() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .uri("/options/unusual?ticker=AAPL&min_volume_oi_ratio=2.0&min_volume=100")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body_bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let res: UnusualOptionsResponse = serde_json::from_slice(&body_bytes).unwrap();
        assert_eq!(res.ticker, "AAPL");
        assert!(res.count > 0);

        for item in &res.items {
            assert_eq!(item.underlying_ticker, "AAPL");
            assert!(item.volume_oi_ratio >= 2.0);
        }
    }

    #[tokio::test]
    async fn test_unusual_options_endpoint_invalid_parameters() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        // 1. Invalid days (> 7)
        let req1 = Request::builder()
            .uri("/options/unusual?days=10")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp1 = app.clone().oneshot(req1).await.unwrap();
        assert_eq!(resp1.status(), StatusCode::BAD_REQUEST);

        // 2. Negative min_volume_oi_ratio
        let req2 = Request::builder()
            .uri("/options/unusual?min_volume_oi_ratio=-5.0")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp2 = app.clone().oneshot(req2).await.unwrap();
        assert_eq!(resp2.status(), StatusCode::BAD_REQUEST);

        // 3. Limit = 0
        let req3 = Request::builder()
            .uri("/options/unusual?limit=0")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();
        let resp3 = app.oneshot(req3).await.unwrap();
        assert_eq!(resp3.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_unusual_options_endpoint_unauthenticated_rejected() {
        let app = create_app();

        let req = Request::builder()
            .uri("/options/unusual")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_usage_stats_endpoint_default_parameters() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .uri("/usage/stats")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body_bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let res: UsageStatsResponse = serde_json::from_slice(&body_bytes).unwrap();
        assert_eq!(res.user_id, "test_quant_fund");
        assert_eq!(res.group_by, "day");
        assert!(res.summary.total_requests > 0);
        assert_eq!(
            res.summary.total_requests,
            res.summary.successful_requests + res.summary.failed_requests
        );
        assert!(res.summary.rate_limited_requests <= res.summary.failed_requests);
        assert!(res.summary.average_latency_ms > 0.0);
        assert!(res.summary.p95_latency_ms >= res.summary.average_latency_ms);
        assert!(res.summary.max_latency_ms >= res.summary.p95_latency_ms);
        assert!(!res.breakdown.is_empty());
    }

    #[tokio::test]
    async fn test_usage_stats_endpoint_endpoint_grouping() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .uri(
                "/usage/stats?start_date=2026-08-01&end_date=2026-08-29&group_by=endpoint&limit=10",
            )
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body_bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let res: UsageStatsResponse = serde_json::from_slice(&body_bytes).unwrap();
        assert_eq!(res.group_by, "endpoint");
        assert_eq!(res.start_date, "2026-08-01");
        assert_eq!(res.end_date, "2026-08-29");
        assert!(!res.breakdown.is_empty());
        assert!(res.breakdown.iter().any(|b| b.key == "/sentiment"));
    }

    #[tokio::test]
    async fn test_usage_stats_endpoint_status_code_grouping() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .uri("/usage/stats?group_by=status_code")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body_bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let res: UsageStatsResponse = serde_json::from_slice(&body_bytes).unwrap();
        assert_eq!(res.group_by, "status_code");
        assert!(res.breakdown.iter().any(|b| b.key == "200"));
    }

    #[tokio::test]
    async fn test_usage_stats_endpoint_invalid_parameters() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        // 1. Invalid date ordering: start_date > end_date
        let req1 = Request::builder()
            .uri("/usage/stats?start_date=2026-08-30&end_date=2026-08-01")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp1 = app.clone().oneshot(req1).await.unwrap();
        assert_eq!(resp1.status(), StatusCode::BAD_REQUEST);

        // 2. Invalid date format
        let req2 = Request::builder()
            .uri("/usage/stats?start_date=2026/08/01")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp2 = app.clone().oneshot(req2).await.unwrap();
        assert_eq!(resp2.status(), StatusCode::BAD_REQUEST);

        // 3. Invalid group_by dimension
        let req3 = Request::builder()
            .uri("/usage/stats?group_by=user_agent")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp3 = app.clone().oneshot(req3).await.unwrap();
        assert_eq!(resp3.status(), StatusCode::BAD_REQUEST);

        // 4. Limit = 0 or limit > 1000
        let req4 = Request::builder()
            .uri("/usage/stats?limit=0")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();
        let resp4 = app.oneshot(req4).await.unwrap();
        assert_eq!(resp4.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_usage_stats_endpoint_unauthenticated_rejected() {
        let app = create_app();

        let req = Request::builder()
            .uri("/usage/stats")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_event_study_endpoint_authenticated_success() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .uri("/events/study?ticker=AAPL&event_date=2025-06-15&event_window=5&estimation_window=60&benchmark_ticker=SPY")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_event_study_endpoint_invalid_parameters() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        // 1. Missing ticker
        let req1 = Request::builder()
            .uri("/events/study?event_date=2025-06-15")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp1 = app.clone().oneshot(req1).await.unwrap();
        assert_eq!(resp1.status(), StatusCode::NOT_FOUND);

        // 2. Invalid date format
        let req2 = Request::builder()
            .uri("/events/study?ticker=AAPL&event_date=15-06-2025")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp2 = app.clone().oneshot(req2).await.unwrap();
        assert_eq!(resp2.status(), StatusCode::NOT_FOUND);

        // 3. Event window out of bounds (> 20)
        let req3 = Request::builder()
            .uri("/events/study?ticker=AAPL&event_date=2025-06-15&event_window=30")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp3 = app.clone().oneshot(req3).await.unwrap();
        assert_eq!(resp3.status(), StatusCode::NOT_FOUND);

        // 4. Estimation window out of bounds (< 10)
        let req4 = Request::builder()
            .uri("/events/study?ticker=AAPL&event_date=2025-06-15&estimation_window=5")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();
        let resp4 = app.oneshot(req4).await.unwrap();
        assert_eq!(resp4.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_event_study_endpoint_unauthenticated_rejected() {
        let app = create_app();

        let req = Request::builder()
            .uri("/events/study?ticker=AAPL&event_date=2025-06-15")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_8k_events_endpoint_authenticated_success() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        // 1. All tickers, default params
        let req1 = Request::builder()
            .uri("/events/8k")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp1 = app.clone().oneshot(req1).await.unwrap();
        assert_eq!(resp1.status(), StatusCode::OK);

        let body1 = resp1.into_body().collect().await.unwrap().to_bytes();
        let json1: EightKResponse = serde_json::from_slice(&body1).unwrap();
        assert_eq!(json1.ticker, "ALL");
        assert_eq!(json1.days, 7);
        assert!(json1.count > 0);
        assert!(!json1.filings.is_empty());

        // 2. Specific ticker filter
        let req2 = Request::builder()
            .uri("/events/8k?ticker=AAPL&days=10")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp2 = app.clone().oneshot(req2).await.unwrap();
        assert_eq!(resp2.status(), StatusCode::OK);

        let body2 = resp2.into_body().collect().await.unwrap().to_bytes();
        let json2: EightKResponse = serde_json::from_slice(&body2).unwrap();
        assert_eq!(json2.ticker, "AAPL");
        assert_eq!(json2.days, 10);
        for f in &json2.filings {
            assert_eq!(f.ticker, "AAPL");
            assert_eq!(f.form_type, "8-K");
            assert!(!f.items.is_empty());
        }

        // 3. Event type filter (e.g. M&A)
        let req3 = Request::builder()
            .uri("/events/8k?event_type=M%26A&days=15")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();
        let resp3 = app.oneshot(req3).await.unwrap();
        assert_eq!(resp3.status(), StatusCode::OK);

        let body3 = resp3.into_body().collect().await.unwrap().to_bytes();
        let json3: EightKResponse = serde_json::from_slice(&body3).unwrap();
        for f in &json3.filings {
            assert_eq!(f.event_type, "M&A");
        }
    }

    #[tokio::test]
    async fn test_8k_events_endpoint_validation_errors() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        // 1. Days out of bounds (> 30)
        let req1 = Request::builder()
            .uri("/events/8k?days=35")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp1 = app.clone().oneshot(req1).await.unwrap();
        assert_eq!(resp1.status(), StatusCode::BAD_REQUEST);

        // 2. Limit out of bounds (0)
        let req2 = Request::builder()
            .uri("/events/8k?limit=0")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp2 = app.clone().oneshot(req2).await.unwrap();
        assert_eq!(resp2.status(), StatusCode::BAD_REQUEST);

        // 3. Empty ticker parameter
        let req3 = Request::builder()
            .uri("/events/8k?ticker=%20%20")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();
        let resp3 = app.oneshot(req3).await.unwrap();
        assert_eq!(resp3.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_8k_events_endpoint_delisted_ticker_rejected() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        // TWTR was delisted on 2022-10-28
        let req = Request::builder()
            .uri("/events/8k?ticker=TWTR&days=7")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let json_err: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(json_err["message"]
            .as_str()
            .unwrap()
            .contains("Point-in-Time Violation"));
    }

    #[tokio::test]
    async fn test_8k_events_endpoint_unauthenticated_rejected() {
        let app = create_app();

        let req = Request::builder()
            .uri("/events/8k")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_supply_chain_risk_endpoint_success() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .uri("/events/supply-chain-risk?ticker=AAPL&depth=1&min_risk_score=0.5")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let json_resp: SupplyChainRiskResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(json_resp.focal_ticker, "AAPL");
        assert_eq!(json_resp.depth, 1);
        assert!(!json_resp.alerts.is_empty());
        for alert in &json_resp.alerts {
            assert_eq!(alert.focal_ticker, "AAPL");
            assert_eq!(alert.depth, 1);
            assert!(alert.risk_score >= 0.5);
            assert!(!alert.related_ticker.is_empty());
            assert!(!alert.event_type.is_empty());
        }
    }

    #[tokio::test]
    async fn test_supply_chain_risk_endpoint_multi_tier_depth_2() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .uri("/events/supply-chain-risk?ticker=AAPL&depth=2&min_risk_score=0.3&limit=20")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let json_resp: SupplyChainRiskResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(json_resp.focal_ticker, "AAPL");
        assert_eq!(json_resp.depth, 2);
        assert!(!json_resp.alerts.is_empty());
        let has_tier2 = json_resp.alerts.iter().any(|a| a.depth == 2);
        assert!(has_tier2, "Should return tier-2 alerts (e.g. ASML via TSM)");
    }

    #[tokio::test]
    async fn test_supply_chain_risk_endpoint_validation_errors() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        // 1. Missing ticker
        let req1 = Request::builder()
            .uri("/events/supply-chain-risk?depth=1")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp1 = app.clone().oneshot(req1).await.unwrap();
        assert_eq!(resp1.status(), StatusCode::BAD_REQUEST);

        // 2. Depth out of bounds (0 or 4)
        let req2 = Request::builder()
            .uri("/events/supply-chain-risk?ticker=AAPL&depth=0")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp2 = app.clone().oneshot(req2).await.unwrap();
        assert_eq!(resp2.status(), StatusCode::BAD_REQUEST);

        let req2b = Request::builder()
            .uri("/events/supply-chain-risk?ticker=AAPL&depth=4")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp2b = app.clone().oneshot(req2b).await.unwrap();
        assert_eq!(resp2b.status(), StatusCode::BAD_REQUEST);

        // 3. Min risk score out of bounds
        let req3 = Request::builder()
            .uri("/events/supply-chain-risk?ticker=AAPL&min_risk_score=1.5")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp3 = app.clone().oneshot(req3).await.unwrap();
        assert_eq!(resp3.status(), StatusCode::BAD_REQUEST);

        // 4. Limit out of bounds
        let req4 = Request::builder()
            .uri("/events/supply-chain-risk?ticker=AAPL&limit=0")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp4 = app.clone().oneshot(req4).await.unwrap();
        assert_eq!(resp4.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_supply_chain_risk_endpoint_delisted_ticker_rejected() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .uri("/events/supply-chain-risk?ticker=TWTR&depth=1")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let json_err: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(json_err["message"]
            .as_str()
            .unwrap()
            .contains("Point-in-Time Violation"));
    }

    #[tokio::test]
    async fn test_supply_chain_risk_endpoint_unauthenticated_rejected() {
        let app = create_app();

        let req = Request::builder()
            .uri("/events/supply-chain-risk?ticker=AAPL")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_sentiment_feed_endpoint_success() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .uri("/sentiment/feed?sector=Technology&min_confidence=0.5&min_quality=0.5&limit=20&sort=desc")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let json_resp: SentimentFeedResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(json_resp.sector, Some("Technology".to_string()));
        assert_eq!(json_resp.limit, 20);
        assert_eq!(json_resp.sort, "desc");
        assert_eq!(json_resp.count, json_resp.records.len());
        for item in &json_resp.records {
            assert!(item.confidence >= 0.5);
            assert!(item.data_quality_score >= 0.5);
            assert!(!item.ticker.is_empty());
        }
    }

    #[tokio::test]
    async fn test_sentiment_feed_endpoint_defaults() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .uri("/sentiment/feed")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let json_resp: SentimentFeedResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(json_resp.limit, 100);
        assert_eq!(json_resp.offset, 0);
        assert_eq!(json_resp.sort, "desc");
        assert_eq!(json_resp.count, json_resp.records.len());
    }

    #[tokio::test]
    async fn test_sentiment_feed_endpoint_validation_errors() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        // 1. Invalid sort order
        let req1 = Request::builder()
            .uri("/sentiment/feed?sort=invalid_order")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp1 = app.clone().oneshot(req1).await.unwrap();
        assert_eq!(resp1.status(), StatusCode::BAD_REQUEST);

        // 2. Invalid min_confidence (> 1.0)
        let req2 = Request::builder()
            .uri("/sentiment/feed?min_confidence=1.5")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp2 = app.clone().oneshot(req2).await.unwrap();
        assert_eq!(resp2.status(), StatusCode::BAD_REQUEST);

        // 3. Invalid min_quality (< 0.0)
        let req3 = Request::builder()
            .uri("/sentiment/feed?min_quality=-0.2")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp3 = app.clone().oneshot(req3).await.unwrap();
        assert_eq!(resp3.status(), StatusCode::BAD_REQUEST);

        // 4. Invalid start_date after end_date
        let req4 = Request::builder()
            .uri("/sentiment/feed?start_date=2026-08-30&end_date=2026-08-01")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp4 = app.clone().oneshot(req4).await.unwrap();
        assert_eq!(resp4.status(), StatusCode::BAD_REQUEST);

        // 5. Unknown sector
        let req5 = Request::builder()
            .uri("/sentiment/feed?sector=NonExistentSector12345")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp5 = app.clone().oneshot(req5).await.unwrap();
        assert_eq!(resp5.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_sentiment_feed_endpoint_unauthenticated_rejected() {
        let app = create_app();

        let req = Request::builder()
            .uri("/sentiment/feed")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_sentiment_feed_endpoint_cursor_pagination_desc() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        // Page 1: limit=5, sort=desc
        let req1 = Request::builder()
            .uri("/sentiment/feed?limit=5&sort=desc")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp1 = app.clone().oneshot(req1).await.unwrap();
        assert_eq!(resp1.status(), StatusCode::OK);

        let body1 = resp1.into_body().collect().await.unwrap().to_bytes();
        let json_p1: SentimentFeedResponse = serde_json::from_slice(&body1).unwrap();
        assert_eq!(json_p1.limit, 5);
        assert_eq!(json_p1.sort, "desc");
        assert_eq!(json_p1.count, 5);
        assert!(json_p1.next_cursor.is_some());
        let cur = json_p1.next_cursor.unwrap();
        assert_eq!(cur, json_p1.records.last().unwrap().published_utc);

        // Page 2: query using cursor
        let req2 = Request::builder()
            .uri(format!("/sentiment/feed?limit=5&sort=desc&cursor={}", cur))
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();
        let resp2 = app.oneshot(req2).await.unwrap();
        assert_eq!(resp2.status(), StatusCode::OK);

        let body2 = resp2.into_body().collect().await.unwrap().to_bytes();
        let json_p2: SentimentFeedResponse = serde_json::from_slice(&body2).unwrap();
        assert!(!json_p2.records.is_empty());

        let p1_titles: std::collections::HashSet<_> =
            json_p1.records.iter().map(|r| &r.title).collect();
        for r in &json_p2.records {
            assert!(
                !p1_titles.contains(&r.title),
                "Page 2 should not overlap with Page 1"
            );
            assert!(
                r.published_utc < cur,
                "Page 2 records must be earlier than cursor"
            );
        }
    }

    #[tokio::test]
    async fn test_sentiment_feed_endpoint_cursor_pagination_asc() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        // Page 1: limit=5, sort=asc
        let req1 = Request::builder()
            .uri("/sentiment/feed?limit=5&sort=asc")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp1 = app.clone().oneshot(req1).await.unwrap();
        assert_eq!(resp1.status(), StatusCode::OK);

        let body1 = resp1.into_body().collect().await.unwrap().to_bytes();
        let json_p1: SentimentFeedResponse = serde_json::from_slice(&body1).unwrap();
        assert_eq!(json_p1.sort, "asc");
        assert!(json_p1.next_cursor.is_some());
        let cur = json_p1.next_cursor.unwrap();

        // Page 2: query using cursor
        let req2 = Request::builder()
            .uri(format!("/sentiment/feed?limit=5&sort=asc&cursor={}", cur))
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();
        let resp2 = app.oneshot(req2).await.unwrap();
        assert_eq!(resp2.status(), StatusCode::OK);

        let body2 = resp2.into_body().collect().await.unwrap().to_bytes();
        let json_p2: SentimentFeedResponse = serde_json::from_slice(&body2).unwrap();
        assert!(!json_p2.records.is_empty());
        for r in &json_p2.records {
            assert!(
                r.published_utc > cur,
                "Page 2 records must be later than cursor"
            );
        }
    }

    #[tokio::test]
    async fn test_sentiment_feed_endpoint_cursor_offset_conflict() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .uri("/sentiment/feed?cursor=2026-08-29T14:30:00Z&offset=10")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_sentiment_feed_endpoint_invalid_cursor() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        // Malformed timestamp
        let req1 = Request::builder()
            .uri("/sentiment/feed?cursor=invalid-cursor-timestamp")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp1 = app.clone().oneshot(req1).await.unwrap();
        assert_eq!(resp1.status(), StatusCode::BAD_REQUEST);

        // Empty cursor
        let req2 = Request::builder()
            .uri("/sentiment/feed?cursor=")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();
        let resp2 = app.oneshot(req2).await.unwrap();
        assert_eq!(resp2.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_sentiment_anomalies_endpoint_success() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .uri("/sentiment/anomalies?sector=Technology&lookback_days=30&zscore_threshold=2.0&min_records=20&limit=10")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let json_resp: SentimentAnomaliesResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(json_resp.lookback_days, 30);
        assert_eq!(json_resp.zscore_threshold, 2.0);
        assert_eq!(json_resp.min_records, 20);
        assert_eq!(json_resp.count, json_resp.items.len());
        for item in &json_resp.items {
            assert!(item.zscore.abs() >= 2.0);
            assert!(item.direction == "bullish" || item.direction == "bearish");
            assert!(!item.ticker.is_empty());
        }
    }

    #[tokio::test]
    async fn test_sentiment_anomalies_endpoint_defaults() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .uri("/sentiment/anomalies")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let json_resp: SentimentAnomaliesResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(json_resp.lookback_days, 30);
        assert_eq!(json_resp.zscore_threshold, 2.0);
        assert_eq!(json_resp.min_records, 20);
        assert_eq!(json_resp.count, json_resp.items.len());
    }

    #[tokio::test]
    async fn test_sentiment_anomalies_endpoint_validation_errors() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        // 1. Invalid lookback_days (> 90)
        let req1 = Request::builder()
            .uri("/sentiment/anomalies?lookback_days=100")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp1 = app.clone().oneshot(req1).await.unwrap();
        assert_eq!(resp1.status(), StatusCode::BAD_REQUEST);

        // 2. Invalid zscore_threshold (< 1.0)
        let req2 = Request::builder()
            .uri("/sentiment/anomalies?zscore_threshold=0.5")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp2 = app.clone().oneshot(req2).await.unwrap();
        assert_eq!(resp2.status(), StatusCode::BAD_REQUEST);

        // 3. Invalid limit (> 100)
        let req3 = Request::builder()
            .uri("/sentiment/anomalies?limit=150")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp3 = app.clone().oneshot(req3).await.unwrap();
        assert_eq!(resp3.status(), StatusCode::BAD_REQUEST);

        // 4. Unknown sector
        let req4 = Request::builder()
            .uri("/sentiment/anomalies?sector=NonExistentSector999")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp4 = app.clone().oneshot(req4).await.unwrap();
        assert_eq!(resp4.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_sentiment_anomalies_endpoint_unauthenticated_rejected() {
        let app = create_app();

        let req = Request::builder()
            .uri("/sentiment/anomalies")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_audio_transcribe_valid_wav() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .method("POST")
            .uri("/audio/transcribe")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::GONE);
        assert_eq!(
            resp.headers().get("sunset").and_then(|h| h.to_str().ok()),
            Some("Wed, 11 Nov 2026 00:00:00 GMT")
        );
    }

    #[tokio::test]
    async fn test_audio_transcribe_invalid_format_rejected() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .method("POST")
            .uri("/audio/transcribe")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::GONE);
        assert_eq!(
            resp.headers().get("sunset").and_then(|h| h.to_str().ok()),
            Some("Wed, 11 Nov 2026 00:00:00 GMT")
        );
    }

    #[tokio::test]
    async fn test_audio_transcribe_missing_audio_field_rejected() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .method("POST")
            .uri("/audio/transcribe")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::GONE);
        assert_eq!(
            resp.headers().get("sunset").and_then(|h| h.to_str().ok()),
            Some("Wed, 11 Nov 2026 00:00:00 GMT")
        );
    }

    #[tokio::test]
    async fn test_audio_transcribe_unauthenticated_rejected() {
        let app = create_app();

        let req = Request::builder()
            .method("POST")
            .uri("/audio/transcribe")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::GONE);
        assert_eq!(
            resp.headers().get("sunset").and_then(|h| h.to_str().ok()),
            Some("Wed, 11 Nov 2026 00:00:00 GMT")
        );
    }

    #[tokio::test]
    async fn test_transcript_create_and_get_lifecycle() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        // 1. Create transcript
        let create_req = serde_json::json!({
            "ticker": "AAPL",
            "quarter": 4,
            "year": 2024,
            "call_date": "2024-10-31",
            "transcript_text": "Apple Inc. reported record revenue of $94.9 billion for the fourth fiscal quarter.",
            "source": "manual"
        });

        let req = Request::builder()
            .method("POST")
            .uri("/transcripts")
            .header(auth_k.clone(), auth_v.clone())
            .header("Content-Type", "application/json")
            .body(Body::from(serde_json::to_vec(&create_req).unwrap()))
            .unwrap();

        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);

        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let created: TranscriptResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(created.ticker, "AAPL");
        assert_eq!(created.quarter, Some(4));
        assert_eq!(created.year, Some(2024));
        assert_eq!(created.word_count, 13);
        assert_eq!(created.sentiment_label.as_deref(), Some("BULLISH"));

        // 2. Get single transcript by ID
        let req_get = Request::builder()
            .uri(format!("/transcripts/{}", created.id))
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();

        let resp_get = app.clone().oneshot(req_get).await.unwrap();
        assert_eq!(resp_get.status(), StatusCode::OK);

        let body_get = resp_get.into_body().collect().await.unwrap().to_bytes();
        let fetched: TranscriptResponse = serde_json::from_slice(&body_get).unwrap();
        assert_eq!(fetched.id, created.id);
        assert_eq!(fetched.transcript_text, created.transcript_text);
    }

    #[tokio::test]
    async fn test_transcript_list_filtering_and_pagination() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        // Seed 2 transcripts
        for (tkr, q, d) in [("NVDA", 3, "2024-11-20"), ("MSFT", 1, "2024-10-25")] {
            let create_req = serde_json::json!({
                "ticker": tkr,
                "quarter": q,
                "year": 2024,
                "call_date": d,
                "transcript_text": format!("{} financial results exceeded expectations with strong growth.", tkr),
                "source": "manual"
            });

            let req = Request::builder()
                .method("POST")
                .uri("/transcripts")
                .header(auth_k.clone(), auth_v.clone())
                .header("Content-Type", "application/json")
                .body(Body::from(serde_json::to_vec(&create_req).unwrap()))
                .unwrap();

            let resp = app.clone().oneshot(req).await.unwrap();
            assert_eq!(resp.status(), StatusCode::CREATED);
        }

        // List NVDA
        let req_list = Request::builder()
            .uri("/transcripts?ticker=NVDA")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();

        let resp_list = app.clone().oneshot(req_list).await.unwrap();
        assert_eq!(resp_list.status(), StatusCode::OK);

        let body_list = resp_list.into_body().collect().await.unwrap().to_bytes();
        let list_resp: TranscriptListResponse = serde_json::from_slice(&body_list).unwrap();
        assert_eq!(list_resp.items.len(), 1);
        assert_eq!(list_resp.items[0].ticker, "NVDA");
    }

    #[tokio::test]
    async fn test_transcript_delete_lifecycle() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        // Create
        let create_req = serde_json::json!({
            "ticker": "TSLA",
            "transcript_text": "Tesla delivery volume increased with energy storage momentum.",
            "source": "manual"
        });

        let req = Request::builder()
            .method("POST")
            .uri("/transcripts")
            .header(auth_k.clone(), auth_v.clone())
            .header("Content-Type", "application/json")
            .body(Body::from(serde_json::to_vec(&create_req).unwrap()))
            .unwrap();

        let resp = app.clone().oneshot(req).await.unwrap();
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let created: TranscriptResponse = serde_json::from_slice(&body).unwrap();

        // Delete
        let req_del = Request::builder()
            .method("DELETE")
            .uri(format!("/transcripts/{}", created.id))
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();

        let resp_del = app.clone().oneshot(req_del).await.unwrap();
        assert_eq!(resp_del.status(), StatusCode::OK);

        // Verify deleted
        let req_get = Request::builder()
            .uri(format!("/transcripts/{}", created.id))
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();

        let resp_get = app.clone().oneshot(req_get).await.unwrap();
        assert_eq!(resp_get.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_audio_transcribe_with_store_param() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .method("POST")
            .uri("/audio/transcribe")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();

        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::GONE);
        assert_eq!(
            resp.headers().get("sunset").and_then(|h| h.to_str().ok()),
            Some("Wed, 11 Nov 2026 00:00:00 GMT")
        );
    }

    #[tokio::test]
    async fn test_market_regime_default_query() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .uri("/market/regime?lookback_days=5&min_data_points=50")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();

        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_market_regime_custom_weights() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .uri("/market/regime?lookback_days=7&sector_weights=Technology:0.5,Financials:0.3,Healthcare:0.2")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();

        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_market_regime_bounds_validation() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        // lookback_days > 30
        let req_bad_lookback = Request::builder()
            .uri("/market/regime?lookback_days=45")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req_bad_lookback).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);

        // min_data_points == 0
        let req_bad_pts = Request::builder()
            .uri("/market/regime?min_data_points=0")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req_bad_pts).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_market_regime_unauthenticated() {
        let app = create_app();
        let req = Request::builder()
            .uri("/market/regime")
            .body(Body::empty())
            .unwrap();

        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_return_correlation_valid_matrix() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .uri("/market/correlation?tickers=AAPL,MSFT,NVDA&start_date=2025-01-01&end_date=2025-03-31&min_periods=15")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();

        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_return_correlation_include_self() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .uri("/market/correlation?tickers=AAPL,NVDA&start_date=2025-01-01&end_date=2025-03-31&include_self=true")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();

        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_return_correlation_validation_errors() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        // Empty tickers
        let req_empty = Request::builder()
            .uri("/market/correlation?tickers=&start_date=2025-01-01&end_date=2025-03-31")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req_empty).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);

        // start_date > end_date
        let req_inv_dates = Request::builder()
            .uri("/market/correlation?tickers=AAPL,MSFT&start_date=2025-05-01&end_date=2025-01-01")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req_inv_dates).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);

        // min_periods < 10
        let req_bad_min = Request::builder()
            .uri("/market/correlation?tickers=AAPL,MSFT&start_date=2025-01-01&end_date=2025-03-31&min_periods=5")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req_bad_min).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_return_correlation_unauthenticated() {
        let app = create_app();
        let req = Request::builder()
            .uri("/market/correlation?tickers=AAPL,MSFT&start_date=2025-01-01&end_date=2025-03-31")
            .body(Body::empty())
            .unwrap();

        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_put_call_ratio_single_ticker_daily() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .uri("/options/put-call-ratio?ticker=AAPL&start_date=2025-01-01&end_date=2025-01-10&ratio_type=volume&granularity=daily")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();

        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let pcr_resp: PutCallRatioResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(pcr_resp.ticker, Some("AAPL".to_string()));
        assert_eq!(pcr_resp.ratio_type, "volume");
        assert_eq!(pcr_resp.granularity, "daily");
        assert!(pcr_resp.points.is_some());
        let pts = pcr_resp.points.unwrap();
        assert!(!pts.is_empty());
        assert!(pcr_resp.average_ratio.is_some());
        let avg = pcr_resp.average_ratio.unwrap();
        assert!(avg > 0.0 && avg < 5.0);
    }

    #[tokio::test]
    async fn test_put_call_ratio_market_wide_total() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .uri("/options/put-call-ratio?start_date=2025-01-01&end_date=2025-01-10&granularity=total")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();

        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let pcr_resp: PutCallRatioResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(pcr_resp.ticker, None);
        assert_eq!(pcr_resp.granularity, "total");
        assert!(pcr_resp.points.is_none());
        assert!(pcr_resp.total_call_volume.unwrap() > 0);
        assert!(pcr_resp.total_put_volume.unwrap() > 0);
        assert!(pcr_resp.total_ratio.is_some());
    }

    #[tokio::test]
    async fn test_put_call_ratio_open_interest() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .uri("/options/put-call-ratio?ticker=AAPL&start_date=2025-01-01&end_date=2025-01-10&ratio_type=open_interest&granularity=daily")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();

        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let pcr_resp: PutCallRatioResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(pcr_resp.ratio_type, "open_interest");
        assert!(pcr_resp.average_ratio.is_some());
    }

    #[tokio::test]
    async fn test_put_call_ratio_validation_errors() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        // Missing start_date
        let req_no_start = Request::builder()
            .uri("/options/put-call-ratio?start_date=&end_date=2025-01-10")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req_no_start).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        // start_date > end_date
        let req_inv_dates = Request::builder()
            .uri("/options/put-call-ratio?start_date=2025-05-01&end_date=2025-01-01")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req_inv_dates).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        // Invalid ratio_type
        let req_bad_type = Request::builder()
            .uri("/options/put-call-ratio?start_date=2025-01-01&end_date=2025-01-10&ratio_type=invalid_type")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req_bad_type).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        // Invalid granularity
        let req_bad_gran = Request::builder()
            .uri("/options/put-call-ratio?start_date=2025-01-01&end_date=2025-01-10&granularity=yearly")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req_bad_gran).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_put_call_ratio_unauthenticated() {
        let app = create_app();
        let req = Request::builder()
            .uri("/options/put-call-ratio?start_date=2025-01-01&end_date=2025-01-10")
            .body(Body::empty())
            .unwrap();

        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_earnings_surprise_single_ticker() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .uri("/events/earnings-surprise?ticker=AAPL&start_date=2025-01-01&end_date=2025-12-31&min_sentiment_shift=0.10")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();

        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let surprise_resp: EarningsSurpriseResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(surprise_resp.ticker.as_deref(), Some("AAPL"));
        assert_eq!(surprise_resp.start_date, "2025-01-01");
        assert_eq!(surprise_resp.end_date, "2025-12-31");
    }

    #[tokio::test]
    async fn test_earnings_surprise_universe_scan() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .uri("/events/earnings-surprise?start_date=2025-01-01&end_date=2025-12-31&min_sentiment_shift=0.10&limit=10")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();

        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let surprise_resp: EarningsSurpriseResponse = serde_json::from_slice(&body).unwrap();
        assert!(surprise_resp.ticker.is_none());
        assert!(surprise_resp.count <= 10);
        for item in &surprise_resp.surprises {
            assert!(item.surprise_score.abs() >= 0.10);
            assert!(item.direction == "positive" || item.direction == "negative");
        }
    }

    #[tokio::test]
    async fn test_earnings_surprise_validation_errors() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        // Invalid start > end
        let req_dates = Request::builder()
            .uri("/events/earnings-surprise?start_date=2025-12-31&end_date=2025-01-01")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req_dates).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        // Invalid min_sentiment_shift out of bounds
        let req_shift = Request::builder()
            .uri("/events/earnings-surprise?min_sentiment_shift=0.99")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req_shift).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        // Invalid pre_days out of bounds
        let req_pre = Request::builder()
            .uri("/events/earnings-surprise?pre_days=50")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req_pre).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_earnings_surprise_unauthenticated() {
        let app = create_app();
        let req = Request::builder()
            .uri("/events/earnings-surprise")
            .body(Body::empty())
            .unwrap();

        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_insider_trading_single_ticker() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .uri("/events/insider-trading?ticker=AAPL&start_date=2025-01-01&end_date=2025-03-31")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();

        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let insider_resp: InsiderTradingResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(insider_resp.ticker.as_deref(), Some("AAPL"));
        assert!(!insider_resp.trades.is_empty());
        for trade in &insider_resp.trades {
            assert_eq!(trade.ticker, "AAPL");
            assert_eq!(trade.source, "SEC Form 4");
            assert!(trade.signal_score >= -1.0 && trade.signal_score <= 1.0);
        }
    }

    #[tokio::test]
    async fn test_insider_trading_universe_scan() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .uri("/events/insider-trading?start_date=2025-01-01&end_date=2025-03-31&limit=25")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();

        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let insider_resp: InsiderTradingResponse = serde_json::from_slice(&body).unwrap();
        assert!(insider_resp.ticker.is_none());
        assert!(insider_resp.count <= 25);
    }

    #[tokio::test]
    async fn test_insider_trading_type_filter() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .uri("/events/insider-trading?transaction_type=purchase&start_date=2025-01-01&end_date=2025-03-31")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();

        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let insider_resp: InsiderTradingResponse = serde_json::from_slice(&body).unwrap();
        for trade in &insider_resp.trades {
            assert_eq!(trade.transaction_type, "purchase");
            assert!(trade.signal_score > 0.0);
        }
    }

    #[tokio::test]
    async fn test_insider_trading_validation_errors() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        // Invalid transaction_type
        let req_type = Request::builder()
            .uri("/events/insider-trading?transaction_type=invalid_type")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req_type).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        // Invalid start > end
        let req_dates = Request::builder()
            .uri("/events/insider-trading?start_date=2025-12-31&end_date=2025-01-01")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req_dates).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        // Invalid min_signal_score out of bounds
        let req_score = Request::builder()
            .uri("/events/insider-trading?min_signal_score=1.5")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req_score).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_insider_trading_unauthenticated() {
        let app = create_app();
        let req = Request::builder()
            .uri("/events/insider-trading")
            .body(Body::empty())
            .unwrap();

        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_sentiment_disagreement_stddev() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .uri("/sentiment/disagreement?ticker=AAPL&start_date=2025-01-01&end_date=2025-03-31&aggregation=stddev")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();

        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let disag_resp: SentimentDisagreementResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(disag_resp.ticker, "AAPL");
        assert_eq!(disag_resp.aggregation, "stddev");
        assert!(disag_resp.disagreement_index >= 0.0);
        assert!(disag_resp.record_count >= 10);
        assert!(disag_resp.source_count > 0);
        assert!(!disag_resp.sources_breakdown.is_empty());
    }

    #[tokio::test]
    async fn test_sentiment_disagreement_iqr_and_mad() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        // IQR
        let req_iqr = Request::builder()
            .uri("/sentiment/disagreement?ticker=NVDA&start_date=2025-01-01&end_date=2025-03-31&aggregation=iqr")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp_iqr = app.clone().oneshot(req_iqr).await.unwrap();
        assert_eq!(resp_iqr.status(), StatusCode::OK);

        // MAD
        let req_mad = Request::builder()
            .uri("/sentiment/disagreement?ticker=MSFT&start_date=2025-01-01&end_date=2025-03-31&aggregation=mad")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();
        let resp_mad = app.clone().oneshot(req_mad).await.unwrap();
        assert_eq!(resp_mad.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_sentiment_disagreement_validation_errors() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        // Invalid aggregation
        let req_agg = Request::builder()
            .uri("/sentiment/disagreement?ticker=AAPL&start_date=2025-01-01&end_date=2025-03-31&aggregation=invalid_agg")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req_agg).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        // min_records < 5
        let req_rec = Request::builder()
            .uri("/sentiment/disagreement?ticker=AAPL&start_date=2025-01-01&end_date=2025-03-31&min_records=2")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req_rec).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        // start > end
        let req_dates = Request::builder()
            .uri("/sentiment/disagreement?ticker=AAPL&start_date=2025-12-31&end_date=2025-01-01")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req_dates).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_sentiment_disagreement_unauthenticated() {
        let app = create_app();
        let req = Request::builder()
            .uri("/sentiment/disagreement?ticker=AAPL&start_date=2025-01-01&end_date=2025-03-31")
            .body(Body::empty())
            .unwrap();

        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_options_vol_surface_default_success() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .uri("/options/vol-surface?ticker=AAPL")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();

        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let surface_resp: OptionsVolSurfaceResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(surface_resp.ticker, "AAPL");
        assert!(surface_resp.spot > 0.0);
        assert_eq!(surface_resp.strikes.len(), 9);
        assert!(!surface_resp.expirations.is_empty());
        assert_eq!(surface_resp.surface.len(), surface_resp.expirations.len());
        for row in &surface_resp.surface {
            assert_eq!(row.ivs.len(), 9);
            for &iv in &row.ivs {
                assert!(iv >= 0.05 && iv <= 2.0);
            }
        }
    }

    #[tokio::test]
    async fn test_options_vol_surface_custom_strikes_and_dates() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .uri("/options/vol-surface?ticker=NVDA&strike_range=0.85-1.15&strike_count=5&start_date=2025-01-01&end_date=2025-06-30")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();

        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let surface_resp: OptionsVolSurfaceResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(surface_resp.ticker, "NVDA");
        assert_eq!(surface_resp.strikes.len(), 5);
    }

    #[tokio::test]
    async fn test_options_vol_surface_validation_errors() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        // Even strike_count
        let req_even = Request::builder()
            .uri("/options/vol-surface?ticker=AAPL&strike_count=8")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req_even).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        // strike_count > 15
        let req_large = Request::builder()
            .uri("/options/vol-surface?ticker=AAPL&strike_count=17")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req_large).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        // Invalid strike_range format
        let req_range = Request::builder()
            .uri("/options/vol-surface?ticker=AAPL&strike_range=invalid_range")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req_range).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        // start_date > end_date
        let req_dates = Request::builder()
            .uri("/options/vol-surface?ticker=AAPL&start_date=2025-12-31&end_date=2025-01-01")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req_dates).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_options_vol_surface_unauthenticated() {
        let app = create_app();
        let req = Request::builder()
            .uri("/options/vol-surface?ticker=AAPL")
            .body(Body::empty())
            .unwrap();

        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_ma_rumors_single_ticker() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .uri("/events/ma-rumors?ticker=NVDA&min_rumor_score=0.4")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();

        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_ma_rumors_universe_scan() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .uri("/events/ma-rumors?min_rumor_score=0.3&limit=5")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();

        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_ma_rumors_validation_errors() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        // min_rumor_score > 1.0
        let req_score = Request::builder()
            .uri("/events/ma-rumors?min_rumor_score=1.5")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req_score).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);

        // lookback_days > 30
        let req_days = Request::builder()
            .uri("/events/ma-rumors?lookback_days=45")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req_days).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_ma_rumors_unauthenticated() {
        let app = create_app();
        let req = Request::builder()
            .uri("/events/ma-rumors")
            .body(Body::empty())
            .unwrap();

        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_regulatory_filings_single_ticker() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .uri("/events/filings?ticker=AAPL")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();

        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_regulatory_filings_filters_and_pagination() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .uri("/events/filings?form_type=10-K&limit=5&offset=0")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();

        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_regulatory_filings_validation_errors() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        // start_date > end_date
        let req_dates = Request::builder()
            .uri("/events/filings?start_date=2025-12-31&end_date=2025-01-01")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req_dates).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);

        // limit > 100
        let req_lim = Request::builder()
            .uri("/events/filings?limit=200")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req_lim).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_regulatory_filings_unauthenticated() {
        let app = create_app();
        let req = Request::builder()
            .uri("/events/filings")
            .body(Body::empty())
            .unwrap();

        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_export_parquet_valid_download() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .uri("/export/parquet?ticker=AAPL&start_date=2025-01-01&end_date=2025-01-10")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();

        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            resp.headers().get("content-type").unwrap(),
            "application/octet-stream"
        );

        let body = resp.into_body().collect().await.unwrap().to_bytes();
        assert!(body.len() > 12);
        assert_eq!(&body[0..4], b"PAR1");
        assert_eq!(&body[body.len() - 4..], b"PAR1");
    }

    #[tokio::test]
    async fn test_export_parquet_with_prices() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .uri("/export/parquet?ticker=NVDA&start_date=2025-01-01&end_date=2025-01-05&include_prices=true")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();

        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = resp.into_body().collect().await.unwrap().to_bytes();
        assert!(body.len() > 12);
        assert_eq!(&body[0..4], b"PAR1");
    }

    #[tokio::test]
    async fn test_export_parquet_validation_errors() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        // start_date > end_date
        let req_dates = Request::builder()
            .uri("/export/parquet?ticker=AAPL&start_date=2025-12-31&end_date=2025-01-01")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req_dates).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        // invalid min_quality
        let req_qual = Request::builder()
            .uri("/export/parquet?ticker=AAPL&start_date=2025-01-01&end_date=2025-01-10&min_quality=1.5")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req_qual).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_export_parquet_unauthenticated() {
        let app = create_app();
        let req = Request::builder()
            .uri("/export/parquet?ticker=AAPL&start_date=2025-01-01&end_date=2025-01-10")
            .body(Body::empty())
            .unwrap();

        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }
    // ─────────────────────────────────────────────────────────────────────────
    // Billing Endpoint Tests
    // ─────────────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_billing_checkout_valid_plan() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let body = serde_json::json!({
            "plan_id": "pro_monthly"
        });

        let req = Request::builder()
            .method("POST")
            .uri("/billing/checkout")
            .header(auth_k, auth_v)
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&body).unwrap()))
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
        assert!(json["checkout_url"]
            .as_str()
            .unwrap()
            .contains("stripe.com"));
        assert!(json["session_id"].as_str().unwrap().starts_with("cs_mock_"));
    }

    #[tokio::test]
    async fn test_billing_checkout_invalid_plan() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let body = serde_json::json!({
            "plan_id": "nonexistent_plan"
        });

        let req = Request::builder()
            .method("POST")
            .uri("/billing/checkout")
            .header(auth_k, auth_v)
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&body).unwrap()))
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_billing_checkout_free_plan_rejected() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let body = serde_json::json!({
            "plan_id": "free"
        });

        let req = Request::builder()
            .method("POST")
            .uri("/billing/checkout")
            .header(auth_k, auth_v)
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&body).unwrap()))
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_billing_checkout_unauthenticated() {
        let app = create_app();

        let body = serde_json::json!({
            "plan_id": "pro_monthly"
        });

        let req = Request::builder()
            .method("POST")
            .uri("/billing/checkout")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&body).unwrap()))
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_billing_portal_unauthenticated() {
        let app = create_app();

        let body = serde_json::json!({});

        let req = Request::builder()
            .method("POST")
            .uri("/billing/portal")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&body).unwrap()))
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_billing_portal_mock_mode() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let body = serde_json::json!({});

        let req = Request::builder()
            .method("POST")
            .uri("/billing/portal")
            .header(auth_k, auth_v)
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&body).unwrap()))
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        // In mock mode (no STRIPE_SECRET_KEY), returns 200 with mock portal URL
        assert_eq!(response.status(), StatusCode::OK);

        let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
        assert!(json["portal_url"].as_str().unwrap().contains("stripe.com"));
    }

    #[tokio::test]
    async fn test_billing_subscription_returns_free_default() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .uri("/billing/subscription")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
        assert_eq!(json["plan_id"], "free");
        assert_eq!(json["status"], "active");
        assert_eq!(json["monthly_request_limit"], 1000);
        assert_eq!(json["current_usage"], 0);
    }

    #[tokio::test]
    async fn test_billing_subscription_unauthenticated() {
        let app = create_app();

        let req = Request::builder()
            .uri("/billing/subscription")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_billing_webhook_valid_event() {
        let app = create_app();

        let event = serde_json::json!({
            "type": "checkout.session.completed",
            "data": {
                "object": {
                    "customer": "cus_test123",
                    "subscription": "sub_test456",
                    "metadata": {
                        "fintext_user_id": "user_abc",
                        "plan_id": "pro_monthly"
                    }
                }
            }
        });

        let req = Request::builder()
            .method("POST")
            .uri("/billing/webhook")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&event).unwrap()))
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
        assert_eq!(json["received"], true);
    }

    #[tokio::test]
    async fn test_billing_webhook_malformed_body() {
        let app = create_app();

        let req = Request::builder()
            .method("POST")
            .uri("/billing/webhook")
            .header("content-type", "application/json")
            .body(Body::from("not valid json"))
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_billing_webhook_does_not_require_jwt() {
        // Webhook should be accessible without JWT (it uses Stripe signature instead)
        let app = create_app();

        let event = serde_json::json!({
            "type": "invoice.payment_succeeded",
            "data": { "object": { "customer": "cus_test" } }
        });

        let req = Request::builder()
            .method("POST")
            .uri("/billing/webhook")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&event).unwrap()))
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        // Should return 200, not 401
        assert_eq!(response.status(), StatusCode::OK);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Organization & Multi-User Team Access Tests
    // ─────────────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_org_create_list_get_flow() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header_for_user("alice_founder");

        // 1. Create Organization
        let create_body = serde_json::json!({
            "name": "Alpha Capital Management"
        });

        let req = Request::builder()
            .method("POST")
            .uri("/orgs")
            .header(auth_k.clone(), auth_v.clone())
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&create_body).unwrap()))
            .unwrap();

        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);

        let body_bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let json_val: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
        let org_id_str = json_val["id"].as_str().unwrap();
        assert_eq!(json_val["name"], "Alpha Capital Management");
        assert_eq!(json_val["role"], "admin");

        // 2. List Organizations
        let list_req = Request::builder()
            .uri("/orgs")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();

        let list_resp = app.clone().oneshot(list_req).await.unwrap();
        assert_eq!(list_resp.status(), StatusCode::OK);

        let list_bytes = list_resp.into_body().collect().await.unwrap().to_bytes();
        let list_json: serde_json::Value = serde_json::from_slice(&list_bytes).unwrap();
        assert_eq!(list_json["count"], 1);
        assert_eq!(
            list_json["organizations"][0]["name"],
            "Alpha Capital Management"
        );
        assert_eq!(list_json["organizations"][0]["my_role"], "admin");

        // 3. Get Organization Details with Roster
        let get_req = Request::builder()
            .uri(format!("/orgs/{}", org_id_str))
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();

        let get_resp = app.clone().oneshot(get_req).await.unwrap();
        assert_eq!(get_resp.status(), StatusCode::OK);

        let get_bytes = get_resp.into_body().collect().await.unwrap().to_bytes();
        let get_json: serde_json::Value = serde_json::from_slice(&get_bytes).unwrap();
        assert_eq!(get_json["name"], "Alpha Capital Management");
        assert_eq!(get_json["members"].as_array().unwrap().len(), 1);
        assert_eq!(get_json["members"][0]["user_id"], "alice_founder");
        assert_eq!(get_json["members"][0]["role"], "admin");
    }

    #[tokio::test]
    async fn test_org_invite_and_rbac_permissions() {
        let app = create_app();
        let (admin_k, admin_v) = test_auth_header_for_user("org_admin_user");
        let (member_k, member_v) = test_auth_header_for_user("invited_member_user");

        // 1. Admin creates org
        let create_req = Request::builder()
            .method("POST")
            .uri("/orgs")
            .header(admin_k.clone(), admin_v.clone())
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::to_string(&serde_json::json!({ "name": "Quant Hedge Corp" })).unwrap(),
            ))
            .unwrap();
        let create_resp = app.clone().oneshot(create_req).await.unwrap();
        let org_id: String = serde_json::from_slice::<serde_json::Value>(
            &create_resp.into_body().collect().await.unwrap().to_bytes(),
        )
        .unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string();

        // 2. Admin invites member
        let invite_req = Request::builder()
            .method("POST")
            .uri(format!("/orgs/{}/invites", org_id))
            .header(admin_k.clone(), admin_v.clone())
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::to_string(
                    &serde_json::json!({ "user_id": "invited_member_user", "role": "member" }),
                )
                .unwrap(),
            ))
            .unwrap();
        let invite_resp = app.clone().oneshot(invite_req).await.unwrap();
        assert_eq!(invite_resp.status(), StatusCode::OK);

        // 3. Member tries to invite someone (Forbidden 403)
        let fail_invite_req = Request::builder()
            .method("POST")
            .uri(format!("/orgs/{}/invites", org_id))
            .header(member_k.clone(), member_v.clone())
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::to_string(
                    &serde_json::json!({ "user_id": "third_user", "role": "viewer" }),
                )
                .unwrap(),
            ))
            .unwrap();
        let fail_invite_resp = app.clone().oneshot(fail_invite_req).await.unwrap();
        assert_eq!(fail_invite_resp.status(), StatusCode::FORBIDDEN);

        // 4. Admin updates member role to admin
        let patch_req = Request::builder()
            .method("PATCH")
            .uri(format!("/orgs/{}/members/invited_member_user", org_id))
            .header(admin_k.clone(), admin_v.clone())
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::to_string(&serde_json::json!({ "role": "admin" })).unwrap(),
            ))
            .unwrap();
        let patch_resp = app.clone().oneshot(patch_req).await.unwrap();
        assert_eq!(patch_resp.status(), StatusCode::OK);

        // 5. Context Selection / Token issuance with org_id
        let select_req = Request::builder()
            .method("POST")
            .uri(format!("/orgs/{}/select", org_id))
            .header(member_k.clone(), member_v.clone())
            .body(Body::empty())
            .unwrap();
        let select_resp = app.clone().oneshot(select_req).await.unwrap();
        assert_eq!(select_resp.status(), StatusCode::OK);
        let select_json: serde_json::Value =
            serde_json::from_slice(&select_resp.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        assert!(select_json["token"].as_str().unwrap().len() > 20);
        assert_eq!(select_json["role"], "admin");
    }

    #[tokio::test]
    async fn test_org_sole_admin_protection_and_leave() {
        let app = create_app();
        let (admin_k, admin_v) = test_auth_header_for_user("sole_admin");

        // 1. Create org
        let create_req = Request::builder()
            .method("POST")
            .uri("/orgs")
            .header(admin_k.clone(), admin_v.clone())
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::to_string(&serde_json::json!({ "name": "Solo Capital" })).unwrap(),
            ))
            .unwrap();
        let create_resp = app.clone().oneshot(create_req).await.unwrap();
        let org_id: String = serde_json::from_slice::<serde_json::Value>(
            &create_resp.into_body().collect().await.unwrap().to_bytes(),
        )
        .unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string();

        // 2. Sole admin tries to demote self to member -> Bad Request 400
        let demote_req = Request::builder()
            .method("PATCH")
            .uri(format!("/orgs/{}/members/sole_admin", org_id))
            .header(admin_k.clone(), admin_v.clone())
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::to_string(&serde_json::json!({ "role": "member" })).unwrap(),
            ))
            .unwrap();
        let demote_resp = app.clone().oneshot(demote_req).await.unwrap();
        assert_eq!(demote_resp.status(), StatusCode::BAD_REQUEST);

        // 3. Sole admin tries to remove self -> Bad Request 400
        let remove_req = Request::builder()
            .method("DELETE")
            .uri(format!("/orgs/{}/members/sole_admin", org_id))
            .header(admin_k.clone(), admin_v.clone())
            .body(Body::empty())
            .unwrap();
        let remove_resp = app.clone().oneshot(remove_req).await.unwrap();
        assert_eq!(remove_resp.status(), StatusCode::BAD_REQUEST);

        // 4. Sole admin tries to leave -> Bad Request 400
        let leave_req = Request::builder()
            .method("POST")
            .uri(format!("/orgs/{}/leave", org_id))
            .header(admin_k.clone(), admin_v.clone())
            .body(Body::empty())
            .unwrap();
        let leave_resp = app.clone().oneshot(leave_req).await.unwrap();
        assert_eq!(leave_resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_ip_whitelist_crud_and_enforcement_middleware() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header_for_user("ip_trader");

        // 1. Initial state: no whitelist rules -> allowed from any IP (e.g. 198.51.100.5)
        let test_req = Request::builder()
            .method("GET")
            .uri("/sentiment?ticker=AAPL")
            .header(auth_k.clone(), auth_v.clone())
            .header("x-forwarded-for", "198.51.100.5")
            .body(Body::empty())
            .unwrap();
        let test_resp = app.clone().oneshot(test_req).await.unwrap();
        assert_eq!(test_resp.status(), StatusCode::OK);

        // 2. Add whitelist CIDR 203.0.113.0/24
        let add_req = Request::builder()
            .method("POST")
            .uri("/security/ip-whitelist")
            .header(auth_k.clone(), auth_v.clone())
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::to_string(&serde_json::json!({
                    "ip_or_cidr": "203.0.113.0/24",
                    "description": "Trading Office VPN"
                }))
                .unwrap(),
            ))
            .unwrap();
        let add_resp = app.clone().oneshot(add_req).await.unwrap();
        assert_eq!(add_resp.status(), StatusCode::CREATED);
        let entry: serde_json::Value =
            serde_json::from_slice(&add_resp.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        let entry_id = entry["id"].as_str().unwrap();

        // 3. Request from whitelisted IP (203.0.113.50) -> OK (200)
        let allowed_req = Request::builder()
            .method("GET")
            .uri("/sentiment?ticker=AAPL")
            .header(auth_k.clone(), auth_v.clone())
            .header("x-forwarded-for", "203.0.113.50")
            .body(Body::empty())
            .unwrap();
        let allowed_resp = app.clone().oneshot(allowed_req).await.unwrap();
        assert_eq!(allowed_resp.status(), StatusCode::OK);

        // 4. Request from non-whitelisted IP (198.51.100.5) -> FORBIDDEN (403)
        let blocked_req = Request::builder()
            .method("GET")
            .uri("/sentiment?ticker=AAPL")
            .header(auth_k.clone(), auth_v.clone())
            .header("x-forwarded-for", "198.51.100.5")
            .body(Body::empty())
            .unwrap();
        let blocked_resp = app.clone().oneshot(blocked_req).await.unwrap();
        assert_eq!(blocked_resp.status(), StatusCode::FORBIDDEN);

        // 5. List entries
        let list_req = Request::builder()
            .method("GET")
            .uri("/security/ip-whitelist")
            .header(auth_k.clone(), auth_v.clone())
            .header("x-forwarded-for", "203.0.113.50")
            .body(Body::empty())
            .unwrap();
        let list_resp = app.clone().oneshot(list_req).await.unwrap();
        assert_eq!(list_resp.status(), StatusCode::OK);

        // 6. Delete whitelist entry
        let del_req = Request::builder()
            .method("DELETE")
            .uri(format!("/security/ip-whitelist/{}", entry_id))
            .header(auth_k.clone(), auth_v.clone())
            .header("x-forwarded-for", "203.0.113.50")
            .body(Body::empty())
            .unwrap();
        let del_resp = app.clone().oneshot(del_req).await.unwrap();
        assert_eq!(del_resp.status(), StatusCode::OK);

        // 7. Whitelist is now empty -> access from 198.51.100.5 is allowed again
        let reopen_req = Request::builder()
            .method("GET")
            .uri("/sentiment?ticker=AAPL")
            .header(auth_k.clone(), auth_v.clone())
            .header("x-forwarded-for", "198.51.100.5")
            .body(Body::empty())
            .unwrap();
        let reopen_resp = app.clone().oneshot(reopen_req).await.unwrap();
        assert_eq!(reopen_resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_news_articles_endpoints() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        // 1. Unauthenticated request to /news/articles -> 401 Unauthorized
        let unauth_req = Request::builder()
            .method("GET")
            .uri("/news/articles")
            .body(Body::empty())
            .unwrap();
        let unauth_resp = app.clone().oneshot(unauth_req).await.unwrap();
        assert_eq!(unauth_resp.status(), StatusCode::UNAUTHORIZED);

        // 2. Authenticated request to /news/articles -> 200 OK with list of articles
        let list_req = Request::builder()
            .method("GET")
            .uri("/news/articles?limit=5")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let list_resp = app.clone().oneshot(list_req).await.unwrap();
        assert_eq!(list_resp.status(), StatusCode::OK);

        let body_bytes = list_resp.into_body().collect().await.unwrap().to_bytes();
        let list_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
        assert!(list_json.get("articles").is_some());
        let articles = list_json["articles"].as_array().unwrap();
        assert_eq!(articles.len(), 5);
        assert!(list_json["total"].as_u64().unwrap() >= 8);

        // Check snippet length <= 200
        let snippet = articles[0]["snippet"].as_str().unwrap();
        assert!(snippet.chars().count() <= 200);

        let first_id = articles[0]["id"].as_str().unwrap();

        // 3. Filter by ticker /news/articles?ticker=AAPL
        let aapl_req = Request::builder()
            .method("GET")
            .uri("/news/articles?ticker=AAPL")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let aapl_resp = app.clone().oneshot(aapl_req).await.unwrap();
        assert_eq!(aapl_resp.status(), StatusCode::OK);
        let aapl_bytes = aapl_resp.into_body().collect().await.unwrap().to_bytes();
        let aapl_json: serde_json::Value = serde_json::from_slice(&aapl_bytes).unwrap();
        let aapl_articles = aapl_json["articles"].as_array().unwrap();
        assert_eq!(aapl_articles.len(), 2);
        for a in aapl_articles {
            assert_eq!(a["ticker"].as_str().unwrap(), "AAPL");
        }

        // 4. Retrieve single full article by ID /news/articles/{id} -> 410 Gone with Sunset header
        let get_req = Request::builder()
            .method("GET")
            .uri(format!("/news/articles/{}", first_id))
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let get_resp = app.clone().oneshot(get_req).await.unwrap();
        assert_eq!(get_resp.status(), StatusCode::GONE);
        assert_eq!(
            get_resp
                .headers()
                .get("sunset")
                .and_then(|h| h.to_str().ok()),
            Some("Wed, 11 Nov 2026 00:00:00 GMT")
        );

        // 5. Retrieve non-existent ID -> 410 Gone (route itself is permanently removed)
        let not_found_req = Request::builder()
            .method("GET")
            .uri("/news/articles/00000000-0000-0000-0000-000000000000")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let not_found_resp = app.clone().oneshot(not_found_req).await.unwrap();
        assert_eq!(not_found_resp.status(), StatusCode::GONE);
    }

    #[tokio::test]
    async fn test_sentiment_entities_endpoint() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        // 1. Unauthenticated request -> 401
        let unauth_req = Request::builder()
            .method("GET")
            .uri("/sentiment/entities")
            .body(Body::empty())
            .unwrap();
        let unauth_resp = app.clone().oneshot(unauth_req).await.unwrap();
        assert_eq!(unauth_resp.status(), StatusCode::UNAUTHORIZED);

        // 2. Authenticated default request -> 200 OK
        let req = Request::builder()
            .method("GET")
            .uri("/sentiment/entities?start_date=2026-08-20&end_date=2026-08-30&min_mentions=5")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

        assert_eq!(json["entity_type"].as_str().unwrap(), "all");
        assert_eq!(json["min_mentions"].as_u64().unwrap(), 5);
        let entities = json["entities"].as_array().unwrap();
        assert!(!entities.is_empty());
        for ent in entities {
            assert!(ent["mention_count"].as_u64().unwrap() >= 5);
            assert!(ent["positive_ratio"].as_f64().unwrap() >= 0.0);
            assert!(ent["negative_ratio"].as_f64().unwrap() >= 0.0);
        }

        // 3. Filter by entity_type=company
        let comp_req = Request::builder()
            .method("GET")
            .uri("/sentiment/entities?entity_type=company&min_mentions=5")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let comp_resp = app.clone().oneshot(comp_req).await.unwrap();
        assert_eq!(comp_resp.status(), StatusCode::OK);
        let comp_bytes = comp_resp.into_body().collect().await.unwrap().to_bytes();
        let comp_json: serde_json::Value = serde_json::from_slice(&comp_bytes).unwrap();
        for ent in comp_json["entities"].as_array().unwrap() {
            assert_eq!(ent["entity_type"].as_str().unwrap(), "company");
        }

        // 4. Invalid entity_type -> 400 Bad Request
        let bad_type_req = Request::builder()
            .method("GET")
            .uri("/sentiment/entities?entity_type=invalid_type")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let bad_type_resp = app.clone().oneshot(bad_type_req).await.unwrap();
        assert_eq!(bad_type_resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_audit_logs_endpoints() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        // 1. Unauthenticated request -> 401 Unauthorized
        let unauth_req = Request::builder()
            .method("GET")
            .uri("/audit/logs")
            .body(Body::empty())
            .unwrap();
        let unauth_resp = app.clone().oneshot(unauth_req).await.unwrap();
        assert_eq!(unauth_resp.status(), StatusCode::UNAUTHORIZED);

        // 2. Authenticated request -> 200 OK
        let req = Request::builder()
            .method("GET")
            .uri("/audit/logs?limit=10&offset=0")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(json["logs"].is_array());
        assert_eq!(json["limit"].as_u64().unwrap(), 10);
        assert_eq!(json["offset"].as_u64().unwrap(), 0);

        // 3. Export JSON -> 200 OK with application/json
        let exp_json_req = Request::builder()
            .method("GET")
            .uri("/audit/export?format=json")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let exp_json_resp = app.clone().oneshot(exp_json_req).await.unwrap();
        assert_eq!(exp_json_resp.status(), StatusCode::OK);
        assert_eq!(
            exp_json_resp
                .headers()
                .get("content-type")
                .unwrap()
                .to_str()
                .unwrap(),
            "application/json; charset=utf-8"
        );

        // 4. Export CSV -> 200 OK with text/csv
        let exp_csv_req = Request::builder()
            .method("GET")
            .uri("/audit/export?format=csv")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let exp_csv_resp = app.clone().oneshot(exp_csv_req).await.unwrap();
        assert_eq!(exp_csv_resp.status(), StatusCode::OK);
        assert_eq!(
            exp_csv_resp
                .headers()
                .get("content-type")
                .unwrap()
                .to_str()
                .unwrap(),
            "text/csv; charset=utf-8"
        );
        let csv_bytes = exp_csv_resp.into_body().collect().await.unwrap().to_bytes();
        let csv_str = String::from_utf8(csv_bytes.to_vec()).unwrap();
        assert!(csv_str.starts_with(
            "id,org_id,user_id,action,entity_type,entity_id,details,ip_address,created_at"
        ));

        // 5. Invalid format -> 400 Bad Request
        let bad_fmt_req = Request::builder()
            .method("GET")
            .uri("/audit/export?format=xml")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let bad_fmt_resp = app.clone().oneshot(bad_fmt_req).await.unwrap();
        assert_eq!(bad_fmt_resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_unified_search_endpoint() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        // 1. Unauthenticated -> 401 Unauthorized
        let unauth_req = Request::builder()
            .method("GET")
            .uri("/search?q=AAPL")
            .body(Body::empty())
            .unwrap();
        let unauth_resp = app.clone().oneshot(unauth_req).await.unwrap();
        assert_eq!(unauth_resp.status(), StatusCode::UNAUTHORIZED);

        // 2. Empty query -> 400 Bad Request
        let empty_q_req = Request::builder()
            .method("GET")
            .uri("/search?q=")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let empty_q_resp = app.clone().oneshot(empty_q_req).await.unwrap();
        assert_eq!(empty_q_resp.status(), StatusCode::BAD_REQUEST);

        // 3. Invalid limit -> 400 Bad Request
        let bad_limit_req = Request::builder()
            .method("GET")
            .uri("/search?q=AAPL&limit=0")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let bad_limit_resp = app.clone().oneshot(bad_limit_req).await.unwrap();
        assert_eq!(bad_limit_resp.status(), StatusCode::BAD_REQUEST);

        // 4. Invalid data type -> 400 Bad Request
        let bad_type_req = Request::builder()
            .method("GET")
            .uri("/search?q=AAPL&types=invalid_type")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let bad_type_resp = app.clone().oneshot(bad_type_req).await.unwrap();
        assert_eq!(bad_type_resp.status(), StatusCode::BAD_REQUEST);

        // 5. Valid search for AAPL (all domains) -> 200 OK
        let search_req = Request::builder()
            .method("GET")
            .uri("/search?q=AAPL")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let search_resp = app.clone().oneshot(search_req).await.unwrap();
        assert_eq!(search_resp.status(), StatusCode::OK);
        let body = search_resp.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["query"].as_str().unwrap(), "AAPL");
        assert!(json["count"].as_u64().unwrap() > 0);
        let results = json["results"].as_array().unwrap();
        assert!(!results.is_empty());
        // First result score should be 1.0 for exact ticker match
        assert_eq!(results[0]["score"].as_f64().unwrap(), 1.0);

        // 6. Filter by types=news,transcripts
        let filtered_req = Request::builder()
            .method("GET")
            .uri("/search?q=Apple&types=news,transcripts")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let filtered_resp = app.clone().oneshot(filtered_req).await.unwrap();
        assert_eq!(filtered_resp.status(), StatusCode::OK);
        let filtered_body = filtered_resp
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes();
        let filtered_json: serde_json::Value = serde_json::from_slice(&filtered_body).unwrap();
        for r in filtered_json["results"].as_array().unwrap() {
            let t = r["type"].as_str().unwrap();
            assert!(t == "news" || t == "transcript");
        }
    }

    #[tokio::test]
    async fn test_sector_rotation_endpoint() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header_for_user("quant_tester");

        let req = Request::builder()
            .method("GET")
            .uri("/market/sector-rotation")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();

        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_email_digest_endpoints() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header_for_user("digest_quant_tester");

        let get_req1 = Request::builder()
            .method("GET")
            .uri("/digest/subscription")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let get_resp1 = app.clone().oneshot(get_req1).await.unwrap();
        assert_eq!(get_resp1.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_kafka_streaming_endpoints() {
        let app = create_app_with_full_surface();
        let (auth_k, auth_v) = test_auth_header();

        // 1. GET /stream/kafka/topics
        let topics_req = Request::builder()
            .method("GET")
            .uri("/internal/stream/kafka/topics")
            .header("X-Admin-Token", DEFAULT_DEV_ADMIN_TOKEN)
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let topics_resp = app.clone().oneshot(topics_req).await.unwrap();
        assert_eq!(topics_resp.status(), StatusCode::OK);
        let topics_json: serde_json::Value =
            serde_json::from_slice(&topics_resp.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        assert_eq!(topics_json["total_topics"].as_u64().unwrap(), 3);
        assert_eq!(topics_json["topics"].as_array().unwrap().len(), 3);

        // 2. GET /stream/kafka/credentials with valid topic
        let creds_req = Request::builder()
            .method("GET")
            .uri("/internal/stream/kafka/credentials?topic=sentiment-events&ttl_minutes=30&consumer_group=test-group-01")
            .header("X-Admin-Token", DEFAULT_DEV_ADMIN_TOKEN)
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let creds_resp = app.clone().oneshot(creds_req).await.unwrap();
        assert_eq!(creds_resp.status(), StatusCode::OK);
        let creds_json: serde_json::Value =
            serde_json::from_slice(&creds_resp.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        let cred_id = creds_json["id"].as_str().unwrap();
        assert_eq!(creds_json["topic"].as_str().unwrap(), "sentiment-events");
        assert_eq!(
            creds_json["consumer_group"].as_str().unwrap(),
            "test-group-01"
        );
        assert!(creds_json["password"].as_str().unwrap().starts_with("sec_"));

        // 3. GET /stream/kafka/credentials with invalid topic -> 400
        let bad_topic_req = Request::builder()
            .method("GET")
            .uri("/internal/stream/kafka/credentials?topic=non_existent_stream")
            .header("X-Admin-Token", DEFAULT_DEV_ADMIN_TOKEN)
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let bad_topic_resp = app.clone().oneshot(bad_topic_req).await.unwrap();
        assert_eq!(bad_topic_resp.status(), StatusCode::BAD_REQUEST);

        // 4. GET /stream/kafka/credentials with invalid TTL -> 400
        let bad_ttl_req = Request::builder()
            .method("GET")
            .uri("/internal/stream/kafka/credentials?topic=sentiment-events&ttl_minutes=999999")
            .header("X-Admin-Token", DEFAULT_DEV_ADMIN_TOKEN)
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let bad_ttl_resp = app.clone().oneshot(bad_ttl_req).await.unwrap();
        assert_eq!(bad_ttl_resp.status(), StatusCode::BAD_REQUEST);

        // 5. DELETE /stream/kafka/credentials/:id -> 200 OK
        let del_req = Request::builder()
            .method("DELETE")
            .uri(format!("/internal/stream/kafka/credentials/{}", cred_id))
            .header("X-Admin-Token", DEFAULT_DEV_ADMIN_TOKEN)
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let del_resp = app.clone().oneshot(del_req).await.unwrap();
        assert_eq!(del_resp.status(), StatusCode::OK);

        // 6. DELETE non-existent credentials -> 404
        let del_404_req = Request::builder()
            .method("DELETE")
            .uri(format!(
                "/internal/stream/kafka/credentials/{}",
                uuid::Uuid::new_v4()
            ))
            .header("X-Admin-Token", DEFAULT_DEV_ADMIN_TOKEN)
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let del_404_resp = app.clone().oneshot(del_404_req).await.unwrap();
        assert_eq!(del_404_resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_retention_policy_endpoints() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        // 1. GET /retention/policies initially empty
        let list_req = Request::builder()
            .method("GET")
            .uri("/retention/policies")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let list_resp = app.clone().oneshot(list_req).await.unwrap();
        assert_eq!(list_resp.status(), StatusCode::OK);
        let list_json: serde_json::Value =
            serde_json::from_slice(&list_resp.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        assert_eq!(list_json["total_policies"], 0);

        // 2. POST /retention/policies with invalid category -> 400
        let bad_cat_req = Request::builder()
            .method("POST")
            .uri("/retention/policies")
            .header(auth_k.clone(), auth_v.clone())
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::to_vec(&serde_json::json!({
                    "data_category": "illegal_category",
                    "retention_days": 90
                }))
                .unwrap(),
            ))
            .unwrap();
        let bad_cat_resp = app.clone().oneshot(bad_cat_req).await.unwrap();
        assert_eq!(bad_cat_resp.status(), StatusCode::BAD_REQUEST);

        // 3. POST /retention/policies with invalid retention_days (0) -> 400
        let bad_days_req = Request::builder()
            .method("POST")
            .uri("/retention/policies")
            .header(auth_k.clone(), auth_v.clone())
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::to_vec(&serde_json::json!({
                    "data_category": "usage_events",
                    "retention_days": 0
                }))
                .unwrap(),
            ))
            .unwrap();
        let bad_days_resp = app.clone().oneshot(bad_days_req).await.unwrap();
        assert_eq!(bad_days_resp.status(), StatusCode::BAD_REQUEST);

        // 4. POST /retention/policies valid -> 200 OK
        let create_req = Request::builder()
            .method("POST")
            .uri("/retention/policies")
            .header(auth_k.clone(), auth_v.clone())
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::to_vec(&serde_json::json!({
                    "data_category": "usage_events",
                    "retention_days": 90
                }))
                .unwrap(),
            ))
            .unwrap();
        let create_resp = app.clone().oneshot(create_req).await.unwrap();
        assert_eq!(create_resp.status(), StatusCode::OK);
        let create_json: serde_json::Value =
            serde_json::from_slice(&create_resp.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        assert_eq!(create_json["data_category"], "usage_events");
        assert_eq!(create_json["retention_days"], 90);
        let policy_id = create_json["id"].as_str().unwrap().to_string();

        // 5. GET /retention/policies now contains 1 policy
        let list_req2 = Request::builder()
            .method("GET")
            .uri("/retention/policies")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let list_resp2 = app.clone().oneshot(list_req2).await.unwrap();
        assert_eq!(list_resp2.status(), StatusCode::OK);
        let list_json2: serde_json::Value =
            serde_json::from_slice(&list_resp2.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        assert_eq!(list_json2["total_policies"], 1);

        // 6. DELETE /retention/policies/:id -> 200 OK
        let del_req = Request::builder()
            .method("DELETE")
            .uri(format!("/retention/policies/{}", policy_id))
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let del_resp = app.clone().oneshot(del_req).await.unwrap();
        assert_eq!(del_resp.status(), StatusCode::OK);

        // 7. DELETE non-existent policy -> 404
        let del_404_req = Request::builder()
            .method("DELETE")
            .uri(format!("/retention/policies/{}", uuid::Uuid::new_v4()))
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let del_404_resp = app.clone().oneshot(del_404_req).await.unwrap();
        assert_eq!(del_404_resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_factor_exposure_endpoint() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .method("GET")
            .uri("/risk/factor-exposure?ticker=AAPL&start_date=2025-01-01&end_date=2025-06-30&benchmark_ticker=SPY")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_esg_scores_endpoint() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .method("GET")
            .uri("/esg/scores?ticker=AAPL&start_date=2025-06-01&end_date=2025-08-30")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_bankruptcy_risk_endpoint() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .method("GET")
            .uri("/risk/bankruptcy?ticker=AAPL&lookback_days=30&include_components=true")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_fx_sentiment_endpoint() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .method("GET")
            .uri("/fx/sentiment?currency_pair=EUR/USD&min_confidence=0.5&limit=10")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_commodity_sentiment_endpoint() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .method("GET")
            .uri("/commodities/sentiment?commodity=crude_oil&min_confidence=0.5&limit=10")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_polling_webhooks_lifecycle() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req_create = Request::builder()
            .method("POST")
            .uri("/polling-webhooks")
            .header(auth_k.clone(), auth_v.clone())
            .header("Content-Type", "application/json")
            .body(Body::from(r#"{"name":"Tech Sentiment Poll","url":"https://quant.fund.com/poll","interval_seconds":300,"query_type":"sentiment","query_params":{"tickers":["AAPL","MSFT"]}}"#))
            .unwrap();
        let resp_create = app.clone().oneshot(req_create).await.unwrap();
        assert_eq!(resp_create.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_crypto_sentiment_endpoint() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req_ok = Request::builder()
            .uri("/crypto/sentiment?asset=BTC&min_confidence=0.5&limit=5")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp_ok = app.clone().oneshot(req_ok).await.unwrap();
        assert_eq!(resp_ok.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_options_microstructure_endpoint() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        // 1. Invalid date range -> 400
        let req_invalid = Request::builder()
            .uri("/options/microstructure?ticker=AAPL&start_date=2025-03-31&end_date=2025-01-01")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp_invalid = app.clone().oneshot(req_invalid).await.unwrap();
        assert_eq!(resp_invalid.status(), StatusCode::BAD_REQUEST);

        // 2. Valid AAPL microstructure query -> 200 OK
        let req_ok = Request::builder()
            .uri("/options/microstructure?ticker=AAPL&start_date=2025-01-01&end_date=2025-01-15&metric=both&interval=daily&limit=10")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp_ok = app.clone().oneshot(req_ok).await.unwrap();
        assert_eq!(resp_ok.status(), StatusCode::OK);

        let body_json: serde_json::Value =
            serde_json::from_slice(&resp_ok.into_body().collect().await.unwrap().to_bytes())
                .unwrap();

        assert_eq!(body_json["ticker"], "AAPL");
        assert_eq!(body_json["metric"], "both");
        assert_eq!(body_json["interval"], "daily");
        assert!(body_json["count"].as_u64().unwrap() > 0);
        assert!(body_json["points"].is_array());
        let pts = body_json["points"].as_array().unwrap();
        assert!(!pts.is_empty());
        assert!(pts[0]["vpin"].is_number());
        assert!(pts[0]["gex"].is_number());
    }

    #[tokio::test]
    async fn test_market_breadth_endpoint() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .uri("/market/breadth?start_date=2025-01-01&end_date=2025-01-15&universe=all&limit=10&include_new_highs_lows=true")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_chat_alerts_endpoints() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req_tg = Request::builder()
            .method("POST")
            .uri("/chat-alerts")
            .header(auth_k.clone(), auth_v.clone())
            .header("Content-Type", "application/json")
            .body(Body::from(
                serde_json::to_string(&serde_json::json!({
                    "channel_type": "telegram",
                    "channel_target": "123456789",
                    "event_types": ["sentiment_anomaly", "8k_filing"]
                }))
                .unwrap(),
            ))
            .unwrap();
        let resp_tg = app.clone().oneshot(req_tg).await.unwrap();
        assert_eq!(resp_tg.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_credit_sentiment_endpoint() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .uri("/risk/credit-sentiment?ticker=AAPL&lookback_days=30")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_backfill_sentiment_endpoint() {
        let app = create_app_with_full_surface();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .method("POST")
            .uri("/internal/sentiment/backfill")
            .header("X-Admin-Token", DEFAULT_DEV_ADMIN_TOKEN)
            .header(auth_k.clone(), auth_v.clone())
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                serde_json::to_vec(&serde_json::json!({
                    "ticker": "AAPL",
                    "start_date": "2025-01-01",
                    "end_date": "2025-03-31",
                    "limit": 500,
                    "overwrite": false
                }))
                .unwrap(),
            ))
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let json_body: serde_json::Value =
            serde_json::from_slice(&resp.into_body().collect().await.unwrap().to_bytes()).unwrap();

        assert_eq!(json_body["ticker"], "AAPL");
        assert_eq!(json_body["start_date"], "2025-01-01");
        assert_eq!(json_body["end_date"], "2025-03-31");
        assert_eq!(json_body["overwrite"], false);
        assert!(json_body["total_articles_found"].is_number());
        assert!(json_body["processed_articles"].is_number());
        assert_eq!(json_body["failed_articles"], 0);
    }

    #[tokio::test]
    async fn test_portfolio_optimize_endpoint() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .method("POST")
            .uri("/portfolio/optimize")
            .header(auth_k.clone(), auth_v.clone())
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                serde_json::to_vec(&serde_json::json!({
                    "tickers": ["AAPL", "MSFT", "NVDA"],
                    "start_date": "2025-01-01",
                    "end_date": "2025-06-30",
                    "optimization_type": "max_sharpe",
                    "risk_free_rate": 0.05
                }))
                .unwrap(),
            ))
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_portfolio_factor_exposure_endpoint() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header();

        let req = Request::builder()
            .method("POST")
            .uri("/risk/portfolio-factor-exposure")
            .header(auth_k.clone(), auth_v.clone())
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                serde_json::to_vec(&serde_json::json!({
                    "tickers": ["AAPL", "MSFT", "NVDA"],
                    "weights": [0.4, 0.3, 0.3],
                    "start_date": "2025-01-01",
                    "end_date": "2025-06-30",
                    "benchmark_ticker": "SPY",
                    "factors": "market,momentum,sentiment,volatility"
                }))
                .unwrap(),
            ))
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_retraining_endpoints() {
        let app = create_app_with_full_surface();
        let (auth_k, auth_v) = test_auth_header();

        // 1. Create retraining job
        let req = Request::builder()
            .method("POST")
            .uri("/internal/retraining/jobs")
            .header("X-Admin-Token", DEFAULT_DEV_ADMIN_TOKEN)
            .header(auth_k.clone(), auth_v.clone())
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                serde_json::to_vec(&serde_json::json!({
                    "model_type": "sentiment",
                    "trigger_type": "manual",
                    "config": {
                        "learning_rate": 0.00002,
                        "epochs": 3,
                        "batch_size": 32
                    }
                }))
                .unwrap(),
            ))
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);

        let json_body: serde_json::Value =
            serde_json::from_slice(&resp.into_body().collect().await.unwrap().to_bytes()).unwrap();

        let job_id_str = json_body["job"]["id"].as_str().unwrap();
        assert_eq!(json_body["job"]["model_type"], "sentiment");
        assert_eq!(json_body["job"]["trigger_type"], "manual");

        // 2. Get retraining job by ID
        let req2 = Request::builder()
            .method("GET")
            .uri(&format!("/internal/retraining/jobs/{}", job_id_str))
            .header("X-Admin-Token", DEFAULT_DEV_ADMIN_TOKEN)
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp2 = app.clone().oneshot(req2).await.unwrap();
        assert_eq!(resp2.status(), StatusCode::OK);

        // 3. List retraining jobs
        let req3 = Request::builder()
            .method("GET")
            .uri("/internal/retraining/jobs?limit=10")
            .header("X-Admin-Token", DEFAULT_DEV_ADMIN_TOKEN)
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp3 = app.clone().oneshot(req3).await.unwrap();
        assert_eq!(resp3.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_fix_order_submission_market_fill() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header_with_role("fix_trader_01", "enterprise");

        let fix_msg = "8=FIX.4.4|9=60|35=D|11=CL-MKT-01|55=AAPL|54=1|38=100|40=1|10=000|";
        let req = Request::builder()
            .method("POST")
            .uri("/fix/order")
            .header(auth_k.clone(), auth_v.clone())
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                serde_json::to_vec(&serde_json::json!({
                    "fix_message": fix_msg
                }))
                .unwrap(),
            ))
            .unwrap();

        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_fix_order_submission_limit_open_and_cancel() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header_with_role("fix_trader_02", "enterprise");

        let fix_limit_msg =
            "8=FIX.4.4|9=70|35=D|11=CL-LMT-01|55=MSFT|54=1|38=50|40=2|44=10.00|10=000|";
        let req_order = Request::builder()
            .method("POST")
            .uri("/fix/order")
            .header(auth_k.clone(), auth_v.clone())
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                serde_json::to_vec(&serde_json::json!({
                    "fix_message": fix_limit_msg
                }))
                .unwrap(),
            ))
            .unwrap();

        let resp_order = app.clone().oneshot(req_order).await.unwrap();
        assert_eq!(resp_order.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_fix_order_list_with_filters() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header_with_role("fix_trader_03", "enterprise");

        let req_all = Request::builder()
            .method("GET")
            .uri("/fix/orders?limit=10")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();
        let resp_all = app.clone().oneshot(req_all).await.unwrap();
        assert_eq!(resp_all.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_fix_order_missing_tags_rejected() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header_with_role("fix_trader_04", "enterprise");

        // Missing Symbol (Tag 55)
        let invalid_msg = "8=FIX.4.4|9=40|35=D|11=INV-01|54=1|38=100|40=1|10=000|";
        let req = Request::builder()
            .method("POST")
            .uri("/fix/order")
            .header(auth_k.clone(), auth_v.clone())
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                serde_json::to_vec(&serde_json::json!({
                    "fix_message": invalid_msg
                }))
                .unwrap(),
            ))
            .unwrap();

        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_fix_order_forbidden_for_non_enterprise_role() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header_with_role("retail_trader_01", "institutional");

        let fix_msg = "8=FIX.4.4|9=60|35=D|11=CL-MKT-01|55=AAPL|54=1|38=100|40=1|10=000|";
        let req = Request::builder()
            .method("POST")
            .uri("/fix/order")
            .header(auth_k.clone(), auth_v.clone())
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                serde_json::to_vec(&serde_json::json!({
                    "fix_message": fix_msg
                }))
                .unwrap(),
            ))
            .unwrap();

        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_fix_bridge_disabled_returns_404() {
        let mut state = AppState::default();
        state.enable_fix_bridge = false;
        let app = create_app_with_state(state);
        let (auth_k, auth_v) = test_auth_header_with_role("ent_user", "enterprise");

        let fix_msg = "8=FIX.4.4|9=60|35=D|11=CL-MKT-01|55=AAPL|54=1|38=100|40=1|10=000|";
        let req = Request::builder()
            .method("POST")
            .uri("/fix/order")
            .header(auth_k.clone(), auth_v.clone())
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                serde_json::to_vec(&serde_json::json!({
                    "fix_message": fix_msg
                }))
                .unwrap(),
            ))
            .unwrap();

        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_dlq_endpoints_auth_and_rbac() {
        let app = create_app_with_full_surface();

        // 1. Unauthenticated GET /internal/dlq/events -> 401
        let req_unauth = Request::builder()
            .method("GET")
            .uri("/internal/dlq/events")
            .body(Body::empty())
            .unwrap();
        let resp_unauth = app.clone().oneshot(req_unauth).await.unwrap();
        assert_eq!(resp_unauth.status(), StatusCode::UNAUTHORIZED);

        // 1b. Missing X-Admin-Token with valid admin JWT -> 401 Unauthorized
        let admin_token = generate_jwt(
            "admin_user",
            DEFAULT_JWT_EXPIRY_SECS,
            Some("admin"),
            DEFAULT_DEV_JWT_SECRET.as_bytes(),
        )
        .unwrap();
        let req_no_admin_tok = Request::builder()
            .method("GET")
            .uri("/internal/dlq/events")
            .header(header::AUTHORIZATION, format!("Bearer {}", admin_token))
            .body(Body::empty())
            .unwrap();
        let resp_no_admin_tok = app.clone().oneshot(req_no_admin_tok).await.unwrap();
        assert_eq!(resp_no_admin_tok.status(), StatusCode::UNAUTHORIZED);

        // 2. Retail user (non-admin) GET /internal/dlq/events with valid admin token -> 403 Forbidden
        let retail_token = generate_jwt(
            "retail_trader_01",
            DEFAULT_JWT_EXPIRY_SECS,
            Some("retail"),
            DEFAULT_DEV_JWT_SECRET.as_bytes(),
        )
        .unwrap();
        let req_retail = Request::builder()
            .method("GET")
            .uri("/internal/dlq/events")
            .header("X-Admin-Token", DEFAULT_DEV_ADMIN_TOKEN)
            .header(header::AUTHORIZATION, format!("Bearer {}", retail_token))
            .body(Body::empty())
            .unwrap();
        let resp_retail = app.clone().oneshot(req_retail).await.unwrap();
        assert_eq!(resp_retail.status(), StatusCode::FORBIDDEN);

        // 3. Admin user GET /internal/dlq/events with admin token -> 200 OK
        let req_admin = Request::builder()
            .method("GET")
            .uri("/internal/dlq/events")
            .header("X-Admin-Token", DEFAULT_DEV_ADMIN_TOKEN)
            .header(header::AUTHORIZATION, format!("Bearer {}", admin_token))
            .body(Body::empty())
            .unwrap();
        let resp_admin = app.clone().oneshot(req_admin).await.unwrap();
        assert_eq!(resp_admin.status(), StatusCode::OK);

        let list_resp: DLQEventsListResponse =
            serde_json::from_slice(&resp_admin.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        assert!(list_resp.total >= 4);

        // 4. Old un-prefixed /dlq/events endpoint -> 404 Not Found
        let req_old = Request::builder()
            .method("GET")
            .uri("/dlq/events")
            .header("X-Admin-Token", DEFAULT_DEV_ADMIN_TOKEN)
            .header(header::AUTHORIZATION, format!("Bearer {}", admin_token))
            .body(Body::empty())
            .unwrap();
        let resp_old = app.oneshot(req_old).await.unwrap();
        assert_eq!(resp_old.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_dlq_list_filtering_and_pagination() {
        let app = create_app_with_full_surface();
        let admin_token = generate_jwt(
            "admin_user",
            DEFAULT_JWT_EXPIRY_SECS,
            Some("admin"),
            DEFAULT_DEV_JWT_SECRET.as_bytes(),
        )
        .unwrap();
        let auth_hdr = format!("Bearer {}", admin_token);

        // Filter by source=sentiment
        let req_sent = Request::builder()
            .method("GET")
            .uri("/internal/dlq/events?source=sentiment&status=all")
            .header("X-Admin-Token", DEFAULT_DEV_ADMIN_TOKEN)
            .header(header::AUTHORIZATION, &auth_hdr)
            .body(Body::empty())
            .unwrap();
        let resp_sent = app.clone().oneshot(req_sent).await.unwrap();
        assert_eq!(resp_sent.status(), StatusCode::OK);
        let list_sent: DLQEventsListResponse =
            serde_json::from_slice(&resp_sent.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        assert!(list_sent.events.iter().all(|e| e.source == "sentiment"));

        // Filter by status=reprocessed
        let req_reproc = Request::builder()
            .method("GET")
            .uri("/internal/dlq/events?status=reprocessed")
            .header("X-Admin-Token", DEFAULT_DEV_ADMIN_TOKEN)
            .header(header::AUTHORIZATION, &auth_hdr)
            .body(Body::empty())
            .unwrap();
        let resp_reproc = app.clone().oneshot(req_reproc).await.unwrap();
        assert_eq!(resp_reproc.status(), StatusCode::OK);
        let list_reproc: DLQEventsListResponse =
            serde_json::from_slice(&resp_reproc.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        assert!(list_reproc.events.iter().all(|e| e.status == "reprocessed"));
    }

    #[tokio::test]
    async fn test_dlq_get_reprocess_and_purge_lifecycle() {
        let app = create_app_with_full_surface();
        let admin_token = generate_jwt(
            "admin_user",
            DEFAULT_JWT_EXPIRY_SECS,
            Some("admin"),
            DEFAULT_DEV_JWT_SECRET.as_bytes(),
        )
        .unwrap();
        let auth_hdr = format!("Bearer {}", admin_token);

        let target_uuid = uuid::Uuid::parse_str("550e8400-e29b-41d4-a716-446655440101").unwrap();

        // 1. GET /internal/dlq/events/{id}
        let req_get = Request::builder()
            .method("GET")
            .uri(format!("/internal/dlq/events/{}", target_uuid))
            .header("X-Admin-Token", DEFAULT_DEV_ADMIN_TOKEN)
            .header(header::AUTHORIZATION, &auth_hdr)
            .body(Body::empty())
            .unwrap();
        let resp_get = app.clone().oneshot(req_get).await.unwrap();
        assert_eq!(resp_get.status(), StatusCode::OK);
        let detail: DLQEventDetail =
            serde_json::from_slice(&resp_get.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        assert_eq!(detail.event_id, "evt_sentiment_nvda_101");
        assert_eq!(detail.source, "sentiment");
        assert_eq!(detail.payload["ticker"], "NVDA");

        // 2. GET non-existent event -> 404
        let req_not_found = Request::builder()
            .method("GET")
            .uri(format!("/internal/dlq/events/{}", uuid::Uuid::new_v4()))
            .header("X-Admin-Token", DEFAULT_DEV_ADMIN_TOKEN)
            .header(header::AUTHORIZATION, &auth_hdr)
            .body(Body::empty())
            .unwrap();
        let resp_not_found = app.clone().oneshot(req_not_found).await.unwrap();
        assert_eq!(resp_not_found.status(), StatusCode::NOT_FOUND);

        // 3. POST /internal/dlq/events/{id}/reprocess -> 200 OK
        let req_reprocess = Request::builder()
            .method("POST")
            .uri(format!(
                "/internal/dlq/events/{id}/reprocess",
                id = target_uuid
            ))
            .header("X-Admin-Token", DEFAULT_DEV_ADMIN_TOKEN)
            .header(header::AUTHORIZATION, &auth_hdr)
            .body(Body::empty())
            .unwrap();
        let resp_reprocess = app.clone().oneshot(req_reprocess).await.unwrap();
        assert_eq!(resp_reprocess.status(), StatusCode::OK);
        let reprocess_resp: ReprocessDLQResponse = serde_json::from_slice(
            &resp_reprocess
                .into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes(),
        )
        .unwrap();
        assert_eq!(reprocess_resp.status, "reprocessed");
        assert_eq!(reprocess_resp.retry_count, 3);

        // 4. DELETE /internal/dlq/events/{id} -> 200 OK
        let req_purge = Request::builder()
            .method("DELETE")
            .uri(format!("/internal/dlq/events/{id}", id = target_uuid))
            .header("X-Admin-Token", DEFAULT_DEV_ADMIN_TOKEN)
            .header(header::AUTHORIZATION, &auth_hdr)
            .body(Body::empty())
            .unwrap();
        let resp_purge = app.clone().oneshot(req_purge).await.unwrap();
        assert_eq!(resp_purge.status(), StatusCode::OK);
        let purge_resp: PurgeDLQResponse =
            serde_json::from_slice(&resp_purge.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        assert_eq!(purge_resp.status, "purged");

        // 5. Attempting to reprocess a purged event -> 400 Bad Request
        let req_reprocess_purged = Request::builder()
            .method("POST")
            .uri(format!(
                "/internal/dlq/events/{id}/reprocess",
                id = target_uuid
            ))
            .header("X-Admin-Token", DEFAULT_DEV_ADMIN_TOKEN)
            .header(header::AUTHORIZATION, &auth_hdr)
            .body(Body::empty())
            .unwrap();
        let resp_reprocess_purged = app.clone().oneshot(req_reprocess_purged).await.unwrap();
        assert_eq!(resp_reprocess_purged.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_sla_status_unauthenticated_returns_401() {
        let app = create_app();
        let req = Request::builder()
            .method("GET")
            .uri("/sla/status")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_sla_status_valid_defaults_returns_200() {
        let app = create_app();
        let token = generate_jwt(
            "sla_enterprise_user",
            DEFAULT_JWT_EXPIRY_SECS,
            Some("institutional"),
            DEFAULT_DEV_JWT_SECRET.as_bytes(),
        )
        .unwrap();
        let req = Request::builder()
            .method("GET")
            .uri("/sla/status")
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body_bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let sla_resp: SLAStatusResponse = serde_json::from_slice(&body_bytes).unwrap();
        assert!(sla_resp.total_requests > 0);
        assert!(sla_resp.average_latency_ms > 0.0);
        assert!(sla_resp.percentiles.contains_key("p50"));
        assert!(sla_resp.percentiles.contains_key("p95"));
        assert!(sla_resp.percentiles.contains_key("p99"));
        assert_eq!(sla_resp.sla_target_ms, 100);
        assert!(sla_resp.compliant_requests <= sla_resp.total_requests);
        assert!(sla_resp.sla_compliance_rate >= 0.0 && sla_resp.sla_compliance_rate <= 100.0);
        assert_eq!(sla_resp.sla_status, "met");
    }

    #[tokio::test]
    async fn test_sla_status_custom_percentiles_and_target() {
        let app = create_app();
        let token = generate_jwt(
            "sla_enterprise_user",
            DEFAULT_JWT_EXPIRY_SECS,
            Some("institutional"),
            DEFAULT_DEV_JWT_SECRET.as_bytes(),
        )
        .unwrap();
        let req = Request::builder()
            .method("GET")
            .uri("/sla/status?percentiles=50,75,90,99.5&sla_target_ms=10")
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body_bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let sla_resp: SLAStatusResponse = serde_json::from_slice(&body_bytes).unwrap();
        assert!(sla_resp.percentiles.contains_key("p50"));
        assert!(sla_resp.percentiles.contains_key("p75"));
        assert!(sla_resp.percentiles.contains_key("p90"));
        assert!(sla_resp.percentiles.contains_key("p99.5"));
        assert_eq!(sla_resp.sla_target_ms, 10);
    }

    #[tokio::test]
    async fn test_sla_status_validation_errors() {
        let app = create_app();
        let token = generate_jwt(
            "sla_enterprise_user",
            DEFAULT_JWT_EXPIRY_SECS,
            Some("institutional"),
            DEFAULT_DEV_JWT_SECRET.as_bytes(),
        )
        .unwrap();
        let auth_hdr = format!("Bearer {}", token);

        // 1. Invalid start_date format
        let req1 = Request::builder()
            .method("GET")
            .uri("/sla/status?start_date=invalid-date")
            .header(header::AUTHORIZATION, &auth_hdr)
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            app.clone().oneshot(req1).await.unwrap().status(),
            StatusCode::BAD_REQUEST
        );

        // 2. start_date > end_date
        let req2 = Request::builder()
            .method("GET")
            .uri("/sla/status?start_date=2026-08-30&end_date=2026-08-01")
            .header(header::AUTHORIZATION, &auth_hdr)
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            app.clone().oneshot(req2).await.unwrap().status(),
            StatusCode::BAD_REQUEST
        );

        // 3. sla_target_ms out of bounds (< 10 or > 5000)
        let req3 = Request::builder()
            .method("GET")
            .uri("/sla/status?sla_target_ms=5")
            .header(header::AUTHORIZATION, &auth_hdr)
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            app.clone().oneshot(req3).await.unwrap().status(),
            StatusCode::BAD_REQUEST
        );

        // 4. Invalid percentile (> 100)
        let req4 = Request::builder()
            .method("GET")
            .uri("/sla/status?percentiles=50,150")
            .header(header::AUTHORIZATION, &auth_hdr)
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            app.clone().oneshot(req4).await.unwrap().status(),
            StatusCode::BAD_REQUEST
        );
    }

    #[tokio::test]
    async fn test_sla_latency_unauthenticated_returns_401() {
        let app = create_app();
        let req = Request::builder()
            .method("GET")
            .uri("/sla/latency")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_sla_latency_valid_defaults_returns_200() {
        let app = create_app();
        let token = generate_jwt(
            "sla_pipeline_user",
            DEFAULT_JWT_EXPIRY_SECS,
            Some("institutional"),
            DEFAULT_DEV_JWT_SECRET.as_bytes(),
        )
        .unwrap();
        let req = Request::builder()
            .method("GET")
            .uri("/sla/latency")
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body_bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let sla_resp: SLALatencyResponse = serde_json::from_slice(&body_bytes).unwrap();
        assert!(sla_resp.total_signals > 0);
        assert!(sla_resp.average_latency_ms > 0.0);
        assert!(sla_resp.percentiles.contains_key("p50"));
        assert!(sla_resp.percentiles.contains_key("p95"));
        assert!(sla_resp.percentiles.contains_key("p99"));
        assert_eq!(sla_resp.sla_target_ms, 500);
        assert!(sla_resp.sla_compliant_signals <= sla_resp.total_signals);
        assert!(sla_resp.sla_compliance_rate >= 0.0 && sla_resp.sla_compliance_rate <= 100.0);
        assert_eq!(sla_resp.sla_status, "met");
        assert!(sla_resp.stage_breakdown.fetch_latency_ms > 0.0);
        assert!(sla_resp.stage_breakdown.normalization_latency_ms > 0.0);
        assert!(sla_resp.stage_breakdown.inference_latency_ms > 0.0);
        assert!(sla_resp.stage_breakdown.write_latency_ms > 0.0);
    }

    #[tokio::test]
    async fn test_sla_latency_custom_ticker_percentiles_and_target() {
        let app = create_app();
        let token = generate_jwt(
            "sla_pipeline_user",
            DEFAULT_JWT_EXPIRY_SECS,
            Some("institutional"),
            DEFAULT_DEV_JWT_SECRET.as_bytes(),
        )
        .unwrap();
        let req = Request::builder()
            .method("GET")
            .uri("/sla/latency?ticker=NVDA&percentiles=50,75,90,99.9&sla_target_ms=300&start_date=2025-01-01&end_date=2025-01-31")
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body_bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let sla_resp: SLALatencyResponse = serde_json::from_slice(&body_bytes).unwrap();
        assert_eq!(sla_resp.ticker.as_deref(), Some("NVDA"));
        assert_eq!(sla_resp.start_date, "2025-01-01");
        assert_eq!(sla_resp.end_date, "2025-01-31");
        assert!(sla_resp.percentiles.contains_key("p50"));
        assert!(sla_resp.percentiles.contains_key("p75"));
        assert!(sla_resp.percentiles.contains_key("p90"));
        assert!(sla_resp.percentiles.contains_key("p99.9"));
        assert_eq!(sla_resp.sla_target_ms, 300);
    }

    #[tokio::test]
    async fn test_sla_latency_validation_errors() {
        let app = create_app();
        let token = generate_jwt(
            "sla_pipeline_user",
            DEFAULT_JWT_EXPIRY_SECS,
            Some("institutional"),
            DEFAULT_DEV_JWT_SECRET.as_bytes(),
        )
        .unwrap();
        let auth_hdr = format!("Bearer {}", token);

        // 1. Invalid start_date format
        let req1 = Request::builder()
            .method("GET")
            .uri("/sla/latency?start_date=not-a-date")
            .header(header::AUTHORIZATION, &auth_hdr)
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            app.clone().oneshot(req1).await.unwrap().status(),
            StatusCode::BAD_REQUEST
        );

        // 2. start_date > end_date
        let req2 = Request::builder()
            .method("GET")
            .uri("/sla/latency?start_date=2026-09-01&end_date=2026-08-01")
            .header(header::AUTHORIZATION, &auth_hdr)
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            app.clone().oneshot(req2).await.unwrap().status(),
            StatusCode::BAD_REQUEST
        );

        // 3. sla_target_ms out of bounds (< 10 or > 10000)
        let req3 = Request::builder()
            .method("GET")
            .uri("/sla/latency?sla_target_ms=25000")
            .header(header::AUTHORIZATION, &auth_hdr)
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            app.clone().oneshot(req3).await.unwrap().status(),
            StatusCode::BAD_REQUEST
        );

        // 4. Invalid percentile (> 100)
        let req4 = Request::builder()
            .method("GET")
            .uri("/sla/latency?percentiles=50,150")
            .header(header::AUTHORIZATION, &auth_hdr)
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            app.clone().oneshot(req4).await.unwrap().status(),
            StatusCode::BAD_REQUEST
        );
    }

    #[tokio::test]
    async fn test_sandbox_unauthenticated_returns_401() {
        let app = create_app();

        let req1 = Request::builder()
            .method("GET")
            .uri("/sandbox/status")
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            app.clone().oneshot(req1).await.unwrap().status(),
            StatusCode::UNAUTHORIZED
        );

        let req2 = Request::builder()
            .method("POST")
            .uri("/sandbox/activate")
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            app.clone().oneshot(req2).await.unwrap().status(),
            StatusCode::UNAUTHORIZED
        );

        let req3 = Request::builder()
            .method("POST")
            .uri("/sandbox/deactivate")
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            app.clone().oneshot(req3).await.unwrap().status(),
            StatusCode::UNAUTHORIZED
        );
    }

    #[tokio::test]
    async fn test_sandbox_activate_deactivate_lifecycle() {
        let state = AppState::default();
        let app = create_app_with_state(state.clone());
        let token = generate_jwt(
            "sandbox_tester_01",
            DEFAULT_JWT_EXPIRY_SECS,
            Some("institutional"),
            DEFAULT_DEV_JWT_SECRET.as_bytes(),
        )
        .unwrap();
        let auth_hdr = format!("Bearer {}", token);

        // 1. Initial status: inactive
        let req1 = Request::builder()
            .method("GET")
            .uri("/sandbox/status")
            .header(header::AUTHORIZATION, &auth_hdr)
            .body(Body::empty())
            .unwrap();
        let res1 = app.clone().oneshot(req1).await.unwrap();
        assert_eq!(res1.status(), StatusCode::OK);
        let bytes1 = axum::body::to_bytes(res1.into_body(), usize::MAX)
            .await
            .unwrap();
        let status1: SandboxStatusResponse = serde_json::from_slice(&bytes1).unwrap();
        assert!(!status1.active);
        assert_eq!(status1.mock_data_version, DEFAULT_SANDBOX_MOCK_VERSION);

        // 2. Activate sandbox
        let req2 = Request::builder()
            .method("POST")
            .uri("/sandbox/activate")
            .header(header::AUTHORIZATION, &auth_hdr)
            .body(Body::empty())
            .unwrap();
        let res2 = app.clone().oneshot(req2).await.unwrap();
        assert_eq!(res2.status(), StatusCode::OK);
        let bytes2 = axum::body::to_bytes(res2.into_body(), usize::MAX)
            .await
            .unwrap();
        let status2: SandboxStatusResponse = serde_json::from_slice(&bytes2).unwrap();
        assert!(status2.active);
        assert!(status2.activated_at.is_some());
        assert!(status2
            .available_endpoints
            .contains(&"/sentiment".to_string()));

        // 3. Status is now active
        let req3 = Request::builder()
            .method("GET")
            .uri("/sandbox/status")
            .header(header::AUTHORIZATION, &auth_hdr)
            .body(Body::empty())
            .unwrap();
        let res3 = app.clone().oneshot(req3).await.unwrap();
        assert_eq!(res3.status(), StatusCode::OK);
        let bytes3 = axum::body::to_bytes(res3.into_body(), usize::MAX)
            .await
            .unwrap();
        let status3: SandboxStatusResponse = serde_json::from_slice(&bytes3).unwrap();
        assert!(status3.active);

        // 4. Deactivate sandbox
        let req4 = Request::builder()
            .method("POST")
            .uri("/sandbox/deactivate")
            .header(header::AUTHORIZATION, &auth_hdr)
            .body(Body::empty())
            .unwrap();
        let res4 = app.clone().oneshot(req4).await.unwrap();
        assert_eq!(res4.status(), StatusCode::OK);
        let bytes4 = axum::body::to_bytes(res4.into_body(), usize::MAX)
            .await
            .unwrap();
        let status4: SandboxStatusResponse = serde_json::from_slice(&bytes4).unwrap();
        assert!(!status4.active);
        assert!(status4.deactivated_at.is_some());
    }

    #[tokio::test]
    async fn test_sandbox_token_claim_and_response_headers() {
        let app = create_app();
        // Token explicitly issued with sandbox: Some(true)
        let token = generate_jwt_with_sandbox(
            "sandbox_trial_user",
            DEFAULT_JWT_EXPIRY_SECS,
            Some("institutional"),
            None,
            Some(true),
            DEFAULT_DEV_JWT_SECRET.as_bytes(),
        )
        .unwrap();
        let auth_hdr = format!("Bearer {}", token);

        let req = Request::builder()
            .method("GET")
            .uri("/sentiment?ticker=AAPL")
            .header(header::AUTHORIZATION, &auth_hdr)
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        // Verify sandbox response headers
        assert_eq!(
            res.headers()
                .get("X-FinText-Sandbox")
                .and_then(|h| h.to_str().ok()),
            Some("true")
        );
        assert_eq!(
            res.headers()
                .get("X-FinText-Mock-Version")
                .and_then(|h| h.to_str().ok()),
            Some(DEFAULT_SANDBOX_MOCK_VERSION)
        );
    }

    #[tokio::test]
    async fn test_provenance_unauthenticated_returns_401() {
        let app = create_app();

        let req = Request::builder()
            .method("GET")
            .uri("/provenance/sentiment/AAPL_2026-08-30T10:15:00Z")
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_provenance_invalid_record_type_returns_400() {
        let app = create_app();
        let token = generate_jwt(
            "prov_tester",
            DEFAULT_JWT_EXPIRY_SECS,
            Some("institutional"),
            DEFAULT_DEV_JWT_SECRET.as_bytes(),
        )
        .unwrap();
        let auth_hdr = format!("Bearer {}", token);

        let req = Request::builder()
            .method("GET")
            .uri("/provenance/invalid_type/AAPL_2026-08-30T10:15:00Z")
            .header(header::AUTHORIZATION, &auth_hdr)
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_provenance_sentiment_record_returns_200() {
        let app = create_app();
        let token = generate_jwt(
            "prov_tester",
            DEFAULT_JWT_EXPIRY_SECS,
            Some("institutional"),
            DEFAULT_DEV_JWT_SECRET.as_bytes(),
        )
        .unwrap();
        let auth_hdr = format!("Bearer {}", token);

        let req = Request::builder()
            .method("GET")
            .uri("/provenance/sentiment/AAPL_2026-08-30T10:15:00Z")
            .header(header::AUTHORIZATION, &auth_hdr)
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        let resp: DataProvenanceResponse = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(resp.record_type, "sentiment");
        assert_eq!(resp.record_id, "AAPL_2026-08-30T10:15:00Z");
        assert!(!resp.provenance_entries.is_empty());
        assert_eq!(resp.provenance_entries[0].source_type, "finnhub");
        assert_eq!(resp.provenance_entries[0].processing_steps.len(), 5);
    }

    #[tokio::test]
    async fn test_provenance_news_record_returns_200() {
        let app = create_app();
        let token = generate_jwt(
            "prov_tester",
            DEFAULT_JWT_EXPIRY_SECS,
            Some("institutional"),
            DEFAULT_DEV_JWT_SECRET.as_bytes(),
        )
        .unwrap();
        let auth_hdr = format!("Bearer {}", token);

        let req = Request::builder()
            .method("GET")
            .uri("/provenance/news/550e8400-e29b-41d4-a716-446655440000")
            .header(header::AUTHORIZATION, &auth_hdr)
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        let resp: DataProvenanceResponse = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(resp.record_type, "news");
        assert_eq!(resp.record_id, "550e8400-e29b-41d4-a716-446655440000");
        assert!(!resp.provenance_entries.is_empty());
        assert_eq!(resp.provenance_entries[0].source_type, "sec_edgar");
        assert_eq!(resp.provenance_entries[0].processing_steps.len(), 5);
    }

    #[tokio::test]
    async fn test_anomaly_scan_unauthenticated_returns_401() {
        let app = create_app();
        let req = Request::builder()
            .method("POST")
            .uri("/anomaly-scan")
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_anomaly_scan_authenticated_returns_200() {
        let app = create_app();
        let token = generate_jwt(
            "anomaly_tester",
            DEFAULT_JWT_EXPIRY_SECS,
            Some("institutional"),
            DEFAULT_DEV_JWT_SECRET.as_bytes(),
        )
        .unwrap();
        let auth_hdr = format!("Bearer {}", token);

        let req = Request::builder()
            .method("POST")
            .uri("/anomaly-scan")
            .header(header::AUTHORIZATION, &auth_hdr)
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_language_detect_unauthenticated_returns_401() {
        let app = create_app();
        let req = Request::builder()
            .method("GET")
            .uri("/language/detect?text=Hello+world")
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_language_detect_empty_text_returns_400() {
        let app = create_app();
        let token = generate_jwt(
            "lang_tester",
            DEFAULT_JWT_EXPIRY_SECS,
            Some("institutional"),
            DEFAULT_DEV_JWT_SECRET.as_bytes(),
        )
        .unwrap();
        let auth_hdr = format!("Bearer {}", token);

        let req = Request::builder()
            .method("GET")
            .uri("/language/detect?text=")
            .header(header::AUTHORIZATION, &auth_hdr)
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_language_detect_multilingual_success() {
        let app = create_app();
        let token = generate_jwt(
            "lang_tester",
            DEFAULT_JWT_EXPIRY_SECS,
            Some("institutional"),
            DEFAULT_DEV_JWT_SECRET.as_bytes(),
        )
        .unwrap();
        let auth_hdr = format!("Bearer {}", token);

        let req = Request::builder()
            .method("GET")
            .uri("/language/detect?text=El+mercado+de+valores+muestra+un+fuerte+crecimiento+en+las+acciones")
            .header(header::AUTHORIZATION, &auth_hdr)
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_model_card_public_endpoint_returns_200() {
        let app = create_app();

        let req = Request::builder()
            .method("GET")
            .uri("/model-card")
            .body(Body::empty())
            .unwrap();

        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        let card: models::ModelCardResponse = serde_json::from_slice(&bytes).unwrap();

        assert!(
            card.model_id == "fintext-sentiment-finbert-finetuned"
                || card.model_id == "fintext-sentiment-finbert"
                || card.model_id == "fintext-sentiment-minilm-l6-v2"
        );
        assert!(card.model_name.contains("FinBERT") || card.model_name.contains("MiniLM"));
        assert!(card.architecture == "bert" || card.architecture == "transformer_encoder");
        assert!(
            card.base_model == "ProsusAI/finbert"
                || card.base_model == "sentence-transformers/all-MiniLM-L6-v2"
        );
        assert_eq!(card.precision, "FP16");
        assert_eq!(card.quantization, "INT8_dynamic");
        assert!(card.sequence_length == 512 || card.sequence_length == 32);
        assert!(card.chunking_strategy.contains("sliding_window"));
        assert!(card.mean_latency_ms > 0.0);
        assert_eq!(card.hardware_requirements.cpu, "8 vCPU");
        assert_eq!(card.hardware_requirements.memory_gb, 4);
        assert!(!card.version_history.is_empty());
        assert!(
            card.licensing.model_license == "apache_2.0"
                || card.licensing.model_license == "internal_proprietary"
        );
    }

    #[tokio::test]
    async fn test_alpha_report_returns_200_with_valid_metrics() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header_for_user("alpha_trader_01");

        let payload = serde_json::json!({
            "tickers": ["AAPL", "MSFT"],
            "start_date": "2024-01-01",
            "end_date": "2024-12-31",
            "signal_config": {
                "signal_type": "sentiment",
                "threshold_long": 0.2,
                "threshold_short": -0.2,
                "holding_days": 5,
                "smoothing_window_days": 3
            },
            "benchmark_ticker": "SPY",
            "initial_capital": 1000000.0
        });

        let req = Request::builder()
            .method("POST")
            .uri("/signals/alpha-report")
            .header(auth_k, auth_v)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(serde_json::to_vec(&payload).unwrap()))
            .unwrap();

        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_alpha_report_validation_empty_tickers() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header_for_user("alpha_trader_02");

        let payload = serde_json::json!({
            "tickers": [],
            "start_date": "2024-01-01",
            "end_date": "2024-12-31",
            "signal_config": {
                "signal_type": "sentiment",
                "threshold_long": 0.2,
                "threshold_short": -0.2,
                "holding_days": 5
            }
        });

        let req = Request::builder()
            .method("POST")
            .uri("/signals/alpha-report")
            .header(auth_k, auth_v)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(serde_json::to_vec(&payload).unwrap()))
            .unwrap();

        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_alpha_report_validation_too_many_tickers() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header_for_user("alpha_trader_03");

        let tickers: Vec<String> = (0..11).map(|i| format!("T{}", i)).collect();
        let payload = serde_json::json!({
            "tickers": tickers,
            "start_date": "2024-01-01",
            "end_date": "2024-12-31",
            "signal_config": {
                "signal_type": "sentiment",
                "threshold_long": 0.2,
                "threshold_short": -0.2,
                "holding_days": 5
            }
        });

        let req = Request::builder()
            .method("POST")
            .uri("/signals/alpha-report")
            .header(auth_k, auth_v)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(serde_json::to_vec(&payload).unwrap()))
            .unwrap();

        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_pit_replay_returns_200_with_valid_params() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header_for_user("quant_researcher_01");

        let req = Request::builder()
            .method("GET")
            .uri("/pit/replay?ticker=AAPL&as_of_utc=2025-06-15T14:30:00Z&limit=10")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();

        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        let body = res.into_body().collect().await.unwrap().to_bytes();
        let replay: models::PITReplayResponse = serde_json::from_slice(&body).unwrap();

        assert_eq!(replay.ticker, "AAPL");
        assert_eq!(replay.as_of_utc, "2025-06-15T14:30:00+00:00");
        assert!(replay.replay_consistency.all_records_consistent);
        assert_eq!(replay.replay_consistency.violations_count, 0);
        assert_eq!(
            replay.summary.total_records,
            replay.news_articles.len()
                + replay.filings.len()
                + replay.events.len()
                + replay.sentiment_records.len()
        );
    }

    #[tokio::test]
    async fn test_pit_replay_validation_empty_ticker() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header_for_user("quant_researcher_02");

        let req = Request::builder()
            .method("GET")
            .uri("/pit/replay?ticker=&as_of_utc=2025-06-15T14:30:00Z")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();

        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_pit_replay_validation_invalid_timestamp() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header_for_user("quant_researcher_03");

        let req = Request::builder()
            .method("GET")
            .uri("/pit/replay?ticker=AAPL&as_of_utc=invalid-timestamp-format")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();

        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_signal_quality_returns_200_with_valid_params() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header_for_user("quant_alpha_evaluator_01");

        let payload = serde_json::json!({
            "signal_type": "sentiment",
            "tickers": ["AAPL", "MSFT", "NVDA"],
            "start_date": "2025-01-01",
            "end_date": "2025-06-30",
            "horizon_days": 5
        });

        let req = Request::builder()
            .method("POST")
            .uri("/signals/quality-report")
            .header(auth_k, auth_v)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(serde_json::to_vec(&payload).unwrap()))
            .unwrap();

        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        let body = res.into_body().collect().await.unwrap().to_bytes();
        let report: models::SignalQualityReportResponse = serde_json::from_slice(&body).unwrap();

        assert_eq!(report.signal_type, "sentiment");
        assert_eq!(report.tickers.len(), 3);
        assert!(report.coverage_pct > 0.0);
        assert!(report.freshness_avg_ms > 0.0);
        assert_eq!(report.decay_curve.len(), 6);
        assert!(report.half_life_days > 0.0);
        assert!(report.hit_rate_pct > 0.0);
        assert!(report.false_positive_rate_pct >= 0.0);
        assert!(report.ic_summary.icir.is_some());
        assert!(report.ic_summary.icir.unwrap().is_finite());
    }

    #[tokio::test]
    async fn test_signal_quality_validation_invalid_signal_type() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header_for_user("quant_alpha_evaluator_02");

        let payload = serde_json::json!({
            "signal_type": "unsupported_unknown_signal_family",
            "tickers": ["AAPL"],
            "start_date": "2025-01-01",
            "end_date": "2025-06-30"
        });

        let req = Request::builder()
            .method("POST")
            .uri("/signals/quality-report")
            .header(auth_k, auth_v)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(serde_json::to_vec(&payload).unwrap()))
            .unwrap();

        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_signal_quality_validation_too_many_tickers() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header_for_user("quant_alpha_evaluator_03");

        let tickers: Vec<String> = (0..21).map(|i| format!("TICK{}", i)).collect();
        let payload = serde_json::json!({
            "signal_type": "sentiment",
            "tickers": tickers,
            "start_date": "2025-01-01",
            "end_date": "2025-06-30"
        });

        let req = Request::builder()
            .method("POST")
            .uri("/signals/quality-report")
            .header(auth_k, auth_v)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(serde_json::to_vec(&payload).unwrap()))
            .unwrap();

        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_pit_certificate_returns_200_with_valid_params() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header_for_user("pit_compliance_officer_01");

        let req = Request::builder()
            .method("GET")
            .uri("/pit/certificate?dataset_version=2.1.0&universe=all&start_date=2025-06-01&end_date=2025-08-31")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();

        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        let body = res.into_body().collect().await.unwrap().to_bytes();
        let cert: PITCertificateResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(cert.dataset_version, "2.1.0");
        assert_eq!(cert.universe, "all");
        assert_eq!(cert.overall_result, "pass");
        assert!(cert.certificate_id.starts_with("PIT-CERT-"));
        assert!(cert.signature.starts_with("sha256:"));
        assert_eq!(cert.tests.signal_availability_ordering.status, "pass");
    }

    #[tokio::test]
    async fn test_pit_certificate_validation_invalid_start_date() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header_for_user("pit_compliance_officer_02");

        let req = Request::builder()
            .method("GET")
            .uri("/pit/certificate?start_date=invalid-date-format")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();

        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_pit_certificate_validation_start_after_end() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header_for_user("pit_compliance_officer_03");

        let req = Request::builder()
            .method("GET")
            .uri("/pit/certificate?start_date=2025-09-01&end_date=2025-01-01")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();

        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_provider_health_unauthorized_returns_401() {
        let app = create_app();

        let req = Request::builder()
            .method("GET")
            .uri("/providers/health")
            .body(Body::empty())
            .unwrap();

        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_provider_health_default_returns_200_all_providers() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header_for_user("provider_health_auditor_01");

        let req = Request::builder()
            .method("GET")
            .uri("/providers/health")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();

        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        let body = res.into_body().collect().await.unwrap().to_bytes();
        let resp: ProviderHealthResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(resp.providers.len(), 3);
        let names: Vec<String> = resp.providers.iter().map(|p| p.provider.clone()).collect();
        assert!(names.contains(&"sec_edgar".to_string()));
        assert!(names.contains(&"finnhub".to_string()));
        assert!(names.contains(&"polygon".to_string()));
        assert!(!resp.generated_at.is_empty());
    }

    #[tokio::test]
    async fn test_provider_health_filter_by_provider() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header_for_user("provider_health_auditor_02");

        let req = Request::builder()
            .method("GET")
            .uri("/providers/health?provider=finnhub&window_minutes=120")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();

        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        let body = res.into_body().collect().await.unwrap().to_bytes();
        let resp: ProviderHealthResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(resp.providers.len(), 1);
        assert_eq!(resp.providers[0].provider, "finnhub");
        assert!(
            resp.providers[0].success_rate_pct >= 0.0
                && resp.providers[0].success_rate_pct <= 100.0
        );
    }

    #[tokio::test]
    async fn test_provider_health_invalid_provider_returns_400() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header_for_user("provider_health_auditor_03");

        let req = Request::builder()
            .method("GET")
            .uri("/providers/health?provider=bloomberg")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();

        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_provider_health_invalid_window_minutes_returns_400() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header_for_user("provider_health_auditor_04");

        let req = Request::builder()
            .method("GET")
            .uri("/providers/health?window_minutes=0")
            .header(auth_k.clone(), auth_v.clone())
            .body(Body::empty())
            .unwrap();

        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);

        let (auth_k2, auth_v2) = test_auth_header_for_user("provider_health_auditor_04");
        let req2 = Request::builder()
            .method("GET")
            .uri("/providers/health?window_minutes=1441")
            .header(auth_k2, auth_v2)
            .body(Body::empty())
            .unwrap();

        let res2 = create_app().oneshot(req2).await.unwrap();
        assert_eq!(res2.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_model_validation_unauthorized_returns_401() {
        let app = create_app();

        let req = Request::builder()
            .method("GET")
            .uri("/model-validation")
            .body(Body::empty())
            .unwrap();

        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_model_validation_returns_200_with_valid_metrics() {
        let app = create_app();
        let (auth_k, auth_v) = test_auth_header_for_user("model_validation_auditor_01");

        let req = Request::builder()
            .method("GET")
            .uri("/model-validation")
            .header(auth_k, auth_v)
            .body(Body::empty())
            .unwrap();

        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        let body = res.into_body().collect().await.unwrap().to_bytes();
        let resp: ModelValidationResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(resp.model_id, "fintext-sentiment-finbert");
        assert_eq!(resp.model_version, "3.0.0");
        assert!(resp.dataset_size >= 100);
        assert!(resp.metrics.accuracy >= 0.80);
        assert!(resp.metrics.macro_f1 >= 0.80);
        assert_eq!(resp.calibration_curve.len(), 10);
        assert!(resp.expected_calibration_error <= 0.15);
    }
}
