use crate::api::{
    audit,
    auth::AuthUser,
    error::{ApiError, ApiResult},
    resources,
    rows::SettingRow,
    validation, AppState,
};
use axum::{extract::State, Json};
use serde_json::{json, Value};

pub async fn show(State(state): State<AppState>, _auth: AuthUser) -> ApiResult<Json<Value>> {
    let setting = get_or_create(&state).await?;
    Ok(Json(json!({ "data": resources::setting_value(&setting) })))
}

pub async fn update(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(payload): Json<Value>,
) -> ApiResult<Json<Value>> {
    auth.require_admin()?;
    let current = get_or_create(&state).await?;
    let data = validation::object(&payload)?;
    let clinic_name = validation::required_string(data, "clinic_name", 255, "اسم العيادة مطلوب.")?;
    let clinic_subtitle = validation::optional_string(data, "clinic_subtitle", 255)?;
    let phone = validation::optional_string(data, "phone", 30)?;
    let address = validation::optional_string(data, "address", 255)?;
    let receipt_footer = validation::optional_string(data, "receipt_footer", 1000)?;
    let currency = validation::required_string(data, "currency", 10, "العملة مطلوبة.")?;
    let print_size = validation::required_string(data, "print_size", 2, "حجم الطباعة مطلوب.")?;
    validation::one_of(&print_size, &["A4", "A5"], "print_size")?;
    let show_logo = validation::boolean(data, "show_logo", current.show_logo)?;
    let body_font_size = bounded_u8(data, "body_font_size", current.body_font_size, 14, 20)?;
    let body_font_weight = font_weight(data, "body_font_weight", current.body_font_weight)?;
    let table_font_size = bounded_u8(data, "table_font_size", current.table_font_size, 13, 18)?;
    let table_font_weight = font_weight(data, "table_font_weight", current.table_font_weight)?;
    let heading_font_size =
        bounded_u8(data, "heading_font_size", current.heading_font_size, 24, 40)?;
    let heading_font_weight =
        font_weight(data, "heading_font_weight", current.heading_font_weight)?;
    let section_font_size =
        bounded_u8(data, "section_font_size", current.section_font_size, 18, 28)?;
    let section_font_weight =
        font_weight(data, "section_font_weight", current.section_font_weight)?;

    let old_values = row_snapshot(&current);
    let mut transaction = state.pool.begin().await?;
    sqlx::query(
        "UPDATE settings SET clinic_name = ?, clinic_subtitle = ?, phone = ?, address = ?, \
         receipt_footer = ?, currency = ?, print_size = ?, show_logo = ?, body_font_size = ?, \
         body_font_weight = ?, table_font_size = ?, table_font_weight = ?, heading_font_size = ?, \
         heading_font_weight = ?, section_font_size = ?, section_font_weight = ?, updated_by = ?, updated_at = NOW() \
         WHERE id = ?",
    )
    .bind(&clinic_name)
    .bind(&clinic_subtitle)
    .bind(&phone)
    .bind(&address)
    .bind(&receipt_footer)
    .bind(&currency)
    .bind(&print_size)
    .bind(show_logo)
    .bind(body_font_size)
    .bind(body_font_weight)
    .bind(table_font_size)
    .bind(table_font_weight)
    .bind(heading_font_size)
    .bind(heading_font_weight)
    .bind(section_font_size)
    .bind(section_font_weight)
    .bind(auth.user.id)
    .bind(current.id)
    .execute(&mut *transaction)
    .await?;
    let new_values = json!({
        "clinic_name": clinic_name,
        "clinic_subtitle": clinic_subtitle,
        "phone": phone,
        "address": address,
        "receipt_footer": receipt_footer,
        "currency": currency,
        "print_size": print_size,
        "show_logo": show_logo,
        "body_font_size": body_font_size,
        "body_font_weight": body_font_weight,
        "table_font_size": table_font_size,
        "table_font_weight": table_font_weight,
        "heading_font_size": heading_font_size,
        "heading_font_weight": heading_font_weight,
        "section_font_size": section_font_size,
        "section_font_weight": section_font_weight,
        "updated_by": auth.user.id,
    });
    audit::log(
        &mut transaction,
        auth.user.id,
        "settings_updated",
        "settings",
        current.id,
        Some(&old_values),
        Some(&new_values),
        None,
    )
    .await?;
    transaction.commit().await?;

    Ok(Json(
        json!({ "data": resources::setting_value(&get_or_create(&state).await?) }),
    ))
}

pub(crate) async fn get_or_create(state: &AppState) -> ApiResult<SettingRow> {
    if let Some(setting) =
        sqlx::query_as::<_, SettingRow>("SELECT * FROM settings ORDER BY id LIMIT 1")
            .fetch_optional(&state.pool)
            .await?
    {
        return Ok(setting);
    }

    sqlx::query(
        "INSERT INTO settings \
         (clinic_name, clinic_subtitle, currency, print_size, show_logo, body_font_size, body_font_weight, \
          table_font_size, table_font_weight, heading_font_size, heading_font_weight, section_font_size, section_font_weight, created_at, updated_at) \
         VALUES ('عيادة عين الحياة', 'للعلاج الطبيعي والتأهيل', 'د.ل', 'A4', 1, 16, 400, 15, 400, 32, 600, 22, 600, NOW(), NOW())",
    )
    .execute(&state.pool)
    .await?;
    sqlx::query_as::<_, SettingRow>("SELECT * FROM settings ORDER BY id LIMIT 1")
        .fetch_one(&state.pool)
        .await
        .map_err(Into::into)
}

fn bounded_u8(
    data: &serde_json::Map<String, Value>,
    field: &str,
    default: u8,
    minimum: u8,
    maximum: u8,
) -> ApiResult<u8> {
    let Some(raw) = data.get(field) else {
        return Ok(default);
    };
    let value = raw
        .as_u64()
        .filter(|value| (*value as u128) <= u8::MAX as u128);
    match value
        .map(|value| value as u8)
        .filter(|value| (*value >= minimum) && (*value <= maximum))
    {
        Some(value) => Ok(value),
        None => Err(ApiError::validation("البيانات المرسلة غير صحيحة.").field(
            field,
            format!("يجب أن تكون القيمة بين {minimum} و{maximum}."),
        )),
    }
}

fn font_weight(data: &serde_json::Map<String, Value>, field: &str, default: u16) -> ApiResult<u16> {
    let value = data
        .get(field)
        .and_then(Value::as_u64)
        .map(|value| value as u16)
        .unwrap_or(default);
    if matches!(value, 400 | 600) {
        Ok(value)
    } else {
        Err(ApiError::validation("البيانات المرسلة غير صحيحة.")
            .field(field, "القيمة المسموحة هي 400 أو 600 فقط."))
    }
}

fn row_snapshot(setting: &SettingRow) -> Value {
    json!({
        "id": setting.id,
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
        "updated_by": setting.updated_by,
    })
}
