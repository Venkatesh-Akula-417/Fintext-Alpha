//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Ingestion Source Circuit Breaker Core
//! Problem #15: Per-Source Fault Isolation, Degraded State Machine & Zero Async Core
//! ═══════════════════════════════════════════════════════════════════════════════

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, RwLock};

/// 5 Static Data Sources — Bounded Cardinality Invariant.
/// Strictly restricted to these 5 sources to prevent Prometheus label explosion.
pub const VALID_RESILIENCE_SOURCES: [&str; 5] = [
    "sec_edgar",
    "fomc",
    "finnhub_ws",
    "polygon_ws",
    "corporate_actions",
];

/// Number of consecutive failures before Closed -> Open transition.
pub const FAILURE_STREAK: u32 = 5;

/// Sliding error evaluation window in milliseconds (60 seconds).
pub const WINDOW_MS: u64 = 60_000;

/// Error ratio threshold in sliding window (>= 50% errors triggers Open).
pub const ERROR_RATIO: f64 = 0.50;

/// Base backoff duration before first HalfOpen probe attempt (30 seconds).
pub const BASE_BACKOFF_MS: u64 = 30_000;

/// Maximum capped backoff duration under persistent outages (300 seconds / 5 minutes).
pub const MAX_BACKOFF_MS: u64 = 300_000;

/// Stale transition threshold for SEC Edgar during prolonged outage (10 minutes).
pub const SEC_EDGAR_STALE_THRESHOLD_MS: u64 = 600_000;

/// Circuit Breaker State Machine variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BreakerState {
    Closed,
    HalfOpen,
    Open,
}

impl BreakerState {
    pub fn as_u8(&self) -> u8 {
        match self {
            BreakerState::Closed => 0,
            BreakerState::HalfOpen => 1,
            BreakerState::Open => 2,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            BreakerState::Closed => "closed",
            BreakerState::HalfOpen => "half_open",
            BreakerState::Open => "open",
        }
    }
}

/// Operational Mode of the Ingestion Pipeline for a given source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IngestionMode {
    /// Normal primary streaming / real-time feed active.
    Primary,
    /// Degraded fallback mode active (e.g. WebSocket down -> polling REST).
    Degraded,
    /// Stale fallback mode active (serving cached / historical data with stale marker).
    Stale,
}

impl IngestionMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            IngestionMode::Primary => "primary",
            IngestionMode::Degraded => "degraded",
            IngestionMode::Stale => "stale",
        }
    }
}

/// Decision returned by `allow_request(now_ms)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AllowDecision {
    /// Normal execution permitted under Closed state.
    Allow,
    /// Exactly one probe permitted under HalfOpen state.
    Probe,
    /// Request denied under Open state or while HalfOpen probe is already in flight.
    Deny { retry_ms: u64 },
}

/// Pure-logic, zero-async circuit breaker state machine for an individual source.
#[derive(Debug, Clone)]
pub struct CircuitBreaker {
    pub source: String,
    state: BreakerState,
    consecutive_failures: u32,
    window: VecDeque<(u64, bool)>, // (epoch_ms, is_success)
    open_until_ms: u64,
    backoff_ms: u64,
    probe_in_flight: bool,
    last_success_ts: Option<u64>,
    first_failure_ms: Option<u64>,
    mode: IngestionMode,
    fallback_activations: u64,
}

