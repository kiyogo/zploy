# Zploy Security Architecture & Policy

Security is a primary design tenet of Zploy. When accepting arbitrary file uploads from users and serving them publicly, multi-layered containment is critical.

## 1. Threat Mitigation Model

### 1.1 Zip Slip & Path Traversal Prevention
* **Threat**: Malicious ZIP archives containing relative paths (such as `../../../../etc/passwd`) designed to overwrite system files during extraction.
* **Mitigation**: Zploy verifies every archive entry path using canonical prefix checks. Any attempt to write outside `storage/staging/` triggers an immediate abort and audit warning.

### 1.2 Extension Whitelisting
* **Threat**: Uploading executable code (`.php`, `.py`, `.sh`, `.exe`, `.cgi`) to execute server-side commands.
* **Mitigation**: Zploy strictly enforces a whitelist of safe static extensions (HTML, CSS, JS, images, fonts, media, and source maps). All other extensions are rejected immediately during staging validation.

### 1.3 Zip Bomb & Quota Protection
* **Threat**: Highly compressed files expanding into gigabytes of data to exhaust server disk space.
* **Mitigation**: During extraction, Zploy tracks total uncompressed bytes against `max_uncompressed_size_mb` and file counts against `max_file_count`. If thresholds are crossed, extraction terminates immediately.

### 1.4 CSRF Protection
* **Threat**: Cross-site request forgery tricking authenticated browsers into modifying sites.
* **Mitigation**: Mutating operations require the custom `X-Zploy-CSRF: 1` header, which cannot be forged across origins by standard HTML form submissions or unauthenticated fetch calls.

### 1.5 Secure Authentication
* Passwords hashed using industry-standard **Argon2id**.
* Session tokens generated via cryptographic random number generators (CSPRNG) and stored as SHA-256 digests in MySQL.
* Session cookies configured with `HttpOnly` and `SameSite=Lax`.
