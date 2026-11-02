use crate::api::{
    audit,
    auth::AuthUser,
    error::{ApiError, ApiResult},
    idempotency, loaders, money, pagination, resources,
    rows::{DoctorPayoutRow, DoctorRow, SessionRow},
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

#[derive(Debug, Deserialize)]
pub struct DoctorQuery {
    search: Option<String>,
    is_active: Option<bool>,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
    page: Option<u64>,
    per_page: Option<u64>,
}

pub async fn index(
    State(state): State<AppState>,
    _auth: AuthUser,
    Query(query): Query<DoctorQuery>,
) -> ApiResult<Json<Value>> {
    let page = pagination::PageQuery {
        page: query.page,
        per_page: query.per_page,
    };
    let search = query.search.as_deref().unwrap_or("").trim();
    let pattern = format!("%{search}%");
    let total = sqlx::query_scalar::<_, u64>("SELECT CAST(COUNT(*) AS UNSIGNED) FROM doctors WHERE (? = '' OR name LIKE ?) AND (? IS NULL OR is_active = ?)")
        .bind(search).bind(&pattern).bind(query.is_active).bind(query.is_active)
        .fetch_one(&state.pool).await?;
    let doctors = sqlx::query_as::<_, DoctorRow>(
        "SELECT * FROM doctors WHERE (? = '' OR name LIKE ?) AND (? IS NULL OR is_active = ?) ORDER BY id DESC LIMIT ? OFFSET ?",
    ).bind(search).bind(&pattern).bind(query.is_active).bind(query.is_active)
        .bind(page.per_page()).bind(page.offset()).fetch_all(&state.pool).await?;
    let mut data = Vec::new();
    for doctor in &doctors {
        let (count, earned, paid) = period_totals(&state, doctor.id, query.from, query.to).await?;
        data.push(resources::doctor_value(doctor, count, earned, paid));
    }
    Ok(Json(
        json!({ "data": data, "meta": pagination::meta(page.page(), page.per_page(), total) }),
    ))
}

pub async fn store(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(payload): Json<Value>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    auth.require_admin()?;
    let input = parse_input(&payload, None)?;
    let mut transaction = state.pool.begin().await?;
    let result = sqlx::query("INSERT INTO doctors (name, phone, commission_rate, is_active, created_by, created_at, updated_at) VALUES (?, ?, ?, ?, ?, NOW(), NOW())")
        .bind(&input.name).bind(&input.phone).bind(input.commission_rate).bind(input.is_active).bind(auth.user.id)
        .execute(&mut *transaction).await?;
    let id = result.last_insert_id();
    let values = json!({ "id": id, "name": input.name, "phone": input.phone, "commission_rate": money::normalize(input.commission_rate), "is_active": input.is_active, "created_by": auth.user.id });
    audit::log(
        &mut transaction,
        auth.user.id,
        "doctor_created",
        "doctor",
        id,
        None,
        Some(&values),
        None,
    )
    .await?;
    transaction.commit().await?;
    let doctor = fetch(&state, id).await?;
    Ok((
        StatusCode::CREATED,
        Json(json!({ "data": resources::doctor_value(&doctor, 0, Decimal::ZERO, Decimal::ZERO) })),
    ))
}

pub async fn show(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<u64>,
    Query(query): Query<DoctorQuery>,
) -> ApiResult<Json<Value>> {
    auth.require_admin()?;
    detail(&state, id, query, false).await
}

pub async fn dues(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<u64>,
    Query(query): Query<DoctorQuery>,
) -> ApiResult<Json<Value>> {
    auth.require_admin()?;
    detail(&state, id, query, true).await
}

async fn detail(
    state: &AppState,
    id: u64,
    query: DoctorQuery,
    dues: bool,
) -> ApiResult<Json<Value>> {
    let doctor = fetch(state, id).await?;
    let (count, earned, paid) = period_totals(state, id, query.from, query.to).await?;
    let sessions = sqlx::query_as::<_, SessionRow>(
        "SELECT * FROM sessions WHERE doctor_id = ? AND status = 'closed' AND (? IS NULL OR DATE(closed_at) >= ?) AND (? IS NULL OR DATE(closed_at) <= ?) ORDER BY closed_at DESC",
    ).bind(id).bind(query.from).bind(query.from).bind(query.to).bind(query.to).fetch_all(&state.pool).await?;
    let mut session_values = Vec::new();
    for session in &sessions {
        session_values.push(loaders::session_from_row(&state.pool, session).await?);
    }
    let payouts = sqlx::query_as::<_, DoctorPayoutRow>(
        "SELECT * FROM doctor_payouts WHERE doctor_id = ? AND (? IS NULL OR date >= ?) AND (? IS NULL OR date <= ?) ORDER BY date DESC, id DESC",
    ).bind(id).bind(query.from).bind(query.from).bind(query.to).bind(query.to).fetch_all(&state.pool).await?;
    let doctor_value = resources::doctor_value(&doctor, count, earned, paid);
    let mut data = if dues {
        json!({
            "doctor": doctor_value,
            "sessions": session_values,
            "payouts": payouts.iter().map(resources::doctor_payout_value).collect::<Vec<_>>(),
            "all_time_remaining": money::normalize(all_time_remaining(state, id).await?),
        })
    } else {
        doctor_value
    };
    if !dues {
        let map = data.as_object_mut().unwrap();
        map.insert("sessions".into(), Value::Array(session_values));
        map.insert(
            "payouts".into(),
            Value::Array(payouts.iter().map(resources::doctor_payout_value).collect()),
        );
    }
    Ok(Json(json!({ "data": data })))
}

pub async fn update(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<u64>,
    Json(payload): Json<Value>,
) -> ApiResult<Json<Value>> {
    auth.require_admin()?;
    let current = fetch(&state, id).await?;
    let input = parse_input(&payload, Some(&current))?;
    let rate_changed = input.commission_rate != current.commission_rate;
    if rate_changed && input.reason.is_none() {
        return Err(ApiError::validation("سبب تغيير نسبة الطبيب مطلوب.")
            .field("reason", "اكتب سبب تغيير النسبة."));
    }
    let old_values = doctor_snapshot(&current);
    let new_values = json!({ "id": id, "name": input.name, "phone": input.phone, "commission_rate": money::normalize(input.commission_rate), "is_active": input.is_active, "created_by": current.created_by });
    let mut transaction = state.pool.begin().await?;
    sqlx::query("UPDATE doctors SET name = ?, phone = ?, commission_rate = ?, is_active = ?, updated_at = NOW() WHERE id = ?")
        .bind(&input.name).bind(&input.phone).bind(input.commission_rate).bind(input.is_active).bind(id)
        .execute(&mut *transaction).await?;
    audit::log(
        &mut transaction,
        auth.user.id,
        if rate_changed {
            "doctor_rate_changed"
        } else {
            "doctor_updated"
        },
        "doctor",
        id,
        Some(&old_values),
        Some(&new_values),
        input.reason.as_deref(),
    )
    .await?;
    transaction.commit().await?;
    let doctor = fetch(&state, id).await?;
    Ok(Json(
        json!({ "data": resources::doctor_value(&doctor, 0, Decimal::ZERO, Decimal::ZERO) }),
    ))
}

pub async fn payout(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<u64>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    auth.require_admin()?;
    fetch(&state, id).await?;
    let key = idempotency::require_key(&headers)?;
    let data = validation::object(&payload)?;
    let amount = validation::required_decimal(data, "amount", true)?;
    let date = validation::required_date(data, "date")?;
    let method = validation::required_string(data, "method", 20, "طريقة الدفع مطلوبة.")?;
    validation::one_of(&method, &["cash", "transfer"], "method")?;
    let note = validation::optional_string(data, "note", 1000)?;
    let actor_id = auth.user.id;
    let route = format!("api/v1/doctors/{id}/payouts");
    let (status, response) = idempotency::execute(&state.pool, &auth, &key, "POST", &route, &payload, move |transaction| {
        let method = method.clone();
        let note = note.clone();
        Box::pin(async move {
            let payout = ledger::doctor_payout(transaction, id, amount, date, &method, note.as_deref(), actor_id).await?;
            let remaining = {
                let earned = sqlx::query_scalar::<_, Option<Decimal>>("SELECT SUM(commission_amount) FROM sessions WHERE doctor_id = ? AND status = 'closed'").bind(id).fetch_one(&mut **transaction).await?.unwrap_or(Decimal::ZERO);
                let paid = sqlx::query_scalar::<_, Option<Decimal>>("SELECT SUM(amount) FROM doctor_payouts WHERE doctor_id = ?").bind(id).fetch_one(&mut **transaction).await?.unwrap_or(Decimal::ZERO);
                earned - paid
            };
            Ok((StatusCode::CREATED, json!({ "data": {
                "payout": resources::doctor_payout_value(&payout),
                "remaining": money::normalize(remaining),
            }})))
        })
    }).await?;
    Ok((status, Json(response)))
}

struct DoctorInput {
    name: String,
    phone: Option<String>,
    commission_rate: Decimal,
    is_active: bool,
    reason: Option<String>,
}

fn parse_input(payload: &Value, current: Option<&DoctorRow>) -> ApiResult<DoctorInput> {
    let data = validation::object(payload)?;
    let commission_rate = validation::required_decimal(data, "commission_rate", false)?;
    if commission_rate > Decimal::ONE_HUNDRED {
        return Err(ApiError::validation("البيانات المرسلة غير صحيحة.")
            .field("commission_rate", "يجب ألا تتجاوز النسبة 100."));
    }
    Ok(DoctorInput {
        name: validation::required_string(data, "name", 255, "اسم الطبيب مطلوب.")?,
        phone: validation::optional_string(data, "phone", 30)?,
        commission_rate,
        is_active: validation::boolean(data, "is_active", current.is_none_or(|row| row.is_active))?,
        reason: validation::optional_string(data, "reason", 1000)?,
    })
}

async fn fetch(state: &AppState, id: u64) -> ApiResult<DoctorRow> {
    sqlx::query_as::<_, DoctorRow>("SELECT * FROM doctors WHERE id = ?")
        .bind(id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(ApiError::not_found)
}

async fn period_totals(
    state: &AppState,
    id: u64,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
) -> ApiResult<(u64, Decimal, Decimal)> {
    let count = sqlx::query_scalar::<_, u64>("SELECT CAST(COUNT(*) AS UNSIGNED) FROM sessions WHERE doctor_id = ? AND status = 'closed' AND (? IS NULL OR DATE(closed_at) >= ?) AND (? IS NULL OR DATE(closed_at) <= ?)")
        .bind(id).bind(from).bind(from).bind(to).bind(to).fetch_one(&state.pool).await?;
    let earned = sqlx::query_scalar::<_, Option<Decimal>>("SELECT SUM(commission_amount) FROM sessions WHERE doctor_id = ? AND status = 'closed' AND (? IS NULL OR DATE(closed_at) >= ?) AND (? IS NULL OR DATE(closed_at) <= ?)")
        .bind(id).bind(from).bind(from).bind(to).bind(to).fetch_one(&state.pool).await?.unwrap_or(Decimal::ZERO);
    let paid = sqlx::query_scalar::<_, Option<Decimal>>("SELECT SUM(amount) FROM doctor_payouts WHERE doctor_id = ? AND (? IS NULL OR date >= ?) AND (? IS NULL OR date <= ?)")
        .bind(id).bind(from).bind(from).bind(to).bind(to).fetch_one(&state.pool).await?.unwrap_or(Decimal::ZERO);
    Ok((count, earned, paid))
}

async fn all_time_remaining(state: &AppState, id: u64) -> ApiResult<Decimal> {
    let (_, earned, paid) = period_totals(state, id, None, None).await?;
    Ok(earned - paid)
}

fn doctor_snapshot(doctor: &DoctorRow) -> Value {
    json!({ "id": doctor.id, "name": doctor.name, "phone": doctor.phone, "commission_rate": money::normalize(doctor.commission_rate), "is_active": doctor.is_active, "created_by": doctor.created_by })
}
