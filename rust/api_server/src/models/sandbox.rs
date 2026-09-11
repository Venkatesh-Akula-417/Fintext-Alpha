//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — API Sandbox Environment Models
//! ═══════════════════════════════════════════════════════════════════════════════

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Default sandbox mock dataset version identifier.
pub const DEFAULT_SANDBOX_MOCK_VERSION: &str = "sandbox-v1.0";

/// Response payload representing the active status and metadata of the API Sandbox environment.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct SandboxStatusResponse {
    /// Whether the sandbox environment is currently active for the authenticated user.
    #[schema(example = true)]
    pub active: bool,

    /// Identifier string for the simulated mock data catalog and versioning.
    #[schema(example = "sandbox-v1.0")]
    pub mock_data_version: String,

    /// Timestamp when sandbox mode was activated (if active).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub activated_at: Option<DateTime<Utc>>,

    /// Timestamp when sandbox mode was last deactivated (if deactivated).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deactivated_at: Option<DateTime<Utc>>,

    /// Key API endpoints available with high-fidelity isolated mock simulation.
    #[schema(example = json!(["/sentiment", "/sentiment/feed", "/options/iv", "/options/unusual", "/spillovers", "/backtest", "/market/regime", "/sla/status"]))]
    pub available_endpoints: Vec<String>,

    /// Human-readable operational status message.
    #[schema(
        example = "Sandbox mode is active. Requests will be served with isolated mock data and will not consume production quota."
    )]
    pub message: String,
}
