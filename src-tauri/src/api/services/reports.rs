use crate::api::{
    error::{ApiError, ApiResult},
    loaders, money, resources,
    rows::{
        AdministrativeExpenseRow, DebtRow, DoctorRow, PatientRow, PaymentRow, SessionRow,
        SessionServiceRow, SponsorRow, SubscriptionRow,
    },
    time, validation, AppState,
};
use axum::http::StatusCode;
use chrono::{NaiveDate, Utc};
use rust_decimal::Decimal;
use serde_json::{json, Value};

#[derive(Debug, Clone, Default)]
pub struct ReportFilters {
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
    pub doctor_id: Option<u64>,
    pub patient_id: Option<u64>,
    pub sponsor_id: Option<u64>,
    pub status: Option<String>,
    pub kind: Option<String>,
    pub r#type: Option<String>,
    pub method: Option<String>,
}

pub async fn report(
    state: &AppState,
    report_type: &str,
    filters: &ReportFilters,
) -> ApiResult<Value> {
    let mut report = match report_type {
        "sessions" => sessions(state, filters).await?,
        "collections" => collections(state, filters).await?,
        "debts" => debts(state, filters).await?,
        "doctor-dues" => doctor_dues(state, filters).await?,
        "subscriptions" => subscriptions(state, filters).await?,
        "expenses" => expenses(state, filters).await?,
        _ => {
            return Err(ApiError::new(
                StatusCode::NOT_FOUND,
                "تعذر العثور على التقرير المطلوب.",
            ));
        }
    };
    report
        .as_object_mut()
        .expect("report must be an object")
        .insert(
            "meta".into(),
            json!({
                "type": report_type,
                "from": filters.from.map(|date| date.format("%Y-%m-%d").to_string()),
                "to": filters.to.map(|date| date.format("%Y-%m-%d").to_string()),
                "generated_at": time::iso(Utc::now()),
            }),
        );
    Ok(report)
}

async fn expenses(state: &AppState, filters: &ReportFilters) -> ApiResult<Value> {
    if let Some(value) = filters.r#type.as_deref() {
        validation::one_of(value, &["purchase", "rent"], "type")?;
    }
    if let Some(value) = filters.method.as_deref() {
        validation::one_of(value, &["cash", "transfer"], "method")?;
    }
    if let Some(value) = filters.status.as_deref() {
        validation::one_of(value, &["active", "cancelled"], "status")?;
    }

    let rows = sqlx::query_as::<_, AdministrativeExpenseRow>(
        "SELECT * FROM administrative_expenses \
         WHERE (? IS NULL OR payment_date >= ?) \
           AND (? IS NULL OR payment_date <= ?) \
           AND (? IS NULL OR `type` = ?) \
           AND (? IS NULL OR payment_method = ?) \
           AND (? IS NULL OR status = ?) \
         ORDER BY payment_date DESC, id DESC",
    )
    .bind(filters.from)
    .bind(filters.from)
    .bind(filters.to)
    .bind(filters.to)
    .bind(&filters.r#type)
    .bind(&filters.r#type)
    .bind(&filters.method)
    .bind(&filters.method)
    .bind(&filters.status)
    .bind(&filters.status)
    .fetch_all(&state.pool)
    .await?;

    let mut count = 0usize;
    let mut total_purchases = Decimal::ZERO;
    let mut total_rent = Decimal::ZERO;
    for expense in &rows {
        if expense.status != "active" {
            continue;
        }
        count += 1;
        match expense.r#type.as_str() {
            "purchase" => total_purchases += expense.amount,
            "rent" => total_rent += expense.amount,
            _ => {}
        }
    }

    Ok(json!({
        "rows": rows.iter().map(resources::administrative_expense_value).collect::<Vec<_>>(),
        "totals": {
            "count": count,
            "total_purchases": money::normalize(total_purchases),
            "total_rent": money::normalize(total_rent),
            "total_expenses": money::normalize(total_purchases + total_rent),
        },
    }))
}

