#[derive(Debug, Clone)]
pub struct ApiConfig {
    pub host: String,
    pub port: u16,
    pub mysql_host: String,
    pub mysql_port: u16,
    pub database: String,
    pub username: String,
    pub password: String,
    pub commission_base: CommissionBase,
    pub near_end_sessions: u32,
    pub near_end_days: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommissionBase {
    Net,
    Gross,
}

impl CommissionBase {
    pub fn from_env() -> Self {
        match std::env::var("CLINIC_COMMISSION_BASE").as_deref() {
            Ok("gross") => Self::Gross,
            _ => Self::Net,
        }
    }
}

impl Default for CommissionBase {
    fn default() -> Self {
        Self::Net
    }
}
