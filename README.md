<p align="center">
  <img src="./assets/logo.svg" alt="Zploy Banner" width="850">
</p>

<p align="center">
  <strong>Blazing-fast, ultra-lightweight, and zero-configuration static web hosting engine in Rust.</strong><br>
  Deploy static sites, HTML5 games, documentation, Single Page Applications, and web assets instantly via drag and drop or CLI.
</p>

<p align="center">
  <a href="https://github.com/kiyogo/zploy/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/kiyogo/zploy/ci.yml?branch=main&label=CI&style=for-the-badge&logo=githubactions&logoColor=white" alt="CI Status"></a>
  <a href="https://github.com/kiyogo/zploy/releases"><img src="https://img.shields.io/github/v/release/kiyogo/zploy?style=for-the-badge&color=blue" alt="Release"></a>
  <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/Language-Rust%202021-orange.svg?style=for-the-badge&logo=rust" alt="Rust"></a>
  <a href="https://github.com/tokio-rs/axum"><img src="https://img.shields.io/badge/Framework-Axum%200.7-5f5fff.svg?style=for-the-badge" alt="Axum"></a>
  <a href="https://www.docker.com/"><img src="https://img.shields.io/badge/Docker-Ready-2496ed.svg?style=for-the-badge&logo=docker&logoColor=white" alt="Docker"></a>
  <a href="https://www.mysql.com/"><img src="https://img.shields.io/badge/Database-MySQL%20%2F%20MariaDB-00758f.svg?style=for-the-badge&logo=mysql&logoColor=white" alt="MySQL"></a>
  <a href="./LICENSE"><img src="https://img.shields.io/badge/License-MIT-emerald.svg?style=for-the-badge" alt="License"></a>
</p>

<p align="center">
  <img src="./assets/dashboard.png" alt="Zploy Web Dashboard" width="900" style="border-radius: 12px; box-shadow: 0 16px 36px rgba(0,0,0,0.4);">
</p>

## Overview

Zploy is an enterprise-grade, lightweight static web hosting server engineered in Rust. It eliminates the traditional headache of configuring Nginx, Caddy, complex Docker containers, DNS records, or slow CI/CD pipelines for static web projects.

Simply drag and drop an HTML file or a bundled ZIP archive, choose your route, and publish. Your website goes live instantly under the main domain path.

Examples:
* `https://example.com/admin` &mdash; Modern administration, deployment, and file management console
* `https://example.com/` &mdash; Root primary website
* `https://example.com/docs` &mdash; Documentation sites (VitePress, Docusaurus, MkDocs, Astro)
* `https://example.com/demo` &mdash; Interactive demos, HTML5 games, and prototypes
* `https://example.com/shop` &mdash; Static e-commerce landing pages and storefronts
* `https://example.com/20/10` &mdash; Nested subpath campaigns, releases, and seasonal promotions

## Key Features

### 1. Instant Zero-Config Publishing
Upload single HTML files, bundled ZIP archives, or rich documentation folders. Zploy automatically prepares staging, validates integrity, extracts contents safely, flattens nested root folders, and binds the website to your requested path in milliseconds.

### 2. Full Markdown and Modern Asset Support
Deploy not just HTML and CSS, but complete static site generator outputs, documentation wikis, and modern web bundles:
* Markdown and documentation assets: `.md`, `.markdown`, `.yaml`, `.yml`, `.toml`, `.csv`, `.tsv`
* Modern graphics and media: `.avif`, `.webp`, `.apng`, `.svg`, `.mp4`, `.webm`, `.mp3`
* Web applications: `.js`, `.mjs`, `.cjs`, `.wasm`, `.json`, `.map`
* Perfect for VitePress, Docusaurus, Astro, Hugo, MkDocs, Jekyll, and Next.js static exports

### 3. Multi-Version Atomic Releases and One-Click Rollback
Every deployment generates an immutable numbered release (`release-0001`, `release-0002`). Deployment switches atomically in MySQL/MariaDB transactions with zero downtime. If an update introduces errors, revert immediately to any earlier release with a single click.

### 4. Smart Trailing Slash Routing and Relative Assets
Proper static site serving requires trailing slash normalization. Zploy automatically redirects clean paths (`/shop` to `/shop/`), ensuring relative stylesheet paths (`./style.css`), script tags (`./app.js`), and media references load seamlessly without 404 errors.

### 5. High-Performance Static Streaming Engine
* ETag and If-None-Match HTTP caching with 304 Not Modified responses
* HTTP 206 Partial Content range requests for seamless video and audio streaming
* Accurate MIME type detection with UTF-8 charset specification
* Optional Single Page Application (SPA) fallback mode

### 6. Multi-Layer Security Architecture
* Zip Slip and Zip Bomb defense with extraction quota and uncompressed size enforcement
* Path traversal immunity preventing directory escape attempts
* Automatic root folder flattening for archives zipped with nested root folders
* Strict file extension filtering rejecting executable and server-side script binaries
* Argon2id password hashing with SHA-256 session token verification
* Cross-Site Request Forgery (CSRF) header validation on mutating operations

