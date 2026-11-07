use super::config::ApiConfig;
use sqlx::{
    mysql::{MySqlConnectOptions, MySqlPoolOptions, MySqlSslMode},
    Executor, MySqlPool,
};
use std::time::Duration;

pub async fn connect(config: &ApiConfig) -> Result<MySqlPool, String> {
    let options = MySqlConnectOptions::new()
        .host(&config.mysql_host)
        .port(config.mysql_port)
        .database(&config.database)
        .username(&config.username)
        .password(&config.password)
        .charset("utf8mb4")
        .collation("utf8mb4_unicode_ci")
        .ssl_mode(MySqlSslMode::Disabled);

    MySqlPoolOptions::new()
        .max_connections(12)
        .min_connections(1)
        .acquire_timeout(Duration::from_secs(10))
        .after_connect(|connection, _| {
            Box::pin(async move {
                connection
                    .execute("SET time_zone = '+02:00', sql_mode = 'STRICT_TRANS_TABLES,ERROR_FOR_DIVISION_BY_ZERO,NO_ENGINE_SUBSTITUTION'")
                    .await?;
                Ok(())
            })
        })
        .connect_with(options)
        .await
        .map_err(|error| format!("تعذر الاتصال بقاعدة بيانات MySQL المحلية: {error}"))
}

#[cfg(test)]
pub async fn connect_database(
    host: &str,
    port: u16,
    database: &str,
    username: &str,
    password: &str,
) -> Result<MySqlPool, String> {
    connect(&ApiConfig {
        host: "127.0.0.1".to_string(),
        port: 0,
        mysql_host: host.to_string(),
        mysql_port: port,
        database: database.to_string(),
        username: username.to_string(),
        password: password.to_string(),
        commission_base: Default::default(),
        subscription_follow_up_sessions: 2,
    })
    .await
}