impl CircuitBreaker {
    pub fn new(source: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            state: BreakerState::Closed,
            consecutive_failures: 0,
            window: VecDeque::new(),
            open_until_ms: 0,
            backoff_ms: BASE_BACKOFF_MS,
            probe_in_flight: false,
            last_success_ts: None,
            first_failure_ms: None,
            mode: IngestionMode::Primary,
            fallback_activations: 0,
        }
    }

    /// Evaluates whether an outbound request is permitted at the injected `now_ms`.
    pub fn allow_request(&mut self, now_ms: u64) -> AllowDecision {
        // Auto-promote Open -> HalfOpen when backoff duration elapses
        if self.state == BreakerState::Open && now_ms >= self.open_until_ms {
            self.transition_to(BreakerState::HalfOpen, now_ms, "backoff_timer_elapsed");
            self.probe_in_flight = false;
        }

        match self.state {
            BreakerState::Closed => AllowDecision::Allow,
            BreakerState::HalfOpen => {
                if !self.probe_in_flight {
                    self.probe_in_flight = true;
                    AllowDecision::Probe
                } else {
                    let retry_ms = self.open_until_ms.saturating_sub(now_ms).max(1000);
                    AllowDecision::Deny { retry_ms }
                }
            }
            BreakerState::Open => {
                let retry_ms = self.open_until_ms.saturating_sub(now_ms).max(1);
                AllowDecision::Deny { retry_ms }
            }
        }
    }

    /// Records a successful fetch or probe completion at `now_ms`.
    pub fn record_success(&mut self, now_ms: u64) {
        self.last_success_ts = Some(now_ms);
        self.first_failure_ms = None;

        match self.state {
            BreakerState::HalfOpen => {
                // Probe succeeded: restore to Closed
                self.probe_in_flight = false;
                self.consecutive_failures = 0;
                self.backoff_ms = BASE_BACKOFF_MS;
                self.window.clear();
                self.window.push_back((now_ms, true));
                self.mode = IngestionMode::Primary;
                self.transition_to(BreakerState::Closed, now_ms, "probe_success");
            }
            BreakerState::Closed => {
                self.consecutive_failures = 0;
                self.prune_window(now_ms);
                self.window.push_back((now_ms, true));
                self.mode = IngestionMode::Primary;
            }
            BreakerState::Open => {
                // Background fallback success (e.g. REST poll while WS is Open)
                // Mode remains degraded until WS probe closes breaker
            }
        }
    }

    /// Records a fetch failure or probe failure at `now_ms`.
    pub fn record_failure(&mut self, now_ms: u64, reason: &str) {
        if self.first_failure_ms.is_none() {
            self.first_failure_ms = Some(now_ms);
        }

        match self.state {
            BreakerState::HalfOpen => {
                // Probe failed: trip back to Open with doubled exponential backoff
                self.probe_in_flight = false;
                self.backoff_ms = std::cmp::min(self.backoff_ms.saturating_mul(2), MAX_BACKOFF_MS);
                self.open_until_ms = now_ms.saturating_add(self.backoff_ms);
                self.fallback_activations += 1;
                self.update_mode_for_outage(now_ms);
                self.transition_to(BreakerState::Open, now_ms, reason);
            }
            BreakerState::Closed => {
                self.consecutive_failures = self.consecutive_failures.saturating_add(1);
                self.prune_window(now_ms);
                self.window.push_back((now_ms, false));

                let error_count = self.window.iter().filter(|(_, ok)| !ok).count();
                let window_len = self.window.len();
                let error_ratio = if window_len > 0 {
                    error_count as f64 / window_len as f64
                } else {
                    0.0
                };

                /// Minimum requests in sliding window before evaluating error ratio (prevents premature trips on sparse samples).
                pub const MIN_WINDOW_REQUESTS: usize = 6;

                let should_trip = self.consecutive_failures >= FAILURE_STREAK
                    || (window_len >= MIN_WINDOW_REQUESTS && error_ratio >= ERROR_RATIO);

                if should_trip {
                    self.backoff_ms = BASE_BACKOFF_MS;
                    self.open_until_ms = now_ms.saturating_add(self.backoff_ms);
                    self.fallback_activations += 1;
                    self.update_mode_for_outage(now_ms);
                    let trip_reason = if self.consecutive_failures >= FAILURE_STREAK {
                        format!("consecutive_failure_streak_{}", self.consecutive_failures)
                    } else {
                        format!("error_ratio_{:.2}_exceeded_{:.2}", error_ratio, ERROR_RATIO)
                    };
                    self.transition_to(BreakerState::Open, now_ms, &trip_reason);
                }
            }
            BreakerState::Open => {
                // Repeated failures while open update mode (e.g. sec_edgar reaching 10min stale threshold)
                self.update_mode_for_outage(now_ms);
            }
        }
    }

    /// Determines the active degraded or stale mode based on source-specific policy.
    fn update_mode_for_outage(&mut self, now_ms: u64) {
        match self.source.as_str() {
            "finnhub_ws" | "polygon_ws" => {
                // WebSocket sources degrade to REST polling
                self.mode = IngestionMode::Degraded;
            }
            "fomc" | "corporate_actions" => {
                // Static regulatory/calendar sources serve cached/historical snapshots marked stale
                self.mode = IngestionMode::Stale;
            }
            "sec_edgar" => {
                // SEC Edgar retries with backoff (Degraded); transitions to Stale after 10 minutes
                if let Some(first_fail) = self.first_failure_ms {
                    if now_ms.saturating_sub(first_fail) >= SEC_EDGAR_STALE_THRESHOLD_MS {
                        self.mode = IngestionMode::Stale;
                    } else {
                        self.mode = IngestionMode::Degraded;
                    }
                } else {
                    self.mode = IngestionMode::Degraded;
                }
            }
            _ => {
                self.mode = IngestionMode::Degraded;
            }
        }
    }

    /// Emits structured JSON log to stderr on state transitions without secrets or payloads.
    fn transition_to(&mut self, new_state: BreakerState, now_ms: u64, reason: &str) {
        if self.state != new_state {
            let from_str = self.state.as_str();
            let to_str = new_state.as_str();
            let log_entry = serde_json::json!({
                "ts": now_ms,
                "source": self.source,
                "from": from_str,
                "to": to_str,
                "reason": reason,
            });
            eprintln!("{}", log_entry);
            self.state = new_state;
        }
    }

    /// Prunes window entries older than 60 seconds (WINDOW_MS).
    fn prune_window(&mut self, now_ms: u64) {
        while let Some(&(ts, _)) = self.window.front() {
            if now_ms.saturating_sub(ts) > WINDOW_MS {
                self.window.pop_front();
            } else {
                break;
            }
        }
    }

    // ── Inspection Accessors ─────────────────────────────────────────────────

    pub fn state(&mut self, now_ms: u64) -> BreakerState {
        if self.state == BreakerState::Open && now_ms >= self.open_until_ms {
            self.transition_to(BreakerState::HalfOpen, now_ms, "backoff_timer_elapsed");
            self.probe_in_flight = false;
        }
        self.state
    }

    pub fn current_state(&self) -> BreakerState {
        self.state
    }

    pub fn consecutive_failures(&self) -> u32 {
        self.consecutive_failures
    }

    pub fn last_success_ts(&self) -> Option<u64> {
        self.last_success_ts
    }

    pub fn mode(&self) -> IngestionMode {
        self.mode
    }

    pub fn set_mode(&mut self, mode: IngestionMode) {
        self.mode = mode;
    }

    pub fn backoff_s(&self, now_ms: u64) -> u64 {
        match self.state {
            BreakerState::Open => self.open_until_ms.saturating_sub(now_ms) / 1000,
            _ => 0,
        }
    }

    pub fn configured_backoff_ms(&self) -> u64 {
        self.backoff_ms
    }

    pub fn fallback_activations(&self) -> u64 {
        self.fallback_activations
    }

    pub fn is_fallback_active(&self) -> bool {
        self.mode != IngestionMode::Primary
    }
}

