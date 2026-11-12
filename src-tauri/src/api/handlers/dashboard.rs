use crate::api::{
    auth::AuthUser,
    error::ApiResult,
    loaders, money, resources,
    rows::{PaymentRow, SessionRow, SubscriptionRow},
    time, AppState,
};
use axum::{
    extract::{Query, State},
    Json,
};
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Debug, Deserialize)]
pub struct DashboardQuery {
    date: Option<NaiveDate>,
}

pub async fn show(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(query): Query<DashboardQuery>,
) -> ApiResult<Json<Value>> {
    let date = query.date.unwrap_or_else(time::today);
    let sessions_count = sqlx::query_scalar::<_, u64>(
        "SELECT CAST(COUNT(*) AS UNSIGNED) FROM sessions WHERE DATE(opened_at) = ? AND status <> 'cancelled'",
    )
    .bind(date)
    .fetch_one(&state.pool)
    .await?;
    let patients_count = sqlx::query_scalar::<_, u64>(
        "SELECT CAST(COUNT(DISTINCT patient_id) AS UNSIGNED) FROM sessions WHERE DATE(opened_at) = ? AND status <> 'cancelled'",
    )
    .bind(date)
    .fetch_one(&state.pool)
    .await?;
    let today_sessions = sqlx::query_as::<_, SessionRow>(
        "SELECT * FROM sessions WHERE DATE(opened_at) = ? AND status <> 'cancelled' ORDER BY opened_at DESC, id DESC",
    )
    .bind(date)
    .fetch_all(&state.pool)
    .await?;
    let open_sessions = sqlx::query_as::<_, SessionRow>(
        "SELECT * FROM sessions WHERE status = 'open' ORDER BY opened_at, id",
    )
    .fetch_all(&state.pool)
    .await?;
    let near_subscriptions = sqlx::query_as::<_, SubscriptionRow>(
        "SELECT * FROM subscriptions \
         WHERE status = 'active' AND (sessions_count - sessions_used) <= ? ORDER BY id DESC",
    )
    .bind(state.config.subscription_follow_up_sessions)
    .fetch_all(&state.pool)
    .await?;

    let open_session_values = load_sessions(&state, &open_sessions).await?;
    let today_session_values = load_sessions(&state, &today_sessions).await?;
    let near_subscription_values = load_subscriptions(&state, &near_subscriptions).await?;
    let date_text = date.format("%Y-%m-%d").to_string();
    let mut stats = vec![
        json!({
            "key": "sessions_today",
            "label": "جلسات اليوم",
            "value": sessions_count,
            "money": false,
            "to": format!("/reports/sessions?from={date_text}&to={date_text}"),
        }),
        json!({
            "key": "patients_today",
            "label": "المرضى الحاضرون اليوم",
            "value": patients_count,
            "money": false,
            "to": format!("/reports/sessions?from={date_text}&to={date_text}"),
        }),
    ];

    if auth.user.role != "admin" {
        stats.push(json!({
            "key": "near_subscriptions",
            "label": "الاشتراكات القريبة من الاكتمال",
            "value": near_subscriptions.len(),
            "money": false,
            "to": "/subscriptions",
        }));
        return Ok(Json(json!({
            "data": {
                "stats": stats,
                "open_sessions": open_session_values,
                "today_sessions": today_session_values,
                "near_subscriptions": near_subscription_values,
            },
        })));
    }

    let service_value = scalar_decimal(
        &state,
        "SELECT SUM(wallet_amount + paid_amount) FROM sessions WHERE DATE(opened_at) = ? AND status = 'closed' AND type = 'individual'",
        Some(date),
    )
    .await?;
    let collections = scalar_decimal(
        &state,
        "SELECT SUM(amount) FROM payments WHERE DATE(created_at) = ?",
        Some(date),
    )
    .await?;
    let debts_remaining = scalar_decimal(
        &state,
        "SELECT SUM(amount - paid_amount) FROM debts WHERE status != 'void'",
        None,
    )
    .await?;
    let doctor_earned = crate::api::services::commissions::total_earned_pool(&state.pool).await?;
    let doctor_paid =
        scalar_decimal(&state, "SELECT SUM(amount) FROM doctor_payouts", None).await?;
    let doctor_remaining = doctor_earned - doctor_paid;
    stats.extend([
        json!({ "key": "service_value_today", "label": "محصّل جلسات اليوم", "value": money::normalize(service_value), "money": true, "caption": "المدفوع والرصيد المستخدم فقط", "to": format!("/reports/sessions?from={date_text}&to={date_text}") }),
        json!({ "key": "collections_today", "label": "المقبوضات اليوم", "value": money::normalize(collections), "money": true, "to": format!("/reports/collections?from={date_text}&to={date_text}") }),
        json!({ "key": "debts_remaining", "label": "إجمالي الديون", "value": money::normalize(debts_remaining), "money": true, "to": "/reports/debts" }),
        json!({ "key": "near_subscriptions", "label": "الاشتراكات القريبة من الاكتمال", "value": near_subscriptions.len(), "money": false, "to": "/subscriptions" }),
        json!({ "key": "doctor_dues_remaining", "label": "مستحقات الأطباء غير المصروفة", "value": money::normalize(doctor_remaining), "money": true, "to": "/reports/doctor-dues" }),
    ]);

    let recent =
        sqlx::query_as::<_, PaymentRow>("SELECT * FROM payments ORDER BY id DESC LIMIT 10")
            .fetch_all(&state.pool)
            .await?;
    let mut recent_movements = Vec::with_capacity(recent.len());
    for payment in &recent {
        let patient = sqlx::query_as::<_, crate::api::rows::PatientRow>(
            "SELECT * FROM patients WHERE id = ?",
        )
        .bind(payment.patient_id)
        .fetch_one(&state.pool)
        .await?;
        recent_movements.push(resources::payment_value(
            payment,
            Some(loaders::patient_summary(&patient)),
        ));
    }

    Ok(Json(json!({
        "data": {
            "stats": stats,
            "open_sessions": open_session_values,
            "today_sessions": today_session_values,
            "near_subscriptions": near_subscription_values,
            "financial_summary": {
                "collections_today": money::normalize(collections),
                "debts_remaining": money::normalize(debts_remaining),
                "doctor_dues_remaining": money::normalize(doctor_remaining),
            },
            "recent_movements": recent_movements,
        },
    })))
}

async fn load_sessions(state: &AppState, rows: &[SessionRow]) -> ApiResult<Vec<Value>> {
    let mut values = Vec::with_capacity(rows.len());
    for row in rows {
        values.push(loaders::session_from_row(&state.pool, row).await?);
    }
    Ok(values)
}

async fn load_subscriptions(state: &AppState, rows: &[SubscriptionRow]) -> ApiResult<Vec<Value>> {
    let mut values = Vec::with_capacity(rows.len());
    for row in rows {
        values.push(loaders::subscription_for_dashboard_from_row(&state.pool, row).await?);
    }
    Ok(values)
}

async fn scalar_decimal(
    state: &AppState,
    query: &str,
    date: Option<NaiveDate>,
) -> ApiResult<Decimal> {
    let mut query = sqlx::query_scalar::<_, Option<Decimal>>(query);
    if let Some(date) = date {
        query = query.bind(date);
    }
    Ok(query.fetch_one(&state.pool).await?.unwrap_or(Decimal::ZERO))
}
