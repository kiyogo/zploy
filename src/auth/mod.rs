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
    let clean_token = raw_token.trim().trim_matches('"');
    let token_hash = hash_token(clean_token);
    sqlx::query("DELETE FROM sessions WHERE token_hash = ?")
        .bind(token_hash)
        .execute(pool)
        .await?;
    Ok(())
}

#[derive(sqlx::FromRow)]
struct SessionUserRow {
    id: String,
    username: String,
    password_hash: String,
    role: String,
    created_at: String,
    updated_at: String,
    expires_at: String,
}

pub async fn validate_session_token(pool: &DbPool, raw_token: &str) -> Option<User> {
    let clean_token = raw_token.trim().trim_matches('"');
    if clean_token.is_empty() {
        return None;
    }
    let token_hash = hash_token(clean_token);

    let row = sqlx::query_as::<_, SessionUserRow>(
        r#"
        SELECT u.id, u.username, u.password_hash, u.role, u.created_at, u.updated_at, s.expires_at
        FROM users u
        INNER JOIN sessions s ON s.user_id = u.id
        WHERE s.token_hash = ?
        LIMIT 1
        "#,
    )
    .bind(token_hash)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()?;

    // Robust expiration check
    let is_valid = match chrono::DateTime::parse_from_rfc3339(&row.expires_at) {
        Ok(exp) => exp.with_timezone(&Utc) > Utc::now(),
        Err(_) => {
            // Fallback string comparison
            let now_str = Utc::now().to_rfc3339();
            row.expires_at > now_str
        }
    };

    if is_valid {
        Some(User {
            id: row.id,
            username: row.username,
            password_hash: row.password_hash,
            role: row.role,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    } else {
        None
    }
}

pub fn extract_candidate_tokens_from_parts(parts: &Parts) -> Vec<String> {
    let mut tokens = Vec::new();

    // 1. Check Authorization header: Bearer <token>
    if let Some(auth_val) = parts.headers.get(header::AUTHORIZATION) {
        if let Ok(auth_str) = auth_val.to_str() {
            if let Some(token) = auth_str.strip_prefix("Bearer ") {
                let clean = token.trim().trim_matches('"').to_string();
                if !clean.is_empty() && !tokens.contains(&clean) {
                    tokens.push(clean);
                }
            }
        }
    }

    // 2. Check Cookie header: zploy_session=<token> or minihost_session=<token>
    if let Some(cookie_val) = parts.headers.get(header::COOKIE) {
        if let Ok(cookie_str) = cookie_val.to_str() {
            for cookie in cookie_str.split(';') {
                let trimmed = cookie.trim();
                if let Some(token) = trimmed.strip_prefix(&format!("{}=", SESSION_COOKIE_NAME)) {
                    let clean = token.trim().trim_matches('"').to_string();
                    if !clean.is_empty() && !tokens.contains(&clean) {
                        tokens.push(clean);
                    }
                }
                if let Some(token) = trimmed.strip_prefix("minihost_session=") {
                    let clean = token.trim().trim_matches('"').to_string();
                    if !clean.is_empty() && !tokens.contains(&clean) {
                        tokens.push(clean);
                    }
                }
            }
        }
    }

    tokens
}

pub fn extract_token_from_parts(parts: &Parts) -> Option<String> {
    extract_candidate_tokens_from_parts(parts).into_iter().next()
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

        let candidate_tokens = extract_candidate_tokens_from_parts(parts);
        if candidate_tokens.is_empty() {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Authentication required to perform this action"})),
            )
                .into_response());
        }

        // CSRF Check: if request is mutating (POST, PUT, PATCH, DELETE), verify custom header or Bearer auth
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

        // Try candidate tokens in order (Bearer token first, then newest cookies)
        for token in candidate_tokens {
            if let Some(user) = validate_session_token(pool, &token).await {
                return Ok(AuthUser(user));
            }
        }

        Err((
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Session has expired or is invalid"})),
        )
            .into_response())
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

        let candidate_tokens = extract_candidate_tokens_from_parts(parts);
        for token in candidate_tokens {
            if let Some(user) = validate_session_token(pool, &token).await {
                return Ok(OptionalAuthUser(Some(user)));
            }
        }

        Ok(OptionalAuthUser(None))
    }
}