/// Thread-safe registry containing circuit breakers for all 5 institutional sources.
#[derive(Debug, Clone)]
pub struct BreakerRegistry {
    breakers: Arc<HashMap<String, Arc<RwLock<CircuitBreaker>>>>,
}

impl Default for BreakerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl BreakerRegistry {
    pub fn new() -> Self {
        let mut map = HashMap::new();
        for &src in &VALID_RESILIENCE_SOURCES {
            map.insert(
                src.to_string(),
                Arc::new(RwLock::new(CircuitBreaker::new(src))),
            );
        }
        Self {
            breakers: Arc::new(map),
        }
    }

    pub fn get(&self, source: &str) -> Option<Arc<RwLock<CircuitBreaker>>> {
        self.breakers.get(source).cloned()
    }

    pub fn allow_request(&self, source: &str, now_ms: u64) -> AllowDecision {
        if let Some(b) = self.breakers.get(source) {
            if let Ok(mut breaker) = b.write() {
                return breaker.allow_request(now_ms);
            }
        }
        AllowDecision::Allow
    }

    pub fn record_success(&self, source: &str, now_ms: u64) {
        if let Some(b) = self.breakers.get(source) {
            if let Ok(mut breaker) = b.write() {
                breaker.record_success(now_ms);
            }
        }
    }

