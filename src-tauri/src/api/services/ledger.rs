use crate::api::{
    audit,
    error::{ApiError, ApiResult},
    money,
    rows::{DebtRow, DoctorPayoutRow, PaymentAllocationRow, PaymentRow, WalletTransactionRow},
};
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde_json::{json, Value};
use sqlx::{MySql, Transaction};

pub async fn wallet_balance(
    transaction: &mut Transaction<'_, MySql>,
    patient_id: u64,
) -> ApiResult<Decimal> {
    sqlx::query_scalar::<_, Option<Decimal>>(
        "SELECT SUM(amount) FROM wallet_transactions WHERE patient_id = ?",
    )
    .bind(patient_id)
    .fetch_one(&mut **transaction)
    .await
    .map(|value| value.unwrap_or(Decimal::ZERO))
    .map_err(Into::into)
}

pub async fn top_up(
    transaction: &mut Transaction<'_, MySql>,
    patient_id: u64,
    amount: Decimal,
    method: &str,
    note: Option<&str>,
    actor_id: u64,
) -> ApiResult<(Decimal, WalletTransactionRow, PaymentRow)> {
    lock_patient(transaction, patient_id).await?;
    let balance = wallet_balance(transaction, patient_id).await? + amount;
    let payment_id = insert_payment(
        transaction,
        patient_id,
        None,
        None,
        None,
        "wallet_topup",
        method,
        amount,
        None,
        note,
        actor_id,
    )
    .await?;
    let payment_audit = json!({
        "patient_id": patient_id,
        "type": "wallet_topup",
        "method": method,
        "amount": money::normalize(amount),
    });
    audit::log(
        transaction,
        actor_id,
        "payment_recorded",
        "payment",
        payment_id,
        None,
        Some(&payment_audit),
        None,
    )
    .await?;

    let result = sqlx::query(
        "INSERT INTO wallet_transactions \
         (patient_id, payment_id, type, amount, balance_after, note, created_by, created_at, updated_at) \
         VALUES (?, ?, 'topup', ?, ?, ?, ?, NOW(), NOW())",
    )
    .bind(patient_id)
    .bind(payment_id)
    .bind(amount)
    .bind(balance)
    .bind(note)
    .bind(actor_id)
    .execute(&mut **transaction)
    .await?;
    let movement_id = result.last_insert_id();
    let movement_audit = json!({
        "patient_id": patient_id,
        "amount": money::normalize(amount),
        "balance_after": money::normalize(balance),
        "payment_id": payment_id,
    });
    audit::log(
        transaction,
        actor_id,
        "wallet_topped_up",
        "wallet_transaction",
        movement_id,
        None,
        Some(&movement_audit),
        None,
    )
    .await?;

    Ok((
        balance,
        fetch_wallet(transaction, movement_id).await?,
        fetch_payment(transaction, payment_id).await?,
    ))
}

pub async fn debit(
    transaction: &mut Transaction<'_, MySql>,
    patient_id: u64,
    session_id: u64,
    session_addition_id: Option<u64>,
    amount: Decimal,
    actor_id: u64,
) -> ApiResult<Option<WalletTransactionRow>> {
    if amount.is_zero() {
        return Ok(None);
    }
    let balance = wallet_balance(transaction, patient_id).await?;
    if balance < amount {
        return Err(
            ApiError::validation("الرصيد غير كافٍ. اشحن رصيد المريض أولًا ثم أعد المحاولة.").field(
                "settlement.wallet_amount",
                "الرصيد المتاح أقل من المبلغ المطلوب.",
            ),
        );
    }
    let result = sqlx::query(
        "INSERT INTO wallet_transactions \
         (patient_id, session_id, session_addition_id, type, amount, balance_after, created_by, created_at, updated_at) \
         VALUES (?, ?, ?, 'debit', ?, ?, ?, NOW(), NOW())",
    )
    .bind(patient_id)
    .bind(session_id)
    .bind(session_addition_id)
    .bind(-amount)
    .bind(balance - amount)
    .bind(actor_id)
    .execute(&mut **transaction)
    .await?;
    Ok(Some(
        fetch_wallet(transaction, result.last_insert_id()).await?,
    ))
}

