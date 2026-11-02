use super::error::ApiResult;
use serde_json::Value;
use sqlx::{MySql, Transaction};

pub async fn log(
    transaction: &mut Transaction<'_, MySql>,
    user_id: u64,
    action: &str,
    entity_type: &str,
    entity_id: u64,
    old_values: Option<&Value>,
    new_values: Option<&Value>,
    reason: Option<&str>,
) -> ApiResult<()> {
    sqlx::query(
        "INSERT INTO audit_logs (user_id, action, entity_type, entity_id, old_values, new_values, reason, created_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, NOW())",
    )
    .bind(user_id)
    .bind(action)
    .bind(entity_type)
    .bind(entity_id)
    .bind(old_values)
    .bind(new_values)
    .bind(reason)
    .execute(&mut **transaction)
    .await?;

    Ok(())
}
