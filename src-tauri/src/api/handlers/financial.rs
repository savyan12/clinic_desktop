use crate::api::{
    auth::AuthUser,
    error::{ApiError, ApiResult},
    idempotency, loaders, money, pagination, resources,
    rows::{DebtRow, PaymentRow, WalletTransactionRow},
    services::ledger,
    validation, AppState,
};
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashSet;

#[derive(Debug, Deserialize)]
pub struct LedgerQuery {
    patient_id: Option<u64>,
    sponsor_id: Option<u64>,
    status: Option<String>,
    kind: Option<String>,
    r#type: Option<String>,
    method: Option<String>,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
    page: Option<u64>,
    per_page: Option<u64>,
}

pub async fn wallet_movements(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(patient_id): Path<u64>,
    Query(query): Query<LedgerQuery>,
) -> ApiResult<Json<Value>> {
    require_patient(&state, patient_id).await?;
    let page = pagination::PageQuery {
        page: query.page,
        per_page: query.per_page,
    };
    let total = sqlx::query_scalar::<_, u64>(
        "SELECT CAST(COUNT(*) AS UNSIGNED) FROM wallet_transactions \
         WHERE patient_id = ? AND (? IS NULL OR type = ?) \
           AND (? IS NULL OR DATE(created_at) >= ?) AND (? IS NULL OR DATE(created_at) <= ?)",
    )
    .bind(patient_id)
    .bind(&query.r#type)
    .bind(&query.r#type)
    .bind(query.from)
    .bind(query.from)
    .bind(query.to)
    .bind(query.to)
    .fetch_one(&state.pool)
    .await?;
    let rows = sqlx::query_as::<_, WalletTransactionRow>(
        "SELECT * FROM wallet_transactions \
         WHERE patient_id = ? AND (? IS NULL OR type = ?) \
           AND (? IS NULL OR DATE(created_at) >= ?) AND (? IS NULL OR DATE(created_at) <= ?) \
         ORDER BY id DESC LIMIT ? OFFSET ?",
    )
    .bind(patient_id)
    .bind(&query.r#type)
    .bind(&query.r#type)
    .bind(query.from)
    .bind(query.from)
    .bind(query.to)
    .bind(query.to)
    .bind(page.per_page())
    .bind(page.offset())
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(json!({
        "data": rows.iter().map(resources::wallet_transaction_value).collect::<Vec<_>>(),
        "meta": pagination::meta(page.page(), page.per_page(), total),
        "balance": money::normalize(loaders::wallet_balance(&state.pool, patient_id).await?),
    })))
}

pub async fn wallet_top_up(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(patient_id): Path<u64>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_patient(&state, patient_id).await?;
    let key = idempotency::require_key(&headers)?;
    let data = validation::object(&payload)?;
    let amount = validation::required_decimal(data, "amount", true)?;
    let method = validation::required_string(data, "method", 20, "طريقة الدفع مطلوبة.")?;
    validation::one_of(&method, &["cash", "transfer"], "method")?;
    let note = validation::optional_string(data, "note", 1000)?;
    let route = format!("api/v1/patients/{patient_id}/wallet/topups");
    let actor_id = auth.user.id;
    let (status, response) = idempotency::execute(
        &state.pool,
        &auth,
        &key,
        "POST",
        &route,
        &payload,
        move |transaction| {
            let method = method.clone();
            let note = note.clone();
            Box::pin(async move {
                let (balance, movement, payment) = ledger::top_up(
                    transaction,
                    patient_id,
                    amount,
                    &method,
                    note.as_deref(),
                    actor_id,
                )
                .await?;
                Ok((
                    StatusCode::CREATED,
                    json!({ "data": {
                        "balance": money::normalize(balance),
                        "movement": resources::wallet_transaction_value(&movement),
                        "payment": resources::payment_value(&payment, None),
                    }}),
                ))
            })
        },
    )
    .await?;
    Ok((status, Json(response)))
}

pub async fn debts_index(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(query): Query<LedgerQuery>,
) -> ApiResult<Json<Value>> {
    if auth.user.role == "receptionist" && query.patient_id.is_none() {
        return Err(ApiError::forbidden("حدد مريضًا لعرض ديونه."));
    }
    list_debts(&state, query).await
}

pub async fn patient_debts(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(patient_id): Path<u64>,
    Query(mut query): Query<LedgerQuery>,
) -> ApiResult<Json<Value>> {
    require_patient(&state, patient_id).await?;
    query.patient_id = Some(patient_id);
    let _ = auth;
    list_debts(&state, query).await
}

async fn list_debts(state: &AppState, query: LedgerQuery) -> ApiResult<Json<Value>> {
    let page = pagination::PageQuery {
        page: query.page,
        per_page: query.per_page,
    };
    let total = sqlx::query_scalar::<_, u64>(
        "SELECT CAST(COUNT(*) AS UNSIGNED) FROM debts WHERE (? IS NULL OR patient_id = ?) AND (? IS NULL OR sponsor_id = ?) \
         AND (? IS NULL OR status = ?) AND (? IS NULL OR kind = ?) \
         AND (? IS NULL OR DATE(created_at) >= ?) AND (? IS NULL OR DATE(created_at) <= ?)",
    )
    .bind(query.patient_id)
    .bind(query.patient_id)
    .bind(query.sponsor_id)
    .bind(query.sponsor_id)
    .bind(&query.status)
    .bind(&query.status)
    .bind(&query.kind)
    .bind(&query.kind)
    .bind(query.from)
    .bind(query.from)
    .bind(query.to)
    .bind(query.to)
    .fetch_one(&state.pool)
    .await?;
    let rows = sqlx::query_as::<_, DebtRow>(
        "SELECT * FROM debts WHERE (? IS NULL OR patient_id = ?) AND (? IS NULL OR sponsor_id = ?) \
         AND (? IS NULL OR status = ?) AND (? IS NULL OR kind = ?) \
         AND (? IS NULL OR DATE(created_at) >= ?) AND (? IS NULL OR DATE(created_at) <= ?) \
         ORDER BY id DESC LIMIT ? OFFSET ?",
    )
    .bind(query.patient_id)
    .bind(query.patient_id)
    .bind(query.sponsor_id)
    .bind(query.sponsor_id)
    .bind(&query.status)
    .bind(&query.status)
    .bind(&query.kind)
    .bind(&query.kind)
    .bind(query.from)
    .bind(query.from)
    .bind(query.to)
    .bind(query.to)
    .bind(page.per_page())
    .bind(page.offset())
    .fetch_all(&state.pool)
    .await?;
    let original = money::sum(rows.iter().map(|debt| debt.amount));
    let paid = money::sum(rows.iter().map(|debt| debt.paid_amount));
    let remaining = money::sum(rows.iter().map(|debt| {
        if debt.status == "void" {
            Decimal::ZERO
        } else {
            debt.amount - debt.paid_amount
        }
    }));
    Ok(Json(json!({
        "data": rows.iter().map(resources::debt_value).collect::<Vec<_>>(),
        "meta": pagination::meta(page.page(), page.per_page(), total),
        "totals": {
            "original": money::normalize(original),
            "paid": money::normalize(paid),
            "remaining": money::normalize(remaining),
        },
    })))
}

pub async fn opening_debt(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(patient_id): Path<u64>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    auth.require_admin()?;
    require_patient(&state, patient_id).await?;
    let key = idempotency::require_key(&headers)?;
    let data = validation::object(&payload)?;
    let amount = validation::required_decimal(data, "amount", true)?;
    let sponsor_id = validation::optional_u64(data, "sponsor_id")?;
    let note = validation::optional_string(data, "note", 1000)?;
    let actor_id = auth.user.id;
    let route = format!("api/v1/patients/{patient_id}/opening-debts");
    let (status, response) = idempotency::execute(
        &state.pool,
        &auth,
        &key,
        "POST",
        &route,
        &payload,
        move |transaction| {
            let note = note.clone();
            Box::pin(async move {
                let debt = ledger::create_opening_debt(
                    transaction,
                    patient_id,
                    amount,
                    sponsor_id,
                    note.as_deref(),
                    actor_id,
                )
                .await?;
                Ok((
                    StatusCode::CREATED,
                    json!({ "data": resources::debt_value(&debt) }),
                ))
            })
        },
    )
    .await?;
    Ok((status, Json(response)))
}

pub async fn settle_debts(
    State(state): State<AppState>,
    auth: AuthUser,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let key = idempotency::require_key(&headers)?;
    let data = validation::object(&payload)?;
    let amount = validation::required_decimal(data, "amount", true)?;
    let method = validation::required_string(data, "method", 20, "طريقة الدفع مطلوبة.")?;
    validation::one_of(&method, &["cash", "transfer"], "method")?;
    let raw_allocations = data
        .get("allocations")
        .and_then(Value::as_array)
        .filter(|values| !values.is_empty())
        .ok_or_else(|| {
            ApiError::validation("حدد تخصيصًا واحدًا على الأقل.")
                .field("allocations", "حدد تخصيصًا واحدًا على الأقل.")
        })?;
    let mut allocations = Vec::new();
    let mut ids = HashSet::new();
    for (index, raw) in raw_allocations.iter().enumerate() {
        let item = validation::object(raw)?;
        let debt_id = validation::required_u64(item, "debt_id")?;
        if !ids.insert(debt_id) {
            return Err(ApiError::validation("البيانات المرسلة غير صحيحة.").field(
                format!("allocations.{index}.debt_id"),
                "لا يجوز تكرار الدين نفسه.",
            ));
        }
        let allocation = validation::required_decimal(item, "amount", true)?;
        allocations.push((debt_id, allocation));
    }
    let actor_id = auth.user.id;
    let (status, response) = idempotency::execute(
        &state.pool,
        &auth,
        &key,
        "POST",
        "api/v1/debts/settlements",
        &payload,
        move |transaction| {
            let method = method.clone();
            let allocations = allocations.clone();
            Box::pin(async move {
                let (payment, allocations) = ledger::settle_debts(
                    transaction,
                    amount,
                    &method,
                    &allocations,
                    actor_id,
                )
                .await?;
                Ok((StatusCode::CREATED, json!({ "data": {
                    "payment": resources::payment_value(&payment, None),
                    "allocations": allocations.iter().map(resources::allocation_value).collect::<Vec<_>>(),
                }})))
            })
        },
    )
    .await?;
    Ok((status, Json(response)))
}

pub async fn payments_index(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(query): Query<LedgerQuery>,
) -> ApiResult<Json<Value>> {
    if auth.user.role == "receptionist" && query.patient_id.is_none() {
        return Err(ApiError::forbidden("حدد مريضًا لعرض مدفوعاته."));
    }
    list_payments(&state, query).await
}

pub async fn patient_payments(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(patient_id): Path<u64>,
    Query(mut query): Query<LedgerQuery>,
) -> ApiResult<Json<Value>> {
    require_patient(&state, patient_id).await?;
    query.patient_id = Some(patient_id);
    list_payments(&state, query).await
}

async fn list_payments(state: &AppState, query: LedgerQuery) -> ApiResult<Json<Value>> {
    let page = pagination::PageQuery {
        page: query.page,
        per_page: query.per_page,
    };
    let filter = "FROM payments AS p WHERE (? IS NULL OR p.patient_id = ?) \
        AND (? IS NULL OR p.type = ?) AND (? IS NULL OR p.method = ?) \
        AND (? IS NULL OR DATE(p.created_at) >= ?) AND (? IS NULL OR DATE(p.created_at) <= ?) \
        AND (? IS NULL OR EXISTS (SELECT 1 FROM payment_allocations pa JOIN debts d ON d.id = pa.debt_id WHERE pa.payment_id = p.id AND d.sponsor_id = ?) \
          OR EXISTS (SELECT 1 FROM sessions s WHERE s.id = p.session_id AND s.sponsor_id = ?) \
          OR EXISTS (SELECT 1 FROM subscriptions sub WHERE sub.id = p.subscription_id AND sub.sponsor_id = ?))";
    let count_sql = format!("SELECT CAST(COUNT(*) AS UNSIGNED) {filter}");
    let select_sql = format!("SELECT p.* {filter} ORDER BY p.id DESC LIMIT ? OFFSET ?");
    let mut count = sqlx::query_scalar::<_, u64>(&count_sql);
    count = bind_payment_filters(count, &query);
    let total = count.fetch_one(&state.pool).await?;
    let mut select = sqlx::query_as::<_, PaymentRow>(&select_sql);
    select = bind_payment_filters_as(select, &query);
    let rows = select
        .bind(page.per_page())
        .bind(page.offset())
        .fetch_all(&state.pool)
        .await?;
    let amount = money::sum(rows.iter().map(|payment| payment.amount));
    Ok(Json(json!({
        "data": rows.iter().map(|payment| resources::payment_value(payment, None)).collect::<Vec<_>>(),
        "meta": pagination::meta(page.page(), page.per_page(), total),
        "totals": { "amount": money::normalize(amount) },
    })))
}

fn bind_payment_filters<'q>(
    query: sqlx::query::QueryScalar<'q, sqlx::MySql, u64, sqlx::mysql::MySqlArguments>,
    filters: &'q LedgerQuery,
) -> sqlx::query::QueryScalar<'q, sqlx::MySql, u64, sqlx::mysql::MySqlArguments> {
    query
        .bind(filters.patient_id)
        .bind(filters.patient_id)
        .bind(&filters.r#type)
        .bind(&filters.r#type)
        .bind(&filters.method)
        .bind(&filters.method)
        .bind(filters.from)
        .bind(filters.from)
        .bind(filters.to)
        .bind(filters.to)
        .bind(filters.sponsor_id)
        .bind(filters.sponsor_id)
        .bind(filters.sponsor_id)
        .bind(filters.sponsor_id)
}

fn bind_payment_filters_as<'q>(
    query: sqlx::query::QueryAs<'q, sqlx::MySql, PaymentRow, sqlx::mysql::MySqlArguments>,
    filters: &'q LedgerQuery,
) -> sqlx::query::QueryAs<'q, sqlx::MySql, PaymentRow, sqlx::mysql::MySqlArguments> {
    query
        .bind(filters.patient_id)
        .bind(filters.patient_id)
        .bind(&filters.r#type)
        .bind(&filters.r#type)
        .bind(&filters.method)
        .bind(&filters.method)
        .bind(filters.from)
        .bind(filters.from)
        .bind(filters.to)
        .bind(filters.to)
        .bind(filters.sponsor_id)
        .bind(filters.sponsor_id)
        .bind(filters.sponsor_id)
        .bind(filters.sponsor_id)
}

pub async fn refund(
    State(state): State<AppState>,
    auth: AuthUser,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    auth.require_admin()?;
    let key = idempotency::require_key(&headers)?;
    let data = validation::object(&payload)?;
    let patient_id = validation::required_u64(data, "patient_id")?;
    let payment_id = validation::required_u64(data, "payment_id")?;
    let amount = validation::required_decimal(data, "amount", true)?;
    let method = validation::required_string(data, "method", 20, "طريقة الدفع مطلوبة.")?;
    validation::one_of(&method, &["cash", "transfer"], "method")?;
    let reason = validation::required_string(data, "reason", 1000, "سبب الاسترجاع مطلوب.")?;
    let actor_id = auth.user.id;
    let (status, response) = idempotency::execute(
        &state.pool,
        &auth,
        &key,
        "POST",
        "api/v1/refunds",
        &payload,
        move |transaction| {
            let method = method.clone();
            let reason = reason.clone();
            Box::pin(async move {
                let payment = ledger::refund(
                    transaction,
                    patient_id,
                    payment_id,
                    amount,
                    &method,
                    &reason,
                    actor_id,
                )
                .await?;
                Ok((
                    StatusCode::CREATED,
                    json!({ "data": resources::payment_value(&payment, None) }),
                ))
            })
        },
    )
    .await?;
    Ok((status, Json(response)))
}

async fn require_patient(state: &AppState, id: u64) -> ApiResult<()> {
    if sqlx::query_scalar::<_, u64>("SELECT CAST(COUNT(*) AS UNSIGNED) FROM patients WHERE id = ?")
        .bind(id)
        .fetch_one(&state.pool)
        .await?
        == 0
    {
        return Err(ApiError::not_found());
    }
    Ok(())
}
