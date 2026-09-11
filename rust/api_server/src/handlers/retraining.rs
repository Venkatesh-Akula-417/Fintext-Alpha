//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Model Retraining Pipeline Route Handlers
//! ═══════════════════════════════════════════════════════════════════════════════

use axum::{
    extract::{Extension, Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use chrono::Utc;
use serde_json::json;
use tracing::info;
use uuid::Uuid;

use crate::audit_logs::log_audit_event;
use crate::auth::Claims;
use crate::models::{
    CreateRetrainingJobRequest, ListRetrainingJobsQuery, ListRetrainingJobsResponse, RetrainingJob,
    RetrainingJobResponse,
};
use crate::retraining::{
    is_valid_model_type, is_valid_trigger_type, trigger_immediate_job_processing,
    VALID_MODEL_TYPES, VALID_TRIGGER_TYPES,
};
use crate::state::AppState;

/// POST /retraining/jobs
///
/// Submit a new machine learning model retraining job.
#[utoipa::path(
    post,
    path = "/retraining/jobs",
    tag = "Model Retraining",
    request_body = CreateRetrainingJobRequest,
    responses(
        (status = 201, description = "Retraining job queued successfully", body = RetrainingJobResponse),
        (status = 400, description = "Invalid model_type, trigger_type, or config payload"),
        (status = 401, description = "Unauthorized - Missing or invalid Bearer JWT"),
    ),
    security(("BearerAuth" = []))
)]
pub async fn create_retraining_job_handler(
    Extension(claims): Extension<Claims>,
    State(state): State<AppState>,
    Json(payload): Json<CreateRetrainingJobRequest>,
) -> Response {
    let model_type = payload
        .model_type
        .as_deref()
        .unwrap_or("sentiment")
        .trim()
        .to_lowercase();

    if !is_valid_model_type(&model_type) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Bad Request",
                "message": format!(
                    "Invalid model_type '{}'. Allowed values: {:?}",
                    payload.model_type.unwrap_or_default(),
                    VALID_MODEL_TYPES
                )
            })),
        )
            .into_response();
    }

    let trigger_type = payload
        .trigger_type
        .as_deref()
        .unwrap_or("manual")
        .trim()
        .to_lowercase();

    if !is_valid_trigger_type(&trigger_type) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Bad Request",
                "message": format!(
                    "Invalid trigger_type '{}'. Allowed values: {:?}",
                    payload.trigger_type.unwrap_or_default(),
                    VALID_TRIGGER_TYPES
                )
            })),
        )
            .into_response();
    }

    let mut config = match payload.config {
        Some(c) => {
            if !c.is_object() {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({
                        "error": "Bad Request",
                        "message": "Config must be a JSON object containing hyperparameters"
                    })),
                )
                    .into_response();
            }
            c
        }
        None => json!({}),
    };

    // Check epochs
    let epochs = payload.epochs.or_else(|| {
        config
            .get("epochs")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32)
    });
    if let Some(ep) = epochs {
        if ep == 0 || ep > 100 {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Bad Request",
                    "message": format!("Invalid epochs '{}'. Must be between 1 and 100", ep)
                })),
            )
                .into_response();
        }
        if let Some(obj) = config.as_object_mut() {
            obj.insert("epochs".to_string(), json!(ep));
        }
    }

    // Check batch_size
    let batch_size = payload.batch_size.or_else(|| {
        config
            .get("batch_size")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
    });
    if let Some(bs) = batch_size {
        if bs == 0 || bs > 512 {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Bad Request",
                    "message": format!("Invalid batch_size '{}'. Must be between 1 and 512", bs)
                })),
            )
                .into_response();
        }
        if let Some(obj) = config.as_object_mut() {
            obj.insert("batch_size".to_string(), json!(bs));
        }
    }

    // Check learning_rate
    let learning_rate = payload
        .learning_rate
        .or_else(|| config.get("learning_rate").and_then(|v| v.as_f64()));
    if let Some(lr) = learning_rate {
        if lr <= 0.0 || lr > 1.0 {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Bad Request",
                    "message": format!("Invalid learning_rate '{}'. Must be > 0.0 and <= 1.0", lr)
                })),
            )
                .into_response();
        }
        if let Some(obj) = config.as_object_mut() {
            obj.insert("learning_rate".to_string(), json!(lr));
        }
    }

    // Merge dataset_path if provided
    if let Some(dp) = payload.dataset_path {
        if let Some(obj) = config.as_object_mut() {
            obj.insert("dataset_path".to_string(), json!(dp));
        }
    }

    let org_uuid = claims
        .org_id
        .as_deref()
        .and_then(|s| Uuid::parse_str(s).ok());
    let job_id = Uuid::new_v4();

    let job = RetrainingJob {
        id: job_id,
        org_id: org_uuid,
        user_id: claims.sub.clone(),
        model_type,
        trigger_type,
        status: "pending".to_string(),
        config: config.clone(),
        created_at: Utc::now(),
        started_at: None,
        completed_at: None,
        metrics: None,
        error_message: None,
    };

    state.retraining_registry.insert(job.clone());

    info!(
        "[Model Retraining] Created job {} for user '{}' (org: {:?})",
        job_id, claims.sub, org_uuid
    );

    // Trigger immediate background processing
    trigger_immediate_job_processing(&state, job_id);

    (
        StatusCode::CREATED,
        Json(RetrainingJobResponse {
            job,
            message: "Retraining job created successfully and queued for execution".to_string(),
        }),
    )
        .into_response()
}

