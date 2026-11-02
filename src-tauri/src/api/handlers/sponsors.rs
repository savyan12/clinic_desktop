use crate::api::{
    audit,
    auth::AuthUser,
    error::{ApiError, ApiResult},
    money, pagination, resources,
    rows::{
        DebtRow, PatientRow, PaymentAllocationRow, PaymentRow, SessionRow, SessionServiceRow,
        SponsorRow,
    },
    validation, AppState,
};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Debug, Deserialize)]
pub struct SponsorListQuery {
    search: Option<String>,
    is_active: Option<bool>,
    page: Option<u64>,
    per_page: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct SponsorStatementQuery {
    patient_id: Option<u64>,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
}

pub async fn index(
    State(state): State<AppState>,
    _auth: AuthUser,
    Query(query): Query<SponsorListQuery>,
) -> ApiResult<Json<Value>> {
    let page = pagination::PageQuery {
        page: query.page,
        per_page: query.per_page,
    };
    let search = query.search.as_deref().unwrap_or("").trim();
    let pattern = format!("%{search}%");
    let total = sqlx::query_scalar::<_, u64>(
        "SELECT CAST(COUNT(*) AS UNSIGNED) FROM sponsors \
         WHERE (? = '' OR name LIKE ?) AND (? IS NULL OR is_active = ?)",
    )
    .bind(search)
    .bind(&pattern)
    .bind(query.is_active)
    .bind(query.is_active)
    .fetch_one(&state.pool)
    .await?;
    let sponsors = sqlx::query_as::<_, SponsorRow>(
        "SELECT * FROM sponsors \
         WHERE (? = '' OR name LIKE ?) AND (? IS NULL OR is_active = ?) \
         ORDER BY id DESC LIMIT ? OFFSET ?",
    )
    .bind(search)
    .bind(&pattern)
    .bind(query.is_active)
    .bind(query.is_active)
    .bind(page.per_page())
    .bind(page.offset())
    .fetch_all(&state.pool)
    .await?;

    let mut data = Vec::with_capacity(sponsors.len());
    for sponsor in &sponsors {
        let (patients_count, total_due, total_paid) = totals(&state, sponsor.id).await?;
        data.push(resources::sponsor_value(
            sponsor,
            patients_count,
            total_due,
            total_paid,
        ));
    }

    Ok(Json(json!({
        "data": data,
        "meta": pagination::meta(page.page(), page.per_page(), total),
    })))
}

pub async fn store(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(payload): Json<Value>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    auth.require_admin()?;
    let input = parse_input(&payload, None)?;
    let mut transaction = state.pool.begin().await?;
    let result = sqlx::query(
        "INSERT INTO sponsors (name, phone, address, is_active, created_by, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, NOW(), NOW())",
    )
    .bind(&input.name)
    .bind(&input.phone)
    .bind(&input.address)
    .bind(input.is_active)
    .bind(auth.user.id)
    .execute(&mut *transaction)
    .await?;
    let id = result.last_insert_id();
    let values = input.snapshot(id, auth.user.id);
    audit::log(
        &mut transaction,
        auth.user.id,
        "sponsor_created",
        "sponsor",
        id,
        None,
        Some(&values),
        None,
    )
    .await?;
    transaction.commit().await?;
    let sponsor = fetch(&state, id).await?;

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "data": resources::sponsor_value(
                &sponsor,
                0,
                Decimal::ZERO,
                Decimal::ZERO,
            ),
        })),
    ))
}

