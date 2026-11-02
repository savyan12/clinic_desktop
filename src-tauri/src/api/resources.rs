use super::{money, rows::*, time};
use rust_decimal::Decimal;
use serde_json::{json, Value};

pub fn patient_value(
    patient: &PatientRow,
    sponsor: Option<Value>,
    wallet_balance: Decimal,
    debt_total: Decimal,
    subscription_status: Option<&str>,
) -> Value {
    json!({
        "id": patient.id,
        "name": patient.name,
        "phone": patient.phone,
        "age": patient.age,
        "gender": patient.gender,
        "area": patient.area,
        "emergency_phone": patient.emergency_phone,
        "sponsor_id": patient.sponsor_id,
        "sponsor": sponsor,
        "wallet_balance": money::normalize(wallet_balance),
        "debt_total": money::normalize(debt_total),
        "subscription_status": subscription_status,
        "is_active": patient.is_active,
        "created_at": patient.created_at.map(time::iso),
    })
}

pub fn sponsor_value(
    sponsor: &SponsorRow,
    patients_count: u64,
    total_due: Decimal,
    total_paid: Decimal,
) -> Value {
    json!({
        "id": sponsor.id,
        "name": sponsor.name,
        "phone": sponsor.phone,
        "address": sponsor.address,
        "is_active": sponsor.is_active,
        "patients_count": patients_count,
        "total_due": money::normalize(total_due),
        "total_paid": money::normalize(total_paid),
        "remaining": money::normalize(total_due - total_paid),
        "created_at": sponsor.created_at.map(time::iso),
    })
}

pub fn doctor_value(
    doctor: &DoctorRow,
    sessions_count: u64,
    earned: Decimal,
    paid: Decimal,
) -> Value {
    json!({
        "id": doctor.id,
        "name": doctor.name,
        "phone": doctor.phone,
        "commission_rate": money::normalize(doctor.commission_rate),
        "is_active": doctor.is_active,
        "sessions_count": sessions_count,
        "earned": money::normalize(earned),
        "paid": money::normalize(paid),
        "remaining": money::normalize(earned - paid),
        "created_at": doctor.created_at.map(time::iso),
    })
}

pub fn service_value(service: &ServiceRow) -> Value {
    json!({
        "id": service.id,
        "name": service.name,
        "price": money::normalize(service.price),
        "included_units": service.included_units,
        "unit_name": service.unit_name,
        "extra_unit_price": service.extra_unit_price.map(money::normalize),
        "is_active": service.is_active,
        "updated_at": service.updated_at.map(time::iso),
    })
}

pub fn subscription_value(
    subscription: &SubscriptionRow,
    patient: Option<Value>,
    sponsor: Option<Value>,
    services: Option<Vec<Value>>,
) -> Value {
    let today = time::today();
    let monthly = subscription.plan_type == "monthly";
    let remaining = (!monthly).then_some(
        subscription
            .sessions_count
            .saturating_sub(subscription.sessions_used),
    );
    let remaining_days = if monthly {
        subscription.end_date.map(|date| {
            if date < today {
                0
            } else {
                (date - today).num_days() + 1
            }
        })
    } else {
        None
    };
    let is_usable = if monthly {
        subscription.status == "active"
            && subscription.start_date <= today
            && subscription.end_date.is_some_and(|date| date >= today)
    } else {
        subscription.status == "active" && subscription.sessions_used < subscription.sessions_count
    };

    let mut value = json!({
        "id": subscription.id,
        "patient_id": subscription.patient_id,
        "plan_type": subscription.plan_type,
        "sessions_count": subscription.sessions_count,
        "sessions_used": subscription.sessions_used,
        "remaining": remaining,
        "remaining_days": remaining_days,
        "is_usable": is_usable,
        "base_total": money::normalize(subscription.base_total),
        "discount": money::normalize(subscription.discount),
        "net_total": money::normalize(subscription.net_total),
        "paid_amount": money::normalize(subscription.paid_amount),
        "remaining_amount": money::normalize(subscription.net_total - subscription.paid_amount),
        "start_date": subscription.start_date.format("%Y-%m-%d").to_string(),
        "expected_end_date": subscription.expected_end_date.map(|date| date.format("%Y-%m-%d").to_string()),
        "end_date": subscription.end_date.map(|date| date.format("%Y-%m-%d").to_string()),
        "status": subscription.status,
        "cancelled_at": subscription.cancelled_at.map(time::iso),
        "cancelled_by": subscription.cancelled_by,
        "cancel_reason": subscription.cancel_reason,
        "refund_amount": money::normalize(subscription.refund_amount),
        "sponsor_id": subscription.sponsor_id,
        "created_by": subscription.created_by,
    });
    let object = value
        .as_object_mut()
        .expect("subscription resource must be an object");
    if let Some(patient) = patient {
        object.insert("patient".into(), patient);
    }
    if let Some(sponsor) = sponsor {
        object.insert("sponsor".into(), sponsor);
    }
    if let Some(services) = services {
        object.insert("services".into(), Value::Array(services));
    }
    value
}

