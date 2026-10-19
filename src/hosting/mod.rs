use axum::{
    body::Body,
    extract::{Path as AxumPath, State},
    http::{header, HeaderMap, Request, Response, StatusCode},
};
use http_range_header::parse_range_header;
use mime_guess::from_path;
use sqlx::MySqlPool;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::Arc;
use tracing::warn;

use crate::config::Config;
use crate::database::models::{Release, Site};
use crate::security::path_traversal::resolve_safe_path;

#[derive(Clone)]
pub struct AppState {
    pub pool: MySqlPool,
    pub config: Arc<Config>,
}

/// Fallback / universal request handler for all hosted sites (root site `/`, subpath slugs like `/20/10/`, etc.)
pub async fn serve_hosted_request(
    State(state): State<AppState>,
    headers: HeaderMap,
    request: Request<Body>,
) -> Response<Body> {
    let raw_uri = request.uri().path().to_string();
    let clean_path = raw_uri.trim_matches('/').to_lowercase();

    // Check system reserved paths
    if clean_path == "admin" || clean_path.starts_with("admin/") {
        let loc = if clean_path == "admin/login" || clean_path.starts_with("admin/login/") {
            "/admin/login"
        } else {
            "/admin"
        };
        return Response::builder()
            .status(StatusCode::SEE_OTHER)
            .header(header::LOCATION, loc)
            .body(Body::empty())
            .unwrap();
    }
    if clean_path == "api" || clean_path.starts_with("api/") {
        return not_found_response("This path belongs to system API endpoints.");
    }
    if clean_path == "assets" || clean_path.starts_with("assets/") {
        return not_found_response("This path belongs to system static assets.");
    }

    // Build candidate slugs from longest path prefix down to root ("")
    let mut candidates = Vec::new();
    if !clean_path.is_empty() {
        let segments: Vec<&str> = clean_path.split('/').collect();
        for i in (1..=segments.len()).rev() {
            candidates.push(segments[..i].join("/"));
        }
    }
    candidates.push("".to_string()); // root slug candidate

    // Find the first matching site from candidates
    let mut matched_site: Option<Site> = None;
    let mut matched_slug = String::new();
    let mut subpath = String::new();

    for cand in &candidates {
        let site = match sqlx::query_as::<_, Site>(
            "SELECT * FROM sites WHERE slug = ? AND status = 'published' LIMIT 1"
        )
        .bind(cand)
        .fetch_optional(&state.pool)
        .await
        {
            Ok(Some(s)) => Some(s),
            Ok(None) => None,
            Err(e) => {
                warn!("Database error while serving site: {}", e);
                return internal_error_response("Database error while looking up website.");
            }
        };

        if let Some(s) = site {
            matched_slug = cand.clone();
            if cand.is_empty() {
                subpath = clean_path.clone();
            } else {
                let remainder = clean_path.strip_prefix(cand).unwrap_or("");
                subpath = remainder.trim_start_matches('/').to_string();
            }
            matched_site = Some(s);
            break;
        }
    }

    let site = match matched_site {
        Some(s) => s,
        None => {
            if clean_path.is_empty() {
                // Root path has no published website yet
                return empty_root_response();
            }
            return not_found_response("No website found at this path.");
        }
    };

    let active_release_id = match site.active_release_id {
        Some(ref id) => id,
        None => return not_found_response("This website has no active published release."),
    };

    let release = match sqlx::query_as::<_, Release>(
        "SELECT * FROM releases WHERE id = ? LIMIT 1"
    )
    .bind(active_release_id)
    .fetch_optional(&state.pool)
    .await
    {
        Ok(Some(r)) => r,
        Ok(None) => return not_found_response("Release data for this website could not be found."),
        Err(e) => {
            warn!("Error querying release: {}", e);
            return internal_error_response("Database error while loading site release.");
        }
    };

    let release_abs_dir = state.config.storage.sites_dir.join(&release.storage_path);
    if !release_abs_dir.exists() {
        return not_found_response("Storage directory for this website does not exist.");
    }

    // Trailing slash redirect check:
    // If site has a non-empty slug (e.g. `20/10` or `demo`) and request was without trailing slash (e.g. `/20/10`)
    // and subpath is empty: redirect to `/{matched_slug}/`
    if !matched_slug.is_empty() && subpath.is_empty() {
        let expected_root = format!("/{}", matched_slug);
        if raw_uri == expected_root {
            let redirect_target = format!("/{}/", matched_slug);
            return Response::builder()
                .status(StatusCode::PERMANENT_REDIRECT)
                .header(header::LOCATION, redirect_target)
                .body(Body::empty())
                .unwrap();
        }
    }

    // Determine target file inside release
    let requested_path = if subpath.is_empty() || subpath == "/" {
        "index.html".to_string()
    } else {
        subpath
    };

    let target_file_path = match resolve_safe_path(&release_abs_dir, &requested_path) {
        Ok(p) => p,
        Err(e) => {
            warn!("Insecure path traversal detected: {:?}", e);
            return not_found_response("Invalid path or access denied.");
        }
    };

    // If path is a directory, check for directory redirect or index.html
    if target_file_path.is_dir() {
        if !raw_uri.ends_with('/') {
            let redirect_target = format!("{}/", raw_uri);
            return Response::builder()
                .status(StatusCode::PERMANENT_REDIRECT)
                .header(header::LOCATION, redirect_target)
                .body(Body::empty())
                .unwrap();
        }

        let index_candidate = target_file_path.join("index.html");
        if index_candidate.is_file() {
            return serve_file_response(&index_candidate, &headers, &request);
        }
    }

    // If file exists and is a file, serve it
    if target_file_path.is_file() {
        return serve_file_response(&target_file_path, &headers, &request);
    }

    // SPA fallback handling
    if site.spa_fallback == 1 || state.config.security.enable_spa_fallback {
        let root_index = release_abs_dir.join("index.html");
        if root_index.is_file() {
            return serve_file_response(&root_index, &headers, &request);
        }
    }

    not_found_response("The requested file was not found on this website (404 Not Found).")
}

