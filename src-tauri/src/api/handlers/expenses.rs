use crate::api::{
    audit,
    auth::AuthUser,
    error::{ApiError, ApiResult},
    money, pagination, resources,
    rows::AdministrativeExpenseRow,
    time, validation, AppState,
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
use sqlx::{MySql, Transaction};

const RENT_RECIPIENT: &str = "صاحب العقار";

#[derive(Debug, Deserialize)]
pub struct ExpenseListQuery {
    search: Option<String>,
    #[serde(default, deserialize_with = "crate::api::query::optional")]
    from: Option<NaiveDate>,
    #[serde(default, deserialize_with = "crate::api::query::optional")]
    to: Option<NaiveDate>,
    r#type: Option<String>,
    method: Option<String>,
    status: Option<String>,
    #[serde(default, deserialize_with = "crate::api::query::optional")]
    page: Option<u64>,
    #[serde(default, deserialize_with = "crate::api::query::optional")]
    per_page: Option<u64>,
}

pub async fn index(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(query): Query<ExpenseListQuery>,
) -> ApiResult<Json<Value>> {
    auth.require_admin()?;
    validate_filters(
        query.from,
        query.to,
        query.r#type.as_deref(),
        query.method.as_deref(),
        query.status.as_deref(),
    )?;

    let page = pagination::PageQuery {
        page: query.page,
        per_page: query.per_page,
    };
    let search = query.search.as_deref().unwrap_or("").trim();
    let pattern = format!("%{search}%");
    let expense_type = query.r#type.as_deref().unwrap_or("").trim();
    let method = query.method.as_deref().unwrap_or("").trim();
    let status = query.status.as_deref().unwrap_or("").trim();
    let filters = "FROM administrative_expenses \
        WHERE (? = '' OR description LIKE ? OR recipient LIKE ? OR COALESCE(reference_number, '') LIKE ?) \
          AND (? IS NULL OR payment_date >= ?) \
          AND (? IS NULL OR payment_date <= ?) \
          AND (? = '' OR `type` = ?) \
          AND (? = '' OR payment_method = ?) \
          AND (? = '' OR status = ?)";

    let count_sql = format!("SELECT CAST(COUNT(*) AS UNSIGNED) {filters}");
    let total = sqlx::query_scalar::<_, u64>(&count_sql)
        .bind(search)
        .bind(&pattern)
        .bind(&pattern)
        .bind(&pattern)
        .bind(query.from)
        .bind(query.from)
        .bind(query.to)
        .bind(query.to)
        .bind(expense_type)
        .bind(expense_type)
        .bind(method)
        .bind(method)
        .bind(status)
        .bind(status)
        .fetch_one(&state.pool)
        .await?;

    let select_sql =
        format!("SELECT * {filters} ORDER BY payment_date DESC, id DESC LIMIT ? OFFSET ?");
    let rows = sqlx::query_as::<_, AdministrativeExpenseRow>(&select_sql)
        .bind(search)
        .bind(&pattern)
        .bind(&pattern)
        .bind(&pattern)
        .bind(query.from)
        .bind(query.from)
        .bind(query.to)
        .bind(query.to)
        .bind(expense_type)
        .bind(expense_type)
        .bind(method)
        .bind(method)
        .bind(status)
        .bind(status)
        .bind(page.per_page())
        .bind(page.offset())
        .fetch_all(&state.pool)
        .await?;

    let summary_sql = format!(
        "SELECT \
            COALESCE(SUM(CASE WHEN `type` = 'purchase' THEN amount ELSE 0 END), 0), \
            COALESCE(SUM(CASE WHEN `type` = 'rent' THEN amount ELSE 0 END), 0) \
         {filters} AND status = 'active'"
    );
    let (total_purchases, total_rent) = sqlx::query_as::<_, (Decimal, Decimal)>(&summary_sql)
        .bind(search)
        .bind(&pattern)
        .bind(&pattern)
        .bind(&pattern)
        .bind(query.from)
        .bind(query.from)
        .bind(query.to)
        .bind(query.to)
        .bind(expense_type)
        .bind(expense_type)
        .bind(method)
        .bind(method)
        .bind(status)
        .bind(status)
        .fetch_one(&state.pool)
        .await?;

    Ok(Json(json!({
        "data": rows.iter().map(resources::administrative_expense_value).collect::<Vec<_>>(),
        "meta": pagination::meta(page.page(), page.per_page(), total),
        "summary": {
            "total_purchases": money::normalize(total_purchases),
            "total_rent": money::normalize(total_rent),
            "total_expenses": money::normalize(total_purchases + total_rent),
        },
    })))
}

pub async fn store(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(payload): Json<Value>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    auth.require_admin()?;
    let input = parse_input(&payload)?;
    let mut transaction = state.pool.begin().await?;
    let result = sqlx::query(
        "INSERT INTO administrative_expenses \
         (`type`, description, recipient, payment_date, amount, payment_method, reference_number, rent_period, \
          rent_month, rent_start_date, rent_end_date, notes, \
          status, created_by, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, NULL, ?, ?, ?, ?, ?, 'active', ?, NOW(), NOW())",
    )
    .bind(&input.expense_type)
    .bind(&input.description)
    .bind(&input.recipient)
    .bind(input.payment_date)
    .bind(input.amount)
    .bind(&input.payment_method)
    .bind(&input.rent_period)
    .bind(&input.rent_month)
    .bind(input.rent_start_date)
    .bind(input.rent_end_date)
    .bind(&input.notes)
    .bind(auth.user.id)
    .execute(&mut *transaction)
    .await?;
    let id = result.last_insert_id();
    let reference_number = generate_reference_number(&mut transaction, id).await?;
    sqlx::query("UPDATE administrative_expenses SET reference_number = ? WHERE id = ?")
        .bind(&reference_number)
        .bind(id)
        .execute(&mut *transaction)
        .await?;
    let expense = fetch_for_update(&mut transaction, id).await?;
    let values = expense_snapshot(&expense);
    audit::log(
        &mut transaction,
        auth.user.id,
        "expense_created",
        "expense",
        id,
        None,
        Some(&values),
        None,
    )
    .await?;
    transaction.commit().await?;

    Ok((
        StatusCode::CREATED,
        Json(json!({ "data": resources::administrative_expense_value(&expense) })),
    ))
}

pub async fn correct(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<u64>,
    Json(payload): Json<Value>,
) -> ApiResult<Json<Value>> {
    auth.require_admin()?;
    let data = validation::object(&payload)?;
    let reason = validation::required_string(data, "reason", 1000, "سبب التصحيح مطلوب.")?;
    let input = parse_input(&payload)?;
    let mut transaction = state.pool.begin().await?;
    let current = fetch_for_update(&mut transaction, id).await?;
    if current.status == "cancelled" {
        return Err(ApiError::validation("لا يمكن تصحيح مصروف ملغي."));
    }
    if input.matches(&current) {
        return Err(ApiError::validation("لم يتم إدخال أي تغيير على المصروف."));
    }
    let old_values = expense_snapshot(&current);
    sqlx::query(
        "UPDATE administrative_expenses SET `type` = ?, description = ?, recipient = ?, payment_date = ?, \
         amount = ?, payment_method = ?, rent_period = ?, rent_month = ?, rent_start_date = ?, \
         rent_end_date = ?, notes = ?, updated_at = NOW() \
         WHERE id = ?",
    )
    .bind(&input.expense_type)
    .bind(&input.description)
    .bind(&input.recipient)
    .bind(input.payment_date)
    .bind(input.amount)
    .bind(&input.payment_method)
    .bind(&input.rent_period)
    .bind(&input.rent_month)
    .bind(input.rent_start_date)
    .bind(input.rent_end_date)
    .bind(&input.notes)
    .bind(id)
    .execute(&mut *transaction)
    .await?;
    let expense = fetch_for_update(&mut transaction, id).await?;
    let new_values = expense_snapshot(&expense);
    audit::log(
        &mut transaction,
        auth.user.id,
        "expense_corrected",
        "expense",
        id,
        Some(&old_values),
        Some(&new_values),
        Some(&reason),
    )
    .await?;
    transaction.commit().await?;

    Ok(Json(
        json!({ "data": resources::administrative_expense_value(&expense) }),
    ))
}

pub async fn cancel(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<u64>,
    Json(payload): Json<Value>,
) -> ApiResult<Json<Value>> {
    auth.require_admin()?;
    let data = validation::object(&payload)?;
    let reason = validation::required_string(data, "reason", 1000, "سبب الإلغاء مطلوب.")?;
    let mut transaction = state.pool.begin().await?;
    let current = fetch_for_update(&mut transaction, id).await?;
    if current.status == "cancelled" {
        return Err(ApiError::validation("هذا المصروف ملغي بالفعل."));
    }
    let old_values = expense_snapshot(&current);
    sqlx::query(
        "UPDATE administrative_expenses SET status = 'cancelled', cancelled_at = NOW(), cancelled_by = ?, \
         cancel_reason = ?, updated_at = NOW() WHERE id = ?",
    )
    .bind(auth.user.id)
    .bind(&reason)
    .bind(id)
    .execute(&mut *transaction)
    .await?;
    let expense = fetch_for_update(&mut transaction, id).await?;
    let new_values = expense_snapshot(&expense);
    audit::log(
        &mut transaction,
        auth.user.id,
        "expense_cancelled",
        "expense",
        id,
        Some(&old_values),
        Some(&new_values),
        Some(&reason),
    )
    .await?;
    transaction.commit().await?;

    Ok(Json(
        json!({ "data": resources::administrative_expense_value(&expense) }),
    ))
}

#[derive(Debug, Clone)]
struct ExpenseInput {
    expense_type: String,
    description: String,
    recipient: String,
    payment_date: NaiveDate,
    amount: Decimal,
    payment_method: String,
    rent_period: Option<String>,
    rent_month: Option<String>,
    rent_start_date: Option<NaiveDate>,
    rent_end_date: Option<NaiveDate>,
    notes: Option<String>,
}

impl ExpenseInput {
    fn matches(&self, current: &AdministrativeExpenseRow) -> bool {
        self.expense_type == current.r#type
            && self.description == current.description
            && self.recipient == current.recipient
            && self.payment_date == current.payment_date
            && self.amount == current.amount
            && self.payment_method == current.payment_method
            && self.rent_period == current.rent_period
            && self.rent_month == current.rent_month
            && self.rent_start_date == current.rent_start_date
            && self.rent_end_date == current.rent_end_date
            && self.notes == current.notes
    }
}

fn parse_input(payload: &Value) -> ApiResult<ExpenseInput> {
    let data = validation::object(payload)?;
    let expense_type = validation::required_string(data, "type", 20, "نوع المصروف مطلوب.")?;
    validation::one_of(&expense_type, &["purchase", "rent"], "type")?;
    let payment_method =
        validation::required_string(data, "payment_method", 20, "طريقة الدفع مطلوبة.")?;
    validation::one_of(&payment_method, &["cash", "transfer"], "payment_method")?;
    let payment_date = validation::required_date(data, "payment_date")?;
    if payment_date > time::today() {
        return Err(ApiError::validation("لا يمكن تسجيل مصروف لم يُدفع بعد.")
            .field("payment_date", "تاريخ الدفع لا يمكن أن يكون في المستقبل."));
    }
    let (recipient, rent_period, rent_month, rent_start_date, rent_end_date) =
        if expense_type == "rent" {
            let rent_month = required_rent_month(data)?;
            let rent_start_date = validation::required_date(data, "rent_start_date")?;
            let rent_end_date = validation::required_date(data, "rent_end_date")?;
            if rent_end_date < rent_start_date {
                return Err(ApiError::validation("فترة الإيجار غير صحيحة.").field(
                    "rent_end_date",
                    "تاريخ نهاية الإيجار يجب ألا يسبق تاريخ البداية.",
                ));
            }
            (
                RENT_RECIPIENT.to_owned(),
                Some(format!(
                    "{} إلى {}",
                    rent_start_date.format("%Y-%m-%d"),
                    rent_end_date.format("%Y-%m-%d")
                )),
                Some(rent_month),
                Some(rent_start_date),
                Some(rent_end_date),
            )
        } else {
            (
                validation::required_string(data, "recipient", 255, "اسم المورد مطلوب.")?,
                None,
                None,
                None,
                None,
            )
        };

    Ok(ExpenseInput {
        expense_type,
        description: validation::required_string(data, "description", 255, "وصف المصروف مطلوب.")?,
        recipient,
        payment_date,
        amount: validation::required_decimal(data, "amount", true)?,
        payment_method,
        rent_period,
        rent_month,
        rent_start_date,
        rent_end_date,
        notes: validation::optional_string(data, "notes", 2000)?,
    })
}

fn required_rent_month(data: &serde_json::Map<String, Value>) -> ApiResult<String> {
    let rent_month = validation::required_string(data, "rent_month", 7, "شهر الإيجار مطلوب.")?;
    let valid = rent_month.len() == 7
        && NaiveDate::parse_from_str(&format!("{rent_month}-01"), "%Y-%m-%d").is_ok();
    if !valid {
        return Err(ApiError::validation("شهر الإيجار غير صحيح.")
            .field("rent_month", "اختر شهر إيجار صحيحًا."));
    }
    Ok(rent_month)
}

pub(crate) fn validate_filters(
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
    expense_type: Option<&str>,
    method: Option<&str>,
    status: Option<&str>,
) -> ApiResult<()> {
    if from.zip(to).is_some_and(|(from, to)| from > to) {
        return Err(
            ApiError::validation("تاريخ البداية يجب أن يسبق تاريخ النهاية.")
                .field("from", "الفترة الزمنية غير صحيحة."),
        );
    }
    if let Some(value) = expense_type.filter(|value| !value.trim().is_empty()) {
        validation::one_of(value, &["purchase", "rent"], "type")?;
    }
    if let Some(value) = method.filter(|value| !value.trim().is_empty()) {
        validation::one_of(value, &["cash", "transfer"], "method")?;
    }
    if let Some(value) = status.filter(|value| !value.trim().is_empty()) {
        validation::one_of(value, &["active", "cancelled"], "status")?;
    }
    Ok(())
}

async fn fetch_for_update(
    transaction: &mut Transaction<'_, MySql>,
    id: u64,
) -> ApiResult<AdministrativeExpenseRow> {
    sqlx::query_as::<_, AdministrativeExpenseRow>(
        "SELECT * FROM administrative_expenses WHERE id = ? FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or_else(ApiError::not_found)
}

async fn generate_reference_number(
    transaction: &mut Transaction<'_, MySql>,
    id: u64,
) -> ApiResult<String> {
    let base = format!("EXP-{id:08}");
    let mut suffix = 1_u64;
    loop {
        let candidate = if suffix == 1 {
            base.clone()
        } else {
            format!("{base}-{suffix}")
        };
        let matches = sqlx::query_scalar::<_, u64>(
            "SELECT CAST(COUNT(*) AS UNSIGNED) FROM administrative_expenses \
             WHERE reference_number = ? AND id <> ?",
        )
        .bind(&candidate)
        .bind(id)
        .fetch_one(&mut **transaction)
        .await?;
        if matches == 0 {
            return Ok(candidate);
        }
        suffix += 1;
    }
}

pub(crate) fn expense_snapshot(expense: &AdministrativeExpenseRow) -> Value {
    json!({
        "id": expense.id,
        "type": expense.r#type,
        "description": expense.description,
        "recipient": expense.recipient,
        "payment_date": expense.payment_date.format("%Y-%m-%d").to_string(),
        "amount": money::normalize(expense.amount),
        "payment_method": expense.payment_method,
        "reference_number": expense.reference_number,
        "rent_period": expense.rent_period,
        "rent_month": expense.rent_month,
        "rent_start_date": expense.rent_start_date.map(|date| date.format("%Y-%m-%d").to_string()),
        "rent_end_date": expense.rent_end_date.map(|date| date.format("%Y-%m-%d").to_string()),
        "notes": expense.notes,
        "status": expense.status,
        "cancelled_at": expense.cancelled_at.map(time::iso),
        "cancelled_by": expense.cancelled_by,
        "cancel_reason": expense.cancel_reason,
        "created_by": expense.created_by,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_payload() -> Value {
        json!({
            "type": "purchase",
            "description": "مواد تنظيف",
            "recipient": "شركة التوريد",
            "payment_date": time::today().format("%Y-%m-%d").to_string(),
            "amount": "125.500",
            "payment_method": "cash",
            "reference_number": "INV-10",
            "notes": "تم الدفع بالكامل"
        })
    }

    #[test]
    fn purchase_does_not_keep_rental_fields() {
        let mut payload = base_payload();
        payload["rent_period"] = json!("2026-11");
        payload["rent_month"] = json!("2026-11");
        payload["rent_start_date"] = json!("2026-11-01");
        payload["rent_end_date"] = json!("2026-11-30");
        let parsed = parse_input(&payload).expect("valid purchase");
        assert_eq!(parsed.expense_type, "purchase");
        assert_eq!(parsed.rent_period, None);
        assert_eq!(parsed.rent_month, None);
        assert_eq!(parsed.rent_start_date, None);
        assert_eq!(parsed.rent_end_date, None);
    }

    #[test]
    fn rent_requires_structured_dates_and_uses_internal_recipient() {
        let mut payload = base_payload();
        payload["type"] = json!("rent");
        let error = parse_input(&payload).expect_err("rent month must be required");
        assert!(error.errors.contains_key("rent_month"));

        payload["rent_month"] = json!("2026-11");
        payload["rent_start_date"] = json!("2026-11-30");
        payload["rent_end_date"] = json!("2026-11-01");
        let error = parse_input(&payload).expect_err("end date must follow start date");
        assert!(error.errors.contains_key("rent_end_date"));

        payload["rent_start_date"] = json!("2026-11-01");
        payload["rent_end_date"] = json!("2026-11-30");
        let parsed = parse_input(&payload).expect("valid rent");
        assert_eq!(parsed.recipient, RENT_RECIPIENT);
        assert_eq!(parsed.rent_month.as_deref(), Some("2026-11"));
        assert_eq!(
            parsed.rent_period.as_deref(),
            Some("2026-11-01 إلى 2026-11-30")
        );
    }

    #[test]
    fn future_payments_are_rejected() {
        let mut payload = base_payload();
        payload["payment_date"] = json!("2999-01-01");
        let error = parse_input(&payload).expect_err("future payment must be rejected");
        assert!(error.errors.contains_key("payment_date"));
    }
}
