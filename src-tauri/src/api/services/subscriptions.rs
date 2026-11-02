use crate::api::{
    audit,
    error::{ApiError, ApiResult},
    money, resources,
    rows::{DebtRow, PatientRow, PaymentRow, ServiceRow, SubscriptionRow, SubscriptionServiceRow},
    services::ledger,
    time,
};
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde_json::{json, Value};
use sqlx::{MySql, Row, Transaction};
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct CreateInput {
    pub patient_id: u64,
    pub plan_type: String,
    pub amount: Option<Decimal>,
    pub service_ids: Vec<u64>,
    pub sessions_count: Option<u32>,
    pub discount: Decimal,
    pub paid_amount: Decimal,
    pub method: Option<String>,
    pub start_date: NaiveDate,
    pub expected_end_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub sponsor_id: Option<u64>,
}

#[derive(Debug)]
pub struct CreationResult {
    pub subscription: Value,
    pub payment: Option<PaymentRow>,
    pub debt: Option<DebtRow>,
}

#[derive(Debug, Clone)]
pub struct CorrectionInput {
    pub expected_end_date_present: bool,
    pub expected_end_date: Option<NaiveDate>,
    pub end_date_present: bool,
    pub end_date: Option<NaiveDate>,
    pub sponsor_id_present: bool,
    pub sponsor_id: Option<u64>,
}

#[derive(Debug)]
pub struct CancellationState {
    pub preview: Value,
    refund_amount: Decimal,
    replacement_debt_amount: Decimal,
    source_payment_ids: Vec<u64>,
    debt_ids_to_void: Vec<u64>,
}

