use chrono::Utc;
use sqlx::MySqlPool;
use tracing::warn;

pub struct AuditService;

impl AuditService {
    pub async fn log(
        pool: &MySqlPool,
        user_id: Option<&str>,
        action: &str,
        target_type: &str,
        target_id: Option<&str>,
        details: Option<&str>,
        ip_address: Option<&str>,
    ) {
        let id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        if let Err(e) = sqlx::query(
            r#"
            INSERT INTO audit_logs (id, user_id, action, target_type, target_id, details, ip_address, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&id)
        .bind(user_id)
        .bind(action)
        .bind(target_type)
        .bind(target_id)
        .bind(details)
        .bind(ip_address)
        .bind(&now)
        .execute(pool)
        .await
        {
            warn!("Failed to record audit log: {}", e);
        }
    }
}
