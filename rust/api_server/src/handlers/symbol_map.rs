//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Permanent Security Identifier Resolution Handler
//! ═══════════════════════════════════════════════════════════════════════════════

use crate::auth::AuthErrorResponse;
use crate::models::{SymbolMapParams, SymbolMapResponse};
use crate::state::AppState;
use crate::symbol_map::SecurityIdentifiers;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};

/// Resolve Security Identifiers (Ticker, FIGI, CUSIP, ISIN).
///
/// Enables bi-directional translation across global financial security identifier formats
/// with automatic pattern inference or explicit format validation.
#[utoipa::path(
    get,
    path = "/symbols/map",
    tag = "Security Identifiers",
    params(
        ("identifier" = String, Query, description = "Security identifier to resolve (e.g., 'AAPL', 'BBG000B9XRY4', '037833100', 'US0378331005')"),
        ("input_type" = Option<String>, Query, description = "Optional input identifier format ('auto', 'ticker', 'figi', 'cusip', 'isin', default: 'auto')"),
        ("output_type" = Option<String>, Query, description = "Requested output format ('ticker', 'figi', 'cusip', 'isin', 'all', default: 'all')")
    ),
    responses(
        (status = 200, description = "Identifier mapped successfully", body = SymbolMapResponse),
        (status = 400, description = "Invalid or empty identifier or unsupported type format", body = AuthErrorResponse),
        (status = 401, description = "Unauthorized (missing or invalid Bearer JWT)", body = AuthErrorResponse),
        (status = 404, description = "Security identifier mapping not found", body = AuthErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::rate_limit::RateLimitErrorResponse)
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn get_symbol_map_handler(
    State(state): State<AppState>,
    Query(params): Query<SymbolMapParams>,
) -> Response {
    let raw_id = params.identifier.trim();
    if raw_id.is_empty() {
        let err = AuthErrorResponse {
            error: "Bad Request".to_string(),
            message: "identifier parameter cannot be empty".to_string(),
        };
        return (StatusCode::BAD_REQUEST, Json(err)).into_response();
    }

    let input_type_raw = params
        .input_type
        .as_deref()
        .unwrap_or("auto")
        .trim()
        .to_lowercase();
    let valid_input_types = ["auto", "ticker", "figi", "cusip", "isin"];
    if !valid_input_types.contains(&input_type_raw.as_str()) {
        let err = AuthErrorResponse {
            error: "Bad Request".to_string(),
            message: format!(
                "Invalid input_type '{}'. Allowed values: {:?}",
                input_type_raw, valid_input_types
            ),
        };
        return (StatusCode::BAD_REQUEST, Json(err)).into_response();
    }

    let output_type_raw = params
        .output_type
        .as_deref()
        .unwrap_or("all")
        .trim()
        .to_lowercase();
    let valid_output_types = ["ticker", "figi", "cusip", "isin", "all"];
    if !valid_output_types.contains(&output_type_raw.as_str()) {
        let err = AuthErrorResponse {
            error: "Bad Request".to_string(),
            message: format!(
                "Invalid output_type '{}'. Allowed values: {:?}",
                output_type_raw, valid_output_types
            ),
        };
        return (StatusCode::BAD_REQUEST, Json(err)).into_response();
    }

    let lookup_res = state.symbol_map.lookup(raw_id, Some(&input_type_raw));
    let (ids, resolved_input_type) = match lookup_res {
        Some(res) => res,
        None => {
            let err = AuthErrorResponse {
                error: "Not Found".to_string(),
                message: format!(
                    "No security identifier mapping found for '{}' (input_type: '{}')",
                    raw_id, input_type_raw
                ),
            };
            return (StatusCode::NOT_FOUND, Json(err)).into_response();
        }
    };

    let filtered_result = match output_type_raw.as_str() {
        "all" => ids,
        "ticker" => SecurityIdentifiers {
            ticker: ids.ticker,
            figi: None,
            cusip: None,
            isin: None,
        },
        "figi" => SecurityIdentifiers {
            ticker: None,
            figi: ids.figi,
            cusip: None,
            isin: None,
        },
        "cusip" => SecurityIdentifiers {
            ticker: None,
            figi: None,
            cusip: ids.cusip,
            isin: None,
        },
        "isin" => SecurityIdentifiers {
            ticker: None,
            figi: None,
            cusip: None,
            isin: ids.isin,
        },
        _ => ids,
    };

    let resp = SymbolMapResponse {
        input_identifier: raw_id.to_string(),
        input_type: resolved_input_type.to_string(),
        output_type: output_type_raw,
        result: filtered_result,
    };

    (StatusCode::OK, Json(resp)).into_response()
}