pub async fn create_opening_debt(
    transaction: &mut Transaction<'_, MySql>,
    patient_id: u64,
    amount: Decimal,
    sponsor_id: Option<u64>,
    note: Option<&str>,
    actor_id: u64,
) -> ApiResult<DebtRow> {
    lock_patient(transaction, patient_id).await?;
    if let Some(id) = sponsor_id {
        require_active(transaction, "sponsors", id, "sponsor_id").await?;
    }
    let result = sqlx::query(
        "INSERT INTO debts \
         (patient_id, sponsor_id, kind, amount, paid_amount, status, note, created_by, created_at, updated_at) \
         VALUES (?, ?, 'opening', ?, 0, 'open', ?, ?, NOW(), NOW())",
    )
    .bind(patient_id)
    .bind(sponsor_id)
    .bind(amount)
    .bind(note)
    .bind(actor_id)
    .execute(&mut **transaction)
    .await?;
    let id = result.last_insert_id();
    let debt = fetch_debt(transaction, id).await?;
    let values = debt_snapshot(&debt);
    audit::log(
        transaction,
        actor_id,
        "opening_debt_created",
        "debt",
        id,
        None,
        Some(&values),
        None,
    )
    .await?;
    Ok(debt)
}

pub async fn settle_debts(
    transaction: &mut Transaction<'_, MySql>,
    amount: Decimal,
    method: &str,
    allocations: &[(u64, Decimal)],
    actor_id: u64,
) -> ApiResult<(PaymentRow, Vec<PaymentAllocationRow>)> {
    let allocation_total = money::sum(allocations.iter().map(|(_, amount)| *amount));
    if allocation_total != amount {
        return Err(
            ApiError::validation("يجب تخصيص كامل مبلغ الدفعة على الديون.")
                .field("allocations", "مجموع التخصيصات يجب أن يساوي مبلغ الدفعة."),
        );
    }
    let mut ids = allocations.iter().map(|(id, _)| *id).collect::<Vec<_>>();
    ids.sort_unstable();
    ids.dedup();
    if ids.len() != allocations.len() {
        return Err(ApiError::validation("البيانات المرسلة غير صحيحة.")
            .field("allocations", "لا يجوز تكرار الدين نفسه."));
    }
    let mut debts = Vec::new();
    for id in ids {
        let debt = sqlx::query_as::<_, DebtRow>("SELECT * FROM debts WHERE id = ? FOR UPDATE")
            .bind(id)
            .fetch_optional(&mut **transaction)
            .await?
            .ok_or_else(|| {
                ApiError::validation("تعذر العثور على أحد الديون المحددة.")
                    .field("allocations", "أحد الديون غير موجود.")
            })?;
        debts.push(debt);
    }
    let patient_id = debts
        .first()
        .map(|debt| debt.patient_id)
        .ok_or_else(|| ApiError::validation("حدد دينًا واحدًا على الأقل."))?;
    if debts.iter().any(|debt| debt.patient_id != patient_id) {
        return Err(ApiError::validation("يجب أن تخص الدفعة مريضًا واحدًا.")
            .field("allocations", "لا يمكن جمع ديون مرضى مختلفين في دفعة واحدة."));
    }
    for (index, (debt_id, allocation)) in allocations.iter().enumerate() {
        let debt = debts.iter().find(|debt| debt.id == *debt_id).unwrap();
        let remaining = debt.amount - debt.paid_amount;
        if matches!(debt.status.as_str(), "void" | "paid") {
            return Err(ApiError::validation("أحد الديون غير متاح للتسوية.").field(
                format!("allocations.{index}.debt_id"),
                "الدين مسدد أو ملغى.",
            ));
        }
        if *allocation > remaining {
            return Err(
                ApiError::validation("مبلغ التخصيص يتجاوز المتبقي على الدين.").field(
                    format!("allocations.{index}.amount"),
                    format!("المتبقي على الدين هو {}.", money::normalize(remaining)),
                ),
            );
        }
    }

    let payment_id = insert_payment(
        transaction,
        patient_id,
        None,
        None,
        None,
        "debt_payment",
        method,
        amount,
        None,
        None,
        actor_id,
    )
    .await?;
    let mut created = Vec::new();
    for (debt_id, allocation) in allocations {
        let debt = debts.iter().find(|debt| debt.id == *debt_id).unwrap();
        let paid = debt.paid_amount + *allocation;
        let status = if paid == debt.amount {
            "paid"
        } else {
            "partial"
        };
        let result = sqlx::query(
            "INSERT INTO payment_allocations (payment_id, debt_id, amount, created_by, created_at, updated_at) \
             VALUES (?, ?, ?, ?, NOW(), NOW())",
        )
        .bind(payment_id)
        .bind(debt_id)
        .bind(allocation)
        .bind(actor_id)
        .execute(&mut **transaction)
        .await?;
        sqlx::query(
            "UPDATE debts SET paid_amount = ?, status = ?, updated_at = NOW() WHERE id = ?",
        )
        .bind(paid)
        .bind(status)
        .bind(debt_id)
        .execute(&mut **transaction)
        .await?;
        created.push(fetch_allocation(transaction, result.last_insert_id()).await?);
    }
    let values = json!({
        "amount": money::normalize(amount),
        "allocations": allocations.iter().map(|(debt_id, amount)| json!({
            "debt_id": debt_id,
            "amount": money::normalize(*amount),
        })).collect::<Vec<_>>(),
    });
    audit::log(
        transaction,
        actor_id,
        "debt_payment_recorded",
        "payment",
        payment_id,
        None,
        Some(&values),
        None,
    )
    .await?;

    Ok((fetch_payment(transaction, payment_id).await?, created))
}