async fn sessions(state: &AppState, filters: &ReportFilters) -> ApiResult<Value> {
    let status = filters.status.as_deref();
    let rows = sqlx::query_as::<_, SessionRow>(
        "SELECT * FROM sessions \
         WHERE (? IS NULL OR DATE(opened_at) >= ?) \
           AND (? IS NULL OR DATE(opened_at) <= ?) \
           AND (? IS NULL OR doctor_id = ?) \
           AND (? IS NULL OR patient_id = ?) \
           AND (? IS NULL OR status = ?) \
         ORDER BY opened_at DESC, id DESC",
    )
    .bind(filters.from)
    .bind(filters.from)
    .bind(filters.to)
    .bind(filters.to)
    .bind(filters.doctor_id)
    .bind(filters.doctor_id)
    .bind(filters.patient_id)
    .bind(filters.patient_id)
    .bind(status)
    .bind(status)
    .fetch_all(&state.pool)
    .await?;
    let mut values = Vec::with_capacity(rows.len());
    let mut service_value = Decimal::ZERO;
    for session in &rows {
        if session.status == "closed" && session.r#type == "individual" {
            service_value += session.net_total;
        }
        let patient = patient(state, session.patient_id).await?;
        let doctor = doctor(state, session.doctor_id).await?;
        let services = session_services(state, session.id).await?;
        values.push(json!({
            "id": session.id,
            "code": session.code,
            "patient_id": session.patient_id,
            "patient": loaders::patient_summary(&patient),
            "doctor_id": session.doctor_id,
            "doctor": { "id": doctor.id, "name": doctor.name },
            "services": services.iter().map(|service| json!({
                "service_id": service.service_id,
                "name": service.service_name,
                "price": money::normalize(service.price),
            })).collect::<Vec<_>>(),
            "type": session.r#type,
            "status": session.status,
            "opened_at": time::iso(session.opened_at),
            "closed_at": session.closed_at.map(time::iso),
            "net_total": money::normalize(session.net_total),
        }));
    }
    Ok(json!({
        "rows": values,
        "totals": {
            "count": rows.len(),
            "service_value": money::normalize(service_value),
        },
    }))
}

async fn collections(state: &AppState, filters: &ReportFilters) -> ApiResult<Value> {
    let payment_type = filters.r#type.as_deref();
    let method = filters.method.as_deref();
    let rows = sqlx::query_as::<_, PaymentRow>(
        "SELECT p.* FROM payments p \
         WHERE (? IS NULL OR DATE(p.created_at) >= ?) \
           AND (? IS NULL OR DATE(p.created_at) <= ?) \
           AND (? IS NULL OR p.patient_id = ?) \
           AND (? IS NULL OR p.type = ?) \
           AND (? IS NULL OR p.method = ?) \
           AND (? IS NULL OR \
                EXISTS (SELECT 1 FROM payment_allocations pa INNER JOIN debts d ON d.id = pa.debt_id WHERE pa.payment_id = p.id AND d.sponsor_id = ?) \
                OR EXISTS (SELECT 1 FROM sessions s WHERE s.id = p.session_id AND s.sponsor_id = ?) \
                OR EXISTS (SELECT 1 FROM payments original INNER JOIN payment_allocations pa ON pa.payment_id = original.id INNER JOIN debts d ON d.id = pa.debt_id WHERE original.id = p.reverses_payment_id AND d.sponsor_id = ?) \
                OR EXISTS (SELECT 1 FROM payments original INNER JOIN sessions s ON s.id = original.session_id WHERE original.id = p.reverses_payment_id AND s.sponsor_id = ?)) \
         ORDER BY p.id DESC",
    )
    .bind(filters.from)
    .bind(filters.from)
    .bind(filters.to)
    .bind(filters.to)
    .bind(filters.patient_id)
    .bind(filters.patient_id)
    .bind(payment_type)
    .bind(payment_type)
    .bind(method)
    .bind(method)
    .bind(filters.sponsor_id)
    .bind(filters.sponsor_id)
    .bind(filters.sponsor_id)
    .bind(filters.sponsor_id)
    .bind(filters.sponsor_id)
    .fetch_all(&state.pool)
    .await?;
    let mut values = Vec::with_capacity(rows.len());
    let mut received = Decimal::ZERO;
    let mut refunds = Decimal::ZERO;
    for payment in &rows {
        if payment.r#type == "refund" {
            refunds += payment.amount.abs();
        } else {
            received += payment.amount;
        }
        let patient = patient(state, payment.patient_id).await?;
        values.push(json!({
            "id": payment.id,
            "patient_id": payment.patient_id,
            "patient": loaders::patient_summary(&patient),
            "session_id": payment.session_id,
            "subscription_id": payment.subscription_id,
            "type": payment.r#type,
            "method": payment.method,
            "amount": money::normalize(payment.amount),
            "created_at": payment.created_at.map(time::iso),
        }));
    }
    Ok(json!({
        "rows": values,
        "totals": {
            "received": money::normalize(received),
            "refunds": money::normalize(refunds),
            "net": money::normalize(received - refunds),
        },
    }))
}

