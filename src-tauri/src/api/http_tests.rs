use super::{
    config::{ApiConfig, CommissionBase},
    db, router, schema, AppState,
};
use axum::{
    body::{to_bytes, Body},
    http::{header, HeaderMap, Method, Request, StatusCode},
    Router,
};
use bcrypt::hash;
use rust_decimal::Decimal;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::MySqlPool;
use std::sync::OnceLock;
use tokio::sync::{Mutex, MutexGuard};
use tower::ServiceExt;
use uuid::Uuid;

const PASSWORD: &str = "Clinic-test-123";

struct TestContext {
    app: Router,
    pool: MySqlPool,
    admin_token: String,
    receptionist_token: String,
    receptionist_id: u64,
    doctor_id: u64,
    patient_id: u64,
    service_id: u64,
    cheaper_service_id: u64,
}

struct TestResponse {
    status: StatusCode,
    headers: HeaderMap,
    body: Vec<u8>,
}

impl TestResponse {
    fn json(&self) -> Value {
        serde_json::from_slice(&self.body).unwrap_or_else(|error| {
            panic!(
                "response was not JSON ({error}): {}",
                String::from_utf8_lossy(&self.body)
            )
        })
    }
}

fn database_mutex() -> &'static Mutex<()> {
    static MUTEX: OnceLock<Mutex<()>> = OnceLock::new();
    MUTEX.get_or_init(|| Mutex::new(()))
}

async fn database_guard() -> MutexGuard<'static, ()> {
    database_mutex().lock().await
}

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
        near_end_sessions: 2,
        near_end_days: 7,
    }
}

async fn setup() -> TestContext {
    let pool = test_pool().await;
    schema::reset(&pool).await.expect("reset Rust test schema");
    let password_hash = hash(PASSWORD, 4).expect("hash test password");
    let admin_id = sqlx::query(
        "INSERT INTO users (name, username, password, role, is_active, created_at, updated_at) \
         VALUES ('مدير الاختبار', 'admin', ?, 'admin', 1, NOW(), NOW())",
    )
    .bind(&password_hash)
    .execute(&pool)
    .await
    .expect("insert admin")
    .last_insert_id();
    let receptionist_id = sqlx::query(
        "INSERT INTO users (name, username, password, role, is_active, created_at, updated_at) \
         VALUES ('موظف الاستقبال', 'reception', ?, 'receptionist', 1, NOW(), NOW())",
    )
    .bind(&password_hash)
    .execute(&pool)
    .await
    .expect("insert receptionist")
    .last_insert_id();
    let doctor_id = sqlx::query(
        "INSERT INTO doctors (name, phone, commission_rate, is_active, created_by, created_at, updated_at) \
         VALUES ('د. أحمد', '0910000001', 30, 1, ?, NOW(), NOW())",
    )
    .bind(admin_id)
    .execute(&pool)
    .await
    .expect("insert doctor")
    .last_insert_id();
    let patient_id = sqlx::query(
        "INSERT INTO patients (name, phone, age, gender, is_active, created_by, created_at, updated_at) \
         VALUES ('مريض الاختبار', '0910000002', 40, 'male', 1, ?, NOW(), NOW())",
    )
    .bind(admin_id)
    .execute(&pool)
    .await
    .expect("insert patient")
    .last_insert_id();
    let service_id = insert_service(&pool, admin_id, "علاج يدوي", "80.000").await;
    let cheaper_service_id = insert_service(&pool, admin_id, "جلسة متابعة", "50.000").await;
    let app = router(AppState {
        pool: pool.clone(),
        config: config(),
        login_rate_limiter: Default::default(),
    });
    let admin_token = login(&app, "admin").await;
    let receptionist_token = login(&app, "reception").await;

    TestContext {
        app,
        pool,
        admin_token,
        receptionist_token,
        receptionist_id,
        doctor_id,
        patient_id,
        service_id,
        cheaper_service_id,
    }
}

async fn insert_service(pool: &MySqlPool, admin_id: u64, name: &str, price: &str) -> u64 {
    sqlx::query(
        "INSERT INTO services (name, price, is_active, created_by, created_at, updated_at) \
         VALUES (?, ?, 1, ?, NOW(), NOW())",
    )
    .bind(name)
    .bind(price)
    .bind(admin_id)
    .execute(pool)
    .await
    .expect("insert service")
    .last_insert_id()
}

