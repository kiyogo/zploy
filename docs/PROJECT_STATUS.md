# Zploy Project Status & Capabilities

* **Project**: Zploy
* **Repository**: https://github.com/kiyogo/zploy
* **Architecture**: Rust (Axum 0.7 + Tokio + MySQL / MariaDB)
* **Status**: Production Ready (v0.1.0)

## Feature Implementation Checklist

### Core Hosting & Routing
* [x] Multi-site subpath routing (`/demo/`, `/shop/`, `/portfolio/`)
* [x] Root path website hosting (`/`)
* [x] Trailing slash automatic redirection (`/demo` -> `/demo/`)
* [x] Safe relative asset resolution (`./style.css`, `<img src="logo.png">`)
* [x] Proper MIME types with `charset=utf-8` specification
* [x] ETag and `If-None-Match` caching (`304 Not Modified`)
* [x] HTTP 206 Partial Content range requests for audio and video streaming
* [x] SPA Fallback mode

### Releases & Version Management
* [x] Multi-version atomic release architecture
* [x] Immutable release directories (`release-0001`, `release-0002`)
* [x] One-click instant release rollback
* [x] Original source ZIP download

### Upload & Extraction Pipeline
* [x] Drag & drop file uploads (single HTML, multi-file, or ZIP)
* [x] Automatic root folder flattening for nested archives
* [x] Zip Slip & Zip Bomb defense
* [x] Static extension whitelist enforcement

### Security & System Administration
* [x] Session management with secure cookies
* [x] Argon2id password hashing
* [x] CSRF header enforcement (`X-Zploy-CSRF`)
* [x] Audit trail recording in MySQL
* [x] Dedicated CLI tool for password resets (`reset_admin`)
* [x] Clean Apple-inspired web dashboard with dark and light themes
