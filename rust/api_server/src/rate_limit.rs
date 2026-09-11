//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Production Per-User Token Bucket Rate Limiter
//! ═══════════════════════════════════════════════════════════════════════════════

use crate::auth::Claims;
use crate::state::AppState;
use axum::extract::{Request, State};
use axum::http::header::HeaderName;
use axum::http::{HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};
use std::env;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tracing::{debug, warn};

pub const HEADER_RATELIMIT_LIMIT: HeaderName = HeaderName::from_static("x-ratelimit-limit");
pub const HEADER_RATELIMIT_REMAINING: HeaderName = HeaderName::from_static("x-ratelimit-remaining");
pub const HEADER_RATELIMIT_RESET: HeaderName = HeaderName::from_static("x-ratelimit-reset");
pub const HEADER_RETRY_AFTER: HeaderName = HeaderName::from_static("retry-after");

/// Global default requests per rate limit window.
pub const DEFAULT_RATE_LIMIT_REQUESTS: u32 = 100;
/// Global default window duration in seconds.
pub const DEFAULT_RATE_LIMIT_WINDOW_SECS: u64 = 60;

/// Rate limit configuration parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    pub requests_per_window: u32,
    pub window_seconds: u64,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        let requests_per_window = env::var("RATE_LIMIT_REQUESTS")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(DEFAULT_RATE_LIMIT_REQUESTS);

        let window_seconds = env::var("RATE_LIMIT_WINDOW_SECONDS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(DEFAULT_RATE_LIMIT_WINDOW_SECS);

        Self {
            requests_per_window,
            window_seconds,
        }
    }
}

/// Token bucket representation for an individual user session.
#[derive(Debug)]
struct UserBucket {
    tokens: f64,
    capacity: u32,
    refill_rate: f64, // tokens per second
    last_updated: Instant,
}

impl UserBucket {
    fn new(capacity: u32, window_seconds: u64) -> Self {
        let refill_rate = (capacity as f64) / (window_seconds.max(1) as f64);
        Self {
            tokens: capacity as f64,
            capacity,
            refill_rate,
            last_updated: Instant::now(),
        }
    }

    fn consume(&mut self) -> RateLimitStatus {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_updated).as_secs_f64();
        self.last_updated = now;

        // Refill tokens according to elapsed duration
        self.tokens = (self.tokens + elapsed * self.refill_rate).min(self.capacity as f64);

        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            let remaining = self.tokens.floor() as u32;
            let used_tokens = (self.capacity as f64) - self.tokens;
            let reset_seconds = if self.refill_rate > 0.0 {
                (used_tokens / self.refill_rate).ceil().max(1.0) as u64
            } else {
                1
            };

            RateLimitStatus {
                allowed: true,
                limit: self.capacity,
                remaining,
                reset_seconds,
            }
        } else {
            let deficit = 1.0 - self.tokens;
            let retry_after = if self.refill_rate > 0.0 {
                (deficit / self.refill_rate).ceil().max(1.0) as u64
            } else {
                1
            };

            RateLimitStatus {
                allowed: false,
                limit: self.capacity,
                remaining: 0,
                reset_seconds: retry_after,
            }
        }
    }
}

/// Result of evaluating a rate limit consumption attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RateLimitStatus {
    pub allowed: bool,
    pub limit: u32,
    pub remaining: u32,
    pub reset_seconds: u64,
}

/// Thread-safe, lock-free concurrent per-user rate limiter with TTL and capacity bounding.
#[derive(Debug, Clone)]
pub struct PerUserRateLimiter {
    config: RateLimitConfig,
    buckets: Arc<crate::cache::TtlCache<String, Arc<Mutex<UserBucket>>>>,
}

impl PerUserRateLimiter {
    /// Initialize with a custom rate limit configuration.
    pub fn new(config: RateLimitConfig) -> Self {
        let cache_cfg = crate::cache::CacheConfig::from_env_or_config();
        let ttl_secs = (config.window_seconds * 5).max(cache_cfg.default_ttl_secs);
        Self {
            config,
            buckets: Arc::new(crate::cache::TtlCache::with_ttl_secs(
                ttl_secs,
                cache_cfg.default_max_capacity,
            )),
        }
    }

    /// Read configuration from environment or defaults.
    pub fn from_env() -> Self {
        Self::new(RateLimitConfig::default())
    }