async fn login(app: &Router, username: &str) -> String {
    let response = request(
        app,
        Method::POST,
        "/api/v1/auth/login",
        None,
        None,
        Some(json!({ "username": username, "password": PASSWORD })),
    )
    .await;
    assert_eq!(response.status, StatusCode::OK, "{}", response.json());
    response.json()["data"]["token"]
        .as_str()
        .expect("login token")
        .to_string()
}

async fn request(
    app: &Router,
    method: Method,
    uri: &str,
    token: Option<&str>,
    idempotency_key: Option<&str>,
    body: Option<Value>,
) -> TestResponse {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(token) = token {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    if let Some(key) = idempotency_key {
        builder = builder.header("Idempotency-Key", key);
    }
    let body = if let Some(body) = body {
        builder = builder.header(header::CONTENT_TYPE, "application/json");
        Body::from(serde_json::to_vec(&body).expect("serialize request"))
    } else {
        Body::empty()
    };
    let response = app
        .clone()
        .oneshot(builder.body(body).expect("build request"))
        .await
        .expect("route response");
    let status = response.status();
    let headers = response.headers().clone();
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read response")
        .to_vec();
    TestResponse {
        status,
        headers,
        body,
    }
}

fn individual_session_payload(context: &TestContext, service_id: u64) -> Value {
    json!({
        "patient_id": context.patient_id,
        "doctor_id": context.doctor_id,
        "type": "individual",
        "service_ids": [service_id],
        "discount": "0.000",
        "settlement": {
            "wallet_amount": "0.000",
            "paid_amount": "80.000",
            "debt_amount": "0.000",
            "method": "cash"
        }
    })
}

async fn create_session(context: &TestContext, payload: Value) -> TestResponse {
    request(
        &context.app,
        Method::POST,
        "/api/v1/sessions",
        Some(&context.receptionist_token),
        Some(&Uuid::new_v4().to_string()),
        Some(payload),
    )
    .await
}

#[tokio::test]
#[ignore = "requires an isolated MySQL 8 database"]
async fn http_auth_dashboard_reports_and_exports_match_laravel_contract() {
    let _guard = database_guard().await;
    let context = setup().await;

    let unauthorized = request(
        &context.app,
        Method::GET,
        "/api/v1/dashboard",
        None,
        None,
        None,
    )
    .await;
    assert_eq!(unauthorized.status, StatusCode::UNAUTHORIZED);
    assert!(unauthorized.json()["message"].is_string());

    for _ in 0..5 {
        let invalid_login = request(
            &context.app,
            Method::POST,
            "/api/v1/auth/login",
            None,
            None,
            Some(json!({
                "username": "rate-limit-test",
                "password": "incorrect"
            })),
        )
        .await;
        assert_eq!(invalid_login.status, StatusCode::UNPROCESSABLE_ENTITY);
    }
    let throttled_login = request(
        &context.app,
        Method::POST,
        "/api/v1/auth/login",
        None,
        None,
        Some(json!({
            "username": "rate-limit-test",
            "password": "incorrect"
        })),
    )
    .await;
    assert_eq!(throttled_login.status, StatusCode::TOO_MANY_REQUESTS);

    let forbidden = request(
        &context.app,
        Method::GET,
        "/api/v1/reports/sessions",
        Some(&context.receptionist_token),
        None,
        None,
    )
    .await;
    assert_eq!(forbidden.status, StatusCode::FORBIDDEN);

    let created = create_session(
        &context,
        individual_session_payload(&context, context.service_id),
    )
    .await;
    assert_eq!(created.status, StatusCode::CREATED, "{}", created.json());
    let created_json = created.json();
    assert_eq!(created_json["data"]["session"]["status"], "closed");
    assert_eq!(created_json["data"]["session"]["net_total"], "80.000");
    assert_eq!(
        created_json["data"]["effects"]["payment"]["amount"],
        "80.000"
    );
    assert!(created_json["data"]["effects"]["payment"]
        .get("patient")
        .is_none());
    assert!(created_json["data"]["effects"]["debt"].is_null());

    let today = super::time::today();
    let subscription = request(
        &context.app,
        Method::POST,
        "/api/v1/subscriptions",
        Some(&context.receptionist_token),
        Some(&Uuid::new_v4().to_string()),
        Some(json!({
            "patient_id": context.patient_id,
            "plan_type": "monthly",
            "amount": "160.000",
            "paid_amount": "80.000",
            "method": "cash",
            "start_date": today.format("%Y-%m-%d").to_string(),
            "end_date": (today + chrono::Duration::days(5)).format("%Y-%m-%d").to_string()
        })),
    )
    .await;
    assert_eq!(
        subscription.status,
        StatusCode::CREATED,
        "{}",
        subscription.json()
    );
    let subscription_json = subscription.json();
    let subscription_id = subscription_json["data"]["subscription"]["id"]
        .as_u64()
        .unwrap();
    assert!(subscription_json["data"]["subscription"]
        .get("sponsor")
        .is_some());

    let sessions_index = request(
        &context.app,
        Method::GET,
        "/api/v1/sessions",
        Some(&context.admin_token),
        None,
        None,
    )
    .await;
    assert_eq!(sessions_index.status, StatusCode::OK);
    let sessions_index_json = sessions_index.json();
    let listed_session = &sessions_index_json["data"][0];
    assert!(listed_session.get("patient").is_some());
    assert!(listed_session.get("doctor").is_some());
    assert!(listed_session.get("services").is_some());
    for unloaded in [
        "created_by_user",
        "closed_by_user",
        "cancelled_by_user",
        "additions",
    ] {
        assert!(listed_session.get(unloaded).is_none(), "{unloaded}");
    }

    let sessions_with_empty_filters = request(
        &context.app,
        Method::GET,
        "/api/v1/sessions?search=&doctor_id=&status=&date=&page=1&per_page=15",
        Some(&context.admin_token),
        None,
        None,
    )
    .await;
    assert_eq!(
        sessions_with_empty_filters.status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&sessions_with_empty_filters.body)
    );

    let audit_with_empty_filters = request(
        &context.app,
        Method::GET,
        "/api/v1/audit-log?user_id=&action=&entity_type=&date=&page=1",
        Some(&context.admin_token),
        None,
        None,
    )
    .await;
    assert_eq!(
        audit_with_empty_filters.status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&audit_with_empty_filters.body)
    );
    assert!(audit_with_empty_filters.json()["data"].is_array());

    let session_id = created_json["data"]["session"]["id"].as_u64().unwrap();
    let session_show = request(
        &context.app,
        Method::GET,
        &format!("/api/v1/sessions/{session_id}"),
        Some(&context.admin_token),
        None,
        None,
    )
    .await;
    assert_eq!(session_show.status, StatusCode::OK);
    let session_show_json = session_show.json();
    let detailed_session = &session_show_json["data"];
    for loaded in [
        "patient",
        "doctor",
        "services",
        "created_by_user",
        "closed_by_user",
        "cancelled_by_user",
        "additions",
    ] {
        assert!(detailed_session.get(loaded).is_some(), "{loaded}");
    }

    let patient_show = request(
        &context.app,
        Method::GET,
        &format!("/api/v1/patients/{}", context.patient_id),
        Some(&context.admin_token),
        None,
        None,
    )
    .await;
    assert_eq!(patient_show.status, StatusCode::OK);
    let patient_show_json = patient_show.json();
    let patient_session = &patient_show_json["data"]["sessions"][0];
    assert!(patient_session.get("patient").is_none());
    assert!(patient_session.get("doctor").is_some());
    assert!(patient_session.get("services").is_some());
    assert!(patient_session.get("additions").is_none());
    let patient_subscription = &patient_show_json["data"]["subscriptions"][0];
    assert!(patient_subscription.get("patient").is_none());
    assert!(patient_subscription.get("sponsor").is_none());
    assert!(patient_subscription.get("services").is_some());

    let subscriptions_index = request(
        &context.app,
        Method::GET,
        "/api/v1/subscriptions",
        Some(&context.admin_token),
        None,
        None,
    )
    .await;
    assert_eq!(subscriptions_index.status, StatusCode::OK);
    let subscriptions_index_json = subscriptions_index.json();
    let listed_subscription = &subscriptions_index_json["data"][0];
    assert!(listed_subscription.get("patient").is_some());
    assert!(listed_subscription.get("sponsor").is_some());
    assert!(listed_subscription.get("services").is_some());

    let receptionist_dashboard = request(
        &context.app,
        Method::GET,
        "/api/v1/dashboard",
        Some(&context.receptionist_token),
        None,
        None,
    )
    .await;
    assert_eq!(receptionist_dashboard.status, StatusCode::OK);
    assert_eq!(
        receptionist_dashboard.json()["data"]["stats"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    assert!(receptionist_dashboard.json()["data"]
        .get("financial_summary")
        .is_none());

    let admin_dashboard = request(
        &context.app,
        Method::GET,
        "/api/v1/dashboard",
        Some(&context.admin_token),
        None,
        None,
    )
    .await;
    assert_eq!(
        admin_dashboard.status,
        StatusCode::OK,
        "{}",
        admin_dashboard.json()
    );
    let dashboard = admin_dashboard.json();
    assert_eq!(dashboard["data"]["stats"].as_array().unwrap().len(), 7);
    assert_eq!(
        dashboard["data"]["recent_movements"][0]["patient"]["name"],
        "مريض الاختبار"
    );
    assert!(dashboard["data"]["financial_summary"].is_object());
    let dashboard_session = &dashboard["data"]["today_sessions"][0];
    assert!(dashboard_session.get("patient").is_some());
    assert!(dashboard_session.get("created_by_user").is_none());
    assert!(dashboard_session.get("additions").is_none());
    let dashboard_subscription = &dashboard["data"]["near_subscriptions"][0];
    assert!(dashboard_subscription.get("patient").is_some());
    assert!(dashboard_subscription.get("services").is_some());
    assert!(dashboard_subscription.get("sponsor").is_none());

    let stopped = request(
        &context.app,
        Method::POST,
        &format!("/api/v1/subscriptions/{subscription_id}/stop"),
        Some(&context.admin_token),
        Some(&Uuid::new_v4().to_string()),
        Some(json!({ "reason": "اختبار تطابق العقد" })),
    )
    .await;
    assert_eq!(stopped.status, StatusCode::OK, "{}", stopped.json());
    assert!(stopped.json()["data"].get("sponsor").is_some());
    let reactivated = request(
        &context.app,
        Method::POST,
        &format!("/api/v1/subscriptions/{subscription_id}/reactivate"),
        Some(&context.admin_token),
        Some(&Uuid::new_v4().to_string()),
        Some(json!({ "reason": "اختبار تطابق العقد" })),
    )
    .await;
    assert_eq!(reactivated.status, StatusCode::OK, "{}", reactivated.json());
    assert!(reactivated.json()["data"].get("sponsor").is_none());

    let report_expectations = [
        ("sessions", "service_value"),
        ("collections", "net"),
        ("debts", "remaining"),
        ("doctor-dues", "earned"),
        ("subscriptions", "net_total"),
    ];
    for (report_type, total_key) in report_expectations {
        let response = request(
            &context.app,
            Method::GET,
            &format!("/api/v1/reports/{report_type}"),
            Some(&context.admin_token),
            None,
            None,
        )
        .await;
        assert_eq!(response.status, StatusCode::OK, "{}", response.json());
        let report = response.json();
        assert!(report["data"]["rows"].is_array());
        assert!(report["data"]["totals"][total_key].is_string());
        assert_eq!(report["data"]["meta"]["type"], report_type);

        for (format, content_type, signature) in [
            ("pdf", "application/pdf", b"%PDF-".as_slice()),
            (
                "xlsx",
                "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
                b"PK".as_slice(),
            ),
        ] {
            let export = request(
                &context.app,
                Method::GET,
                &format!("/api/v1/reports/{report_type}/export?format={format}"),
                Some(&context.admin_token),
                None,
                None,
            )
            .await;
            assert_eq!(export.status, StatusCode::OK);
            assert_eq!(
                export.headers.get(header::CONTENT_TYPE).unwrap(),
                content_type
            );
            assert!(export
                .headers
                .get(header::CONTENT_DISPOSITION)
                .unwrap()
                .to_str()
                .unwrap()
                .contains("attachment"));
            assert!(export.body.starts_with(signature));
        }
    }

    let filtered = request(
        &context.app,
        Method::GET,
        &format!(
            "/api/v1/reports/sessions?patient_id={}&doctor_id={}&status=closed",
            context.patient_id, context.doctor_id
        ),
        Some(&context.admin_token),
        None,
        None,
    )
    .await;
    assert_eq!(filtered.status, StatusCode::OK);
    assert_eq!(filtered.json()["data"]["rows"].as_array().unwrap().len(), 1);

    let empty_filter = request(
        &context.app,
        Method::GET,
        "/api/v1/reports/sessions?status=open",
        Some(&context.admin_token),
        None,
        None,
    )
    .await;
    assert_eq!(empty_filter.status, StatusCode::OK);
    assert!(empty_filter.json()["data"]["rows"]
        .as_array()
        .unwrap()
        .is_empty());

    let invalid_period = request(
        &context.app,
        Method::GET,
        "/api/v1/reports/sessions?from=2026-11-30&to=2026-11-01",
        Some(&context.admin_token),
        None,
        None,
    )
    .await;
    assert_eq!(invalid_period.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(invalid_period.json()["errors"]["from"].is_array());
}

#[tokio::test]
#[ignore = "requires an isolated MySQL 8 database"]
async fn http_idempotency_replays_and_rejects_conflicting_payloads() {
    let _guard = database_guard().await;
    let context = setup().await;
    let key = Uuid::new_v4().to_string();
    let first = request(
        &context.app,
        Method::POST,
        &format!("/api/v1/patients/{}/wallet/topups", context.patient_id),
        Some(&context.receptionist_token),
        Some(&key),
        Some(json!({ "amount": "100.000", "method": "cash" })),
    )
    .await;
    assert_eq!(first.status, StatusCode::CREATED, "{}", first.json());
    let replay = request(
        &context.app,
        Method::POST,
        &format!("/api/v1/patients/{}/wallet/topups", context.patient_id),
        Some(&context.receptionist_token),
        Some(&key),
        Some(json!({ "method": "cash", "amount": "100.000" })),
    )
    .await;
    assert_eq!(replay.status, StatusCode::CREATED);
    assert_eq!(replay.body, first.body);

    let conflict = request(
        &context.app,
        Method::POST,
        &format!("/api/v1/patients/{}/wallet/topups", context.patient_id),
        Some(&context.receptionist_token),
        Some(&key),
        Some(json!({ "amount": "101.000", "method": "cash" })),
    )
    .await;
    assert_eq!(conflict.status, StatusCode::CONFLICT);
    let payment_count = sqlx::query_scalar::<_, u64>(
        "SELECT CAST(COUNT(*) AS UNSIGNED) FROM payments WHERE patient_id = ? AND type = 'wallet_topup'",
    )
    .bind(context.patient_id)
    .fetch_one(&context.pool)
    .await
    .unwrap();
    let movement_count = sqlx::query_scalar::<_, u64>(
        "SELECT CAST(COUNT(*) AS UNSIGNED) FROM wallet_transactions WHERE patient_id = ? AND type = 'topup'",
    )
    .bind(context.patient_id)
    .fetch_one(&context.pool)
    .await
    .unwrap();
    assert_eq!((payment_count, movement_count), (1, 1));
}

#[tokio::test]
#[ignore = "requires an isolated MySQL 8 database"]
async fn concurrent_close_is_single_effect_and_insufficient_wallet_rolls_back() {
    let _guard = database_guard().await;
    let context = setup().await;
    let open_id = insert_open_session(&context, "wallet", "80.000", "0.000", "0.000", None).await;
    let first_key = Uuid::new_v4().to_string();
    let second_key = Uuid::new_v4().to_string();
    let uri = format!("/api/v1/sessions/{open_id}/close");
    let first = request(
        &context.app,
        Method::POST,
        &uri,
        Some(&context.receptionist_token),
        Some(&first_key),
        Some(json!({})),
    );
    let second = request(
        &context.app,
        Method::POST,
        &uri,
        Some(&context.receptionist_token),
        Some(&second_key),
        Some(json!({})),
    );
    let (first, second) = tokio::join!(first, second);
    assert_eq!(first.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(second.status, StatusCode::UNPROCESSABLE_ENTITY);
    let status = sqlx::query_scalar::<_, String>("SELECT status FROM sessions WHERE id = ?")
        .bind(open_id)
        .fetch_one(&context.pool)
        .await
        .unwrap();
    assert_eq!(status, "open");
    assert_eq!(
        sqlx::query_scalar::<_, u64>(
            "SELECT CAST(COUNT(*) AS UNSIGNED) FROM wallet_transactions WHERE session_id = ?",
        )
        .bind(open_id)
        .fetch_one(&context.pool)
        .await
        .unwrap(),
        0
    );

    request(
        &context.app,
        Method::POST,
        &format!("/api/v1/patients/{}/wallet/topups", context.patient_id),
        Some(&context.receptionist_token),
        Some(&Uuid::new_v4().to_string()),
        Some(json!({ "amount": "100.000", "method": "cash" })),
    )
    .await;
    let third_key = Uuid::new_v4().to_string();
    let fourth_key = Uuid::new_v4().to_string();
    let third = request(
        &context.app,
        Method::POST,
        &uri,
        Some(&context.receptionist_token),
        Some(&third_key),
        Some(json!({})),
    );
    let fourth = request(
        &context.app,
        Method::POST,
        &uri,
        Some(&context.receptionist_token),
        Some(&fourth_key),
        Some(json!({})),
    );
    let (third, fourth) = tokio::join!(third, fourth);
    assert_eq!(third.status, StatusCode::OK, "{}", third.json());
    assert_eq!(fourth.status, StatusCode::OK, "{}", fourth.json());
    let replayed = [
        third.json()["data"]["replayed"].as_bool().unwrap(),
        fourth.json()["data"]["replayed"].as_bool().unwrap(),
    ];
    assert_eq!(replayed.iter().filter(|value| **value).count(), 1);
    assert_eq!(
        sqlx::query_scalar::<_, u64>(
            "SELECT CAST(COUNT(*) AS UNSIGNED) FROM wallet_transactions WHERE session_id = ? AND type = 'debit'",
        )
        .bind(open_id)
        .fetch_one(&context.pool)
        .await
        .unwrap(),
        1
    );
}

#[tokio::test]
#[ignore = "requires an isolated MySQL 8 database"]
async fn correction_and_paid_debt_cancellation_use_reversing_records() {
    let _guard = database_guard().await;
    let context = setup().await;

    let paid = create_session(
        &context,
        individual_session_payload(&context, context.service_id),
    )
    .await;
    assert_eq!(paid.status, StatusCode::CREATED, "{}", paid.json());
    let paid_session_id = paid.json()["data"]["session"]["id"].as_u64().unwrap();
    let correction = request(
        &context.app,
        Method::POST,
        &format!("/api/v1/sessions/{paid_session_id}/correct"),
        Some(&context.admin_token),
        Some(&Uuid::new_v4().to_string()),
        Some(json!({
            "reason": "تصحيح الخدمة المسجلة",
            "changes": {
                "service_ids": [context.cheaper_service_id],
                "discount": "0.000",
                "settlement": {
                    "wallet_amount": "0.000",
                    "paid_amount": "50.000",
                    "debt_amount": "0.000",
                    "method": "cash"
                }
            }
        })),
    )
    .await;
    assert_eq!(correction.status, StatusCode::OK, "{}", correction.json());
    assert_eq!(correction.json()["data"]["session"]["net_total"], "50.000");
    assert_eq!(
        correction.json()["data"]["financial_impact"]["net_delta"],
        "-30.000"
    );
    let payment_sum = sqlx::query_scalar::<_, Option<Decimal>>(
        "SELECT SUM(amount) FROM payments WHERE session_id = ?",
    )
    .bind(paid_session_id)
    .fetch_one(&context.pool)
    .await
    .unwrap()
    .unwrap_or(Decimal::ZERO);
    assert_eq!(payment_sum, Decimal::from(50));

    let debt_payload = json!({
        "patient_id": context.patient_id,
        "doctor_id": context.doctor_id,
        "type": "individual",
        "service_ids": [context.service_id],
        "discount": "0.000",
        "settlement": {
            "wallet_amount": "0.000",
            "paid_amount": "0.000",
            "debt_amount": "80.000",
            "method": null
        }
    });
    let debt_session = create_session(&context, debt_payload).await;
    assert_eq!(
        debt_session.status,
        StatusCode::CREATED,
        "{}",
        debt_session.json()
    );
    let debt_session_id = debt_session.json()["data"]["session"]["id"]
        .as_u64()
        .unwrap();
    let debt_id = debt_session.json()["data"]["effects"]["debt"]["id"]
        .as_u64()
        .unwrap();
    let settlement = request(
        &context.app,
        Method::POST,
        "/api/v1/debts/settlements",
        Some(&context.receptionist_token),
        Some(&Uuid::new_v4().to_string()),
        Some(json!({
            "amount": "80.000",
            "method": "cash",
            "allocations": [{ "debt_id": debt_id, "amount": "80.000" }]
        })),
    )
    .await;
    assert_eq!(
        settlement.status,
        StatusCode::CREATED,
        "{}",
        settlement.json()
    );

    let receptionist_cancel = request(
        &context.app,
        Method::POST,
        &format!("/api/v1/sessions/{debt_session_id}/cancel"),
        Some(&context.receptionist_token),
        Some(&Uuid::new_v4().to_string()),
        Some(json!({ "reason": "اختبار رفض الصلاحية" })),
    )
    .await;
    assert_eq!(receptionist_cancel.status, StatusCode::FORBIDDEN);

    let cancellation = request(
        &context.app,
        Method::POST,
        &format!("/api/v1/sessions/{debt_session_id}/cancel"),
        Some(&context.admin_token),
        Some(&Uuid::new_v4().to_string()),
        Some(json!({ "reason": "إلغاء بعد تسديد الدين" })),
    )
    .await;
    assert_eq!(
        cancellation.status,
        StatusCode::OK,
        "{}",
        cancellation.json()
    );
    assert_eq!(
        cancellation.json()["data"]["session"]["status"],
        "cancelled"
    );
    assert!(cancellation.json()["data"]["session"]["commission_amount"].is_null());
    assert_eq!(
        cancellation.json()["data"]["reversals"]["refunds"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let debt_status = sqlx::query_scalar::<_, String>("SELECT status FROM debts WHERE id = ?")
        .bind(debt_id)
        .fetch_one(&context.pool)
        .await
        .unwrap();
    assert_eq!(debt_status, "void");
    let refund_total = sqlx::query_scalar::<_, Option<Decimal>>(
        "SELECT SUM(ABS(amount)) FROM payments WHERE type = 'refund' AND patient_id = ?",
    )
    .bind(context.patient_id)
    .fetch_one(&context.pool)
    .await
    .unwrap()
    .unwrap_or(Decimal::ZERO);
    assert_eq!(refund_total, Decimal::from(160));
}

async fn insert_open_session(
    context: &TestContext,
    coverage: &str,
    wallet_amount: &str,
    paid_amount: &str,
    debt_amount: &str,
    payment_method: Option<&str>,
) -> u64 {
    let result = sqlx::query(
        "INSERT INTO sessions \
         (code, patient_id, doctor_id, subscription_id, type, status, opened_at, created_by, \
          services_total, discount, net_total, coverage, wallet_amount, paid_amount, debt_amount, payment_method, created_at, updated_at) \
         VALUES (?, ?, ?, NULL, 'individual', 'open', NOW(), ?, 80, 0, 80, ?, ?, ?, ?, ?, NOW(), NOW())",
    )
    .bind(format!("S-TEST-{}", Uuid::new_v4()))
    .bind(context.patient_id)
    .bind(context.doctor_id)
    .bind(context.receptionist_id)
    .bind(coverage)
    .bind(wallet_amount)
    .bind(paid_amount)
    .bind(debt_amount)
    .bind(payment_method)
    .execute(&context.pool)
    .await
    .expect("insert open session");
    let id = result.last_insert_id();
    sqlx::query(
        "INSERT INTO session_services \
         (session_id, service_id, service_name, price, base_price, included_units, extra_units, item_type, is_addition, created_by, created_at, updated_at) \
         VALUES (?, ?, 'علاج يدوي', 80, 80, 0, 0, 'service', 0, ?, NOW(), NOW())",
    )
    .bind(id)
    .bind(context.service_id)
    .bind(context.receptionist_id)
    .execute(&context.pool)
    .await
    .expect("insert session snapshot");
    id
}

#[test]
fn sanctum_hash_format_remains_compatible() {
    let plain = "plain-token";
    let digest = hex::encode(Sha256::digest(plain.as_bytes()));
    assert_eq!(digest.len(), 64);
    assert_eq!(
        digest,
        "23fb79e20d37abf2418d78115eb0cc8c74b52f4ed8b91dda7fc03a1d41fc15e3"
    );
}
