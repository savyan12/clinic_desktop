use crate::api::{
    audit,
    auth::AuthUser,
    error::{ApiError, ApiResult},
    loaders, money, pagination, resources,
    rows::{
        DebtRow, PatientRow, PaymentAllocationRow, PaymentRow, SessionRow, SubscriptionRow,
        WalletTransactionRow,
    },
    validation, AppState,
};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use chrono::NaiveDate;
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::Row;

#[derive(Debug, Deserialize)]
pub struct PatientListQuery {
    search: Option<String>,
    subscription_status: Option<String>,
    page: Option<u64>,
    per_page: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct PeriodQuery {
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
}

pub async fn index(
    State(state): State<AppState>,
    _auth: AuthUser,
    Query(query): Query<PatientListQuery>,
) -> ApiResult<Json<Value>> {
    let page = pagination::PageQuery {
        page: query.page,
        per_page: query.per_page,
    };
    let search = query.search.as_deref().unwrap_or("").trim();
    let pattern = format!("%{search}%");
    let status = query.subscription_status.as_deref().unwrap_or("");
    let filter = "FROM patients p WHERE (? = '' OR p.name LIKE ? OR p.phone LIKE ?) \
        AND (? = '' \
          OR (? = 'active' AND EXISTS (SELECT 1 FROM subscriptions sub WHERE sub.patient_id = p.id AND sub.status = 'active' \
             AND sub.sessions_used < sub.sessions_count)) \
          OR (? = 'none' AND NOT EXISTS (SELECT 1 FROM subscriptions sub WHERE sub.patient_id = p.id AND sub.status = 'active' \
             AND sub.sessions_used < sub.sessions_count)))";
    let count_sql = format!("SELECT CAST(COUNT(*) AS UNSIGNED) {filter}");
    let select_sql = format!("SELECT p.* {filter} ORDER BY p.id DESC LIMIT ? OFFSET ?");
    let total = sqlx::query_scalar::<_, u64>(&count_sql)
        .bind(search)
        .bind(&pattern)
        .bind(&pattern)
        .bind(status)
        .bind(status)
        .bind(status)
        .fetch_one(&state.pool)
        .await?;
    let rows = sqlx::query_as::<_, PatientRow>(&select_sql)
        .bind(search)
        .bind(&pattern)
        .bind(&pattern)
        .bind(status)
        .bind(status)
        .bind(status)
        .bind(page.per_page())
        .bind(page.offset())
        .fetch_all(&state.pool)
        .await?;
    let mut values = Vec::new();
    for patient in &rows {
        values.push(summary(&state, patient).await?);
    }
    Ok(Json(json!({
        "data": values,
        "meta": pagination::meta(page.page(), page.per_page(), total),
    })))
}

pub async fn store(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(payload): Json<Value>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let input = parse_input(&payload, None)?;
    validate_sponsor(&state, input.sponsor_id).await?;
    ensure_phone(&state, &input.phone, None, input.confirm_duplicate).await?;
    let mut transaction = state.pool.begin().await?;
    let result = sqlx::query(
        "INSERT INTO patients (name, phone, age, gender, area, emergency_phone, sponsor_id, is_active, created_by, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, NOW(), NOW())",
    )
    .bind(&input.name).bind(&input.phone).bind(input.age).bind(&input.gender)
    .bind(&input.area).bind(&input.emergency_phone).bind(input.sponsor_id)
    .bind(input.is_active).bind(auth.user.id)
    .execute(&mut *transaction).await?;
    let id = result.last_insert_id();
    let patient = fetch_tx(&mut transaction, id).await?;
    let values = patient_snapshot(&patient);
    audit::log(
        &mut transaction,
        auth.user.id,
        "patient_created",
        "patient",
        id,
        None,
        Some(&values),
        None,
    )
    .await?;
    transaction.commit().await?;
    let patient = fetch(&state, id).await?;
    Ok((
        StatusCode::CREATED,
        Json(json!({ "data": summary(&state, &patient).await? })),
    ))
}

pub async fn show(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<u64>,
) -> ApiResult<Json<Value>> {
    let patient = fetch(&state, id).await?;
    let mut value = summary(&state, &patient).await?;

    let sessions = sqlx::query_as::<_, SessionRow>(
        "SELECT * FROM sessions WHERE patient_id = ? ORDER BY opened_at DESC",
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await?;
    let mut session_values = Vec::new();
    for session in &sessions {
        session_values.push(loaders::session_for_patient_from_row(&state.pool, session).await?);
    }
    let subscriptions = sqlx::query_as::<_, SubscriptionRow>(
        "SELECT * FROM subscriptions WHERE patient_id = ? ORDER BY id DESC",
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await?;
    let mut subscription_values = Vec::new();
    for subscription in &subscriptions {
        subscription_values
            .push(loaders::subscription_for_patient_from_row(&state.pool, subscription).await?);
    }
    let movements = sqlx::query_as::<_, WalletTransactionRow>(
        "SELECT * FROM wallet_transactions WHERE patient_id = ? ORDER BY id DESC",
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await?;
    let debts =
        sqlx::query_as::<_, DebtRow>("SELECT * FROM debts WHERE patient_id = ? ORDER BY id DESC")
            .bind(id)
            .fetch_all(&state.pool)
            .await?;
    let payments = sqlx::query_as::<_, PaymentRow>(
        "SELECT * FROM payments WHERE patient_id = ? ORDER BY id DESC",
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await?;
    let map = value
        .as_object_mut()
        .expect("patient resource is an object");
    map.insert("sessions".into(), Value::Array(session_values));
    map.insert("subscriptions".into(), Value::Array(subscription_values));
    map.insert(
        "wallet_movements".into(),
        Value::Array(
            movements
                .iter()
                .map(resources::wallet_transaction_value)
                .collect(),
        ),
    );
    map.insert(
        "debts".into(),
        Value::Array(debts.iter().map(resources::debt_value).collect()),
    );
    map.insert(
        "payments".into(),
        Value::Array(
            payments
                .iter()
                .map(|row| resources::payment_value(row, None))
                .collect(),
        ),
    );
    Ok(Json(json!({ "data": value })))
}

pub async fn update(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<u64>,
    Json(payload): Json<Value>,
) -> ApiResult<Json<Value>> {
    let current = fetch(&state, id).await?;
    let input = parse_input(&payload, Some(&current))?;
    validate_sponsor(&state, input.sponsor_id).await?;
    ensure_phone(&state, &input.phone, Some(id), input.confirm_duplicate).await?;
    let old_values = patient_snapshot(&current);
    let mut transaction = state.pool.begin().await?;
    sqlx::query(
        "UPDATE patients SET name = ?, phone = ?, age = ?, gender = ?, area = ?, emergency_phone = ?, sponsor_id = ?, is_active = ?, updated_at = NOW() WHERE id = ?",
    )
    .bind(&input.name).bind(&input.phone).bind(input.age).bind(&input.gender)
    .bind(&input.area).bind(&input.emergency_phone).bind(input.sponsor_id)
    .bind(input.is_active).bind(id).execute(&mut *transaction).await?;
    let updated = fetch_tx(&mut transaction, id).await?;
    let new_values = patient_snapshot(&updated);
    audit::log(
        &mut transaction,
        auth.user.id,
        "patient_updated",
        "patient",
        id,
        Some(&old_values),
        Some(&new_values),
        None,
    )
    .await?;
    transaction.commit().await?;
    let updated = fetch(&state, id).await?;
    Ok(Json(json!({ "data": summary(&state, &updated).await? })))
}

pub async fn statement(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<u64>,
    Query(period): Query<PeriodQuery>,
) -> ApiResult<Json<Value>> {
    let patient = fetch(&state, id).await?;
    let movements = sqlx::query_as::<_, WalletTransactionRow>(
        "SELECT * FROM wallet_transactions WHERE patient_id = ? AND (? IS NULL OR DATE(created_at) >= ?) AND (? IS NULL OR DATE(created_at) <= ?) ORDER BY id",
    ).bind(id).bind(period.from).bind(period.from).bind(period.to).bind(period.to).fetch_all(&state.pool).await?;
    let debts = sqlx::query_as::<_, DebtRow>(
        "SELECT * FROM debts WHERE patient_id = ? AND (? IS NULL OR DATE(created_at) >= ?) AND (? IS NULL OR DATE(created_at) <= ?) ORDER BY id",
    ).bind(id).bind(period.from).bind(period.from).bind(period.to).bind(period.to).fetch_all(&state.pool).await?;
    let payments = sqlx::query_as::<_, PaymentRow>(
        "SELECT * FROM payments WHERE patient_id = ? AND (? IS NULL OR DATE(created_at) >= ?) AND (? IS NULL OR DATE(created_at) <= ?) ORDER BY id",
    ).bind(id).bind(period.from).bind(period.from).bind(period.to).bind(period.to).fetch_all(&state.pool).await?;
    let allocations = sqlx::query_as::<_, PaymentAllocationRow>(
        "SELECT pa.* FROM payment_allocations pa JOIN payments p ON p.id = pa.payment_id WHERE p.patient_id = ? AND (? IS NULL OR DATE(p.created_at) >= ?) AND (? IS NULL OR DATE(p.created_at) <= ?) ORDER BY pa.id",
    ).bind(id).bind(period.from).bind(period.from).bind(period.to).bind(period.to).fetch_all(&state.pool).await?;
    Ok(Json(json!({ "data": {
        "patient": loaders::patient_summary(&patient),
        "wallet_balance": money::normalize(loaders::wallet_balance(&state.pool, id).await?),
        "wallet_movements": movements.iter().map(resources::wallet_transaction_value).collect::<Vec<_>>(),
        "debts": debts.iter().map(resources::debt_value).collect::<Vec<_>>(),
        "payments": payments.iter().map(|row| resources::payment_value(row, None)).collect::<Vec<_>>(),
        "allocations": allocations.iter().map(resources::allocation_value).collect::<Vec<_>>(),
        "totals": {
            "debt_original": money::normalize(money::sum(debts.iter().map(|row| row.amount))),
            "debt_paid": money::normalize(money::sum(debts.iter().map(|row| row.paid_amount))),
        },
    }})))
}

struct PatientInput {
    name: String,
    phone: String,
    age: Option<u16>,
    gender: Option<String>,
    area: Option<String>,
    emergency_phone: Option<String>,
    sponsor_id: Option<u64>,
    is_active: bool,
    confirm_duplicate: bool,
}

fn parse_input(payload: &Value, current: Option<&PatientRow>) -> ApiResult<PatientInput> {
    let data = validation::object(payload)?;
    let name = validation::required_string(data, "name", 255, "اسم المريض مطلوب.")?;
    let phone = validation::required_string(data, "phone", 30, "رقم الهاتف مطلوب.")?;
    let age = match data.get("age") {
        None | Some(Value::Null) => None,
        Some(value) => Some(value.as_u64().filter(|age| *age <= 130).ok_or_else(|| {
            ApiError::validation("البيانات المرسلة غير صحيحة.")
                .field("age", "العمر يجب أن يكون بين 0 و130.")
        })? as u16),
    };
    let gender = validation::optional_string(data, "gender", 10)?;
    if let Some(value) = &gender {
        validation::one_of(value, &["male", "female"], "gender")?;
    }
    Ok(PatientInput {
        name,
        phone,
        age,
        gender,
        area: validation::optional_string(data, "area", 255)?,
        emergency_phone: validation::optional_string(data, "emergency_phone", 30)?,
        sponsor_id: validation::optional_u64(data, "sponsor_id")?,
        is_active: validation::boolean(data, "is_active", current.is_none_or(|row| row.is_active))?,
        confirm_duplicate: validation::boolean(data, "confirm_duplicate", false)?,
    })
}

async fn fetch(state: &AppState, id: u64) -> ApiResult<PatientRow> {
    sqlx::query_as::<_, PatientRow>("SELECT * FROM patients WHERE id = ?")
        .bind(id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(ApiError::not_found)
}

async fn fetch_tx(
    transaction: &mut sqlx::Transaction<'_, sqlx::MySql>,
    id: u64,
) -> ApiResult<PatientRow> {
    sqlx::query_as::<_, PatientRow>("SELECT * FROM patients WHERE id = ?")
        .bind(id)
        .fetch_one(&mut **transaction)
        .await
        .map_err(Into::into)
}

async fn summary(state: &AppState, patient: &PatientRow) -> ApiResult<Value> {
    let sponsor = match patient.sponsor_id {
        Some(id) => sqlx::query("SELECT id, name FROM sponsors WHERE id = ?").bind(id).fetch_optional(&state.pool).await?
            .map(|row| -> Result<Value, sqlx::Error> { Ok(json!({ "id": row.try_get::<u64,_>("id")?, "name": row.try_get::<String,_>("name")? })) }).transpose()?,
        None => None,
    };
    let active = loaders::has_usable_subscription(&state.pool, patient.id).await?;
    Ok(resources::patient_value(
        patient,
        sponsor,
        loaders::wallet_balance(&state.pool, patient.id).await?,
        loaders::debt_total(&state.pool, patient.id).await?,
        active.then_some("active"),
    ))
}

async fn ensure_phone(
    state: &AppState,
    phone: &str,
    except: Option<u64>,
    confirmed: bool,
) -> ApiResult<()> {
    let count = sqlx::query_scalar::<_, u64>(
        "SELECT CAST(COUNT(*) AS UNSIGNED) FROM patients WHERE phone = ? AND (? IS NULL OR id != ?)",
    )
    .bind(phone)
    .bind(except)
    .bind(except)
    .fetch_one(&state.pool)
    .await?;
    if count > 0 && !confirmed {
        return Err(ApiError::validation(
            "يوجد مريض مسجل برقم الهاتف نفسه. أكد الحفظ إذا كان هذا سجلًا مختلفًا.",
        )
        .field("phone", "رقم الهاتف مستخدم في سجل آخر.")
        .field("confirm_duplicate", "يلزم تأكيد إنشاء سجل مكرر."));
    }
    Ok(())
}

async fn validate_sponsor(state: &AppState, sponsor_id: Option<u64>) -> ApiResult<()> {
    if let Some(id) = sponsor_id {
        let count = sqlx::query_scalar::<_, u64>(
            "SELECT CAST(COUNT(*) AS UNSIGNED) FROM sponsors WHERE id = ? AND is_active = 1",
        )
        .bind(id)
        .fetch_one(&state.pool)
        .await?;
        if count == 0 {
            return Err(ApiError::validation("الجهة المحددة غير متاحة.")
                .field("sponsor_id", "الجهة المحددة غير موجودة أو غير نشطة."));
        }
    }
    Ok(())
}

fn patient_snapshot(patient: &PatientRow) -> Value {
    json!({
        "id": patient.id, "name": patient.name, "phone": patient.phone, "age": patient.age,
        "gender": patient.gender, "area": patient.area, "emergency_phone": patient.emergency_phone,
        "sponsor_id": patient.sponsor_id, "is_active": patient.is_active, "created_by": patient.created_by,
    })
}
