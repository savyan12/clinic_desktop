use super::{
    auth::AuthUser,
    error::{ApiError, ApiResult},
};
use axum::http::{HeaderMap, StatusCode};
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::{MySql, MySqlPool, Row, Transaction};
use std::{future::Future, pin::Pin};
use uuid::Uuid;

pub type OperationFuture<'a> =
    Pin<Box<dyn Future<Output = ApiResult<(StatusCode, Value)>> + Send + 'a>>;

const MAX_TRANSACTION_ATTEMPTS: usize = 3;

pub fn require_key(headers: &HeaderMap) -> ApiResult<String> {
    let key = headers
        .get("Idempotency-Key")
        .and_then(|value| value.to_str().ok())
        .filter(|value| Uuid::parse_str(value).is_ok())
        .ok_or_else(|| {
            ApiError::validation("مفتاح العملية مطلوب ويجب أن يكون UUID صالحًا.")
                .field("idempotency_key", "أرسل ترويسة Idempotency-Key صالحة.")
        })?;

    Ok(key.to_owned())
}

pub async fn execute<F>(
    pool: &MySqlPool,
    auth: &AuthUser,
    key: &str,
    method: &str,
    route: &str,
    payload: &Value,
    mut operation: F,
) -> ApiResult<(StatusCode, Value)>
where
    F: for<'a> FnMut(&'a mut Transaction<'_, MySql>) -> OperationFuture<'a>,
{
    for attempt in 1..=MAX_TRANSACTION_ATTEMPTS {
        let result = execute_once(pool, auth, key, method, route, payload, &mut operation).await;
        match result {
            Err(error)
                if error.is_retryable_transaction() && attempt < MAX_TRANSACTION_ATTEMPTS =>
            {
                continue;
            }
            result => return result,
        }
    }

    unreachable!("transaction retry loop always returns")
}

async fn execute_once<F>(
    pool: &MySqlPool,
    auth: &AuthUser,
    key: &str,
    method: &str,
    route: &str,
    payload: &Value,
    operation: &mut F,
) -> ApiResult<(StatusCode, Value)>
where
    F: for<'a> FnMut(&'a mut Transaction<'_, MySql>) -> OperationFuture<'a>,
{
    let route = route.trim_start_matches('/');
    let request_hash = request_hash(method, route, payload);
    let mut transaction = pool.begin().await?;

    if let Some(replayed) =
        lookup_locked(&mut transaction, auth.user.id, route, key, &request_hash).await?
    {
        transaction.commit().await?;
        return Ok(replayed);
    }

    let insert = sqlx::query(
        "INSERT INTO idempotency_keys \
         (user_id, `key`, route, request_hash, status, response, created_at, updated_at) \
         VALUES (?, ?, ?, ?, NULL, NULL, NOW(), NOW())",
    )
    .bind(auth.user.id)
    .bind(key)
    .bind(route)
    .bind(&request_hash)
    .execute(&mut *transaction)
    .await;

    if let Err(error) = insert {
        transaction.rollback().await?;
        if is_unique_violation(&error) {
            return replay_after_race(pool, auth.user.id, route, key, &request_hash).await;
        }
        return Err(error.into());
    }

    let (status, response) = match operation(&mut transaction).await {
        Ok(result) => result,
        Err(error) => {
            let _ = transaction.rollback().await;
            return Err(error);
        }
    };
    if status.is_server_error() {
        return Err(ApiError::internal(
            "Idempotent operation failed with a server error.",
        ));
    }

    sqlx::query(
        "UPDATE idempotency_keys SET status = ?, response = ?, updated_at = NOW() \
         WHERE user_id = ? AND route = ? AND `key` = ?",
    )
    .bind(status.as_u16())
    .bind(&response)
    .bind(auth.user.id)
    .bind(route)
    .bind(key)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;

    Ok((status, response))
}

async fn lookup_locked(
    transaction: &mut Transaction<'_, MySql>,
    user_id: u64,
    route: &str,
    key: &str,
    request_hash: &str,
) -> ApiResult<Option<(StatusCode, Value)>> {
    let record = sqlx::query(
        "SELECT request_hash, status, response FROM idempotency_keys \
         WHERE user_id = ? AND route = ? AND `key` = ? FOR UPDATE",
    )
    .bind(user_id)
    .bind(route)
    .bind(key)
    .fetch_optional(&mut **transaction)
    .await?;

    record.map(|row| replay_row(row, request_hash)).transpose()
}

async fn replay_after_race(
    pool: &MySqlPool,
    user_id: u64,
    route: &str,
    key: &str,
    request_hash: &str,
) -> ApiResult<(StatusCode, Value)> {
    let row = sqlx::query(
        "SELECT request_hash, status, response FROM idempotency_keys \
         WHERE user_id = ? AND route = ? AND `key` = ? LIMIT 1",
    )
    .bind(user_id)
    .bind(route)
    .bind(key)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| ApiError::conflict("العملية نفسها قيد التنفيذ، حاول بعد قليل."))?;

    replay_row(row, request_hash)
}

fn replay_row(row: sqlx::mysql::MySqlRow, request_hash: &str) -> ApiResult<(StatusCode, Value)> {
    let stored_hash: String = row.try_get("request_hash")?;
    if stored_hash != request_hash {
        return Err(ApiError::conflict("مفتاح العملية مستخدم لطلب مختلف."));
    }

    let status = row.try_get::<Option<u16>, _>("status")?;
    let response = row.try_get::<Option<Value>, _>("response")?;
    match (status, response) {
        (Some(status), Some(response)) => Ok((
            StatusCode::from_u16(status).unwrap_or(StatusCode::OK),
            response,
        )),
        _ => Err(ApiError::conflict(
            "العملية نفسها قيد التنفيذ، حاول بعد قليل.",
        )),
    }
}

fn request_hash(method: &str, route: &str, payload: &Value) -> String {
    let method = serde_json::to_string(method).expect("HTTP method is serializable");
    let route = serde_json::to_string(route).expect("route is serializable");
    let payload = serde_json::to_string(&canonical(payload)).expect("payload is serializable");
    let encoded = format!("{{\"method\":{method},\"route\":{route},\"payload\":{payload}}}");
    hex::encode(Sha256::digest(encoded.as_bytes()))
}

fn canonical(value: &Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(key, value)| (key.clone(), canonical(value)))
                .collect(),
        ),
        Value::Array(values) => Value::Array(values.iter().map(canonical).collect()),
        Value::Number(number) if number.as_f64().is_some_and(|value| value.fract() == 0.0) => {
            let value = number.as_f64().unwrap();
            if value >= 0.0 {
                Value::from(value as u64)
            } else {
                Value::from(value as i64)
            }
        }
        other => other.clone(),
    }
}

fn is_unique_violation(error: &sqlx::Error) -> bool {
    matches!(error, sqlx::Error::Database(database) if database.is_unique_violation())
}

#[cfg(test)]
mod tests {
    use super::request_hash;
    use serde_json::json;

    #[test]
    fn hash_is_stable_for_associative_key_order() {
        let first = request_hash(
            "POST",
            "api/v1/example",
            &json!({"b": 2, "a": {"d": 4, "c": 3}}),
        );
        let second = request_hash(
            "POST",
            "api/v1/example",
            &json!({"a": {"c": 3, "d": 4}, "b": 2}),
        );
        assert_eq!(first, second);
    }
}