async fn debts(state: &AppState, filters: &ReportFilters) -> ApiResult<Value> {
    let status = filters.status.as_deref();
    let kind = filters.kind.as_deref();
    let rows = sqlx::query_as::<_, DebtRow>(
        "SELECT * FROM debts \
         WHERE (? IS NULL OR DATE(created_at) >= ?) \
           AND (? IS NULL OR DATE(created_at) <= ?) \
           AND (? IS NULL OR patient_id = ?) \
           AND (? IS NULL OR sponsor_id = ?) \
           AND (? IS NULL OR status = ?) \
           AND (? IS NULL OR kind = ?) \
         ORDER BY id DESC",
    )
    .bind(filters.from)
    .bind(filters.from)
    .bind(filters.to)
    .bind(filters.to)
    .bind(filters.patient_id)
    .bind(filters.patient_id)
    .bind(filters.sponsor_id)
    .bind(filters.sponsor_id)
    .bind(status)
    .bind(status)
    .bind(kind)
    .bind(kind)
    .fetch_all(&state.pool)
    .await?;
    let mut values = Vec::with_capacity(rows.len());
    let mut original = Decimal::ZERO;
    let mut paid = Decimal::ZERO;
    let mut remaining = Decimal::ZERO;
    for debt in &rows {
        original += debt.amount;
        paid += debt.paid_amount;
        let debt_remaining = if debt.status == "void" {
            Decimal::ZERO
        } else {
            debt.amount - debt.paid_amount
        };
        remaining += debt_remaining;
        let patient = patient(state, debt.patient_id).await?;
        let sponsor = match debt.sponsor_id {
            Some(id) => Some(sponsor(state, id).await?),
            None => None,
        };
        values.push(json!({
            "id": debt.id,
            "patient_id": debt.patient_id,
            "patient": loaders::patient_summary(&patient),
            "sponsor_id": debt.sponsor_id,
            "sponsor": sponsor.map(|row| json!({ "id": row.id, "name": row.name })),
            "kind": debt.kind,
            "status": debt.status,
            "amount": money::normalize(debt.amount),
            "paid_amount": money::normalize(debt.paid_amount),
            "remaining": money::normalize(debt_remaining),
            "created_at": debt.created_at.map(time::iso),
        }));
    }
    Ok(json!({
        "rows": values,
        "totals": {
            "original": money::normalize(original),
            "paid": money::normalize(paid),
            "remaining": money::normalize(remaining),
        },
    }))
}

async fn doctor_dues(state: &AppState, filters: &ReportFilters) -> ApiResult<Value> {
    let doctors = sqlx::query_as::<_, DoctorRow>(
        "SELECT * FROM doctors WHERE (? IS NULL OR id = ?) ORDER BY name",
    )
    .bind(filters.doctor_id)
    .bind(filters.doctor_id)
    .fetch_all(&state.pool)
    .await?;
    let mut values = Vec::with_capacity(doctors.len());
    let mut total_earned = Decimal::ZERO;
    let mut total_paid = Decimal::ZERO;
    for doctor in &doctors {
        let sessions_count = sqlx::query_scalar::<_, u64>(
            "SELECT CAST(COUNT(*) AS UNSIGNED) FROM sessions \
             WHERE doctor_id = ? AND type = 'individual' AND status = 'closed' AND commission_amount IS NOT NULL \
               AND (? IS NULL OR DATE(closed_at) >= ?) \
               AND (? IS NULL OR DATE(closed_at) <= ?)",
        )
        .bind(doctor.id)
        .bind(filters.from)
        .bind(filters.from)
        .bind(filters.to)
        .bind(filters.to)
        .fetch_one(&state.pool)
        .await?;
        let earned = sqlx::query_scalar::<_, Option<Decimal>>(
            "SELECT SUM(commission_amount) FROM sessions \
             WHERE doctor_id = ? AND type = 'individual' AND status = 'closed' AND commission_amount IS NOT NULL \
               AND (? IS NULL OR DATE(closed_at) >= ?) \
               AND (? IS NULL OR DATE(closed_at) <= ?)",
        )
        .bind(doctor.id)
        .bind(filters.from)
        .bind(filters.from)
        .bind(filters.to)
        .bind(filters.to)
        .fetch_one(&state.pool)
        .await?
        .unwrap_or(Decimal::ZERO);
        let paid = sqlx::query_scalar::<_, Option<Decimal>>(
            "SELECT SUM(amount) FROM doctor_payouts \
             WHERE doctor_id = ? \
               AND (? IS NULL OR date >= ?) \
               AND (? IS NULL OR date <= ?)",
        )
        .bind(doctor.id)
        .bind(filters.from)
        .bind(filters.from)
        .bind(filters.to)
        .bind(filters.to)
        .fetch_one(&state.pool)
        .await?
        .unwrap_or(Decimal::ZERO);
        total_earned += earned;
        total_paid += paid;
        values.push(json!({
            "id": doctor.id,
            "name": doctor.name,
            "phone": doctor.phone,
            "commission_rate": money::normalize(doctor.commission_rate),
            "is_active": doctor.is_active,
            "sessions_count": sessions_count,
            "earned": money::normalize(earned),
            "paid": money::normalize(paid),
            "remaining": money::normalize(earned - paid),
        }));
    }
    Ok(json!({
        "rows": values,
        "totals": {
            "earned": money::normalize(total_earned),
            "paid": money::normalize(total_paid),
            "remaining": money::normalize(total_earned - total_paid),
        },
    }))
}

