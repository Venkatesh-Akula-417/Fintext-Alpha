//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — News Article Full Text Retrieval Handlers
//! ═══════════════════════════════════════════════════════════════════════════════

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde_json::json;
use uuid::Uuid;

use crate::models::ListNewsArticlesQuery;
use crate::state::AppState;

/// List news articles with optional filters, pagination, and 200-char preview snippets.
#[utoipa::path(
    get,
    path = "/news/articles",
    tag = "Financial News & Full Text",
    params(
        ListNewsArticlesQuery
    ),
    responses(
        (status = 200, description = "List of news articles with metadata and preview snippets", body = NewsArticlesListResponse),
        (status = 400, description = "Invalid query parameters"),
        (status = 401, description = "Missing or invalid authorization token")
    ),
    security(
        ("bearer_auth" = []),
        ("api_key_auth" = [])
    )
)]
pub async fn list_news_articles_handler(
    State(state): State<AppState>,
    Query(query): Query<ListNewsArticlesQuery>,
) -> impl IntoResponse {
    if let Some(limit) = query.limit {
        if limit == 0 || limit > 100 {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Bad Request",
                    "message": "Limit must be between 1 and 100"
                })),
            );
        }
    }

    let response = state.news_article_registry.list_articles(&query);
    (StatusCode::OK, Json(json!(response)))
}

/// Retrieve the full text and detailed NLP signals for a specific news article by ID.
#[utoipa::path(
    get,
    path = "/news/articles/{id}",
    tag = "Financial News & Full Text",
    params(
        ("id" = String, Path, description = "UUID of the news article to retrieve")
    ),
    responses(
        (status = 200, description = "Full news article text and metadata", body = NewsArticleFull),
        (status = 401, description = "Missing or invalid authorization token"),
        (status = 404, description = "News article not found")
    ),
    security(
        ("bearer_auth" = []),
        ("api_key_auth" = [])
    )
)]
pub async fn get_news_article_handler(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match state.news_article_registry.get_article(&id) {
        Some(article) => (StatusCode::OK, Json(json!(article))),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": "Not Found",
                "message": format!("News article '{}' not found", id)
            })),
        ),
    }
}
