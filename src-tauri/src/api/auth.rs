use super::{error::ApiError, rows::UserRow, time, AppState};
use axum::{
    extract::FromRequestParts,
    http::{header::AUTHORIZATION, request::Parts},
};
use rand::{distr::Alphanumeric, Rng};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

const LOGIN_LIMIT: usize = 5;
const LOGIN_WINDOW: Duration = Duration::from_secs(60);

#[derive(Clone, Default)]
pub struct LoginRateLimiter {
    attempts: Arc<Mutex<HashMap<String, VecDeque<Instant>>>>,
}

impl LoginRateLimiter {
    pub fn record(&self, username: &str) -> Result<(), ApiError> {
        let now = Instant::now();
        let mut attempts = self
            .attempts
            .lock()
            .map_err(|_| ApiError::internal("Login rate limiter lock is poisoned."))?;
        let key = username.to_lowercase();
        let entries = attempts.entry(key).or_default();
        while entries
            .front()
            .is_some_and(|attempt| now.duration_since(*attempt) >= LOGIN_WINDOW)
        {
            entries.pop_front();
        }
        if entries.len() >= LOGIN_LIMIT {
            return Err(ApiError::new(
                axum::http::StatusCode::TOO_MANY_REQUESTS,
                "تجاوزت عدد محاولات تسجيل الدخول المسموح. حاول بعد دقيقة.",
            ));
        }
        entries.push_back(now);
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user: UserRow,
    pub token_id: u64,
}

impl AuthUser {
    pub fn require_admin(&self) -> Result<(), ApiError> {
        if self.user.is_active && self.user.role == "admin" {
            Ok(())
        } else {
            Err(ApiError::forbidden("ليس لديك صلاحية لتنفيذ هذا الإجراء."))
        }
    }
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let bearer = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("Bearer "))
            .ok_or_else(ApiError::unauthorized)?;
        let (token_id, plain) = bearer.split_once('|').ok_or_else(ApiError::unauthorized)?;
        let token_id = token_id
            .parse::<u64>()
            .map_err(|_| ApiError::unauthorized())?;
        let digest = hex::encode(Sha256::digest(plain.as_bytes()));

        let user = sqlx::query_as::<_, UserRow>(
            "SELECT u.id, u.name, u.username, u.password, u.role, u.is_active, \
                    u.last_login_at, u.created_at, u.updated_at \
             FROM personal_access_tokens AS token \
             INNER JOIN users AS u ON u.id = token.tokenable_id \
             WHERE token.id = ? AND token.token = ? \
               AND token.tokenable_type = 'App\\\\Models\\\\User' \
               AND (token.expires_at IS NULL OR token.expires_at > NOW()) \
             LIMIT 1",
        )
        .bind(token_id)
        .bind(digest)
        .fetch_optional(&state.pool)
        .await?
        .filter(|user| user.is_active)
        .ok_or_else(ApiError::unauthorized)?;

        sqlx::query("UPDATE personal_access_tokens SET last_used_at = NOW(), updated_at = NOW() WHERE id = ?")
            .bind(token_id)
            .execute(&state.pool)
            .await?;

        Ok(Self { user, token_id })
    }
}

pub fn user_value(user: &UserRow) -> Value {
    json!({
        "id": user.id,
        "name": user.name,
        "username": user.username,
        "role": user.role,
        "is_active": user.is_active,
        "created_at": user.created_at.map(time::iso),
        "last_login_at": user.last_login_at.map(time::iso),
    })
}

pub async fn issue_token(
    transaction: &mut sqlx::Transaction<'_, sqlx::MySql>,
    user_id: u64,
) -> Result<String, ApiError> {
    let plain = rand::rng()
        .sample_iter(Alphanumeric)
        .take(40)
        .map(char::from)
        .collect::<String>();
    let digest = hex::encode(Sha256::digest(plain.as_bytes()));
    let result = sqlx::query(
        "INSERT INTO personal_access_tokens \
         (tokenable_type, tokenable_id, name, token, abilities, created_at, updated_at) \
         VALUES ('App\\\\Models\\\\User', ?, 'clinic-api', ?, '[\"*\"]', NOW(), NOW())",
    )
    .bind(user_id)
    .bind(digest)
    .execute(&mut **transaction)
    .await?;

    Ok(format!("{}|{plain}", result.last_insert_id()))
}
