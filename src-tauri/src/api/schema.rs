use sqlx::{Executor, MySqlPool, Row};
use std::collections::HashSet;

const MIGRATIONS: &[&str] = &[
    "0001_01_01_000000_create_users_table",
    "2026_10_29_005359_create_personal_access_tokens_table",
    "2026_10_29_014622_create_sponsors_table",
    "2026_10_29_014623_create_doctors_table",
    "2026_10_29_014624_create_services_table",
    "2026_10_29_014625_create_patients_table",
    "2026_10_29_014626_create_subscriptions_table",
    "2026_10_29_014627_create_subscription_services_table",
    "2026_10_29_014628_create_sessions_table",
    "2026_10_29_014629_create_session_services_table",
    "2026_10_29_014630_create_subscription_usages_table",
    "2026_10_29_014631_create_payments_table",
    "2026_10_29_014632_create_wallet_transactions_table",
    "2026_10_29_014633_create_debts_table",
    "2026_10_29_014634_create_payment_allocations_table",
    "2026_10_29_014635_create_doctor_payouts_table",
    "2026_10_29_014636_create_audit_logs_table",
    "2026_10_29_014637_create_idempotency_keys_table",
    "2026_10_29_014638_create_settings_table",
    "2026_10_30_120000_add_cancellation_fields_to_subscriptions_table",
    "2026_11_01_173532_add_monthly_subscription_and_service_addon_fields",
    "2026_11_01_173533_add_session_addition_support",
    "2026_11_01_173534_make_patient_demographics_optional",
    "2026_11_01_173535_add_typography_settings",
];

pub async fn reset(pool: &MySqlPool) -> Result<(), String> {
    pool.execute("SET FOREIGN_KEY_CHECKS = 0")
        .await
        .map_err(migration_error)?;

    for table in [
        "payment_allocations",
        "wallet_transactions",
        "payments",
        "debts",
        "subscription_usages",
        "session_services",
        "session_additions",
        "sessions",
        "subscription_services",
        "subscriptions",
        "doctor_payouts",
        "audit_logs",
        "idempotency_keys",
        "personal_access_tokens",
        "patients",
        "services",
        "doctors",
        "sponsors",
        "settings",
        "users",
        "migrations",
    ] {
        pool.execute(format!("DROP TABLE IF EXISTS `{table}`").as_str())
            .await
            .map_err(migration_error)?;
    }

    pool.execute("SET FOREIGN_KEY_CHECKS = 1")
        .await
        .map_err(migration_error)?;
    migrate(pool).await
}

pub async fn migrate(pool: &MySqlPool) -> Result<(), String> {
    pool.execute(
        "CREATE TABLE IF NOT EXISTS migrations (\
            id INT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY, \
            migration VARCHAR(255) NOT NULL, \
            batch INT NOT NULL\
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci",
    )
    .await
    .map_err(migration_error)?;

    let rows = sqlx::query("SELECT migration FROM migrations")
        .fetch_all(pool)
        .await
        .map_err(migration_error)?;
    let applied = rows
        .into_iter()
        .filter_map(|row| row.try_get::<String, _>("migration").ok())
        .collect::<HashSet<_>>();
    let batch = sqlx::query_scalar::<_, Option<i32>>("SELECT MAX(batch) FROM migrations")
        .fetch_one(pool)
        .await
        .map_err(migration_error)?
        .unwrap_or(0)
        + 1;

    for migration in MIGRATIONS {
        if applied.contains(*migration) {
            continue;
        }

        apply(pool, migration).await?;
        sqlx::query("INSERT INTO migrations (migration, batch) VALUES (?, ?)")
            .bind(migration)
            .bind(batch)
            .execute(pool)
            .await
            .map_err(migration_error)?;
    }

    validate(pool).await
}

pub async fn validate(pool: &MySqlPool) -> Result<(), String> {
    for table in [
        "migrations",
        "users",
        "personal_access_tokens",
        "patients",
        "sponsors",
        "doctors",
        "services",
        "sessions",
        "session_services",
        "session_additions",
        "subscriptions",
        "subscription_services",
        "subscription_usages",
        "wallet_transactions",
        "payments",
        "debts",
        "payment_allocations",
        "doctor_payouts",
        "audit_logs",
        "idempotency_keys",
        "settings",
    ] {
        if !table_exists(pool, table).await? {
            return Err(format!("قاعدة البيانات تفتقد جدول النظام `{table}`."));
        }
    }

    for (table, column) in [
        ("services", "extra_unit_price"),
        ("subscriptions", "plan_type"),
        ("subscriptions", "cancelled_at"),
        ("sessions", "commission_amount"),
        ("session_services", "session_addition_id"),
        ("payments", "session_addition_id"),
        ("debts", "session_addition_id"),
        ("wallet_transactions", "session_addition_id"),
        ("settings", "section_font_weight"),
    ] {
        if !column_exists(pool, table, column).await? {
            return Err(format!(
                "قاعدة البيانات تفتقد الحقل المطلوب `{table}.{column}`."
            ));
        }
    }

    Ok(())
}

