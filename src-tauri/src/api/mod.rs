pub mod audit;
pub mod auth;
pub mod config;
pub mod db;
pub mod error;
pub mod exports;
pub mod handlers;
pub mod idempotency;
pub mod loaders;
pub mod money;
pub mod pagination;
pub mod query;
pub mod resources;
pub mod rows;
pub mod schema;
pub mod services;
pub mod time;
pub mod validation;

use axum::{
    routing::{get, post},
    Router,
};
use sqlx::MySqlPool;
use tower_http::cors::CorsLayer;

#[derive(Clone)]
pub struct AppState {
    pub pool: MySqlPool,
    pub config: config::ApiConfig,
    pub login_rate_limiter: auth::LoginRateLimiter,
}

pub fn router(state: AppState) -> Router {
    let protected = Router::new()
        .route("/auth/logout", post(handlers::auth::logout))
        .route("/auth/me", get(handlers::auth::me))
        .route("/dashboard", get(handlers::dashboard::show))
        .route(
            "/users",
            get(handlers::users::index).post(handlers::users::store),
        )
        .route("/users/{id}", axum::routing::patch(handlers::users::update))
        .route(
            "/users/{id}/reset-password",
            post(handlers::users::reset_password),
        )
        .route(
            "/settings",
            get(handlers::settings::show).put(handlers::settings::update),
        )
        .route(
            "/services",
            get(handlers::services::index).post(handlers::services::store),
        )
        .route(
            "/services/{id}",
            axum::routing::patch(handlers::services::update),
        )
        .route(
            "/sponsors",
            get(handlers::sponsors::index).post(handlers::sponsors::store),
        )
        .route(
            "/sponsors/{id}",
            get(handlers::sponsors::show).patch(handlers::sponsors::update),
        )
        .route(
            "/sponsors/{id}/statement",
            get(handlers::sponsors::statement),
        )
        .route(
            "/doctors",
            get(handlers::doctors::index).post(handlers::doctors::store),
        )
        .route(
            "/doctors/{id}",
            get(handlers::doctors::show).patch(handlers::doctors::update),
        )
        .route("/doctors/{id}/dues", get(handlers::doctors::dues))
        .route("/doctors/{id}/payouts", post(handlers::doctors::payout))
        .route(
            "/patients/{id}/wallet/movements",
            get(handlers::financial::wallet_movements),
        )
        .route(
            "/patients",
            get(handlers::patients::index).post(handlers::patients::store),
        )
        .route(
            "/patients/{id}",
            get(handlers::patients::show).patch(handlers::patients::update),
        )
        .route(
            "/patients/{id}/statement",
            get(handlers::patients::statement),
        )
        .route(
            "/patients/{id}/wallet/topups",
            post(handlers::financial::wallet_top_up),
        )
        .route(
            "/patients/{id}/debts",
            get(handlers::financial::patient_debts),
        )
        .route(
            "/patients/{id}/payments",
            get(handlers::financial::patient_payments),
        )
        .route(
            "/patients/{id}/opening-debts",
            post(handlers::financial::opening_debt),
        )
        .route("/debts", get(handlers::financial::debts_index))
        .route(
            "/debts/settlements",
            post(handlers::financial::settle_debts),
        )
        .route("/payments", get(handlers::financial::payments_index))
        .route("/refunds", post(handlers::financial::refund))
        .route("/reports/{type}", get(handlers::reports::show))
        .route("/reports/{type}/export", get(handlers::reports::export))
        .route("/audit-log", get(handlers::audit_log::index))
        .route(
            "/sessions",
            get(handlers::sessions::index).post(handlers::sessions::store),
        )
        .route(
            "/sessions/{id}",
            get(handlers::sessions::show).patch(handlers::sessions::update),
        )
        .route("/sessions/{id}/close", post(handlers::sessions::close))
        .route(
            "/sessions/{id}/services",
            post(handlers::sessions::add_services),
        )
        .route("/sessions/{id}/cancel", post(handlers::sessions::cancel))
        .route("/sessions/{id}/correct", post(handlers::sessions::correct));

    let protected = protected
        .route(
            "/subscriptions",
            get(handlers::subscriptions::index).post(handlers::subscriptions::store),
        )
        .route("/subscriptions/{id}", get(handlers::subscriptions::show))
        .route(
            "/subscriptions/{id}/usage",
            get(handlers::subscriptions::usage),
        )
        .route(
            "/subscriptions/{id}/stop",
            post(handlers::subscriptions::stop),
        )
        .route(
            "/subscriptions/{id}/reactivate",
            post(handlers::subscriptions::reactivate),
        )
        .route(
            "/subscriptions/{id}/cancel-preview",
            get(handlers::subscriptions::cancel_preview),
        )
        .route(
            "/subscriptions/{id}/cancel",
            post(handlers::subscriptions::cancel),
        )
        .route(
            "/subscriptions/{id}/correct",
            post(handlers::subscriptions::correct),
        );

    Router::new()
        .nest(
            "/api/v1",
            Router::new()
                .route("/health", get(handlers::auth::health))
                .route("/auth/login", post(handlers::auth::login))
                .merge(protected),
        )
        .layer(CorsLayer::permissive())
        .with_state(state)
}

#[cfg(test)]
mod http_tests;
