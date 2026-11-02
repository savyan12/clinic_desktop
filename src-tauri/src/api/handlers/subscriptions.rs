use crate::api::{
    auth::AuthUser,
    error::{ApiError, ApiResult},
    idempotency, loaders, pagination, resources,
    rows::{DebtRow, PaymentRow, SessionRow, SubscriptionRow, SubscriptionUsageRow},
    services::subscriptions::{self, CorrectionInput, CreateInput},
    validation, AppState,
};
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use chrono::Duration;
use rust_decimal::Decimal;
use serde::Deserialize;
use serde_json::{json, Map, Value};
use std::collections::HashSet;

#[derive(Debug, Deserialize)]
pub struct SubscriptionListQuery {
    search: Option<String>,
    status: Option<String>,
    near_end: Option<u8>,
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
    expire_monthly(&state).await?;
    let page = pagination::PageQuery {
        page: query.page,
        per_page: query.per_page,
    };
    let search = query.search.as_deref().unwrap_or("").trim();
    let pattern = format!("%{search}%");
    let status = query.status.as_deref().unwrap_or("");
    if !status.is_empty() {
        validation::one_of(
            status,
            &["active", "completed", "stopped", "cancelled"],
            "status",
        )?;
    }
    let near_end = query.near_end.unwrap_or(0) != 0;
    let near_date = crate::api::time::today() + Duration::days(state.config.near_end_days);
    let filter = "FROM subscriptions sub \
        WHERE (? = '' OR EXISTS (SELECT 1 FROM patients p WHERE p.id = sub.patient_id AND (p.name LIKE ? OR p.phone LIKE ?))) \
          AND (? = '' OR sub.status = ?) \
          AND (? = 0 OR (sub.status = 'active' AND ( \
               (sub.plan_type = 'legacy_package' AND ((sub.sessions_count - sub.sessions_used) <= ? OR sub.expected_end_date <= ?)) \
               OR (sub.plan_type = 'monthly' AND sub.end_date <= ?) \
          )))";
    let count_sql = format!("SELECT CAST(COUNT(*) AS UNSIGNED) {filter}");
    let select_sql = format!("SELECT sub.* {filter} ORDER BY sub.id DESC LIMIT ? OFFSET ?");
    let total = sqlx::query_scalar::<_, u64>(&count_sql)
        .bind(search)
        .bind(&pattern)
        .bind(&pattern)
        .bind(status)
        .bind(status)
        .bind(near_end)
        .bind(state.config.near_end_sessions)
        .bind(near_date)
        .bind(near_date)
        .fetch_one(&state.pool)
        .await?;
    let rows = sqlx::query_as::<_, SubscriptionRow>(&select_sql)
        .bind(search)
        .bind(&pattern)
        .bind(&pattern)
        .bind(status)
        .bind(status)
        .bind(near_end)
        .bind(state.config.near_end_sessions)
        .bind(near_date)
        .bind(near_date)
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
    expire_monthly(&state).await?;
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
    let payments = sqlx::query_as::<_, PaymentRow>(
        "SELECT * FROM payments WHERE subscription_id = ? ORDER BY id DESC",
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
    object.insert(
        "payments".into(),
        Value::Array(
            payments
                .iter()
                .map(|row| resources::payment_value(row, None))
                .collect(),
        ),
    );
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

pub async fn stop(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<u64>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    auth.require_admin()?;
    expire_monthly(&state).await?;
    let key = idempotency::require_key(&headers)?;
    let data = validation::object(&payload)?;
    let reason = validation::required_string(data, "reason", 1000, "سبب إيقاف الاشتراك مطلوب.")?;
    let actor_id = auth.user.id;
    let route = format!("api/v1/subscriptions/{id}/stop");
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
                let subscription = subscriptions::stop(transaction, id, &reason, actor_id).await?;
                Ok((StatusCode::OK, json!({ "data": subscription })))
            })
        },
    )
    .await?;
    Ok((status, Json(response)))
}

pub async fn reactivate(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<u64>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    auth.require_admin()?;
    let key = idempotency::require_key(&headers)?;
    let data = validation::object(&payload)?;
    let reason = validation::optional_string(data, "reason", 1000)?;
    let actor_id = auth.user.id;
    let route = format!("api/v1/subscriptions/{id}/reactivate");
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
                let subscription =
                    subscriptions::reactivate(transaction, id, reason.as_deref(), actor_id).await?;
                Ok((StatusCode::OK, json!({ "data": subscription })))
            })
        },
    )
    .await?;
    Ok((status, Json(response)))
}

