use axum::{
    extract::{multipart::Multipart, Path as AxumPath, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use serde_json::json;

use crate::auth::AuthUser;
use crate::database::models::{Release, Site};
use crate::hosting::AppState;
use crate::services::{AuditService, SiteService};
use crate::upload::process_upload_multipart;

#[derive(Debug, Deserialize)]
pub struct RollbackPayload {
    pub target_release_id: String,
}

pub async fn list_releases(
    State(state): State<AppState>,
    AxumPath(site_id): AxumPath<String>,
) -> impl IntoResponse {
    let releases = match sqlx::query_as::<_, Release>(
        "SELECT * FROM releases WHERE site_id = ? ORDER BY version_num DESC"
    )
    .bind(&site_id)
    .fetch_all(&state.pool)
    .await
    {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to query release history: {}", e)})),
            )
                .into_response()
        }
    };

    (StatusCode::OK, Json(json!({ "releases": releases }))).into_response()
}

pub async fn upload_new_release(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    AxumPath(site_id): AxumPath<String>,
    multipart: Multipart,
) -> impl IntoResponse {
    let site = match sqlx::query_as::<_, Site>("SELECT * FROM sites WHERE id = ?")
        .bind(&site_id)
        .fetch_optional(&state.pool)
        .await
    {
        Ok(Some(s)) => s,
        _ => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "Website not found for update"})),
            )
                .into_response()
        }
    };

    let staged = match process_upload_multipart(multipart, &state.config).await {
        Ok(s) => s,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": e.to_string() })),
            )
                .into_response();
        }
    };

    let note = staged.form_note.as_deref().unwrap_or("Release update");

    match SiteService::deploy_release(
        &state.pool,
        &site.id,
        &staged.staging_dir,
        staged.file_count,
        staged.total_size_bytes,
        Some(note),
        &state.config,
    )
    .await
    {
        Ok(release) => {
            staged.cleanup();

            AuditService::log(
                &state.pool,
                Some(&user.id),
                "DEPLOY_RELEASE",
                "SITE",
                Some(&site.id),
                Some(&format!("Deployed new release v{}", release.version_num)),
                None,
            )
            .await;

            (
                StatusCode::OK,
                Json(json!({
                    "success": true,
                    "message": "Website updated successfully",
                    "release": release
                })),
            )
                .into_response()
        }
        Err(e) => {
            staged.cleanup();
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Failed to deploy release update: {}", e) })),
            )
                .into_response()
        }
    }
}

pub async fn rollback_release(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    AxumPath(site_id): AxumPath<String>,
    Json(payload): Json<RollbackPayload>,
) -> impl IntoResponse {
    match SiteService::rollback_release(&state.pool, &site_id, &payload.target_release_id).await {
        Ok(site) => {
            AuditService::log(
                &state.pool,
                Some(&user.id),
                "ROLLBACK_RELEASE",
                "SITE",
                Some(&site_id),
                Some(&format!("Rolled back to release id {}", payload.target_release_id)),
                None,
            )
            .await;

            (
                StatusCode::OK,
                Json(json!({
                    "success": true,
                    "message": "Release rollback completed successfully",
                    "site": site
                })),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}