pub async fn refund(
    transaction: &mut Transaction<'_, MySql>,
    patient_id: u64,
    payment_id: u64,
    amount: Decimal,
    method: &str,
    reason: &str,
    actor_id: u64,
) -> ApiResult<PaymentRow> {
    let original = sqlx::query_as::<_, PaymentRow>(
        "SELECT * FROM payments WHERE id = ? AND patient_id = ? FOR UPDATE",
    )
    .bind(payment_id)
    .bind(patient_id)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or_else(ApiError::not_found)?;
    if original.r#type == "refund" || original.amount <= Decimal::ZERO {
        return Err(ApiError::validation("لا يمكن استرجاع هذه الدفعة.")
            .field("payment_id", "الدفعة المحددة ليست دفعة قابلة للاسترجاع."));
    }
    let attributed_to_subscription = original.subscription_id.is_some()
        || sqlx::query_scalar::<_, u64>(
            "SELECT CAST(COUNT(*) AS UNSIGNED) FROM payment_allocations pa \
             INNER JOIN debts d ON d.id = pa.debt_id \
             WHERE pa.payment_id = ? AND d.subscription_id IS NOT NULL",
        )
        .bind(original.id)
        .fetch_one(&mut **transaction)
        .await?
            > 0;
    if attributed_to_subscription {
        return Err(
            ApiError::validation("استرجاع دفعات الاشتراك يتم من خلال إلغاء الاشتراك فقط.").field(
                "payment_id",
                "استخدم إجراء إلغاء الاشتراك لتسجيل الاسترجاع وتخصيصه ماليًا.",
            ),
        );
    }
    let already_refunded = sqlx::query_scalar::<_, Option<Decimal>>(
        "SELECT SUM(ABS(amount)) FROM payments WHERE reverses_payment_id = ? AND type = 'refund'",
    )
    .bind(original.id)
    .fetch_one(&mut **transaction)
    .await?
    .unwrap_or(Decimal::ZERO);
    let available = original.amount - already_refunded;
    if amount > available {
        return Err(
            ApiError::validation("مبلغ الاسترجاع يتجاوز المبلغ المتاح.").field(
                "amount",
                format!("الحد المتاح للاسترجاع هو {}.", money::normalize(available)),
            ),
        );
    }
    if original.r#type == "wallet_topup" {
        lock_patient(transaction, original.patient_id).await?;
        if wallet_balance(transaction, original.patient_id).await? < amount {
            return Err(
                ApiError::validation("رصيد المريض الحالي لا يكفي لاسترجاع هذا المبلغ.")
                    .field("amount", "رصيد المريض الحالي لا يكفي لاسترجاع هذا المبلغ."),
            );
        }
    }
    let refund_id = insert_payment(
        transaction,
        original.patient_id,
        original.session_id,
        None,
        original.subscription_id,
        "refund",
        method,
        -amount,
        Some(original.id),
        Some(reason),
        actor_id,
    )
    .await?;
    let values = json!({
        "payment_id": original.id,
        "amount": money::normalize(-amount),
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

    if original.r#type == "wallet_topup" {
        let balance = wallet_balance(transaction, original.patient_id).await?;
        let result = sqlx::query(
            "INSERT INTO wallet_transactions \
             (patient_id, payment_id, type, amount, balance_after, note, created_by, created_at, updated_at) \
             VALUES (?, ?, 'debit', ?, ?, ?, ?, NOW(), NOW())",
        )
        .bind(original.patient_id)
        .bind(refund_id)
        .bind(-amount)
        .bind(balance - amount)
        .bind(reason)
        .bind(actor_id)
        .execute(&mut **transaction)
        .await?;
        let movement_values = json!({
            "patient_id": original.patient_id,
            "refund_payment_id": refund_id,
            "amount": money::normalize(-amount),
            "balance_after": money::normalize(balance - amount),
        });
        audit::log(
            transaction,
            actor_id,
            "wallet_topup_refunded",
            "wallet_transaction",
            result.last_insert_id(),
            None,
            Some(&movement_values),
            Some(reason),
        )
        .await?;
    }

    fetch_payment(transaction, refund_id).await
}

