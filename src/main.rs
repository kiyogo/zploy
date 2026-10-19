use axum::{
    extract::DefaultBodyLimit,
    http::{header, StatusCode},
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::{
    cors::{Any, CorsLayer},
    services::ServeDir,
    trace::TraceLayer,
};
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use zploy::api::create_api_router;
use zploy::config::Config;
use zploy::database::init_db;
use zploy::hosting::{serve_hosted_request, AppState};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize structured logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "zploy=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Starting Zploy Web Server...");

    // Load configuration
    let config = Config::load()?;
    info!(
        "Configuration loaded. Port: {}, Sites directory: {:?}",
        config.server.port, config.storage.sites_dir
    );

    // Initialize database & run migrations
    let pool = init_db(&config).await?;

    let app_state = AppState {
        pool,
        config: Arc::new(config.clone()),
    };

    // Body size limit for uploads
    let body_limit = config.upload.max_upload_size_mb * 1024 * 1024;

    // CORS configuration
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers([
            header::AUTHORIZATION,
            header::CONTENT_TYPE,
            header::HeaderName::from_static("x-zploy-csrf"),
            header::HeaderName::from_static("x-minihost-csrf"),
        ]);

    // Ensure public and assets folders exist
    std::fs::create_dir_all("public/assets/css")?;
    std::fs::create_dir_all("public/assets/js")?;

    // Build the master router
    let app = Router::new()
        // API Router
        .nest("/api", create_api_router())
        // Zploy frontend static assets
        .nest_service("/assets", ServeDir::new("public/assets"))
        // Zploy admin management page (supports /admin, /admin/, and /admin/*subpath)
        .route("/admin", get(serve_admin_page))
        .route("/admin/", get(serve_admin_page))
        .route("/admin/*subpath", get(serve_admin_page))
        // Dynamic site hosting: fallback for root site `/`, subpath slugs like `/20/10/`, etc.
        .fallback(serve_hosted_request)
        .layer(DefaultBodyLimit::max(body_limit))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(app_state);

    let addr = SocketAddr::new(
        config.server.host.parse().unwrap_or([0, 0, 0, 0].into()),
        config.server.port,
    );

    info!("Zploy Server is ready at http://{}", addr);
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    info!("Zploy Server has shut down safely.");
    Ok(())
}

async fn serve_admin_page() -> impl IntoResponse {
    let admin_path = std::path::Path::new("public/admin.html");
    if admin_path.exists() {
        match std::fs::read_to_string(admin_path) {
            Ok(content) => Html(content).into_response(),
            Err(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to read admin interface",
            )
                .into_response(),
        }
    } else {
        (
            StatusCode::NOT_FOUND,
            "Admin interface file public/admin.html not found",
        )
            .into_response()
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C signal handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("Received shutdown signal from user (Ctrl+C).");
        },
        _ = terminate => {
            info!("Received SIGTERM termination signal.");
        },
    }
}
