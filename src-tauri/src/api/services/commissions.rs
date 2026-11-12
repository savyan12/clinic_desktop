use crate::api::{error::ApiResult, money, rows::SessionRow};
use chrono::NaiveDate;
use rust_decimal::Decimal;
use sqlx::{MySql, MySqlPool, Transaction};

fn effective_amount(
    full_amount: Decimal,
    net_total: Decimal,
    outstanding_debt: Decimal,
) -> Decimal {
    if net_total <= Decimal::ZERO {
        return Decimal::ZERO;
    }

    let collected = (net_total - outstanding_debt)
        .max(Decimal::ZERO)
        .min(net_total);
    money::round(full_amount * collected / net_total)
}

pub fn apply_outstanding_debt(session: &SessionRow, outstanding_debt: Decimal) -> SessionRow {
    let mut effective = session.clone();
    if session.r#type != "individual" || session.status != "closed" {
        return effective;
    }

    effective.commission_base_amount = session
        .commission_base_amount
        .map(|amount| effective_amount(amount, session.net_total, outstanding_debt));
    effective.commission_amount = session
        .commission_amount
        .map(|amount| effective_amount(amount, session.net_total, outstanding_debt));
    effective
}

pub async fn effective_session_pool(
    pool: &MySqlPool,
    session: &SessionRow,
) -> ApiResult<SessionRow> {
    let outstanding = sqlx::query_scalar::<_, Option<Decimal>>(
        "SELECT SUM(amount - paid_amount) FROM debts WHERE session_id = ? AND status <> 'void'",
    )
    .bind(session.id)
    .fetch_one(pool)
    .await?
    .unwrap_or(Decimal::ZERO);
    Ok(apply_outstanding_debt(session, outstanding))
}

pub async fn effective_session_transaction(
    transaction: &mut Transaction<'_, MySql>,
    session: &SessionRow,
) -> ApiResult<SessionRow> {
    let outstanding = sqlx::query_scalar::<_, Option<Decimal>>(
        "SELECT SUM(amount - paid_amount) FROM debts WHERE session_id = ? AND status <> 'void'",
    )
    .bind(session.id)
    .fetch_one(&mut **transaction)
    .await?
    .unwrap_or(Decimal::ZERO);
    Ok(apply_outstanding_debt(session, outstanding))
}

pub async fn doctor_earned_pool(
    pool: &MySqlPool,
    doctor_id: u64,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
) -> ApiResult<Decimal> {
    effective_sum_pool(pool, Some(doctor_id), from, to).await
}

pub async fn total_earned_pool(pool: &MySqlPool) -> ApiResult<Decimal> {
    effective_sum_pool(pool, None, None, None).await
}

async fn effective_sum_pool(
    pool: &MySqlPool,
    doctor_id: Option<u64>,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
) -> ApiResult<Decimal> {
    let earned = sqlx::query_scalar::<_, Option<Decimal>>(
        "SELECT SUM(CASE \
            WHEN s.net_total <= 0 THEN 0 \
            ELSE ROUND(s.commission_amount * \
                GREATEST(s.net_total - COALESCE(( \
                    SELECT SUM(d.amount - d.paid_amount) FROM debts d \
                    WHERE d.session_id = s.id AND d.status <> 'void' \
                ), 0), 0) / s.net_total, 3) \
            END) \
         FROM sessions s \
         WHERE s.type = 'individual' AND s.status = 'closed' AND s.commission_amount IS NOT NULL \
           AND (? IS NULL OR s.doctor_id = ?) \
           AND (? IS NULL OR DATE(s.closed_at) >= ?) \
           AND (? IS NULL OR DATE(s.closed_at) <= ?)",
    )
    .bind(doctor_id)
    .bind(doctor_id)
    .bind(from)
    .bind(from)
    .bind(to)
    .bind(to)
    .fetch_one(pool)
    .await?
    .unwrap_or(Decimal::ZERO);
    Ok(money::round(earned))
}

pub async fn doctor_earned_transaction(
    transaction: &mut Transaction<'_, MySql>,
    doctor_id: u64,
) -> ApiResult<Decimal> {
    let earned = sqlx::query_scalar::<_, Option<Decimal>>(
        "SELECT SUM(CASE \
            WHEN s.net_total <= 0 THEN 0 \
            ELSE ROUND(s.commission_amount * \
                GREATEST(s.net_total - COALESCE(( \
                    SELECT SUM(d.amount - d.paid_amount) FROM debts d \
                    WHERE d.session_id = s.id AND d.status <> 'void' \
                ), 0), 0) / s.net_total, 3) \
            END) \
         FROM sessions s \
         WHERE s.doctor_id = ? AND s.type = 'individual' \
           AND s.status = 'closed' AND s.commission_amount IS NOT NULL",
    )
    .bind(doctor_id)
    .fetch_one(&mut **transaction)
    .await?
    .unwrap_or(Decimal::ZERO);
    Ok(money::round(earned))
}

#[cfg(test)]
mod tests {
    use super::effective_amount;
    use rust_decimal::Decimal;

    #[test]
    fn commission_is_earned_only_on_collected_value() {
        assert_eq!(
            effective_amount(Decimal::from(24), Decimal::from(80), Decimal::from(30)),
            Decimal::from(15)
        );
        assert_eq!(
            effective_amount(Decimal::from(24), Decimal::from(80), Decimal::from(15)),
            Decimal::new(195, 1)
        );
        assert_eq!(
            effective_amount(Decimal::from(24), Decimal::from(80), Decimal::ZERO),
            Decimal::from(24)
        );
    }

    #[test]
    fn gross_commission_is_deferred_proportionally() {
        assert_eq!(
            effective_amount(Decimal::from(30), Decimal::from(80), Decimal::from(30)),
            Decimal::new(1875, 2)
        );
    }
}