pub async fn doctor_payout(
    transaction: &mut Transaction<'_, MySql>,
    doctor_id: u64,
    amount: Decimal,
    date: NaiveDate,
    method: &str,
    note: Option<&str>,
    actor_id: u64,
) -> ApiResult<DoctorPayoutRow> {
    sqlx::query("SELECT id FROM doctors WHERE id = ? FOR UPDATE")
        .bind(doctor_id)
        .fetch_optional(&mut **transaction)
        .await?
        .ok_or_else(ApiError::not_found)?;
    let earned = sqlx::query_scalar::<_, Option<Decimal>>(
        "SELECT SUM(commission_amount) FROM sessions WHERE doctor_id = ? AND type = 'individual' AND status = 'closed' AND commission_amount IS NOT NULL",
    )
    .bind(doctor_id)
    .fetch_one(&mut **transaction)
    .await?
    .unwrap_or(Decimal::ZERO);
    let paid = sqlx::query_scalar::<_, Option<Decimal>>(
        "SELECT SUM(amount) FROM doctor_payouts WHERE doctor_id = ?",
    )
    .bind(doctor_id)
    .fetch_one(&mut **transaction)
    .await?
    .unwrap_or(Decimal::ZERO);
    let remaining = earned - paid;
    if amount > remaining {
        return Err(
            ApiError::validation("مبلغ الصرف يتجاوز مستحقات الطبيب المتبقية.").field(
                "amount",
                format!("المستحق المتبقي هو {}.", money::normalize(remaining)),
            ),
        );
    }
    let result = sqlx::query(
        "INSERT INTO doctor_payouts (doctor_id, amount, date, method, note, created_by, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, NOW(), NOW())",
    )
    .bind(doctor_id)
    .bind(amount)
    .bind(date)
    .bind(method)
    .bind(note)
    .bind(actor_id)
    .execute(&mut **transaction)
    .await?;
    let id = result.last_insert_id();
    let payout = fetch_payout(transaction, id).await?;
    let values = json!({
        "id": payout.id,
        "doctor_id": payout.doctor_id,
        "amount": money::normalize(payout.amount),
        "date": payout.date.format("%Y-%m-%d").to_string(),
        "method": payout.method,
        "note": payout.note,
        "created_by": payout.created_by,
    });
    audit::log(
        transaction,
        actor_id,
        "doctor_payout_created",
        "doctor_payout",
        id,
        None,
        Some(&values),
        None,
    )
    .await?;
    Ok(payout)
}

