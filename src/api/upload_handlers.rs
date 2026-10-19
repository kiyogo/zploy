use axum::{
    extract::{multipart::Multipart, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde_json::json;

use crate::auth::AuthUser;
use crate::database::models::Site;
use crate::hosting::AppState;
use crate::services::{AuditService, SiteService};
use crate::upload::process_upload_multipart;

pub async fn quick_publish(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    multipart: Multipart,
) -> impl IntoResponse {
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

    let raw_slug = staged.form_slug.clone().unwrap_or_default();
    let slug = crate::security::normalize_slug(&raw_slug);

    if let Err(e) = crate::security::validate_slug(&slug, &state.config.security.reserved_slugs) {
        staged.cleanup();
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response();
    }

    let name = staged
        .form_name
        .clone()
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| {
            if slug.is_empty() {
                "Root Website".to_string()
            } else {
                slug.clone()
            }
        });

    // Check if site already exists
    let existing_site = match sqlx::query_as::<_, Site>("SELECT * FROM sites WHERE slug = ? LIMIT 1")
        .bind(&slug)
        .fetch_optional(&state.pool)
        .await
    {
        Ok(s) => s,
        Err(e) => {
            staged.cleanup();
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Database error: {}", e) })),
            )
                .into_response();
        }
    };

    let (site, is_new) = match existing_site {
        Some(site) => (site, false),
        None => {
            // Create new site
            match SiteService::create_site(&state.pool, &user.id, &name, &slug, &state.config).await {
                Ok(s) => (s, true),
                Err(e) => {
                    staged.cleanup();
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(json!({ "error": e.to_string() })),
                    )
                        .into_response();
                }
            }
        }
    };

    let note = if is_new {
        "Initial publish"
    } else {
        "Website update"
    };

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

            let action = if is_new { "QUICK_PUBLISH_CREATE" } else { "QUICK_PUBLISH_UPDATE" };
            AuditService::log(
                &state.pool,
                Some(&user.id),
                action,
                "SITE",
                Some(&site.id),
                Some(&format!("Successfully published website '{}' at /{}", site.name, site.slug)),
                None,
            )
            .await;

            let site_url = if site.slug.is_empty() {
                format!("{}/", state.config.server.base_url.trim_end_matches('/'))
            } else {
                format!("{}/{}/", state.config.server.base_url.trim_end_matches('/'), site.slug)
            };

            (
                StatusCode::OK,
                Json(json!({
                    "success": true,
                    "message": if is_new { "Your website has been published successfully." } else { "Website updated successfully." },
                    "url": site_url,
                    "slug": site.slug,
                    "site_id": site.id,
                    "version": release.version_num,
                    "file_count": release.file_count,
                    "total_size_bytes": release.total_size_bytes
                })),
            )
                .into_response()
        }
        Err(e) => {
            staged.cleanup();
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Failed to publish website: {}", e) })),
            )
                .into_response()
        }
    }
}
