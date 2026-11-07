use crate::api::{
    auth::AuthUser,
    error::{ApiError, ApiResult},
    idempotency, loaders, pagination,
    rows::SessionRow,
    services::sessions,
    time, validation, AppState,
};
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use chrono::NaiveDate;
use serde::Deserialize;
use serde_json::{json, Map, Value};

#[derive(Debug, Deserialize)]
pub struct SessionListQuery {
    search: Option<String>,
    #[serde(default, deserialize_with = "crate::api::query::optional")]
    doctor_id: Option<u64>,
    status: Option<String>,
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
    Query(query): Query<SessionListQuery>,
) -> ApiResult<Json<Value>> {
    let page = pagination::PageQuery {
        page: query.page,
        per_page: query.per_page,
    };
    let receptionist_today = auth.user.role != "admin";
    let search = query.search.as_deref().unwrap_or("").trim();
    let pattern = format!("%{search}%");
    let status = query.status.as_deref().unwrap_or("");
    let date = (!receptionist_today).then_some(query.date).flatten();
    let from = (!receptionist_today).then_some(query.from).flatten();
    let to = (!receptionist_today).then_some(query.to).flatten();
    let filter = "FROM sessions s INNER JOIN patients p ON p.id = s.patient_id \
        WHERE (? = 0 OR DATE(s.opened_at) = CURDATE()) \
          AND (? = '' OR p.name LIKE ? OR p.phone LIKE ? OR s.code LIKE ?) \
          AND (? IS NULL OR s.doctor_id = ?) \
          AND (? = '' OR s.status = ?) \
          AND (? IS NULL OR DATE(s.opened_at) = ?) \
          AND (? IS NULL OR DATE(s.opened_at) >= ?) \
          AND (? IS NULL OR DATE(s.opened_at) <= ?)";
    let count_sql = format!("SELECT CAST(COUNT(*) AS UNSIGNED) {filter}");
    let select_sql =
        format!("SELECT s.* {filter} ORDER BY s.opened_at DESC, s.id DESC LIMIT ? OFFSET ?");
    let total = sqlx::query_scalar::<_, u64>(&count_sql)
        .bind(receptionist_today)
        .bind(search)
        .bind(&pattern)
        .bind(&pattern)
        .bind(&pattern)
        .bind(query.doctor_id)
        .bind(query.doctor_id)
        .bind(status)
        .bind(status)
        .bind(date)
        .bind(date)
        .bind(from)
        .bind(from)
        .bind(to)
        .bind(to)
        .fetch_one(&state.pool)
        .await?;
    let rows = sqlx::query_as::<_, SessionRow>(&select_sql)
        .bind(receptionist_today)
        .bind(search)
        .bind(&pattern)
        .bind(&pattern)
        .bind(&pattern)
        .bind(query.doctor_id)
        .bind(query.doctor_id)
        .bind(status)
        .bind(status)
        .bind(date)
        .bind(date)
        .bind(from)
        .bind(from)
        .bind(to)
        .bind(to)
        .bind(page.per_page())
        .bind(page.offset())
        .fetch_all(&state.pool)
        .await?;
    let mut values = Vec::with_capacity(rows.len());
    for row in &rows {
        values.push(loaders::session_from_row(&state.pool, row).await?);
    }
    Ok(Json(json!({
        "data": values,
        "meta": pagination::meta(page.page(), page.per_page(), total),
    })))
}

pub async fn store(
    State(state): State<AppState>,
    auth: AuthUser,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let key = idempotency::require_key(&headers)?;
    let actor_id = auth.user.id;
    let config = state.config.clone();
    let operation_payload = payload.clone();
    let (status, response) = idempotency::execute(
        &state.pool,
        &auth,
        &key,
        "POST",
        "api/v1/sessions",
        &payload,
        move |transaction| {
            let operation_payload = operation_payload.clone();
            let config = config.clone();
            Box::pin(async move {
                let result =
                    sessions::create_and_close(transaction, &operation_payload, actor_id, &config)
                        .await?;
                Ok((
                    StatusCode::CREATED,
                    json!({ "data": {
                        "session": result.session,
                        "effects": result.effects.value(),
                        "replayed": result.replayed,
                    }}),
                ))
            })
        },
    )
    .await?;
    Ok((status, Json(response)))
}

pub async fn show(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<u64>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        json!({ "data": loaders::session(&state.pool, id, true).await? }),
    ))
}

pub async fn update(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<u64>,
    Json(payload): Json<Value>,
) -> ApiResult<Json<Value>> {
    let mut transaction = state.pool.begin().await?;
    let session = sessions::update(&mut transaction, id, &payload, auth.user.id).await?;
    transaction.commit().await?;
    Ok(Json(json!({ "data": session })))
}

