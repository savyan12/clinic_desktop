use crate::api::{
    auth::{issue_token, user_value, AuthUser},
    error::{ApiError, ApiResult},
    rows::UserRow,
    AppState,
};
use axum::{extract::State, http::StatusCode, Json};
use bcrypt::verify;
use chrono::{SecondsFormat, Utc};
use serde_json::{json, Value};

pub async fn health() -> Json<Value> {
    Json(json!({
        "status": "ok",
        "time": Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
    }))
}

pub async fn login(
    State(state): State<AppState>,
    Json(payload): Json<Value>,
) -> ApiResult<Json<Value>> {
    let data = payload.as_object().ok_or_else(|| {
        ApiError::validation("البيانات المرسلة غير صحيحة.").field("username", "اسم المستخدم مطلوب.")
    })?;
    let username = data
        .get("username")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            ApiError::validation("اسم المستخدم مطلوب.").field("username", "اسم المستخدم مطلوب.")
        })?;
    let password = data
        .get("password")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            ApiError::validation("كلمة المرور مطلوبة.").field("password", "كلمة المرور مطلوبة.")
        })?;
    state.login_rate_limiter.record(username)?;

    let user = sqlx::query_as::<_, UserRow>(
        "SELECT id, name, username, password, role, is_active, last_login_at, created_at, updated_at \
         FROM users WHERE username = ? LIMIT 1",
    )
    .bind(username)
    .fetch_optional(&state.pool)
    .await?;

    let Some(mut user) = user else {
        return Err(invalid_credentials());
    };
    if !verify(password, &user.password).unwrap_or(false) {
        return Err(invalid_credentials());
    }
    if !user.is_active {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "هذا الحساب معطل. راجع مدير النظام.",
        ));
    }

    let mut transaction = state.pool.begin().await?;
    sqlx::query("UPDATE users SET last_login_at = NOW(), updated_at = NOW() WHERE id = ?")
        .bind(user.id)
        .execute(&mut *transaction)
        .await?;
    let token = issue_token(&mut transaction, user.id).await?;
    transaction.commit().await?;
    user = sqlx::query_as::<_, UserRow>(
        "SELECT id, name, username, password, role, is_active, last_login_at, created_at, updated_at \
         FROM users WHERE id = ?",
    )
    .bind(user.id)
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(json!({ "data": {
        "token": token,
        "user": user_value(&user),
    }})))
}

pub async fn logout(State(state): State<AppState>, auth: AuthUser) -> ApiResult<Json<Value>> {
    sqlx::query("DELETE FROM personal_access_tokens WHERE id = ?")
        .bind(auth.token_id)
        .execute(&state.pool)
        .await?;

    Ok(Json(json!({ "data": {
        "message": "تم تسجيل الخروج بنجاح."
    }})))
}

pub async fn me(auth: AuthUser) -> Json<Value> {
    Json(json!({ "data": user_value(&auth.user) }))
}

fn invalid_credentials() -> ApiError {
    ApiError::validation("اسم المستخدم أو كلمة المرور غير صحيحة.")
        .field("username", "تعذر تسجيل الدخول بهذه البيانات.")
}