pub async fn show(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<u64>,
) -> ApiResult<Json<Value>> {
    auth.require_admin()?;
    let sponsor = fetch(&state, id).await?;
    let (patients_count, total_due, total_paid) = totals(&state, id).await?;
    let patients = sponsor_patients(&state, id, None, None, None, true).await?;
    let debts = sponsor_debts(&state, id, None, None, None).await?;
    let sessions = sponsor_sessions(&state, id, None, None, None).await?;
    let payments = sponsor_payments(&state, id, None, None, None).await?;
    let mut sponsor_value =
        resources::sponsor_value(&sponsor, patients_count, total_due, total_paid);
    let object = sponsor_value
        .as_object_mut()
        .expect("sponsor resource must be an object");
    object.insert(
        "patients".into(),
        Value::Array(patients.iter().map(patient_summary).collect()),
    );
    object.insert("sessions".into(), Value::Array(sessions));
    object.insert(
        "debts".into(),
        Value::Array(debts.iter().map(resources::debt_value).collect()),
    );
    object.insert(
        "payments".into(),
        Value::Array(payments.iter().map(payment_without_patient).collect()),
    );

    Ok(Json(json!({ "data": sponsor_value })))
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
    let old_values = sponsor_snapshot(&current);
    let new_values = input.snapshot(id, current.created_by);
    let mut transaction = state.pool.begin().await?;
    sqlx::query(
        "UPDATE sponsors SET name = ?, phone = ?, address = ?, is_active = ?, updated_at = NOW() \
         WHERE id = ?",
    )
    .bind(&input.name)
    .bind(&input.phone)
    .bind(&input.address)
    .bind(input.is_active)
    .bind(id)
    .execute(&mut *transaction)
    .await?;
    audit::log(
        &mut transaction,
        auth.user.id,
        "sponsor_updated",
        "sponsor",
        id,
        Some(&old_values),
        Some(&new_values),
        None,
    )
    .await?;
    transaction.commit().await?;
    let sponsor = fetch(&state, id).await?;
    let (patients_count, total_due, total_paid) = totals(&state, id).await?;

    Ok(Json(json!({
        "data": resources::sponsor_value(
            &sponsor,
            patients_count,
            total_due,
            total_paid,
        ),
    })))
}

pub async fn statement(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<u64>,
    Query(query): Query<SponsorStatementQuery>,
) -> ApiResult<Json<Value>> {
    auth.require_admin()?;
    let sponsor = fetch(&state, id).await?;
    let patients =
        sponsor_patients(&state, id, query.patient_id, query.from, query.to, false).await?;
    let debts = sponsor_debts(&state, id, query.patient_id, query.from, query.to).await?;
    let sessions = sponsor_sessions(&state, id, query.patient_id, query.from, query.to).await?;
    let payments = sponsor_payments(&state, id, query.patient_id, query.from, query.to).await?;
    let allocations =
        sponsor_allocations(&state, id, query.patient_id, query.from, query.to).await?;
    let (due, paid) = debts
        .iter()
        .filter(|debt| debt.status != "void")
        .fold((Decimal::ZERO, Decimal::ZERO), |(due, paid), debt| {
            (due + debt.amount, paid + debt.paid_amount)
        });

    Ok(Json(json!({
        "data": {
            "sponsor": { "id": sponsor.id, "name": sponsor.name },
            "patients": patients.iter().map(patient_summary).collect::<Vec<_>>(),
            "sessions": sessions,
            "debts": debts.iter().map(resources::debt_value).collect::<Vec<_>>(),
            "payments": payments.iter().map(payment_without_patient).collect::<Vec<_>>(),
            "allocations": allocations.iter().map(resources::allocation_value).collect::<Vec<_>>(),
            "totals": {
                "due": money::normalize(due),
                "paid": money::normalize(paid),
                "remaining": money::normalize(due - paid),
            },
        },
    })))
}

struct SponsorInput {
    name: String,
    phone: Option<String>,
    address: Option<String>,
    is_active: bool,
}

impl SponsorInput {
    fn snapshot(&self, id: u64, created_by: u64) -> Value {
        json!({
            "id": id,
            "name": self.name,
            "phone": self.phone,
            "address": self.address,
            "is_active": self.is_active,
            "created_by": created_by,
        })
    }
}

fn parse_input(payload: &Value, current: Option<&SponsorRow>) -> ApiResult<SponsorInput> {
    let data = validation::object(payload)?;
    Ok(SponsorInput {
        name: validation::required_string(data, "name", 255, "اسم الجهة الراعية مطلوب.")?,
        phone: validation::optional_string(data, "phone", 30)?,
        address: validation::optional_string(data, "address", 255)?,
        is_active: validation::boolean(data, "is_active", current.is_none_or(|row| row.is_active))?,
    })
}