#[derive(Debug)]
pub struct CancellationResult {
    pub subscription: Value,
    pub preview: Value,
    pub refund: Option<PaymentRow>,
    pub voided_debt_ids: Vec<u64>,
    pub replacement_debt: Option<DebtRow>,
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
        ApiError::validation("المريض المحدد غير متاح.").field("patient_id", "اختر مريضًا نشطًا.")
    })?;
    require_active_sponsor(transaction, input.sponsor_id).await?;

    let (sessions_count, base_total, discount, net_total, expected_end_date, end_date, services) =
        if input.plan_type == "monthly" {
            let amount = input.amount.ok_or_else(|| {
                ApiError::validation("قيمة الاشتراك مطلوبة.")
                    .field("amount", "أدخل قيمة الاشتراك الشهري.")
            })?;
            let end_date = input.end_date.ok_or_else(|| {
                ApiError::validation("تاريخ نهاية الاشتراك مطلوب.")
                    .field("end_date", "اختر تاريخ نهاية الاشتراك.")
            })?;
            if end_date < input.start_date {
                return Err(
                    ApiError::validation("تاريخ نهاية الاشتراك يجب ألا يسبق تاريخ البداية.").field(
                        "end_date",
                        "اختر تاريخ نهاية يساوي تاريخ البداية أو يأتي بعده.",
                    ),
                );
            }
            (
                0,
                amount,
                Decimal::ZERO,
                amount,
                None,
                Some(end_date),
                Vec::new(),
            )
        } else {
            let sessions_count = input.sessions_count.ok_or_else(|| {
                ApiError::validation("عدد الجلسات مطلوب.")
                    .field("sessions_count", "أدخل عدد جلسات صحيحًا.")
            })?;
            if sessions_count == 0 {
                return Err(ApiError::validation("عدد الجلسات غير صالح.")
                    .field("sessions_count", "يجب أن يكون عدد الجلسات أكبر من صفر."));
            }
            if input.service_ids.is_empty() {
                return Err(ApiError::validation("اختر خدمة واحدة على الأقل.")
                    .field("service_ids", "اختر خدمة واحدة على الأقل."));
            }
            let mut service_ids = input.service_ids.clone();
            service_ids.sort_unstable();
            service_ids.dedup();
            if service_ids.len() != input.service_ids.len() {
                return Err(ApiError::validation("البيانات المرسلة غير صحيحة.")
                    .field("service_ids", "لا يجوز تكرار الخدمة نفسها."));
            }
            let mut services = Vec::with_capacity(service_ids.len());
            for service_id in service_ids {
                let service = sqlx::query_as::<_, ServiceRow>(
                    "SELECT * FROM services WHERE id = ? AND is_active = 1 FOR UPDATE",
                )
                .bind(service_id)
                .fetch_optional(&mut **transaction)
                .await?
                .ok_or_else(|| {
                    ApiError::validation("إحدى الخدمات غير متاحة للاشتراكات الجديدة.")
                        .field("service_ids", "اختر خدمات نشطة فقط.")
                })?;
                services.push(service);
            }
            let services_per_session = money::sum(services.iter().map(|service| service.price));
            let base_total = money::round(services_per_session * Decimal::from(sessions_count));
            if input.discount > base_total {
                return Err(
                    ApiError::validation("الخصم لا يمكن أن يتجاوز السعر الأساسي للاشتراك.")
                        .field("discount", "قيمة الخصم أكبر من السعر الأساسي."),
                );
            }
            (
                sessions_count,
                base_total,
                input.discount,
                money::round(base_total - input.discount),
                input.expected_end_date,
                None,
                services,
            )
        };

    validate_initial_payment(net_total, input.paid_amount, input.method.as_deref())?;
    let result = sqlx::query(
        "INSERT INTO subscriptions \
         (patient_id, sponsor_id, plan_type, sessions_count, sessions_used, base_total, discount, net_total, paid_amount, start_date, expected_end_date, end_date, status, refund_amount, created_by, created_at, updated_at) \
         VALUES (?, ?, ?, ?, 0, ?, ?, ?, ?, ?, ?, ?, 'active', 0, ?, NOW(), NOW())",
    )
    .bind(patient.id)
    .bind(input.sponsor_id)
    .bind(&input.plan_type)
    .bind(sessions_count)
    .bind(base_total)
    .bind(discount)
    .bind(net_total)
    .bind(input.paid_amount)
    .bind(input.start_date)
    .bind(expected_end_date)
    .bind(end_date)
    .bind(actor_id)
    .execute(&mut **transaction)
    .await?;
    let subscription_id = result.last_insert_id();

    for service in &services {
        sqlx::query(
            "INSERT INTO subscription_services \
             (subscription_id, service_id, service_name, price, created_by, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, NOW(), NOW())",
        )
        .bind(subscription_id)
        .bind(service.id)
        .bind(&service.name)
        .bind(service.price)
        .bind(actor_id)
        .execute(&mut **transaction)
        .await?;
    }

    let payment = if input.paid_amount > Decimal::ZERO {
        let method = input.method.as_deref().expect("validated payment method");
        let payment_id = ledger::insert_payment(
            transaction,
            patient.id,
            None,
            None,
            Some(subscription_id),
            "subscription_payment",
            method,
            input.paid_amount,
            None,
            None,
            actor_id,
        )
        .await?;
        let values = json!({
            "subscription_id": subscription_id,
            "type": "subscription_payment",
            "method": method,
            "amount": money::normalize(input.paid_amount),
        });
        audit::log(
            transaction,
            actor_id,
            "payment_recorded",
            "payment",
            payment_id,
            None,
            Some(&values),
            None,
        )
        .await?;
        Some(ledger::fetch_payment(transaction, payment_id).await?)
    } else {
        None
    };

    let remaining = money::round(net_total - input.paid_amount);
    let debt = if remaining > Decimal::ZERO {
        let result = sqlx::query(
            "INSERT INTO debts \
             (patient_id, sponsor_id, subscription_id, kind, amount, paid_amount, status, created_by, created_at, updated_at) \
             VALUES (?, ?, ?, 'subscription', ?, 0, 'open', ?, NOW(), NOW())",
        )
        .bind(patient.id)
        .bind(input.sponsor_id)
        .bind(subscription_id)
        .bind(remaining)
        .bind(actor_id)
        .execute(&mut **transaction)
        .await?;
        Some(ledger::fetch_debt(transaction, result.last_insert_id()).await?)
    } else {
        None
    };

    let subscription_row = fetch_locked_subscription(transaction, subscription_id).await?;
    let mut audit_values = subscription_snapshot(&subscription_row);
    let audit_object = audit_values
        .as_object_mut()
        .expect("subscription snapshot must be an object");
    if input.plan_type == "legacy_package" {
        audit_object.insert(
            "service_ids".into(),
            Value::Array(input.service_ids.iter().copied().map(Value::from).collect()),
        );
    }
    audit_object.insert(
        "payment_id".into(),
        payment
            .as_ref()
            .map(|row| Value::from(row.id))
            .unwrap_or(Value::Null),
    );
    audit_object.insert(
        "debt_id".into(),
        debt.as_ref()
            .map(|row| Value::from(row.id))
            .unwrap_or(Value::Null),
    );
    audit::log(
        transaction,
        actor_id,
        "subscription_created",
        "subscription",
        subscription_id,
        None,
        Some(&audit_values),
        None,
    )
    .await?;

    Ok(CreationResult {
        subscription: subscription_value(transaction, &subscription_row).await?,
        payment,
        debt,
    })
}