async fn apply(pool: &MySqlPool, migration: &str) -> Result<(), String> {
    match migration {
        "0001_01_01_000000_create_users_table" => execute_all(pool, &[USERS]).await,
        "2026_10_29_005359_create_personal_access_tokens_table" => {
            execute_all(pool, &[PERSONAL_ACCESS_TOKENS]).await
        }
        "2026_10_29_014622_create_sponsors_table" => execute_all(pool, &[SPONSORS]).await,
        "2026_10_29_014623_create_doctors_table" => execute_all(pool, &[DOCTORS]).await,
        "2026_10_29_014624_create_services_table" => execute_all(pool, &[SERVICES]).await,
        "2026_10_29_014625_create_patients_table" => execute_all(pool, &[PATIENTS]).await,
        "2026_10_29_014626_create_subscriptions_table" => execute_all(pool, &[SUBSCRIPTIONS]).await,
        "2026_10_29_014627_create_subscription_services_table" => {
            execute_all(pool, &[SUBSCRIPTION_SERVICES]).await
        }
        "2026_10_29_014628_create_sessions_table" => execute_all(pool, &[SESSIONS]).await,
        "2026_10_29_014629_create_session_services_table" => {
            execute_all(pool, &[SESSION_SERVICES]).await
        }
        "2026_10_29_014630_create_subscription_usages_table" => {
            execute_all(pool, &[SUBSCRIPTION_USAGES]).await
        }
        "2026_10_29_014631_create_payments_table" => execute_all(pool, &[PAYMENTS]).await,
        "2026_10_29_014632_create_wallet_transactions_table" => {
            execute_all(pool, &[WALLET_TRANSACTIONS]).await
        }
        "2026_10_29_014633_create_debts_table" => execute_all(pool, &[DEBTS]).await,
        "2026_10_29_014634_create_payment_allocations_table" => {
            execute_all(pool, &[PAYMENT_ALLOCATIONS]).await
        }
        "2026_10_29_014635_create_doctor_payouts_table" => {
            execute_all(pool, &[DOCTOR_PAYOUTS]).await
        }
        "2026_10_29_014636_create_audit_logs_table" => execute_all(pool, &[AUDIT_LOGS]).await,
        "2026_10_29_014637_create_idempotency_keys_table" => {
            execute_all(pool, &[IDEMPOTENCY_KEYS]).await
        }
        "2026_10_29_014638_create_settings_table" => execute_all(pool, &[SETTINGS]).await,
        "2026_10_30_120000_add_cancellation_fields_to_subscriptions_table" => {
            add_subscription_cancellation(pool).await
        }
        "2026_11_01_173532_add_monthly_subscription_and_service_addon_fields" => {
            add_monthly_and_service_units(pool).await
        }
        "2026_11_01_173533_add_session_addition_support" => add_session_additions(pool).await,
        "2026_11_01_173534_make_patient_demographics_optional" => {
            execute_all(
                pool,
                &[
                    "ALTER TABLE patients MODIFY COLUMN age SMALLINT UNSIGNED NULL",
                    "ALTER TABLE patients MODIFY COLUMN gender ENUM('male','female') NULL",
                ],
            )
            .await
        }
        "2026_11_01_173535_add_typography_settings" => add_typography(pool).await,
        _ => Err(format!("ترحيل قاعدة البيانات غير معروف: {migration}")),
    }
}

async fn add_subscription_cancellation(pool: &MySqlPool) -> Result<(), String> {
    pool.execute(
        "ALTER TABLE subscriptions MODIFY COLUMN status ENUM('active','completed','stopped','cancelled') NOT NULL DEFAULT 'active'",
    )
    .await
    .map_err(migration_error)?;

    if !column_exists(pool, "subscriptions", "cancelled_at").await? {
        execute_all(
            pool,
            &[
                "ALTER TABLE subscriptions ADD COLUMN cancelled_at TIMESTAMP NULL AFTER status",
                "ALTER TABLE subscriptions ADD COLUMN cancelled_by BIGINT UNSIGNED NULL AFTER cancelled_at, ADD CONSTRAINT subscriptions_cancelled_by_foreign FOREIGN KEY (cancelled_by) REFERENCES users(id) ON DELETE RESTRICT",
                "ALTER TABLE subscriptions ADD COLUMN cancel_reason TEXT NULL AFTER cancelled_by",
                "ALTER TABLE subscriptions ADD COLUMN refund_amount DECIMAL(12,3) UNSIGNED NOT NULL DEFAULT 0 AFTER cancel_reason",
            ],
        )
        .await?;
    }

    Ok(())
}

async fn add_monthly_and_service_units(pool: &MySqlPool) -> Result<(), String> {
    if !column_exists(pool, "services", "included_units").await? {
        execute_all(
            pool,
            &[
                "ALTER TABLE services ADD COLUMN included_units SMALLINT UNSIGNED NULL AFTER price",
                "ALTER TABLE services ADD COLUMN unit_name VARCHAR(50) NULL AFTER included_units",
                "ALTER TABLE services ADD COLUMN extra_unit_price DECIMAL(12,3) UNSIGNED NULL AFTER unit_name",
            ],
        )
        .await?;
    }

    if !column_exists(pool, "subscriptions", "plan_type").await? {
        execute_all(
            pool,
            &[
                "ALTER TABLE subscriptions ADD COLUMN plan_type VARCHAR(32) NOT NULL DEFAULT 'legacy_package' AFTER sponsor_id",
                "CREATE INDEX subscriptions_plan_type_index ON subscriptions(plan_type)",
                "ALTER TABLE subscriptions ADD COLUMN end_date DATE NULL AFTER expected_end_date",
            ],
        )
        .await?;
    }

    if !column_exists(pool, "session_services", "base_price").await? {
        execute_all(
            pool,
            &[
                "ALTER TABLE session_services ADD COLUMN base_price DECIMAL(12,3) UNSIGNED NULL AFTER price",
                "ALTER TABLE session_services ADD COLUMN included_units SMALLINT UNSIGNED NOT NULL DEFAULT 0 AFTER base_price",
                "ALTER TABLE session_services ADD COLUMN extra_units SMALLINT UNSIGNED NOT NULL DEFAULT 0 AFTER included_units",
                "ALTER TABLE session_services ADD COLUMN unit_name VARCHAR(50) NULL AFTER extra_units",
                "ALTER TABLE session_services ADD COLUMN extra_unit_price DECIMAL(12,3) UNSIGNED NULL AFTER unit_name",
                "ALTER TABLE session_services ADD COLUMN item_type VARCHAR(32) NOT NULL DEFAULT 'service' AFTER extra_unit_price",
                "ALTER TABLE session_services ADD COLUMN is_addition TINYINT(1) NOT NULL DEFAULT 0 AFTER item_type",
                "ALTER TABLE session_services ADD COLUMN added_at TIMESTAMP NULL AFTER is_addition",
                "UPDATE session_services SET base_price = price WHERE base_price IS NULL",
            ],
        )
        .await?;
    }

    Ok(())
}