pub async fn close(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<u64>,
    headers: HeaderMap,
    payload: Option<Json<Value>>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let payload = payload
        .map(|Json(value)| value)
        .unwrap_or_else(|| json!({}));
    let key = idempotency::require_key(&headers)?;
    let actor_id = auth.user.id;
    let config = state.config.clone();
    let route = format!("api/v1/sessions/{id}/close");
    let (status, response) = idempotency::execute(
        &state.pool,
        &auth,
        &key,
        "POST",
        &route,
        &payload,
        move |transaction| {
            let config = config.clone();
            Box::pin(async move {
                let result = sessions::close(transaction, id, actor_id, &config).await?;
                Ok((
                    StatusCode::OK,
                    json!({ "data": {
                        "session": result.session,
                        "effects": result.effects.value(),
                        "replayed": result.replayed,
                    }}),
                ))
            })
        },
    )
    .await?;
    Ok((status, Json(response)))
}

pub async fn add_services(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<u64>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let key = idempotency::require_key(&headers)?;
    let actor_id = auth.user.id;
    let operation_payload = payload.clone();
    let route = format!("api/v1/sessions/{id}/services");
    let (status, response) = idempotency::execute(
        &state.pool,
        &auth,
        &key,
        "POST",
        &route,
        &payload,
        move |transaction| {
            let operation_payload = operation_payload.clone();
            Box::pin(async move {
                let result =
                    sessions::add_services(transaction, id, &operation_payload, actor_id).await?;
                Ok((
                    StatusCode::CREATED,
                    json!({ "data": {
                        "session": result.session,
                        "addition": {
                            "id": result.addition.id,
                            "services_total": crate::api::money::normalize(result.addition.services_total),
                            "net_total": crate::api::money::normalize(result.addition.net_total),
                            "created_at": result.addition.created_at.map(time::iso),
                        },
                        "effects": result.effects.value(),
                    }}),
                ))
            })
        },
    )
    .await?;
    Ok((status, Json(response)))
}

pub async fn cancel(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<u64>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let key = idempotency::require_key(&headers)?;
    let data = validation::object(&payload)?;
    let reason = validation::required_string(data, "reason", 1000, "سبب إلغاء الجلسة مطلوب.")?;
    let actor_id = auth.user.id;
    let route = format!("api/v1/sessions/{id}/cancel");
    let (status, response) = idempotency::execute(
        &state.pool,
        &auth,
        &key,
        "POST",
        &route,
        &payload,
        move |transaction| {
            let reason = reason.clone();
            Box::pin(async move {
                let result = sessions::cancel(transaction, id, &reason, actor_id).await?;
                Ok((
                    StatusCode::OK,
                    json!({ "data": {
                        "session": result.session,
                        "reversals": result.reversals,
                    }}),
                ))
            })
        },
    )
    .await?;
    Ok((status, Json(response)))
}

pub async fn correct(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<u64>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    auth.require_admin()?;
    let key = idempotency::require_key(&headers)?;
    let (changes, reason) = correction_input(&payload)?;
    let actor_id = auth.user.id;
    let config = state.config.clone();
    let route = format!("api/v1/sessions/{id}/correct");
    let (status, response) = idempotency::execute(
        &state.pool,
        &auth,
        &key,
        "POST",
        &route,
        &payload,
        move |transaction| {
            let changes = changes.clone();
            let reason = reason.clone();
            let config = config.clone();
            Box::pin(async move {
                let result =
                    sessions::correct(transaction, id, &changes, &reason, actor_id, &config)
                        .await?;
                Ok((
                    StatusCode::OK,
                    json!({ "data": {
                        "session": result.session,
                        "financial_impact": result.financial_impact,
                    }}),
                ))
            })
        },
    )
    .await?;
    Ok((status, Json(response)))
}

fn correction_input(payload: &Value) -> ApiResult<(Map<String, Value>, String)> {
    let data = validation::object(payload)?;
    let reason = validation::required_string(data, "reason", 1000, "سبب التصحيح مطلوب.")?;
    let allowed = [
        "patient_id",
        "doctor_id",
        "service_ids",
        "discount",
        "settlement",
    ];
    let mut changes = Map::new();
    if let Some(value) = data.get("changes") {
        let nested = value.as_object().ok_or_else(|| {
            ApiError::validation("البيانات المرسلة غير صحيحة.")
                .field("changes", "يجب إرسال التغييرات في كائن صالح.")
        })?;
        if nested.keys().any(|key| !allowed.contains(&key.as_str())) {
            return Err(ApiError::validation("البيانات المرسلة غير صحيحة.")
                .field("changes", "تحتوي التغييرات على حقل غير مسموح."));
        }
        changes.extend(nested.clone());
    }
    for field in allowed {
        if let Some(value) = data.get(field) {
            changes.insert(field.into(), value.clone());
        }
    }
    if changes.is_empty() {
        return Err(ApiError::validation("أرسل تغييرًا واحدًا على الأقل.")
            .field("changes", "أرسل تغييرًا واحدًا على الأقل."));
    }
    Ok((changes, reason))
}
