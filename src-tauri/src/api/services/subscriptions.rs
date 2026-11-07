use crate::api::{
    audit,
    error::{ApiError, ApiResult},
    money, resources,
    rows::{DebtRow, PatientRow, PaymentRow, ServiceRow, SubscriptionRow, SubscriptionServiceRow},
    services::ledger,
    time,
};
use rust_decimal::Decimal;
use serde_json::{json, Value};
use sqlx::{MySql, Row, Transaction};

#[derive(Debug, Clone)]
pub struct CreateInput {
    pub patient_id: u64,
    pub name: String,
    pub service_ids: Vec<u64>,
    pub sessions_count: u32,
    pub total_price: Decimal,
    pub paid_amount: Decimal,
    pub method: Option<String>,
}

#[derive(Debug)]
pub struct CreationResult {
    pub subscription: Value,
    pub payment: Option<PaymentRow>,
    pub debt: Option<DebtRow>,
}

#[derive(Debug, Clone)]
pub struct CorrectionInput {
    pub sessions_count: Option<u32>,
    pub total_price: Option<Decimal>,
}

#[derive(Debug, Clone)]
pub struct CancellationInput {
    pub reason: String,
    pub refund_amount: Decimal,
    pub refund_method: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FinancialSummary {
    pub received_amount: Decimal,
    pub refunded_amount: Decimal,
    pub retained_amount: Decimal,
    pub outstanding_amount: Decimal,
}

#[derive(Debug)]
pub struct CancellationResult {
    pub subscription: Value,
    pub refund: Option<PaymentRow>,
    pub voided_debt_ids: Vec<u64>,
}

pub async fn create(
    transaction: &mut Transaction<'_, MySql>,
    input: &CreateInput,
    actor_id: u64,
) -> ApiResult<CreationResult> {
    let patient = sqlx::query_as::<_, PatientRow>(
        "SELECT * FROM patients WHERE id = ? AND is_active = 1 FOR UPDATE",
    )
    .bind(input.patient_id)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or_else(|| {
        ApiError::validation("اختر مريضًا نشطًا.")
            .field("patient_id", "المريض المحدد غير موجود أو غير نشط.")
    })?;
    let services = active_subscription_services(transaction, &input.service_ids).await?;
    validate_initial_payment(
        input.total_price,
        input.paid_amount,
        input.method.as_deref(),
    )?;

    let result = sqlx::query(
        "INSERT INTO subscriptions \
         (patient_id, name, sessions_count, sessions_used, total_price, start_date, status, created_by, created_at, updated_at) \
         VALUES (?, ?, ?, 0, ?, CURDATE(), 'active', ?, NOW(), NOW())",
    )
    .bind(input.patient_id)
    .bind(input.name.trim())
    .bind(input.sessions_count)
    .bind(input.total_price)
    .bind(actor_id)
    .execute(&mut **transaction)
    .await?;
    let subscription_id = result.last_insert_id();

    for service in &services {
        sqlx::query(
            "INSERT INTO subscription_services \
             (subscription_id, service_id, service_name, created_by, created_at, updated_at) \
             VALUES (?, ?, ?, ?, NOW(), NOW())",
        )
        .bind(subscription_id)
        .bind(service.id)
        .bind(&service.name)
        .bind(actor_id)
        .execute(&mut **transaction)
        .await?;
    }

    let payment = if input.paid_amount > Decimal::ZERO {
        let payment_id = ledger::insert_payment(
            transaction,
            patient.id,
            None,
            None,
            Some(subscription_id),
            "subscription_payment",
            input.method.as_deref().expect("validated payment method"),
            input.paid_amount,
            None,
            Some("دفعة إنشاء الاشتراك"),
            actor_id,
        )
        .await?;
        Some(ledger::fetch_payment(transaction, payment_id).await?)
    } else {
        None
    };

    let remaining = money::round(input.total_price - input.paid_amount);
    let debt = if remaining > Decimal::ZERO {
        let debt_id = insert_subscription_debt(
            transaction,
            patient.id,
            subscription_id,
            remaining,
            actor_id,
        )
        .await?;
        Some(ledger::fetch_debt(transaction, debt_id).await?)
    } else {
        None
    };

    let created = fetch_locked_subscription(transaction, subscription_id).await?;
    let values = subscription_snapshot(&created);
    audit::log(
        transaction,
        actor_id,
        "subscription_created",
        "subscription",
        subscription_id,
        None,
        Some(&values),
        None,
    )
    .await?;

    Ok(CreationResult {
        subscription: subscription_value(transaction, &created).await?,
        payment,
        debt,
    })
}

pub async fn correct(
    transaction: &mut Transaction<'_, MySql>,
    subscription_id: u64,
    input: &CorrectionInput,
    reason: &str,
    actor_id: u64,
) -> ApiResult<Value> {
    let current = fetch_locked_subscription(transaction, subscription_id).await?;
    if current.status == "cancelled" {
        return Err(ApiError::validation("لا يمكن تعديل اشتراك ملغي."));
    }
    let sessions_count = input.sessions_count.unwrap_or(current.sessions_count);
    if sessions_count < current.sessions_used {
        return Err(
            ApiError::validation("عدد الجلسات الجديد أقل من الجلسات المستخدمة.").field(
                "changes.sessions_count",
                format!("الحد الأدنى هو {} جلسة.", current.sessions_used),
            ),
        );
    }
    let total_price = input.total_price.unwrap_or(current.total_price);
    let financials = financial_summary(transaction, &current).await?;
    if total_price < financials.retained_amount {
        return Err(
            ApiError::validation("قيمة الاشتراك الجديدة أقل من صافي المبلغ المحصل.").field(
                "changes.total_price",
                format!(
                    "لا يمكن أن تقل القيمة عن {}.",
                    money::normalize(financials.retained_amount)
                ),
            ),
        );
    }

    let status = if sessions_count == current.sessions_used {
        "completed"
    } else {
        "active"
    };
    sqlx::query(
        "UPDATE subscriptions SET sessions_count = ?, total_price = ?, status = ?, updated_at = NOW() WHERE id = ?",
    )
    .bind(sessions_count)
    .bind(total_price)
    .bind(status)
    .bind(subscription_id)
    .execute(&mut **transaction)
    .await?;

    reconcile_subscription_debt(
        transaction,
        &current,
        total_price,
        financials.received_amount,
        actor_id,
    )
    .await?;

    let updated = fetch_locked_subscription(transaction, subscription_id).await?;
    let old_values = subscription_snapshot(&current);
    let new_values = subscription_snapshot(&updated);
    audit::log(
        transaction,
        actor_id,
        "subscription_corrected",
        "subscription",
        subscription_id,
        Some(&old_values),
        Some(&new_values),
        Some(reason),
    )
    .await?;
    subscription_value(transaction, &updated).await
}

pub async fn cancellation_preview(
    transaction: &mut Transaction<'_, MySql>,
    subscription_id: u64,
) -> ApiResult<Value> {
    let subscription = fetch_locked_subscription(transaction, subscription_id).await?;
    require_cancellable(transaction, &subscription).await?;
    let summary = financial_summary(transaction, &subscription).await?;
    Ok(json!({
        "subscription_id": subscription.id,
        "subscription_name": subscription.name,
        "sessions_count": subscription.sessions_count,
        "sessions_used": subscription.sessions_used,
        "sessions_remaining": subscription.sessions_count.saturating_sub(subscription.sessions_used),
        "total_price": money::normalize(subscription.total_price),
        "received_amount": money::normalize(summary.received_amount),
        "refunded_amount": money::normalize(summary.refunded_amount),
        "maximum_refund": money::normalize(summary.retained_amount),
        "outstanding_amount": money::normalize(summary.outstanding_amount),
    }))
}

pub async fn cancel(
    transaction: &mut Transaction<'_, MySql>,
    subscription_id: u64,
    input: &CancellationInput,
    actor_id: u64,
) -> ApiResult<CancellationResult> {
    let current = fetch_locked_subscription(transaction, subscription_id).await?;
    require_cancellable(transaction, &current).await?;
    let summary = financial_summary(transaction, &current).await?;
    if input.refund_amount < Decimal::ZERO || input.refund_amount > summary.retained_amount {
        return Err(ApiError::validation("مبلغ الاسترجاع غير صالح.").field(
            "refund_amount",
            format!(
                "الحد الأقصى المتاح للاسترجاع هو {}.",
                money::normalize(summary.retained_amount)
            ),
        ));
    }
    if input.refund_amount > Decimal::ZERO && input.refund_method.is_none() {
        return Err(ApiError::validation("طريقة الاسترجاع مطلوبة.")
            .field("refund_method", "اختر طريقة الاسترجاع."));
    }

    let refund = if input.refund_amount > Decimal::ZERO {
        let refund_id = ledger::insert_payment(
            transaction,
            current.patient_id,
            None,
            None,
            Some(current.id),
            "refund",
            input
                .refund_method
                .as_deref()
                .expect("validated refund method"),
            -input.refund_amount,
            None,
            Some(&input.reason),
            actor_id,
        )
        .await?;
        allocate_subscription_refund(
            transaction,
            current.id,
            refund_id,
            input.refund_amount,
            actor_id,
        )
        .await?;
        Some(ledger::fetch_payment(transaction, refund_id).await?)
    } else {
        None
    };

    let debts = sqlx::query_as::<_, DebtRow>(
        "SELECT * FROM debts WHERE subscription_id = ? AND status IN ('open','partial') FOR UPDATE",
    )
    .bind(current.id)
    .fetch_all(&mut **transaction)
    .await?;
    let voided_debt_ids = debts.iter().map(|debt| debt.id).collect::<Vec<_>>();
    if !voided_debt_ids.is_empty() {
        sqlx::query(
            "UPDATE debts SET status = 'void', voided_at = NOW(), voided_by = ?, void_reason = ?, updated_at = NOW() \
             WHERE subscription_id = ? AND status IN ('open','partial')",
        )
        .bind(actor_id)
        .bind(&input.reason)
        .bind(current.id)
        .execute(&mut **transaction)
        .await?;
    }

    sqlx::query(
        "UPDATE subscriptions SET status = 'cancelled', cancelled_at = NOW(), cancelled_by = ?, cancel_reason = ?, updated_at = NOW() WHERE id = ?",
    )
    .bind(actor_id)
    .bind(&input.reason)
    .bind(current.id)
    .execute(&mut **transaction)
    .await?;

    let updated = fetch_locked_subscription(transaction, current.id).await?;
    let old_values = subscription_snapshot(&current);
    let new_values = json!({
        "subscription": subscription_snapshot(&updated),
        "refund_amount": money::normalize(input.refund_amount),
        "refund_payment_id": refund.as_ref().map(|payment| payment.id),
        "voided_debt_ids": voided_debt_ids,
    });
    audit::log(
        transaction,
        actor_id,
        "subscription_cancelled",
        "subscription",
        current.id,
        Some(&old_values),
        Some(&new_values),
        Some(&input.reason),
    )
    .await?;

    Ok(CancellationResult {
        subscription: subscription_value(transaction, &updated).await?,
        refund,
        voided_debt_ids,
    })
}

pub async fn financial_summary(
    transaction: &mut Transaction<'_, MySql>,
    subscription: &SubscriptionRow,
) -> ApiResult<FinancialSummary> {
    let received_amount = sqlx::query_scalar::<_, Decimal>(
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
    .bind(subscription.id)
    .bind(subscription.id)
    .fetch_one(&mut **transaction)
    .await?;
    let refunded_amount = sqlx::query_scalar::<_, Decimal>(
        "SELECT COALESCE(SUM(ABS(amount)), 0) FROM payments \
         WHERE subscription_id = ? AND type = 'refund' AND amount < 0",
    )
    .bind(subscription.id)
    .fetch_one(&mut **transaction)
    .await?;
    let retained_amount = money::round(received_amount - refunded_amount);
    let outstanding_amount = if subscription.status == "cancelled" {
        Decimal::ZERO
    } else {
        money::round((subscription.total_price - received_amount).max(Decimal::ZERO))
    };
    Ok(FinancialSummary {
        received_amount,
        refunded_amount,
        retained_amount,
        outstanding_amount,
    })
}

pub async fn subscription_value(
    transaction: &mut Transaction<'_, MySql>,
    subscription: &SubscriptionRow,
) -> ApiResult<Value> {
    let patient = sqlx::query_as::<_, PatientRow>("SELECT * FROM patients WHERE id = ?")
        .bind(subscription.patient_id)
        .fetch_one(&mut **transaction)
        .await?;
    let services = sqlx::query_as::<_, SubscriptionServiceRow>(
        "SELECT * FROM subscription_services WHERE subscription_id = ? ORDER BY id",
    )
    .bind(subscription.id)
    .fetch_all(&mut **transaction)
    .await?
    .iter()
    .map(resources::subscription_service_value)
    .collect::<Vec<_>>();
    let summary = financial_summary(transaction, subscription).await?;
    Ok(resources::subscription_value(
        subscription,
        Some(json!({
            "id": patient.id,
            "name": patient.name,
            "phone": patient.phone,
        })),
        Some(services),
        summary.received_amount,
        summary.refunded_amount,
    ))
}

pub async fn fetch_locked_subscription(
    transaction: &mut Transaction<'_, MySql>,
    subscription_id: u64,
) -> ApiResult<SubscriptionRow> {
    sqlx::query_as::<_, SubscriptionRow>("SELECT * FROM subscriptions WHERE id = ? FOR UPDATE")
        .bind(subscription_id)
        .fetch_optional(&mut **transaction)
        .await?
        .ok_or_else(ApiError::not_found)
}

async fn active_subscription_services(
    transaction: &mut Transaction<'_, MySql>,
    service_ids: &[u64],
) -> ApiResult<Vec<ServiceRow>> {
    let mut services = Vec::with_capacity(service_ids.len());
    for (index, service_id) in service_ids.iter().enumerate() {
        let service = sqlx::query_as::<_, ServiceRow>(
            "SELECT * FROM services WHERE id = ? AND is_active = 1 FOR UPDATE",
        )
        .bind(service_id)
        .fetch_optional(&mut **transaction)
        .await?
        .ok_or_else(|| {
            ApiError::validation("إحدى الخدمات المحددة غير متاحة.").field(
                format!("service_ids.{index}"),
                "الخدمة غير موجودة أو غير نشطة.",
            )
        })?;
        services.push(service);
    }
    Ok(services)
}

fn validate_initial_payment(
    total_price: Decimal,
    paid_amount: Decimal,
    method: Option<&str>,
) -> ApiResult<()> {
    if total_price <= Decimal::ZERO {
        return Err(
            ApiError::validation("قيمة الاشتراك يجب أن تكون أكبر من صفر.")
                .field("total_price", "أدخل قيمة اشتراك صحيحة."),
        );
    }
    if paid_amount < Decimal::ZERO || paid_amount > total_price {
        return Err(ApiError::validation("المبلغ المدفوع غير صالح.")
            .field("paid_amount", "يجب ألا يتجاوز المدفوع قيمة الاشتراك."));
    }
    if paid_amount > Decimal::ZERO && method.is_none() {
        return Err(
            ApiError::validation("طريقة الدفع مطلوبة.").field("method", "اختر طريقة الدفع.")
        );
    }
    Ok(())
}

async fn insert_subscription_debt(
    transaction: &mut Transaction<'_, MySql>,
    patient_id: u64,
    subscription_id: u64,
    amount: Decimal,
    actor_id: u64,
) -> ApiResult<u64> {
    let result = sqlx::query(
        "INSERT INTO debts \
         (patient_id, sponsor_id, subscription_id, kind, amount, paid_amount, status, note, created_by, created_at, updated_at) \
         VALUES (?, NULL, ?, 'subscription', ?, 0, 'open', 'المتبقي من قيمة الاشتراك', ?, NOW(), NOW())",
    )
    .bind(patient_id)
    .bind(subscription_id)
    .bind(amount)
    .bind(actor_id)
    .execute(&mut **transaction)
    .await?;
    Ok(result.last_insert_id())
}

async fn reconcile_subscription_debt(
    transaction: &mut Transaction<'_, MySql>,
    subscription: &SubscriptionRow,
    total_price: Decimal,
    received_amount: Decimal,
    actor_id: u64,
) -> ApiResult<()> {
    let outstanding = money::round((total_price - received_amount).max(Decimal::ZERO));
    let debts = sqlx::query_as::<_, DebtRow>(
        "SELECT * FROM debts WHERE subscription_id = ? AND status != 'void' ORDER BY id FOR UPDATE",
    )
    .bind(subscription.id)
    .fetch_all(&mut **transaction)
    .await?;
    if debts.len() > 1 {
        return Err(ApiError::internal(
            "Subscription has more than one active debt record.",
        ));
    }
    if let Some(debt) = debts.first() {
        let amount = money::round(debt.paid_amount + outstanding);
        let status = if outstanding == Decimal::ZERO {
            "paid"
        } else if debt.paid_amount > Decimal::ZERO {
            "partial"
        } else {
            "open"
        };
        sqlx::query("UPDATE debts SET amount = ?, status = ?, updated_at = NOW() WHERE id = ?")
            .bind(amount)
            .bind(status)
            .bind(debt.id)
            .execute(&mut **transaction)
            .await?;
    } else if outstanding > Decimal::ZERO {
        insert_subscription_debt(
            transaction,
            subscription.patient_id,
            subscription.id,
            outstanding,
            actor_id,
        )
        .await?;
    }
    Ok(())
}

async fn require_cancellable(
    transaction: &mut Transaction<'_, MySql>,
    subscription: &SubscriptionRow,
) -> ApiResult<()> {
    if subscription.status == "cancelled" {
        return Err(ApiError::validation("الاشتراك ملغي بالفعل."));
    }
    let open_session = sqlx::query_scalar::<_, u64>(
        "SELECT id FROM sessions WHERE subscription_id = ? AND status = 'open' LIMIT 1 FOR UPDATE",
    )
    .bind(subscription.id)
    .fetch_optional(&mut **transaction)
    .await?;
    if open_session.is_some() {
        return Err(ApiError::validation(
            "أغلق أو ألغِ الجلسة المفتوحة قبل إلغاء الاشتراك.",
        ));
    }
    Ok(())
}

async fn allocate_subscription_refund(
    transaction: &mut Transaction<'_, MySql>,
    subscription_id: u64,
    refund_payment_id: u64,
    refund_amount: Decimal,
    actor_id: u64,
) -> ApiResult<()> {
    let rows = sqlx::query(
        "SELECT source.payment_id, source.attributed_amount - COALESCE(refunded.amount, 0) AS available_amount \
         FROM (\
             SELECT p.id AS payment_id, p.amount AS attributed_amount \
             FROM payments p \
             WHERE p.subscription_id = ? AND p.type = 'subscription_payment' AND p.amount > 0 \
             UNION ALL \
             SELECT p.id AS payment_id, SUM(pa.amount) AS attributed_amount \
             FROM payments p \
             INNER JOIN payment_allocations pa ON pa.payment_id = p.id \
             INNER JOIN debts d ON d.id = pa.debt_id \
             WHERE d.subscription_id = ? AND p.type = 'debt_payment' \
             GROUP BY p.id\
         ) source \
         LEFT JOIN (\
             SELECT pra.original_payment_id, SUM(pra.amount) AS amount \
             FROM payment_refund_allocations pra \
             INNER JOIN payments rp ON rp.id = pra.refund_payment_id \
             WHERE rp.subscription_id = ? AND rp.type = 'refund' \
             GROUP BY pra.original_payment_id\
         ) refunded ON refunded.original_payment_id = source.payment_id \
         ORDER BY source.payment_id DESC",
    )
    .bind(subscription_id)
    .bind(subscription_id)
    .bind(subscription_id)
    .fetch_all(&mut **transaction)
    .await?;

    let mut remaining = refund_amount;
    for row in rows {
        if remaining == Decimal::ZERO {
            break;
        }
        let payment_id = row.try_get::<u64, _>("payment_id")?;
        sqlx::query("SELECT id FROM payments WHERE id = ? FOR UPDATE")
            .bind(payment_id)
            .fetch_one(&mut **transaction)
            .await?;
        let available = row.try_get::<Decimal, _>("available_amount")?;
        if available <= Decimal::ZERO {
            continue;
        }
        let allocated = available.min(remaining);
        sqlx::query(
            "INSERT INTO payment_refund_allocations \
             (refund_payment_id, original_payment_id, amount, created_by, created_at, updated_at) \
             VALUES (?, ?, ?, ?, NOW(), NOW())",
        )
        .bind(refund_payment_id)
        .bind(payment_id)
        .bind(allocated)
        .bind(actor_id)
        .execute(&mut **transaction)
        .await?;
        remaining -= allocated;
    }
    if remaining > Decimal::ZERO {
        return Err(ApiError::internal(
            "Refund allocation exceeded attributable subscription payments.",
        ));
    }
    Ok(())
}

fn subscription_snapshot(subscription: &SubscriptionRow) -> Value {
    json!({
        "id": subscription.id,
        "patient_id": subscription.patient_id,
        "name": subscription.name,
        "sessions_count": subscription.sessions_count,
        "sessions_used": subscription.sessions_used,
        "total_price": money::normalize(subscription.total_price),
        "start_date": subscription.start_date.format("%Y-%m-%d").to_string(),
        "status": subscription.status,
        "cancelled_at": subscription.cancelled_at.map(time::iso),
        "cancelled_by": subscription.cancelled_by,
        "cancel_reason": subscription.cancel_reason,
        "created_by": subscription.created_by,
    })
}
