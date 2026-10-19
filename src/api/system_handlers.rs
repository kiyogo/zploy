use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde_json::json;

use crate::auth::AuthUser;
use crate::hosting::AppState;

pub async fn health() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(json!({
            "status": "ok",
            "version": env!("CARGO_PKG_VERSION"),
            "service": "Zploy"
        })),
    )
}

pub async fn stats(
    State(state): State<AppState>,
    AuthUser(_user): AuthUser,
) -> impl IntoResponse {
    let site_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM sites")
        .fetch_one(&state.pool)
        .await
        .unwrap_or((0,));

    let release_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM releases")
        .fetch_one(&state.pool)
        .await
        .unwrap_or((0,));

    let total_bytes: (Option<i64>,) = sqlx::query_as("SELECT SUM(total_size_bytes) FROM sites")
        .fetch_one(&state.pool)
        .await
        .unwrap_or((Some(0),));

    (
        StatusCode::OK,
        Json(json!({
            "sites": site_count.0,
            "releases": release_count.0,
            "total_size_bytes": total_bytes.0.unwrap_or(0),
            "max_upload_size_mb": state.config.upload.max_upload_size_mb,
            "base_url": state.config.server.base_url
        })),
    )
}
