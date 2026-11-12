use crate::api::{
    audit,
    config::{ApiConfig, CommissionBase},
    error::{ApiError, ApiResult},
    money, resources,
    rows::{
        DebtRow, DoctorRow, PatientRow, PaymentRow, ServiceRow, SessionAdditionRow, SessionRow,
        SessionServiceRow, SubscriptionRow, SubscriptionUsageRow, WalletTransactionRow,
    },
    services::{commissions, ledger},
    time,
};
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde_json::{json, Map, Value};
use sqlx::{MySql, Row, Transaction};
use std::{
    collections::{HashMap, HashSet},
    str::FromStr,
};
use uuid::Uuid;

#[derive(Debug)]
pub struct SessionOperationResult {
    pub session: Value,
    pub effects: Effects,
    pub replayed: bool,
}

#[derive(Debug)]
pub struct AdditionResult {
    pub session: Value,
    pub addition: SessionAdditionRow,
    pub effects: Effects,
}

#[derive(Debug)]
pub struct CancellationResult {
    pub session: Value,
    pub reversals: Value,
}

#[derive(Debug)]
pub struct CorrectionResult {
    pub session: Value,
    pub financial_impact: Value,
}

#[derive(Debug, Default)]
pub struct Effects {
    pub subscription_usage: Option<SubscriptionUsageRow>,
    pub wallet_transaction: Option<WalletTransactionRow>,
    pub payment: Option<PaymentRow>,
    pub debt: Option<DebtRow>,
}

impl Effects {
    pub fn value(&self) -> Value {
        json!({
            "subscription_usage": self.subscription_usage.as_ref().map(resources::subscription_usage_value),
            "wallet_transaction": self.wallet_transaction.as_ref().map(resources::wallet_transaction_value),
            "payment": self.payment.as_ref().map(|row| resources::payment_value(row, None)),
            "debt": self.debt.as_ref().map(resources::debt_value),
        })
    }
}

#[derive(Debug, Clone)]
struct Settlement {
    wallet_amount: Decimal,
    paid_amount: Decimal,
    debt_amount: Decimal,
    method: Option<String>,
    sponsor_id: Option<u64>,
}

impl Default for Settlement {
    fn default() -> Self {
        Self {
            wallet_amount: Decimal::ZERO,
            paid_amount: Decimal::ZERO,
            debt_amount: Decimal::ZERO,
            method: None,
            sponsor_id: None,
        }
    }
}

#[derive(Debug, Clone)]
struct ServiceItemInput {
    service_id: u64,
    item_type: String,
    extra_units: u16,
}

#[derive(Debug, Clone)]
struct ResolvedService {
    service_id: u64,
    service_name: String,
    price: Decimal,
    base_price: Decimal,
    included_units: u16,
    extra_units: u16,
    unit_name: Option<String>,
    extra_unit_price: Option<Decimal>,
    item_type: String,
}

impl ResolvedService {
    fn from_snapshot(row: &SessionServiceRow) -> Self {
        Self {
            service_id: row.service_id,
            service_name: row.service_name.clone(),
            price: row.price,
            base_price: row.base_price.unwrap_or(row.price),
            included_units: row.included_units,
            extra_units: row.extra_units,
            unit_name: row.unit_name.clone(),
            extra_unit_price: row.extra_unit_price,
            item_type: row.item_type.clone(),
        }
    }
}

