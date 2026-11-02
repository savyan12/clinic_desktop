use crate::api::{
    error::{ApiError, ApiResult},
    loaders, money,
    rows::{
        DebtRow, DoctorRow, PatientRow, PaymentRow, SessionRow, SessionServiceRow, SponsorRow,
        SubscriptionRow, SubscriptionServiceRow,
    },
    time, AppState,
};
use axum::http::StatusCode;
use chrono::{Duration, NaiveDate, Utc};
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
    pub near_end: bool,
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
        if session.status == "closed" {
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
                OR EXISTS (SELECT 1 FROM subscriptions sub WHERE sub.id = p.subscription_id AND sub.sponsor_id = ?) \
                OR EXISTS (SELECT 1 FROM payments original INNER JOIN payment_allocations pa ON pa.payment_id = original.id INNER JOIN debts d ON d.id = pa.debt_id WHERE original.id = p.reverses_payment_id AND d.sponsor_id = ?) \
                OR EXISTS (SELECT 1 FROM payments original INNER JOIN sessions s ON s.id = original.session_id WHERE original.id = p.reverses_payment_id AND s.sponsor_id = ?) \
                OR EXISTS (SELECT 1 FROM payments original INNER JOIN subscriptions sub ON sub.id = original.subscription_id WHERE original.id = p.reverses_payment_id AND sub.sponsor_id = ?)) \
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
             WHERE doctor_id = ? AND status = 'closed' \
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
             WHERE doctor_id = ? AND status = 'closed' \
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
    let near_date = time::today() + Duration::days(state.config.near_end_days);
    let rows = sqlx::query_as::<_, SubscriptionRow>(
        "SELECT * FROM subscriptions \
         WHERE (? IS NULL OR start_date >= ?) \
           AND (? IS NULL OR start_date <= ?) \
           AND (? IS NULL OR patient_id = ?) \
           AND (? IS NULL OR status = ?) \
           AND (? = 0 OR (status = 'active' AND ( \
               (plan_type = 'legacy_package' AND ((sessions_count - sessions_used) <= ? OR expected_end_date <= ?)) \
               OR (plan_type = 'monthly' AND end_date <= ?) \
           ))) \
         ORDER BY id DESC",
    )
    .bind(filters.from)
    .bind(filters.from)
    .bind(filters.to)
    .bind(filters.to)
    .bind(filters.patient_id)
    .bind(filters.patient_id)
    .bind(status)
    .bind(status)
    .bind(filters.near_end)
    .bind(state.config.near_end_sessions)
    .bind(near_date)
    .bind(near_date)
    .fetch_all(&state.pool)
    .await?;
    let mut values = Vec::with_capacity(rows.len());
    let mut active = 0usize;
    let mut remaining_sessions = 0u64;
    let mut net_total = Decimal::ZERO;
    for subscription in &rows {
        let monthly = subscription.plan_type == "monthly";
        let remaining = (!monthly).then_some(
            subscription
                .sessions_count
                .saturating_sub(subscription.sessions_used),
        );
        let remaining_days = if monthly {
            subscription.end_date.map(|end| {
                if end < time::today() {
                    0
                } else {
                    (end - time::today()).num_days() + 1
                }
            })
        } else {
            None
        };
        if subscription.status == "active" {
            active += 1;
        }
        remaining_sessions += u64::from(remaining.unwrap_or(0));
        net_total += subscription.net_total;
        let patient = patient(state, subscription.patient_id).await?;
        let services = subscription_services(state, subscription.id).await?;
        values.push(json!({
            "id": subscription.id,
            "patient_id": subscription.patient_id,
            "patient": loaders::patient_summary(&patient),
            "plan_type": subscription.plan_type,
            "services": services.iter().map(|service| json!({
                "service_id": service.service_id,
                "name": service.service_name,
                "price": money::normalize(service.price),
            })).collect::<Vec<_>>(),
            "sessions_count": if monthly { Value::Null } else { json!(subscription.sessions_count) },
            "sessions_used": if monthly { Value::Null } else { json!(subscription.sessions_used) },
            "remaining": remaining,
            "remaining_days": remaining_days,
            "net_total": money::normalize(subscription.net_total),
            "discount": money::normalize(subscription.discount),
            "status": subscription.status,
            "start_date": subscription.start_date.format("%Y-%m-%d").to_string(),
            "expected_end_date": subscription.expected_end_date.map(|date| date.format("%Y-%m-%d").to_string()),
            "end_date": subscription.end_date.map(|date| date.format("%Y-%m-%d").to_string()),
        }));
    }
    Ok(json!({
        "rows": values,
        "totals": {
            "count": rows.len(),
            "active": active,
            "remaining_sessions": remaining_sessions,
            "net_total": money::normalize(net_total),
        },
    }))
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

async fn subscription_services(
    state: &AppState,
    id: u64,
) -> ApiResult<Vec<SubscriptionServiceRow>> {
    Ok(sqlx::query_as::<_, SubscriptionServiceRow>(
        "SELECT * FROM subscription_services WHERE subscription_id = ? ORDER BY id",
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await?)
}
