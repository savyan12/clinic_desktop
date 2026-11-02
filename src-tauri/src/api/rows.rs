use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde_json::Value;
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow)]
pub struct UserRow {
    pub id: u64,
    pub name: String,
    pub username: String,
    pub password: String,
    pub role: String,
    pub is_active: bool,
    pub last_login_at: Option<DateTime<Utc>>,
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, FromRow)]
pub struct PatientRow {
    pub id: u64,
    pub name: String,
    pub phone: String,
    pub age: Option<u16>,
    pub gender: Option<String>,
    pub area: Option<String>,
    pub emergency_phone: Option<String>,
    pub sponsor_id: Option<u64>,
    pub is_active: bool,
    pub created_by: u64,
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, FromRow)]
pub struct SponsorRow {
    pub id: u64,
    pub name: String,
    pub phone: Option<String>,
    pub address: Option<String>,
    pub is_active: bool,
    pub created_by: u64,
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, FromRow)]
pub struct DoctorRow {
    pub id: u64,
    pub name: String,
    pub phone: Option<String>,
    pub commission_rate: Decimal,
    pub is_active: bool,
    pub created_by: u64,
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, FromRow)]
pub struct ServiceRow {
    pub id: u64,
    pub name: String,
    pub price: Decimal,
    pub included_units: Option<u16>,
    pub unit_name: Option<String>,
    pub extra_unit_price: Option<Decimal>,
    pub is_active: bool,
    pub created_by: u64,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, FromRow)]
pub struct SubscriptionRow {
    pub id: u64,
    pub patient_id: u64,
    pub sponsor_id: Option<u64>,
    pub plan_type: String,
    pub sessions_count: u32,
    pub sessions_used: u32,
    pub base_total: Decimal,
    pub discount: Decimal,
    pub net_total: Decimal,
    pub paid_amount: Decimal,
    pub start_date: NaiveDate,
    pub expected_end_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub status: String,
    pub cancelled_at: Option<DateTime<Utc>>,
    pub cancelled_by: Option<u64>,
    pub cancel_reason: Option<String>,
    pub refund_amount: Decimal,
    pub created_by: u64,
}

#[derive(Debug, Clone, FromRow)]
pub struct SubscriptionServiceRow {
    pub service_id: u64,
    pub service_name: String,
    pub price: Decimal,
}

#[derive(Debug, Clone, FromRow)]
pub struct SessionRow {
    pub id: u64,
    pub code: String,
    pub patient_id: u64,
    pub doctor_id: u64,
    pub subscription_id: Option<u64>,
    pub r#type: String,
    pub status: String,
    pub opened_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
    pub cancelled_at: Option<DateTime<Utc>>,
    pub created_by: u64,
    pub closed_by: Option<u64>,
    pub cancelled_by: Option<u64>,
    pub cancel_reason: Option<String>,
    pub services_total: Decimal,
    pub discount: Decimal,
    pub net_total: Decimal,
    pub coverage: String,
    pub wallet_amount: Decimal,
    pub paid_amount: Decimal,
    pub debt_amount: Decimal,
    pub payment_method: Option<String>,
    pub sponsor_id: Option<u64>,
    pub commission_rate: Option<Decimal>,
    pub commission_base_amount: Option<Decimal>,
    pub commission_amount: Option<Decimal>,
}

#[derive(Debug, Clone, FromRow)]
pub struct SessionServiceRow {
    pub id: u64,
    pub session_addition_id: Option<u64>,
    pub service_id: u64,
    pub service_name: String,
    pub price: Decimal,
    pub base_price: Option<Decimal>,
    pub included_units: u16,
    pub extra_units: u16,
    pub unit_name: Option<String>,
    pub extra_unit_price: Option<Decimal>,
    pub item_type: String,
    pub is_addition: bool,
    pub added_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, FromRow)]
pub struct SessionAdditionRow {
    pub id: u64,
    pub services_total: Decimal,
    pub net_total: Decimal,
    pub coverage: String,
    pub wallet_amount: Decimal,
    pub paid_amount: Decimal,
    pub debt_amount: Decimal,
    pub payment_method: Option<String>,
    pub sponsor_id: Option<u64>,
    pub created_by: u64,
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, FromRow)]
pub struct SubscriptionUsageRow {
    pub id: u64,
    pub subscription_id: u64,
    pub session_id: u64,
    pub r#type: String,
    pub reverses_usage_id: Option<u64>,
    pub created_by: u64,
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, FromRow)]
pub struct PaymentRow {
    pub id: u64,
    pub patient_id: u64,
    pub session_id: Option<u64>,
    pub session_addition_id: Option<u64>,
    pub subscription_id: Option<u64>,
    pub r#type: String,
    pub method: String,
    pub amount: Decimal,
    pub reverses_payment_id: Option<u64>,
    pub note: Option<String>,
    pub created_by: u64,
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, FromRow)]
pub struct WalletTransactionRow {
    pub id: u64,
    pub patient_id: u64,
    pub session_id: Option<u64>,
    pub session_addition_id: Option<u64>,
    pub payment_id: Option<u64>,
    pub r#type: String,
    pub amount: Decimal,
    pub balance_after: Decimal,
    pub reverses_transaction_id: Option<u64>,
    pub note: Option<String>,
    pub created_by: u64,
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, FromRow)]
pub struct DebtRow {
    pub id: u64,
    pub patient_id: u64,
    pub sponsor_id: Option<u64>,
    pub session_id: Option<u64>,
    pub session_addition_id: Option<u64>,
    pub subscription_id: Option<u64>,
    pub kind: String,
    pub amount: Decimal,
    pub paid_amount: Decimal,
    pub status: String,
    pub voided_at: Option<DateTime<Utc>>,
    pub void_reason: Option<String>,
    pub note: Option<String>,
    pub created_by: u64,
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, FromRow)]
pub struct PaymentAllocationRow {
    pub id: u64,
    pub payment_id: u64,
    pub debt_id: u64,
    pub amount: Decimal,
    pub created_by: u64,
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, FromRow)]
pub struct DoctorPayoutRow {
    pub id: u64,
    pub doctor_id: u64,
    pub amount: Decimal,
    pub date: NaiveDate,
    pub method: String,
    pub note: Option<String>,
    pub created_by: u64,
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, FromRow)]
pub struct AuditLogRow {
    pub id: u64,
    pub user_id: u64,
    pub action: String,
    pub entity_type: String,
    pub entity_id: u64,
    pub old_values: Option<Value>,
    pub new_values: Option<Value>,
    pub reason: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct SettingRow {
    pub id: u64,
    pub clinic_name: String,
    pub clinic_subtitle: Option<String>,
    pub phone: Option<String>,
    pub address: Option<String>,
    pub receipt_footer: Option<String>,
    pub currency: String,
    pub print_size: String,
    pub show_logo: bool,
    pub body_font_size: u8,
    pub body_font_weight: u16,
    pub table_font_size: u8,
    pub table_font_weight: u16,
    pub heading_font_size: u8,
    pub heading_font_weight: u16,
    pub section_font_size: u8,
    pub section_font_weight: u16,
    pub updated_by: Option<u64>,
}