#[derive(Debug)]
struct PreparedSession {
    patient_id: u64,
    doctor_id: u64,
    subscription_id: Option<u64>,
    session_type: String,
    services_total: Decimal,
    discount: Decimal,
    net_total: Decimal,
    coverage: String,
    settlement: Settlement,
    services: Vec<ResolvedService>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct CommissionSnapshot {
    rate: Decimal,
    base_amount: Decimal,
    amount: Decimal,
}

fn session_commission(
    is_subscription_session: bool,
    commission_base: CommissionBase,
    services_total: Decimal,
    net_total: Decimal,
    rate: Decimal,
) -> Option<CommissionSnapshot> {
    if is_subscription_session {
        return None;
    }
    let base_amount = match commission_base {
        CommissionBase::Gross => services_total,
        CommissionBase::Net => net_total,
    };
    Some(CommissionSnapshot {
        rate,
        base_amount,
        amount: money::percentage(base_amount, rate),
    })
}

pub async fn create_and_close(
    transaction: &mut Transaction<'_, MySql>,
    payload: &Value,
    actor_id: u64,
    config: &ApiConfig,
) -> ApiResult<SessionOperationResult> {
    let data = object(payload)?;
    let session_id = create_open(transaction, data, actor_id).await?;
    close(transaction, session_id, actor_id, config).await
}

pub async fn update(
    transaction: &mut Transaction<'_, MySql>,
    session_id: u64,
    payload: &Value,
    actor_id: u64,
) -> ApiResult<Value> {
    let data = object(payload)?;
    let current = fetch_locked_session(transaction, session_id).await?;
    let is_subscription_session = validate_session_relationship(&current)?;
    if current.status != "open" {
        let role = user_role(transaction, actor_id).await?;
        if role == "receptionist" && current.status == "closed" {
            return Err(ApiError::forbidden(
                "لا يستطيع موظف الاستقبال تعديل جلسة مغلقة.",
            ));
        }
        return Err(ApiError::conflict("يمكن تعديل الجلسة المفتوحة فقط."));
    }
    if is_subscription_session {
        validate_subscription_session_update_input(data)?;
    }

    let old = session_snapshot(transaction, &current).await?;
    let prepared = prepare_open_session(transaction, data, Some(&current)).await?;
    update_prepared_session(transaction, session_id, &prepared).await?;
    replace_snapshots(transaction, session_id, &prepared.services, actor_id).await?;
    let updated = fetch_locked_session(transaction, session_id).await?;
    let new = session_snapshot(transaction, &updated).await?;
    audit::log(
        transaction,
        actor_id,
        "session_updated",
        "session",
        session_id,
        Some(&old),
        Some(&new),
        None,
    )
    .await?;
    session_value(transaction, &updated, false).await
}

pub async fn close(
    transaction: &mut Transaction<'_, MySql>,
    session_id: u64,
    actor_id: u64,
    config: &ApiConfig,
) -> ApiResult<SessionOperationResult> {
    let current = fetch_locked_session(transaction, session_id).await?;
    let is_subscription_session = validate_session_relationship(&current)?;
    if current.status == "closed" {
        return Ok(SessionOperationResult {
            session: session_value(transaction, &current, false).await?,
            effects: existing_effects(transaction, &current).await?,
            replayed: true,
        });
    }
    if current.status == "cancelled" {
        return Err(ApiError::conflict("لا يمكن إغلاق جلسة ملغاة."));
    }

    lock_patient(transaction, current.patient_id, false).await?;
    let effects = if is_subscription_session {
        Effects {
            subscription_usage: consume_subscription(transaction, &current, actor_id).await?,
            ..Effects::default()
        }
    } else {
        apply_settlement(
            transaction,
            &current,
            None,
            &Settlement {
                wallet_amount: current.wallet_amount,
                paid_amount: current.paid_amount,
                debt_amount: current.debt_amount,
                method: current.payment_method.clone(),
                sponsor_id: current.sponsor_id,
            },
            "session_payment",
            actor_id,
        )
        .await?
    };

    let commission = if is_subscription_session {
        None
    } else {
        let doctor = sqlx::query_as::<_, DoctorRow>("SELECT * FROM doctors WHERE id = ?")
            .bind(current.doctor_id)
            .fetch_optional(&mut **transaction)
            .await?
            .ok_or_else(ApiError::not_found)?;
        session_commission(
            false,
            config.commission_base,
            current.services_total,
            current.net_total,
            doctor.commission_rate,
        )
    };
    sqlx::query(
        "UPDATE sessions SET status = 'closed', closed_at = NOW(), closed_by = ?, \
         commission_rate = ?, commission_base_amount = ?, commission_amount = ?, updated_at = NOW() WHERE id = ?",
    )
    .bind(actor_id)
    .bind(commission.map(|snapshot| snapshot.rate))
    .bind(commission.map(|snapshot| snapshot.base_amount))
    .bind(commission.map(|snapshot| snapshot.amount))
    .bind(session_id)
    .execute(&mut **transaction)
    .await?;

    let updated = fetch_locked_session(transaction, session_id).await?;
    let new = session_snapshot(transaction, &updated).await?;
    let old = json!({ "status": "open" });
    audit::log(
        transaction,
        actor_id,
        "session_closed",
        "session",
        session_id,
        Some(&old),
        Some(&new),
        None,
    )
    .await?;

    Ok(SessionOperationResult {
        session: session_value(transaction, &updated, false).await?,
        effects,
        replayed: false,
    })
}

pub async fn add_services(
    transaction: &mut Transaction<'_, MySql>,
    session_id: u64,
    payload: &Value,
    actor_id: u64,
) -> ApiResult<AdditionResult> {
    let data = object(payload)?;
    let current = fetch_locked_session(transaction, session_id).await?;
    let is_subscription_session = validate_session_relationship(&current)?;
    if is_subscription_session {
        return Err(
            ApiError::validation("لا يمكن إضافة خدمات إلى جلسة اشتراك.").field(
                "services",
                "خدمات جلسة الاشتراك ثابتة ولا تقبل إضافات بعد تسجيل الجلسة.",
            ),
        );
    }
    if current.status != "closed" {
        return Err(ApiError::conflict("يمكن إضافة خدمات إلى جلسة مغلقة فقط."));
    }
    let old = session_snapshot(transaction, &current).await?;
    let inputs = parse_service_items(data, true)?;
    let services = resolve_service_items(transaction, &inputs, true).await?;
    let addition_total = money::sum(services.iter().map(|service| service.price));
    lock_patient(transaction, current.patient_id, false).await?;

    let settlement = parse_settlement(data, None, true)?;
    if money::sum([
        settlement.wallet_amount,
        settlement.paid_amount,
        settlement.debt_amount,
    ]) != addition_total
    {
        return Err(
            ApiError::validation("مجموع تسوية الخدمات الإضافية يجب أن يساوي قيمتها.")
                .field("settlement", "راجع مبالغ الرصيد والمدفوع والدين."),
        );
    }
    validate_paid_method(&settlement)?;
    validate_sponsor_if_present(transaction, data, &settlement, true).await?;
    let addition_coverage = coverage(&settlement);
    let result = sqlx::query(
        "INSERT INTO session_additions \
         (session_id, services_total, net_total, coverage, wallet_amount, paid_amount, debt_amount, payment_method, sponsor_id, created_by, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NOW(), NOW())",
    )
    .bind(session_id)
    .bind(addition_total)
    .bind(addition_total)
    .bind(&addition_coverage)
    .bind(settlement.wallet_amount)
    .bind(settlement.paid_amount)
    .bind(settlement.debt_amount)
    .bind(&settlement.method)
    .bind(settlement.sponsor_id)
    .bind(actor_id)
    .execute(&mut **transaction)
    .await?;
    let addition_id = result.last_insert_id();
    insert_snapshots(
        transaction,
        session_id,
        Some(addition_id),
        &services,
        actor_id,
        true,
    )
    .await?;

    let effects = apply_settlement(
        transaction,
        &current,
        Some(addition_id),
        &settlement,
        "adjustment",
        actor_id,
    )
    .await?;

    let wallet_amount = money::round(current.wallet_amount + settlement.wallet_amount);
    let paid_amount = money::round(current.paid_amount + settlement.paid_amount);
    let debt_amount = money::round(current.debt_amount + settlement.debt_amount);
    let payment_method = if settlement.paid_amount > Decimal::ZERO {
        if current.paid_amount.is_zero() || current.payment_method == settlement.method {
            settlement.method.clone()
        } else {
            None
        }
    } else {
        current.payment_method.clone()
    };
    let total_settlement = Settlement {
        wallet_amount,
        paid_amount,
        debt_amount,
        method: payment_method.clone(),
        sponsor_id: current.sponsor_id,
    };
    let final_coverage = coverage(&total_settlement);
    let commission_rate = current.commission_rate.unwrap_or(Decimal::ZERO);
    let commission_delta = money::percentage(addition_total, commission_rate);
    let commission_base_amount = Some(money::round(
        current.commission_base_amount.unwrap_or(Decimal::ZERO) + addition_total,
    ));
    let commission_amount = Some(money::round(
        current.commission_amount.unwrap_or(Decimal::ZERO) + commission_delta,
    ));
    sqlx::query(
        "UPDATE sessions SET services_total = ?, net_total = ?, wallet_amount = ?, paid_amount = ?, debt_amount = ?, \
         coverage = ?, payment_method = ?, commission_base_amount = ?, commission_amount = ?, updated_at = NOW() WHERE id = ?",
    )
    .bind(money::round(current.services_total + addition_total))
    .bind(money::round(current.net_total + addition_total))
    .bind(wallet_amount)
    .bind(paid_amount)
    .bind(debt_amount)
    .bind(final_coverage)
    .bind(payment_method)
    .bind(commission_base_amount)
    .bind(commission_amount)
    .bind(session_id)
    .execute(&mut **transaction)
    .await?;

    let updated = fetch_locked_session(transaction, session_id).await?;
    let mut new = session_snapshot(transaction, &updated).await?;
    new.as_object_mut()
        .expect("session snapshot is an object")
        .insert("addition_id".into(), Value::from(addition_id));
    audit::log(
        transaction,
        actor_id,
        "session_services_added",
        "session",
        session_id,
        Some(&old),
        Some(&new),
        None,
    )
    .await?;
    let addition =
        sqlx::query_as::<_, SessionAdditionRow>("SELECT * FROM session_additions WHERE id = ?")
            .bind(addition_id)
            .fetch_one(&mut **transaction)
            .await?;

    Ok(AdditionResult {
        session: session_value(transaction, &updated, false).await?,
        addition,
        effects,
    })
}

pub async fn cancel(
    transaction: &mut Transaction<'_, MySql>,
    session_id: u64,
    reason: &str,
    actor_id: u64,
) -> ApiResult<CancellationResult> {
    let current = fetch_locked_session(transaction, session_id).await?;
    let is_subscription_session = validate_session_relationship(&current)?;
    if current.status == "cancelled" {
        return Err(ApiError::conflict("تم إلغاء الجلسة بالفعل."));
    }
    let role = user_role(transaction, actor_id).await?;
    if current.status == "closed" && role != "admin" {
        return Err(ApiError::forbidden(
            "إلغاء جلسة مغلقة متاح لمدير النظام فقط.",
        ));
    }

    let old = session_snapshot(transaction, &current).await?;
    let was_closed = current.status == "closed";
    let effective_current =
        commissions::effective_session_transaction(transaction, &current).await?;
    let mut reversals = if was_closed {
        reverse_closed_effects(transaction, &current, reason, actor_id).await?
    } else {
        Value::Array(Vec::new())
    };
    let remove_commission = was_closed && !is_subscription_session;
    if remove_commission {
        reversals
            .as_object_mut()
            .expect("closed-session reversals are an object")
            .insert(
                "commission_removed".into(),
                json!({
                    "rate": money::normalize(current.commission_rate.unwrap_or(Decimal::ZERO)),
                    "base_amount": money::normalize(effective_current.commission_base_amount.unwrap_or(Decimal::ZERO)),
                    "amount": money::normalize(effective_current.commission_amount.unwrap_or(Decimal::ZERO)),
                }),
            );
    }

    sqlx::query(
        "UPDATE sessions SET status = 'cancelled', cancelled_at = NOW(), cancelled_by = ?, cancel_reason = ?, \
         commission_rate = ?, commission_base_amount = ?, commission_amount = ?, updated_at = NOW() WHERE id = ?",
    )
    .bind(actor_id)
    .bind(reason)
    .bind(if remove_commission {
        None
    } else {
        current.commission_rate
    })
    .bind(if remove_commission {
        None
    } else {
        current.commission_base_amount
    })
    .bind(if remove_commission {
        None
    } else {
        current.commission_amount
    })
    .bind(session_id)
    .execute(&mut **transaction)
    .await?;
    let updated = fetch_locked_session(transaction, session_id).await?;
    let new = session_snapshot(transaction, &updated).await?;
    audit::log(
        transaction,
        actor_id,
        "session_cancelled",
        "session",
        session_id,
        Some(&old),
        Some(&new),
        Some(reason),
    )
    .await?;

    Ok(CancellationResult {
        session: session_value(transaction, &updated, false).await?,
        reversals,
    })
}

pub async fn correct(
    transaction: &mut Transaction<'_, MySql>,
    session_id: u64,
    changes: &Map<String, Value>,
    reason: &str,
    actor_id: u64,
    config: &ApiConfig,
) -> ApiResult<CorrectionResult> {
    let current = fetch_locked_session(transaction, session_id).await?;
    let is_subscription_session = validate_session_relationship(&current)?;
    if current.status != "closed" {
        return Err(ApiError::conflict("يمكن تصحيح الجلسات المغلقة فقط."));
    }
    let old = session_snapshot(transaction, &current).await?;
    let old_net_total = current.net_total;
    let old_effective = commissions::effective_session_transaction(transaction, &current).await?;
    let old_commission_amount = old_effective.commission_amount.unwrap_or(Decimal::ZERO);
    let old_commission_rate = current.commission_rate.unwrap_or(Decimal::ZERO);
    let old_doctor_id = current.doctor_id;

    let mut impact = if is_subscription_session {
        if changes.keys().any(|key| key != "doctor_id") {
            return Err(ApiError::validation(
                "تصحيح جلسة الاشتراك يقتصر على الطبيب دون تغيير الاستهلاك أو القيمة.",
            )
            .field(
                "changes",
                "ألغِ الجلسة المغلقة ثم افتح جلسة صحيحة لتغيير الاشتراك أو الخدمات.",
            ));
        }
        if let Some(value) = changes.get("doctor_id") {
            let doctor_id = value_u64(value).ok_or_else(|| {
                ApiError::validation("البيانات المرسلة غير صحيحة.")
                    .field("doctor_id", "معرف الطبيب غير صحيح.")
            })?;
            let doctor = active_doctor(transaction, doctor_id).await?;
            sqlx::query("UPDATE sessions SET doctor_id = ?, updated_at = NOW() WHERE id = ?")
                .bind(doctor.id)
                .bind(session_id)
                .execute(&mut **transaction)
                .await?;
        }
        json!({ "supervising_doctor_updated": changes.contains_key("doctor_id") })
    } else {
        let additions = sqlx::query_scalar::<_, u64>(
            "SELECT CAST(COUNT(*) AS UNSIGNED) FROM session_additions WHERE session_id = ?",
        )
        .bind(session_id)
        .fetch_one(&mut **transaction)
        .await?;
        if additions > 0 {
            return Err(ApiError::conflict(
                "لا يمكن استبدال جلسة مغلقة تحتوي على إضافات. ألغِ الجلسة وأنشئ جلسة صحيحة للحفاظ على السجل المالي.",
            ));
        }

        let mut impact = reverse_closed_effects(
            transaction,
            &current,
            &format!("تصحيح جلسة مغلقة: {reason}"),
            actor_id,
        )
        .await?;
        let prepared = prepare_open_session(transaction, changes, Some(&current)).await?;
        if prepared.session_type != "individual" {
            return Err(ApiError::validation("نوع الجلسة غير صحيح.")
                .field("type", "لا يمكن تحويل نوع جلسة مغلقة أثناء التصحيح."));
        }
        update_prepared_session(transaction, session_id, &prepared).await?;
        replace_snapshots(transaction, session_id, &prepared.services, actor_id).await?;
        let replacement_row = fetch_locked_session(transaction, session_id).await?;
        lock_patient(transaction, replacement_row.patient_id, false).await?;
        let replacement = apply_settlement(
            transaction,
            &replacement_row,
            None,
            &Settlement {
                wallet_amount: replacement_row.wallet_amount,
                paid_amount: replacement_row.paid_amount,
                debt_amount: replacement_row.debt_amount,
                method: replacement_row.payment_method.clone(),
                sponsor_id: replacement_row.sponsor_id,
            },
            "adjustment",
            actor_id,
        )
        .await?;
        let commission_rate = if replacement_row.doctor_id != old_doctor_id {
            sqlx::query_as::<_, DoctorRow>("SELECT * FROM doctors WHERE id = ?")
                .bind(replacement_row.doctor_id)
                .fetch_one(&mut **transaction)
                .await?
                .commission_rate
        } else {
            old_commission_rate
        };
        let commission_base = match config.commission_base {
            CommissionBase::Gross => replacement_row.services_total,
            CommissionBase::Net => replacement_row.net_total,
        };
        sqlx::query(
            "UPDATE sessions SET commission_rate = ?, commission_base_amount = ?, commission_amount = ?, updated_at = NOW() WHERE id = ?",
        )
        .bind(commission_rate)
        .bind(commission_base)
        .bind(money::percentage(commission_base, commission_rate))
        .bind(session_id)
        .execute(&mut **transaction)
        .await?;
        impact
            .as_object_mut()
            .expect("reversal impact is an object")
            .insert("replacement".into(), replacement.value());
        impact
    };

    let updated = fetch_locked_session(transaction, session_id).await?;
    let effective_updated =
        commissions::effective_session_transaction(transaction, &updated).await?;
    impact
        .as_object_mut()
        .expect("financial impact is an object")
        .insert(
            "net_delta".into(),
            Value::String(money::normalize(updated.net_total - old_net_total)),
        );
    impact
        .as_object_mut()
        .expect("financial impact is an object")
        .insert(
            "commission_delta".into(),
            Value::String(money::normalize(
                effective_updated.commission_amount.unwrap_or(Decimal::ZERO)
                    - old_commission_amount,
            )),
        );
    let new = session_snapshot(transaction, &updated).await?;
    audit::log(
        transaction,
        actor_id,
        "session_corrected",
        "session",
        session_id,
        Some(&old),
        Some(&new),
        Some(reason),
    )
    .await?;

    Ok(CorrectionResult {
        session: session_value(transaction, &updated, false).await?,
        financial_impact: impact,
    })
}

pub async fn session_value(
    transaction: &mut Transaction<'_, MySql>,
    session: &SessionRow,
    include_wallet: bool,
) -> ApiResult<Value> {
    let effective_session =
        commissions::effective_session_transaction(transaction, session).await?;
    let patient = sqlx::query_as::<_, PatientRow>("SELECT * FROM patients WHERE id = ?")
        .bind(session.patient_id)
        .fetch_one(&mut **transaction)
        .await?;
    let patient = if include_wallet {
        json!({
            "id": patient.id,
            "name": patient.name,
            "phone": patient.phone,
            "wallet_balance": money::normalize(ledger::wallet_balance(transaction, patient.id).await?),
        })
    } else {
        json!({ "id": patient.id, "name": patient.name, "phone": patient.phone })
    };
    let doctor = sqlx::query("SELECT id, name FROM doctors WHERE id = ?")
        .bind(session.doctor_id)
        .fetch_one(&mut **transaction)
        .await?;
    let doctor = json!({
        "id": doctor.try_get::<u64, _>("id")?,
        "name": doctor.try_get::<String, _>("name")?,
    });
    let services = sqlx::query_as::<_, SessionServiceRow>(
        "SELECT * FROM session_services WHERE session_id = ? ORDER BY id",
    )
    .bind(session.id)
    .fetch_all(&mut **transaction)
    .await?
    .iter()
    .map(resources::session_service_value)
    .collect::<Vec<_>>();
    let additions = sqlx::query_as::<_, SessionAdditionRow>(
        "SELECT * FROM session_additions WHERE session_id = ? ORDER BY id",
    )
    .bind(session.id)
    .fetch_all(&mut **transaction)
    .await?
    .iter()
    .map(resources::session_addition_value)
    .collect::<Vec<_>>();
    let creator = user_summary(transaction, session.created_by).await?;
    let closed_by = Some(match session.closed_by {
        Some(id) => user_summary(transaction, id).await?,
        None => Value::Null,
    });
    let cancelled_by = Some(match session.cancelled_by {
        Some(id) => user_summary(transaction, id).await?,
        None => Value::Null,
    });
    Ok(resources::session_value(
        &effective_session,
        Some(patient),
        Some(doctor),
        Some(services),
        Some(creator),
        closed_by,
        cancelled_by,
        Some(additions),
    ))
}

pub async fn fetch_locked_session(
    transaction: &mut Transaction<'_, MySql>,
    session_id: u64,
) -> ApiResult<SessionRow> {
    sqlx::query_as::<_, SessionRow>("SELECT * FROM sessions WHERE id = ? FOR UPDATE")
        .bind(session_id)
        .fetch_optional(&mut **transaction)
        .await?
        .ok_or_else(ApiError::not_found)
}

async fn create_open(
    transaction: &mut Transaction<'_, MySql>,
    data: &Map<String, Value>,
    actor_id: u64,
) -> ApiResult<u64> {
    let prepared = prepare_open_session(transaction, data, None).await?;
    let session_date = match data.get("session_date") {
        Some(Value::String(value)) => {
            NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| {
                ApiError::validation("تاريخ الجلسة غير صالح.")
                    .field("session_date", "أدخل تاريخ جلسة صحيحًا.")
            })?
        }
        Some(Value::Null) | None => time::today(),
        _ => {
            return Err(ApiError::validation("تاريخ الجلسة غير صالح.")
                .field("session_date", "أدخل تاريخ جلسة صحيحًا."));
        }
    };
    let pending_code = format!("pending-{}", Uuid::new_v4());
    let result = sqlx::query(
        "INSERT INTO sessions \
         (code, patient_id, doctor_id, subscription_id, type, status, opened_at, created_by, services_total, discount, net_total, coverage, wallet_amount, paid_amount, debt_amount, payment_method, sponsor_id, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, 'open', TIMESTAMP(?, CURTIME()), ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NOW(), NOW())",
    )
    .bind(pending_code)
    .bind(prepared.patient_id)
    .bind(prepared.doctor_id)
    .bind(prepared.subscription_id)
    .bind(&prepared.session_type)
    .bind(session_date)
    .bind(actor_id)
    .bind(prepared.services_total)
    .bind(prepared.discount)
    .bind(prepared.net_total)
    .bind(&prepared.coverage)
    .bind(prepared.settlement.wallet_amount)
    .bind(prepared.settlement.paid_amount)
    .bind(prepared.settlement.debt_amount)
    .bind(&prepared.settlement.method)
    .bind(prepared.settlement.sponsor_id)
    .execute(&mut **transaction)
    .await?;
    let session_id = result.last_insert_id();
    let code = format!("S-{:04}", 1000 + session_id);
    sqlx::query("UPDATE sessions SET code = ?, updated_at = NOW() WHERE id = ?")
        .bind(code)
        .bind(session_id)
        .execute(&mut **transaction)
        .await?;
    insert_snapshots(
        transaction,
        session_id,
        None,
        &prepared.services,
        actor_id,
        false,
    )
    .await?;
    let session = fetch_locked_session(transaction, session_id).await?;
    let values = session_snapshot(transaction, &session).await?;
    audit::log(
        transaction,
        actor_id,
        "session_created",
        "session",
        session_id,
        None,
        Some(&values),
        None,
    )
    .await?;
    Ok(session_id)
}