pub async fn cancel_preview(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<u64>,
) -> ApiResult<Json<Value>> {
    auth.require_admin()?;
    expire_monthly(&state).await?;
    let mut transaction = state.pool.begin().await?;
    let (_, cancellation) = subscriptions::cancellation_state(&mut transaction, id).await?;
    transaction.commit().await?;
    Ok(Json(json!({ "data": cancellation.preview })))
}

pub async fn cancel(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<u64>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    auth.require_admin()?;
    expire_monthly(&state).await?;
    let key = idempotency::require_key(&headers)?;
    let data = validation::object(&payload)?;
    let reason = validation::required_string(data, "reason", 1000, "سبب الإلغاء مطلوب.")?;
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
            let reason = reason.clone();
            Box::pin(async move {
                let result = subscriptions::cancel(transaction, id, &reason, actor_id).await?;
                Ok((StatusCode::OK, json!({ "data": {
                    "subscription": result.subscription,
                    "preview": result.preview,
                    "refund": result.refund.as_ref().map(|row| resources::payment_value(row, None)),
                    "voided_debt_ids": result.voided_debt_ids,
                    "replacement_debt": result.replacement_debt.as_ref().map(resources::debt_value),
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
                    json!({ "data": {
                        "subscription": subscription,
                        "impact": { "historical_financial_rows_changed": false },
                    }}),
                ))
            })
        },
    )
    .await?;
    Ok((status, Json(response)))
}

pub async fn expire_monthly(state: &AppState) -> ApiResult<()> {
    sqlx::query(
        "UPDATE subscriptions SET status = 'completed', updated_at = NOW() \
         WHERE plan_type = 'monthly' AND status = 'active' AND end_date < CURDATE()",
    )
    .execute(&state.pool)
    .await?;
    Ok(())
}

fn parse_create(payload: &Value) -> ApiResult<CreateInput> {
    let data = validation::object(payload)?;
    let patient_id = validation::required_u64(data, "patient_id")?;
    let plan_type = data
        .get("plan_type")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| {
            if data.contains_key("service_ids") || data.contains_key("sessions_count") {
                "legacy_package".into()
            } else {
                "monthly".into()
            }
        });
    validation::one_of(&plan_type, &["legacy_package", "monthly"], "plan_type")?;
    let start_date = validation::required_date(data, "start_date")?;
    let paid_amount = validation::optional_decimal(data, "paid_amount", Decimal::ZERO)?;
    let method = validation::optional_string(data, "method", 20)?;
    if let Some(method) = method.as_deref() {
        validation::one_of(method, &["cash", "transfer"], "method")?;
    }
    let sponsor_id = validation::optional_u64(data, "sponsor_id")?;

    let (amount, service_ids, sessions_count, discount, expected_end_date, end_date) =
        if plan_type == "monthly" {
            (
                Some(validation::required_decimal(data, "amount", false)?),
                Vec::new(),
                None,
                Decimal::ZERO,
                None,
                Some(validation::required_date(data, "end_date")?),
            )
        } else {
            (
                None,
                required_id_array(data, "service_ids")?,
                Some(required_u32(data, "sessions_count")?),
                validation::optional_decimal(data, "discount", Decimal::ZERO)?,
                validation::optional_date(data, "expected_end_date")?,
                None,
            )
        };

    Ok(CreateInput {
        patient_id,
        plan_type,
        amount,
        service_ids,
        sessions_count,
        discount,
        paid_amount,
        method,
        start_date,
        expected_end_date,
        end_date,
        sponsor_id,
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
    let allowed = ["expected_end_date", "end_date", "sponsor_id"];
    if changes.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err(ApiError::validation("البيانات المرسلة غير صحيحة.")
            .field("changes", "تحتوي التغييرات على حقل غير مسموح."));
    }
    let expected_end_date_present = changes.contains_key("expected_end_date");
    let expected_end_date = if expected_end_date_present {
        validation::optional_date(changes, "expected_end_date")?
    } else {
        None
    };
    let end_date_present = changes.contains_key("end_date");
    let end_date = if end_date_present {
        Some(validation::required_date(changes, "end_date")?)
    } else {
        None
    };
    let sponsor_id_present = changes.contains_key("sponsor_id");
    let sponsor_id = if sponsor_id_present {
        validation::optional_u64(changes, "sponsor_id")?
    } else {
        None
    };
    Ok((
        CorrectionInput {
            expected_end_date_present,
            expected_end_date,
            end_date_present,
            end_date,
            sponsor_id_present,
            sponsor_id,
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
    let value = data
        .get(field)
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
        })?;
    Ok(value)
}