pub fn subscription_service_value(service: &SubscriptionServiceRow) -> Value {
    json!({
        "service_id": service.service_id,
        "name": service.service_name,
        "price": money::normalize(service.price),
    })
}

pub fn session_value(
    session: &SessionRow,
    patient: Option<Value>,
    doctor: Option<Value>,
    services: Option<Vec<Value>>,
    creator: Option<Value>,
    closed_by: Option<Value>,
    cancelled_by: Option<Value>,
    additions: Option<Vec<Value>>,
) -> Value {
    let service_ids = services.as_ref().map(|values| {
        values
            .iter()
            .filter_map(|value| value.get("service_id").cloned())
            .collect::<Vec<_>>()
    });

    let mut value = json!({
        "id": session.id,
        "code": session.code,
        "patient_id": session.patient_id,
        "doctor_id": session.doctor_id,
        "type": session.r#type,
        "subscription_id": session.subscription_id,
        "status": session.status,
        "opened_at": time::iso(session.opened_at),
        "closed_at": session.closed_at.map(time::iso),
        "cancelled_at": session.cancelled_at.map(time::iso),
        "cancel_reason": session.cancel_reason,
        "services_total": money::normalize(session.services_total),
        "discount": money::normalize(session.discount),
        "net_total": money::normalize(session.net_total),
        "settlement": {
            "coverage": session.coverage,
            "wallet_amount": money::normalize(session.wallet_amount),
            "paid_amount": money::normalize(session.paid_amount),
            "debt_amount": money::normalize(session.debt_amount),
            "method": session.payment_method,
            "sponsor_id": session.sponsor_id,
        },
        "commission_rate": session.commission_rate.map(money::normalize),
        "commission_base_amount": session.commission_base_amount.map(money::normalize),
        "commission_amount": session.commission_amount.map(money::normalize),
        "created_by": session.created_by,
        "closed_by": session.closed_by,
        "cancelled_by": session.cancelled_by,
    });
    let object = value
        .as_object_mut()
        .expect("session resource must be an object");
    if let Some(patient) = patient {
        object.insert("patient".into(), patient);
    }
    if let Some(doctor) = doctor {
        object.insert("doctor".into(), doctor);
    }
    if let Some(services) = services {
        object.insert("services".into(), Value::Array(services));
        object.insert(
            "service_ids".into(),
            Value::Array(service_ids.expect("service ids accompany loaded services")),
        );
    }
    if let Some(creator) = creator {
        object.insert("created_by_user".into(), creator);
    }
    if let Some(closed_by) = closed_by {
        object.insert("closed_by_user".into(), closed_by);
    }
    if let Some(cancelled_by) = cancelled_by {
        object.insert("cancelled_by_user".into(), cancelled_by);
    }
    if let Some(additions) = additions {
        object.insert("additions".into(), Value::Array(additions));
    }
    value
}

pub fn session_service_value(service: &SessionServiceRow) -> Value {
    json!({
        "id": service.id,
        "service_id": service.service_id,
        "name": service.service_name,
        "price": money::normalize(service.price),
        "base_price": money::normalize(service.base_price.unwrap_or(service.price)),
        "included_units": service.included_units,
        "extra_units": service.extra_units,
        "total_units": service.included_units + service.extra_units,
        "unit_name": service.unit_name,
        "extra_unit_price": service.extra_unit_price.map(money::normalize),
        "item_type": service.item_type,
        "is_addition": service.is_addition,
        "added_at": service.added_at.map(time::iso),
        "session_addition_id": service.session_addition_id,
    })
}

pub fn session_addition_value(addition: &SessionAdditionRow) -> Value {
    json!({
        "id": addition.id,
        "services_total": money::normalize(addition.services_total),
        "net_total": money::normalize(addition.net_total),
        "coverage": addition.coverage,
        "wallet_amount": money::normalize(addition.wallet_amount),
        "paid_amount": money::normalize(addition.paid_amount),
        "debt_amount": money::normalize(addition.debt_amount),
        "method": addition.payment_method,
        "sponsor_id": addition.sponsor_id,
        "created_by": addition.created_by,
        "created_at": addition.created_at.map(time::iso),
    })
}

