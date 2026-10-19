use axum::{
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use serde_json::json;

use chrono::Utc;

use crate::auth::{create_session, destroy_session, extract_token_from_parts, AuthUser, OptionalAuthUser, SESSION_COOKIE_NAME};
use crate::database::models::User;
use crate::hosting::AppState;
use crate::security::{hash_password, verify_password};
use crate::services::AuditService;

#[derive(Debug, Deserialize)]
pub struct LoginPayload {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct SetupPayload {
    pub username: String,
    pub password: String,
}

pub async fn login(
    State(state): State<AppState>,
    Json(payload): Json<LoginPayload>,
) -> impl IntoResponse {
    let username = payload.username.trim();
    if username.is_empty() || payload.password.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Please enter both username and password"})),
        )
            .into_response();
    }

    let user = match sqlx::query_as::<_, User>("SELECT * FROM users WHERE username = ? LIMIT 1")
        .bind(username)
        .fetch_optional(&state.pool)
        .await
    {
        Ok(Some(u)) => u,
        _ => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Invalid username or password"})),
            )
                .into_response();
        }
    };

    if !verify_password(&payload.password, &user.password_hash) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Invalid username or password"})),
        )
            .into_response();
    }

    let token = match create_session(
        &state.pool,
        &user.id,
        state.config.auth.session_expiry_hours,
    )
    .await
    {
        Ok(t) => t,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to create session: {}", e)})),
            )
                .into_response();
        }
    };

    AuditService::log(
        &state.pool,
        Some(&user.id),
        "LOGIN",
        "USER",
        Some(&user.id),
        Some("User logged in successfully"),
        None,
    )
    .await;

    let cookie_secure_flag = if state.config.auth.cookie_secure {
        "; Secure"
    } else {
        ""
    };

    let cookie_header = format!(
        "{}={}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}{}",
        SESSION_COOKIE_NAME,
        token,
        state.config.auth.session_expiry_hours * 3600,
        cookie_secure_flag
    );

    let mut headers = HeaderMap::new();
    headers.insert(header::SET_COOKIE, cookie_header.parse().unwrap());

    (
        StatusCode::OK,
        headers,
        Json(json!({
            "success": true,
            "message": "Logged in successfully",
            "token": token,
            "user": {
                "id": user.id,
                "username": user.username,
                "role": user.role
            }
        })),
    )
        .into_response()
}

pub async fn logout(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    headers: HeaderMap,
) -> impl IntoResponse {
    let dummy_uri = axum::http::Uri::from_static("/api/auth/logout");
    let mut parts = axum::http::Request::builder().uri(dummy_uri).body(()).unwrap().into_parts().0;
    parts.headers = headers;

    if let Some(token) = extract_token_from_parts(&parts) {
        let _ = destroy_session(&state.pool, &token).await;
    }

    AuditService::log(
        &state.pool,
        Some(&user.id),
        "LOGOUT",
        "USER",
        Some(&user.id),
        Some("User logged out successfully"),
        None,
    )
    .await;

    let clear_cookie = format!(
        "{}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0",
        SESSION_COOKIE_NAME
    );

    let mut resp_headers = HeaderMap::new();
    resp_headers.insert(header::SET_COOKIE, clear_cookie.parse().unwrap());

    (
        StatusCode::OK,
        resp_headers,
        Json(json!({
            "success": true,
            "message": "Logged out successfully"
        })),
    )
}

pub async fn me(
    State(state): State<AppState>,
    OptionalAuthUser(maybe_user): OptionalAuthUser,
) -> impl IntoResponse {
    let users_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
        .fetch_one(&state.pool)
        .await
        .unwrap_or((0,));

    let is_initialized = users_count.0 > 0;

    match maybe_user {
        Some(user) => (
            StatusCode::OK,
            Json(json!({
                "authenticated": true,
                "initialized": is_initialized,
                "user": {
                    "id": user.id,
                    "username": user.username,
                    "role": user.role
                }
            })),
        ),
        None => (
            StatusCode::OK,
            Json(json!({
                "authenticated": false,
                "initialized": is_initialized,
                "user": null
            })),
        ),
    }
}

pub async fn setup_initial_admin(
    State(state): State<AppState>,
    Json(payload): Json<SetupPayload>,
) -> impl IntoResponse {
    let users_count: (i64,) = match sqlx::query_as("SELECT COUNT(*) FROM users")
        .fetch_one(&state.pool)
        .await
    {
        Ok(c) => c,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Database query error"})),
            )
                .into_response()
        }
    };

    if users_count.0 > 0 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "The system already has an administrator account. Please sign in."})),
        )
            .into_response();
    }

    let username = payload.username.trim();
    if username.len() < 3 || payload.password.len() < 6 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Username must be at least 3 characters, password at least 6 characters"})),
        )
            .into_response();
    }

    let password_hash = match hash_password(&payload.password) {
        Ok(h) => h,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Password hashing failed: {}", e)})),
            )
                .into_response()
        }
    };

    let user_id = uuid::Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    if let Err(e) = sqlx::query(
        r#"
        INSERT INTO users (id, username, password_hash, role, created_at, updated_at)
        VALUES (?, ?, ?, 'admin', ?, ?)
        "#,
    )
    .bind(&user_id)
    .bind(username)
    .bind(password_hash)
    .bind(&now)
    .bind(&now)
    .execute(&state.pool)
    .await
    {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to save account: {}", e)})),
        )
            .into_response();
    }

    AuditService::log(
        &state.pool,
        Some(&user_id),
        "SETUP",
        "USER",
        Some(&user_id),
        Some("Initial administrator account created"),
        None,
    )
    .await;

    (
        StatusCode::CREATED,
        Json(json!({
            "success": true,
            "message": "Administrator account created successfully. Please sign in."
        })),
    )
        .into_response()
}
