# Zploy System Architecture

Zploy is a high-performance, minimalist, and secure static web hosting server engineered in **Rust**. It enables users to deploy and publish static web applications (HTML, CSS, JS, media, or ZIP archives) instantly under custom paths (e.g. `https://example.com/demo` or `https://example.com/`).

## 1. High-Level Architecture Diagram

```mermaid
graph TD
    User([User / Web Browser]) -->|Visit /admin| FrontEnd[Zploy Dashboard Vanilla JS/CSS]
    User -->|Visit /{slug}/| StaticRouter[Axum Static Hosting Handler]
    User -->|Call API /api/*| ApiRouter[Axum REST API Router]

    subgraph Zploy Backend [Zploy Rust Core]
        ApiRouter --> Auth[Auth & Session Guard]
        ApiRouter --> UploadProcessor[Upload & Multipart Engine]
        UploadProcessor --> ZipSecurity[Zip Slip & Bomb Detector]
        
        ZipSecurity --> Staging[storage/staging/ UUID]
        Staging --> SiteService[Site & Release Service]
        SiteService --> AtomicDeploy[Atomic Release Switch]
        AtomicDeploy --> SitesStorage[storage/sites/{site_id}/releases/release-{v}/]
        
        StaticRouter --> TrailingSlashRedirect[Trailing Slash Resolver]
        TrailingSlashRedirect --> PathTraversalGuard[Safe Path & Symlink Resolver]
        PathTraversalGuard --> MimeAndCache[MIME Guesser, ETag & HTTP Range]
        MimeAndCache --> SitesStorage
        
        SiteService --> Database[(MySQL / MariaDB)]
        Auth --> Database
    end

    subgraph Production Deployment
        Internet([Internet Visitors]) --> Proxy[Caddy / Nginx Reverse Proxy HTTPS]
        Proxy -->|Proxy 127.0.0.1:3000| Zploy
    end
```

## 2. Core Components

### 2.1 Backend (Rust + Axum + Tokio)
* **Framework**: `Axum 0.7` running on Tokio multithreaded non-blocking async runtime.
* **Resource Optimization**: Zero per-site processes or virtual hosts. A single efficient binary serves thousands of sites concurrently with microsecond latency.
* **Streaming Uploads**: Multipart uploads stream directly to disk buffer in `storage/temp/` without exhausting memory.

### 2.2 Storage & Immutable Releases
Each site uses an internal UUID v4 independent of its URL slug.
```text
storage/
├── sites/
│   └── {site-uuid}/
│       └── releases/
│           ├── release-0001/
│           │   ├── index.html
│           │   ├── css/
│           │   └── js/
│           └── release-0002/
│               └── ...
├── staging/
└── temp/
```

### 2.3 Atomic Switching
Publishing a new release updates `current_release_id` in a database transaction. Incoming web requests immediately resolve to the new release without interruption or file-lock conflicts.

### 2.4 Multi-Level Security Engine
* **Zip Slip Defense**: Validates canonical paths before extraction.
* **Path Traversal Defense**: Prevents directory escape (`..`, symlink jailbreaks).
* **Extension Whitelist**: Only allows safe static assets; rejects executable binaries and server-side script files.
* **Argon2id**: High-security password hashing for administrators.