async fn add_session_additions(pool: &MySqlPool) -> Result<(), String> {
    execute_all(pool, &[SESSION_ADDITIONS]).await?;

    if !column_exists(pool, "session_services", "session_addition_id").await? {
        if !index_exists(
            pool,
            "session_services",
            "session_services_session_id_index",
        )
        .await?
        {
            pool.execute(
                "CREATE INDEX session_services_session_id_index ON session_services(session_id)",
            )
            .await
            .map_err(migration_error)?;
        }
        if index_exists(
            pool,
            "session_services",
            "session_services_session_id_service_id_unique",
        )
        .await?
        {
            pool.execute(
                "ALTER TABLE session_services DROP INDEX session_services_session_id_service_id_unique",
            )
            .await
            .map_err(migration_error)?;
        }
        execute_all(
            pool,
            &[
                "ALTER TABLE session_services ADD COLUMN session_addition_id BIGINT UNSIGNED NULL AFTER session_id, ADD CONSTRAINT session_services_session_addition_id_foreign FOREIGN KEY (session_addition_id) REFERENCES session_additions(id) ON DELETE RESTRICT",
                "CREATE INDEX session_services_session_id_service_id_index ON session_services(session_id, service_id)",
            ],
        )
        .await?;
        if index_exists(
            pool,
            "session_services",
            "session_services_session_id_index",
        )
        .await?
        {
            pool.execute(
                "ALTER TABLE session_services DROP INDEX session_services_session_id_index",
            )
            .await
            .map_err(migration_error)?;
        }
    }

    for (table, constraint) in [
        ("payments", "payments_session_addition_id_foreign"),
        ("debts", "debts_session_addition_id_foreign"),
        (
            "wallet_transactions",
            "wallet_transactions_session_addition_id_foreign",
        ),
    ] {
        if !column_exists(pool, table, "session_addition_id").await? {
            pool.execute(
                format!(
                    "ALTER TABLE `{table}` ADD COLUMN session_addition_id BIGINT UNSIGNED NULL AFTER session_id, ADD CONSTRAINT `{constraint}` FOREIGN KEY (session_addition_id) REFERENCES session_additions(id) ON DELETE RESTRICT"
                )
                .as_str(),
            )
            .await
            .map_err(migration_error)?;
        }
    }

    Ok(())
}

async fn add_typography(pool: &MySqlPool) -> Result<(), String> {
    if column_exists(pool, "settings", "body_font_size").await? {
        return Ok(());
    }
    execute_all(
        pool,
        &[
            "ALTER TABLE settings ADD COLUMN body_font_size TINYINT UNSIGNED NOT NULL DEFAULT 16 AFTER show_logo",
            "ALTER TABLE settings ADD COLUMN body_font_weight SMALLINT UNSIGNED NOT NULL DEFAULT 400 AFTER body_font_size",
            "ALTER TABLE settings ADD COLUMN table_font_size TINYINT UNSIGNED NOT NULL DEFAULT 15 AFTER body_font_weight",
            "ALTER TABLE settings ADD COLUMN table_font_weight SMALLINT UNSIGNED NOT NULL DEFAULT 400 AFTER table_font_size",
            "ALTER TABLE settings ADD COLUMN heading_font_size TINYINT UNSIGNED NOT NULL DEFAULT 32 AFTER table_font_weight",
            "ALTER TABLE settings ADD COLUMN heading_font_weight SMALLINT UNSIGNED NOT NULL DEFAULT 600 AFTER heading_font_size",
            "ALTER TABLE settings ADD COLUMN section_font_size TINYINT UNSIGNED NOT NULL DEFAULT 22 AFTER heading_font_weight",
            "ALTER TABLE settings ADD COLUMN section_font_weight SMALLINT UNSIGNED NOT NULL DEFAULT 600 AFTER section_font_size",
        ],
    )
    .await
}

async fn execute_all(pool: &MySqlPool, statements: &[&str]) -> Result<(), String> {
    for statement in statements {
        pool.execute(*statement).await.map_err(migration_error)?;
    }
    Ok(())
}

async fn table_exists(pool: &MySqlPool, table: &str) -> Result<bool, String> {
    sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = DATABASE() AND table_name = ?",
    )
    .bind(table)
    .fetch_one(pool)
    .await
    .map(|count| count > 0)
    .map_err(migration_error)
}

async fn column_exists(pool: &MySqlPool, table: &str, column: &str) -> Result<bool, String> {
    sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM information_schema.columns WHERE table_schema = DATABASE() AND table_name = ? AND column_name = ?",
    )
    .bind(table)
    .bind(column)
    .fetch_one(pool)
    .await
    .map(|count| count > 0)
    .map_err(migration_error)
}

async fn index_exists(pool: &MySqlPool, table: &str, index: &str) -> Result<bool, String> {
    sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM information_schema.statistics WHERE table_schema = DATABASE() AND table_name = ? AND index_name = ?",
    )
    .bind(table)
    .bind(index)
    .fetch_one(pool)
    .await
    .map(|count| count > 0)
    .map_err(migration_error)
}

