use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub type ApiResult<T> = Result<T, ApiError>;

#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub message: String,
    pub errors: BTreeMap<String, Vec<String>>,
    retryable_transaction: bool,
    internal_detail: Option<String>,
}

impl ApiError {
    pub fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
            errors: BTreeMap::new(),
            retryable_transaction: false,
            internal_detail: None,
        }
    }

    pub fn validation(message: impl Into<String>) -> Self {
        Self::new(StatusCode::UNPROCESSABLE_ENTITY, message)
    }

    pub fn field(mut self, field: impl Into<String>, message: impl Into<String>) -> Self {
        self.errors
            .entry(field.into())
            .or_default()
            .push(message.into());
        self
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::new(StatusCode::FORBIDDEN, message)
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, message)
    }

    pub fn not_found() -> Self {
        Self::new(StatusCode::NOT_FOUND, "تعذر العثور على السجل المطلوب.")
    }

    pub fn unauthorized() -> Self {
        Self::new(
            StatusCode::UNAUTHORIZED,
            "انتهت جلسة الدخول، يرجى تسجيل الدخول مجددًا.",
        )
    }

    pub fn internal(error: impl std::fmt::Display) -> Self {
        let mut api_error = Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "تعذر إتمام الطلب بسبب خطأ داخلي.",
        );
        api_error.internal_detail = Some(error.to_string());
        api_error
    }

    fn retryable_transaction(error: impl std::fmt::Display) -> Self {
        let mut api_error = Self::internal(error);
        api_error.retryable_transaction = true;
        api_error
    }

    pub fn is_retryable_transaction(&self) -> bool {
        self.retryable_transaction
    }

    pub fn payload(&self) -> Value {
        if self.errors.is_empty() {
            json!({ "message": self.message })
        } else {
            json!({ "message": self.message, "errors": self.errors })
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        if let Some(detail) = &self.internal_detail {
            eprintln!("Rust API internal error: {detail}");
        }
        (self.status, Json(self.payload())).into_response()
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(error: sqlx::Error) -> Self {
        match error {
            sqlx::Error::RowNotFound => Self::not_found(),
            sqlx::Error::Database(database)
                if matches!(database.code().as_deref(), Some("1205" | "1213")) =>
            {
                Self::retryable_transaction(database)
            }
            other => Self::internal(other),
        }
    }
}

impl From<serde_json::Error> for ApiError {
    fn from(error: serde_json::Error) -> Self {
        Self::internal(error)
    }
}