    pub fn record_failure(&self, source: &str, now_ms: u64, reason: &str) {
        if let Some(b) = self.breakers.get(source) {
            if let Ok(mut breaker) = b.write() {
                breaker.record_failure(now_ms, reason);
            }
        }
    }

    pub fn get_mode(&self, source: &str) -> IngestionMode {
        if let Some(b) = self.breakers.get(source) {
            if let Ok(breaker) = b.read() {
                return breaker.mode();
            }
        }
        IngestionMode::Primary
    }

    pub fn set_mode(&self, source: &str, mode: IngestionMode) {
        if let Some(b) = self.breakers.get(source) {
            if let Ok(mut breaker) = b.write() {
                breaker.set_mode(mode);
            }
        }
    }

    /// Formats sorted JSON object for internal 9102 `/providers` endpoint.
    /// Output contains strictly zero credentials or customer payloads.
    pub fn get_provider_health_json(&self, now_ms: u64) -> serde_json::Value {
        let mut sorted_sources = VALID_RESILIENCE_SOURCES.to_vec();
        sorted_sources.sort();

        let mut map = serde_json::Map::new();
        for &src in &sorted_sources {
            if let Some(b) = self.breakers.get(src) {
                if let Ok(mut breaker) = b.write() {
                    let state = breaker.state(now_ms);
                    let info = serde_json::json!({
                        "state": state.as_str(),
                        "consecutive_failures": breaker.consecutive_failures(),
                        "last_success_ts": breaker.last_success_ts().map(|t| t / 1000),
                        "mode": breaker.mode().as_str(),
                        "backoff_s": breaker.backoff_s(now_ms),
                    });
                    map.insert(src.to_string(), info);
                }
            }
        }
        serde_json::Value::Object(map)
    }

