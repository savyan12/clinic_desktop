use crate::api::{
    auth::AuthUser,
    error::{ApiError, ApiResult},
    idempotency, loaders, money, pagination, resources,
    rows::{DebtRow, PaymentRow, SessionRow, SubscriptionRow, SubscriptionUsageRow},
    services::subscriptions::{self, CancellationInput, CorrectionInput, CreateInput},
    validation, AppState,
};
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use rust_decimal::Decimal;
use serde::Deserialize;
use serde_json::{json, Map, Value};
use sqlx::Row;
use std::collections::HashSet;

#[derive(Debug, Deserialize)]
pub struct SubscriptionListQuery {
    search: Option<String>,
    status: Option<String>,
    page: Option<u64>,
    per_page: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct UsageQuery {
    page: Option<u64>,
    per_page: Option<u64>,
}

pub async fn index(
    State(state): State<AppState>,
    _auth: AuthUser,
    Query(query): Query<SubscriptionListQuery>,
) -> ApiResult<Json<Value>> {
    let page = pagination::PageQuery {
        page: query.page,
        per_page: query.per_page,
    };
    let search = query.search.as_deref().unwrap_or("").trim();
    let pattern = format!("%{search}%");
    let status = query.status.as_deref().unwrap_or("");
    if !status.is_empty() {
        validation::one_of(status, &["active", "completed", "cancelled"], "status")?;
    }
    let filter = "FROM subscriptions sub \
        WHERE (? = '' OR sub.name LIKE ? OR EXISTS (\
            SELECT 1 FROM patients p WHERE p.id = sub.patient_id AND (p.name LIKE ? OR p.phone LIKE ?)\
        )) AND (? = '' OR sub.status = ?)";
    let count_sql = format!("SELECT CAST(COUNT(*) AS UNSIGNED) {filter}");
    let select_sql = format!("SELECT sub.* {filter} ORDER BY sub.id DESC LIMIT ? OFFSET ?");
    let total = sqlx::query_scalar::<_, u64>(&count_sql)
        .bind(search)
        .bind(&pattern)
        .bind(&pattern)
        .bind(&pattern)
        .bind(status)
        .bind(status)
        .fetch_one(&state.pool)
        .await?;
    let rows = sqlx::query_as::<_, SubscriptionRow>(&select_sql)
        .bind(search)
        .bind(&pattern)
        .bind(&pattern)
        .bind(&pattern)
        .bind(status)
        .bind(status)
        .bind(page.per_page())
        .bind(page.offset())
        .fetch_all(&state.pool)
        .await?;
    let mut values = Vec::with_capacity(rows.len());
    for row in &rows {
        values.push(loaders::subscription_from_row(&state.pool, row).await?);
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
    let input = parse_create(&payload)?;
    let actor_id = auth.user.id;
    let (status, response) = idempotency::execute(
        &state.pool,
        &auth,
        &key,
        "POST",
        "api/v1/subscriptions",
        &payload,
        move |transaction| {
            let input = input.clone();
            Box::pin(async move {
                let result = subscriptions::create(transaction, &input, actor_id).await?;
                Ok((StatusCode::CREATED, json!({ "data": {
                    "subscription": result.subscription,
                    "payment": result.payment.as_ref().map(|row| resources::payment_value(row, None)),
                    "debt": result.debt.as_ref().map(resources::debt_value),
                }})))
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
    let mut value = loaders::subscription(&state.pool, id).await?;
    let usages = sqlx::query_as::<_, SubscriptionUsageRow>(
        "SELECT * FROM subscription_usages WHERE subscription_id = ? ORDER BY id DESC",
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await?;
    let sessions = sqlx::query_as::<_, SessionRow>(
        "SELECT * FROM sessions WHERE subscription_id = ? ORDER BY opened_at DESC, id DESC",
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await?;
    let mut session_values = Vec::with_capacity(sessions.len());
    for session in &sessions {
        session_values.push(loaders::session_from_row(&state.pool, session).await?);
    }
    let direct_payments = sqlx::query_as::<_, PaymentRow>(
        "SELECT * FROM payments WHERE subscription_id = ? ORDER BY id DESC",
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await?;
    let allocated_payments = sqlx::query(
        "SELECT p.id, p.type, p.method, p.created_at, p.note, SUM(pa.amount) AS amount \
         FROM payments p \
         INNER JOIN payment_allocations pa ON pa.payment_id = p.id \
         INNER JOIN debts d ON d.id = pa.debt_id \
         WHERE d.subscription_id = ? AND p.type = 'debt_payment' \
         GROUP BY p.id, p.type, p.method, p.created_at, p.note ORDER BY p.id DESC",
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await?;
    let debts = sqlx::query_as::<_, DebtRow>(
        "SELECT * FROM debts WHERE subscription_id = ? ORDER BY id DESC",
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await?;
    let mut payment_values = direct_payments
        .iter()
        .map(|row| resources::payment_value(row, None))
        .collect::<Vec<_>>();
    for row in allocated_payments {
        payment_values.push(json!({
            "id": row.try_get::<u64, _>("id")?,
            "type": row.try_get::<String, _>("type")?,
            "method": row.try_get::<String, _>("method")?,
            "amount": money::normalize(row.try_get::<Decimal, _>("amount")?),
            "note": row.try_get::<Option<String>, _>("note")?,
            "created_at": row.try_get::<Option<chrono::DateTime<chrono::Utc>>, _>("created_at")?.map(crate::api::time::iso),
            "source": "debt_allocation",
            "subscription_id": id,
        }));
    }
    payment_values.sort_by_key(|payment| std::cmp::Reverse(payment["id"].as_u64().unwrap_or(0)));

    let object = value
        .as_object_mut()
        .expect("subscription resource must be an object");
    object.insert(
        "usages".into(),
        Value::Array(
            usages
                .iter()
                .map(resources::subscription_usage_value)
                .collect(),
        ),
    );
    object.insert("sessions".into(), Value::Array(session_values));
    object.insert("payments".into(), Value::Array(payment_values));
    object.insert(
        "debts".into(),
        Value::Array(debts.iter().map(resources::debt_value).collect()),
    );
    Ok(Json(json!({ "data": value })))
}

pub async fn usage(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<u64>,
    Query(query): Query<UsageQuery>,
) -> ApiResult<Json<Value>> {
    sqlx::query("SELECT id FROM subscriptions WHERE id = ?")
        .bind(id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(ApiError::not_found)?;
    let page = pagination::PageQuery {
        page: query.page,
        per_page: query.per_page,
    };
    let total = sqlx::query_scalar::<_, u64>(
        "SELECT CAST(COUNT(*) AS UNSIGNED) FROM subscription_usages WHERE subscription_id = ?",
    )
    .bind(id)
    .fetch_one(&state.pool)
    .await?;
    let rows = sqlx::query_as::<_, SubscriptionUsageRow>(
        "SELECT * FROM subscription_usages WHERE subscription_id = ? ORDER BY id DESC LIMIT ? OFFSET ?",
    )
    .bind(id)
    .bind(page.per_page())
    .bind(page.offset())
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(json!({
        "data": rows.iter().map(resources::subscription_usage_value).collect::<Vec<_>>(),
        "meta": pagination::meta(page.page(), page.per_page(), total),
    })))
}

pub async fn cancel_preview(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<u64>,
) -> ApiResult<Json<Value>> {
    auth.require_admin()?;
    let mut transaction = state.pool.begin().await?;
    let preview = subscriptions::cancellation_preview(&mut transaction, id).await?;
    transaction.commit().await?;
    Ok(Json(json!({ "data": preview })))
}

pub async fn cancel(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<u64>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    auth.require_admin()?;
    let key = idempotency::require_key(&headers)?;
    let input = parse_cancellation(&payload)?;
    let actor_id = auth.user.id;
    let route = format!("api/v1/subscriptions/{id}/cancel");
    let (status, response) = idempotency::execute(
        &state.pool,
        &auth,
        &key,
        "POST",
        &route,
        &payload,
        move |transaction| {
            let input = input.clone();
            Box::pin(async move {
                let result = subscriptions::cancel(transaction, id, &input, actor_id).await?;
                Ok((StatusCode::OK, json!({ "data": {
                    "subscription": result.subscription,
                    "refund": result.refund.as_ref().map(|row| resources::payment_value(row, None)),
                    "voided_debt_ids": result.voided_debt_ids,
                }})))
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
    let (input, reason) = parse_correction(&payload)?;
    let actor_id = auth.user.id;
    let route = format!("api/v1/subscriptions/{id}/correct");
    let (status, response) = idempotency::execute(
        &state.pool,
        &auth,
        &key,
        "POST",
        &route,
        &payload,
        move |transaction| {
            let input = input.clone();
            let reason = reason.clone();
            Box::pin(async move {
                let subscription =
                    subscriptions::correct(transaction, id, &input, &reason, actor_id).await?;
                Ok((
                    StatusCode::OK,
                    json!({ "data": { "subscription": subscription } }),
                ))
            })
        },
    )
    .await?;
    Ok((status, Json(response)))
}

fn parse_create(payload: &Value) -> ApiResult<CreateInput> {
    let data = validation::object(payload)?;
    let patient_id = validation::required_u64(data, "patient_id")?;
    let name = validation::required_string(data, "name", 255, "اسم الاشتراك مطلوب.")?;
    let service_ids = required_id_array(data, "service_ids")?;
    let sessions_count = required_u32(data, "sessions_count")?;
    let total_price = validation::required_decimal(data, "total_price", true)?;
    let paid_amount = validation::optional_decimal(data, "paid_amount", Decimal::ZERO)?;
    let method = validation::optional_string(data, "method", 20)?;
    if let Some(method) = method.as_deref() {
        validation::one_of(method, &["cash", "transfer"], "method")?;
    }
    Ok(CreateInput {
        patient_id,
        name,
        service_ids,
        sessions_count,
        total_price,
        paid_amount,
        method,
    })
}

fn parse_cancellation(payload: &Value) -> ApiResult<CancellationInput> {
    let data = validation::object(payload)?;
    let reason = validation::required_string(data, "reason", 1000, "سبب الإلغاء مطلوب.")?;
    let refund_amount = validation::optional_decimal(data, "refund_amount", Decimal::ZERO)?;
    let refund_method = validation::optional_string(data, "refund_method", 20)?;
    if let Some(method) = refund_method.as_deref() {
        validation::one_of(method, &["cash", "transfer"], "refund_method")?;
    }
    Ok(CancellationInput {
        reason,
        refund_amount,
        refund_method,
    })
}

fn parse_correction(payload: &Value) -> ApiResult<(CorrectionInput, String)> {
    let data = validation::object(payload)?;
    let reason = validation::required_string(data, "reason", 1000, "سبب التصحيح مطلوب.")?;
    let changes = data
        .get("changes")
        .and_then(Value::as_object)
        .filter(|changes| !changes.is_empty())
        .ok_or_else(|| {
            ApiError::validation("أرسل تغييرًا واحدًا على الأقل.")
                .field("changes", "أرسل تغييرًا واحدًا على الأقل.")
        })?;
    let allowed = ["sessions_count", "total_price"];
    if changes.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err(ApiError::validation("البيانات المرسلة غير صحيحة.")
            .field("changes", "تحتوي التغييرات على حقل غير مسموح."));
    }
    let sessions_count = if changes.contains_key("sessions_count") {
        Some(required_u32(changes, "sessions_count")?)
    } else {
        None
    };
    let total_price = if changes.contains_key("total_price") {
        Some(validation::required_decimal(changes, "total_price", true)?)
    } else {
        None
    };
    Ok((
        CorrectionInput {
            sessions_count,
            total_price,
        },
        reason,
    ))
}

fn required_id_array(data: &Map<String, Value>, field: &str) -> ApiResult<Vec<u64>> {
    let values = data
        .get(field)
        .and_then(Value::as_array)
        .filter(|values| !values.is_empty())
        .ok_or_else(|| {
            ApiError::validation("اختر خدمة واحدة على الأقل.")
                .field(field, "اختر خدمة واحدة على الأقل.")
        })?;
    let mut result = Vec::with_capacity(values.len());
    let mut unique = HashSet::new();
    for (index, value) in values.iter().enumerate() {
        let id = value
            .as_u64()
            .or_else(|| value.as_str().and_then(|value| value.parse().ok()))
            .ok_or_else(|| {
                ApiError::validation("البيانات المرسلة غير صحيحة.")
                    .field(format!("{field}.{index}"), "معرف الخدمة غير صحيح.")
            })?;
        if !unique.insert(id) {
            return Err(ApiError::validation("البيانات المرسلة غير صحيحة.")
                .field(format!("{field}.{index}"), "لا يجوز تكرار الخدمة نفسها."));
        }
        result.push(id);
    }
    Ok(result)
}

fn required_u32(data: &Map<String, Value>, field: &str) -> ApiResult<u32> {
    data.get(field)
        .and_then(|value| {
            value
                .as_u64()
                .or_else(|| value.as_str().and_then(|value| value.parse().ok()))
        })
        .and_then(|value| u32::try_from(value).ok())
        .filter(|value| *value > 0)
        .ok_or_else(|| {
            ApiError::validation("البيانات المرسلة غير صحيحة.")
                .field(field, "أدخل رقمًا صحيحًا أكبر من صفر.")
        })
}