fn migration_error(error: sqlx::Error) -> String {
    format!("فشل ترحيل قاعدة البيانات: {error}")
}

const USERS: &str = "CREATE TABLE IF NOT EXISTS users (\
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY, name VARCHAR(255) NOT NULL, \
    username VARCHAR(255) NOT NULL, password VARCHAR(255) NOT NULL, \
    role ENUM('admin','receptionist') NOT NULL DEFAULT 'receptionist', \
    is_active TINYINT(1) NOT NULL DEFAULT 1, last_login_at TIMESTAMP NULL, \
    created_at TIMESTAMP NULL, updated_at TIMESTAMP NULL, UNIQUE KEY users_username_unique(username), \
    INDEX users_is_active_index(is_active)\
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci";

const PERSONAL_ACCESS_TOKENS: &str = "CREATE TABLE IF NOT EXISTS personal_access_tokens (\
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY, tokenable_type VARCHAR(255) NOT NULL, \
    tokenable_id BIGINT UNSIGNED NOT NULL, name TEXT NOT NULL, token VARCHAR(64) NOT NULL, \
    abilities TEXT NULL, last_used_at TIMESTAMP NULL, expires_at TIMESTAMP NULL, \
    created_at TIMESTAMP NULL, updated_at TIMESTAMP NULL, UNIQUE KEY personal_access_tokens_token_unique(token), \
    INDEX personal_access_tokens_tokenable_type_tokenable_id_index(tokenable_type, tokenable_id), \
    INDEX personal_access_tokens_expires_at_index(expires_at)\
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci";

const SPONSORS: &str = "CREATE TABLE IF NOT EXISTS sponsors (\
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY, name VARCHAR(255) NOT NULL, phone VARCHAR(30) NULL, \
    address VARCHAR(255) NULL, is_active TINYINT(1) NOT NULL DEFAULT 1, created_by BIGINT UNSIGNED NOT NULL, \
    created_at TIMESTAMP NULL, updated_at TIMESTAMP NULL, INDEX sponsors_name_index(name), \
    INDEX sponsors_is_active_index(is_active), CONSTRAINT sponsors_created_by_foreign FOREIGN KEY(created_by) REFERENCES users(id) ON DELETE RESTRICT\
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci";

const DOCTORS: &str = "CREATE TABLE IF NOT EXISTS doctors (\
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY, name VARCHAR(255) NOT NULL, phone VARCHAR(30) NULL, \
    commission_rate DECIMAL(6,3) UNSIGNED NOT NULL, is_active TINYINT(1) NOT NULL DEFAULT 1, \
    created_by BIGINT UNSIGNED NOT NULL, created_at TIMESTAMP NULL, updated_at TIMESTAMP NULL, \
    INDEX doctors_name_index(name), INDEX doctors_is_active_index(is_active), \
    CONSTRAINT doctors_created_by_foreign FOREIGN KEY(created_by) REFERENCES users(id) ON DELETE RESTRICT\
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci";

const SERVICES: &str = "CREATE TABLE IF NOT EXISTS services (\
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY, name VARCHAR(255) NOT NULL, price DECIMAL(12,3) UNSIGNED NOT NULL, \
    is_active TINYINT(1) NOT NULL DEFAULT 1, created_by BIGINT UNSIGNED NOT NULL, created_at TIMESTAMP NULL, updated_at TIMESTAMP NULL, \
    INDEX services_name_index(name), INDEX services_is_active_index(is_active), \
    CONSTRAINT services_created_by_foreign FOREIGN KEY(created_by) REFERENCES users(id) ON DELETE RESTRICT\
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci";

const PATIENTS: &str = "CREATE TABLE IF NOT EXISTS patients (\
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY, name VARCHAR(255) NOT NULL, phone VARCHAR(30) NOT NULL, \
    age SMALLINT UNSIGNED NOT NULL, gender ENUM('male','female') NOT NULL, area VARCHAR(255) NULL, emergency_phone VARCHAR(30) NULL, \
    sponsor_id BIGINT UNSIGNED NULL, is_active TINYINT(1) NOT NULL DEFAULT 1, created_by BIGINT UNSIGNED NOT NULL, \
    created_at TIMESTAMP NULL, updated_at TIMESTAMP NULL, INDEX patients_name_index(name), INDEX patients_phone_index(phone), \
    INDEX patients_is_active_index(is_active), CONSTRAINT patients_sponsor_id_foreign FOREIGN KEY(sponsor_id) REFERENCES sponsors(id) ON DELETE RESTRICT, \
    CONSTRAINT patients_created_by_foreign FOREIGN KEY(created_by) REFERENCES users(id) ON DELETE RESTRICT\
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci";

const SUBSCRIPTIONS: &str = "CREATE TABLE IF NOT EXISTS subscriptions (\
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY, patient_id BIGINT UNSIGNED NOT NULL, sponsor_id BIGINT UNSIGNED NULL, \
    sessions_count INT UNSIGNED NOT NULL, sessions_used INT UNSIGNED NOT NULL DEFAULT 0, base_total DECIMAL(12,3) UNSIGNED NOT NULL, \
    discount DECIMAL(12,3) UNSIGNED NOT NULL DEFAULT 0, net_total DECIMAL(12,3) UNSIGNED NOT NULL, paid_amount DECIMAL(12,3) UNSIGNED NOT NULL DEFAULT 0, \
    start_date DATE NOT NULL, expected_end_date DATE NULL, status ENUM('active','completed','stopped') NOT NULL DEFAULT 'active', \
    created_by BIGINT UNSIGNED NOT NULL, created_at TIMESTAMP NULL, updated_at TIMESTAMP NULL, \
    INDEX subscriptions_patient_id_status_index(patient_id,status), INDEX subscriptions_status_start_date_index(status,start_date), \
    CONSTRAINT subscriptions_patient_id_foreign FOREIGN KEY(patient_id) REFERENCES patients(id) ON DELETE RESTRICT, \
    CONSTRAINT subscriptions_sponsor_id_foreign FOREIGN KEY(sponsor_id) REFERENCES sponsors(id) ON DELETE RESTRICT, \
    CONSTRAINT subscriptions_created_by_foreign FOREIGN KEY(created_by) REFERENCES users(id) ON DELETE RESTRICT\
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci";