/// Handler for `/{slug}` (without subpath) for backwards-compatibility
pub async fn serve_site_root(
    State(state): State<AppState>,
    AxumPath(_slug): AxumPath<String>,
    headers: HeaderMap,
    request: Request<Body>,
) -> Response<Body> {
    serve_hosted_request(State(state), headers, request).await
}

/// Handler for `/{slug}/*subpath` for backwards-compatibility
pub async fn serve_site_path(
    State(state): State<AppState>,
    AxumPath((_slug, _subpath)): AxumPath<(String, String)>,
    headers: HeaderMap,
    request: Request<Body>,
) -> Response<Body> {
    serve_hosted_request(State(state), headers, request).await
}

fn serve_file_response(
    file_path: &Path,
    headers: &HeaderMap,
    _request: &Request<Body>,
) -> Response<Body> {
    let metadata = match std::fs::metadata(file_path) {
        Ok(m) => m,
        Err(_) => return not_found_response("Cannot read file metadata."),
    };

    let file_size = metadata.len();
    let modified_time = metadata.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);

    // Generate ETag: W/"<size>-<modified_unix_secs>"
    let modified_secs = modified_time
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let etag_value = format!("\"{:x}-{:x}\"", file_size, modified_secs);

    // Check If-None-Match
    if let Some(if_none_match) = headers.get(header::IF_NONE_MATCH) {
        if let Ok(val) = if_none_match.to_str() {
            if val == etag_value || val == "*" {
                return Response::builder()
                    .status(StatusCode::NOT_MODIFIED)
                    .header(header::ETAG, etag_value)
                    .body(Body::empty())
                    .unwrap();
            }
        }
    }

    // Determine MIME type
    let mime = from_path(file_path).first_or_octet_stream();
    let mut content_type = mime.to_string();

    // Ensure utf-8 for text/html, text/css, js, json, svg
    if content_type.starts_with("text/")
        || content_type == "application/javascript"
        || content_type == "application/json"
    {
        if !content_type.contains("charset=") {
            content_type.push_str("; charset=utf-8");
        }
    }

    let is_html = content_type.contains("text/html");
    let cache_control = if is_html {
        "no-cache, must-revalidate"
    } else {
        "public, max-age=86400"
    };

    // Range Request Support for Media (mp4, webm, mp3, ogg, wav)
    if let Some(range_header) = headers.get(header::RANGE) {
        if let Ok(range_str) = range_header.to_str() {
            if let Ok(ranges) = parse_range_header(range_str) {
                if let Some(bound) = ranges.validate(file_size).ok().and_then(|mut r| r.pop()) {
                    let start = *bound.start();
                    let end = *bound.end(); // inclusive
                    let length = end - start + 1;

                    let mut file = match File::open(file_path) {
                        Ok(f) => f,
                        Err(_) => return internal_error_response("Error opening media file."),
                    };

                    if file.seek(SeekFrom::Start(start)).is_err() {
                        return internal_error_response("Error seeking media content.");
                    }

                    let mut buffer = vec![0u8; length as usize];
                    if file.read_exact(&mut buffer).is_err() {
                        return internal_error_response("Error reading media content.");
                    }

                    let content_range = format!("bytes {}-{}/{}", start, end, file_size);

                    return Response::builder()
                        .status(StatusCode::PARTIAL_CONTENT)
                        .header(header::CONTENT_TYPE, content_type)
                        .header(header::CONTENT_LENGTH, length.to_string())
                        .header(header::CONTENT_RANGE, content_range)
                        .header(header::ACCEPT_RANGES, "bytes")
                        .header(header::ETAG, etag_value)
                        .header(header::CACHE_CONTROL, cache_control)
                        .header("X-Content-Type-Options", "nosniff")
                        .body(Body::from(buffer))
                        .unwrap();
                }
            }
        }
    }

    // Normal full file read
    let content = match std::fs::read(file_path) {
        Ok(c) => c,
        Err(_) => return internal_error_response("Failed to read file from storage disk."),
    };

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CONTENT_LENGTH, file_size.to_string())
        .header(header::ETAG, etag_value)
        .header(header::CACHE_CONTROL, cache_control)
        .header(header::ACCEPT_RANGES, "bytes")
        .header("X-Content-Type-Options", "nosniff")
        .body(Body::from(content))
        .unwrap()
}

