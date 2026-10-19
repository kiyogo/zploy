pub mod models;

use anyhow::{Context, Result};
use chrono::Utc;
use sqlx::{
    mysql::{MySqlConnectOptions, MySqlPoolOptions},
    MySqlPool,
};
use std::str::FromStr;
use tracing::info;

use crate::config::Config;
use crate::security::password;

pub type DbPool = MySqlPool;

pub async fn init_db(config: &Config) -> Result<DbPool> {
    let connect_options = MySqlConnectOptions::from_str(&config.database.url)?;

    let pool = MySqlPoolOptions::new()
        .max_connections(20)
        .connect_with(connect_options)
        .await
        .context("Failed to connect to MySQL database")?;

    // Run embedded migrations
    if let Err(e) = sqlx::migrate!("./migrations").run(&pool).await {
        let err_str = e.to_string();
        if !err_str.contains("Duplicate entry")
            && !err_str.contains("already exists")
            && !err_str.contains("has been modified")
        {
            return Err(e).context("Error running MySQL database migrations");
        }
    }

    info!("Connected to MySQL database and completed migrations.");

    // Seed initial admin if specified in config and no users exist
    seed_initial_admin_if_needed(&pool, config).await?;

    Ok(pool)
}

async fn seed_initial_admin_if_needed(pool: &DbPool, config: &Config) -> Result<()> {
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
        .fetch_one(pool)
        .await?;

    if count.0 == 0 && !config.auth.initial_admin_password.trim().is_empty() {
        let password_hash = password::hash_password(&config.auth.initial_admin_password)?;
        let user_id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            r#"
            INSERT INTO users (id, username, password_hash, role, created_at, updated_at)
            VALUES (?, ?, ?, 'admin', ?, ?)
            "#,
        )
        .bind(user_id)
        .bind(&config.auth.initial_admin_username)
        .bind(password_hash)
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await?;

        info!(
            "Default administrator account initialized: {}",
            config.auth.initial_admin_username
        );
    }

    Ok(())
}
