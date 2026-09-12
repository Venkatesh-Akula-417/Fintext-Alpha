//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Model Card & Lineage Governance Handler
//! ═══════════════════════════════════════════════════════════════════════════════

use axum::extract::State;
use axum::response::Json;

use crate::models::model_card::ModelCardResponse;
use crate::state::AppState;

/// Retrieve Standardized Model Card and Lineage Governance Report.
///
/// Returns the institutional model card documenting the exact neural architecture,
/// foundation base checkpoint, fine-tuning dataset, quantization, precision,
/// sequence length, latency benchmarks, hardware specifications, and version history.
#[utoipa::path(
    get,
    path = "/model-card",
    tag = "Model Governance",
    responses(
        (status = 200, description = "Standardized model card and lineage metadata report", body = ModelCardResponse),
    )
)]
pub async fn get_model_card_handler(State(_state): State<AppState>) -> Json<ModelCardResponse> {
    Json(ModelCardResponse::from_config_or_defaults())
}
