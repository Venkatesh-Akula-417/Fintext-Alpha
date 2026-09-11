//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Security Symbol Mapping Models
//! ═══════════════════════════════════════════════════════════════════════════════

use crate::symbol_map::SecurityIdentifiers;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Request query parameters for security identifier resolution (`GET /symbols/map`).
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct SymbolMapParams {
    /// The security identifier to map (e.g. 'AAPL', 'BBG000B9XRY4', '037833100', 'US0378331005')
    #[schema(example = "AAPL")]
    pub identifier: String,
    /// Identifier input format: 'auto', 'ticker', 'figi', 'cusip', 'isin' (default: 'auto')
    #[schema(example = "auto")]
    pub input_type: Option<String>,
    /// Desired output identifier type: 'ticker', 'figi', 'cusip', 'isin', 'all' (default: 'all')
    #[schema(example = "all")]
    pub output_type: Option<String>,
}

/// Response payload for security identifier resolution (`GET /symbols/map`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct SymbolMapResponse {
    /// Input query identifier
    #[schema(example = "AAPL")]
    pub input_identifier: String,
    /// Resolved input identifier format ('ticker', 'figi', 'cusip', 'isin')
    #[schema(example = "ticker")]
    pub input_type: String,
    /// Requested output identifier format
    #[schema(example = "all")]
    pub output_type: String,
    /// Resolved security identifier mapping
    pub result: SecurityIdentifiers,
}
