use anyhow::Result;
use chrono::Utc;
use sqlx::mysql::MySqlPoolOptions;
use std::env;
use zploy::config::Config;
use zploy::security::hash_password;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    let username = args.get(1).map(|s| s.as_str()).unwrap_or("admin");
    let password = args.get(2).map(|s| s.as_str()).unwrap_or("admin123");

    println!("========================================================");
    println!("     Zploy - Reset Administrator Password");
    println!("========================================================");

    let config = Config::load().unwrap_or_default();
    let db_path = &config.database.url;

    let pool = match MySqlPoolOptions::new().connect(db_path).await {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Failed to connect to MySQL database: {}", e);
            std::process::exit(1);
        }
    };

    let hash = match hash_password(password) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("Password hashing error: {}", e);
            std::process::exit(1);
        }
    };

    let now = Utc::now().to_rfc3339();

    // Check if user already exists
    let affected = sqlx::query("UPDATE users SET password_hash = ?, updated_at = ? WHERE username = ?")
        .bind(&hash)
        .bind(&now)
        .bind(username)
        .execute(&pool)
        .await?;

    if affected.rows_affected() == 0 {
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO users (id, username, password_hash, role, created_at, updated_at) VALUES (?, ?, ?, 'admin', ?, ?)")
            .bind(&id)
            .bind(username)
            .bind(&hash)
            .bind(&now)
            .bind(&now)
            .execute(&pool)
            .await?;
        println!(" New administrator account created successfully!");
    } else {
        println!(" Password updated successfully!");
    }

    // Invalidate all existing sessions to force re-login
    let _ = sqlx::query("DELETE FROM sessions").execute(&pool).await;

    println!(" Username: {}", username);
    println!(" New Password: {}", password);
    println!("========================================================");

    Ok(())
}