pub async fn stop(
    transaction: &mut Transaction<'_, MySql>,
    subscription_id: u64,
    reason: &str,
    actor_id: u64,
) -> ApiResult<Value> {
    let current = fetch_locked_subscription(transaction, subscription_id).await?;
    if current.status != "active" {
        return Err(ApiError::conflict("لا يمكن إيقاف اشتراك غير نشط."));
    }
    let old = subscription_snapshot(&current);
    sqlx::query("UPDATE subscriptions SET status = 'stopped', updated_at = NOW() WHERE id = ?")
        .bind(subscription_id)
        .execute(&mut **transaction)
        .await?;
    let updated = fetch_locked_subscription(transaction, subscription_id).await?;
    let new = subscription_snapshot(&updated);
    audit::log(
        transaction,
        actor_id,
        "subscription_stopped",
        "subscription",
        subscription_id,
        Some(&old),
        Some(&new),
        Some(reason),
    )
    .await?;
    subscription_value(transaction, &updated).await
}

pub async fn reactivate(
    transaction: &mut Transaction<'_, MySql>,
    subscription_id: u64,
    reason: Option<&str>,
    actor_id: u64,
) -> ApiResult<Value> {
    let current = fetch_locked_subscription(transaction, subscription_id).await?;
    if current.status != "stopped" {
        return Err(ApiError::conflict("يمكن إعادة تفعيل الاشتراك الموقوف فقط."));
    }
    if current.plan_type == "monthly" && current.end_date.is_some_and(|date| date < time::today()) {
        return Err(
            ApiError::validation("انتهت مدة الاشتراك ولا يمكن إعادة تفعيله.")
                .field("subscription", "أنشئ اشتراكًا شهريًا جديدًا للمريض."),
        );
    }
    if current.plan_type != "monthly" && current.sessions_used >= current.sessions_count {
        return Err(
            ApiError::validation("لا توجد جلسات متبقية لإعادة تفعيل الاشتراك.").field(
                "subscription",
                "أنشئ اشتراكًا جديدًا للمريض بدلًا من إعادة التفعيل.",
            ),
        );
    }
    let old = subscription_snapshot(&current);
    sqlx::query("UPDATE subscriptions SET status = 'active', updated_at = NOW() WHERE id = ?")
        .bind(subscription_id)
        .execute(&mut **transaction)
        .await?;
    let updated = fetch_locked_subscription(transaction, subscription_id).await?;
    let new = subscription_snapshot(&updated);
    audit::log(
        transaction,
        actor_id,
        "subscription_reactivated",
        "subscription",
        subscription_id,
        Some(&old),
        Some(&new),
        reason,
    )
    .await?;
    let mut value = subscription_value(transaction, &updated).await?;
    value
        .as_object_mut()
        .expect("subscription resource must be an object")
        .remove("sponsor");
    Ok(value)
}

