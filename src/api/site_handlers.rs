use axum::{
    body::Body,
    extract::{Path as AxumPath, Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use serde_json::json;

use crate::auth::AuthUser;
use crate::database::models::{Release, Site};
use crate::hosting::AppState;
use crate::services::{AuditService, SiteService};

#[derive(Debug, Deserialize)]
pub struct ListSitesQuery {
    pub search: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateSitePayload {
    pub name: String,
    pub slug: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateSitePayload {
    pub name: Option<String>,
    pub slug: Option<String>,
    pub spa_fallback: Option<bool>,
}

pub async fn list_sites(
    State(state): State<AppState>,
    Query(query): Query<ListSitesQuery>,
) -> impl IntoResponse {
    let sites_result = if let Some(ref search) = query.search {
        let pattern = format!("%{}%", search.trim());
        sqlx::query_as::<_, Site>(
            r#"
            SELECT * FROM sites
            WHERE name LIKE ? OR slug LIKE ?
            ORDER BY updated_at DESC
            "#,
        )
        .bind(&pattern)
        .bind(&pattern)
        .fetch_all(&state.pool)
        .await
    } else {
        sqlx::query_as::<_, Site>("SELECT * FROM sites ORDER BY updated_at DESC")
            .fetch_all(&state.pool)
            .await
    };

    match sites_result {
        Ok(sites) => (StatusCode::OK, Json(json!({ "sites": sites }))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to list websites: {}", e) })),
        )
            .into_response(),
    }
}

pub async fn get_site(
    State(state): State<AppState>,
    AxumPath(site_id): AxumPath<String>,
) -> impl IntoResponse {
    let site = match sqlx::query_as::<_, Site>("SELECT * FROM sites WHERE id = ?")
        .bind(&site_id)
        .fetch_optional(&state.pool)
        .await
    {
        Ok(Some(s)) => s,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "Website not found"})),
            )
                .into_response()
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)})),
            )
                .into_response()
        }
    };

    let active_release = if let Some(ref release_id) = site.active_release_id {
        sqlx::query_as::<_, Release>("SELECT * FROM releases WHERE id = ?")
            .bind(release_id)
            .fetch_optional(&state.pool)
            .await
            .unwrap_or(None)
    } else {
        None
    };

    (
        StatusCode::OK,
        Json(json!({
            "site": site,
            "active_release": active_release
        })),
    )
        .into_response()
}

pub async fn create_site(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Json(payload): Json<CreateSitePayload>,
) -> impl IntoResponse {
    match SiteService::create_site(
        &state.pool,
        &user.id,
        &payload.name,
        &payload.slug,
        &state.config,
    )
    .await
    {
        Ok(site) => {
            AuditService::log(
                &state.pool,
                Some(&user.id),
                "CREATE_SITE",
                "SITE",
                Some(&site.id),
                Some(&format!("Created website '{}' with slug '{}'", site.name, site.slug)),
                None,
            )
            .await;

            (
                StatusCode::CREATED,
                Json(json!({
                    "success": true,
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

pub async fn update_site(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    AxumPath(site_id): AxumPath<String>,
    Json(payload): Json<UpdateSitePayload>,
) -> impl IntoResponse {
    match SiteService::update_site_metadata(
        &state.pool,
        &site_id,
        payload.name.as_deref(),
        payload.slug.as_deref(),
        payload.spa_fallback,
        &state.config,
    )
    .await
    {
        Ok(site) => {
            AuditService::log(
                &state.pool,
                Some(&user.id),
                "UPDATE_SITE",
                "SITE",
                Some(&site.id),
                Some("Updated website details"),
                None,
            )
            .await;

            (
                StatusCode::OK,
                Json(json!({
                    "success": true,
                    "message": "Website updated successfully",
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

pub async fn delete_site(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    AxumPath(site_id): AxumPath<String>,
) -> impl IntoResponse {
    match SiteService::delete_site(&state.pool, &site_id, &state.config).await {
        Ok(_) => {
            AuditService::log(
                &state.pool,
                Some(&user.id),
                "DELETE_SITE",
                "SITE",
                Some(&site_id),
                Some("Deleted website and storage data"),
                None,
            )
            .await;

            (
                StatusCode::OK,
                Json(json!({
                    "success": true,
                    "message": "Website deleted successfully"
                })),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to delete website: {}", e) })),
        )
            .into_response(),
    }
}

pub async fn list_site_files(
    State(state): State<AppState>,
    AxumPath(site_id): AxumPath<String>,
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
                Json(json!({"error": "Website not found"})),
            )
                .into_response()
        }
    };

    let active_release_id = match site.active_release_id {
        Some(r) => r,
        None => return (StatusCode::OK, Json(json!({ "files": [] }))).into_response(),
    };

    let release = match sqlx::query_as::<_, Release>("SELECT * FROM releases WHERE id = ?")
        .bind(active_release_id)
        .fetch_optional(&state.pool)
        .await
    {
        Ok(Some(r)) => r,
        _ => return (StatusCode::OK, Json(json!({ "files": [] }))).into_response(),
    };

    let release_abs_dir = state.config.storage.sites_dir.join(&release.storage_path);
    match SiteService::list_files_in_release(&release_abs_dir) {
        Ok(files) => (
            StatusCode::OK,
            Json(json!({
                "site_id": site_id,
                "version": release.version_num,
                "files": files
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to list files: {}", e) })),
        )
            .into_response(),
    }
}

pub async fn download_site_zip(
    State(state): State<AppState>,
    AxumPath(site_id): AxumPath<String>,
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
                Json(json!({"error": "Website not found"})),
            )
                .into_response()
        }
    };

    let active_release_id = match site.active_release_id {
        Some(r) => r,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "Website does not have any releases to download"})),
            )
                .into_response()
        }
    };

    let release = match sqlx::query_as::<_, Release>("SELECT * FROM releases WHERE id = ?")
        .bind(active_release_id)
        .fetch_optional(&state.pool)
        .await
    {
        Ok(Some(r)) => r,
        _ => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "Release not found"})),
            )
                .into_response()
        }
    };

    let release_abs_dir = state.config.storage.sites_dir.join(&release.storage_path);
    let temp_zip_path = state
        .config
        .storage
        .temp_dir
        .join(format!("download-{}.zip", uuid::Uuid::new_v4()));

    if let Err(e) = SiteService::create_zip_from_dir(&release_abs_dir, &temp_zip_path) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to create download ZIP archive: {}", e) })),
        )
            .into_response();
    }

    let zip_bytes = match std::fs::read(&temp_zip_path) {
        Ok(b) => b,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to read generated ZIP file"})),
            )
                .into_response()
        }
    };

    let _ = std::fs::remove_file(&temp_zip_path);

    let filename = format!("{}.zip", site.slug);
    let disposition = format!("attachment; filename=\"{}\"", filename);

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/zip")
        .header(header::CONTENT_DISPOSITION, disposition)
        .body(Body::from(zip_bytes))
        .unwrap()
}