/// GET /retraining/jobs
///
/// List historical and active model retraining jobs with pagination and status filters.
#[utoipa::path(
    get,
    path = "/retraining/jobs",
    tag = "Model Retraining",
    params(ListRetrainingJobsQuery),
    responses(
        (status = 200, description = "Retraining jobs retrieved successfully", body = ListRetrainingJobsResponse),
        (status = 401, description = "Unauthorized - Missing or invalid Bearer JWT"),
    ),
    security(("BearerAuth" = []))
)]
pub async fn list_retraining_jobs_handler(
    Extension(claims): Extension<Claims>,
    State(state): State<AppState>,
    Query(query): Query<ListRetrainingJobsQuery>,
) -> Response {
    let org_uuid = claims
        .org_id
        .as_deref()
        .and_then(|s| Uuid::parse_str(s).ok());
    let is_admin = claims.role == "admin" || claims.role == "institutional";

    let limit = query.limit.unwrap_or(50).clamp(1, 100);
    let offset = query.offset.unwrap_or(0);

    let (jobs, total) = state
        .retraining_registry
        .list(&claims.sub, org_uuid, &query, is_admin);

    Json(ListRetrainingJobsResponse {
        jobs,
        total,
        limit,
        offset,
    })
    .into_response()
}

/// GET /retraining/jobs/{id}
///
/// Get detailed status and logs for a specific model retraining job.
#[utoipa::path(
    get,
    path = "/retraining/jobs/{id}",
    tag = "Model Retraining",
    params(
        ("id" = Uuid, Path, description = "Unique retraining job identifier (UUID)")
    ),
    responses(
        (status = 200, description = "Retraining job details returned", body = RetrainingJob),
        (status = 401, description = "Unauthorized - Missing or invalid Bearer JWT"),
        (status = 403, description = "Forbidden - Caller does not own this job"),
        (status = 404, description = "Retraining job not found"),
    ),
    security(("BearerAuth" = []))
)]
pub async fn get_retraining_job_handler(
    Extension(claims): Extension<Claims>,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Response {
    let job = match state.retraining_registry.get(&id) {
        Some(j) => j,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "error": "Not Found",
                    "message": format!("Retraining job '{}' not found", id)
                })),
            )
                .into_response();
        }
    };

    let org_uuid = claims
        .org_id
        .as_deref()
        .and_then(|s| Uuid::parse_str(s).ok());
    let is_admin = claims.role == "admin" || claims.role == "institutional";

    if !is_admin && job.user_id != claims.sub && job.org_id != org_uuid {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({
                "error": "Forbidden",
                "message": "Access denied: You do not have permission to view this retraining job"
            })),
        )
            .into_response();
    }

    Json(RetrainingJobResponse {
        job,
        message: "Retraining job retrieved successfully".to_string(),
    })
    .into_response()
}

/// POST /retraining/jobs/{id}/cancel
///
/// Cancel an active or pending model retraining job.
#[utoipa::path(
    post,
    path = "/retraining/jobs/{id}/cancel",
    tag = "Model Retraining",
    params(
        ("id" = Uuid, Path, description = "Unique retraining job identifier (UUID)")
    ),
    responses(
        (status = 200, description = "Retraining job cancelled successfully", body = RetrainingJobResponse),
        (status = 400, description = "Job cannot be cancelled in its current state"),
        (status = 401, description = "Unauthorized - Missing or invalid Bearer JWT"),
        (status = 403, description = "Forbidden - Caller does not own this job"),
        (status = 404, description = "Retraining job not found"),
    ),
    security(("BearerAuth" = []))
)]
pub async fn cancel_retraining_job_handler(
    Extension(claims): Extension<Claims>,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Response {
    let org_uuid = claims
        .org_id
        .as_deref()
        .and_then(|s| Uuid::parse_str(s).ok());
    let is_admin = claims.role == "admin" || claims.role == "institutional";

    match state
        .retraining_registry
        .cancel(&id, &claims.sub, org_uuid, is_admin)
    {
        Ok(job) => {
            log_audit_event(
                &state,
                job.org_id,
                &claims.sub,
                "model.retraining_cancelled",
                "retraining_job",
                Some(&id.to_string()),
                json!({
                    "model_type": job.model_type,
                    "reason": "Cancelled by user request"
                }),
                None,
            )
            .await;

            Json(RetrainingJobResponse {
                job,
                message: "Retraining job was successfully cancelled".to_string(),
            })
            .into_response()
        }
        Err(err_msg) => {
            if err_msg.contains("not found") {
                (
                    StatusCode::NOT_FOUND,
                    Json(json!({"error": "Not Found", "message": err_msg})),
                )
                    .into_response()
            } else if err_msg.contains("Access denied") {
                (
                    StatusCode::FORBIDDEN,
                    Json(json!({"error": "Forbidden", "message": err_msg})),
                )
                    .into_response()
            } else {
                (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"error": "Bad Request", "message": err_msg})),
                )
                    .into_response()
            }
        }
    }
}