pub fn subscription_usage_value(usage: &SubscriptionUsageRow) -> Value {
    json!({
        "id": usage.id,
        "subscription_id": usage.subscription_id,
        "session_id": usage.session_id,
        "type": usage.r#type,
        "reverses_usage_id": usage.reverses_usage_id,
        "created_by": usage.created_by,
        "created_at": usage.created_at.map(time::iso),
    })
}

pub fn payment_value(payment: &PaymentRow, patient: Option<Value>) -> Value {
    let mut value = json!({
        "id": payment.id,
        "patient_id": payment.patient_id,
        "session_id": payment.session_id,
        "session_addition_id": payment.session_addition_id,
        "subscription_id": payment.subscription_id,
        "type": payment.r#type,
        "method": payment.method,
        "amount": money::normalize(payment.amount),
        "reverses_payment_id": payment.reverses_payment_id,
        "note": payment.note,
        "created_by": payment.created_by,
        "created_at": payment.created_at.map(time::iso),
    });
    if let Some(patient) = patient {
        value
            .as_object_mut()
            .expect("payment resource must be an object")
            .insert("patient".into(), patient);
    }
    value
}

pub fn wallet_transaction_value(transaction: &WalletTransactionRow) -> Value {
    json!({
        "id": transaction.id,
        "patient_id": transaction.patient_id,
        "session_id": transaction.session_id,
        "session_addition_id": transaction.session_addition_id,
        "payment_id": transaction.payment_id,
        "type": transaction.r#type,
        "amount": money::normalize(transaction.amount),
        "balance_after": money::normalize(transaction.balance_after),
        "reverses_transaction_id": transaction.reverses_transaction_id,
        "note": transaction.note,
        "created_by": transaction.created_by,
        "created_at": transaction.created_at.map(time::iso),
    })
}

pub fn debt_value(debt: &DebtRow) -> Value {
    let remaining = if debt.status == "void" {
        Decimal::ZERO
    } else {
        debt.amount - debt.paid_amount
    };
    json!({
        "id": debt.id,
        "patient_id": debt.patient_id,
        "sponsor_id": debt.sponsor_id,
        "session_id": debt.session_id,
        "session_addition_id": debt.session_addition_id,
        "subscription_id": debt.subscription_id,
        "kind": debt.kind,
        "amount": money::normalize(debt.amount),
        "paid_amount": money::normalize(debt.paid_amount),
        "remaining": money::normalize(remaining),
        "status": debt.status,
        "voided_at": debt.voided_at.map(time::iso),
        "void_reason": debt.void_reason,
        "note": debt.note,
        "created_by": debt.created_by,
        "created_at": debt.created_at.map(time::iso),
    })
}

pub fn allocation_value(allocation: &PaymentAllocationRow) -> Value {
    json!({
        "id": allocation.id,
        "payment_id": allocation.payment_id,
        "debt_id": allocation.debt_id,
        "amount": money::normalize(allocation.amount),
        "created_by": allocation.created_by,
        "created_at": allocation.created_at.map(time::iso),
    })
}

pub fn doctor_payout_value(payout: &DoctorPayoutRow) -> Value {
    json!({
        "id": payout.id,
        "doctor_id": payout.doctor_id,
        "amount": money::normalize(payout.amount),
        "date": payout.date.format("%Y-%m-%d").to_string(),
        "method": payout.method,
        "note": payout.note,
        "created_by": payout.created_by,
        "created_at": payout.created_at.map(time::iso),
    })
}

pub fn audit_value(audit: &AuditLogRow, user: Option<Value>) -> Value {
    json!({
        "id": audit.id,
        "user_id": audit.user_id,
        "user": user,
        "action": audit.action,
        "entity_type": audit.entity_type,
        "entity_id": audit.entity_id,
        "old_values": audit.old_values,
        "new_values": audit.new_values,
        "reason": audit.reason,
        "created_at": time::iso(audit.created_at),
    })
}

pub fn setting_value(setting: &SettingRow) -> Value {
    json!({
        "clinic_name": setting.clinic_name,
        "clinic_subtitle": setting.clinic_subtitle,
        "phone": setting.phone,
        "address": setting.address,
        "receipt_footer": setting.receipt_footer,
        "currency": setting.currency,
        "print_size": setting.print_size,
        "show_logo": setting.show_logo,
        "body_font_size": setting.body_font_size,
        "body_font_weight": setting.body_font_weight,
        "table_font_size": setting.table_font_size,
        "table_font_weight": setting.table_font_weight,
        "heading_font_size": setting.heading_font_size,
        "heading_font_weight": setting.heading_font_weight,
        "section_font_size": setting.section_font_size,
        "section_font_weight": setting.section_font_weight,
    })
}