pub async fn correct(
    transaction: &mut Transaction<'_, MySql>,
    subscription_id: u64,
    input: &CorrectionInput,
    reason: &str,
    actor_id: u64,
) -> ApiResult<Value> {
    let current = fetch_locked_subscription(transaction, subscription_id).await?;
    let old = subscription_snapshot(&current);

    if current.plan_type == "monthly" && input.end_date_present {
        let end_date = input.end_date.ok_or_else(|| {
            ApiError::validation("تاريخ نهاية الاشتراك مطلوب.")
                .field("changes.end_date", "اختر تاريخ نهاية صحيحًا.")
        })?;
        if end_date < current.start_date {
            return Err(
                ApiError::validation("تاريخ نهاية الاشتراك يجب ألا يسبق تاريخ البداية.")
                    .field("changes.end_date", "اختر تاريخ نهاية صحيحًا."),
            );
        }
        sqlx::query("UPDATE subscriptions SET end_date = ?, updated_at = NOW() WHERE id = ?")
            .bind(end_date)
            .bind(subscription_id)
            .execute(&mut **transaction)
            .await?;
    }

    if current.plan_type != "monthly" && input.expected_end_date_present {
        sqlx::query(
            "UPDATE subscriptions SET expected_end_date = ?, updated_at = NOW() WHERE id = ?",
        )
        .bind(input.expected_end_date)
        .bind(subscription_id)
        .execute(&mut **transaction)
        .await?;
    }

    if input.sponsor_id_present {
        let debt_exists =
            sqlx::query("SELECT id FROM debts WHERE subscription_id = ? LIMIT 1 FOR UPDATE")
                .bind(subscription_id)
                .fetch_optional(&mut **transaction)
                .await?
                .is_some();
        if debt_exists {
            return Err(ApiError::validation(
                "لا يمكن تغيير الجهة الراعية بعد تسجيل دين على الاشتراك.",
            )
            .field(
                "changes.sponsor_id",
                "أنشئ تصحيحًا ماليًا مستقلًا يحافظ على السجل الأصلي.",
            ));
        }
        require_active_sponsor(transaction, input.sponsor_id).await?;
        sqlx::query("UPDATE subscriptions SET sponsor_id = ?, updated_at = NOW() WHERE id = ?")
            .bind(input.sponsor_id)
            .bind(subscription_id)
            .execute(&mut **transaction)
            .await?;
    }

    let updated = fetch_locked_subscription(transaction, subscription_id).await?;
    let new = subscription_snapshot(&updated);
    audit::log(
        transaction,
        actor_id,
        "subscription_corrected",
        "subscription",
        subscription_id,
        Some(&old),
        Some(&new),
        Some(reason),
    )
    .await?;
    subscription_value(transaction, &updated).await
}

