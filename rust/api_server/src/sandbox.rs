//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Sandbox Environment & Mock Data Isolation Registry
//! ═══════════════════════════════════════════════════════════════════════════════

use crate::models::sandbox::{SandboxStatusResponse, DEFAULT_SANDBOX_MOCK_VERSION};
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::info;

/// Internal persistent or cached sandbox activation metadata.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SandboxInfo {
    pub user_id: String,
    pub active: bool,
    pub mock_data_version: String,
    pub activated_at: Option<DateTime<Utc>>,
    pub deactivated_at: Option<DateTime<Utc>>,
}

/// Request extension conveying whether the current incoming request is executing in sandbox mode.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SandboxContext {
    pub is_sandbox: bool,
    pub mock_version: String,
}

impl Default for SandboxContext {
    fn default() -> Self {
        Self {
            is_sandbox: false,
            mock_version: DEFAULT_SANDBOX_MOCK_VERSION.to_string(),
        }
    }
}

/// Thread-safe registry tracking per-user active sandbox state.
#[derive(Debug, Clone, Default)]
pub struct SandboxRegistry {
    sandboxes: Arc<DashMap<String, SandboxInfo>>,
}

impl SandboxRegistry {
    /// Creates a new, empty in-memory sandbox registry.
    pub fn new() -> Self {
        Self {
            sandboxes: Arc::new(DashMap::new()),
        }
    }

    /// List of key endpoints supported by the isolated mock simulation engine.
    pub fn available_mock_endpoints() -> Vec<String> {
        vec![
            "/sentiment".to_string(),
            "/sentiment/feed".to_string(),
            "/sentiment/anomalies".to_string(),
            "/sentiment/disagreement".to_string(),
            "/options/iv".to_string(),
            "/options/unusual".to_string(),
            "/options/vol-surface".to_string(),
            "/options/put-call-ratio".to_string(),
            "/spillovers".to_string(),
            "/spillovers/matrix".to_string(),
            "/backtest".to_string(),
            "/market/regime".to_string(),
            "/macro/breadth".to_string(),
            "/sla/status".to_string(),
        ]
    }

    /// Activates sandbox mode for a user ID.
    pub fn activate(&self, user_id: &str) -> SandboxInfo {
        let user_id = user_id.trim().to_string();
        let now = Utc::now();
        let info = SandboxInfo {
            user_id: user_id.clone(),
            active: true,
            mock_data_version: DEFAULT_SANDBOX_MOCK_VERSION.to_string(),
            activated_at: Some(now),
            deactivated_at: None,
        };
        self.sandboxes.insert(user_id.clone(), info.clone());
        info!("[Sandbox] Activated sandbox mode for user '{}'", user_id);
        info
    }

    /// Deactivates sandbox mode for a user ID.
    pub fn deactivate(&self, user_id: &str) -> SandboxInfo {
        let user_id = user_id.trim().to_string();
        let now = Utc::now();
        let prev = self.sandboxes.get(&user_id).map(|e| e.value().clone());
        let info = SandboxInfo {
            user_id: user_id.clone(),
            active: false,
            mock_data_version: DEFAULT_SANDBOX_MOCK_VERSION.to_string(),
            activated_at: prev.and_then(|p| p.activated_at),
            deactivated_at: Some(now),
        };
        self.sandboxes.insert(user_id.clone(), info.clone());
        info!("[Sandbox] Deactivated sandbox mode for user '{}'", user_id);
        info
    }

    /// Returns the current sandbox status for a user ID (defaulting to inactive if not activated).
    pub fn get_status(&self, user_id: &str) -> SandboxInfo {
        let user_id = user_id.trim().to_string();
        if let Some(entry) = self.sandboxes.get(&user_id) {
            entry.value().clone()
        } else {
            SandboxInfo {
                user_id,
                active: false,
                mock_data_version: DEFAULT_SANDBOX_MOCK_VERSION.to_string(),
                activated_at: None,
                deactivated_at: None,
            }
        }
    }

    /// Returns true if the user currently has an active sandbox session.
    pub fn is_active(&self, user_id: &str) -> bool {
        let user_id = user_id.trim();
        self.sandboxes
            .get(user_id)
            .map(|entry| entry.active)
            .unwrap_or(false)
    }

    /// Converts internal `SandboxInfo` to `SandboxStatusResponse`.
    pub fn to_response(info: &SandboxInfo) -> SandboxStatusResponse {
        let message = if info.active {
            "Sandbox mode is active. Requests will be served with isolated mock data and will not consume production quota."
                .to_string()
        } else {
            "Sandbox mode is inactive. Requests will be served with live production data stores."
                .to_string()
        };

        SandboxStatusResponse {
            active: info.active,
            mock_data_version: info.mock_data_version.clone(),
            activated_at: info.activated_at,
            deactivated_at: info.deactivated_at,
            available_endpoints: Self::available_mock_endpoints(),
            message,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sandbox_lifecycle() {
        let registry = SandboxRegistry::new();
        let user = "quant_fund_sandbox_01";

        // Initial state
        let status0 = registry.get_status(user);
        assert!(!status0.active);
        assert!(!registry.is_active(user));

        // Activate
        let status1 = registry.activate(user);
        assert!(status1.active);
        assert!(registry.is_active(user));
        assert!(status1.activated_at.is_some());
        assert_eq!(status1.mock_data_version, DEFAULT_SANDBOX_MOCK_VERSION);

        // Deactivate
        let status2 = registry.deactivate(user);
        assert!(!status2.active);
        assert!(!registry.is_active(user));
        assert!(status2.deactivated_at.is_some());

        // Response formatting
        let resp = SandboxRegistry::to_response(&status1);
        assert!(resp.active);
        assert!(!resp.available_endpoints.is_empty());
    }
}
