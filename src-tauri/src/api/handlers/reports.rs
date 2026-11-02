use crate::api::{
    auth::AuthUser,
    error::{ApiError, ApiResult},
    services::reports::{self, ReportFilters},
    AppState,
};
use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, HeaderValue},
    response::Response,
    Json,
};
use chrono::NaiveDate;
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Debug, Clone, Deserialize)]
pub struct ReportQuery {
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
    pub doctor_id: Option<u64>,
    pub patient_id: Option<u64>,
    pub sponsor_id: Option<u64>,
    pub status: Option<String>,
    pub kind: Option<String>,
    pub r#type: Option<String>,
    pub method: Option<String>,
    pub near_end: Option<bool>,
    pub format: Option<String>,
}

pub async fn show(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(report_type): Path<String>,
    Query(query): Query<ReportQuery>,
) -> ApiResult<Json<Value>> {
    auth.require_admin()?;
    validate_period(&query)?;
    let report = reports::report(&state, &report_type, &ReportFilters::from(&query)).await?;
    Ok(Json(json!({ "data": report })))
}

pub async fn export(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(report_type): Path<String>,
    Query(query): Query<ReportQuery>,
) -> ApiResult<Response> {
    auth.require_admin()?;
    validate_period(&query)?;
    let filters = ReportFilters::from(&query);
    let report = reports::report(&state, &report_type, &filters).await?;
    let format = query.format.as_deref().unwrap_or("");
    let exported = crate::api::exports::export(&report_type, format, &report)?;
    let disposition = format!(
        "attachment; filename=\"report-{report_type}.{format}\"; filename*=UTF-8''{}",
        encode_header_filename(&exported.filename)
    );
    let content_type = HeaderValue::from_static(exported.content_type);
    let disposition = HeaderValue::from_str(&disposition)
        .map_err(|error| ApiError::internal(format!("Invalid export filename: {error}")))?;
    Response::builder()
        .status(axum::http::StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CONTENT_DISPOSITION, disposition)
        .body(Body::from(exported.bytes))
        .map_err(ApiError::internal)
}

fn validate_period(query: &ReportQuery) -> ApiResult<()> {
    if query.from.zip(query.to).is_some_and(|(from, to)| from > to) {
        return Err(
            ApiError::validation("تاريخ البداية يجب أن يسبق تاريخ النهاية.")
                .field("from", "الفترة الزمنية غير صحيحة."),
        );
    }
    Ok(())
}

impl From<&ReportQuery> for ReportFilters {
    fn from(query: &ReportQuery) -> Self {
        Self {
            from: query.from,
            to: query.to,
            doctor_id: query.doctor_id,
            patient_id: query.patient_id,
            sponsor_id: query.sponsor_id,
            status: query.status.clone().filter(|value| !value.is_empty()),
            kind: query.kind.clone().filter(|value| !value.is_empty()),
            r#type: query.r#type.clone().filter(|value| !value.is_empty()),
            method: query.method.clone().filter(|value| !value.is_empty()),
            near_end: query.near_end.unwrap_or(false),
        }
    }
}

fn encode_header_filename(value: &str) -> String {
    value
        .as_bytes()
        .iter()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() || matches!(*byte, b'-' | b'_' | b'.') {
                (*byte as char).to_string()
            } else {
                format!("%{byte:02X}")
            }
        })
        .collect()
}