async fn prepare_open_session(
    transaction: &mut Transaction<'_, MySql>,
    data: &Map<String, Value>,
    current: Option<&SessionRow>,
) -> ApiResult<PreparedSession> {
    let patient_id = field_u64(data, "patient_id", current.map(|row| row.patient_id))?;
    let doctor_id = field_u64(data, "doctor_id", current.map(|row| row.doctor_id))?;
    let patient_can_be_inactive =
        current.is_some_and(|row| row.patient_id == patient_id && !data.contains_key("patient_id"));
    let doctor_can_be_inactive =
        current.is_some_and(|row| row.doctor_id == doctor_id && !data.contains_key("doctor_id"));
    let patient = lock_patient(transaction, patient_id, !patient_can_be_inactive).await?;
    let _doctor = if doctor_can_be_inactive {
        sqlx::query_as::<_, DoctorRow>("SELECT * FROM doctors WHERE id = ?")
            .bind(doctor_id)
            .fetch_optional(&mut **transaction)
            .await?
            .ok_or_else(ApiError::not_found)?
    } else {
        active_doctor(transaction, doctor_id).await?
    };
    let session_type = data
        .get("type")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .or_else(|| current.map(|row| row.r#type.clone()))
        .ok_or_else(|| {
            ApiError::validation("نوع الجلسة مطلوب.")
                .field("type", "اختر جلسة فردية أو جلسة اشتراك.")
        })?;
    if !matches!(session_type.as_str(), "individual" | "subscription") {
        return Err(ApiError::validation("البيانات المرسلة غير صحيحة.")
            .field("type", "نوع الجلسة غير صحيح."));
    }
    if current.is_some_and(|row| row.r#type != session_type) {
        return Err(ApiError::conflict(
            "لا يمكن تحويل الجلسة بين النوع الفردي ونوع الاشتراك.",
        ));
    }

    if session_type == "subscription" {
        validate_subscription_session_financial_input(data)?;
        let subscription_id = field_optional_u64(
            data,
            "subscription_id",
            current.and_then(|row| row.subscription_id),
        )?
        .ok_or_else(|| {
            ApiError::validation("الاشتراك مطلوب للجلسة من نوع اشتراك.")
                .field("subscription_id", "اختر اشتراكًا نشطًا للمريض.")
        })?;
        let subscription = fetch_locked_subscription(transaction, subscription_id).await?;
        if subscription.patient_id != patient.id || subscription.status != "active" {
            return Err(ApiError::validation("الاشتراك المحدد غير صالح لهذا المريض.")
                .field("subscription_id", "اختر اشتراكًا نشطًا يخص المريض."));
        }
        if subscription.sessions_used >= subscription.sessions_count {
            return Err(ApiError::validation("لا توجد جلسات متبقية في الاشتراك.")
                .field("subscription_id", "الاشتراك مكتمل."));
        }
        let mut services = resolve_requested_services(transaction, data, current, false).await?;
        validate_subscription_service_coverage(
            transaction,
            subscription.id,
            services.iter().map(|service| service.service_id).collect(),
        )
        .await?;
        for service in &mut services {
            service.price = Decimal::ZERO;
            service.base_price = Decimal::ZERO;
            service.included_units = 0;
            service.extra_units = 0;
            service.extra_unit_price = None;
        }
        return Ok(PreparedSession {
            patient_id: patient.id,
            doctor_id,
            subscription_id: Some(subscription.id),
            session_type,
            services_total: Decimal::ZERO,
            discount: Decimal::ZERO,
            net_total: Decimal::ZERO,
            coverage: "subscription".into(),
            settlement: Settlement::default(),
            services,
        });
    }

    if data
        .get("subscription_id")
        .is_some_and(|value| !value.is_null())
    {
        return Err(ApiError::validation("الجلسة الفردية لا ترتبط باشتراك.")
            .field("subscription_id", "احذف الاشتراك أو اختر نوع جلسة اشتراك."));
    }

    let services = resolve_requested_services(transaction, data, current, false).await?;
    let services_total = money::sum(services.iter().map(|service| service.price));
    let discount = decimal_field(
        data,
        "discount",
        current.map(|row| row.discount).unwrap_or(Decimal::ZERO),
        "discount",
    )?;
    if discount > services_total {
        return Err(ApiError::validation("الخصم لا يمكن أن يتجاوز قيمة الخدمات.")
            .field("discount", "قيمة الخصم أكبر من إجمالي الخدمات."));
    }
    let net_total = money::round(services_total - discount);
    let settlement = parse_settlement(
        data,
        current.map(|row| Settlement {
            wallet_amount: row.wallet_amount,
            paid_amount: row.paid_amount,
            debt_amount: row.debt_amount,
            method: row.payment_method.clone(),
            sponsor_id: row.sponsor_id,
        }),
        current.is_none(),
    )?;
    if money::sum([
        settlement.wallet_amount,
        settlement.paid_amount,
        settlement.debt_amount,
    ]) != net_total
    {
        return Err(
            ApiError::validation("مجموع أجزاء التسوية يجب أن يساوي صافي قيمة الجلسة.")
                .field("settlement", "راجع مبالغ الرصيد والمدفوع والدين."),
        );
    }
    validate_paid_method(&settlement)?;
    validate_sponsor_if_present(transaction, data, &settlement, current.is_none()).await?;
    if settlement.wallet_amount > Decimal::ZERO
        && ledger::wallet_balance(transaction, patient.id).await? < settlement.wallet_amount
    {
        return Err(
            ApiError::validation("الرصيد غير كافٍ. اشحن رصيد المريض أولًا ثم أعد المحاولة.").field(
                "settlement.wallet_amount",
                "الرصيد المتاح أقل من المبلغ المطلوب.",
            ),
        );
    }
    let coverage = coverage(&settlement);
    Ok(PreparedSession {
        patient_id: patient.id,
        doctor_id,
        subscription_id: None,
        session_type,
        services_total,
        discount,
        net_total,
        coverage,
        settlement,
        services,
    })
}

async fn resolve_requested_services(
    transaction: &mut Transaction<'_, MySql>,
    data: &Map<String, Value>,
    current: Option<&SessionRow>,
    allow_extra_only: bool,
) -> ApiResult<Vec<ResolvedService>> {
    let snapshots = if let Some(current) = current {
        sqlx::query_as::<_, SessionServiceRow>(
            "SELECT * FROM session_services WHERE session_id = ? ORDER BY id",
        )
        .bind(current.id)
        .fetch_all(&mut **transaction)
        .await?
    } else {
        Vec::new()
    };
    let current_ids = snapshots
        .iter()
        .map(|row| row.service_id)
        .collect::<Vec<_>>();
    let has_structured_services = data.contains_key("services");
    let has_service_ids = data.contains_key("service_ids");

    if current.is_some() && !has_structured_services && !has_service_ids {
        if snapshots.is_empty() {
            return Err(ApiError::validation("اختر خدمة واحدة على الأقل.")
                .field("services", "حقل الخدمات مطلوب."));
        }
        return Ok(snapshots
            .iter()
            .map(ResolvedService::from_snapshot)
            .collect());
    }
    if has_service_ids && !has_structured_services {
        let requested_ids = parse_id_array(data, "service_ids")?;
        let mut requested_sorted = requested_ids.clone();
        requested_sorted.sort_unstable();
        let mut current_sorted = current_ids;
        current_sorted.sort_unstable();
        if current.is_some() && requested_sorted == current_sorted {
            return Ok(snapshots
                .iter()
                .map(ResolvedService::from_snapshot)
                .collect());
        }
    }
    let inputs = parse_service_items(data, allow_extra_only)?;
    resolve_service_items(transaction, &inputs, allow_extra_only).await
}

fn parse_service_items(
    data: &Map<String, Value>,
    _allow_extra_only: bool,
) -> ApiResult<Vec<ServiceItemInput>> {
    if let Some(raw) = data.get("services") {
        let items = raw
            .as_array()
            .filter(|items| !items.is_empty())
            .ok_or_else(|| {
                ApiError::validation("اختر خدمة واحدة على الأقل.")
                    .field("services", "حقل الخدمات مطلوب.")
            })?;
        let mut parsed = Vec::with_capacity(items.len());
        for (index, value) in items.iter().enumerate() {
            let item = value.as_object().ok_or_else(|| {
                ApiError::validation("البيانات المرسلة غير صحيحة.")
                    .field(format!("services.{index}"), "بند الخدمة غير صحيح.")
            })?;
            let service_id = item.get("service_id").and_then(value_u64).ok_or_else(|| {
                ApiError::validation("البيانات المرسلة غير صحيحة.")
                    .field(format!("services.{index}.service_id"), "معرف الخدمة مطلوب.")
            })?;
            let item_type = item
                .get("item_type")
                .and_then(Value::as_str)
                .unwrap_or("service")
                .to_string();
            if !matches!(item_type.as_str(), "service" | "extra_units") {
                return Err(ApiError::validation("البيانات المرسلة غير صحيحة.").field(
                    format!("services.{index}.item_type"),
                    "نوع بند الخدمة غير صحيح.",
                ));
            }
            let extra_units = match item.get("extra_units") {
                None | Some(Value::Null) => 0,
                Some(value) => value_u64(value)
                    .and_then(|value| u16::try_from(value).ok())
                    .filter(|value| *value <= 10_000)
                    .ok_or_else(|| {
                        ApiError::validation("البيانات المرسلة غير صحيحة.").field(
                            format!("services.{index}.extra_units"),
                            "عدد الوحدات الإضافية غير صحيح.",
                        )
                    })?,
            };
            parsed.push(ServiceItemInput {
                service_id,
                item_type,
                extra_units,
            });
        }
        return Ok(parsed);
    }
    parse_id_array(data, "service_ids").map(|ids| {
        ids.into_iter()
            .map(|service_id| ServiceItemInput {
                service_id,
                item_type: "service".into(),
                extra_units: 0,
            })
            .collect()
    })
}

async fn resolve_service_items(
    transaction: &mut Transaction<'_, MySql>,
    inputs: &[ServiceItemInput],
    allow_extra_only: bool,
) -> ApiResult<Vec<ResolvedService>> {
    if inputs.is_empty() {
        return Err(ApiError::validation("اختر خدمة واحدة على الأقل.")
            .field("services", "حقل الخدمات مطلوب."));
    }
    let mut services = HashMap::new();
    for service_id in inputs.iter().map(|item| item.service_id) {
        if services.contains_key(&service_id) {
            continue;
        }
        let service = sqlx::query_as::<_, ServiceRow>(
            "SELECT * FROM services WHERE id = ? AND is_active = 1",
        )
        .bind(service_id)
        .fetch_optional(&mut **transaction)
        .await?
        .ok_or_else(|| {
            ApiError::validation("إحدى الخدمات غير متاحة للجلسات الجديدة.")
                .field("services", "اختر خدمات نشطة فقط.")
        })?;
        services.insert(service_id, service);
    }
    let mut seen = HashSet::new();
    let mut resolved = Vec::with_capacity(inputs.len());
    for item in inputs {
        let key = format!("{}:{}", item.service_id, item.item_type);
        if !seen.insert(key) {
            return Err(
                ApiError::validation("لا يمكن تكرار بند الخدمة نفسه في العملية الواحدة.")
                    .field("services", "ادمج الكمية الإضافية في بند واحد."),
            );
        }
        if item.item_type == "extra_units" && !allow_extra_only {
            return Err(ApiError::validation(
                "الوحدات الإضافية المنفردة متاحة عند الإضافة إلى جلسة مغلقة فقط.",
            )
            .field("services", "اختر الخدمة الأساسية عند إنشاء الجلسة."));
        }
        let service = services
            .get(&item.service_id)
            .expect("all requested services were loaded");
        if item.extra_units > 0
            && (service.included_units.is_none() || service.extra_unit_price.is_none())
        {
            return Err(ApiError::validation("الخدمة المحددة لا تدعم وحدات إضافية.")
                .field("services", "راجع إعدادات ملحقات الخدمة."));
        }
        if item.item_type == "extra_units" && item.extra_units < 1 {
            return Err(ApiError::validation("أدخل عدد الوحدات الإضافية.")
                .field("services", "عدد الوحدات الإضافية يجب أن يكون 1 على الأقل."));
        }
        let base_price = if item.item_type == "service" {
            service.price
        } else {
            Decimal::ZERO
        };
        let extras_total = service
            .extra_unit_price
            .map(|price| money::round(price * Decimal::from(item.extra_units)))
            .unwrap_or(Decimal::ZERO);
        resolved.push(ResolvedService {
            service_id: service.id,
            service_name: service.name.clone(),
            price: money::round(base_price + extras_total),
            base_price,
            included_units: if item.item_type == "service" {
                service.included_units.unwrap_or(0)
            } else {
                0
            },
            extra_units: item.extra_units,
            unit_name: service.unit_name.clone(),
            extra_unit_price: service.extra_unit_price,
            item_type: item.item_type.clone(),
        });
    }
    Ok(resolved)
}

fn parse_settlement(
    data: &Map<String, Value>,
    current: Option<Settlement>,
    required: bool,
) -> ApiResult<Settlement> {
    let base = current.unwrap_or_default();
    let Some(value) = data.get("settlement") else {
        if required {
            return Err(ApiError::validation("بيانات تسوية الجلسة مطلوبة.")
                .field("settlement", "أدخل مبالغ تسوية الجلسة."));
        }
        return Ok(base);
    };
    let settlement = value.as_object().ok_or_else(|| {
        ApiError::validation("البيانات المرسلة غير صحيحة.")
            .field("settlement", "بيانات التسوية غير صحيحة.")
    })?;
    let wallet_amount = decimal_field(
        settlement,
        "wallet_amount",
        base.wallet_amount,
        "settlement.wallet_amount",
    )?;
    let paid_amount = decimal_field(
        settlement,
        "paid_amount",
        base.paid_amount,
        "settlement.paid_amount",
    )?;
    let debt_amount = decimal_field(
        settlement,
        "debt_amount",
        base.debt_amount,
        "settlement.debt_amount",
    )?;
    let method = if settlement.contains_key("method") {
        match settlement.get("method") {
            None | Some(Value::Null) => None,
            Some(Value::String(method)) if matches!(method.as_str(), "cash" | "transfer") => {
                Some(method.clone())
            }
            _ => {
                return Err(ApiError::validation("البيانات المرسلة غير صحيحة.")
                    .field("settlement.method", "طريقة الدفع غير صحيحة."));
            }
        }
    } else {
        base.method
    };
    let sponsor_id = if settlement.contains_key("sponsor_id") {
        match settlement.get("sponsor_id") {
            None | Some(Value::Null) => None,
            Some(value) => Some(value_u64(value).ok_or_else(|| {
                ApiError::validation("البيانات المرسلة غير صحيحة.")
                    .field("settlement.sponsor_id", "معرف الجهة الراعية غير صحيح.")
            })?),
        }
    } else {
        base.sponsor_id
    };
    Ok(Settlement {
        wallet_amount,
        paid_amount,
        debt_amount,
        method,
        sponsor_id,
    })
}

fn validate_paid_method(settlement: &Settlement) -> ApiResult<()> {
    if settlement.paid_amount > Decimal::ZERO && settlement.method.is_none() {
        return Err(
            ApiError::validation("طريقة الدفع مطلوبة عند تسجيل مبلغ مدفوع.")
                .field("settlement.method", "اختر نقدًا أو تحويلًا مصرفيًا."),
        );
    }
    Ok(())
}

async fn validate_sponsor_if_present(
    transaction: &mut Transaction<'_, MySql>,
    data: &Map<String, Value>,
    settlement: &Settlement,
    validate_when_present_on_create: bool,
) -> ApiResult<()> {
    let explicitly_present = data
        .get("settlement")
        .and_then(Value::as_object)
        .is_some_and(|value| value.contains_key("sponsor_id"));
    if !explicitly_present && !validate_when_present_on_create {
        return Ok(());
    }
    let Some(sponsor_id) = settlement.sponsor_id else {
        return Ok(());
    };
    let exists = sqlx::query("SELECT id FROM sponsors WHERE id = ? AND is_active = 1")
        .bind(sponsor_id)
        .fetch_optional(&mut **transaction)
        .await?
        .is_some();
    if !exists {
        return Err(ApiError::validation("الجهة الراعية المحددة غير متاحة.")
            .field("settlement.sponsor_id", "اختر جهة راعية نشطة."));
    }
    Ok(())
}

fn coverage(settlement: &Settlement) -> String {
    let wallet = settlement.wallet_amount > Decimal::ZERO;
    let paid = settlement.paid_amount > Decimal::ZERO;
    let debt = settlement.debt_amount > Decimal::ZERO;
    if [wallet, paid, debt]
        .into_iter()
        .filter(|used| *used)
        .count()
        > 1
    {
        return "mixed".into();
    }
    if wallet {
        "wallet".into()
    } else if debt {
        "debt".into()
    } else if paid && settlement.method.as_deref() == Some("transfer") {
        "transfer".into()
    } else {
        "cash".into()
    }
}

async fn apply_settlement(
    transaction: &mut Transaction<'_, MySql>,
    session: &SessionRow,
    addition_id: Option<u64>,
    settlement: &Settlement,
    payment_type: &str,
    actor_id: u64,
) -> ApiResult<Effects> {
    let wallet_transaction = ledger::debit(
        transaction,
        session.patient_id,
        session.id,
        addition_id,
        settlement.wallet_amount,
        actor_id,
    )
    .await?;
    let payment = if settlement.paid_amount > Decimal::ZERO {
        let method = settlement.method.as_deref().ok_or_else(|| {
            ApiError::validation("طريقة الدفع مطلوبة عند تسجيل مبلغ مدفوع.")
                .field("settlement.method", "اختر نقدًا أو تحويلًا مصرفيًا.")
        })?;
        let payment_id = ledger::insert_payment(
            transaction,
            session.patient_id,
            Some(session.id),
            addition_id,
            None,
            payment_type,
            method,
            settlement.paid_amount,
            None,
            None,
            actor_id,
        )
        .await?;
        let values = json!({
            "session_id": session.id,
            "type": payment_type,
            "method": method,
            "amount": money::normalize(settlement.paid_amount),
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
    let debt = if settlement.debt_amount > Decimal::ZERO {
        let result = sqlx::query(
            "INSERT INTO debts \
             (patient_id, sponsor_id, session_id, session_addition_id, kind, amount, paid_amount, status, created_by, created_at, updated_at) \
             VALUES (?, ?, ?, ?, 'session', ?, 0, 'open', ?, NOW(), NOW())",
        )
        .bind(session.patient_id)
        .bind(settlement.sponsor_id)
        .bind(session.id)
        .bind(addition_id)
        .bind(settlement.debt_amount)
        .bind(actor_id)
        .execute(&mut **transaction)
        .await?;
        Some(ledger::fetch_debt(transaction, result.last_insert_id()).await?)
    } else {
        None
    };
    Ok(Effects {
        subscription_usage: None,
        wallet_transaction,
        payment,
        debt,
    })
}

async fn consume_subscription(
    transaction: &mut Transaction<'_, MySql>,
    session: &SessionRow,
    actor_id: u64,
) -> ApiResult<Option<SubscriptionUsageRow>> {
    let subscription_id = session
        .subscription_id
        .ok_or_else(|| ApiError::internal("Subscription session is missing subscription_id."))?;
    let subscription = fetch_locked_subscription(transaction, subscription_id).await?;
    if subscription.status != "active" {
        return Err(ApiError::validation("الاشتراك لم يعد صالحًا للاستخدام.")
            .field("subscription_id", "الاشتراك مكتمل أو ملغي."));
    }
    if subscription.sessions_used >= subscription.sessions_count {
        return Err(ApiError::validation("الاشتراك لم يعد صالحًا للاستخدام.")
            .field("subscription_id", "لا توجد جلسات متبقية."));
    }
    let selected = sqlx::query_scalar::<_, u64>(
        "SELECT service_id FROM session_services WHERE session_id = ? ORDER BY service_id",
    )
    .bind(session.id)
    .fetch_all(&mut **transaction)
    .await?;
    validate_subscription_service_coverage(transaction, subscription.id, selected).await?;
    let result = sqlx::query(
        "INSERT INTO subscription_usages \
         (subscription_id, session_id, type, created_by, created_at, updated_at) \
         VALUES (?, ?, 'consume', ?, NOW(), NOW())",
    )
    .bind(subscription.id)
    .bind(session.id)
    .bind(actor_id)
    .execute(&mut **transaction)
    .await?;
    let used = subscription.sessions_used + 1;
    let status = if used >= subscription.sessions_count {
        "completed"
    } else {
        "active"
    };
    sqlx::query(
        "UPDATE subscriptions SET sessions_used = ?, status = ?, updated_at = NOW() WHERE id = ?",
    )
    .bind(used)
    .bind(status)
    .bind(subscription.id)
    .execute(&mut **transaction)
    .await?;
    let usage =
        sqlx::query_as::<_, SubscriptionUsageRow>("SELECT * FROM subscription_usages WHERE id = ?")
            .bind(result.last_insert_id())
            .fetch_one(&mut **transaction)
            .await?;
    Ok(Some(usage))
}

async fn existing_effects(
    transaction: &mut Transaction<'_, MySql>,
    session: &SessionRow,
) -> ApiResult<Effects> {
    let subscription_usage = sqlx::query_as::<_, SubscriptionUsageRow>(
        "SELECT * FROM subscription_usages WHERE session_id = ? AND type = 'consume' ORDER BY id LIMIT 1",
    )
    .bind(session.id)
    .fetch_optional(&mut **transaction)
    .await?;
    let wallet_transaction = sqlx::query_as::<_, WalletTransactionRow>(
        "SELECT * FROM wallet_transactions WHERE session_id = ? AND amount < 0 ORDER BY id DESC LIMIT 1",
    )
    .bind(session.id)
    .fetch_optional(&mut **transaction)
    .await?;
    let payment = sqlx::query_as::<_, PaymentRow>(
        "SELECT * FROM payments WHERE session_id = ? AND amount > 0 ORDER BY id DESC LIMIT 1",
    )
    .bind(session.id)
    .fetch_optional(&mut **transaction)
    .await?;
    let debt = sqlx::query_as::<_, DebtRow>(
        "SELECT * FROM debts WHERE session_id = ? AND status != 'void' ORDER BY id DESC LIMIT 1",
    )
    .bind(session.id)
    .fetch_optional(&mut **transaction)
    .await?;
    Ok(Effects {
        subscription_usage,
        wallet_transaction,
        payment,
        debt,
    })
}

async fn reverse_closed_effects(
    transaction: &mut Transaction<'_, MySql>,
    session: &SessionRow,
    reason: &str,
    actor_id: u64,
) -> ApiResult<Value> {
    let mut subscription_usage = Value::Null;
    let mut wallet = Vec::new();
    let mut refunds = Vec::new();
    let mut voided_debts = Vec::new();

    if session.r#type == "subscription" {
        let consume = sqlx::query_as::<_, SubscriptionUsageRow>(
            "SELECT * FROM subscription_usages WHERE session_id = ? AND type = 'consume' ORDER BY id LIMIT 1 FOR UPDATE",
        )
        .bind(session.id)
        .fetch_optional(&mut **transaction)
        .await?;
        if let Some(consume) = consume {
            let restored = sqlx::query(
                "SELECT id FROM subscription_usages WHERE reverses_usage_id = ? LIMIT 1",
            )
            .bind(consume.id)
            .fetch_optional(&mut **transaction)
            .await?
            .is_some();
            if !restored {
                let subscription =
                    fetch_locked_subscription(transaction, consume.subscription_id).await?;
                let result = sqlx::query(
                    "INSERT INTO subscription_usages \
                     (subscription_id, session_id, type, reverses_usage_id, created_by, created_at, updated_at) \
                     VALUES (?, ?, 'restore', ?, ?, NOW(), NOW())",
                )
                .bind(subscription.id)
                .bind(session.id)
                .bind(consume.id)
                .bind(actor_id)
                .execute(&mut **transaction)
                .await?;
                let status = if subscription.status == "cancelled" {
                    subscription.status.clone()
                } else {
                    "active".into()
                };
                sqlx::query(
                    "UPDATE subscriptions SET sessions_used = ?, status = ?, updated_at = NOW() WHERE id = ?",
                )
                .bind(subscription.sessions_used.saturating_sub(1))
                .bind(status)
                .bind(subscription.id)
                .execute(&mut **transaction)
                .await?;
                let restore = sqlx::query_as::<_, SubscriptionUsageRow>(
                    "SELECT * FROM subscription_usages WHERE id = ?",
                )
                .bind(result.last_insert_id())
                .fetch_one(&mut **transaction)
                .await?;
                subscription_usage = resources::subscription_usage_value(&restore);
            }
        }
        return Ok(json!({
            "subscription_usage": subscription_usage,
            "wallet": wallet,
            "refunds": refunds,
            "voided_debts": voided_debts,
        }));
    }

    lock_patient(transaction, session.patient_id, false).await?;
    let debits = sqlx::query_as::<_, WalletTransactionRow>(
        "SELECT * FROM wallet_transactions WHERE session_id = ? AND amount < 0 ORDER BY id FOR UPDATE",
    )
    .bind(session.id)
    .fetch_all(&mut **transaction)
    .await?;
    for debit in debits {
        let existing = sqlx::query_as::<_, WalletTransactionRow>(
            "SELECT * FROM wallet_transactions WHERE reverses_transaction_id = ? LIMIT 1",
        )
        .bind(debit.id)
        .fetch_optional(&mut **transaction)
        .await?;
        let reversal = if let Some(existing) = existing {
            existing
        } else {
            let amount = debit.amount.abs();
            let balance = ledger::wallet_balance(transaction, debit.patient_id).await?;
            let result = sqlx::query(
                "INSERT INTO wallet_transactions \
                 (patient_id, session_id, session_addition_id, type, amount, balance_after, reverses_transaction_id, note, created_by, created_at, updated_at) \
                 VALUES (?, ?, ?, 'reversal', ?, ?, ?, ?, ?, NOW(), NOW())",
            )
            .bind(debit.patient_id)
            .bind(debit.session_id)
            .bind(debit.session_addition_id)
            .bind(amount)
            .bind(money::round(balance + amount))
            .bind(debit.id)
            .bind(reason)
            .bind(actor_id)
            .execute(&mut **transaction)
            .await?;
            sqlx::query_as::<_, WalletTransactionRow>(
                "SELECT * FROM wallet_transactions WHERE id = ?",
            )
            .bind(result.last_insert_id())
            .fetch_one(&mut **transaction)
            .await?
        };
        wallet.push(resources::wallet_transaction_value(&reversal));
    }

    let payments = sqlx::query_as::<_, PaymentRow>(
        "SELECT * FROM payments WHERE session_id = ? AND type IN ('session_payment', 'adjustment') AND amount > 0 ORDER BY id FOR UPDATE",
    )
    .bind(session.id)
    .fetch_all(&mut **transaction)
    .await?;
    for payment in payments {
        let refunded = refunded_total(transaction, payment.id).await?;
        let refundable = money::round(payment.amount - refunded);
        if refundable > Decimal::ZERO {
            let refund = ledger::refund(
                transaction,
                payment.patient_id,
                payment.id,
                refundable,
                &payment.method,
                reason,
                actor_id,
            )
            .await?;
            refunds.push(resources::payment_value(&refund, None));
        }
    }

    let debts = sqlx::query_as::<_, DebtRow>(
        "SELECT * FROM debts WHERE session_id = ? AND status != 'void' ORDER BY id FOR UPDATE",
    )
    .bind(session.id)
    .fetch_all(&mut **transaction)
    .await?;
    for debt in debts {
        let allocations = sqlx::query(
            "SELECT pa.amount AS allocation_amount, p.* \
             FROM payment_allocations pa INNER JOIN payments p ON p.id = pa.payment_id \
             WHERE pa.debt_id = ? ORDER BY pa.id FOR UPDATE",
        )
        .bind(debt.id)
        .fetch_all(&mut **transaction)
        .await?;
        for row in allocations {
            let payment = PaymentRow {
                id: row.try_get("id")?,
                patient_id: row.try_get("patient_id")?,
                session_id: row.try_get("session_id")?,
                session_addition_id: row.try_get("session_addition_id")?,
                subscription_id: row.try_get("subscription_id")?,
                r#type: row.try_get("type")?,
                method: row.try_get("method")?,
                amount: row.try_get("amount")?,
                reverses_payment_id: row.try_get("reverses_payment_id")?,
                note: row.try_get("note")?,
                created_by: row.try_get("created_by")?,
                created_at: row.try_get("created_at")?,
            };
            let allocation_amount: Decimal = row.try_get("allocation_amount")?;
            let available =
                money::round(payment.amount - refunded_total(transaction, payment.id).await?);
            let refund_amount = allocation_amount.min(available);
            if refund_amount > Decimal::ZERO {
                let refund = ledger::refund(
                    transaction,
                    payment.patient_id,
                    payment.id,
                    refund_amount,
                    &payment.method,
                    reason,
                    actor_id,
                )
                .await?;
                refunds.push(resources::payment_value(&refund, None));
            }
        }
        sqlx::query(
            "UPDATE debts SET status = 'void', voided_at = NOW(), voided_by = ?, void_reason = ?, updated_at = NOW() WHERE id = ?",
        )
        .bind(actor_id)
        .bind(reason)
        .bind(debt.id)
        .execute(&mut **transaction)
        .await?;
        let voided = ledger::fetch_debt(transaction, debt.id).await?;
        voided_debts.push(resources::debt_value(&voided));
    }

    Ok(json!({
        "subscription_usage": subscription_usage,
        "wallet": wallet,
        "refunds": refunds,
        "voided_debts": voided_debts,
    }))
}

async fn refunded_total(
    transaction: &mut Transaction<'_, MySql>,
    payment_id: u64,
) -> ApiResult<Decimal> {
    Ok(sqlx::query_scalar::<_, Option<Decimal>>(
        "SELECT SUM(ABS(amount)) FROM payments WHERE reverses_payment_id = ? AND type = 'refund'",
    )
    .bind(payment_id)
    .fetch_one(&mut **transaction)
    .await?
    .unwrap_or(Decimal::ZERO))
}

async fn update_prepared_session(
    transaction: &mut Transaction<'_, MySql>,
    session_id: u64,
    prepared: &PreparedSession,
) -> ApiResult<()> {
    sqlx::query(
        "UPDATE sessions SET patient_id = ?, doctor_id = ?, subscription_id = ?, type = ?, services_total = ?, discount = ?, net_total = ?, \
         coverage = ?, wallet_amount = ?, paid_amount = ?, debt_amount = ?, payment_method = ?, sponsor_id = ?, updated_at = NOW() WHERE id = ?",
    )
    .bind(prepared.patient_id)
    .bind(prepared.doctor_id)
    .bind(prepared.subscription_id)
    .bind(&prepared.session_type)
    .bind(prepared.services_total)
    .bind(prepared.discount)
    .bind(prepared.net_total)
    .bind(&prepared.coverage)
    .bind(prepared.settlement.wallet_amount)
    .bind(prepared.settlement.paid_amount)
    .bind(prepared.settlement.debt_amount)
    .bind(&prepared.settlement.method)
    .bind(prepared.settlement.sponsor_id)
    .bind(session_id)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn replace_snapshots(
    transaction: &mut Transaction<'_, MySql>,
    session_id: u64,
    services: &[ResolvedService],
    actor_id: u64,
) -> ApiResult<()> {
    sqlx::query("DELETE FROM session_services WHERE session_id = ?")
        .bind(session_id)
        .execute(&mut **transaction)
        .await?;
    insert_snapshots(transaction, session_id, None, services, actor_id, false).await
}

async fn insert_snapshots(
    transaction: &mut Transaction<'_, MySql>,
    session_id: u64,
    addition_id: Option<u64>,
    services: &[ResolvedService],
    actor_id: u64,
    is_addition: bool,
) -> ApiResult<()> {
    for service in services {
        sqlx::query(
            "INSERT INTO session_services \
             (session_id, session_addition_id, service_id, service_name, price, base_price, included_units, extra_units, unit_name, extra_unit_price, item_type, is_addition, added_at, created_by, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, IF(?, NOW(), NULL), ?, NOW(), NOW())",
        )
        .bind(session_id)
        .bind(addition_id)
        .bind(service.service_id)
        .bind(&service.service_name)
        .bind(service.price)
        .bind(service.base_price)
        .bind(service.included_units)
        .bind(service.extra_units)
        .bind(&service.unit_name)
        .bind(service.extra_unit_price)
        .bind(&service.item_type)
        .bind(is_addition)
        .bind(is_addition)
        .bind(actor_id)
        .execute(&mut **transaction)
        .await?;
    }
    Ok(())
}

async fn session_snapshot(
    transaction: &mut Transaction<'_, MySql>,
    session: &SessionRow,
) -> ApiResult<Value> {
    let effective_session =
        commissions::effective_session_transaction(transaction, session).await?;
    let services = sqlx::query_as::<_, SessionServiceRow>(
        "SELECT * FROM session_services WHERE session_id = ? ORDER BY id",
    )
    .bind(session.id)
    .fetch_all(&mut **transaction)
    .await?;
    Ok(json!({
        "patient_id": session.patient_id,
        "doctor_id": session.doctor_id,
        "subscription_id": session.subscription_id,
        "type": session.r#type,
        "status": session.status,
        "services_total": money::normalize(session.services_total),
        "discount": money::normalize(session.discount),
        "net_total": money::normalize(session.net_total),
        "coverage": session.coverage,
        "wallet_amount": money::normalize(session.wallet_amount),
        "paid_amount": money::normalize(session.paid_amount),
        "debt_amount": money::normalize(session.debt_amount),
        "payment_method": session.payment_method,
        "sponsor_id": session.sponsor_id,
        "commission_rate": effective_session.commission_rate.map(money::normalize),
        "commission_base_amount": effective_session.commission_base_amount.map(money::normalize),
        "commission_amount": effective_session.commission_amount.map(money::normalize),
        "closed_at": session.closed_at.map(time::iso),
        "cancelled_at": session.cancelled_at.map(time::iso),
        "cancel_reason": session.cancel_reason,
        "services": services.iter().map(|service| json!({
            "service_id": service.service_id,
            "name": service.service_name,
            "price": money::normalize(service.price),
            "base_price": money::normalize(service.base_price.unwrap_or(service.price)),
            "included_units": service.included_units,
            "extra_units": service.extra_units,
            "unit_name": service.unit_name,
            "extra_unit_price": service.extra_unit_price.map(money::normalize),
            "item_type": service.item_type,
            "is_addition": service.is_addition,
        })).collect::<Vec<_>>(),
    }))
}

async fn fetch_locked_subscription(
    transaction: &mut Transaction<'_, MySql>,
    subscription_id: u64,
) -> ApiResult<SubscriptionRow> {
    sqlx::query_as::<_, SubscriptionRow>("SELECT * FROM subscriptions WHERE id = ? FOR UPDATE")
        .bind(subscription_id)
        .fetch_optional(&mut **transaction)
        .await?
        .ok_or_else(ApiError::not_found)
}

fn validate_session_relationship(session: &SessionRow) -> ApiResult<bool> {
    match (session.r#type.as_str(), session.subscription_id) {
        ("subscription", Some(_)) => Ok(true),
        ("individual", None) => Ok(false),
        ("subscription", None) => Err(ApiError::internal(
            "Subscription session is missing its parent subscription.",
        )),
        ("individual", Some(_)) => Err(ApiError::internal(
            "Individual session cannot reference a subscription.",
        )),
        _ => Err(ApiError::internal("Unknown session type.")),
    }
}

fn validate_subscription_session_financial_input(data: &Map<String, Value>) -> ApiResult<()> {
    let discount = decimal_field(data, "discount", Decimal::ZERO, "discount")?;
    if !discount.is_zero() {
        return Err(
            ApiError::validation("جلسة الاشتراك لا تقبل خصمًا أو قيمة مالية مستقلة.")
                .field("discount", "قيمة جلسة الاشتراك مغطاة بعقد الاشتراك."),
        );
    }
    if data.contains_key("settlement") {
        return Err(
            ApiError::validation("جلسة الاشتراك لا تقبل تسوية مالية مستقلة.").field(
                "settlement",
                "لا ترسل مدفوعًا أو دينًا أو خصمًا من المحفظة لجلسة الاشتراك.",
            ),
        );
    }
    Ok(())
}

fn validate_subscription_session_update_input(data: &Map<String, Value>) -> ApiResult<()> {
    if data.contains_key("services")
        || data.contains_key("service_ids")
        || data.contains_key("subscription_id")
    {
        return Err(
            ApiError::validation("لا يمكن تغيير خدمات جلسة الاشتراك.").field(
                "services",
                "ألغِ الجلسة وأنشئ جلسة صحيحة بدل إضافة خدمات أو تغيير الاشتراك المرتبط.",
            ),
        );
    }
    Ok(())
}

async fn validate_subscription_service_coverage(
    transaction: &mut Transaction<'_, MySql>,
    subscription_id: u64,
    selected_service_ids: Vec<u64>,
) -> ApiResult<()> {
    let covered_service_ids = sqlx::query_scalar::<_, u64>(
        "SELECT service_id FROM subscription_services WHERE subscription_id = ?",
    )
    .bind(subscription_id)
    .fetch_all(&mut **transaction)
    .await?
    .into_iter()
    .collect::<HashSet<_>>();
    if covered_service_ids.is_empty()
        || selected_service_ids.is_empty()
        || selected_service_ids
            .iter()
            .any(|service_id| !covered_service_ids.contains(service_id))
    {
        return Err(ApiError::validation("إحدى الخدمات غير مغطاة بهذا الاشتراك.")
            .field("services", "اختر خدمات الاشتراك المحددة فقط."));
    }
    Ok(())
}

async fn lock_patient(
    transaction: &mut Transaction<'_, MySql>,
    patient_id: u64,
    require_active: bool,
) -> ApiResult<PatientRow> {
    let sql = if require_active {
        "SELECT * FROM patients WHERE id = ? AND is_active = 1 FOR UPDATE"
    } else {
        "SELECT * FROM patients WHERE id = ? FOR UPDATE"
    };
    sqlx::query_as::<_, PatientRow>(sql)
        .bind(patient_id)
        .fetch_optional(&mut **transaction)
        .await?
        .ok_or_else(|| {
            if require_active {
                ApiError::validation("المريض المحدد غير متاح.")
                    .field("patient_id", "اختر مريضًا نشطًا.")
            } else {
                ApiError::not_found()
            }
        })
}

async fn active_doctor(
    transaction: &mut Transaction<'_, MySql>,
    doctor_id: u64,
) -> ApiResult<DoctorRow> {
    sqlx::query_as::<_, DoctorRow>("SELECT * FROM doctors WHERE id = ? AND is_active = 1")
        .bind(doctor_id)
        .fetch_optional(&mut **transaction)
        .await?
        .ok_or_else(|| {
            ApiError::validation("الطبيب المحدد غير متاح.").field("doctor_id", "اختر طبيبًا نشطًا.")
        })
}

async fn user_role(transaction: &mut Transaction<'_, MySql>, user_id: u64) -> ApiResult<String> {
    sqlx::query_scalar::<_, String>("SELECT role FROM users WHERE id = ?")
        .bind(user_id)
        .fetch_optional(&mut **transaction)
        .await?
        .ok_or_else(ApiError::not_found)
}

async fn user_summary(transaction: &mut Transaction<'_, MySql>, user_id: u64) -> ApiResult<Value> {
    let row = sqlx::query("SELECT id, name FROM users WHERE id = ?")
        .bind(user_id)
        .fetch_one(&mut **transaction)
        .await?;
    Ok(json!({
        "id": row.try_get::<u64, _>("id")?,
        "name": row.try_get::<String, _>("name")?,
    }))
}

fn object(value: &Value) -> ApiResult<&Map<String, Value>> {
    value.as_object().ok_or_else(|| {
        ApiError::validation("البيانات المرسلة غير صحيحة.")
            .field("request", "يجب إرسال كائن JSON صالح.")
    })
}

fn field_u64(data: &Map<String, Value>, field: &str, current: Option<u64>) -> ApiResult<u64> {
    data.get(field)
        .and_then(value_u64)
        .or(current)
        .ok_or_else(|| {
            ApiError::validation("البيانات المرسلة غير صحيحة.")
                .field(field, "هذا الحقل مطلوب ويجب أن يكون رقمًا صحيحًا.")
        })
}

fn field_optional_u64(
    data: &Map<String, Value>,
    field: &str,
    current: Option<u64>,
) -> ApiResult<Option<u64>> {
    match data.get(field) {
        None => Ok(current),
        Some(Value::Null) => Ok(None),
        Some(value) => value_u64(value).map(Some).ok_or_else(|| {
            ApiError::validation("البيانات المرسلة غير صحيحة.")
                .field(field, "قيمة الحقل غير صحيحة.")
        }),
    }
}

fn value_u64(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_str().and_then(|value| value.parse().ok()))
}

fn decimal_field(
    data: &Map<String, Value>,
    field: &str,
    default: Decimal,
    error_field: &str,
) -> ApiResult<Decimal> {
    let Some(value) = data.get(field) else {
        return Ok(default);
    };
    if value.is_null() || value.as_str() == Some("") {
        return Ok(default);
    }
    let parsed = match value {
        Value::String(value) => Decimal::from_str(value).ok(),
        Value::Number(value) => Decimal::from_str(&value.to_string()).ok(),
        _ => None,
    }
    .ok_or_else(|| {
        ApiError::validation("البيانات المرسلة غير صحيحة.")
            .field(error_field, "يجب إدخال مبلغ صالح حتى 3 منازل عشرية.")
    })?;
    if parsed.is_sign_negative() || parsed.scale() > 3 {
        return Err(ApiError::validation("البيانات المرسلة غير صحيحة.")
            .field(error_field, "يجب إدخال مبلغ صالح حتى 3 منازل عشرية."));
    }
    Ok(money::round(parsed))
}

fn parse_id_array(data: &Map<String, Value>, field: &str) -> ApiResult<Vec<u64>> {
    let values = data
        .get(field)
        .and_then(Value::as_array)
        .filter(|values| !values.is_empty())
        .ok_or_else(|| {
            ApiError::validation("اختر خدمة واحدة على الأقل.").field(field, "حقل الخدمات مطلوب.")
        })?;
    let mut ids = Vec::with_capacity(values.len());
    let mut seen = HashSet::new();
    for (index, value) in values.iter().enumerate() {
        let id = value_u64(value).ok_or_else(|| {
            ApiError::validation("البيانات المرسلة غير صحيحة.")
                .field(format!("{field}.{index}"), "معرف الخدمة غير صحيح.")
        })?;
        if !seen.insert(id) {
            return Err(ApiError::validation("البيانات المرسلة غير صحيحة.")
                .field(format!("{field}.{index}"), "لا يجوز تكرار الخدمة نفسها."));
        }
        ids.push(id);
    }
    Ok(ids)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{
        config::CommissionBase,
        db, schema,
        services::{ledger, subscriptions},
    };
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

    fn config() -> ApiConfig {
        ApiConfig {
            host: "127.0.0.1".into(),
            port: 0,
            mysql_host: "127.0.0.1".into(),
            mysql_port: 0,
            database: "test".into(),
            username: "test".into(),
            password: "test".into(),
            commission_base: CommissionBase::Net,
            subscription_follow_up_sessions: 2,
        }
    }

    #[test]
    fn subscription_sessions_do_not_create_doctor_commission() {
        let rate = Decimal::from(25);
        assert!(session_commission(
            true,
            CommissionBase::Net,
            Decimal::from(200),
            Decimal::from(160),
            rate,
        )
        .is_none());

        let individual = session_commission(
            false,
            CommissionBase::Net,
            Decimal::from(200),
            Decimal::from(160),
            rate,
        )
        .expect("individual session commission");
        assert_eq!(individual.rate, rate);
        assert_eq!(individual.base_amount, Decimal::from(160));
        assert_eq!(individual.amount, Decimal::from(40));
    }

    #[test]
    fn subscription_session_rejects_individual_financial_payloads() {
        let valid = json!({ "discount": "0.000" });
        validate_subscription_session_financial_input(valid.as_object().unwrap())
            .expect("zero discount without settlement is valid");

        for invalid in [
            json!({ "discount": "1.000" }),
            json!({
                "settlement": {
                    "wallet_amount": "0.000",
                    "paid_amount": "0.000",
                    "debt_amount": "0.000",
                    "method": null
                }
            }),
            json!({
                "settlement": {
                    "wallet_amount": "10.000",
                    "paid_amount": "0.000",
                    "debt_amount": "0.000",
                    "method": null
                }
            }),
        ] {
            assert!(
                validate_subscription_session_financial_input(invalid.as_object().unwrap())
                    .is_err()
            );
        }
    }

    #[test]
    fn subscription_session_rejects_service_mutation_payloads() {
        validate_subscription_session_update_input(json!({ "doctor_id": 7 }).as_object().unwrap())
            .expect("changing only the supervising doctor is valid");

        for invalid in [
            json!({ "service_ids": [1, 2] }),
            json!({ "services": [{ "service_id": 1 }] }),
            json!({ "subscription_id": 2 }),
        ] {
            assert!(
                validate_subscription_session_update_input(invalid.as_object().unwrap()).is_err()
            );
        }
    }

    async fn seed(pool: &MySqlPool) -> (u64, u64, u64, u64, u64, u64) {
        schema::reset(pool).await.expect("reset test schema");
        let admin_id = sqlx::query(
            "INSERT INTO users (name, username, password, role, is_active, created_at, updated_at) \
             VALUES ('المدير', 'admin', 'test', 'admin', 1, NOW(), NOW())",
        )
        .execute(pool)
        .await
        .expect("insert admin")
        .last_insert_id();
        let receptionist_id = sqlx::query(
            "INSERT INTO users (name, username, password, role, is_active, created_at, updated_at) \
             VALUES ('الاستقبال', 'reception', 'test', 'receptionist', 1, NOW(), NOW())",
        )
        .execute(pool)
        .await
        .expect("insert receptionist")
        .last_insert_id();
        let doctor_id = sqlx::query(
            "INSERT INTO doctors (name, commission_rate, is_active, created_by, created_at, updated_at) \
             VALUES ('الطبيب', 30, 1, ?, NOW(), NOW())",
        )
        .bind(admin_id)
        .execute(pool)
        .await
        .expect("insert doctor")
        .last_insert_id();
        let patient_id = sqlx::query(
            "INSERT INTO patients (name, phone, is_active, created_by, created_at, updated_at) \
             VALUES ('المريض', '0910000000', 1, ?, NOW(), NOW())",
        )
        .bind(admin_id)
        .execute(pool)
        .await
        .expect("insert patient")
        .last_insert_id();
        let service_id = sqlx::query(
            "INSERT INTO services (name, price, is_active, created_by, created_at, updated_at) \
             VALUES ('علاج يدوي', 80, 1, ?, NOW(), NOW())",
        )
        .bind(admin_id)
        .execute(pool)
        .await
        .expect("insert service")
        .last_insert_id();
        let addon_service_id = sqlx::query(
            "INSERT INTO services (name, price, included_units, unit_name, extra_unit_price, is_active, created_by, created_at, updated_at) \
             VALUES ('الحجامة', 30, 7, 'كوب', 5, 1, ?, NOW(), NOW())",
        )
        .bind(admin_id)
        .execute(pool)
        .await
        .expect("insert addon service")
        .last_insert_id();
        (
            admin_id,
            receptionist_id,
            doctor_id,
            patient_id,
            service_id,
            addon_service_id,
        )
    }

    #[tokio::test]
    #[ignore = "requires an isolated MySQL 8 database"]
    async fn individual_session_financial_lifecycle_matches_laravel() {
        let pool = test_pool().await;
        let (admin_id, receptionist_id, doctor_id, patient_id, service_id, addon_service_id) =
            seed(&pool).await;

        let mut transaction = pool.begin().await.expect("begin top-up");
        ledger::top_up(
            &mut transaction,
            patient_id,
            Decimal::from(100),
            "cash",
            None,
            receptionist_id,
        )
        .await
        .expect("top up wallet");
        transaction.commit().await.expect("commit top-up");

        let payload = json!({
            "patient_id": patient_id,
            "doctor_id": doctor_id,
            "type": "individual",
            "service_ids": [service_id],
            "discount": "0.000",
            "settlement": {
                "wallet_amount": "20.000",
                "paid_amount": "30.000",
                "debt_amount": "30.000",
                "method": "cash",
            },
        });
        let mut transaction = pool.begin().await.expect("begin session");
        let created = create_and_close(&mut transaction, &payload, receptionist_id, &config())
            .await
            .expect("create and close individual session");
        let session_id = created.session["id"].as_u64().expect("session id");
        let debt_id = created.effects.debt.as_ref().unwrap().id;
        assert_eq!(created.session["status"], "closed");
        assert_eq!(created.session["commission_amount"], "15.000");
        assert_eq!(
            created.effects.payment.as_ref().unwrap().amount,
            Decimal::from(30)
        );
        assert_eq!(
            created.effects.debt.as_ref().unwrap().amount,
            Decimal::from(30)
        );
        transaction.commit().await.expect("commit session");

        let addition_payload = json!({
            "services": [{
                "service_id": addon_service_id,
                "item_type": "extra_units",
                "extra_units": 3,
            }],
            "settlement": {
                "wallet_amount": "0.000",
                "paid_amount": "15.000",
                "debt_amount": "0.000",
                "method": "cash",
            },
        });
        let mut transaction = pool.begin().await.expect("begin addition");
        let addition = add_services(
            &mut transaction,
            session_id,
            &addition_payload,
            receptionist_id,
        )
        .await
        .expect("append extra units");
        assert_eq!(addition.session["net_total"], "95.000");
        assert_eq!(addition.session["commission_amount"], "19.500");
        transaction.commit().await.expect("commit addition");

        let mut transaction = pool.begin().await.expect("begin debt settlement");
        ledger::settle_debts(
            &mut transaction,
            Decimal::from(15),
            "cash",
            &[(debt_id, Decimal::from(15))],
            receptionist_id,
        )
        .await
        .expect("settle half the debt");
        let settled_row = fetch_locked_session(&mut transaction, session_id)
            .await
            .expect("fetch settled session");
        let settled = session_value(&mut transaction, &settled_row, false)
            .await
            .expect("load settled session");
        assert_eq!(settled["commission_amount"], "24.000");
        transaction.commit().await.expect("commit debt settlement");

        let mut transaction = pool.begin().await.expect("begin cancel");
        let cancelled = cancel(&mut transaction, session_id, "إلغاء إداري", admin_id)
            .await
            .expect("cancel closed session");
        assert_eq!(cancelled.session["status"], "cancelled");
        assert!(cancelled.session["commission_amount"].is_null());
        transaction.commit().await.expect("commit cancellation");

        let wallet = sqlx::query_scalar::<_, Option<Decimal>>(
            "SELECT SUM(amount) FROM wallet_transactions WHERE patient_id = ?",
        )
        .bind(patient_id)
        .fetch_one(&pool)
        .await
        .expect("wallet balance")
        .unwrap_or(Decimal::ZERO);
        assert_eq!(wallet, Decimal::from(100));
        let refunds = sqlx::query_scalar::<_, Option<Decimal>>(
            "SELECT SUM(ABS(amount)) FROM payments WHERE session_id = ? AND type = 'refund'",
        )
        .bind(session_id)
        .fetch_one(&pool)
        .await
        .expect("refund total")
        .unwrap_or(Decimal::ZERO);
        assert_eq!(refunds, Decimal::from(45));
        let debt_status = sqlx::query_scalar::<_, String>(
            "SELECT status FROM debts WHERE session_id = ? LIMIT 1",
        )
        .bind(session_id)
        .fetch_one(&pool)
        .await
        .expect("debt status");
        assert_eq!(debt_status, "void");
    }

    #[tokio::test]
    #[ignore = "requires an isolated MySQL 8 database"]
    async fn subscription_sessions_consume_one_shared_allowance_and_restore_once() {
        let pool = test_pool().await;
        let (admin_id, receptionist_id, doctor_id, patient_id, service_id, addon_service_id) =
            seed(&pool).await;

        let mut transaction = pool.begin().await.expect("begin subscription");
        let created = subscriptions::create(
            &mut transaction,
            &subscriptions::CreateInput {
                patient_id,
                name: "تأهيل متكامل".into(),
                service_ids: vec![service_id, addon_service_id],
                sessions_count: 1,
                total_price: Decimal::from(800),
                paid_amount: Decimal::from(800),
                method: Some("cash".into()),
            },
            receptionist_id,
        )
        .await
        .expect("create subscription");
        let subscription_id = created.subscription["id"]
            .as_u64()
            .expect("subscription id");
        transaction.commit().await.expect("commit subscription");

        let mut transaction = pool.begin().await.expect("begin subscription session");
        let session = create_and_close(
            &mut transaction,
            &json!({
                "patient_id": patient_id,
                "doctor_id": doctor_id,
                "type": "subscription",
                "subscription_id": subscription_id,
                "services": [
                    { "service_id": service_id, "item_type": "service", "extra_units": 0 },
                    { "service_id": addon_service_id, "item_type": "service", "extra_units": 0 }
                ],
            }),
            receptionist_id,
            &config(),
        )
        .await
        .expect("close subscription session");
        let session_id = session.session["id"].as_u64().unwrap();
        assert_eq!(session.session["net_total"], "0.000");
        assert_eq!(session.session["services_total"], "0.000");
        assert!(session.effects.subscription_usage.is_some());
        assert!(session.effects.wallet_transaction.is_none());
        assert!(session.effects.payment.is_none());
        assert!(session.effects.debt.is_none());
        assert!(session.session["commission_rate"].is_null());
        assert!(session.session["commission_base_amount"].is_null());
        assert!(session.session["commission_amount"].is_null());
        let replay = close(&mut transaction, session_id, receptionist_id, &config())
            .await
            .expect("replay close");
        assert!(replay.replayed);
        transaction
            .commit()
            .await
            .expect("commit subscription session");

        let usage_count = sqlx::query_scalar::<_, u64>(
            "SELECT CAST(COUNT(*) AS UNSIGNED) FROM subscription_usages WHERE session_id = ? AND type = 'consume'",
        )
        .bind(session_id)
        .fetch_one(&pool)
        .await
        .expect("usage count");
        assert_eq!(usage_count, 1);
        let completed =
            sqlx::query_as::<_, SubscriptionRow>("SELECT * FROM subscriptions WHERE id = ?")
                .bind(subscription_id)
                .fetch_one(&pool)
                .await
                .expect("completed subscription");
        assert_eq!(completed.sessions_used, 1);
        assert_eq!(completed.status, "completed");

        let mut transaction = pool.begin().await.expect("begin excess session");
        let excess = create_and_close(
            &mut transaction,
            &json!({
                "patient_id": patient_id,
                "doctor_id": doctor_id,
                "type": "subscription",
                "subscription_id": subscription_id,
                "services": [{ "service_id": service_id, "item_type": "service", "extra_units": 0 }],
            }),
            receptionist_id,
            &config(),
        )
        .await;
        assert!(excess.is_err());
        transaction
            .rollback()
            .await
            .expect("rollback excess session");

        let mut transaction = pool.begin().await.expect("begin prohibited addition");
        let prohibited_addition = add_services(
            &mut transaction,
            session_id,
            &json!({
                "services": [{
                    "service_id": addon_service_id,
                    "item_type": "service",
                    "extra_units": 0,
                }]
            }),
            receptionist_id,
        )
        .await;
        assert!(prohibited_addition.is_err());
        transaction
            .rollback()
            .await
            .expect("rollback prohibited addition");
        let addition_count = sqlx::query_scalar::<_, u64>(
            "SELECT CAST(COUNT(*) AS UNSIGNED) FROM session_additions WHERE session_id = ?",
        )
        .bind(session_id)
        .fetch_one(&pool)
        .await
        .expect("subscription addition count");
        assert_eq!(addition_count, 0);

        let session_payment_count = sqlx::query_scalar::<_, u64>(
            "SELECT CAST(COUNT(*) AS UNSIGNED) FROM payments WHERE session_id = ?",
        )
        .bind(session_id)
        .fetch_one(&pool)
        .await
        .expect("subscription session payment count");
        assert_eq!(session_payment_count, 0);
        let debt_count = sqlx::query_scalar::<_, u64>(
            "SELECT CAST(COUNT(*) AS UNSIGNED) FROM debts WHERE session_id = ?",
        )
        .bind(session_id)
        .fetch_one(&pool)
        .await
        .expect("subscription session debt count");
        assert_eq!(debt_count, 0);
        let wallet_debit_count = sqlx::query_scalar::<_, u64>(
            "SELECT CAST(COUNT(*) AS UNSIGNED) FROM wallet_transactions WHERE session_id = ?",
        )
        .bind(session_id)
        .fetch_one(&pool)
        .await
        .expect("subscription session wallet movement count");
        assert_eq!(wallet_debit_count, 0);

        let mut transaction = pool.begin().await.expect("begin subscription cancellation");
        cancel(&mut transaction, session_id, "إلغاء جلسة الاشتراك", admin_id)
            .await
            .expect("cancel subscription session");
        transaction
            .commit()
            .await
            .expect("commit subscription cancellation");
        let restored =
            sqlx::query_as::<_, SubscriptionRow>("SELECT * FROM subscriptions WHERE id = ?")
                .bind(subscription_id)
                .fetch_one(&pool)
                .await
                .expect("restored subscription");
        assert_eq!(restored.sessions_used, 0);
        assert_eq!(restored.status, "active");
        let restore_count = sqlx::query_scalar::<_, u64>(
            "SELECT CAST(COUNT(*) AS UNSIGNED) FROM subscription_usages WHERE session_id = ? AND type = 'restore'",
        )
        .bind(session_id)
        .fetch_one(&pool)
        .await
        .expect("restore count");
        assert_eq!(restore_count, 1);
    }

    #[tokio::test]
    #[ignore = "requires an isolated MySQL 8 database"]
    async fn concurrent_subscription_sessions_cannot_exceed_the_shared_allowance() {
        let pool = test_pool().await;
        let (_admin_id, receptionist_id, doctor_id, patient_id, service_id, _) = seed(&pool).await;

        let mut transaction = pool.begin().await.expect("begin subscription");
        let created = subscriptions::create(
            &mut transaction,
            &subscriptions::CreateInput {
                patient_id,
                name: "اشتراك متزامن".into(),
                service_ids: vec![service_id],
                sessions_count: 1,
                total_price: Decimal::from(100),
                paid_amount: Decimal::from(100),
                method: Some("cash".into()),
            },
            receptionist_id,
        )
        .await
        .expect("create concurrent subscription");
        let subscription_id = created.subscription["id"].as_u64().unwrap();
        transaction.commit().await.expect("commit subscription");

        let payload = json!({
            "patient_id": patient_id,
            "doctor_id": doctor_id,
            "type": "subscription",
            "subscription_id": subscription_id,
            "services": [{ "service_id": service_id, "item_type": "service", "extra_units": 0 }],
        });
        let first_pool = pool.clone();
        let first_payload = payload.clone();
        let first = async move {
            let mut transaction = first_pool.begin().await.expect("begin first request");
            let result =
                create_and_close(&mut transaction, &first_payload, receptionist_id, &config())
                    .await;
            if result.is_ok() {
                transaction.commit().await.expect("commit first request");
            } else {
                transaction
                    .rollback()
                    .await
                    .expect("rollback first request");
            }
            result
        };
        let second_pool = pool.clone();
        let second = async move {
            let mut transaction = second_pool.begin().await.expect("begin second request");
            let result =
                create_and_close(&mut transaction, &payload, receptionist_id, &config()).await;
            if result.is_ok() {
                transaction.commit().await.expect("commit second request");
            } else {
                transaction
                    .rollback()
                    .await
                    .expect("rollback second request");
            }
            result
        };
        let (first_result, second_result) = tokio::join!(first, second);
        assert_eq!(
            [first_result.is_ok(), second_result.is_ok()]
                .into_iter()
                .filter(|success| *success)
                .count(),
            1
        );

        let subscription =
            sqlx::query_as::<_, SubscriptionRow>("SELECT * FROM subscriptions WHERE id = ?")
                .bind(subscription_id)
                .fetch_one(&pool)
                .await
                .expect("load concurrent subscription");
        assert_eq!(subscription.sessions_used, 1);
        assert_eq!(subscription.status, "completed");
        let session_count = sqlx::query_scalar::<_, u64>(
            "SELECT CAST(COUNT(*) AS UNSIGNED) FROM sessions WHERE subscription_id = ? AND status = 'closed'",
        )
        .bind(subscription_id)
        .fetch_one(&pool)
        .await
        .expect("count concurrent subscription sessions");
        assert_eq!(session_count, 1);
    }
}
