use crate::api::{
    audit,
    auth::{user_value, AuthUser},
    error::{ApiError, ApiResult},
    pagination,
    rows::UserRow,
    validation, AppState,
};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use bcrypt::{hash, DEFAULT_COST};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Debug, Deserialize)]
pub struct UserListQuery {
    search: Option<String>,
    is_active: Option<bool>,
    page: Option<u64>,
    per_page: Option<u64>,
}

pub async fn index(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(query): Query<UserListQuery>,
) -> ApiResult<Json<Value>> {
    auth.require_admin()?;
    let page = pagination::PageQuery {
        page: query.page,
        per_page: query.per_page,
    };
    let search = query.search.as_deref().unwrap_or("").trim();
    let search_pattern = format!("%{search}%");

    let total = sqlx::query_scalar::<_, u64>(
        "SELECT CAST(COUNT(*) AS UNSIGNED) FROM users \
         WHERE role = 'receptionist' \
           AND (? = '' OR name LIKE ? OR username LIKE ?) \
           AND (? IS NULL OR is_active = ?)",
    )
    .bind(search)
    .bind(&search_pattern)
    .bind(&search_pattern)
    .bind(query.is_active)
    .bind(query.is_active)
    .fetch_one(&state.pool)
    .await?;
    let users = sqlx::query_as::<_, UserRow>(
        "SELECT id, name, username, password, role, is_active, last_login_at, created_at, updated_at \
         FROM users \
         WHERE role = 'receptionist' \
           AND (? = '' OR name LIKE ? OR username LIKE ?) \
           AND (? IS NULL OR is_active = ?) \
         ORDER BY id DESC LIMIT ? OFFSET ?",
    )
    .bind(search)
    .bind(&search_pattern)
    .bind(&search_pattern)
    .bind(query.is_active)
    .bind(query.is_active)
    .bind(page.per_page())
    .bind(page.offset())
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(json!({
        "data": users.iter().map(user_value).collect::<Vec<_>>(),
        "meta": pagination::meta(page.page(), page.per_page(), total),
    })))
}

pub async fn store(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(payload): Json<Value>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    auth.require_admin()?;
    let data = validation::object(&payload)?;
    let name = validation::required_string(data, "name", 255, "اسم الموظف مطلوب.")?;
    let username = validation::required_string(data, "username", 100, "اسم المستخدم مطلوب.")?;
    validate_username(&username)?;
    let password = validation::required_string(data, "password", 255, "كلمة المرور مطلوبة.")?;
    validate_password_confirmation(data, &password)?;
    let is_active = validation::boolean(data, "is_active", true)?;

    if username_exists(&state, &username, None).await? {
        return Err(ApiError::validation("اسم المستخدم مستخدم مسبقًا.")
            .field("username", "اسم المستخدم مستخدم مسبقًا."));
    }

    let hashed = hash(password, DEFAULT_COST).map_err(ApiError::internal)?;
    let mut transaction = state.pool.begin().await?;
    let result = sqlx::query(
        "INSERT INTO users (name, username, password, role, is_active, created_at, updated_at) \
         VALUES (?, ?, ?, 'receptionist', ?, NOW(), NOW())",
    )
    .bind(&name)
    .bind(&username)
    .bind(hashed)
    .bind(is_active)
    .execute(&mut *transaction)
    .await?;
    let id = result.last_insert_id();
    let new_values = json!({
        "name": name,
        "username": username,
        "role": "receptionist",
        "is_active": is_active,
    });
    audit::log(
        &mut transaction,
        auth.user.id,
        "user_created",
        "user",
        id,
        None,
        Some(&new_values),
        None,
    )
    .await?;
    transaction.commit().await?;
    let user = fetch_receptionist(&state, id).await?;

    Ok((
        StatusCode::CREATED,
        Json(json!({ "data": user_value(&user) })),
    ))
}