fn empty_root_response() -> Response<Body> {
    let html = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title></title>
    <style>
        html, body {
            margin: 0;
            padding: 0;
            width: 100%;
            height: 100%;
            background-color: #ffffff;
            background: #ffffff;
            overflow: hidden;
        }
    </style>
</head>
<body></body>
</html>"#;

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .header("X-Content-Type-Options", "nosniff")
        .body(Body::from(html))
        .unwrap()
}

fn not_found_response(message: &str) -> Response<Body> {
    let html = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>404 - Page Not Found | Zploy</title>
    <style>
        body {{
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
            background-color: #fbfbfd;
            color: #1d1d1f;
            display: flex;
            align-items: center;
            justify-content: center;
            min-height: 100vh;
            margin: 0;
            padding: 20px;
        }}
        .container {{
            max-width: 480px;
            text-align: center;
            background: #ffffff;
            padding: 40px;
            border-radius: 18px;
            box-shadow: 0 4px 24px rgba(0,0,0,0.06);
            border: 1px solid rgba(0,0,0,0.08);
        }}
        h1 {{
            font-size: 56px;
            font-weight: 700;
            margin: 0 0 10px 0;
            letter-spacing: -0.02em;
        }}
        p {{
            font-size: 16px;
            color: #86868b;
            line-height: 1.5;
            margin: 0;
        }}
    </style>
</head>
<body>
    <div class="container">
        <h1>404</h1>
        <p>{}</p>
    </div>
</body>
</html>"#,
        message
    );

    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .header("X-Content-Type-Options", "nosniff")
        .body(Body::from(html))
        .unwrap()
}

fn internal_error_response(message: &str) -> Response<Body> {
    Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .body(Body::from(message.to_string()))
        .unwrap()
}