async fn fetch(state: &AppState, id: u64) -> ApiResult<SponsorRow> {
    sqlx::query_as::<_, SponsorRow>("SELECT * FROM sponsors WHERE id = ?")
        .bind(id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(ApiError::not_found)
}

async fn totals(state: &AppState, sponsor_id: u64) -> ApiResult<(u64, Decimal, Decimal)> {
    let patients_count = sqlx::query_scalar::<_, u64>(
        "SELECT CAST(COUNT(*) AS UNSIGNED) FROM patients WHERE sponsor_id = ?",
    )
    .bind(sponsor_id)
    .fetch_one(&state.pool)
    .await?;
    let total_due = sqlx::query_scalar::<_, Option<Decimal>>(
        "SELECT SUM(amount) FROM debts WHERE sponsor_id = ? AND status != 'void'",
    )
    .bind(sponsor_id)
    .fetch_one(&state.pool)
    .await?
    .unwrap_or(Decimal::ZERO);
    let total_paid = sqlx::query_scalar::<_, Option<Decimal>>(
        "SELECT SUM(paid_amount) FROM debts WHERE sponsor_id = ? AND status != 'void'",
    )
    .bind(sponsor_id)
    .fetch_one(&state.pool)
    .await?
    .unwrap_or(Decimal::ZERO);
    Ok((patients_count, total_due, total_paid))
}

async fn sponsor_patients(
    state: &AppState,
    sponsor_id: u64,
    patient_id: Option<u64>,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
    include_linked_patients: bool,
) -> ApiResult<Vec<PatientRow>> {
    sqlx::query_as::<_, PatientRow>(
        "SELECT DISTINCT p.* FROM patients p \
         WHERE (? IS NULL OR p.id = ?) \
           AND ((? = 1 AND p.sponsor_id = ?) OR EXISTS ( \
               SELECT 1 FROM debts d WHERE d.patient_id = p.id AND d.sponsor_id = ? \
                 AND (? IS NULL OR DATE(d.created_at) >= ?) \
                 AND (? IS NULL OR DATE(d.created_at) <= ?) \
           )) \
         ORDER BY p.name",
    )
    .bind(patient_id)
    .bind(patient_id)
    .bind(include_linked_patients)
    .bind(sponsor_id)
    .bind(sponsor_id)
    .bind(from)
    .bind(from)
    .bind(to)
    .bind(to)
    .fetch_all(&state.pool)
    .await
    .map_err(Into::into)
}

async fn sponsor_debts(
    state: &AppState,
    sponsor_id: u64,
    patient_id: Option<u64>,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
) -> ApiResult<Vec<DebtRow>> {
    sqlx::query_as::<_, DebtRow>(
        "SELECT * FROM debts \
         WHERE sponsor_id = ? \
           AND (? IS NULL OR patient_id = ?) \
           AND (? IS NULL OR DATE(created_at) >= ?) \
           AND (? IS NULL OR DATE(created_at) <= ?) \
         ORDER BY id DESC",
    )
    .bind(sponsor_id)
    .bind(patient_id)
    .bind(patient_id)
    .bind(from)
    .bind(from)
    .bind(to)
    .bind(to)
    .fetch_all(&state.pool)
    .await
    .map_err(Into::into)
}

async fn sponsor_sessions(
    state: &AppState,
    sponsor_id: u64,
    patient_id: Option<u64>,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
) -> ApiResult<Vec<Value>> {
    let sessions = sqlx::query_as::<_, SessionRow>(
        "SELECT DISTINCT s.* FROM sessions s \
         WHERE EXISTS ( \
             SELECT 1 FROM debts d \
             WHERE d.sponsor_id = ? \
               AND (? IS NULL OR d.patient_id = ?) \
               AND (? IS NULL OR DATE(d.created_at) >= ?) \
               AND (? IS NULL OR DATE(d.created_at) <= ?) \
               AND (d.session_id = s.id OR (d.subscription_id IS NOT NULL AND d.subscription_id = s.subscription_id)) \
         ) \
         ORDER BY s.opened_at DESC",
    )
    .bind(sponsor_id)
    .bind(patient_id)
    .bind(patient_id)
    .bind(from)
    .bind(from)
    .bind(to)
    .bind(to)
    .fetch_all(&state.pool)
    .await?;
    let mut values = Vec::with_capacity(sessions.len());
    for session in &sessions {
        let patient = sqlx::query_as::<_, PatientRow>("SELECT * FROM patients WHERE id = ?")
            .bind(session.patient_id)
            .fetch_one(&state.pool)
            .await?;
        let services = sqlx::query_as::<_, SessionServiceRow>(
            "SELECT * FROM session_services WHERE session_id = ? ORDER BY id",
        )
        .bind(session.id)
        .fetch_all(&state.pool)
        .await?;
        values.push(json!({
            "id": session.id,
            "code": session.code,
            "patient_id": session.patient_id,
            "patient": patient_summary(&patient),
            "services": services.iter().map(|service| json!({
                "service_id": service.service_id,
                "name": service.service_name,
                "price": money::normalize(service.price),
            })).collect::<Vec<_>>(),
            "status": session.status,
            "opened_at": crate::api::time::iso(session.opened_at),
            "closed_at": session.closed_at.map(crate::api::time::iso),
            "net_total": money::normalize(session.net_total),
        }));
    }
    Ok(values)
}

async fn sponsor_payments(
    state: &AppState,
    sponsor_id: u64,
    patient_id: Option<u64>,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
) -> ApiResult<Vec<PaymentRow>> {
    sqlx::query_as::<_, PaymentRow>(
        "SELECT DISTINCT p.* FROM payments p \
         WHERE p.id IN ( \
             SELECT pa.payment_id FROM payment_allocations pa \
             INNER JOIN debts d ON d.id = pa.debt_id \
             WHERE d.sponsor_id = ? \
               AND (? IS NULL OR d.patient_id = ?) \
               AND (? IS NULL OR DATE(d.created_at) >= ?) \
               AND (? IS NULL OR DATE(d.created_at) <= ?) \
         ) OR p.reverses_payment_id IN ( \
             SELECT pa.payment_id FROM payment_allocations pa \
             INNER JOIN debts d ON d.id = pa.debt_id \
             WHERE d.sponsor_id = ? \
               AND (? IS NULL OR d.patient_id = ?) \
               AND (? IS NULL OR DATE(d.created_at) >= ?) \
               AND (? IS NULL OR DATE(d.created_at) <= ?) \
         ) \
         ORDER BY p.id DESC",
    )
    .bind(sponsor_id)
    .bind(patient_id)
    .bind(patient_id)
    .bind(from)
    .bind(from)
    .bind(to)
    .bind(to)
    .bind(sponsor_id)
    .bind(patient_id)
    .bind(patient_id)
    .bind(from)
    .bind(from)
    .bind(to)
    .bind(to)
    .fetch_all(&state.pool)
    .await
    .map_err(Into::into)
}

async fn sponsor_allocations(
    state: &AppState,
    sponsor_id: u64,
    patient_id: Option<u64>,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
) -> ApiResult<Vec<PaymentAllocationRow>> {
    sqlx::query_as::<_, PaymentAllocationRow>(
        "SELECT pa.* FROM payment_allocations pa \
         INNER JOIN debts d ON d.id = pa.debt_id \
         WHERE d.sponsor_id = ? \
           AND (? IS NULL OR d.patient_id = ?) \
           AND (? IS NULL OR DATE(d.created_at) >= ?) \
           AND (? IS NULL OR DATE(d.created_at) <= ?) \
         ORDER BY d.id DESC, pa.id",
    )
    .bind(sponsor_id)
    .bind(patient_id)
    .bind(patient_id)
    .bind(from)
    .bind(from)
    .bind(to)
    .bind(to)
    .fetch_all(&state.pool)
    .await
    .map_err(Into::into)
}

fn patient_summary(patient: &PatientRow) -> Value {
    json!({
        "id": patient.id,
        "name": patient.name,
        "phone": patient.phone,
    })
}

fn payment_without_patient(payment: &PaymentRow) -> Value {
    let mut value = resources::payment_value(payment, None);
    value
        .as_object_mut()
        .expect("payment resource must be an object")
        .remove("patient");
    value
}

fn sponsor_snapshot(sponsor: &SponsorRow) -> Value {
    json!({
        "id": sponsor.id,
        "name": sponsor.name,
        "phone": sponsor.phone,
        "address": sponsor.address,
        "is_active": sponsor.is_active,
        "created_by": sponsor.created_by,
    })
}
