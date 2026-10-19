use axum::{
    async_trait,
    extract::{FromRef, FromRequestParts},
    http::{header, request::Parts, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::{Duration, Utc};
use serde_json::json;
use tracing::warn;

use crate::database::{models::User, DbPool};
use crate::hosting::AppState;
use crate::security::{generate_secure_token, hash_token};

pub const SESSION_COOKIE_NAME: &str = "zploy_session";

#[derive(Debug, Clone)]
pub struct AuthUser(pub User);

#[derive(Debug, Clone)]
pub struct OptionalAuthUser(pub Option<User>);

pub async fn create_session(pool: &DbPool, user_id: &str, expiry_hours: i64) -> anyhow::Result<String> {
    let raw_token = generate_secure_token();
    let token_hash = hash_token(&raw_token);
    let session_id = uuid::Uuid::new_v4().to_string();
    let expires_at = Utc::now() + Duration::hours(expiry_hours);
    let expires_at_str = expires_at.to_rfc3339();

    let now_str = Utc::now().to_rfc3339();

    sqlx::query(
        r#"
        INSERT INTO sessions (id, user_id, token_hash, expires_at, created_at)
        VALUES (?, ?, ?, ?, ?)
        "#,
    )
    .bind(session_id)
    .bind(user_id)
    .bind(token_hash)
    .bind(expires_at_str)
    .bind(now_str)
    .execute(pool)
    .await?;

    Ok(raw_token)
}

pub async fn destroy_session(pool: &DbPool, raw_token: &str) -> anyhow::Result<()> {
    let token_hash = hash_token(raw_token);
    sqlx::query("DELETE FROM sessions WHERE token_hash = ?")
        .bind(token_hash)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn validate_session_token(pool: &DbPool, raw_token: &str) -> Option<User> {
    let token_hash = hash_token(raw_token);
    let now_str = Utc::now().to_rfc3339();

    let user = sqlx::query_as::<_, User>(
        r#"
        SELECT u.id, u.username, u.password_hash, u.role, u.created_at, u.updated_at
        FROM users u
        INNER JOIN sessions s ON s.user_id = u.id
        WHERE s.token_hash = ? AND s.expires_at > ?
        LIMIT 1
        "#,
    )
    .bind(token_hash)
    .bind(now_str)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    user
}

pub fn extract_token_from_parts(parts: &Parts) -> Option<String> {
    // 1. Check Authorization header: Bearer <token>
    if let Some(auth_val) = parts.headers.get(header::AUTHORIZATION) {
        if let Ok(auth_str) = auth_val.to_str() {
            if let Some(token) = auth_str.strip_prefix("Bearer ") {
                return Some(token.trim().to_string());
            }
        }
    }

    // 2. Check Cookie header: zploy_session=<token> or minihost_session=<token>
    if let Some(cookie_val) = parts.headers.get(header::COOKIE) {
        if let Ok(cookie_str) = cookie_val.to_str() {
            for cookie in cookie_str.split(';') {
                let trimmed = cookie.trim();
                if let Some(token) = trimmed.strip_prefix(&format!("{}=", SESSION_COOKIE_NAME)) {
                    return Some(token.to_string());
                }
                if let Some(token) = trimmed.strip_prefix("minihost_session=") {
                    return Some(token.to_string());
                }
            }
        }
    }

    None
}

// Extractor for required authentication
#[async_trait]
impl<S> FromRequestParts<S> for AuthUser
where
    AppState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let app_state = AppState::from_ref(state);
        let pool = &app_state.pool;

        let raw_token = match extract_token_from_parts(parts) {
            Some(t) => t,
            None => {
                return Err((
                    StatusCode::UNAUTHORIZED,
                    Json(json!({"error": "Authentication required to perform this action"})),
                )
                    .into_response())
            }
        };

        // CSRF Check: if request is mutating (POST, PUT, PATCH, DELETE) and uses cookie auth, verify custom header
        if parts.method == axum::http::Method::POST
            || parts.method == axum::http::Method::PATCH
            || parts.method == axum::http::Method::DELETE
            || parts.method == axum::http::Method::PUT
        {
            let has_csrf_header = parts.headers.contains_key("X-Zploy-CSRF")
                || parts.headers.contains_key("x-zploy-csrf")
                || parts.headers.contains_key("X-MiniHost-CSRF")
                || parts.headers.contains_key("x-minihost-csrf")
                || parts.headers.contains_key(header::AUTHORIZATION);
            if !has_csrf_header {
                warn!("Rejected request due to missing CSRF protection header");
                return Err((
                    StatusCode::FORBIDDEN,
                    Json(json!({"error": "Request rejected due to missing CSRF protection header (X-Zploy-CSRF)"})),
                )
                    .into_response());
            }
        }

        match validate_session_token(pool, &raw_token).await {
            Some(user) => Ok(AuthUser(user)),
            None => Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Session has expired or is invalid"})),
            )
                .into_response()),
        }
    }
}

// Extractor for optional authentication
#[async_trait]
impl<S> FromRequestParts<S> for OptionalAuthUser
where
    AppState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let app_state = AppState::from_ref(state);
        let pool = &app_state.pool;

        let raw_token = match extract_token_from_parts(parts) {
            Some(t) => t,
            None => return Ok(OptionalAuthUser(None)),
        };

        let user = validate_session_token(pool, &raw_token).await;
        Ok(OptionalAuthUser(user))
    }
}
