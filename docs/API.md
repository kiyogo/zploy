# Zploy REST API Specification

All administrative and operational endpoints are routed under the `/api` prefix.
Mutating HTTP requests (`POST`, `PUT`, `PATCH`, `DELETE`) require the CSRF protection header `X-Zploy-CSRF: 1` or an `Authorization: Bearer <token>` header.

## 1. Authentication & Session Management

### 1.1 Check Authentication Status
* **Endpoint**: `GET /api/auth/me`
* **Access**: Public
* **Description**: Returns current session identity and indicates whether the primary administrator account has been initialized.
* **Response `200 OK`**:
```json
{
  "authenticated": true,
  "initialized": true,
  "user": {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "username": "admin",
    "role": "admin"
  }
}
```

### 1.2 Initial Setup Wizard
* **Endpoint**: `POST /api/auth/setup`
* **Access**: Only available when no user exists in the system
* **Request Body**:
```json
{
  "username": "admin",
  "password": "YourStrongPassword123"
}
```
* **Response `201 Created`**:
```json
{
  "success": true,
  "message": "Administrator account created successfully. Please sign in."
}
```

### 1.3 Sign In
* **Endpoint**: `POST /api/auth/login`
* **Access**: Public
* **Request Body**:
```json
{
  "username": "admin",
  "password": "admin123"
}
```
* **Response `200 OK`**:
  * Automatically sets session cookie: `Set-Cookie: zploy_session=<token>; Path=/; HttpOnly; SameSite=Lax`
```json
{
  "success": true,
  "message": "Signed in successfully",
  "token": "a1b2c3d4e5f6...",
  "user": {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "username": "admin",
    "role": "admin"
  }
}
```

### 1.4 Sign Out
* **Endpoint**: `POST /api/auth/logout`
* **Access**: Authenticated
* **Response `200 OK`**:
```json
{
  "success": true,
  "message": "Signed out successfully"
}
```

## 2. Slugs & Path Routing

### 2.1 Check Slug Availability
* **Endpoint**: `GET /api/slugs/check?slug=demo`
* **Access**: Public
* **Description**: Verifies format syntax, checks reserved system words, and checks database collisions.
* **Response `200 OK` (Available)**:
```json
{
  "valid": true,
  "available": true,
  "slug": "demo",
  "message": "Path is available for publishing"
}
```

## 3. Website Management

### 3.1 List Websites
* **Endpoint**: `GET /api/sites`
* **Access**: Public / Authenticated

### 3.2 Inspect Website Files
* **Endpoint**: `GET /api/sites/:id/files`
* **Access**: Public / Authenticated

### 3.3 Download Website ZIP
* **Endpoint**: `GET /api/sites/:id/download`
* **Access**: Authenticated

### 3.4 Update Website Metadata
* **Endpoint**: `PATCH /api/sites/:id`
* **Access**: Authenticated
* **Request Body**:
```json
{
  "name": "Updated Title",
  "slug": "new-path"
}
```

### 3.5 Delete Website
* **Endpoint**: `DELETE /api/sites/:id`
* **Access**: Authenticated

## 4. Releases & Rollbacks

### 4.1 Upload New Release
* **Endpoint**: `POST /api/sites/:id/releases`
* **Access**: Authenticated
* **Content-Type**: `multipart/form-data`

### 4.2 List Release History
* **Endpoint**: `GET /api/sites/:id/releases`
* **Access**: Authenticated

### 4.3 Rollback Release
* **Endpoint**: `POST /api/sites/:id/rollback`
* **Access**: Authenticated
* **Request Body**:
```json
{
  "release_id": "release-uuid-here"
}
```

## 5. Instant Quick Publish

### 5.1 Quick Publish Endpoint
* **Endpoint**: `POST /api/uploads/quick-publish`
* **Access**: Authenticated
* **Content-Type**: `multipart/form-data`
* **Form Fields**:
  * `slug`: Website subpath (e.g. `shop`, `20/10`, or empty for root)
  * `name`: Optional display title
  * `note`: Optional release description note
  * `files`: Uploaded archive or HTML file(s)

## 6. System & Monitoring

### 6.1 Health Check
* **Endpoint**: `GET /api/health`
* **Response `200 OK`**:
```json
{
  "status": "ok",
  "version": "0.1.0",
  "service": "Zploy"
}
```

### 6.2 System Statistics
* **Endpoint**: `GET /api/system/stats`
* **Access**: Authenticated
