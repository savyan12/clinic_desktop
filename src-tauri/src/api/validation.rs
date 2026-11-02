use super::error::{ApiError, ApiResult};
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde_json::{Map, Value};
use std::str::FromStr;

pub fn object(value: &Value) -> ApiResult<&Map<String, Value>> {
    value.as_object().ok_or_else(|| {
        ApiError::validation("البيانات المرسلة غير صحيحة.")
            .field("request", "يجب إرسال كائن JSON صالح.")
    })
}

pub fn required_string(
    data: &Map<String, Value>,
    field: &str,
    max: usize,
    message: &str,
) -> ApiResult<String> {
    let value = data
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::validation(message).field(field, message))?;
    if value.chars().count() > max {
        return Err(ApiError::validation("البيانات المرسلة غير صحيحة.")
            .field(field, format!("يجب ألا يتجاوز الحقل {max} حرفًا.")));
    }
    Ok(value.to_owned())
}

pub fn optional_string(
    data: &Map<String, Value>,
    field: &str,
    max: usize,
) -> ApiResult<Option<String>> {
    let Some(value) = data.get(field) else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    let value = value.as_str().ok_or_else(|| {
        ApiError::validation("البيانات المرسلة غير صحيحة.").field(field, "قيمة الحقل غير صحيحة.")
    })?;
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    if value.chars().count() > max {
        return Err(ApiError::validation("البيانات المرسلة غير صحيحة.")
            .field(field, format!("يجب ألا يتجاوز الحقل {max} حرفًا.")));
    }
    Ok(Some(value.to_owned()))
}

pub fn boolean(data: &Map<String, Value>, field: &str, default: bool) -> ApiResult<bool> {
    match data.get(field) {
        None => Ok(default),
        Some(Value::Bool(value)) => Ok(*value),
        Some(Value::Number(value)) if value.as_u64().is_some_and(|value| value <= 1) => {
            Ok(value.as_u64() == Some(1))
        }
        _ => Err(ApiError::validation("البيانات المرسلة غير صحيحة.")
            .field(field, "قيمة الحقل يجب أن تكون نعم أو لا.")),
    }
}

pub fn required_u64(data: &Map<String, Value>, field: &str) -> ApiResult<u64> {
    data.get(field).and_then(as_u64).ok_or_else(|| {
        ApiError::validation("البيانات المرسلة غير صحيحة.")
            .field(field, "هذا الحقل مطلوب ويجب أن يكون رقمًا صحيحًا.")
    })
}

pub fn optional_u64(data: &Map<String, Value>, field: &str) -> ApiResult<Option<u64>> {
    match data.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => as_u64(value).map(Some).ok_or_else(|| {
            ApiError::validation("البيانات المرسلة غير صحيحة.")
                .field(field, "قيمة الحقل غير صحيحة.")
        }),
    }
}

pub fn required_decimal(
    data: &Map<String, Value>,
    field: &str,
    positive: bool,
) -> ApiResult<Decimal> {
    let value = data.get(field).and_then(as_decimal).ok_or_else(|| {
        ApiError::validation("البيانات المرسلة غير صحيحة.").field(
            field,
            "هذا الحقل مطلوب ويجب أن يكون مبلغًا صالحًا حتى 3 منازل عشرية.",
        )
    })?;
    validate_decimal(value, field, positive)
}

pub fn optional_decimal(
    data: &Map<String, Value>,
    field: &str,
    default: Decimal,
) -> ApiResult<Decimal> {
    let Some(raw) = data.get(field) else {
        return Ok(default);
    };
    if raw.is_null() || raw.as_str() == Some("") {
        return Ok(default);
    }
    let value = as_decimal(raw).ok_or_else(|| {
        ApiError::validation("البيانات المرسلة غير صحيحة.")
            .field(field, "يجب إدخال مبلغ صالح حتى 3 منازل عشرية.")
    })?;
    validate_decimal(value, field, false)
}

pub fn required_date(data: &Map<String, Value>, field: &str) -> ApiResult<NaiveDate> {
    let raw = data.get(field).and_then(Value::as_str).ok_or_else(|| {
        ApiError::validation("البيانات المرسلة غير صحيحة.").field(field, "هذا التاريخ مطلوب.")
    })?;
    NaiveDate::parse_from_str(raw, "%Y-%m-%d").map_err(|_| {
        ApiError::validation("البيانات المرسلة غير صحيحة.").field(field, "صيغة التاريخ غير صحيحة.")
    })
}

pub fn optional_date(data: &Map<String, Value>, field: &str) -> ApiResult<Option<NaiveDate>> {
    match data.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) if value.is_empty() => Ok(None),
        Some(Value::String(value)) => NaiveDate::parse_from_str(value, "%Y-%m-%d")
            .map(Some)
            .map_err(|_| {
                ApiError::validation("البيانات المرسلة غير صحيحة.")
                    .field(field, "صيغة التاريخ غير صحيحة.")
            }),
        _ => Err(ApiError::validation("البيانات المرسلة غير صحيحة.")
            .field(field, "صيغة التاريخ غير صحيحة.")),
    }
}

pub fn one_of(value: &str, allowed: &[&str], field: &str) -> ApiResult<()> {
    if allowed.contains(&value) {
        Ok(())
    } else {
        Err(ApiError::validation("البيانات المرسلة غير صحيحة.")
            .field(field, "القيمة المحددة غير صحيحة."))
    }
}

fn as_u64(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_str().and_then(|value| value.parse().ok()))
}

fn as_decimal(value: &Value) -> Option<Decimal> {
    match value {
        Value::String(value) => Decimal::from_str(value).ok(),
        Value::Number(value) => Decimal::from_str(&value.to_string()).ok(),
        _ => None,
    }
}

fn validate_decimal(value: Decimal, field: &str, positive: bool) -> ApiResult<Decimal> {
    if value.scale() > 3 || value.is_sign_negative() || (positive && value.is_zero()) {
        return Err(ApiError::validation("البيانات المرسلة غير صحيحة.")
            .field(field, "يجب إدخال مبلغ صالح حتى 3 منازل عشرية."));
    }
    Ok(value)
}
