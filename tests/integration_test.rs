use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use http_body_util::BodyExt;
use zploy::config::Config;
use zploy::database::init_db;
use zploy::hosting::AppState;
use zploy::services::SiteService;
use serde_json::Value;
use std::sync::Arc;
use tempfile::tempdir;
use tower::ServiceExt;

async fn setup_test_app() -> (axum::Router, AppState, tempfile::TempDir) {
    let dir = tempdir().unwrap();
    let mut config = Config::default();
    config.database.url = "mysql://root:@127.0.0.1:3306/minihost".to_string();
    config.storage.sites_dir = dir.path().join("sites");
    config.storage.staging_dir = dir.path().join("staging");
    config.storage.temp_dir = dir.path().join("temp");

    std::fs::create_dir_all(&config.storage.sites_dir).unwrap();
    std::fs::create_dir_all(&config.storage.staging_dir).unwrap();
    std::fs::create_dir_all(&config.storage.temp_dir).unwrap();

    let pool = init_db(&config).await.expect("Failed to initialize test db");

    // Clean test tables
    let _ = sqlx::query("DELETE FROM releases").execute(&pool).await;
    let _ = sqlx::query("DELETE FROM sites").execute(&pool).await;
    let _ = sqlx::query("DELETE FROM sessions").execute(&pool).await;
    let _ = sqlx::query("DELETE FROM users WHERE username LIKE 'admin_test%' OR username LIKE 'admin_root%'").execute(&pool).await;

    let app_state = AppState {
        pool,
        config: Arc::new(config),
    };

    let router = axum::Router::new()
        .nest("/api", zploy::api::create_api_router())
        .fallback(zploy::hosting::serve_hosted_request)
        .with_state(app_state.clone());

    (router, app_state, dir)
}

#[tokio::test]
async fn test_health_check() {
    let (app, _, _dir) = setup_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], "ok");
    assert_eq!(json["service"], "Zploy");
}

#[tokio::test]
async fn test_slug_availability_api() {
    let (app, _, _dir) = setup_test_app().await;

    // Available slug
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/slugs/check?slug=myshop")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["valid"], true);
    assert_eq!(json["available"], true);

    // Reserved slug
    let res2 = app
        .oneshot(
            Request::builder()
                .uri("/api/slugs/check?slug=api")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res2.status(), StatusCode::OK);
    let body2 = res2.into_body().collect().await.unwrap().to_bytes();
    let json2: Value = serde_json::from_slice(&body2).unwrap();
    assert_eq!(json2["valid"], false);
    assert_eq!(json2["available"], false);
}