async fn subscriptions(state: &AppState, filters: &ReportFilters) -> ApiResult<Value> {
    let status = filters.status.as_deref();
    let rows = sqlx::query_as::<_, SubscriptionRow>(
        "SELECT sub.* FROM subscriptions sub \
         WHERE (? IS NULL OR sub.start_date >= ?) \
           AND (? IS NULL OR sub.start_date <= ?) \
           AND (? IS NULL OR sub.patient_id = ?) \
           AND (? IS NULL OR EXISTS ( \
               SELECT 1 FROM sessions supervised \
               WHERE supervised.subscription_id = sub.id \
                 AND supervised.type = 'subscription' \
                 AND supervised.status IN ('closed','cancelled') \
                 AND supervised.doctor_id = ? \
                 AND (? IS NULL OR DATE(supervised.opened_at) >= ?) \
                 AND (? IS NULL OR DATE(supervised.opened_at) <= ?) \
           )) \
           AND (? IS NULL OR sub.status = ?) \
         ORDER BY sub.id DESC",
    )
    .bind(filters.from)
    .bind(filters.from)
    .bind(filters.to)
    .bind(filters.to)
    .bind(filters.patient_id)
    .bind(filters.patient_id)
    .bind(filters.doctor_id)
    .bind(filters.doctor_id)
    .bind(filters.from)
    .bind(filters.from)
    .bind(filters.to)
    .bind(filters.to)
    .bind(status)
    .bind(status)
    .fetch_all(&state.pool)
    .await?;
    let mut values = Vec::with_capacity(rows.len());
    let mut active = 0usize;
    let mut completed = 0usize;
    let mut cancelled = 0usize;
    let mut total_sessions = 0u64;
    let mut used_sessions = 0u64;
    let mut completed_sessions = 0u64;
    let mut remaining_sessions = 0u64;
    let mut total_value = Decimal::ZERO;
    let mut received_amount = Decimal::ZERO;
    let mut refunded_amount = Decimal::ZERO;
    let mut outstanding_amount = Decimal::ZERO;
    for subscription in &rows {
        match subscription.status.as_str() {
            "active" => active += 1,
            "completed" => completed += 1,
            "cancelled" => cancelled += 1,
            _ => {}
        }
        let remaining = subscription
            .sessions_count
            .saturating_sub(subscription.sessions_used);
        total_sessions += u64::from(subscription.sessions_count);
        used_sessions += u64::from(subscription.sessions_used);
        remaining_sessions += u64::from(remaining);
        total_value += subscription.total_price;
        let (received, refunded) = subscription_financials(state, subscription.id).await?;
        let outstanding = if subscription.status == "cancelled" {
            Decimal::ZERO
        } else {
            (subscription.total_price - received).max(Decimal::ZERO)
        };
        received_amount += received;
        refunded_amount += refunded;
        outstanding_amount += outstanding;
        let supervised_sessions =
            subscription_supervised_sessions(state, subscription.id, filters).await?;
        completed_sessions += supervised_sessions
            .iter()
            .filter(|session| session.get("status").and_then(Value::as_str) == Some("closed"))
            .count() as u64;
        let mut value = loaders::subscription_from_row(&state.pool, subscription).await?;
        value
            .as_object_mut()
            .expect("subscription resource must be an object")
            .insert(
                "supervised_sessions".into(),
                Value::Array(supervised_sessions),
            );
        values.push(value);
    }
    Ok(json!({
        "rows": values,
        "totals": {
            "count": rows.len(),
            "active": active,
            "completed": completed,
            "cancelled": cancelled,
            "total_sessions": total_sessions,
            "used_sessions": used_sessions,
            "completed_sessions": completed_sessions,
            "remaining_sessions": remaining_sessions,
            "total_value": money::normalize(total_value),
            "received_amount": money::normalize(received_amount),
            "refunded_amount": money::normalize(refunded_amount),
            "outstanding_amount": money::normalize(outstanding_amount),
            "retained_amount": money::normalize(received_amount - refunded_amount),
        },
    }))
}

