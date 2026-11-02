use crate::api::{
    auth::AuthUser, error::ApiResult, pagination, resources, rows::AuditLogRow, AppState,
};
use axum::{
    extract::{Query, State},
    Json,
};
use chrono::NaiveDate;
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::Row;

#[derive(Debug, Deserialize)]
pub struct AuditLogQuery {
    #[serde(default, deserialize_with = "crate::api::query::optional")]
    user_id: Option<u64>,
    #[serde(default, deserialize_with = "crate::api::query::optional")]
    entity_id: Option<u64>,
    action: Option<String>,
    entity_type: Option<String>,
    #[serde(default, deserialize_with = "crate::api::query::optional")]
    date: Option<NaiveDate>,
    #[serde(default, deserialize_with = "crate::api::query::optional")]
    from: Option<NaiveDate>,
    #[serde(default, deserialize_with = "crate::api::query::optional")]
    to: Option<NaiveDate>,
    #[serde(default, deserialize_with = "crate::api::query::optional")]
    page: Option<u64>,
    #[serde(default, deserialize_with = "crate::api::query::optional")]
    per_page: Option<u64>,
}

pub async fn index(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(query): Query<AuditLogQuery>,
) -> ApiResult<Json<Value>> {
    auth.require_admin()?;
    let page = pagination::PageQuery {
        page: query.page,
        per_page: query.per_page,
    };
    let action = query.action.as_deref().unwrap_or("").trim();
    let entity_type = query.entity_type.as_deref().unwrap_or("").trim();
    let total = sqlx::query_scalar::<_, u64>(
        "SELECT CAST(COUNT(*) AS UNSIGNED) FROM audit_logs \
         WHERE (? IS NULL OR user_id = ?) \
           AND (? IS NULL OR entity_id = ?) \
           AND (? = '' OR action = ?) \
           AND (? = '' OR entity_type = ?) \
           AND (? IS NULL OR DATE(created_at) = ?) \
           AND (? IS NULL OR DATE(created_at) >= ?) \
           AND (? IS NULL OR DATE(created_at) <= ?)",
    )
    .bind(query.user_id)
    .bind(query.user_id)
    .bind(query.entity_id)
    .bind(query.entity_id)
    .bind(action)
    .bind(action)
    .bind(entity_type)
    .bind(entity_type)
    .bind(query.date)
    .bind(query.date)
    .bind(query.from)
    .bind(query.from)
    .bind(query.to)
    .bind(query.to)
    .fetch_one(&state.pool)
    .await?;
    let rows = sqlx::query_as::<_, AuditLogRow>(
        "SELECT * FROM audit_logs \
         WHERE (? IS NULL OR user_id = ?) \
           AND (? IS NULL OR entity_id = ?) \
           AND (? = '' OR action = ?) \
           AND (? = '' OR entity_type = ?) \
           AND (? IS NULL OR DATE(created_at) = ?) \
           AND (? IS NULL OR DATE(created_at) >= ?) \
           AND (? IS NULL OR DATE(created_at) <= ?) \
         ORDER BY id DESC LIMIT ? OFFSET ?",
    )
    .bind(query.user_id)
    .bind(query.user_id)
    .bind(query.entity_id)
    .bind(query.entity_id)
    .bind(action)
    .bind(action)
    .bind(entity_type)
    .bind(entity_type)
    .bind(query.date)
    .bind(query.date)
    .bind(query.from)
    .bind(query.from)
    .bind(query.to)
    .bind(query.to)
    .bind(page.per_page())
    .bind(page.offset())
    .fetch_all(&state.pool)
    .await?;

    let mut data = Vec::with_capacity(rows.len());
    for row in &rows {
        let user = sqlx::query("SELECT id, name FROM users WHERE id = ?")
            .bind(row.user_id)
            .fetch_one(&state.pool)
            .await?;
        data.push(resources::audit_value(
            row,
            Some(json!({
                "id": user.try_get::<u64, _>("id")?,
                "name": user.try_get::<String, _>("name")?,
            })),
        ));
    }

    Ok(Json(json!({
        "data": data,
        "meta": pagination::meta(page.page(), page.per_page(), total),
    })))
}