pub async fn update(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<u64>,
    Json(payload): Json<Value>,
) -> ApiResult<Json<Value>> {
    auth.require_admin()?;
    let current = fetch_receptionist(&state, id).await?;
    let data = validation::object(&payload)?;
    let name = validation::required_string(data, "name", 255, "اسم الموظف مطلوب.")?;
    let username = validation::required_string(data, "username", 100, "اسم المستخدم مطلوب.")?;
    validate_username(&username)?;
    let is_active = validation::boolean(data, "is_active", current.is_active)?;
    if username_exists(&state, &username, Some(id)).await? {
        return Err(ApiError::validation("اسم المستخدم مستخدم مسبقًا.")
            .field("username", "اسم المستخدم مستخدم مسبقًا."));
    }

    let old_values = json!({
        "name": current.name,
        "username": current.username,
        "is_active": current.is_active,
    });
    let new_values = json!({
        "name": name,
        "username": username,
        "is_active": is_active,
    });
    let mut transaction = state.pool.begin().await?;
    sqlx::query(
        "UPDATE users SET name = ?, username = ?, is_active = ?, updated_at = NOW() WHERE id = ?",
    )
    .bind(&name)
    .bind(&username)
    .bind(is_active)
    .bind(id)
    .execute(&mut *transaction)
    .await?;
    if !is_active {
        sqlx::query("DELETE FROM personal_access_tokens WHERE tokenable_id = ? AND tokenable_type = 'App\\\\Models\\\\User'")
            .bind(id)
            .execute(&mut *transaction)
            .await?;
    }
    audit::log(
        &mut transaction,
        auth.user.id,
        "user_updated",
        "user",
        id,
        Some(&old_values),
        Some(&new_values),
        None,
    )
    .await?;
    transaction.commit().await?;

    Ok(Json(
        json!({ "data": user_value(&fetch_receptionist(&state, id).await?) }),
    ))
}

pub async fn reset_password(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<u64>,
    Json(payload): Json<Value>,
) -> ApiResult<Json<Value>> {
    auth.require_admin()?;
    fetch_receptionist(&state, id).await?;
    let data = validation::object(&payload)?;
    let password = validation::required_string(data, "password", 255, "كلمة المرور مطلوبة.")?;
    validate_password_confirmation(data, &password)?;
    let hashed = hash(password, DEFAULT_COST).map_err(ApiError::internal)?;

    let mut transaction = state.pool.begin().await?;
    sqlx::query("UPDATE users SET password = ?, updated_at = NOW() WHERE id = ?")
        .bind(hashed)
        .bind(id)
        .execute(&mut *transaction)
        .await?;
    sqlx::query("DELETE FROM personal_access_tokens WHERE tokenable_id = ? AND tokenable_type = 'App\\\\Models\\\\User'")
        .bind(id)
        .execute(&mut *transaction)
        .await?;
    let values = json!({ "tokens_revoked": true });
    audit::log(
        &mut transaction,
        auth.user.id,
        "user_password_reset",
        "user",
        id,
        None,
        Some(&values),
        None,
    )
    .await?;
    transaction.commit().await?;

    Ok(Json(json!({ "data": {
        "message": "تمت إعادة تعيين كلمة المرور بنجاح."
    }})))
}

async fn fetch_receptionist(state: &AppState, id: u64) -> ApiResult<UserRow> {
    sqlx::query_as::<_, UserRow>(
        "SELECT id, name, username, password, role, is_active, last_login_at, created_at, updated_at \
         FROM users WHERE id = ? AND role = 'receptionist' LIMIT 1",
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(ApiError::not_found)
}

async fn username_exists(
    state: &AppState,
    username: &str,
    except_id: Option<u64>,
) -> ApiResult<bool> {
    let count = sqlx::query_scalar::<_, u64>(
        "SELECT CAST(COUNT(*) AS UNSIGNED) FROM users WHERE username = ? AND (? IS NULL OR id != ?)",
    )
    .bind(username)
    .bind(except_id)
    .bind(except_id)
    .fetch_one(&state.pool)
    .await?;
    Ok(count > 0)
}

fn validate_username(username: &str) -> ApiResult<()> {
    if username
        .chars()
        .all(|character| character.is_alphanumeric() || matches!(character, '-' | '_'))
    {
        Ok(())
    } else {
        Err(ApiError::validation("اسم المستخدم غير صالح.").field(
            "username",
            "يُسمح بالحروف والأرقام والشرطة والشرطة السفلية فقط.",
        ))
    }
}

fn validate_password_confirmation(
    data: &serde_json::Map<String, Value>,
    password: &str,
) -> ApiResult<()> {
    if password.chars().count() < 8 {
        return Err(ApiError::validation("كلمة المرور قصيرة.")
            .field("password", "يجب ألا تقل كلمة المرور عن 8 أحرف."));
    }
    if data.get("password_confirmation").and_then(Value::as_str) != Some(password) {
        return Err(ApiError::validation("تأكيد كلمة المرور غير مطابق.")
            .field("password", "تأكيد كلمة المرور غير مطابق."));
    }
    Ok(())
}
