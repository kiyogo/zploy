pub mod auth_handlers;
pub mod release_handlers;
pub mod site_handlers;
pub mod slug_handlers;
pub mod system_handlers;
pub mod upload_handlers;

use axum::{
    routing::{get, post},
    Router,
};

use crate::hosting::AppState;

pub fn create_api_router() -> Router<AppState> {
    Router::new()
        // Auth
        .route("/auth/login", post(auth_handlers::login))
        .route("/auth/logout", post(auth_handlers::logout))
        .route("/auth/me", get(auth_handlers::me))
        .route("/auth/setup", post(auth_handlers::setup_initial_admin))
        // Slugs
        .route("/slugs/check", get(slug_handlers::check_slug))
        // Sites
        .route("/sites", get(site_handlers::list_sites).post(site_handlers::create_site))
        .route(
            "/sites/:id",
            get(site_handlers::get_site)
                .patch(site_handlers::update_site)
                .delete(site_handlers::delete_site),
        )
        .route("/sites/:id/files", get(site_handlers::list_site_files))
        .route("/sites/:id/download", get(site_handlers::download_site_zip))
        // Releases
        .route(
            "/sites/:id/releases",
            get(release_handlers::list_releases).post(release_handlers::upload_new_release),
        )
        .route("/sites/:id/rollback", post(release_handlers::rollback_release))
        // Uploads
        .route("/uploads/quick-publish", post(upload_handlers::quick_publish))
        // System
        .route("/health", get(system_handlers::health))
        .route("/system/stats", get(system_handlers::stats))
}