const SUBSCRIPTION_SERVICES: &str = "CREATE TABLE IF NOT EXISTS subscription_services (\
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY, subscription_id BIGINT UNSIGNED NOT NULL, service_id BIGINT UNSIGNED NOT NULL, \
    service_name VARCHAR(255) NOT NULL, price DECIMAL(12,3) UNSIGNED NOT NULL, created_by BIGINT UNSIGNED NOT NULL, \
    created_at TIMESTAMP NULL, updated_at TIMESTAMP NULL, UNIQUE KEY subscription_services_subscription_id_service_id_unique(subscription_id,service_id), \
    CONSTRAINT subscription_services_subscription_id_foreign FOREIGN KEY(subscription_id) REFERENCES subscriptions(id) ON DELETE RESTRICT, \
    CONSTRAINT subscription_services_service_id_foreign FOREIGN KEY(service_id) REFERENCES services(id) ON DELETE RESTRICT, \
    CONSTRAINT subscription_services_created_by_foreign FOREIGN KEY(created_by) REFERENCES users(id) ON DELETE RESTRICT\
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci";

const SESSIONS: &str = "CREATE TABLE IF NOT EXISTS sessions (\
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY, code VARCHAR(255) NOT NULL, patient_id BIGINT UNSIGNED NOT NULL, \
    doctor_id BIGINT UNSIGNED NOT NULL, subscription_id BIGINT UNSIGNED NULL, type ENUM('individual','subscription') NOT NULL, \
    status ENUM('open','closed','cancelled') NOT NULL DEFAULT 'open', opened_at TIMESTAMP NOT NULL, closed_at TIMESTAMP NULL, cancelled_at TIMESTAMP NULL, \
    created_by BIGINT UNSIGNED NOT NULL, closed_by BIGINT UNSIGNED NULL, cancelled_by BIGINT UNSIGNED NULL, cancel_reason TEXT NULL, \
    services_total DECIMAL(12,3) UNSIGNED NOT NULL, discount DECIMAL(12,3) UNSIGNED NOT NULL DEFAULT 0, net_total DECIMAL(12,3) UNSIGNED NOT NULL, \
    coverage ENUM('subscription','cash','transfer','wallet','debt','mixed') NOT NULL, wallet_amount DECIMAL(12,3) UNSIGNED NOT NULL DEFAULT 0, \
    paid_amount DECIMAL(12,3) UNSIGNED NOT NULL DEFAULT 0, debt_amount DECIMAL(12,3) UNSIGNED NOT NULL DEFAULT 0, payment_method ENUM('cash','transfer') NULL, \
    sponsor_id BIGINT UNSIGNED NULL, commission_rate DECIMAL(6,3) UNSIGNED NULL, commission_base_amount DECIMAL(12,3) UNSIGNED NULL, \
    commission_amount DECIMAL(12,3) UNSIGNED NULL, created_at TIMESTAMP NULL, updated_at TIMESTAMP NULL, \
    UNIQUE KEY sessions_code_unique(code), \
    INDEX sessions_status_opened_at_index(status,opened_at), INDEX sessions_patient_id_opened_at_index(patient_id,opened_at), \
    INDEX sessions_doctor_id_opened_at_index(doctor_id,opened_at), \
    CONSTRAINT sessions_patient_id_foreign FOREIGN KEY(patient_id) REFERENCES patients(id) ON DELETE RESTRICT, \
    CONSTRAINT sessions_doctor_id_foreign FOREIGN KEY(doctor_id) REFERENCES doctors(id) ON DELETE RESTRICT, \
    CONSTRAINT sessions_subscription_id_foreign FOREIGN KEY(subscription_id) REFERENCES subscriptions(id) ON DELETE RESTRICT, \
    CONSTRAINT sessions_created_by_foreign FOREIGN KEY(created_by) REFERENCES users(id) ON DELETE RESTRICT, \
    CONSTRAINT sessions_closed_by_foreign FOREIGN KEY(closed_by) REFERENCES users(id) ON DELETE RESTRICT, \
    CONSTRAINT sessions_cancelled_by_foreign FOREIGN KEY(cancelled_by) REFERENCES users(id) ON DELETE RESTRICT, \
    CONSTRAINT sessions_sponsor_id_foreign FOREIGN KEY(sponsor_id) REFERENCES sponsors(id) ON DELETE RESTRICT\
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci";

const SESSION_SERVICES: &str = "CREATE TABLE IF NOT EXISTS session_services (\
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY, session_id BIGINT UNSIGNED NOT NULL, service_id BIGINT UNSIGNED NOT NULL, \
    service_name VARCHAR(255) NOT NULL, price DECIMAL(12,3) UNSIGNED NOT NULL, created_by BIGINT UNSIGNED NOT NULL, \
    created_at TIMESTAMP NULL, updated_at TIMESTAMP NULL, UNIQUE KEY session_services_session_id_service_id_unique(session_id,service_id), \
    CONSTRAINT session_services_session_id_foreign FOREIGN KEY(session_id) REFERENCES sessions(id) ON DELETE RESTRICT, \
    CONSTRAINT session_services_service_id_foreign FOREIGN KEY(service_id) REFERENCES services(id) ON DELETE RESTRICT, \
    CONSTRAINT session_services_created_by_foreign FOREIGN KEY(created_by) REFERENCES users(id) ON DELETE RESTRICT\
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci";