pub async fn insert_payment(
    transaction: &mut Transaction<'_, MySql>,
    patient_id: u64,
    session_id: Option<u64>,
    session_addition_id: Option<u64>,
    subscription_id: Option<u64>,
    payment_type: &str,
    method: &str,
    amount: Decimal,
    reverses_payment_id: Option<u64>,
    note: Option<&str>,
    actor_id: u64,
) -> ApiResult<u64> {
    let result = sqlx::query(
        "INSERT INTO payments \
         (patient_id, session_id, session_addition_id, subscription_id, type, method, amount, reverses_payment_id, note, created_by, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NOW(), NOW())",
    )
    .bind(patient_id)
    .bind(session_id)
    .bind(session_addition_id)
    .bind(subscription_id)
    .bind(payment_type)
    .bind(method)
    .bind(amount)
    .bind(reverses_payment_id)
    .bind(note)
    .bind(actor_id)
    .execute(&mut **transaction)
    .await?;
    Ok(result.last_insert_id())
}

pub async fn fetch_payment(
    transaction: &mut Transaction<'_, MySql>,
    id: u64,
) -> ApiResult<PaymentRow> {
    sqlx::query_as::<_, PaymentRow>("SELECT * FROM payments WHERE id = ?")
        .bind(id)
        .fetch_one(&mut **transaction)
        .await
        .map_err(Into::into)
}

pub async fn fetch_debt(transaction: &mut Transaction<'_, MySql>, id: u64) -> ApiResult<DebtRow> {
    sqlx::query_as::<_, DebtRow>("SELECT * FROM debts WHERE id = ?")
        .bind(id)
        .fetch_one(&mut **transaction)
        .await
        .map_err(Into::into)
}

async fn fetch_wallet(
    transaction: &mut Transaction<'_, MySql>,
    id: u64,
) -> ApiResult<WalletTransactionRow> {
    sqlx::query_as::<_, WalletTransactionRow>("SELECT * FROM wallet_transactions WHERE id = ?")
        .bind(id)
        .fetch_one(&mut **transaction)
        .await
        .map_err(Into::into)
}

async fn fetch_allocation(
    transaction: &mut Transaction<'_, MySql>,
    id: u64,
) -> ApiResult<PaymentAllocationRow> {
    sqlx::query_as::<_, PaymentAllocationRow>("SELECT * FROM payment_allocations WHERE id = ?")
        .bind(id)
        .fetch_one(&mut **transaction)
        .await
        .map_err(Into::into)
}

async fn fetch_payout(
    transaction: &mut Transaction<'_, MySql>,
    id: u64,
) -> ApiResult<DoctorPayoutRow> {
    sqlx::query_as::<_, DoctorPayoutRow>("SELECT * FROM doctor_payouts WHERE id = ?")
        .bind(id)
        .fetch_one(&mut **transaction)
        .await
        .map_err(Into::into)
}

async fn lock_patient(transaction: &mut Transaction<'_, MySql>, patient_id: u64) -> ApiResult<()> {
    sqlx::query("SELECT id FROM patients WHERE id = ? FOR UPDATE")
        .bind(patient_id)
        .fetch_optional(&mut **transaction)
        .await?
        .ok_or_else(ApiError::not_found)?;
    Ok(())
}

async fn require_active(
    transaction: &mut Transaction<'_, MySql>,
    table: &str,
    id: u64,
    field: &str,
) -> ApiResult<()> {
    let query = format!("SELECT id FROM `{table}` WHERE id = ? AND is_active = 1");
    if sqlx::query(&query)
        .bind(id)
        .fetch_optional(&mut **transaction)
        .await?
        .is_none()
    {
        return Err(ApiError::validation("البيانات المرسلة غير صحيحة.")
            .field(field, "السجل المحدد غير موجود أو غير نشط."));
    }
    Ok(())
}

fn debt_snapshot(debt: &DebtRow) -> Value {
    json!({
        "id": debt.id,
        "patient_id": debt.patient_id,
        "sponsor_id": debt.sponsor_id,
        "session_id": debt.session_id,
        "subscription_id": debt.subscription_id,
        "kind": debt.kind,
        "amount": money::normalize(debt.amount),
        "paid_amount": money::normalize(debt.paid_amount),
        "status": debt.status,
        "note": debt.note,
        "created_by": debt.created_by,
    })
}
