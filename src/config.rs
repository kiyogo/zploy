use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub storage: StorageConfig,
    pub upload: UploadConfig,
    pub auth: AuthConfig,
    pub security: SecurityConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub base_url: String,
    pub trust_proxy: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    pub root_dir: PathBuf,
    pub sites_dir: PathBuf,
    pub staging_dir: PathBuf,
    pub temp_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadConfig {
    pub max_upload_size_mb: usize,
    pub max_uncompressed_size_mb: usize,
    pub max_file_count: usize,
    pub max_single_file_size_mb: usize,
    pub allowed_extensions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub session_expiry_hours: i64,
    pub cookie_secure: bool,
    pub csrf_protection: bool,
    pub initial_admin_username: String,
    pub initial_admin_password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub reserved_slugs: Vec<String>,
    pub enable_spa_fallback: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                host: "0.0.0.0".to_string(),
                port: 3000,
                base_url: "http://localhost:3000".to_string(),
                trust_proxy: false,
            },
            database: DatabaseConfig {
                url: "mysql://root:@127.0.0.1:3306/minihost".to_string(),
            },
            storage: StorageConfig {
                root_dir: PathBuf::from("./storage"),
                sites_dir: PathBuf::from("./storage/sites"),
                staging_dir: PathBuf::from("./storage/staging"),
                temp_dir: PathBuf::from("./storage/temp"),
            },
            upload: UploadConfig {
                max_upload_size_mb: 100,
                max_uncompressed_size_mb: 250,
                max_file_count: 5000,
                max_single_file_size_mb: 50,
                allowed_extensions: vec![
                    "html", "htm", "css", "js", "mjs", "json",
                    "svg", "png", "jpg", "jpeg", "webp", "gif", "ico",
                    "woff", "woff2", "ttf", "eot", "otf",
                    "txt", "xml", "webmanifest", "wasm",
                    "mp3", "mp4", "webm", "ogg", "wav", "pdf", "map",
                ]
                .into_iter()
                .map(String::from)
                .collect(),
            },
            auth: AuthConfig {
                session_expiry_hours: 72,
                cookie_secure: false,
                csrf_protection: true,
                initial_admin_username: "admin".to_string(),
                initial_admin_password: String::new(),
            },
            security: SecurityConfig {
                reserved_slugs: vec![
                    "api", "assets", "public", "static", "login", "logout",
                    "admin", "health", "system", "setup", "dashboard", "app",
                    "favicon.ico", "robots.txt", "sitemap.xml", "manifest.json",
                ]
                .into_iter()
                .map(String::from)
                .collect(),
                enable_spa_fallback: false,
            },
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let config_path = std::env::var("ZPLOY_CONFIG")
            .or_else(|_| std::env::var("MINIHOST_CONFIG"))
            .unwrap_or_else(|_| "config/config.toml".to_string());

        let mut config = if Path::new(&config_path).exists() {
            let content = std::fs::read_to_string(&config_path)
                .with_context(|| format!("Failed to read configuration file: {}", config_path))?;
            toml::from_str(&content)
                .with_context(|| format!("TOML syntax error in configuration file: {}", config_path))?
        } else if Path::new("config/config.example.toml").exists() {
            let content = std::fs::read_to_string("config/config.example.toml")
                .context("Failed to read sample configuration file config/config.example.toml")?;
            toml::from_str(&content).unwrap_or_default()
        } else {
            Config::default()
        };

        // Environment overrides
        if let Ok(host) = std::env::var("ZPLOY_HOST").or_else(|_| std::env::var("MINIHOST_HOST")) {
            config.server.host = host;
        }
        if let Ok(port) = std::env::var("ZPLOY_PORT").or_else(|_| std::env::var("MINIHOST_PORT")) {
            if let Ok(p) = port.parse() {
                config.server.port = p;
            }
        }
        if let Ok(db_url) = std::env::var("DATABASE_URL") {
            config.database.url = db_url;
        }
        if let Ok(base_url) = std::env::var("ZPLOY_BASE_URL").or_else(|_| std::env::var("MINIHOST_BASE_URL")) {
            config.server.base_url = base_url;
        }

        // Ensure storage directories exist
        std::fs::create_dir_all(&config.storage.sites_dir)
            .context("Failed to create storage/sites directory")?;
        std::fs::create_dir_all(&config.storage.staging_dir)
            .context("Failed to create storage/staging directory")?;
        std::fs::create_dir_all(&config.storage.temp_dir)
            .context("Failed to create storage/temp directory")?;

        Ok(config)
    }
}
