use super::{
    error::{ApiError, ApiResult},
    money, resources,
    rows::{
        PatientRow, SessionAdditionRow, SessionRow, SessionServiceRow, SubscriptionRow,
        SubscriptionServiceRow,
    },
};
use rust_decimal::Decimal;
use serde_json::{json, Value};
use sqlx::{MySqlPool, Row};

pub async fn session(pool: &MySqlPool, id: u64, include_wallet: bool) -> ApiResult<Value> {
    let session = sqlx::query_as::<_, SessionRow>("SELECT * FROM sessions WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(ApiError::not_found)?;
    session_detail_from_row(pool, &session, include_wallet).await
}

pub async fn session_from_row(pool: &MySqlPool, session: &SessionRow) -> ApiResult<Value> {
    session_summary_from_row(pool, session, true).await
}

pub async fn session_for_patient_from_row(
    pool: &MySqlPool,
    session: &SessionRow,
) -> ApiResult<Value> {
    session_summary_from_row(pool, session, false).await
}

async fn session_summary_from_row(
    pool: &MySqlPool,
    session: &SessionRow,
    include_patient: bool,
) -> ApiResult<Value> {
    let patient = if include_patient {
        let patient = sqlx::query_as::<_, PatientRow>("SELECT * FROM patients WHERE id = ?")
            .bind(session.patient_id)
            .fetch_one(pool)
            .await?;
        Some(patient_summary(&patient))
    } else {
        None
    };
    let doctor = sqlx::query("SELECT id, name FROM doctors WHERE id = ?")
        .bind(session.doctor_id)
        .fetch_one(pool)
        .await?;
    let doctor = json!({
        "id": doctor.try_get::<u64, _>("id")?,
        "name": doctor.try_get::<String, _>("name")?,
    });
    let services = sqlx::query_as::<_, SessionServiceRow>(
        "SELECT * FROM session_services WHERE session_id = ? ORDER BY id",
    )
    .bind(session.id)
    .fetch_all(pool)
    .await?
    .iter()
    .map(resources::session_service_value)
    .collect::<Vec<_>>();

    Ok(resources::session_value(
        session,
        patient,
        Some(doctor),
        Some(services),
        None,
        None,
        None,
        None,
    ))
}

async fn session_detail_from_row(
    pool: &MySqlPool,
    session: &SessionRow,
    include_wallet: bool,
) -> ApiResult<Value> {
    let patient = sqlx::query_as::<_, PatientRow>("SELECT * FROM patients WHERE id = ?")
        .bind(session.patient_id)
        .fetch_one(pool)
        .await?;
    let patient_value = if include_wallet {
        json!({
            "id": patient.id,
            "name": patient.name,
            "phone": patient.phone,
            "wallet_balance": money::normalize(wallet_balance(pool, patient.id).await?),
        })
    } else {
        patient_summary(&patient)
    };
    let doctor = sqlx::query("SELECT id, name FROM doctors WHERE id = ?")
        .bind(session.doctor_id)
        .fetch_one(pool)
        .await?;
    let doctor_value = json!({
        "id": doctor.try_get::<u64, _>("id")?,
        "name": doctor.try_get::<String, _>("name")?,
    });
    let services = sqlx::query_as::<_, SessionServiceRow>(
        "SELECT * FROM session_services WHERE session_id = ? ORDER BY id",
    )
    .bind(session.id)
    .fetch_all(pool)
    .await?
    .iter()
    .map(resources::session_service_value)
    .collect::<Vec<_>>();
    let additions = sqlx::query_as::<_, SessionAdditionRow>(
        "SELECT * FROM session_additions WHERE session_id = ? ORDER BY id",
    )
    .bind(session.id)
    .fetch_all(pool)
    .await?
    .iter()
    .map(resources::session_addition_value)
    .collect::<Vec<_>>();
    let creator = user_summary(pool, session.created_by).await?;
    let closed_by = Some(match session.closed_by {
        Some(id) => user_summary(pool, id).await?,
        None => Value::Null,
    });
    let cancelled_by = Some(match session.cancelled_by {
        Some(id) => user_summary(pool, id).await?,
        None => Value::Null,
    });

    Ok(resources::session_value(
        session,
        Some(patient_value),
        Some(doctor_value),
        Some(services),
        Some(creator),
        closed_by,
        cancelled_by,
        Some(additions),
    ))
}

pub async fn subscription(pool: &MySqlPool, id: u64) -> ApiResult<Value> {
    let subscription =
        sqlx::query_as::<_, SubscriptionRow>("SELECT * FROM subscriptions WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await?
            .ok_or_else(ApiError::not_found)?;
    subscription_from_row(pool, &subscription).await
}

pub async fn subscription_from_row(
    pool: &MySqlPool,
    subscription: &SubscriptionRow,
) -> ApiResult<Value> {
    subscription_with_relations(pool, subscription, true, true).await
}

pub async fn subscription_for_dashboard_from_row(
    pool: &MySqlPool,
    subscription: &SubscriptionRow,
) -> ApiResult<Value> {
    subscription_with_relations(pool, subscription, true, false).await
}

pub async fn subscription_for_patient_from_row(
    pool: &MySqlPool,
    subscription: &SubscriptionRow,
) -> ApiResult<Value> {
    subscription_with_relations(pool, subscription, false, false).await
}

async fn subscription_with_relations(
    pool: &MySqlPool,
    subscription: &SubscriptionRow,
    include_patient: bool,
    include_sponsor: bool,
) -> ApiResult<Value> {
    let patient = if include_patient {
        let patient = sqlx::query_as::<_, PatientRow>("SELECT * FROM patients WHERE id = ?")
            .bind(subscription.patient_id)
            .fetch_one(pool)
            .await?;
        Some(patient_summary(&patient))
    } else {
        None
    };
    let sponsor = if include_sponsor {
        Some(match subscription.sponsor_id {
            Some(id) => sqlx::query("SELECT id, name FROM sponsors WHERE id = ?")
                .bind(id)
                .fetch_optional(pool)
                .await?
                .map(|row| -> Result<Value, sqlx::Error> {
                    Ok(json!({
                        "id": row.try_get::<u64, _>("id")?,
                        "name": row.try_get::<String, _>("name")?,
                    }))
                })
                .transpose()?
                .unwrap_or(Value::Null),
            None => Value::Null,
        })
    } else {
        None
    };
    let services = sqlx::query_as::<_, SubscriptionServiceRow>(
        "SELECT * FROM subscription_services WHERE subscription_id = ? ORDER BY id",
    )
    .bind(subscription.id)
    .fetch_all(pool)
    .await?
    .iter()
    .map(resources::subscription_service_value)
    .collect::<Vec<_>>();

    Ok(resources::subscription_value(
        subscription,
        patient,
        sponsor,
        Some(services),
    ))
}

pub async fn wallet_balance(pool: &MySqlPool, patient_id: u64) -> ApiResult<Decimal> {
    sqlx::query_scalar::<_, Option<Decimal>>(
        "SELECT SUM(amount) FROM wallet_transactions WHERE patient_id = ?",
    )
    .bind(patient_id)
    .fetch_one(pool)
    .await
    .map(|value| value.unwrap_or(Decimal::ZERO))
    .map_err(Into::into)
}

pub async fn debt_total(pool: &MySqlPool, patient_id: u64) -> ApiResult<Decimal> {
    sqlx::query_scalar::<_, Option<Decimal>>(
        "SELECT SUM(amount - paid_amount) FROM debts WHERE patient_id = ? AND status != 'void'",
    )
    .bind(patient_id)
    .fetch_one(pool)
    .await
    .map(|value| value.unwrap_or(Decimal::ZERO))
    .map_err(Into::into)
}

pub async fn has_usable_subscription(pool: &MySqlPool, patient_id: u64) -> ApiResult<bool> {
    let count = sqlx::query_scalar::<_, u64>(
        "SELECT CAST(COUNT(*) AS UNSIGNED) FROM subscriptions \
         WHERE patient_id = ? AND status = 'active' \
           AND (plan_type = 'legacy_package' OR (plan_type = 'monthly' AND start_date <= CURDATE() AND end_date >= CURDATE()))",
    )
    .bind(patient_id)
    .fetch_one(pool)
    .await?;
    Ok(count > 0)
}

pub fn patient_summary(patient: &PatientRow) -> Value {
    json!({
        "id": patient.id,
        "name": patient.name,
        "phone": patient.phone,
    })
}

pub async fn user_summary(pool: &MySqlPool, id: u64) -> ApiResult<Value> {
    let row = sqlx::query("SELECT id, name FROM users WHERE id = ?")
        .bind(id)
        .fetch_one(pool)
        .await?;
    Ok(json!({
        "id": row.try_get::<u64, _>("id")?,
        "name": row.try_get::<String, _>("name")?,
    }))
}