const SUBSCRIPTION_USAGES: &str = "CREATE TABLE IF NOT EXISTS subscription_usages (\
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY, subscription_id BIGINT UNSIGNED NOT NULL, session_id BIGINT UNSIGNED NOT NULL, \
    type ENUM('consume','restore') NOT NULL, reverses_usage_id BIGINT UNSIGNED NULL, created_by BIGINT UNSIGNED NOT NULL, \
    created_at TIMESTAMP NULL, updated_at TIMESTAMP NULL, UNIQUE KEY subscription_usages_session_id_type_unique(session_id,type), \
    INDEX subscription_usages_subscription_id_created_at_index(subscription_id,created_at), \
    CONSTRAINT subscription_usages_subscription_id_foreign FOREIGN KEY(subscription_id) REFERENCES subscriptions(id) ON DELETE RESTRICT, \
    CONSTRAINT subscription_usages_session_id_foreign FOREIGN KEY(session_id) REFERENCES sessions(id) ON DELETE RESTRICT, \
    CONSTRAINT subscription_usages_reverses_usage_id_foreign FOREIGN KEY(reverses_usage_id) REFERENCES subscription_usages(id) ON DELETE RESTRICT, \
    CONSTRAINT subscription_usages_created_by_foreign FOREIGN KEY(created_by) REFERENCES users(id) ON DELETE RESTRICT\
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci";

const PAYMENTS: &str = "CREATE TABLE IF NOT EXISTS payments (\
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY, patient_id BIGINT UNSIGNED NOT NULL, session_id BIGINT UNSIGNED NULL, \
    subscription_id BIGINT UNSIGNED NULL, type ENUM('session_payment','subscription_payment','wallet_topup','debt_payment','refund','adjustment') NOT NULL, \
    method ENUM('cash','transfer') NOT NULL, amount DECIMAL(12,3) NOT NULL, reverses_payment_id BIGINT UNSIGNED NULL, note TEXT NULL, \
    created_by BIGINT UNSIGNED NOT NULL, created_at TIMESTAMP NULL, updated_at TIMESTAMP NULL, \
    INDEX payments_patient_id_created_at_index(patient_id,created_at), INDEX payments_type_created_at_index(type,created_at), \
    CONSTRAINT payments_patient_id_foreign FOREIGN KEY(patient_id) REFERENCES patients(id) ON DELETE RESTRICT, \
    CONSTRAINT payments_session_id_foreign FOREIGN KEY(session_id) REFERENCES sessions(id) ON DELETE RESTRICT, \
    CONSTRAINT payments_subscription_id_foreign FOREIGN KEY(subscription_id) REFERENCES subscriptions(id) ON DELETE RESTRICT, \
    CONSTRAINT payments_reverses_payment_id_foreign FOREIGN KEY(reverses_payment_id) REFERENCES payments(id) ON DELETE RESTRICT, \
    CONSTRAINT payments_created_by_foreign FOREIGN KEY(created_by) REFERENCES users(id) ON DELETE RESTRICT\
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci";

const WALLET_TRANSACTIONS: &str = "CREATE TABLE IF NOT EXISTS wallet_transactions (\
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY, patient_id BIGINT UNSIGNED NOT NULL, session_id BIGINT UNSIGNED NULL, payment_id BIGINT UNSIGNED NULL, \
    type ENUM('topup','debit','reversal') NOT NULL, amount DECIMAL(12,3) NOT NULL, balance_after DECIMAL(12,3) UNSIGNED NOT NULL, \
    reverses_transaction_id BIGINT UNSIGNED NULL, note TEXT NULL, created_by BIGINT UNSIGNED NOT NULL, created_at TIMESTAMP NULL, updated_at TIMESTAMP NULL, \
    INDEX wallet_transactions_patient_id_created_at_index(patient_id,created_at), \
    CONSTRAINT wallet_transactions_patient_id_foreign FOREIGN KEY(patient_id) REFERENCES patients(id) ON DELETE RESTRICT, \
    CONSTRAINT wallet_transactions_session_id_foreign FOREIGN KEY(session_id) REFERENCES sessions(id) ON DELETE RESTRICT, \
    CONSTRAINT wallet_transactions_payment_id_foreign FOREIGN KEY(payment_id) REFERENCES payments(id) ON DELETE RESTRICT, \
    CONSTRAINT wallet_transactions_reverses_transaction_id_foreign FOREIGN KEY(reverses_transaction_id) REFERENCES wallet_transactions(id) ON DELETE RESTRICT, \
    CONSTRAINT wallet_transactions_created_by_foreign FOREIGN KEY(created_by) REFERENCES users(id) ON DELETE RESTRICT\
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci";