    /// Access active configuration.
    pub fn config(&self) -> &RateLimitConfig {
        &self.config
    }

    /// Check and consume one token for the given user ID.
    pub fn check_rate_limit(&self, user_id: &str) -> RateLimitStatus {
        let bucket_arc = self.buckets.get_or_insert_with(user_id.to_string(), || {
            Arc::new(Mutex::new(UserBucket::new(
                self.config.requests_per_window,
                self.config.window_seconds,
            )))
        });

        let mut bucket = bucket_arc.lock().unwrap();
        bucket.consume()
    }

    /// Reset quota for a specific user (useful for testing).
    pub fn reset_user(&self, user_id: &str) {
        self.buckets.remove(&user_id.to_string());
    }

    /// Remove expired inactive user buckets.
    pub fn remove_expired(&self) -> usize {
        self.buckets.remove_expired()
    }

    /// Current number of tracked user buckets.
    pub fn len(&self) -> usize {
        self.buckets.len()
    }

    /// Check if no user buckets are tracked.
    pub fn is_empty(&self) -> bool {
        self.buckets.is_empty()
    }
}

impl Default for PerUserRateLimiter {
    fn default() -> Self {
        Self::from_env()
    }
}

use utoipa::ToSchema;

/// JSON error response for rate limit violations.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RateLimitErrorResponse {
    /// Error category name
    #[schema(example = "Too Many Requests")]
    pub error: String,
    /// Detailed diagnostic message with quota and retry duration
    #[schema(example = "Rate limit exceeded. Quota: 100 requests per 60s. Retry after 3s.")]
    pub message: String,
}