pub async fn cancellation_state(
    transaction: &mut Transaction<'_, MySql>,
    subscription_id: u64,
) -> ApiResult<(SubscriptionRow, CancellationState)> {
    let subscription = fetch_locked_subscription(transaction, subscription_id).await?;
    if subscription.status == "cancelled" {
        return Err(ApiError::conflict("تم إلغاء الاشتراك بالفعل."));
    }
    if subscription.status == "completed" {
        return Err(ApiError::conflict("لا يمكن إلغاء اشتراك مكتمل."));
    }
    if !matches!(subscription.status.as_str(), "active" | "stopped") {
        return Err(ApiError::conflict("لا يمكن إلغاء الاشتراك في حالته الحالية."));
    }

    let open_session = sqlx::query(
        "SELECT id FROM sessions WHERE subscription_id = ? AND status = 'open' LIMIT 1 FOR UPDATE",
    )
    .bind(subscription_id)
    .fetch_optional(&mut **transaction)
    .await?
    .is_some();
    if open_session {
        return Err(ApiError::conflict(
            "توجد جلسة مفتوحة مرتبطة بالاشتراك. أغلقها أو ألغها أولًا.",
        ));
    }

    let monthly = subscription.plan_type == "monthly";
    let (sessions_remaining, period_days, elapsed_days, remaining_days, per_session_value) =
        if monthly {
            let end_date = subscription.end_date.ok_or_else(|| {
                ApiError::internal("Monthly subscription is missing its end date.")
            })?;
            let period_days = (end_date - subscription.start_date).num_days() + 1;
            if period_days <= 0 {
                return Err(ApiError::internal(
                    "Monthly subscription has an invalid date range.",
                ));
            }
            let today = time::today();
            let elapsed_days = if today < subscription.start_date {
                0
            } else {
                let capped = today.min(end_date);
                ((capped - subscription.start_date).num_days() + 1).min(period_days)
            };
            (
                None,
                Some(period_days as u32),
                Some(elapsed_days as u32),
                Some((period_days - elapsed_days).max(0) as u32),
                None,
            )
        } else {
            let remaining = subscription
                .sessions_count
                .saturating_sub(subscription.sessions_used);
            if remaining == 0 {
                return Err(ApiError::validation("لا توجد جلسات متبقية لإلغاء الاشتراك.")
                    .field("subscription", "يجب أن يحتوي الاشتراك على جلسات متبقية."));
            }
            (
                Some(remaining),
                None,
                None,
                None,
                Some(
                    money::divide(subscription.net_total, subscription.sessions_count)
                        .map_err(ApiError::internal)?,
                ),
            )
        };

    let debts = sqlx::query_as::<_, DebtRow>(
        "SELECT * FROM debts WHERE subscription_id = ? ORDER BY id FOR UPDATE",
    )
    .bind(subscription_id)
    .fetch_all(&mut **transaction)
    .await?;

    let direct_payments = sqlx::query_as::<_, PaymentRow>(
        "SELECT * FROM payments WHERE subscription_id = ? AND type = 'subscription_payment' AND amount > 0 FOR UPDATE",
    )
    .bind(subscription_id)
    .fetch_all(&mut **transaction)
    .await?;
    let allocation_rows = sqlx::query(
        "SELECT pa.payment_id, pa.amount AS contribution, p.amount AS payment_amount \
         FROM payment_allocations pa \
         INNER JOIN debts d ON d.id = pa.debt_id \
         INNER JOIN payments p ON p.id = pa.payment_id \
         WHERE d.subscription_id = ? FOR UPDATE",
    )
    .bind(subscription_id)
    .fetch_all(&mut **transaction)
    .await?;

    let mut contributions = BTreeMap::<u64, Decimal>::new();
    for payment in &direct_payments {
        *contributions.entry(payment.id).or_default() += payment.amount;
    }
    for row in allocation_rows {
        let payment_id = row.try_get::<u64, _>("payment_id")?;
        let payment_amount = row.try_get::<Decimal, _>("payment_amount")?;
        if payment_amount > Decimal::ZERO {
            let amount = row.try_get::<Decimal, _>("contribution")?;
            *contributions.entry(payment_id).or_default() += amount;
        }
    }

    let mut paid_total = Decimal::ZERO;
    let mut source_payment_ids = Vec::new();
    for (payment_id, contribution) in contributions {
        let refunded = sqlx::query_scalar::<_, Option<Decimal>>(
            "SELECT SUM(ABS(amount)) FROM payments WHERE reverses_payment_id = ? AND type = 'refund' FOR UPDATE",
        )
        .bind(payment_id)
        .fetch_one(&mut **transaction)
        .await?
        .unwrap_or(Decimal::ZERO);
        let net = if refunded >= contribution {
            Decimal::ZERO
        } else {
            money::round(contribution - refunded)
        };
        if net > Decimal::ZERO {
            paid_total += net;
            source_payment_ids.push(payment_id);
        }
    }
    paid_total = money::round(paid_total);

    let consumed_value = if monthly {
        let elapsed = elapsed_days.expect("monthly elapsed days");
        let period = period_days.expect("monthly period days");
        money::round(subscription.net_total * Decimal::from(elapsed) / Decimal::from(period))
    } else {
        money::round(
            subscription.net_total * Decimal::from(subscription.sessions_used)
                / Decimal::from(subscription.sessions_count),
        )
    };
    let refund_amount = if paid_total > consumed_value {
        money::round(paid_total - consumed_value)
    } else {
        Decimal::ZERO
    };
    let replacement_debt_amount = if consumed_value > paid_total {
        money::round(consumed_value - paid_total)
    } else {
        Decimal::ZERO
    };
    let debts_to_void = debts
        .iter()
        .filter(|debt| matches!(debt.status.as_str(), "open" | "partial"))
        .collect::<Vec<_>>();
    let debt_voided_amount = money::sum(
        debts_to_void
            .iter()
            .map(|debt| debt.amount - debt.paid_amount),
    );
    let preview = json!({
        "plan_type": subscription.plan_type,
        "sessions_count": if monthly { Value::Null } else { Value::from(subscription.sessions_count) },
        "sessions_used": if monthly { Value::Null } else { Value::from(subscription.sessions_used) },
        "sessions_remaining": sessions_remaining,
        "period_days": period_days,
        "elapsed_days": elapsed_days,
        "remaining_days": remaining_days,
        "per_session_value": per_session_value.map(money::normalize),
        "consumed_value": money::normalize(consumed_value),
        "paid_total": money::normalize(paid_total),
        "refund_amount": money::normalize(refund_amount),
        "debt_voided_amount": money::normalize(debt_voided_amount),
        "replacement_debt_amount": money::normalize(replacement_debt_amount),
    });

    Ok((
        subscription,
        CancellationState {
            preview,
            refund_amount,
            replacement_debt_amount,
            source_payment_ids,
            debt_ids_to_void: debts_to_void.iter().map(|debt| debt.id).collect(),
        },
    ))
}

