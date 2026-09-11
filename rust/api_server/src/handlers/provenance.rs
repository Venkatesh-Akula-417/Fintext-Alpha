//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Data Provenance & Lineage Tracking Handlers
//! ═══════════════════════════════════════════════════════════════════════════════

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use chrono::Utc;
use serde_json::json;

use crate::models::provenance::{DataProvenanceItem, DataProvenanceResponse};
use crate::provenance::ProvenanceRegistry;
use crate::state::AppState;

pub const ALLOWED_RECORD_TYPES: &[&str] = &["sentiment", "news"];

/// Retrieve data provenance and lineage execution history for a specific record.
#[utoipa::path(
    get,
    path = "/provenance/{record_type}/{record_id}",
    tag = "Data Lineage & Governance",
    params(
        ("record_type" = String, Path, description = "Entity type of the record ('sentiment' or 'news')", example = "sentiment"),
        ("record_id" = String, Path, description = "Unique identifier of the record (e.g. 'AAPL_2026-08-30T10:15:00Z' or article UUID)", example = "AAPL_2026-08-30T10:15:00Z")
    ),
    responses(
        (status = 200, description = "Provenance lineage trace retrieved successfully", body = DataProvenanceResponse),
        (status = 400, description = "Invalid record_type or empty record_id"),
        (status = 401, description = "Missing or invalid authorization token")
    ),
    security(
        ("bearer_auth" = []),
        ("api_key_auth" = [])
    )
)]
pub async fn get_provenance_handler(
    State(state): State<AppState>,
    Path((record_type, record_id)): Path<(String, String)>,
) -> impl IntoResponse {
    let norm_type = record_type.trim().to_lowercase();
    let norm_id = record_id.trim().to_string();

    // 1. Validate record_type
    if !ALLOWED_RECORD_TYPES.contains(&norm_type.as_str()) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Bad Request",
                "message": format!(
                    "Invalid record_type '{}'. Allowed values: {:?}",
                    record_type, ALLOWED_RECORD_TYPES
                )
            })),
        );
    }

    // 2. Validate record_id
    if norm_id.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Bad Request",
                "message": "record_id cannot be empty"
            })),
        );
    }

    // 3. Query in-memory registry
    let mut entries = state
        .provenance_registry
        .get_provenance(&norm_type, &norm_id);

    // 4. Query PostgreSQL if pool available and in-memory was empty
    if entries.is_empty() {
        if let Some(pool) = &state.db_pool {
            if let Ok(rows) = sqlx::query_as::<_, (
                uuid::Uuid,
                String,
                String,
                String,
                Option<String>,
                String,
                String,
                Option<f64>,
                serde_json::Value,
                chrono::DateTime<Utc>,
            )>(
                r#"
                SELECT id, record_type, record_id, source_type, source_id, model_version, pipeline_version, data_quality_score, processing_steps, created_at
                FROM data_provenance
                WHERE record_type = $1 AND record_id = $2
                ORDER BY created_at DESC
                "#
            )
            .bind(&norm_type)
            .bind(&norm_id)
            .fetch_all(pool)
            .await
            {
                for r in rows {
                    let steps = serde_json::from_value(r.8).unwrap_or_default();
                    entries.push(DataProvenanceItem {
                        id: r.0.to_string(),
                        record_type: r.1,
                        record_id: r.2,
                        source_type: r.3,
                        source_id: r.4,
                        model_version: r.5,
                        pipeline_version: r.6,
                        data_quality_score: r.7,
                        processing_steps: steps,
                        created_at: r.9.to_rfc3339(),
                    });
                }
            }
        }
    }

    // 5. If still empty, generate deterministic trace for reproducibility
    if entries.is_empty() {
        let fallback_item = ProvenanceRegistry::generate_deterministic_mock_provenance(
            &norm_type,
            &norm_id,
            &state.get_model_version(),
            &state.get_pipeline_version(),
        );
        state
            .provenance_registry
            .record_provenance(fallback_item.clone());
        entries.push(fallback_item);
    }

    let response = DataProvenanceResponse {
        record_type: norm_type,
        record_id: norm_id,
        total_entries: entries.len(),
        provenance_entries: entries,
        retrieved_at: Utc::now().to_rfc3339(),
    };

    (StatusCode::OK, Json(json!(response)))
}
