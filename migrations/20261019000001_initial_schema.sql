-- Schema migration for MiniHost (MySQL / MariaDB)
CREATE TABLE IF NOT EXISTS users (
    id VARCHAR(36) PRIMARY KEY NOT NULL,
    username VARCHAR(191) NOT NULL UNIQUE,
    password_hash VARCHAR(255) NOT NULL,
    role VARCHAR(50) NOT NULL DEFAULT 'admin',
    created_at VARCHAR(64) NOT NULL DEFAULT '',
    updated_at VARCHAR(64) NOT NULL DEFAULT ''
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS sessions (
    id VARCHAR(36) PRIMARY KEY NOT NULL,
    user_id VARCHAR(36) NOT NULL,
    token_hash VARCHAR(191) NOT NULL UNIQUE,
    expires_at VARCHAR(64) NOT NULL,
    created_at VARCHAR(64) NOT NULL DEFAULT '',
    INDEX idx_sessions_token_hash (token_hash),
    CONSTRAINT fk_sessions_user FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS sites (
    id VARCHAR(36) PRIMARY KEY NOT NULL,
    owner_id VARCHAR(36) NOT NULL,
    name VARCHAR(255) NOT NULL,
    slug VARCHAR(191) NOT NULL UNIQUE,
    active_release_id VARCHAR(36),
    status VARCHAR(50) NOT NULL DEFAULT 'published',
    total_size_bytes BIGINT NOT NULL DEFAULT 0,
    file_count BIGINT NOT NULL DEFAULT 0,
    spa_fallback BIGINT NOT NULL DEFAULT 0,
    created_at VARCHAR(64) NOT NULL DEFAULT '',
    updated_at VARCHAR(64) NOT NULL DEFAULT '',
    INDEX idx_sites_slug (slug),
    CONSTRAINT fk_sites_owner FOREIGN KEY (owner_id) REFERENCES users(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS releases (
    id VARCHAR(36) PRIMARY KEY NOT NULL,
    site_id VARCHAR(36) NOT NULL,
    version_num BIGINT NOT NULL,
    storage_path VARCHAR(500) NOT NULL,
    file_count BIGINT NOT NULL DEFAULT 0,
    total_size_bytes BIGINT NOT NULL DEFAULT 0,
    status VARCHAR(50) NOT NULL DEFAULT 'active',
    note TEXT,
    created_at VARCHAR(64) NOT NULL DEFAULT '',
    INDEX idx_releases_site_id (site_id),
    CONSTRAINT fk_releases_site FOREIGN KEY (site_id) REFERENCES sites(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS upload_jobs (
    id VARCHAR(36) PRIMARY KEY NOT NULL,
    site_id VARCHAR(36),
    status VARCHAR(50) NOT NULL DEFAULT 'pending',
    error_message TEXT,
    created_at VARCHAR(64) NOT NULL DEFAULT '',
    completed_at VARCHAR(64),
    CONSTRAINT fk_upload_jobs_site FOREIGN KEY (site_id) REFERENCES sites(id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS audit_logs (
    id VARCHAR(36) PRIMARY KEY NOT NULL,
    user_id VARCHAR(36),
    action VARCHAR(100) NOT NULL,
    target_type VARCHAR(100) NOT NULL,
    target_id VARCHAR(36),
    details TEXT,
    ip_address VARCHAR(100),
    created_at VARCHAR(64) NOT NULL DEFAULT '',
    INDEX idx_audit_logs_created (created_at),
    CONSTRAINT fk_audit_logs_user FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