async fn subscription_supervised_sessions(
    state: &AppState,
    subscription_id: u64,
    filters: &ReportFilters,
) -> ApiResult<Vec<Value>> {
    let sessions = sqlx::query_as::<_, SessionRow>(
        "SELECT * FROM sessions \
         WHERE subscription_id = ? AND type = 'subscription' AND status IN ('closed','cancelled') \
           AND (? IS NULL OR DATE(opened_at) >= ?) \
           AND (? IS NULL OR DATE(opened_at) <= ?) \
           AND (? IS NULL OR doctor_id = ?) \
         ORDER BY closed_at, id",
    )
    .bind(subscription_id)
    .bind(filters.from)
    .bind(filters.from)
    .bind(filters.to)
    .bind(filters.to)
    .bind(filters.doctor_id)
    .bind(filters.doctor_id)
    .fetch_all(&state.pool)
    .await?;
    let mut values = Vec::with_capacity(sessions.len());
    for session in sessions {
        let supervising_doctor = doctor(state, session.doctor_id).await?;
        values.push(json!({
            "id": session.id,
            "code": session.code,
            "opened_at": time::iso(session.opened_at),
            "closed_at": session.closed_at.map(time::iso),
            "status": session.status,
            "doctor": {
                "id": supervising_doctor.id,
                "name": supervising_doctor.name,
            },
        }));
    }
    Ok(values)
}

async fn subscription_financials(
    state: &AppState,
    subscription_id: u64,
) -> ApiResult<(Decimal, Decimal)> {
    let received = sqlx::query_scalar::<_, Decimal>(
        "SELECT COALESCE(SUM(source.amount), 0) FROM (\
            SELECT p.amount AS amount FROM payments p \
            WHERE p.subscription_id = ? AND p.type = 'subscription_payment' AND p.amount > 0 \
            UNION ALL \
            SELECT pa.amount AS amount FROM payment_allocations pa \
            INNER JOIN payments p ON p.id = pa.payment_id AND p.type = 'debt_payment' \
            INNER JOIN debts d ON d.id = pa.debt_id \
            WHERE d.subscription_id = ?\
        ) source",
    )
    .bind(subscription_id)
    .bind(subscription_id)
    .fetch_one(&state.pool)
    .await?;
    let refunded = sqlx::query_scalar::<_, Decimal>(
        "SELECT COALESCE(SUM(ABS(amount)), 0) FROM payments \
         WHERE subscription_id = ? AND type = 'refund' AND amount < 0",
    )
    .bind(subscription_id)
    .fetch_one(&state.pool)
    .await?;
    Ok((received, refunded))
}

async fn patient(state: &AppState, id: u64) -> ApiResult<PatientRow> {
    Ok(
        sqlx::query_as::<_, PatientRow>("SELECT * FROM patients WHERE id = ?")
            .bind(id)
            .fetch_one(&state.pool)
            .await?,
    )
}

async fn doctor(state: &AppState, id: u64) -> ApiResult<DoctorRow> {
    Ok(
        sqlx::query_as::<_, DoctorRow>("SELECT * FROM doctors WHERE id = ?")
            .bind(id)
            .fetch_one(&state.pool)
            .await?,
    )
}

async fn sponsor(state: &AppState, id: u64) -> ApiResult<SponsorRow> {
    Ok(
        sqlx::query_as::<_, SponsorRow>("SELECT * FROM sponsors WHERE id = ?")
            .bind(id)
            .fetch_one(&state.pool)
            .await?,
    )
}

async fn session_services(state: &AppState, id: u64) -> ApiResult<Vec<SessionServiceRow>> {
    Ok(sqlx::query_as::<_, SessionServiceRow>(
        "SELECT * FROM session_services WHERE session_id = ? ORDER BY id",
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await?)
}
