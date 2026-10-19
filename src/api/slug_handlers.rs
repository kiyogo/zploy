use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use serde_json::json;

use crate::hosting::AppState;
use crate::security::{normalize_slug, validate_slug};

#[derive(Debug, Deserialize)]
pub struct SlugCheckQuery {
    pub slug: Option<String>,
}

pub async fn check_slug(
    State(state): State<AppState>,
    Query(query): Query<SlugCheckQuery>,
) -> impl IntoResponse {
    let raw_slug = query.slug.unwrap_or_default();
    let clean_slug = normalize_slug(&raw_slug);

    if clean_slug.is_empty() {
        // Check if root site (/) is already published
        let existing: Option<(String,)> = sqlx::query_as("SELECT id FROM sites WHERE slug = '' LIMIT 1")
            .fetch_optional(&state.pool)
            .await
            .unwrap_or(None);

        if existing.is_some() {
            return (
                StatusCode::OK,
                Json(json!({
                    "valid": true,
                    "available": false,
                    "slug": "",
                    "message": "The root path (/) is already in use by another website."
                })),
            );
        } else {
            return (
                StatusCode::OK,
                Json(json!({
                    "valid": true,
                    "available": true,
                    "slug": "",
                    "message": "Root path (/) is available for publishing"
                })),
            );
        }
    }

    if let Err(e) = validate_slug(&clean_slug, &state.config.security.reserved_slugs) {
        return (
            StatusCode::OK,
            Json(json!({
                "valid": false,
                "available": false,
                "message": e.to_string()
            })),
        );
    }

    let existing: Option<(String,)> = sqlx::query_as("SELECT id FROM sites WHERE slug = ? LIMIT 1")
        .bind(&clean_slug)
        .fetch_optional(&state.pool)
        .await
        .unwrap_or(None);

    if existing.is_some() {
        return (
            StatusCode::OK,
            Json(json!({
                "valid": true,
                "available": false,
                "slug": clean_slug,
                "message": format!("Path '/{}' is already taken by another website", clean_slug)
            })),
        );
    }

    (
        StatusCode::OK,
        Json(json!({
            "valid": true,
            "available": true,
            "slug": clean_slug,
            "message": format!("Path '/{}' is valid and available for publishing", clean_slug)
        })),
    )
}