#[tokio::test]
async fn test_site_lifecycle_and_static_serving() {
    let (app, state, _dir) = setup_test_app().await;

    // 1. Create a user and test site programmatically
    let user_id = uuid::Uuid::new_v4().to_string();
    let uname = format!("admin_test_{}", &user_id[..8]);
    sqlx::query("INSERT INTO users (id, username, password_hash, role) VALUES (?, ?, 'hash123', 'admin')")
        .bind(&user_id)
        .bind(&uname)
        .execute(&state.pool)
        .await
        .unwrap();

    let site = SiteService::create_site(
        &state.pool,
        &user_id,
        "Demo Website",
        "demo",
        &state.config,
    )
    .await
    .unwrap();
    assert_eq!(site.slug, "demo");

    // 2. Prepare staging files for release 1
    let staging = state.config.storage.staging_dir.join("stage-1");
    std::fs::create_dir_all(staging.join("css")).unwrap();
    std::fs::write(
        staging.join("index.html"),
        "<!DOCTYPE html><html><head><link rel=\"stylesheet\" href=\"./css/style.css\"></head><body><h1>Welcome to Zploy!</h1></body></html>",
    )
    .unwrap();
    std::fs::write(
        staging.join("css/style.css"),
        "body { font-family: sans-serif; background: #fff; }",
    )
    .unwrap();

    let rel1 = SiteService::deploy_release(
        &state.pool,
        &site.id,
        &staging,
        2,
        200,
        Some("Initial release"),
        &state.config,
    )
    .await
    .unwrap();
    assert_eq!(rel1.version_num, 1);

    // 3. Test trailing slash redirect: GET /demo -> 308 redirect to /demo/
    let redirect_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/demo")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(redirect_res.status(), StatusCode::PERMANENT_REDIRECT);
    assert_eq!(
        redirect_res.headers().get(header::LOCATION).unwrap(),
        "/demo/"
    );

    // 4. Test serving index.html at GET /demo/
    let index_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/demo/")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(index_res.status(), StatusCode::OK);
    let ct = index_res.headers().get(header::CONTENT_TYPE).unwrap().to_str().unwrap();
    assert!(ct.contains("text/html"));
    assert!(ct.contains("charset=utf-8"));
    let body = index_res.into_body().collect().await.unwrap().to_bytes();
    assert!(body.starts_with(b"<!DOCTYPE html>"));

    // 5. Test serving sub-resource GET /demo/css/style.css
    let css_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/demo/css/style.css")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(css_res.status(), StatusCode::OK);
    let css_ct = css_res.headers().get(header::CONTENT_TYPE).unwrap().to_str().unwrap();
    assert!(css_ct.contains("text/css"));

    // 6. Test 404 for missing file
    let not_found_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/demo/nonexistent.png")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(not_found_res.status(), StatusCode::NOT_FOUND);

    // 7. Update site with Release 2
    let staging2 = state.config.storage.staging_dir.join("stage-2");
    std::fs::create_dir_all(&staging2).unwrap();
    std::fs::write(
        staging2.join("index.html"),
        "<!DOCTYPE html><html><body><h1>Version 2 Updated!</h1></body></html>",
    )
    .unwrap();

    let rel2 = SiteService::deploy_release(
        &state.pool,
        &site.id,
        &staging2,
        1,
        100,
        Some("Release v2 update"),
        &state.config,
    )
    .await
    .unwrap();
    assert_eq!(rel2.version_num, 2);

    // Verify v2 is served
    let v2_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/demo/")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let v2_body = v2_res.into_body().collect().await.unwrap().to_bytes();
    assert!(String::from_utf8_lossy(&v2_body).contains("Version 2 Updated!"));

    // 8. Rollback to Release 1
    SiteService::rollback_release(&state.pool, &site.id, &rel1.id)
        .await
        .unwrap();

    // Verify v1 is served again after rollback
    let rollback_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/demo/")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let rollback_body = rollback_res.into_body().collect().await.unwrap().to_bytes();
    assert!(String::from_utf8_lossy(&rollback_body).contains("Welcome to Zploy!"));

    // 9. Delete site
    SiteService::delete_site(&state.pool, &site.id, &state.config)
        .await
        .unwrap();

    // Verify site is gone
    let deleted_res = app
        .oneshot(
            Request::builder()
                .uri("/demo/")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(deleted_res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_root_site_and_nested_slug_serving() {
    let (app, state, _dir) = setup_test_app().await;

    let user_id = uuid::Uuid::new_v4().to_string();
    let uname = format!("admin_root_{}", &user_id[..8]);
    sqlx::query("INSERT INTO users (id, username, password_hash, role) VALUES (?, ?, 'hash123', 'admin')")
        .bind(&user_id)
        .bind(&uname)
        .execute(&state.pool)
        .await
        .unwrap();

    // 1. Create a root site with empty slug ("")
    let root_site = SiteService::create_site(
        &state.pool,
        &user_id,
        "Root Website",
        "",
        &state.config,
    )
    .await
    .unwrap();
    assert_eq!(root_site.slug, "");

    // Verify cannot create another site at root "/"
    let dup_root = SiteService::create_site(
        &state.pool,
        &user_id,
        "Root Website 2",
        "/",
        &state.config,
    )
    .await;
    assert!(dup_root.is_err());

    // Deploy release for root site
    let root_staging = state.config.storage.staging_dir.join("root_v1");
    std::fs::create_dir_all(&root_staging).unwrap();
    std::fs::write(root_staging.join("index.html"), "<h1>Root Website Home</h1>").unwrap();
    std::fs::write(root_staging.join("global.css"), "body { color: blue; }").unwrap();

    SiteService::deploy_release(
        &state.pool,
        &root_site.id,
        &root_staging,
        2,
        50,
        Some("Initial root website release"),
        &state.config,
    )
    .await
    .unwrap();

    // 2. Create a nested subpath site with slug "20/10"
    let nested_site = SiteService::create_site(
        &state.pool,
        &user_id,
        "October 20th Promotion",
        "20/10",
        &state.config,
    )
    .await
    .unwrap();
    assert_eq!(nested_site.slug, "20/10");

    let nested_staging = state.config.storage.staging_dir.join("nested_v1");
    std::fs::create_dir_all(&nested_staging).unwrap();
    std::fs::write(nested_staging.join("index.html"), "<h1>October 20th Celebration</h1>").unwrap();
    std::fs::write(nested_staging.join("style.css"), "h1 { color: pink; }").unwrap();

    SiteService::deploy_release(
        &state.pool,
        &nested_site.id,
        &nested_staging,
        2,
        60,
        Some("Release 20-10"),
        &state.config,
    )
    .await
    .unwrap();

    // 3. Test GET / -> serves root site's index.html
    let root_res = app
        .clone()
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(root_res.status(), StatusCode::OK);
    let root_body = root_res.into_body().collect().await.unwrap().to_bytes();
    assert!(String::from_utf8_lossy(&root_body).contains("Root Website Home"));

    // 4. Test GET /global.css -> serves root site's global.css
    let css_res = app
        .clone()
        .oneshot(Request::builder().uri("/global.css").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(css_res.status(), StatusCode::OK);
    let css_body = css_res.into_body().collect().await.unwrap().to_bytes();
    assert!(String::from_utf8_lossy(&css_body).contains("color: blue"));

    // 5. Test GET /20/10 -> 308 redirect to /20/10/
    let redirect_res = app
        .clone()
        .oneshot(Request::builder().uri("/20/10").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(redirect_res.status(), StatusCode::PERMANENT_REDIRECT);
    assert_eq!(redirect_res.headers().get(header::LOCATION).unwrap(), "/20/10/");

    // 6. Test GET /20/10/ -> serves nested site's index.html
    let nested_res = app
        .clone()
        .oneshot(Request::builder().uri("/20/10/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(nested_res.status(), StatusCode::OK);
    let nested_body = nested_res.into_body().collect().await.unwrap().to_bytes();
    assert!(String::from_utf8_lossy(&nested_body).contains("October 20th Celebration"));

    // 7. Test GET /20/10/style.css -> serves nested site's style.css
    let nested_css_res = app
        .clone()
        .oneshot(Request::builder().uri("/20/10/style.css").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(nested_css_res.status(), StatusCode::OK);
    let nested_css_body = nested_css_res.into_body().collect().await.unwrap().to_bytes();
    assert!(String::from_utf8_lossy(&nested_css_body).contains("color: pink"));
}