    /// Returns list of sources with their state, fallback active flag, and total activations.
    /// Returns list of sources with their state, fallback active flag, and total activations (sorted).
    pub fn get_telemetry_snapshot(&self, now_ms: u64) -> Vec<(String, u8, u8, u64)> {
        let mut sorted_sources = VALID_RESILIENCE_SOURCES.to_vec();
        sorted_sources.sort();
        let mut out = Vec::new();
        for &src in &sorted_sources {
            if let Some(b) = self.breakers.get(src) {
                if let Ok(mut breaker) = b.write() {
                    let state_val = breaker.state(now_ms).as_u8();
                    let fallback_val = if breaker.is_fallback_active() { 1 } else { 0 };
                    let activations = breaker.fallback_activations();
                    out.push((src.to_string(), state_val, fallback_val, activations));
                }
            }
        }
        out
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit Tests (Injected Clock, Zero Sleeps)
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_breaker_initial_state_closed() {
        let mut breaker = CircuitBreaker::new("finnhub_ws");
        assert_eq!(breaker.current_state(), BreakerState::Closed);
        assert_eq!(breaker.allow_request(1000), AllowDecision::Allow);
        assert_eq!(breaker.consecutive_failures(), 0);
        assert_eq!(breaker.mode(), IngestionMode::Primary);
    }

    #[test]
    fn test_breaker_trips_on_consecutive_failure_streak() {
        let mut breaker = CircuitBreaker::new("finnhub_ws");
        let start_ms = 1_000_000u64;

        // 4 failures should not trip
        for i in 1..=4 {
            breaker.record_failure(start_ms + i * 1000, "socket_timeout");
            assert_eq!(breaker.current_state(), BreakerState::Closed);
            assert_eq!(
                breaker.allow_request(start_ms + i * 1000 + 100),
                AllowDecision::Allow
            );
        }

        // 5th failure must trip to Open
        breaker.record_failure(start_ms + 5000, "socket_timeout");
        assert_eq!(breaker.current_state(), BreakerState::Open);
        assert_eq!(breaker.mode(), IngestionMode::Degraded);
        assert_eq!(breaker.consecutive_failures(), 5);

        // Immediate subsequent request must be fast-denied
        match breaker.allow_request(start_ms + 5100) {
            AllowDecision::Deny { retry_ms } => {
                assert!(retry_ms > 0 && retry_ms <= BASE_BACKOFF_MS);
            }
            other => panic!("Expected Deny, got {:?}", other),
        }
    }

    #[test]
    fn test_breaker_trips_on_sliding_window_error_ratio() {
        let mut breaker = CircuitBreaker::new("polygon_ws");
        let t0 = 100_000u64;

        // 3 successes, then 3 failures (50% ratio, len=6 >= MIN_WINDOW_REQUESTS) -> should trip
        for i in 0..3 {
            breaker.record_success(t0 + i * 1000);
            assert_eq!(breaker.current_state(), BreakerState::Closed);
        }
        for i in 3..5 {
            breaker.record_failure(t0 + i * 1000, "http_502");
            assert_eq!(breaker.current_state(), BreakerState::Closed);
        }
        // 6th request (3rd failure out of 6 -> 50% error ratio) trips breaker
        breaker.record_failure(t0 + 5000, "http_502");

        assert_eq!(breaker.current_state(), BreakerState::Open);
        assert_eq!(breaker.mode(), IngestionMode::Degraded);
    }

    #[test]
    fn test_breaker_backoff_doubling_and_cap() {
        let mut breaker = CircuitBreaker::new("finnhub_ws");
        let mut t = 1_000_000u64;

        // Trip breaker to Open (backoff = 30s)
        for _ in 0..5 {
            breaker.record_failure(t, "err");
            t += 1000;
        }
        assert_eq!(breaker.current_state(), BreakerState::Open);
        assert_eq!(breaker.configured_backoff_ms(), 30_000);

        // Advance clock past 30s -> HalfOpen
        t += 30_001;
        assert_eq!(breaker.allow_request(t), AllowDecision::Probe);
        assert_eq!(breaker.current_state(), BreakerState::HalfOpen);

        // Probe fails -> back to Open with doubled backoff (60s)
        breaker.record_failure(t, "probe_failed");
        assert_eq!(breaker.current_state(), BreakerState::Open);
        assert_eq!(breaker.configured_backoff_ms(), 60_000);

        // Advance 60s -> HalfOpen again
        t += 60_001;
        assert_eq!(breaker.allow_request(t), AllowDecision::Probe);

        // Probe fails -> backoff doubles to 120s
        breaker.record_failure(t, "probe_failed");
        assert_eq!(breaker.configured_backoff_ms(), 120_000);

        // Advance 120s -> HalfOpen again
        t += 120_001;
        assert_eq!(breaker.allow_request(t), AllowDecision::Probe);

        // Probe fails -> backoff doubles to 240s
        breaker.record_failure(t, "probe_failed");
        assert_eq!(breaker.configured_backoff_ms(), 240_000);

        // Advance 240s -> HalfOpen again
        t += 240_001;
        assert_eq!(breaker.allow_request(t), AllowDecision::Probe);

        // Probe fails -> backoff doubles to 480s capped at 300s (MAX_BACKOFF_MS)
        breaker.record_failure(t, "probe_failed");
        assert_eq!(breaker.configured_backoff_ms(), MAX_BACKOFF_MS);
    }

    #[test]
    fn test_breaker_half_open_admits_single_probe() {
        let mut breaker = CircuitBreaker::new("polygon_ws");
        let mut t = 500_000u64;

        // Trip to Open
        for _ in 0..5 {
            breaker.record_failure(t, "err");
            t += 1000;
        }

        // Advance past backoff
        t += 30_001;

        // First request is allowed as Probe
        assert_eq!(breaker.allow_request(t), AllowDecision::Probe);
        assert_eq!(breaker.current_state(), BreakerState::HalfOpen);

        // Second concurrent request while probe in flight must be Denied
        match breaker.allow_request(t + 50) {
            AllowDecision::Deny { .. } => {}
            other => panic!("Expected Deny for concurrent probe, got {:?}", other),
        }

        // Probe succeeds -> Closed
        breaker.record_success(t + 200);
        assert_eq!(breaker.current_state(), BreakerState::Closed);
        assert_eq!(breaker.mode(), IngestionMode::Primary);
        assert_eq!(breaker.allow_request(t + 300), AllowDecision::Allow);
    }

    #[test]
    fn test_breaker_window_pruning_and_expiry() {
        let mut breaker = CircuitBreaker::new("fomc");
        let t0 = 1_000_000u64;

        // Record 1 failure at t0
        breaker.record_failure(t0, "err");

        // Record 1 success 65 seconds later (past WINDOW_MS = 60s)
        breaker.record_success(t0 + 65_000);

        // Breaker should have pruned the t0 failure, so error ratio is 0%
        assert_eq!(breaker.current_state(), BreakerState::Closed);
        assert_eq!(breaker.consecutive_failures(), 0);
    }

    #[test]
    fn test_breaker_per_source_isolation() {
        let registry = BreakerRegistry::new();
        let t = 1_000_000u64;

        // Trip finnhub_ws
        for _ in 0..5 {
            registry.record_failure("finnhub_ws", t, "ws_disconnect");
        }

        // finnhub_ws must be Open
        match registry.allow_request("finnhub_ws", t + 100) {
            AllowDecision::Deny { .. } => {}
            other => panic!("Expected Deny for finnhub_ws, got {:?}", other),
        }

        // polygon_ws and sec_edgar must remain completely unaffected (Closed / Allow)
        assert_eq!(
            registry.allow_request("polygon_ws", t + 100),
            AllowDecision::Allow
        );
        assert_eq!(
            registry.allow_request("sec_edgar", t + 100),
            AllowDecision::Allow
        );
        assert_eq!(registry.get_mode("polygon_ws"), IngestionMode::Primary);
        assert_eq!(registry.get_mode("finnhub_ws"), IngestionMode::Degraded);
    }

    #[test]
    fn test_breaker_mode_labeling_and_transitions() {
        let mut sec_breaker = CircuitBreaker::new("sec_edgar");
        let mut fomc_breaker = CircuitBreaker::new("fomc");
        let t = 1_000_000u64;

        // Trip sec_edgar: initially Degraded
        for _ in 0..5 {
            sec_breaker.record_failure(t, "sec_503");
        }
        assert_eq!(sec_breaker.mode(), IngestionMode::Degraded);

        // After 10 minutes (600_000 ms) of outage, sec_edgar becomes Stale
        sec_breaker.record_failure(t + 600_001, "sec_still_down");
        assert_eq!(sec_breaker.mode(), IngestionMode::Stale);

        // FOMC trips directly into Stale mode (calendar cache)
        for _ in 0..5 {
            fomc_breaker.record_failure(t, "fed_site_down");
        }
        assert_eq!(fomc_breaker.mode(), IngestionMode::Stale);
    }
}