const DEBTS: &str = "CREATE TABLE IF NOT EXISTS debts (\
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY, patient_id BIGINT UNSIGNED NOT NULL, sponsor_id BIGINT UNSIGNED NULL, \
    session_id BIGINT UNSIGNED NULL, subscription_id BIGINT UNSIGNED NULL, kind ENUM('session','subscription','opening') NOT NULL, \
    amount DECIMAL(12,3) UNSIGNED NOT NULL, paid_amount DECIMAL(12,3) UNSIGNED NOT NULL DEFAULT 0, status ENUM('open','partial','paid','void') NOT NULL DEFAULT 'open', \
    voided_at TIMESTAMP NULL, voided_by BIGINT UNSIGNED NULL, void_reason TEXT NULL, note TEXT NULL, created_by BIGINT UNSIGNED NOT NULL, \
    created_at TIMESTAMP NULL, updated_at TIMESTAMP NULL, INDEX debts_status_created_at_index(status,created_at), \
    INDEX debts_patient_id_status_index(patient_id,status), INDEX debts_sponsor_id_status_index(sponsor_id,status), \
    CONSTRAINT debts_patient_id_foreign FOREIGN KEY(patient_id) REFERENCES patients(id) ON DELETE RESTRICT, \
    CONSTRAINT debts_sponsor_id_foreign FOREIGN KEY(sponsor_id) REFERENCES sponsors(id) ON DELETE RESTRICT, \
    CONSTRAINT debts_session_id_foreign FOREIGN KEY(session_id) REFERENCES sessions(id) ON DELETE RESTRICT, \
    CONSTRAINT debts_subscription_id_foreign FOREIGN KEY(subscription_id) REFERENCES subscriptions(id) ON DELETE RESTRICT, \
    CONSTRAINT debts_voided_by_foreign FOREIGN KEY(voided_by) REFERENCES users(id) ON DELETE RESTRICT, \
    CONSTRAINT debts_created_by_foreign FOREIGN KEY(created_by) REFERENCES users(id) ON DELETE RESTRICT\
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci";

const PAYMENT_ALLOCATIONS: &str = "CREATE TABLE IF NOT EXISTS payment_allocations (\
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY, payment_id BIGINT UNSIGNED NOT NULL, debt_id BIGINT UNSIGNED NOT NULL, \
    amount DECIMAL(12,3) UNSIGNED NOT NULL, created_by BIGINT UNSIGNED NOT NULL, created_at TIMESTAMP NULL, updated_at TIMESTAMP NULL, \
    UNIQUE KEY payment_allocations_payment_id_debt_id_unique(payment_id,debt_id), \
    CONSTRAINT payment_allocations_payment_id_foreign FOREIGN KEY(payment_id) REFERENCES payments(id) ON DELETE RESTRICT, \
    CONSTRAINT payment_allocations_debt_id_foreign FOREIGN KEY(debt_id) REFERENCES debts(id) ON DELETE RESTRICT, \
    CONSTRAINT payment_allocations_created_by_foreign FOREIGN KEY(created_by) REFERENCES users(id) ON DELETE RESTRICT\
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci";

const DOCTOR_PAYOUTS: &str = "CREATE TABLE IF NOT EXISTS doctor_payouts (\
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY, doctor_id BIGINT UNSIGNED NOT NULL, amount DECIMAL(12,3) UNSIGNED NOT NULL, \
    date DATE NOT NULL, method ENUM('cash','transfer') NOT NULL, note TEXT NULL, created_by BIGINT UNSIGNED NOT NULL, \
    created_at TIMESTAMP NULL, updated_at TIMESTAMP NULL, INDEX doctor_payouts_doctor_id_date_index(doctor_id,date), \
    CONSTRAINT doctor_payouts_doctor_id_foreign FOREIGN KEY(doctor_id) REFERENCES doctors(id) ON DELETE RESTRICT, \
    CONSTRAINT doctor_payouts_created_by_foreign FOREIGN KEY(created_by) REFERENCES users(id) ON DELETE RESTRICT\
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci";

const AUDIT_LOGS: &str = "CREATE TABLE IF NOT EXISTS audit_logs (\
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY, user_id BIGINT UNSIGNED NOT NULL, action VARCHAR(255) NOT NULL, \
    entity_type VARCHAR(255) NOT NULL, entity_id BIGINT UNSIGNED NOT NULL, old_values JSON NULL, new_values JSON NULL, reason TEXT NULL, \
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP, INDEX audit_logs_action_index(action), \
    INDEX audit_logs_entity_type_entity_id_index(entity_type,entity_id), INDEX audit_logs_user_id_created_at_index(user_id,created_at), \
    CONSTRAINT audit_logs_user_id_foreign FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE RESTRICT\
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci";

const IDEMPOTENCY_KEYS: &str = "CREATE TABLE IF NOT EXISTS idempotency_keys (\
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY, user_id BIGINT UNSIGNED NOT NULL, `key` CHAR(36) NOT NULL, route VARCHAR(255) NOT NULL, \
    request_hash VARCHAR(64) NOT NULL, status SMALLINT UNSIGNED NULL, response JSON NULL, created_at TIMESTAMP NULL, updated_at TIMESTAMP NULL, \
    UNIQUE KEY idempotency_keys_user_id_route_key_unique(user_id,route,`key`), \
    CONSTRAINT idempotency_keys_user_id_foreign FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE RESTRICT\
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci";

const SETTINGS: &str = "CREATE TABLE IF NOT EXISTS settings (\
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY, clinic_name VARCHAR(255) NOT NULL, clinic_subtitle VARCHAR(255) NULL, \
    phone VARCHAR(30) NULL, address VARCHAR(255) NULL, receipt_footer TEXT NULL, currency VARCHAR(10) NOT NULL DEFAULT 'د.ل', \
    print_size ENUM('A4','A5') NOT NULL DEFAULT 'A4', show_logo TINYINT(1) NOT NULL DEFAULT 1, updated_by BIGINT UNSIGNED NULL, \
    created_at TIMESTAMP NULL, updated_at TIMESTAMP NULL, CONSTRAINT settings_updated_by_foreign FOREIGN KEY(updated_by) REFERENCES users(id) ON DELETE RESTRICT\
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci";