/// Axum middleware function that enforces per-user rate limiting on authenticated requests.
///
/// Two layers of enforcement:
/// 1. **Per-second token bucket** — prevents burst abuse (existing).
/// 2. **Monthly plan quota** — enforces subscription-tier monthly request limits (new).
pub async fn rate_limit_middleware(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Result<Response, Response> {
    // 1. Identify user from JWT Claims injected by auth_middleware
    let user_id = req
        .extensions()
        .get::<Claims>()
        .map(|c| c.sub.clone())
        .unwrap_or_else(|| "anonymous".to_string());

    // 2. Evaluate per-second token bucket rate limit
    let status = state.rate_limiter.check_rate_limit(&user_id);

    if status.allowed {
        // 3. Monthly plan quota check (if database is available)
        if let Some(pool) = &state.db_pool {
            let quota_exceeded =
                check_monthly_quota(pool, &user_id, &state.monthly_quota_cache).await;

            if quota_exceeded {
                warn!(
                    "[RateLimit] HTTP 429 Monthly quota exceeded for user '{}'",
                    user_id
                );

                let err_body = Json(RateLimitErrorResponse {
                    error: "Too Many Requests".to_string(),
                    message: "Monthly quota exceeded. Please upgrade.".to_string(),
                });

                let response = (StatusCode::TOO_MANY_REQUESTS, err_body).into_response();
                return Err(response);
            }

            // Optimistically increment cached usage
            state.monthly_quota_cache.increment_usage(&user_id);
        }

        debug!(
            "[RateLimit] Allowed request for user '{}' (remaining: {}/{})",
            user_id, status.remaining, status.limit
        );

        let mut response = next.run(req).await;
        let headers = response.headers_mut();

        if let Ok(val) = HeaderValue::from_str(&status.limit.to_string()) {
            headers.insert(HEADER_RATELIMIT_LIMIT, val);
        }
        if let Ok(val) = HeaderValue::from_str(&status.remaining.to_string()) {
            headers.insert(HEADER_RATELIMIT_REMAINING, val);
        }
        if let Ok(val) = HeaderValue::from_str(&status.reset_seconds.to_string()) {
            headers.insert(HEADER_RATELIMIT_RESET, val);
        }

        Ok(response)
    } else {
        warn!(
            "[RateLimit] HTTP 429 Rate Limit Exceeded for user '{}'. Retry after {}s",
            user_id, status.reset_seconds
        );

        let err_body = Json(RateLimitErrorResponse {
            error: "Too Many Requests".to_string(),
            message: format!(
                "Rate limit exceeded. Quota: {} requests per {}s. Retry after {}s.",
                status.limit,
                state.rate_limiter.config().window_seconds,
                status.reset_seconds
            ),
        });

        let mut response = (StatusCode::TOO_MANY_REQUESTS, err_body).into_response();
        let headers = response.headers_mut();

        if let Ok(val) = HeaderValue::from_str(&status.limit.to_string()) {
            headers.insert(HEADER_RATELIMIT_LIMIT, val);
        }
        headers.insert(HEADER_RATELIMIT_REMAINING, HeaderValue::from_static("0"));
        if let Ok(val) = HeaderValue::from_str(&status.reset_seconds.to_string()) {
            headers.insert(HEADER_RATELIMIT_RESET, val.clone());
            headers.insert(HEADER_RETRY_AFTER, val);
        }

        Err(response)
    }
}

/// Checks whether the user has exceeded their monthly plan quota.
///
/// Uses an in-memory cache with 60-second TTL to avoid hitting PostgreSQL on every request.
/// Returns `true` if the user has exceeded their quota.
async fn check_monthly_quota(
    pool: &sqlx::PgPool,
    user_id: &str,
    cache: &crate::billing::MonthlyQuotaCache,
) -> bool {
    use crate::billing::{
        get_monthly_usage_count, get_plan_request_quota, get_user_subscription, QuotaCacheEntry,
    };

    // 1. Try cache first
    if let Some(entry) = cache.get(user_id) {
        return match entry.monthly_limit {
            Some(limit) => entry.current_usage >= limit,
            None => false, // Unlimited
        };
    }

    // 2. Cache miss — query database
    let plan_id = match get_user_subscription(pool, user_id).await {
        Ok(Some(sub)) if sub.status == "active" || sub.status == "past_due" => sub.plan_id,
        _ => "free".to_string(),
    };

    let monthly_limit = get_plan_request_quota(&plan_id);
    let current_usage = get_monthly_usage_count(pool, user_id).await.unwrap_or(0);

    // 3. Populate cache
    cache.set(
        user_id,
        QuotaCacheEntry {
            plan_id,
            monthly_limit,
            current_usage,
            cached_at: std::time::Instant::now(),
        },
    );

    match monthly_limit {
        Some(limit) => current_usage >= limit,
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;
    use std::time::Duration;

    #[test]
    fn test_rate_limiter_allows_within_capacity() {
        let limiter = PerUserRateLimiter::new(RateLimitConfig {
            requests_per_window: 5,
            window_seconds: 10,
        });

        for i in 0..5 {
            let res = limiter.check_rate_limit("trader_alpha");
            assert!(res.allowed);
            assert_eq!(res.limit, 5);
            assert_eq!(res.remaining, 4 - i);
        }
    }

    #[test]
    fn test_rate_limiter_blocks_when_quota_exhausted() {
        let limiter = PerUserRateLimiter::new(RateLimitConfig {
            requests_per_window: 3,
            window_seconds: 60,
        });

        assert!(limiter.check_rate_limit("trader_beta").allowed);
        assert!(limiter.check_rate_limit("trader_beta").allowed);
        assert!(limiter.check_rate_limit("trader_beta").allowed);

        // 4th request must be rejected
        let blocked = limiter.check_rate_limit("trader_beta");
        assert!(!blocked.allowed);
        assert_eq!(blocked.remaining, 0);
        assert!(blocked.reset_seconds >= 1);
    }

    #[test]
    fn test_rate_limiter_independent_user_quotas() {
        let limiter = PerUserRateLimiter::new(RateLimitConfig {
            requests_per_window: 2,
            window_seconds: 60,
        });

        assert!(limiter.check_rate_limit("user_1").allowed);
        assert!(limiter.check_rate_limit("user_1").allowed);
        assert!(!limiter.check_rate_limit("user_1").allowed);

        // user_2 should have full quota
        assert!(limiter.check_rate_limit("user_2").allowed);
        assert!(limiter.check_rate_limit("user_2").allowed);
        assert!(!limiter.check_rate_limit("user_2").allowed);
    }

    #[test]
    fn test_rate_limiter_refills_over_time() {
        let limiter = PerUserRateLimiter::new(RateLimitConfig {
            requests_per_window: 1,
            window_seconds: 1, // 1 token per second
        });

        assert!(limiter.check_rate_limit("trader_gamma").allowed);
        assert!(!limiter.check_rate_limit("trader_gamma").allowed);

        // Sleep 1.1 seconds to refill
        sleep(Duration::from_millis(1100));

        let refilled = limiter.check_rate_limit("trader_gamma");
        assert!(refilled.allowed);
    }
}