### 7. Apple-Inspired Minimalist Dashboard
* Clean, fluid responsive interface with dark and light theme options
* Real-time debounced path availability validation
* Interactive file inspector and original ZIP archive download
* Client-side static analyzer warning if absolute paths like `/style.css` are used instead of `./style.css`

## Deployment Flow

```
(1) User Upload
    Drop .zip or .html into Zploy Web Dashboard or POST to /api/uploads/quick-publish
    v
(2) Security and Extraction
    Validate extensions -> Unpack staging -> Flatten root folder -> Sanitize paths
    v
(3) Atomic Database Commit
    Generate release record -> Update current_release_id in MySQL -> Instant live
    v
(4) High-Performance Serving
    Fast file resolution -> ETag caching -> HTTP 206 streaming -> Trailing slash router
```

## Quick Start with Docker Compose

The fastest way to deploy Zploy with an isolated MariaDB database:

```bash
git clone https://github.com/kiyogo/zploy.git
cd zploy
docker compose up -d
```

Open `http://localhost:3000/admin` in your browser to complete the initial setup wizard and start deploying websites.

## Native Quick Start

### Prerequisites
* Rust 1.75+ (or precompiled binary)
* MySQL 8.0+ or MariaDB 10.5+

### Installation and Build

Clone the repository:
```bash
git clone https://github.com/kiyogo/zploy.git
cd zploy
```

Configure your database in `config/config.toml`:
```toml
[server]
host = "0.0.0.0"
port = 3000
base_url = "http://localhost:3000"

[database]
url = "mysql://root:password@127.0.0.1:3306/zploy"
```

Compile the optimized release binary:
```bash
cargo build --release
```

Run the server:
```bash
./target/release/zploy
```

Access the administration console at `http://localhost:3000/admin`.

## Password Reset Utility

If you forget the administrator credentials, reset them directly from the CLI without running raw database queries:

```bash
cargo run --bin reset_admin -- admin newpassword123
```

On Windows:
```cmd
reset_admin.bat admin newpassword123
```

## Configuration Reference

Configuration can be specified in `config/config.toml` or overridden using environment variables:

* `ZPLOY_PORT` &mdash; Server port (default: `3000`)
* `ZPLOY_HOST` &mdash; Bind address (default: `0.0.0.0`)
* `ZPLOY_BASE_URL` &mdash; Public base URL (default: `http://localhost:3000`)
* `DATABASE_URL` &mdash; MySQL/MariaDB connection URL
* `ZPLOY_CONFIG` &mdash; Path to custom configuration TOML file

Storage folders created automatically:
* `storage/sites/` &mdash; Production site releases
* `storage/staging/` &mdash; Temporary upload unpack staging
* `storage/temp/` &mdash; Working archive storage

## REST API Overview

All operational endpoints are routed under the `/api` prefix:

Authentication:
* `POST /api/auth/setup` &mdash; First-time setup wizard
* `POST /api/auth/login` &mdash; Administrator sign in
* `POST /api/auth/logout` &mdash; Terminate session
* `GET  /api/auth/me` &mdash; Current user profile and session verification

Website Management:
* `GET    /api/sites` &mdash; List all published websites
* `POST   /api/sites` &mdash; Create a new site entry
* `GET    /api/sites/:id` &mdash; Inspect site details
* `PATCH  /api/sites/:id` &mdash; Update site title or slug
* `DELETE /api/sites/:id` &mdash; Delete site and all associated releases
* `GET    /api/sites/:id/files` &mdash; Browse directory structure and files
* `GET    /api/sites/:id/download` &mdash; Download current site source as ZIP

Releases and Rollbacks:
* `GET  /api/sites/:id/releases` &mdash; List release history
* `POST /api/sites/:id/releases` &mdash; Upload new version release
* `POST /api/sites/:id/rollback` &mdash; Roll back to a previous release version

System and Utilities:
* `GET  /api/slugs/check?slug=:slug` &mdash; Validate slug format and availability
* `POST /api/uploads/quick-publish` &mdash; Instant multipart file upload and deployment
* `GET  /api/health` &mdash; Service health check
* `GET  /api/system/stats` &mdash; Aggregate hosting statistics

## Security Best Practices

1. Run behind a reverse proxy (Caddy, Nginx, Cloudflare) with HTTPS enabled.
2. In production, configure `cookie_secure = true` in `config/config.toml`.
3. Set a strong password for the initial administrator account.
4. Keep the storage directory outside web server root paths.

## Tech Stack

* Core Engine: Rust
* Web Framework: Axum 0.7
* Async Runtime: Tokio
* HTTP Tools: Tower, Tower-HTTP, Http-Body-Util
* Database Engine: SQLx with MySQL and MariaDB connection pooling
* Authentication and Hashing: Argon2id, SHA-256
* Compression: Zip-rs with deflate support
* Frontend: Vanilla JavaScript, Apple-inspired CSS design system

## License

Released under the MIT License. Copyright (c) 2026 KiyoGo.