pub async fn cancel(
    transaction: &mut Transaction<'_, MySql>,
    subscription_id: u64,
    reason: &str,
    actor_id: u64,
) -> ApiResult<CancellationResult> {
    let (current, state) = cancellation_state(transaction, subscription_id).await?;
    sqlx::query("SELECT id FROM patients WHERE id = ? FOR UPDATE")
        .bind(current.patient_id)
        .fetch_one(&mut **transaction)
        .await?;
    let old = subscription_snapshot(&current);

    let refund = if state.refund_amount > Decimal::ZERO {
        let reverses_payment_id =
            (state.source_payment_ids.len() == 1).then_some(state.source_payment_ids[0]);
        let refund_id = ledger::insert_payment(
            transaction,
            current.patient_id,
            None,
            None,
            Some(current.id),
            "refund",
            "cash",
            -state.refund_amount,
            reverses_payment_id,
            Some(reason),
            actor_id,
        )
        .await?;
        let values = json!({
            "subscription_id": current.id,
            "amount": money::normalize(-state.refund_amount),
            "method": "cash",
        });
        audit::log(
            transaction,
            actor_id,
            "payment_refunded",
            "payment",
            refund_id,
            None,
            Some(&values),
            Some(reason),
        )
        .await?;
        Some(ledger::fetch_payment(transaction, refund_id).await?)
    } else {
        None
    };

    for debt_id in &state.debt_ids_to_void {
        sqlx::query(
            "UPDATE debts SET status = 'void', voided_at = NOW(), voided_by = ?, void_reason = ?, updated_at = NOW() WHERE id = ?",
        )
        .bind(actor_id)
        .bind(reason)
        .bind(debt_id)
        .execute(&mut **transaction)
        .await?;
    }

    let replacement_debt = if state.replacement_debt_amount > Decimal::ZERO {
        let result = sqlx::query(
            "INSERT INTO debts \
             (patient_id, sponsor_id, subscription_id, kind, amount, paid_amount, status, note, created_by, created_at, updated_at) \
             VALUES (?, ?, ?, 'subscription', ?, 0, 'open', ?, ?, NOW(), NOW())",
        )
        .bind(current.patient_id)
        .bind(current.sponsor_id)
        .bind(current.id)
        .bind(state.replacement_debt_amount)
        .bind(format!("دين معدل بعد إلغاء الاشتراك #{}", current.id))
        .bind(actor_id)
        .execute(&mut **transaction)
        .await?;
        Some(ledger::fetch_debt(transaction, result.last_insert_id()).await?)
    } else {
        None
    };

    sqlx::query(
        "UPDATE subscriptions SET status = 'cancelled', cancelled_at = NOW(), cancelled_by = ?, cancel_reason = ?, refund_amount = ?, updated_at = NOW() WHERE id = ?",
    )
    .bind(actor_id)
    .bind(reason)
    .bind(state.refund_amount)
    .bind(current.id)
    .execute(&mut **transaction)
    .await?;
    let updated = fetch_locked_subscription(transaction, current.id).await?;
    let mut new = subscription_snapshot(&updated);
    let new_object = new
        .as_object_mut()
        .expect("subscription snapshot must be an object");
    new_object.insert("preview".into(), state.preview.clone());
    new_object.insert(
        "refund_payment_id".into(),
        refund
            .as_ref()
            .map(|row| Value::from(row.id))
            .unwrap_or(Value::Null),
    );
    new_object.insert(
        "voided_debt_ids".into(),
        Value::Array(
            state
                .debt_ids_to_void
                .iter()
                .copied()
                .map(Value::from)
                .collect(),
        ),
    );
    new_object.insert(
        "replacement_debt_id".into(),
        replacement_debt
            .as_ref()
            .map(|row| Value::from(row.id))
            .unwrap_or(Value::Null),
    );
    audit::log(
        transaction,
        actor_id,
        "subscription_cancelled",
        "subscription",
        current.id,
        Some(&old),
        Some(&new),
        Some(reason),
    )
    .await?;

    Ok(CancellationResult {
        subscription: subscription_value(transaction, &updated).await?,
        preview: state.preview,
        refund,
        voided_debt_ids: state.debt_ids_to_void,
        replacement_debt,
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
    let sponsor = Some(match subscription.sponsor_id {
        Some(id) => sqlx::query("SELECT id, name FROM sponsors WHERE id = ?")
            .bind(id)
            .fetch_optional(&mut **transaction)
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
    });
    let services = sqlx::query_as::<_, SubscriptionServiceRow>(
        "SELECT * FROM subscription_services WHERE subscription_id = ? ORDER BY id",
    )
    .bind(subscription.id)
    .fetch_all(&mut **transaction)
    .await?
    .iter()
    .map(resources::subscription_service_value)
    .collect::<Vec<_>>();

    Ok(resources::subscription_value(
        subscription,
        Some(json!({
            "id": patient.id,
            "name": patient.name,
            "phone": patient.phone,
        })),
        sponsor,
        Some(services),
    ))
}

pub async fn fetch_locked_subscription(
    transaction: &mut Transaction<'_, MySql>,
    id: u64,
) -> ApiResult<SubscriptionRow> {
    sqlx::query_as::<_, SubscriptionRow>("SELECT * FROM subscriptions WHERE id = ? FOR UPDATE")
        .bind(id)
        .fetch_optional(&mut **transaction)
        .await?
        .ok_or_else(ApiError::not_found)
}

fn validate_initial_payment(
    net_total: Decimal,
    paid_amount: Decimal,
    method: Option<&str>,
) -> ApiResult<()> {
    if paid_amount > net_total {
        return Err(
            ApiError::validation("المبلغ المدفوع يتجاوز قيمة الاشتراك.").field(
                "paid_amount",
                format!("أقصى مبلغ مسموح هو {}.", money::normalize(net_total)),
            ),
        );
    }
    if paid_amount > Decimal::ZERO && method.is_none() {
        return Err(
            ApiError::validation("طريقة الدفع مطلوبة عند تسجيل مبلغ مدفوع.")
                .field("method", "اختر نقدًا أو تحويلًا مصرفيًا."),
        );
    }
    Ok(())
}

async fn require_active_sponsor(
    transaction: &mut Transaction<'_, MySql>,
    sponsor_id: Option<u64>,
) -> ApiResult<()> {
    let Some(sponsor_id) = sponsor_id else {
        return Ok(());
    };
    let exists = sqlx::query("SELECT id FROM sponsors WHERE id = ? AND is_active = 1")
        .bind(sponsor_id)
        .fetch_optional(&mut **transaction)
        .await?
        .is_some();
    if !exists {
        return Err(ApiError::validation("الجهة الراعية المحددة غير متاحة.")
            .field("sponsor_id", "اختر جهة راعية نشطة."));
    }
    Ok(())
}

fn subscription_snapshot(subscription: &SubscriptionRow) -> Value {
    json!({
        "id": subscription.id,
        "patient_id": subscription.patient_id,
        "sponsor_id": subscription.sponsor_id,
        "plan_type": subscription.plan_type,
        "sessions_count": subscription.sessions_count,
        "sessions_used": subscription.sessions_used,
        "base_total": money::normalize(subscription.base_total),
        "discount": money::normalize(subscription.discount),
        "net_total": money::normalize(subscription.net_total),
        "paid_amount": money::normalize(subscription.paid_amount),
        "start_date": subscription.start_date.format("%Y-%m-%d").to_string(),
        "expected_end_date": subscription.expected_end_date.map(|date| date.format("%Y-%m-%d").to_string()),
        "end_date": subscription.end_date.map(|date| date.format("%Y-%m-%d").to_string()),
        "status": subscription.status,
        "cancelled_at": subscription.cancelled_at.map(crate::api::time::iso),
        "cancelled_by": subscription.cancelled_by,
        "cancel_reason": subscription.cancel_reason,
        "refund_amount": money::normalize(subscription.refund_amount),
        "created_by": subscription.created_by,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{db, schema, services::ledger};
    use sqlx::MySqlPool;

    async fn test_pool() -> MySqlPool {
        let database = std::env::var("RUST_MYSQL_DATABASE")
            .expect("RUST_MYSQL_DATABASE must name an isolated MySQL test database");
        assert!(
            database.starts_with("clinic_rust_test_"),
            "refusing to reset a database outside the clinic_rust_test_* namespace"
        );
        let host = std::env::var("RUST_MYSQL_HOST").unwrap_or_else(|_| "127.0.0.1".into());
        let port = std::env::var("RUST_MYSQL_PORT")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(3306);
        let username = std::env::var("RUST_MYSQL_USER").unwrap_or_else(|_| "root".into());
        let password = std::env::var("RUST_MYSQL_PASSWORD").unwrap_or_default();
        db::connect_database(&host, port, &database, &username, &password)
            .await
            .expect("connect to isolated MySQL test database")
    }

    #[tokio::test]
    #[ignore = "requires an isolated MySQL 8 database"]
    async fn subscription_financial_lifecycle_matches_laravel() {
        let pool = test_pool().await;
        schema::reset(&pool).await.expect("reset test schema");

        let actor_id = sqlx::query(
            "INSERT INTO users (name, username, password, role, is_active, created_at, updated_at) \
             VALUES ('المدير', 'admin', 'test', 'admin', 1, NOW(), NOW())",
        )
        .execute(&pool)
        .await
        .unwrap()
        .last_insert_id();
        let patient_id = sqlx::query(
            "INSERT INTO patients (name, phone, age, gender, is_active, created_by, created_at, updated_at) \
             VALUES ('مريض اختبار', '0910000000', 30, 'male', 1, ?, NOW(), NOW())",
        )
        .bind(actor_id)
        .execute(&pool)
        .await
        .unwrap()
        .last_insert_id();
        let service_id = sqlx::query(
            "INSERT INTO services (name, price, is_active, created_by, created_at, updated_at) \
             VALUES ('علاج يدوي', 100, 1, ?, NOW(), NOW())",
        )
        .bind(actor_id)
        .execute(&pool)
        .await
        .unwrap()
        .last_insert_id();

        let today = time::today();
        let monthly = CreateInput {
            patient_id,
            plan_type: "monthly".into(),
            amount: Some(Decimal::from(650)),
            service_ids: Vec::new(),
            sessions_count: None,
            discount: Decimal::ZERO,
            paid_amount: Decimal::from(250),
            method: Some("cash".into()),
            start_date: today,
            expected_end_date: None,
            end_date: Some(today + chrono::Duration::days(29)),
            sponsor_id: None,
        };
        let mut transaction = pool.begin().await.unwrap();
        let monthly_result = create(&mut transaction, &monthly, actor_id).await.unwrap();
        transaction.commit().await.unwrap();
        assert_eq!(monthly_result.subscription["base_total"], "650.000");
        assert_eq!(monthly_result.payment.unwrap().amount, Decimal::from(250));
        assert_eq!(monthly_result.debt.unwrap().amount, Decimal::from(400));

        let legacy = CreateInput {
            patient_id,
            plan_type: "legacy_package".into(),
            amount: None,
            service_ids: vec![service_id],
            sessions_count: Some(10),
            discount: Decimal::ZERO,
            paid_amount: Decimal::from(300),
            method: Some("cash".into()),
            start_date: today,
            expected_end_date: None,
            end_date: None,
            sponsor_id: None,
        };
        let mut transaction = pool.begin().await.unwrap();
        let legacy_result = create(&mut transaction, &legacy, actor_id).await.unwrap();
        let legacy_id = legacy_result.subscription["id"].as_u64().unwrap();
        let debt_id = legacy_result.debt.unwrap().id;
        transaction.commit().await.unwrap();
        sqlx::query("UPDATE subscriptions SET sessions_used = 4 WHERE id = ?")
            .bind(legacy_id)
            .execute(&pool)
            .await
            .unwrap();

        let mut transaction = pool.begin().await.unwrap();
        ledger::settle_debts(
            &mut transaction,
            Decimal::from(200),
            "cash",
            &[(debt_id, Decimal::from(200))],
            actor_id,
        )
        .await
        .unwrap();
        transaction.commit().await.unwrap();

        let mut transaction = pool.begin().await.unwrap();
        let (_, state) = cancellation_state(&mut transaction, legacy_id)
            .await
            .unwrap();
        assert_eq!(state.preview["paid_total"], "500.000");
        assert_eq!(state.preview["consumed_value"], "400.000");
        assert_eq!(state.preview["refund_amount"], "100.000");
        assert_eq!(state.preview["debt_voided_amount"], "500.000");
        transaction.commit().await.unwrap();

        let mut transaction = pool.begin().await.unwrap();
        let result = cancel(&mut transaction, legacy_id, "إلغاء اختبار", actor_id)
            .await
            .unwrap();
        transaction.commit().await.unwrap();
        assert_eq!(result.subscription["status"], "cancelled");
        assert_eq!(result.refund.unwrap().amount, Decimal::from(-100));
        assert!(result.replacement_debt.is_none());
        let debt_status: String = sqlx::query_scalar("SELECT status FROM debts WHERE id = ?")
            .bind(debt_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(debt_status, "void");
    }
}
