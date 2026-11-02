use crate::api::{
    audit,
    auth::AuthUser,
    error::{ApiError, ApiResult},
    money, pagination, resources,
    rows::ServiceRow,
    validation, AppState,
};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use rust_decimal::Decimal;
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Debug, Deserialize)]
pub struct ServiceListQuery {
    search: Option<String>,
    is_active: Option<bool>,
    page: Option<u64>,
    per_page: Option<u64>,
}

pub async fn index(
    State(state): State<AppState>,
    _auth: AuthUser,
    Query(query): Query<ServiceListQuery>,
) -> ApiResult<Json<Value>> {
    let page = pagination::PageQuery {
        page: query.page,
        per_page: query.per_page,
    };
    let search = query.search.as_deref().unwrap_or("").trim();
    let pattern = format!("%{search}%");
    let total = sqlx::query_scalar::<_, u64>(
        "SELECT CAST(COUNT(*) AS UNSIGNED) FROM services WHERE (? = '' OR name LIKE ?) AND (? IS NULL OR is_active = ?)",
    )
    .bind(search)
    .bind(&pattern)
    .bind(query.is_active)
    .bind(query.is_active)
    .fetch_one(&state.pool)
    .await?;
    let rows = sqlx::query_as::<_, ServiceRow>(
        "SELECT * FROM services WHERE (? = '' OR name LIKE ?) AND (? IS NULL OR is_active = ?) \
         ORDER BY id DESC LIMIT ? OFFSET ?",
    )
    .bind(search)
    .bind(&pattern)
    .bind(query.is_active)
    .bind(query.is_active)
    .bind(page.per_page())
    .bind(page.offset())
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(json!({
        "data": rows.iter().map(resources::service_value).collect::<Vec<_>>(),
        "meta": pagination::meta(page.page(), page.per_page(), total),
    })))
}

pub async fn store(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(payload): Json<Value>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    auth.require_admin()?;
    let input = parse_input(&payload, None)?;
    let mut transaction = state.pool.begin().await?;
    let result = sqlx::query(
        "INSERT INTO services (name, price, included_units, unit_name, extra_unit_price, is_active, created_by, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, NOW(), NOW())",
    )
    .bind(&input.name)
    .bind(input.price)
    .bind(input.included_units)
    .bind(&input.unit_name)
    .bind(input.extra_unit_price)
    .bind(input.is_active)
    .bind(auth.user.id)
    .execute(&mut *transaction)
    .await?;
    let id = result.last_insert_id();
    let values = input.snapshot(id, auth.user.id);
    audit::log(
        &mut transaction,
        auth.user.id,
        "service_created",
        "service",
        id,
        None,
        Some(&values),
        None,
    )
    .await?;
    transaction.commit().await?;

    Ok((
        StatusCode::CREATED,
        Json(json!({ "data": resources::service_value(&fetch(&state, id).await?) })),
    ))
}

pub async fn update(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<u64>,
    Json(payload): Json<Value>,
) -> ApiResult<Json<Value>> {
    auth.require_admin()?;
    let current = fetch(&state, id).await?;
    let input = parse_input(&payload, Some(&current))?;
    let price_changed = input.price != current.price;
    if price_changed && input.reason.is_none() {
        return Err(ApiError::validation("سبب تغيير سعر الخدمة مطلوب.")
            .field("reason", "اكتب سبب تغيير السعر."));
    }
    let old_values = service_snapshot(&current);
    let new_values = input.snapshot(id, current.created_by);
    let mut transaction = state.pool.begin().await?;
    sqlx::query(
        "UPDATE services SET name = ?, price = ?, included_units = ?, unit_name = ?, extra_unit_price = ?, is_active = ?, updated_at = NOW() WHERE id = ?",
    )
    .bind(&input.name)
    .bind(input.price)
    .bind(input.included_units)
    .bind(&input.unit_name)
    .bind(input.extra_unit_price)
    .bind(input.is_active)
    .bind(id)
    .execute(&mut *transaction)
    .await?;
    audit::log(
        &mut transaction,
        auth.user.id,
        if price_changed {
            "service_price_changed"
        } else {
            "service_updated"
        },
        "service",
        id,
        Some(&old_values),
        Some(&new_values),
        input.reason.as_deref(),
    )
    .await?;
    transaction.commit().await?;

    Ok(Json(
        json!({ "data": resources::service_value(&fetch(&state, id).await?) }),
    ))
}

struct ServiceInput {
    name: String,
    price: Decimal,
    included_units: Option<u16>,
    unit_name: Option<String>,
    extra_unit_price: Option<Decimal>,
    is_active: bool,
    reason: Option<String>,
}

impl ServiceInput {
    fn snapshot(&self, id: u64, created_by: u64) -> Value {
        json!({
            "id": id,
            "name": self.name,
            "price": money::normalize(self.price),
            "included_units": self.included_units,
            "unit_name": self.unit_name,
            "extra_unit_price": self.extra_unit_price.map(money::normalize),
            "is_active": self.is_active,
            "created_by": created_by,
        })
    }
}

fn parse_input(payload: &Value, current: Option<&ServiceRow>) -> ApiResult<ServiceInput> {
    let data = validation::object(payload)?;
    let name = validation::required_string(data, "name", 255, "اسم الخدمة مطلوب.")?;
    let price = validation::required_decimal(data, "price", false)?;
    let included_units = match data.get("included_units") {
        None | Some(Value::Null) => None,
        Some(value) => Some(
            value
                .as_u64()
                .filter(|value| (1..=10000).contains(value))
                .map(|value| value as u16)
                .ok_or_else(|| {
                    ApiError::validation("البيانات المرسلة غير صحيحة.")
                        .field("included_units", "عدد الوحدات غير صحيح.")
                })?,
        ),
    };
    let unit_name = validation::optional_string(data, "unit_name", 50)?;
    let extra_unit_price = match data.get("extra_unit_price") {
        None | Some(Value::Null) => None,
        Some(_) => Some(validation::optional_decimal(
            data,
            "extra_unit_price",
            Decimal::ZERO,
        )?),
    };
    let configured = [
        included_units.is_some(),
        unit_name.is_some(),
        extra_unit_price.is_some(),
    ]
    .into_iter()
    .filter(|value| *value)
    .count();
    if configured != 0 && configured != 3 {
        return Err(ApiError::validation("البيانات المرسلة غير صحيحة.").field(
            "included_units",
            "أدخل عدد الوحدات المشمولة واسم الوحدة وسعر الوحدة الإضافية معًا.",
        ));
    }
    Ok(ServiceInput {
        name,
        price,
        included_units,
        unit_name,
        extra_unit_price,
        is_active: validation::boolean(data, "is_active", current.is_none_or(|row| row.is_active))?,
        reason: validation::optional_string(data, "reason", 1000)?,
    })
}

async fn fetch(state: &AppState, id: u64) -> ApiResult<ServiceRow> {
    sqlx::query_as::<_, ServiceRow>("SELECT * FROM services WHERE id = ?")
        .bind(id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(ApiError::not_found)
}

fn service_snapshot(service: &ServiceRow) -> Value {
    json!({
        "id": service.id,
        "name": service.name,
        "price": money::normalize(service.price),
        "included_units": service.included_units,
        "unit_name": service.unit_name,
        "extra_unit_price": service.extra_unit_price.map(money::normalize),
        "is_active": service.is_active,
        "created_by": service.created_by,
    })
}