const SESSION_ADDITIONS: &str = "CREATE TABLE IF NOT EXISTS session_additions (\
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY, session_id BIGINT UNSIGNED NOT NULL, services_total DECIMAL(12,3) UNSIGNED NOT NULL, \
    net_total DECIMAL(12,3) UNSIGNED NOT NULL, coverage ENUM('subscription','cash','transfer','wallet','debt','mixed') NOT NULL, \
    wallet_amount DECIMAL(12,3) UNSIGNED NOT NULL DEFAULT 0, paid_amount DECIMAL(12,3) UNSIGNED NOT NULL DEFAULT 0, \
    debt_amount DECIMAL(12,3) UNSIGNED NOT NULL DEFAULT 0, payment_method ENUM('cash','transfer') NULL, sponsor_id BIGINT UNSIGNED NULL, \
    created_by BIGINT UNSIGNED NOT NULL, created_at TIMESTAMP NULL, updated_at TIMESTAMP NULL, \
    INDEX session_additions_session_id_created_at_index(session_id,created_at), \
    CONSTRAINT session_additions_session_id_foreign FOREIGN KEY(session_id) REFERENCES sessions(id) ON DELETE RESTRICT, \
    CONSTRAINT session_additions_sponsor_id_foreign FOREIGN KEY(sponsor_id) REFERENCES sponsors(id) ON DELETE RESTRICT, \
    CONSTRAINT session_additions_created_by_foreign FOREIGN KEY(created_by) REFERENCES users(id) ON DELETE RESTRICT\
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::db;

    async fn test_pool(variable: &str) -> MySqlPool {
        let database = std::env::var(variable)
            .unwrap_or_else(|_| panic!("{variable} must name an isolated MySQL test database"));
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

    #[tokio::test]
    #[ignore = "requires an isolated MySQL 8 database"]
    async fn creates_and_validates_fresh_schema() {
        let pool = test_pool("RUST_MYSQL_DATABASE").await;
        reset(&pool).await.expect("create fresh Rust schema");
        validate(&pool).await.expect("validate fresh Rust schema");

        let migration_count =
            sqlx::query_scalar::<_, u64>("SELECT CAST(COUNT(*) AS UNSIGNED) FROM migrations")
                .fetch_one(&pool)
                .await
                .expect("count applied migrations");
        assert_eq!(migration_count, MIGRATIONS.len() as u64);
    }

    #[tokio::test]
    #[ignore = "requires a Laravel-migrated isolated MySQL 8 database"]
    async fn accepts_existing_laravel_schema_without_changing_rows() {
        let pool = test_pool("RUST_MYSQL_EXISTING_DATABASE").await;
        let before = table_counts(&pool).await;
        migrate(&pool)
            .await
            .expect("accept existing Laravel schema");
        let after = table_counts(&pool).await;
        assert_eq!(after, before);
    }

    #[tokio::test]
    #[ignore = "requires isolated Rust-fresh and Laravel-migrated MySQL 8 databases"]
    async fn fresh_schema_matches_laravel_schema_definition() {
        let rust_pool = test_pool("RUST_MYSQL_DATABASE").await;
        let laravel_pool = test_pool("RUST_MYSQL_EXISTING_DATABASE").await;
        reset(&rust_pool).await.expect("create fresh Rust schema");

        assert_eq!(
            schema_signature(&rust_pool).await,
            schema_signature(&laravel_pool).await,
        );
    }

    async fn table_counts(pool: &MySqlPool) -> Vec<(String, u64)> {
        let tables = sqlx::query_scalar::<_, String>(
            "SELECT CAST(table_name AS CHAR) FROM information_schema.tables \
             WHERE table_schema = DATABASE() AND table_name != 'migrations' ORDER BY table_name",
        )
        .fetch_all(pool)
        .await
        .expect("list test tables");
        let mut counts = Vec::with_capacity(tables.len());
        for table in tables {
            let query = format!("SELECT CAST(COUNT(*) AS UNSIGNED) FROM `{table}`");
            let count = sqlx::query_scalar::<_, u64>(&query)
                .fetch_one(pool)
                .await
                .expect("count test table rows");
            counts.push((table, count));
        }
        counts
    }

    async fn schema_signature(pool: &MySqlPool) -> Vec<String> {
        let mut signature = sqlx::query_scalar::<_, String>(
            "SELECT CAST(CONCAT_WS('|', 'column', table_name, column_name, ordinal_position, \
                    column_type, is_nullable, COALESCE(CAST(column_default AS CHAR), '<NULL>'), extra) AS CHAR) \
             FROM information_schema.columns WHERE table_schema = DATABASE()",
        )
        .fetch_all(pool)
        .await
        .expect("read schema columns");
        signature.extend(
            sqlx::query_scalar::<_, String>(
                "SELECT CAST(CONCAT_WS('|', 'index', table_name, index_name, non_unique, \
                        seq_in_index, column_name) AS CHAR) \
                 FROM information_schema.statistics WHERE table_schema = DATABASE()",
            )
            .fetch_all(pool)
            .await
            .expect("read schema indexes"),
        );
        signature.extend(
            sqlx::query_scalar::<_, String>(
                "SELECT CAST(CONCAT_WS('|', 'foreign', k.table_name, k.constraint_name, \
                        k.column_name, k.referenced_table_name, k.referenced_column_name, r.delete_rule) AS CHAR) \
                 FROM information_schema.key_column_usage k \
                 INNER JOIN information_schema.referential_constraints r \
                   ON r.constraint_schema = k.constraint_schema AND r.constraint_name = k.constraint_name \
                 WHERE k.table_schema = DATABASE() AND k.referenced_table_name IS NOT NULL",
            )
            .fetch_all(pool)
            .await
            .expect("read schema foreign keys"),
        );
        signature.sort();
        signature
    }
}
