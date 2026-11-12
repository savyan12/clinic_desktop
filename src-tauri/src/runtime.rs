use crate::{
    api::{self, config::ApiConfig, AppState},
    models::{
        ApiServerProcess, DesktopState, InstallConfig, NewActivationRequest, RuntimeContext,
        RuntimeExecutables, RuntimeInfo, RuntimePaths,
    },
};
use crate::{native_process, runtime_files};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use bcrypt::{hash, DEFAULT_COST};
use sha2::{Digest, Sha256};
use sqlx::MySqlPool;
use std::fs::{self, File, OpenOptions};
use std::future::Future;
use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Output, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};
use tokio::sync::oneshot;
use uuid::Uuid;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

const CONFIG_FORMAT_VERSION: u32 = 1;
const STARTUP_TIMEOUT: Duration = Duration::from_secs(45);
const MYSQL_STARTUP_TIMEOUT: Duration = Duration::from_secs(120);
const DATABASE_OPERATION_TIMEOUT: Duration = Duration::from_secs(180);
const ACTIVATION_DEADLOCK_RETRIES: usize = 2;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

pub(crate) fn hide_console_window(command: &mut Command) {
    #[cfg(target_os = "windows")]
    command.creation_flags(CREATE_NO_WINDOW);

    #[cfg(not(target_os = "windows"))]
    let _ = command;
}

pub fn initialize(app: &AppHandle, state: &DesktopState) -> Result<(), String> {
    let _operation = state
        .operation_lock
        .lock()
        .map_err(|_| "تعذر بدء تشغيل النظام المحلي.".to_string())?;
    let paths = resolve_paths(app)?;
    let config = prepare_install_config(state, &paths)?;

    {
        let mut inner = state
            .inner
            .lock()
            .map_err(|_| "تعذر الوصول إلى حالة التطبيق المحلي.".to_string())?;
        inner.paths = Some(paths.clone());
        inner.config = Some(config.clone());
        inner.ready = false;
        inner.last_error = None;
    }

    let executables = resolve_executables(&paths)?;
    state
        .inner
        .lock()
        .map_err(|_| "تعذر الوصول إلى حالة التطبيق المحلي.".to_string())?
        .executables = Some(executables);

    start_prepared_runtime(state)
}

fn start_prepared_runtime(state: &DesktopState) -> Result<(), String> {
    let prepared = context(state)?;
    let config = &prepared.config;
    if config.activated {
        ensure_mysql_running(state)?;
        start_rust_api(state)?;
    } else if mysql_data_exists(&prepared.paths.mysql_data_dir)? {
        // A crash during activation may have committed the users before its flag.
        let context = ensure_mysql_running(state)?;
        let output = mysql_output(&context, &[
            "--batch".into(), "--skip-column-names".into(), "-e".into(),
            format!("SELECT COUNT(*) FROM information_schema.tables WHERE table_schema='{}' AND table_name='users'", escape_sql_string(&config.database)),
        ])?;
        if parse_mysql_count(&output.stdout)? > 0 {
            let output = mysql_output(
                &context,
                &[
                    "--batch".into(),
                    "--skip-column-names".into(),
                    "-e".into(),
                    format!(
                        "SELECT COUNT(*) FROM {}.users",
                        quote_identifier(&config.database)
                    ),
                ],
            )?;
            if parse_mysql_count(&output.stdout)? > 0 {
                migrate_database(&context, &config.database, true)?;
                validate_restored_database(&context, &config.database)?;
                mark_activated_and_start_api(state)?;
                return Ok(());
            }
        }
        let api_config = database_api_config(&context, &config.database, true);
        let empty = run_async_operation(
            async move {
                let pool = api::db::connect(&api_config).await?;
                let result = ensure_no_existing_rows(&pool).await;
                pool.close().await;
                result
            },
            "انتهت مهلة فحص بيانات التثبيت الموجود.",
        );
        if let Err(error) = empty {
            state
                .inner
                .lock()
                .map_err(|_| "تعذر الوصول لحالة النظام.".to_string())?
                .recovery_required = true;
            return Err(error);
        }
    }

    Ok(())
}

fn prepare_install_config(
    state: &DesktopState,
    paths: &RuntimePaths,
) -> Result<InstallConfig, String> {
    {
        let mut inner = state
            .inner
            .lock()
            .map_err(|_| "تعذر الوصول لحالة النظام.".to_string())?;
        inner.paths = Some(paths.clone());
        inner.ready = false;
        if inner.instance_lock.is_none() {
            inner.instance_lock = Some(runtime_files::acquire_instance_lock(&paths.runtime_dir)?);
        }
    }
    if paths.runtime_dir.join("RECOVERY_REQUIRED").exists() {
        state
            .inner
            .lock()
            .map_err(|_| "تعذر الوصول لحالة النظام.".to_string())?
            .recovery_required = true;
        return Err("استعادة سابقة لم تكتمل بأمان. احتفظ بالنسخة الاحتياطية ومجلد restore-work؛ يلزم أن يتحقق المسؤول من البيانات قبل تشغيل النظام.".into());
    }
    let loaded = runtime_files::load_config(&paths.config_file).and_then(|loaded| {
        if loaded.is_none() && mysql_data_exists(&paths.mysql_data_dir)? {
            return Err("ملفات التشغيل مفقودة مع وجود بيانات MySQL. بيانات العيادة محفوظة؛ يلزم استرداد إعدادات التشغيل بواسطة المسؤول.".into());
        }
        Ok(loaded)
    });
    let loaded = match loaded {
        Ok(loaded) => loaded,
        Err(error) => {
            state
                .inner
                .lock()
                .map_err(|_| "تعذر الوصول لحالة النظام.".to_string())?
                .recovery_required = true;
            return Err(error);
        }
    };
    let (config, recovered) = match loaded {
        Some(loaded) => (loaded.config, loaded.recovered),
        None => {
            let config = default_config();
            save_config(&paths.config_file, &config)?;
            (config, false)
        }
    };
    let mut inner = state
        .inner
        .lock()
        .map_err(|_| "تعذر الوصول لحالة النظام.".to_string())?;
    inner.recovery_required = false;
    inner.recovery_notice = recovered.then(|| "استُعيدت إعدادات التشغيل من النسخة السليمة. أنشئ نسخة احتياطية حديثة بعد التحقق من البيانات.".into());
    Ok(config)
}

fn mysql_data_exists(directory: &Path) -> Result<bool, String> {
    match fs::read_dir(directory) {
        Ok(mut entries) => entries
            .next()
            .transpose()
            .map(|entry| entry.is_some())
            .map_err(|error| format!("تعذر فحص بيانات MySQL الموجودة: {error}")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!("تعذر فحص بيانات MySQL الموجودة: {error}")),
    }
}

pub fn retry_runtime(app: &AppHandle, state: &DesktopState) -> Result<(), String> {
    let reinitialize = {
        let inner = state
            .inner
            .lock()
            .map_err(|_| "تعذر الوصول لحالة النظام.".to_string())?;
        inner.config.as_ref().is_none_or(|config| !config.activated)
            || inner.instance_lock.is_none()
            || inner.recovery_required
    };
    let result = if reinitialize {
        initialize(app, state)
    } else {
        start_existing(state)
    };
    if let Err(error) = &result {
        record_runtime_error(state, error);
    }
    result
}

pub fn runtime_info(state: &DesktopState) -> Result<RuntimeInfo, String> {
    let (should_check_health, api_port) = {
        let mut inner = state
            .inner
            .lock()
            .map_err(|_| "تعذر الوصول إلى حالة التطبيق المحلي.".to_string())?;
        let api_finished = inner
            .api_server
            .as_ref()
            .is_some_and(|server| server.task.inner().is_finished());
        if inner.ready && (inner.api_server.is_none() || api_finished) {
            inner.api_server = None;
            inner.ready = false;
            inner.last_error =
                Some("توقفت خدمة Rust المحلية. أعد تشغيلها من شاشة التفعيل.".to_string());
        }
        (
            inner.ready,
            inner
                .config
                .as_ref()
                .map(|config| config.api_port)
                .unwrap_or(0),
        )
    };

    if should_check_health && !health_request(api_port).unwrap_or(false) {
        let mut inner = state
            .inner
            .lock()
            .map_err(|_| "تعذر الوصول إلى حالة التطبيق المحلي.".to_string())?;
        inner.ready = false;
        inner.last_error =
            Some("خدمة Rust المحلية لا تستجيب. أعد تشغيلها من شاشة التفعيل.".to_string());
    }

    let inner = state
        .inner
        .lock()
        .map_err(|_| "تعذر الوصول إلى حالة التطبيق المحلي.".to_string())?;
    let mysql_port = inner
        .config
        .as_ref()
        .map(|config| config.mysql_port)
        .unwrap_or(0);
    let api_port = inner
        .config
        .as_ref()
        .map(|config| config.api_port)
        .unwrap_or(0);

    Ok(RuntimeInfo {
        desktop: true,
        ready: inner.ready,
        activated: inner.config.as_ref().is_some_and(|value| value.activated),
        api_base_url: format!("http://127.0.0.1:{api_port}/api/v1"),
        mysql_port,
        api_port,
        error: inner.last_error.clone(),
        recovery_required: inner.recovery_required,
        recovery_notice: inner.recovery_notice.clone(),
    })
}

pub fn activate_new(
    state: &DesktopState,
    request: NewActivationRequest,
) -> Result<RuntimeInfo, String> {
    let _operation = state
        .operation_lock
        .lock()
        .map_err(|_| "تعذر بدء عملية التفعيل المحلية.".to_string())?;
    validate_activation_request(&request)?;
    let context = ensure_mysql_running(state)?;

    if context.config.activated {
        return Err("تم تفعيل النظام مسبقًا.".to_string());
    }

    activate_new_database(&context, request)?;

    mark_activated_and_start_api(state)
}

pub fn start_existing(state: &DesktopState) -> Result<(), String> {
    let _operation = state
        .operation_lock
        .lock()
        .map_err(|_| "تعذر بدء تشغيل الخدمات المحلية.".to_string())?;
    ensure_mysql_running(state)?;
    start_rust_api(state)?;
    Ok(())
}

pub fn ensure_mysql_running(state: &DesktopState) -> Result<RuntimeContext, String> {
    let mut inner = state
        .inner
        .lock()
        .map_err(|_| "تعذر الوصول إلى حالة التطبيق المحلي.".to_string())?;
    let paths = inner
        .paths
        .clone()
        .ok_or_else(|| "مسارات التشغيل المحلي غير مهيأة.".to_string())?;
    let executables = inner
        .executables
        .clone()
        .ok_or_else(|| "ملفات التشغيل المحلي غير متاحة.".to_string())?;
    let mut config = inner.config.clone().ok_or_else(|| {
        "إعدادات التشغيل غير متاحة؛ استرداد الإعدادات مطلوب قبل تشغيل MySQL.".to_string()
    })?;
    if inner.recovery_required {
        return Err("يلزم استرداد النظام قبل إعادة تشغيل الخدمات.".into());
    }
    if inner.instance_lock.is_none() {
        inner.instance_lock = Some(runtime_files::acquire_instance_lock(&paths.runtime_dir)?);
    }

    if inner.mysql_child.as_mut().is_some_and(child_is_running) {
        return Ok(RuntimeContext {
            config,
            paths,
            executables,
        });
    }

    if let Some(mut exited_child) = inner.mysql_child.take() {
        let _ = exited_child.wait();
    }

    if health_request(config.api_port).unwrap_or(false) {
        return Err(
            "خدمة نسخة أخرى من النظام ما زالت تعمل. أغلق النسخة الأخرى ثم أعد المحاولة.".into(),
        );
    }

    save_config(&paths.config_file, &config)?;
    write_mysql_client_file(&paths.root_client_file, &config)?;

    let mut context = RuntimeContext {
        config: config.clone(),
        paths: paths.clone(),
        executables: executables.clone(),
    };

    if port_is_in_use(config.mysql_port) {
        if mysql_credentials_work(&context) {
            if !mysql_uses_data_directory(&context)? {
                return Err(
                    "منفذ MySQL مستخدم بواسطة خادم له مجلد بيانات مختلف. لم تُعدّل بيانات العيادة."
                        .into(),
                );
            }
            inner.config = Some(config);
            inner.mysql_managed = true;
            drop(inner);
            ensure_application_database(&context)?;

            return Ok(context);
        }

        config.mysql_port = available_port_except(config.api_port)?;
        context.config = config.clone();
        save_config(&paths.config_file, &config)?;
        write_mysql_client_file(&paths.root_client_file, &config)?;
    }

    let initialized = initialize_mysql_data(&paths, &executables)?;
    let bootstrap_file = paths.runtime_dir.join("mysql-bootstrap.sql");
    if initialized || bootstrap_file.exists() {
        write_mysql_bootstrap_file(&bootstrap_file, &config)?;
    }

    let child = spawn_mysql(
        &paths,
        &executables,
        &config,
        bootstrap_file.exists().then_some(bootstrap_file.as_path()),
    )?;
    inner.mysql_child = Some(child);
    inner.mysql_managed = true;
    inner.config = Some(config.clone());
    drop(inner);

    let setup_result = (|| {
        wait_for_mysql_port(state, config.mysql_port)?;
        establish_mysql_credentials(&context)?;
        ensure_application_database(&context)
    })();

    if let Err(error) = setup_result {
        stop_mysql_child(state);
        record_runtime_error(state, &error);

        return Err(error);
    }

    if bootstrap_file.exists() {
        fs::remove_file(&bootstrap_file)
            .map_err(|error| format!("تعذر حذف ملف تهيئة MySQL المؤقت: {error}"))?;
    }

    Ok(context)
}

pub fn context(state: &DesktopState) -> Result<RuntimeContext, String> {
    let inner = state
        .inner
        .lock()
        .map_err(|_| "تعذر الوصول إلى حالة التطبيق المحلي.".to_string())?;

    Ok(RuntimeContext {
        config: inner
            .config
            .clone()
            .ok_or_else(|| "لم يتم إعداد التشغيل المحلي بعد.".to_string())?,
        paths: inner
            .paths
            .clone()
            .ok_or_else(|| "مسارات التشغيل المحلي غير مهيأة.".to_string())?,
        executables: inner
            .executables
            .clone()
            .ok_or_else(|| "ملفات التشغيل المحلي غير متاحة.".to_string())?,
    })
}

fn activate_new_database(
    context: &RuntimeContext,
    request: NewActivationRequest,
) -> Result<(), String> {
    let config = database_api_config(context, &context.config.database, false);
    let clinic_name = request.clinic_name.trim().to_string();
    let clinic_subtitle = request.clinic_subtitle.and_then(|value| {
        let value = value.trim().to_string();
        (!value.is_empty()).then_some(value)
    });
    let admin_name = request.admin_name.trim().to_string();
    let admin_username = request.admin_username.trim().to_string();
    let password_hash = hash(request.admin_password, DEFAULT_COST)
        .map_err(|error| format!("تعذر حماية كلمة مرور المدير: {error}"))?;

    run_async_operation(
        async move {
            let mut last_deadlock = None;
            for attempt in 0..=ACTIVATION_DEADLOCK_RETRIES {
                let pool = api::db::connect(&config).await?;
                let result = activate_new_database_attempt(
                    &pool,
                    clinic_name.clone(),
                    clinic_subtitle.clone(),
                    admin_name.clone(),
                    admin_username.clone(),
                    password_hash.clone(),
                )
                .await;
                pool.close().await;

                match result {
                    Err(error)
                        if is_mysql_deadlock(&error) && attempt < ACTIVATION_DEADLOCK_RETRIES =>
                    {
                        last_deadlock = Some(error);
                        tokio::time::sleep(Duration::from_millis(250 * (attempt as u64 + 1))).await;
                    }
                    result => return result,
                }
            }

            Err(last_deadlock
                .unwrap_or_else(|| "تعذر تهيئة قاعدة بيانات العيادة بعد عدة محاولات.".to_string()))
        },
        "انتهت مهلة تهيئة قاعدة بيانات العيادة.",
    )
}

async fn activate_new_database_attempt(
    pool: &MySqlPool,
    clinic_name: String,
    clinic_subtitle: Option<String>,
    admin_name: String,
    admin_username: String,
    password_hash: String,
) -> Result<(), String> {
    let users_table_exists = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM information_schema.tables \
         WHERE table_schema = DATABASE() AND table_name = 'users'",
    )
    .fetch_one(pool)
    .await
    .map_err(|error| format!("تعذر فحص قاعدة بيانات التفعيل: {error}"))?
        > 0;
    if users_table_exists {
        let users_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM users")
            .fetch_one(pool)
            .await
            .map_err(|error| format!("تعذر فحص مستخدمي النظام: {error}"))?;
        if users_count > 0 {
            return Err(
                "قاعدة البيانات المحلية تحتوي على مستخدمين ولا يمكن تهيئتها كتثبيت جديد."
                    .to_string(),
            );
        }
    }

    ensure_no_existing_rows(pool).await?;

    api::schema::reset(pool).await?;
    let mut transaction = pool
        .begin()
        .await
        .map_err(|error| format!("تعذر بدء معاملة التفعيل: {error}"))?;
    let admin_id = sqlx::query(
        "INSERT INTO users \
         (name, username, password, role, is_active, created_at, updated_at) \
         VALUES (?, ?, ?, 'admin', 1, NOW(), NOW())",
    )
    .bind(admin_name)
    .bind(admin_username)
    .bind(password_hash)
    .execute(&mut *transaction)
    .await
    .map_err(|error| format!("تعذر إنشاء حساب المدير: {error}"))?
    .last_insert_id();
    sqlx::query(
        "INSERT INTO settings \
         (clinic_name, clinic_subtitle, currency, print_size, show_logo, \
          body_font_size, body_font_weight, table_font_size, table_font_weight, \
          heading_font_size, heading_font_weight, section_font_size, \
          section_font_weight, updated_by, created_at, updated_at) \
         VALUES (?, ?, 'د.ل', 'A4', 1, 16, 400, 15, 400, 32, 600, 22, 600, ?, NOW(), NOW())",
    )
    .bind(clinic_name)
    .bind(clinic_subtitle)
    .bind(admin_id)
    .execute(&mut *transaction)
    .await
    .map_err(|error| format!("تعذر حفظ إعدادات العيادة: {error}"))?;
    transaction
        .commit()
        .await
        .map_err(|error| format!("تعذر اعتماد تفعيل النظام: {error}"))?;
    Ok(())
}

async fn ensure_no_existing_rows(pool: &MySqlPool) -> Result<(), String> {
    // A missing/deleted users table does not make existing clinical data disposable.
    let tables = sqlx::query_scalar::<_, String>(
        "SELECT table_name FROM information_schema.tables \
         WHERE table_schema = DATABASE() AND table_type = 'BASE TABLE' AND table_name <> 'migrations'",
    )
    .fetch_all(pool)
    .await
    .map_err(|error| format!("تعذر فحص البيانات الموجودة قبل التفعيل: {error}"))?;
    for table in tables {
        let has_rows = sqlx::query_scalar::<_, i64>(&format!(
            "SELECT EXISTS(SELECT 1 FROM {} LIMIT 1)",
            quote_identifier(&table),
        ))
        .fetch_one(pool)
        .await
        .map_err(|error| format!("تعذر التأكد من خلو قاعدة البيانات قبل التفعيل: {error}"))?;
        if has_rows != 0 {
            return Err("قاعدة البيانات المحلية تحتوي على سجلات ولا يمكن تهيئتها كتثبيت جديد. يلزم استرداد التثبيت الموجود بواسطة المسؤول.".into());
        }
    }

    Ok(())
}

fn is_mysql_deadlock(error: &str) -> bool {
    error.contains("1213")
        || error.contains("ER_LOCK_DEADLOCK")
        || error.contains("Deadlock found when trying to get lock")
}

pub fn migrate_database(
    context: &RuntimeContext,
    database: &str,
    use_root: bool,
) -> Result<(), String> {
    let config = database_api_config(context, database, use_root);
    run_async_operation(
        async move {
            let pool = api::db::connect(&config).await?;
            let result = api::schema::migrate(&pool).await;
            pool.close().await;
            result
        },
        "انتهت مهلة ترحيل قاعدة البيانات.",
    )
}

fn database_api_config(context: &RuntimeContext, database: &str, use_root: bool) -> ApiConfig {
    let (username, password) = if use_root {
        ("root".to_string(), context.config.root_password.clone())
    } else {
        (
            context.config.app_user.clone(),
            context.config.app_password.clone(),
        )
    };
    ApiConfig {
        host: "127.0.0.1".to_string(),
        port: context.config.api_port,
        mysql_host: "127.0.0.1".to_string(),
        mysql_port: context.config.mysql_port,
        database: database.to_string(),
        username,
        password,
        commission_base: api::config::CommissionBase::from_env(),
        subscription_follow_up_sessions: 2,
    }
}

fn run_async_operation<T, F>(future: F, timeout_message: &str) -> Result<T, String>
where
    T: Send + 'static,
    F: Future<Output = Result<T, String>> + Send + 'static,
{
    let (sender, receiver) = mpsc::sync_channel(1);
    let task = tauri::async_runtime::spawn(async move {
        let _ = sender.send(future.await);
    });

    match receiver.recv_timeout(DATABASE_OPERATION_TIMEOUT) {
        Ok(result) => result,
        Err(mpsc::RecvTimeoutError::Timeout) => {
            if task.inner().is_finished() {
                if let Ok(result) = receiver.try_recv() {
                    return result;
                }
            }

            task.abort();
            while !task.inner().is_finished() {
                thread::sleep(Duration::from_millis(10));
            }

            match receiver.try_recv() {
                Ok(result) => result,
                Err(_) => Err(format!(
                    "{timeout_message} أوقفنا المحاولة السابقة قبل السماح بإعادة المحاولة."
                )),
            }
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            Err("توقفت عملية قاعدة البيانات قبل إكمالها.".to_string())
        }
    }
}

pub fn mark_activated_and_start_api(state: &DesktopState) -> Result<RuntimeInfo, String> {
    {
        let mut inner = state
            .inner
            .lock()
            .map_err(|_| "تعذر الوصول إلى حالة التطبيق المحلي.".to_string())?;
        let paths = inner
            .paths
            .clone()
            .ok_or_else(|| "مسارات التشغيل المحلي غير مهيأة.".to_string())?;
        let mut config = inner
            .config
            .clone()
            .ok_or_else(|| "إعدادات التشغيل المحلي غير موجودة.".to_string())?;
        config.activated = true;
        save_config(&paths.config_file, &config)?;
        inner.config = Some(config);
        inner.last_error = None;
    }
    start_rust_api(state)?;
    runtime_info(state)
}

pub fn start_rust_api(state: &DesktopState) -> Result<(), String> {
    let existing_port = {
        let inner = state
            .inner
            .lock()
            .map_err(|_| "تعذر الوصول إلى حالة التطبيق المحلي.".to_string())?;
        if inner.recovery_required {
            return Err("يلزم التحقق من الاستعادة وإكمال الاسترداد قبل تشغيل الخدمة المحلية.".into());
        }
        inner.api_server.as_ref().and_then(|server| {
            (!server.task.inner().is_finished())
                .then(|| inner.config.as_ref().map(|config| config.api_port))
                .flatten()
        })
    };

    if let Some(port) = existing_port {
        if health_request(port).unwrap_or(false) {
            let mut inner = state
                .inner
                .lock()
                .map_err(|_| "تعذر الوصول إلى حالة التطبيق المحلي.".to_string())?;
            inner.ready = true;
            inner.last_error = None;
            return Ok(());
        }
    }

    stop_rust_api(state)?;

    let api_config = {
        let mut inner = state
            .inner
            .lock()
            .map_err(|_| "تعذر الوصول إلى حالة التطبيق المحلي.".to_string())?;
        let paths = inner
            .paths
            .clone()
            .ok_or_else(|| "مسارات التشغيل المحلي غير مهيأة.".to_string())?;
        let mut config = inner
            .config
            .clone()
            .ok_or_else(|| "إعدادات التشغيل المحلي غير موجودة.".to_string())?;

        if port_is_in_use(config.api_port) {
            config.api_port = available_port_except(config.mysql_port)?;
            save_config(&paths.config_file, &config)?;
            inner.config = Some(config.clone());
        }

        ApiConfig {
            host: "127.0.0.1".to_string(),
            port: config.api_port,
            mysql_host: "127.0.0.1".to_string(),
            mysql_port: config.mysql_port,
            database: config.database,
            username: config.app_user,
            password: config.app_password,
            commission_base: api::config::CommissionBase::from_env(),
            subscription_follow_up_sessions: 2,
        }
    };

    let api_port = api_config.port;
    let (shutdown_sender, shutdown_receiver) = oneshot::channel();
    let (startup_sender, startup_receiver) = mpsc::sync_channel(1);
    let task_state = state.clone();
    let task = tauri::async_runtime::spawn(async move {
        let mut startup_sender = Some(startup_sender);
        let result = async {
            let pool = api::db::connect(&api_config).await?;
            api::schema::migrate(&pool).await?;
            let listener =
                tokio::net::TcpListener::bind((api_config.host.as_str(), api_config.port))
                    .await
                    .map_err(|error| format!("تعذر تشغيل خدمة Rust المحلية: {error}"))?;
            if let Some(sender) = startup_sender.take() {
                let _ = sender.send(Ok(()));
            }

            let server_result = axum::serve(
                listener,
                api::router(AppState {
                    pool: pool.clone(),
                    config: api_config,
                    login_rate_limiter: Default::default(),
                }),
            )
            .with_graceful_shutdown(async move {
                let _ = shutdown_receiver.await;
            })
            .await
            .map_err(|error| format!("توقفت خدمة Rust المحلية: {error}"));
            pool.close().await;
            server_result
        }
        .await;

        if let Err(error) = &result {
            if let Some(sender) = startup_sender.take() {
                let _ = sender.send(Err(error.clone()));
            }
        }
        if let Ok(mut inner) = task_state.inner.lock() {
            inner.ready = false;
            if let Err(error) = result {
                inner.last_error = Some(error);
            }
        }
    });

    {
        let mut inner = state
            .inner
            .lock()
            .map_err(|_| "تعذر الوصول إلى حالة التطبيق المحلي.".to_string())?;
        inner.api_server = Some(ApiServerProcess {
            shutdown: shutdown_sender,
            task,
        });
        inner.ready = false;
        inner.last_error = None;
    }

    match startup_receiver.recv_timeout(DATABASE_OPERATION_TIMEOUT) {
        Ok(Ok(())) => {}
        Ok(Err(error)) => {
            let _ = stop_rust_api(state);
            record_runtime_error(state, &error);
            return Err(error);
        }
        Err(_) => {
            let error = "انتهت مهلة تجهيز خدمة Rust المحلية.".to_string();
            let _ = stop_rust_api(state);
            record_runtime_error(state, &error);
            return Err(error);
        }
    }

    if let Err(error) = wait_for_rust_health(state, api_port) {
        let _ = stop_rust_api(state);
        record_runtime_error(state, &error);
        return Err(error);
    }

    let mut inner = state
        .inner
        .lock()
        .map_err(|_| "تعذر الوصول إلى حالة التطبيق المحلي.".to_string())?;
    inner.ready = true;
    inner.last_error = None;
    Ok(())
}

pub fn stop_rust_api(state: &DesktopState) -> Result<(), String> {
    let server = {
        let mut inner = state
            .inner
            .lock()
            .map_err(|_| "تعذر الوصول إلى حالة التطبيق المحلي.".to_string())?;
        inner.ready = false;
        inner.api_server.take()
    };

    if let Some(ApiServerProcess { shutdown, task }) = server {
        let _ = shutdown.send(());
        let deadline = Instant::now() + Duration::from_secs(8);
        while Instant::now() < deadline && !task.inner().is_finished() {
            thread::sleep(Duration::from_millis(50));
        }
        if !task.inner().is_finished() {
            task.abort();
            while !task.inner().is_finished() {
                thread::sleep(Duration::from_millis(10));
            }
        }
    }

    Ok(())
}

fn stop_mysql_child(state: &DesktopState) {
    if let Ok(mut inner) = state.inner.lock() {
        if let Some(mut child) = inner.mysql_child.take() {
            native_process::terminate_owned_child(&mut child);
        }
        inner.mysql_managed = false;
    }
}

pub fn shutdown(state: &DesktopState) {
    let _operation = state.operation_lock.lock().ok();
    let context = context(state).ok();
    let managed = state
        .inner
        .lock()
        .map(|inner| inner.mysql_managed || inner.mysql_child.is_some())
        .unwrap_or(false);
    let _ = stop_rust_api(state);

    if let Some(context) = context.filter(|_| managed) {
        let mut command = Command::new(&context.executables.mysqladmin);
        hide_console_window(&mut command);
        command
            .arg(format!(
                "--defaults-file={}",
                context.paths.root_client_file.display()
            ))
            .args(["--no-login-paths", "--connect-timeout=5", "shutdown"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let result = native_process::output(&mut command, Duration::from_secs(15), "إغلاق MySQL");
        if let Err(error) = result.and_then(|output| {
            if output.status.success() {
                Ok(())
            } else {
                Err("تعذر إغلاق MySQL بشكل سليم.".into())
            }
        }) {
            record_runtime_error(state, &error);
        }
    }

    if let Ok(mut inner) = state.inner.lock() {
        if let Some(mut child) = inner.mysql_child.take() {
            let deadline = Instant::now() + Duration::from_secs(8);
            while Instant::now() < deadline {
                if child.try_wait().ok().flatten().is_some() {
                    break;
                }
                thread::sleep(Duration::from_millis(100));
            }
            if child.try_wait().ok().flatten().is_none() {
                native_process::terminate_owned_child(&mut child);
            }
        }
        inner.mysql_managed = false;
        inner.instance_lock = None;
    }
}

pub fn mysql_output(context: &RuntimeContext, args: &[String]) -> Result<Output, String> {
    let mut command = mysql_command(context);
    command
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let output = native_process::output(
        &mut command,
        DATABASE_OPERATION_TIMEOUT,
        "الاتصال بقاعدة البيانات المحلية",
    )?;

    if !output.status.success() {
        return Err(format!(
            "فشل أمر قاعدة البيانات: {}",
            command_error(&output)
        ));
    }

    Ok(output)
}

pub fn mysql_command(context: &RuntimeContext) -> Command {
    let mut command = Command::new(&context.executables.mysql);
    hide_console_window(&mut command);
    command.arg(format!(
        "--defaults-file={}",
        context.paths.root_client_file.display()
    ));
    command.args(["--no-login-paths", "--connect-timeout=5"]);
    command
}

pub fn mysqldump_command(context: &RuntimeContext) -> Command {
    let mut command = Command::new(&context.executables.mysqldump);
    hide_console_window(&mut command);
    command.arg(format!(
        "--defaults-file={}",
        context.paths.root_client_file.display()
    ));
    command.arg("--no-login-paths");
    command
}

pub fn quote_identifier(value: &str) -> String {
    format!("`{}`", value.replace('`', "``"))
}

fn resolve_paths(app: &AppHandle) -> Result<RuntimePaths, String> {
    let local_data = app
        .path()
        .app_local_data_dir()
        .map_err(|error| format!("تعذر تحديد مجلد بيانات التطبيق: {error}"))?;
    let runtime_dir = local_data.join("runtime");
    let mysql_data_dir = local_data.join("mysql-data");
    let logs_dir = local_data.join("logs");
    for directory in [&runtime_dir, &mysql_data_dir, &logs_dir] {
        fs::create_dir_all(directory)
            .map_err(|error| format!("تعذر إنشاء مجلد التشغيل {}: {error}", directory.display()))?;
    }

    let resource_dir = app
        .path()
        .resource_dir()
        .map_err(|error| format!("تعذر تحديد ملفات التطبيق: {error}"))?;
    let packaged_root = resource_dir.join("runtime");
    let development_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/runtime");
    let resource_root = if packaged_root.exists() {
        packaged_root
    } else {
        development_root
    };

    Ok(RuntimePaths {
        resource_root,
        runtime_dir: runtime_dir.clone(),
        mysql_data_dir,
        mysql_log: logs_dir.join("mysql.log"),
        config_file: runtime_dir.join("install.json"),
        root_client_file: runtime_dir.join("mysql-root.cnf"),
    })
}

fn resolve_executables(_paths: &RuntimePaths) -> Result<RuntimeExecutables, String> {
    #[cfg(target_os = "windows")]
    if !_paths.resource_root.to_string_lossy().is_ascii() {
        return Err(format!(
            "مسار تثبيت البرنامج يحتوي على أحرف غير إنجليزية لا يدعمها MySQL المرفق حاليًا. أعد تثبيت البرنامج في مسار إنجليزي مثل D:\\ClinicDesktop. بيانات العيادة المحفوظة لن تُحذف.\nالمسار الحالي: {}",
            _paths.resource_root.display()
        ));
    }

    #[cfg(target_os = "windows")]
    let executables = RuntimeExecutables {
        mysqld: _paths.resource_root.join("mysql/bin/mysqld.exe"),
        mysql: _paths.resource_root.join("mysql/bin/mysql.exe"),
        mysqldump: _paths.resource_root.join("mysql/bin/mysqldump.exe"),
        mysqladmin: _paths.resource_root.join("mysql/bin/mysqladmin.exe"),
        mysql_base: Some(_paths.resource_root.join("mysql")),
    };

    #[cfg(not(target_os = "windows"))]
    let executables = RuntimeExecutables {
        mysqld: resolve_system_executable(
            "mysqld",
            &[
                "/usr/sbin/mysqld",
                "/usr/bin/mysqld",
                "/usr/libexec/mysqld",
                "/usr/local/mysql/bin/mysqld",
            ],
        ),
        mysql: resolve_system_executable("mysql", &["/usr/bin/mysql", "/usr/local/bin/mysql"]),
        mysqldump: resolve_system_executable(
            "mysqldump",
            &["/usr/bin/mysqldump", "/usr/local/bin/mysqldump"],
        ),
        mysqladmin: resolve_system_executable(
            "mysqladmin",
            &["/usr/bin/mysqladmin", "/usr/local/bin/mysqladmin"],
        ),
        mysql_base: None,
    };

    #[cfg(target_os = "windows")]
    for executable in [
        &executables.mysqld,
        &executables.mysql,
        &executables.mysqldump,
        &executables.mysqladmin,
    ] {
        if !executable.exists() {
            return Err(format!(
                "ملف التشغيل المحلي غير موجود: {}",
                executable.display()
            ));
        }
    }

    Ok(executables)
}

#[cfg(not(target_os = "windows"))]
fn resolve_system_executable(name: &str, fallback_paths: &[&str]) -> PathBuf {
    if let Some(path) = std::env::var_os("PATH").and_then(|path_value| {
        std::env::split_paths(&path_value)
            .map(|directory| directory.join(name))
            .find(|candidate| candidate.is_file())
    }) {
        return path;
    }

    fallback_paths
        .iter()
        .map(PathBuf::from)
        .find(|candidate| candidate.is_file())
        .unwrap_or_else(|| PathBuf::from(name))
}

fn default_config() -> InstallConfig {
    let mysql_port = available_port().unwrap_or(33068);
    let api_port = available_port_except(mysql_port).unwrap_or(18000);

    InstallConfig {
        format_version: CONFIG_FORMAT_VERSION,
        mysql_port,
        api_port,
        database: "clinic_system".to_string(),
        app_user: "clinic_app".to_string(),
        app_password: random_secret(),
        root_password: random_secret(),
        app_key: generate_app_key(),
        activated: false,
    }
}

fn save_config(path: &Path, config: &InstallConfig) -> Result<(), String> {
    runtime_files::save_config(path, config)
}

fn write_mysql_client_file(path: &Path, config: &InstallConfig) -> Result<(), String> {
    let contents = format!(
        "[client]\nuser=root\npassword=\"{}\"\nhost=127.0.0.1\nport={}\nprotocol=tcp\ndefault-character-set=utf8mb4\n",
        config.root_password.replace('\\', "\\\\").replace('"', "\\\""), config.mysql_port
    );
    runtime_files::atomic_write(path, contents.as_bytes())
}

fn write_mysql_bootstrap_file(path: &Path, config: &InstallConfig) -> Result<(), String> {
    let password = escape_sql_string(&config.root_password);
    let sql = format!(
        "ALTER USER 'root'@'localhost' IDENTIFIED BY '{password}';\n\
         CREATE USER IF NOT EXISTS 'root'@'127.0.0.1' IDENTIFIED BY '{password}';\n\
         ALTER USER 'root'@'127.0.0.1' IDENTIFIED BY '{password}';\n\
         GRANT ALL PRIVILEGES ON *.* TO 'root'@'127.0.0.1' WITH GRANT OPTION;\n\
         FLUSH PRIVILEGES;\n"
    );
    runtime_files::atomic_write(path, sql.as_bytes())
}

fn mysql_credentials_work(context: &RuntimeContext) -> bool {
    let mut command = mysql_command(context);
    command
        .args(["--batch", "--skip-column-names", "-e", "SELECT 1"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    native_process::output(&mut command, Duration::from_secs(15), "فحص اتصال MySQL")
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn mysql_uses_data_directory(context: &RuntimeContext) -> Result<bool, String> {
    let output = mysql_output(
        context,
        &[
            "--batch".into(),
            "--skip-column-names".into(),
            "-e".into(),
            "SELECT @@datadir".into(),
        ],
    )?;
    let actual = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());
    let actual = actual
        .canonicalize()
        .map_err(|error| format!("تعذر التحقق من مجلد MySQL الجاري: {error}"))?;
    let expected = context
        .paths
        .mysql_data_dir
        .canonicalize()
        .map_err(|error| format!("تعذر التحقق من مجلد بيانات العيادة: {error}"))?;
    Ok(actual == expected)
}

fn initialize_mysql_data(
    paths: &RuntimePaths,
    executables: &RuntimeExecutables,
) -> Result<bool, String> {
    if paths.mysql_data_dir.join("mysql").exists() {
        return Ok(false);
    }
    if mysql_data_exists(&paths.mysql_data_dir)? {
        return Err("مجلد MySQL يحتوي ملفات موجودة أو تهيئة غير مكتملة. لم تُعد تهيئته لحماية البيانات؛ يلزم فحصه بواسطة المسؤول.".into());
    }
    fs::create_dir_all(&paths.mysql_data_dir)
        .map_err(|error| format!("تعذر إنشاء بيانات MySQL: {error}"))?;
    let mut command = Command::new(&executables.mysqld);
    hide_console_window(&mut command);
    command
        .arg("--no-defaults")
        .arg("--initialize-insecure")
        .arg(format!("--datadir={}", paths.mysql_data_dir.display()))
        .arg(format!("--log-error={}", paths.mysql_log.display()));
    if let Some(base) = &executables.mysql_base {
        command.arg(format!("--basedir={}", base.display()));
    }
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let output = native_process::output(
        &mut command,
        DATABASE_OPERATION_TIMEOUT,
        "تهيئة MySQL المحلي",
    )?;
    if !output.status.success() {
        return Err(format!(
            "فشلت تهيئة MySQL المحلي: {}",
            command_error(&output)
        ));
    }
    Ok(true)
}

fn spawn_mysql(
    paths: &RuntimePaths,
    executables: &RuntimeExecutables,
    config: &InstallConfig,
    init_file: Option<&Path>,
) -> Result<Child, String> {
    let socket_file = paths.runtime_dir.join("mysql.sock");
    let socket_lock_file = paths.runtime_dir.join("mysql.sock.lock");

    for stale_file in [&socket_file, &socket_lock_file] {
        if stale_file.exists() {
            fs::remove_file(stale_file).map_err(|error| {
                format!(
                    "تعذر تنظيف ملف MySQL المحلي {}: {error}",
                    stale_file.display()
                )
            })?;
        }
    }

    let stdout = append_log(&paths.mysql_log)?;
    let stderr = stdout
        .try_clone()
        .map_err(|error| format!("تعذر فتح سجل MySQL: {error}"))?;
    let mut command = Command::new(&executables.mysqld);
    hide_console_window(&mut command);
    command
        .arg("--no-defaults")
        .arg(format!("--datadir={}", paths.mysql_data_dir.display()))
        .arg(format!("--port={}", config.mysql_port))
        .arg("--bind-address=127.0.0.1")
        .arg(format!("--socket={}", socket_file.display()))
        .arg("--mysqlx=OFF")
        .arg("--skip-name-resolve")
        .arg("--character-set-server=utf8mb4")
        .arg("--collation-server=utf8mb4_unicode_ci")
        .arg(format!("--log-error={}", paths.mysql_log.display()))
        .arg(format!(
            "--pid-file={}",
            paths.runtime_dir.join("mysql.pid").display()
        ))
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr));
    if let Some(base) = &executables.mysql_base {
        command.arg(format!("--basedir={}", base.display()));
    }
    if let Some(init_file) = init_file {
        command.arg(format!("--init-file={}", init_file.display()));
    }
    command
        .spawn()
        .map_err(|error| format!("تعذر تشغيل MySQL المحلي: {error}"))
}

fn establish_mysql_credentials(context: &RuntimeContext) -> Result<(), String> {
    if mysql_credentials_work(context) {
        return Ok(());
    }

    let sql = format!(
        "ALTER USER 'root'@'localhost' IDENTIFIED BY '{password}'; \
         CREATE USER IF NOT EXISTS 'root'@'127.0.0.1' IDENTIFIED BY '{password}'; \
         ALTER USER 'root'@'127.0.0.1' IDENTIFIED BY '{password}'; \
         GRANT ALL PRIVILEGES ON *.* TO 'root'@'127.0.0.1' WITH GRANT OPTION; \
         FLUSH PRIVILEGES;",
        password = escape_sql_string(&context.config.root_password),
    );

    for host in ["127.0.0.1", "localhost"] {
        let mut command = Command::new(&context.executables.mysql);
        hide_console_window(&mut command);
        command
            .args([
                "--no-defaults",
                "--no-login-paths",
                "--connect-timeout=5",
                "--protocol=tcp",
                &format!("--host={host}"),
                &format!("--port={}", context.config.mysql_port),
                "--user=root",
                "--skip-password",
                "-e",
                &sql,
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let output = native_process::output(
            &mut command,
            Duration::from_secs(15),
            "تأمين حساب MySQL المحلي",
        )?;
        if output.status.success() && mysql_credentials_work(context) {
            return Ok(());
        }
    }

    Err("تعذر تأمين حساب MySQL المحلي والتحقق من بيانات دخوله.".to_string())
}

fn ensure_application_database(context: &RuntimeContext) -> Result<(), String> {
    let database = quote_identifier(&context.config.database);
    let user = escape_sql_string(&context.config.app_user);
    let password = escape_sql_string(&context.config.app_password);
    let sql = format!(
        "CREATE DATABASE IF NOT EXISTS {database} CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci; \
         CREATE USER IF NOT EXISTS '{user}'@'127.0.0.1' IDENTIFIED BY '{password}'; \
         CREATE USER IF NOT EXISTS '{user}'@'localhost' IDENTIFIED BY '{password}'; \
         ALTER USER '{user}'@'127.0.0.1' IDENTIFIED BY '{password}'; \
         ALTER USER '{user}'@'localhost' IDENTIFIED BY '{password}'; \
         GRANT ALL PRIVILEGES ON {database}.* TO '{user}'@'127.0.0.1'; \
         GRANT ALL PRIVILEGES ON {database}.* TO '{user}'@'localhost'; FLUSH PRIVILEGES;"
    );
    mysql_output(context, &["-e".to_string(), sql])?;
    Ok(())
}

fn wait_for_mysql_port(state: &DesktopState, port: u16) -> Result<(), String> {
    let deadline = Instant::now() + MYSQL_STARTUP_TIMEOUT;
    while Instant::now() < deadline {
        if TcpStream::connect_timeout(
            &SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port),
            Duration::from_millis(300),
        )
        .is_ok()
        {
            return Ok(());
        }
        let exited = {
            let mut inner = state
                .inner
                .lock()
                .map_err(|_| "تعذر الوصول إلى حالة التطبيق المحلي.".to_string())?;
            inner
                .mysql_child
                .as_mut()
                .and_then(|value| value.try_wait().ok().flatten())
        };
        if let Some(status) = exited {
            return Err(mysql_startup_error(state, status));
        }
        thread::sleep(Duration::from_millis(250));
    }
    Err("انتهت مهلة تشغيل الخدمة المحلية.".to_string())
}

fn mysql_startup_error(state: &DesktopState, status: ExitStatus) -> String {
    let log_path = state
        .inner
        .lock()
        .ok()
        .and_then(|inner| inner.paths.as_ref().map(|paths| paths.mysql_log.clone()));
    let exit_code = status
        .code()
        .map(|code| code.to_string())
        .unwrap_or_else(|| status.to_string());

    if let Some(path) = log_path {
        format!(
            "توقفت خدمة MySQL المحلية مبكرًا (رمز الخروج: {exit_code}).\nآخر 12 سطرًا من سجل MySQL:\n{}\nمسار السجل: {}",
            mysql_log_tail(&path, 12),
            path.display()
        )
    } else {
        format!("توقفت خدمة MySQL المحلية مبكرًا (رمز الخروج: {exit_code}).")
    }
}

fn mysql_log_tail(path: &Path, line_count: usize) -> String {
    match fs::read(path) {
        Ok(bytes) => {
            let text = String::from_utf8_lossy(&bytes);
            let mut lines = text.lines().rev().take(line_count).collect::<Vec<_>>();
            lines.reverse();
            if lines.is_empty() {
                "سجل MySQL فارغ.".to_string()
            } else {
                lines.join("\n")
            }
        }
        Err(error) => format!("تعذر قراءة سجل MySQL: {error}"),
    }
}

fn wait_for_rust_health(state: &DesktopState, port: u16) -> Result<(), String> {
    let deadline = Instant::now() + STARTUP_TIMEOUT;
    while Instant::now() < deadline {
        if health_request(port).unwrap_or(false) {
            return Ok(());
        }
        let failure = {
            let inner = state
                .inner
                .lock()
                .map_err(|_| "تعذر الوصول إلى حالة التطبيق المحلي.".to_string())?;
            inner.api_server.as_ref().and_then(|server| {
                server.task.inner().is_finished().then(|| {
                    inner.last_error.clone().unwrap_or_else(|| {
                        "توقفت خدمة Rust المحلية قبل اكتمال فحص الجاهزية.".to_string()
                    })
                })
            })
        };
        if let Some(error) = failure {
            return Err(error);
        }
        thread::sleep(Duration::from_millis(100));
    }
    Err("بدأت خدمة Rust لكن فحص الجاهزية لم ينجح.".to_string())
}

fn health_request(port: u16) -> Result<bool, String> {
    let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(1))
        .map_err(|error| error.to_string())?;
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|error| error.to_string())?;
    stream
        .write_all(
            format!(
                "GET /api/v1/health HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n"
            )
            .as_bytes(),
        )
        .map_err(|error| error.to_string())?;
    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .map_err(|error| error.to_string())?;
    Ok(response.starts_with("HTTP/1.1 200") || response.starts_with("HTTP/1.0 200"))
}

fn available_port() -> Result<u16, String> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .map_err(|error| format!("تعذر حجز منفذ محلي: {error}"))?;
    listener
        .local_addr()
        .map(|address| address.port())
        .map_err(|error| format!("تعذر قراءة المنفذ المحلي: {error}"))
}

fn available_port_except(excluded: u16) -> Result<u16, String> {
    for _ in 0..10 {
        let port = available_port()?;
        if port != excluded {
            return Ok(port);
        }
    }

    Err("تعذر حجز منفذين محليين مختلفين لخدمات النظام.".to_string())
}

fn port_is_in_use(port: u16) -> bool {
    TcpListener::bind((Ipv4Addr::LOCALHOST, port)).is_err()
}

fn child_is_running(child: &mut Child) -> bool {
    child.try_wait().is_ok_and(|status| status.is_none())
}

fn append_log(path: &Path) -> Result<File, String> {
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| format!("تعذر فتح سجل التشغيل {}: {error}", path.display()))
}

fn validate_activation_request(request: &NewActivationRequest) -> Result<(), String> {
    if request.clinic_name.trim().is_empty()
        || request.admin_name.trim().is_empty()
        || request.admin_username.trim().is_empty()
    {
        return Err("اسم العيادة واسم المدير واسم المستخدم مطلوبة.".to_string());
    }
    if request.admin_password.chars().count() < 8 {
        return Err("كلمة مرور المدير يجب ألا تقل عن 8 أحرف.".to_string());
    }
    Ok(())
}

fn random_secret() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

fn generate_app_key() -> String {
    let mut hasher = Sha256::new();
    hasher.update(Uuid::new_v4().as_bytes());
    hasher.update(Uuid::new_v4().as_bytes());
    format!("base64:{}", BASE64.encode(hasher.finalize()))
}

fn escape_sql_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('\'', "''")
}

fn command_error(output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !stderr.is_empty() {
        stderr
    } else if !stdout.is_empty() {
        stdout
    } else {
        output.status.to_string()
    }
}

fn parse_mysql_count(bytes: &[u8]) -> Result<u64, String> {
    String::from_utf8_lossy(bytes)
        .trim()
        .parse()
        .map_err(|_| "تعذر تأكيد حالة قاعدة البيانات؛ لم يُعتبر التثبيت فارغًا.".to_string())
}

pub fn record_runtime_error(state: &DesktopState, error: &str) {
    let log = if let Ok(mut inner) = state.inner.lock() {
        let mut safe_error = error.to_string();
        if let Some(config) = &inner.config {
            for secret in [&config.root_password, &config.app_password, &config.app_key] {
                if !secret.is_empty() {
                    safe_error = safe_error.replace(secret, "[محجوب]");
                }
            }
        }
        inner.ready = false;
        inner.last_error = Some(safe_error.clone());
        inner
            .paths
            .as_ref()
            .and_then(|paths| paths.mysql_log.parent())
            .map(|parent| (parent.join("runtime.log"), safe_error))
    } else {
        None
    };
    if let Some((path, error)) = log {
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
            let _ = writeln!(
                file,
                "{} {}",
                chrono::Utc::now().to_rfc3339(),
                error.replace(['\r', '\n'], " ")
            );
            let _ = file.sync_all();
        }
    }
}

pub fn require_recovery(state: &DesktopState, error: &str) {
    let marker = if let Ok(mut inner) = state.inner.lock() {
        inner.recovery_required = true;
        inner
            .paths
            .as_ref()
            .map(|paths| paths.runtime_dir.join("RECOVERY_REQUIRED"))
    } else {
        None
    };
    if let Some(marker) = marker.filter(|path| !path.exists()) {
        if let Err(marker_error) = runtime_files::atomic_write(
            &marker,
            b"An incomplete restore requires a verified recovery before startup.\n",
        ) {
            record_runtime_error(state, &marker_error);
        }
    }
    record_runtime_error(state, error);
}

pub fn begin_database_replacement(state: &DesktopState, work_dir: &Path) -> Result<(), String> {
    let mut inner = state
        .inner
        .lock()
        .map_err(|_| "تعذر حماية عملية الاستعادة.".to_string())?;
    if inner.recovery_required {
        return Err("توجد عملية استرداد سابقة لم تكتمل.".into());
    }
    let paths = inner
        .paths
        .as_ref()
        .ok_or_else(|| "مسارات التشغيل غير متاحة.".to_string())?;
    let message = format!(
        "An incomplete restore requires verified recovery before startup.\nRestore work: {}\n",
        work_dir.display()
    );
    runtime_files::atomic_write(
        &paths.runtime_dir.join("RECOVERY_REQUIRED"),
        message.as_bytes(),
    )?;
    inner.recovery_required = true;
    inner.ready = false;
    Ok(())
}

// Call only after the restored database or its rollback has passed validation.
pub fn finish_database_replacement(state: &DesktopState) -> Result<(), String> {
    let mut inner = state
        .inner
        .lock()
        .map_err(|_| "تعذر إكمال حماية الاستعادة.".to_string())?;
    let paths = inner
        .paths
        .as_ref()
        .ok_or_else(|| "مسارات التشغيل غير متاحة.".to_string())?;
    match fs::remove_file(paths.runtime_dir.join("RECOVERY_REQUIRED")) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(format!(
                "تم التحقق من البيانات لكن تعذر إزالة علامة الاسترداد: {error}"
            ))
        }
    }
    inner.recovery_required = false;
    Ok(())
}

pub fn validate_restored_database(context: &RuntimeContext, database: &str) -> Result<(), String> {
    let config = database_api_config(context, database, true);
    run_async_operation(
        async move {
            let pool = api::db::connect(&config).await?;
            let result = async {
                api::schema::validate(&pool).await?;
                let admins = sqlx::query_scalar::<_, i64>(
                    "SELECT COUNT(*) FROM users WHERE role='admin' AND is_active=1",
                )
                .fetch_one(&pool)
                .await
                .map_err(|error| format!("تعذر فحص حساب المدير في النسخة: {error}"))?;
                let settings = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM settings")
                    .fetch_one(&pool)
                    .await
                    .map_err(|error| format!("تعذر فحص إعدادات العيادة في النسخة: {error}"))?;
                if admins < 1 || settings != 1 {
                    return Err("النسخة لا تحتوي حساب مدير فعالًا وإعدادات عيادة مكتملة.".into());
                }
                Ok(())
            }
            .await;
            pool.close().await;
            result
        },
        "انتهت مهلة التحقق من قاعدة البيانات المستعادة.",
    )
}

#[cfg(test)]
mod tests {
    use super::{
        activate_new_database, available_port, escape_sql_string, health_request, mysql_log_tail,
        quote_identifier, run_async_operation, runtime_info, start_rust_api, stop_rust_api,
    };
    use crate::{
        api,
        models::{
            DesktopState, InstallConfig, NewActivationRequest, RuntimeContext, RuntimeExecutables,
            RuntimePaths,
        },
    };
    use bcrypt::verify;
    use rust_decimal::Decimal;
    use std::{
        fs,
        io::{Read, Write},
        net::{Ipv4Addr, TcpListener, TcpStream},
        path::PathBuf,
        thread,
        time::{Duration, Instant},
    };
    use uuid::Uuid;

    struct RuntimeFilesFixture {
        root: PathBuf,
        paths: RuntimePaths,
    }
    impl RuntimeFilesFixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "clinic-runtime-regression-{}",
                Uuid::new_v4().simple()
            ));
            let runtime_dir = root.join("runtime");
            let mysql_data_dir = root.join("mysql-data");
            let logs_dir = root.join("logs");
            for directory in [&runtime_dir, &mysql_data_dir, &logs_dir] {
                fs::create_dir_all(directory).unwrap();
            }
            Self {
                paths: RuntimePaths {
                    resource_root: PathBuf::new(),
                    runtime_dir: runtime_dir.clone(),
                    mysql_data_dir,
                    mysql_log: logs_dir.join("mysql.log"),
                    config_file: runtime_dir.join("install.json"),
                    root_client_file: runtime_dir.join("mysql-root.cnf"),
                },
                root,
            }
        }
    }
    impl Drop for RuntimeFilesFixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn missing_config_with_existing_mysql_data_never_generates_new_credentials() {
        let fixture = RuntimeFilesFixture::new();
        let data = fixture.paths.mysql_data_dir.join("ibdata1");
        fs::write(&data, b"existing data fixture").unwrap();
        let state = DesktopState::default();
        let error = super::prepare_install_config(&state, &fixture.paths).unwrap_err();
        super::record_runtime_error(&state, &error);
        let info = runtime_info(&state).unwrap();
        assert!(info.recovery_required);
        assert!(!info.ready);
        assert!(!fixture.paths.config_file.exists());
        assert_eq!(fs::read(data).unwrap(), b"existing data fixture");
        super::shutdown(&state);
    }

    #[test]
    fn corrupt_configuration_cannot_fall_through_to_a_new_installation() {
        let fixture = RuntimeFilesFixture::new();
        fs::write(&fixture.paths.config_file, [0; 390]).unwrap();
        let state = DesktopState::default();
        assert!(super::prepare_install_config(&state, &fixture.paths).is_err());
        assert!(state.inner.lock().unwrap().recovery_required);
        assert!(super::ensure_mysql_running(&state).is_err());
        assert_eq!(fs::read(&fixture.paths.config_file).unwrap(), vec![0; 390]);
        super::shutdown(&state);
    }

    #[test]
    fn zeroed_mysql_client_file_is_regenerated_from_the_same_config() {
        let fixture = RuntimeFilesFixture::new();
        let config = super::default_config();
        super::write_mysql_client_file(&fixture.paths.root_client_file, &config).unwrap();
        let expected = fs::read(&fixture.paths.root_client_file).unwrap();
        fs::write(&fixture.paths.root_client_file, [0; 162]).unwrap();
        super::write_mysql_client_file(&fixture.paths.root_client_file, &config).unwrap();
        assert_eq!(fs::read(&fixture.paths.root_client_file).unwrap(), expected);
    }

    #[test]
    fn recovered_configuration_preserves_activation_and_existing_data() {
        let fixture = RuntimeFilesFixture::new();
        let mut original = super::default_config();
        original.activated = true;
        super::save_config(&fixture.paths.config_file, &original).unwrap();
        fs::write(&fixture.paths.config_file, [0; 390]).unwrap();
        fs::write(&fixture.paths.root_client_file, [0; 162]).unwrap();
        let data = fixture.paths.mysql_data_dir.join("ibdata1");
        fs::write(&data, b"existing clinic data fixture").unwrap();
        let state = DesktopState::default();

        let recovered = super::prepare_install_config(&state, &fixture.paths).unwrap();
        assert_eq!(recovered.root_password, original.root_password);
        assert_eq!(recovered.app_password, original.app_password);
        assert!(recovered.activated);
        assert!(!state.inner.lock().unwrap().recovery_required);
        assert!(state.inner.lock().unwrap().recovery_notice.is_some());
        super::write_mysql_client_file(&fixture.paths.root_client_file, &recovered).unwrap();
        assert!(!fs::read(&fixture.paths.root_client_file)
            .unwrap()
            .contains(&0));
        assert_eq!(fs::read(data).unwrap(), b"existing clinic data fixture");
        super::shutdown(&state);
    }

    #[test]
    fn second_runtime_is_rejected_before_repairing_or_creating_config_files() {
        let fixture = RuntimeFilesFixture::new();
        let owner =
            crate::runtime_files::acquire_instance_lock(&fixture.paths.runtime_dir).unwrap();
        fs::write(&fixture.paths.config_file, [0; 390]).unwrap();
        let state = DesktopState::default();

        let error = super::prepare_install_config(&state, &fixture.paths).unwrap_err();
        assert!(error.contains("نسخة أخرى"));
        assert!(state.inner.lock().unwrap().instance_lock.is_none());
        assert_eq!(fs::read(&fixture.paths.config_file).unwrap(), vec![0; 390]);
        assert!(!fixture
            .paths
            .config_file
            .with_extension("json.backup")
            .exists());
        assert!(!fixture.paths.root_client_file.exists());
        super::shutdown(&state);
        drop(owner);
    }

    #[test]
    fn activation_is_not_reported_as_committed_when_config_save_fails() {
        let fixture = RuntimeFilesFixture::new();
        let original = super::default_config();
        // A blocked destination makes the final replacement fail on every OS.
        fs::create_dir(&fixture.paths.config_file).unwrap();
        let state = DesktopState::default();
        {
            let mut inner = state.inner.lock().unwrap();
            inner.paths = Some(fixture.paths.clone());
            inner.config = Some(original);
        }

        assert!(super::mark_activated_and_start_api(&state).is_err());
        let info = super::runtime_info(&state).unwrap();
        assert!(!info.activated);
        assert!(!info.ready);
        assert!(state.inner.lock().unwrap().api_server.is_none());
        super::shutdown(&state);
    }

    #[test]
    #[ignore = "requires bundled MySQL 8 executables; uses only its own temporary database"]
    fn runtime_recovers_zeroed_files_and_interrupted_activation_with_existing_rows() {
        struct ShutdownOnDrop(DesktopState);
        impl Drop for ShutdownOnDrop {
            fn drop(&mut self) {
                super::shutdown(&self.0);
            }
        }

        let fixture = RuntimeFilesFixture::new();
        #[cfg(windows)]
        let executables = {
            let base = std::env::var_os("CLINIC_TEST_MYSQL_BASE")
                .map(PathBuf::from)
                .unwrap_or_else(|| {
                    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/runtime/mysql")
                });
            RuntimeExecutables {
                mysqld: base.join("bin/mysqld.exe"),
                mysql: base.join("bin/mysql.exe"),
                mysqldump: base.join("bin/mysqldump.exe"),
                mysqladmin: base.join("bin/mysqladmin.exe"),
                mysql_base: Some(base),
            }
        };
        #[cfg(not(windows))]
        let executables = RuntimeExecutables {
            mysqld: PathBuf::from("/usr/sbin/mysqld"),
            mysql: PathBuf::from("/usr/bin/mysql"),
            mysqldump: PathBuf::from("/usr/bin/mysqldump"),
            mysqladmin: PathBuf::from("/usr/bin/mysqladmin"),
            mysql_base: None,
        };
        let configure = |state: &DesktopState, config: InstallConfig| {
            let mut inner = state.inner.lock().unwrap();
            inner.paths = Some(fixture.paths.clone());
            inner.config = Some(config);
            inner.executables = Some(executables.clone());
        };
        let original = DesktopState::default();
        let _original_cleanup = ShutdownOnDrop(original.clone());
        let mut config = super::default_config();
        config.database = "clinic_rust_test_runtime_recovery".into();
        configure(&original, config);
        super::activate_new(
            &original,
            NewActivationRequest {
                clinic_name: "عيادة اختبار حماية التشغيل".into(),
                clinic_subtitle: None,
                admin_name: "مدير اختبار التشغيل".into(),
                admin_username: "runtime-test-admin".into(),
                admin_password: "Runtime-test-only-123!".into(),
            },
        )
        .unwrap();
        let context = super::context(&original).unwrap();
        let database = quote_identifier(&context.config.database);
        let query = |context: &RuntimeContext, sql: String| {
            super::mysql_output(
                context,
                &[
                    "--batch".into(),
                    "--skip-column-names".into(),
                    "-e".into(),
                    sql,
                ],
            )
            .unwrap()
            .stdout
        };
        query(&context, format!(
            "INSERT INTO {database}.patients (name, phone, gender, is_active, created_by, created_at, updated_at) \
             SELECT 'مريض اختبار الاسترداد', '0910000000', 'male', 1, id, NOW(), NOW() FROM {database}.users LIMIT 1"
        ));
        let verification_sql = format!(
            "SELECT id, username, password, role FROM {database}.users ORDER BY id; \
             SELECT id, name, phone, created_by FROM {database}.patients ORDER BY id; \
             SELECT id, clinic_name FROM {database}.settings ORDER BY id"
        );
        let expected = query(&context, verification_sql.clone());
        let credentials = context.config.clone();
        super::shutdown(&original);

        // Reproduce the two zero-filled files from the incident on test data only.
        fs::write(&fixture.paths.config_file, [0; 390]).unwrap();
        fs::write(&fixture.paths.root_client_file, [0; 162]).unwrap();
        let recovered = DesktopState::default();
        let _recovered_cleanup = ShutdownOnDrop(recovered.clone());
        let config = super::prepare_install_config(&recovered, &fixture.paths).unwrap();
        assert_eq!(config.root_password, credentials.root_password);
        assert_eq!(config.app_password, credentials.app_password);
        configure(&recovered, config);
        super::start_prepared_runtime(&recovered).unwrap();
        let info = runtime_info(&recovered).unwrap();
        assert!(info.ready && info.activated);
        assert!(info.recovery_notice.is_some());
        let context = super::context(&recovered).unwrap();
        assert_eq!(query(&context, verification_sql.clone()), expected);
        super::shutdown(&recovered);

        // Committed users with a lost activation flag must resume the existing install.
        let mut interrupted = context.config.clone();
        interrupted.activated = false;
        super::save_config(&fixture.paths.config_file, &interrupted).unwrap();
        let resumed = DesktopState::default();
        let _resumed_cleanup = ShutdownOnDrop(resumed.clone());
        let config = super::prepare_install_config(&resumed, &fixture.paths).unwrap();
        configure(&resumed, config);
        super::start_prepared_runtime(&resumed).unwrap();
        let info = runtime_info(&resumed).unwrap();
        assert!(info.ready && info.activated);
        let context = super::context(&resumed).unwrap();
        assert_eq!(query(&context, verification_sql), expected);

        // Missing users with other rows must present recovery instead of new setup.
        query(
            &context,
            format!("SET FOREIGN_KEY_CHECKS=0; DELETE FROM {database}.users"),
        );
        super::shutdown(&resumed);
        let mut incomplete = context.config.clone();
        incomplete.activated = false;
        super::save_config(&fixture.paths.config_file, &incomplete).unwrap();
        let blocked = DesktopState::default();
        let _blocked_cleanup = ShutdownOnDrop(blocked.clone());
        let config = super::prepare_install_config(&blocked, &fixture.paths).unwrap();
        configure(&blocked, config);
        let error = super::start_prepared_runtime(&blocked).unwrap_err();
        super::record_runtime_error(&blocked, &error);
        let info = runtime_info(&blocked).unwrap();
        assert!(info.recovery_required);
        assert!(!info.ready && !info.activated);
        assert!(blocked.inner.lock().unwrap().api_server.is_none());
        let context = super::context(&blocked).unwrap();
        assert_eq!(
            super::parse_mysql_count(&query(
                &context,
                format!("SELECT COUNT(*) FROM {database}.patients")
            ))
            .unwrap(),
            1
        );
    }

    #[test]
    fn incomplete_mysql_directory_is_rejected_before_launching_the_initializer() {
        let fixture = RuntimeFilesFixture::new();
        let data = fixture.paths.mysql_data_dir.join("ibdata1");
        fs::write(&data, b"preserve partial data").unwrap();
        let executables = RuntimeExecutables {
            mysqld: PathBuf::from("not-a-real-test-executable"),
            mysql: PathBuf::new(),
            mysqldump: PathBuf::new(),
            mysqladmin: PathBuf::new(),
            mysql_base: None,
        };
        assert!(super::initialize_mysql_data(&fixture.paths, &executables).is_err());
        assert_eq!(fs::read(data).unwrap(), b"preserve partial data");
    }

    #[test]
    fn failed_restore_marker_blocks_startup_across_process_restarts() {
        let fixture = RuntimeFilesFixture::new();
        fs::write(
            fixture.paths.runtime_dir.join("RECOVERY_REQUIRED"),
            b"failed restore",
        )
        .unwrap();
        let state = DesktopState::default();
        assert!(super::prepare_install_config(&state, &fixture.paths).is_err());
        assert!(state.inner.lock().unwrap().recovery_required);
        assert!(!fixture.paths.config_file.exists());
        super::shutdown(&state);
    }

    #[test]
    fn interrupted_replacement_persists_a_guard_before_a_new_process_starts() {
        let fixture = RuntimeFilesFixture::new();
        let state = DesktopState::default();
        state.inner.lock().unwrap().paths = Some(fixture.paths.clone());
        let work_dir = fixture.paths.runtime_dir.join("restore-work/test-owned");
        super::begin_database_replacement(&state, &work_dir).unwrap();
        assert!(state.inner.lock().unwrap().recovery_required);
        assert!(super::start_rust_api(&state).is_err());
        super::require_recovery(&state, "test interrupted restore");
        let marker =
            fs::read_to_string(fixture.paths.runtime_dir.join("RECOVERY_REQUIRED")).unwrap();
        assert!(marker.contains(&work_dir.display().to_string()));
        let restarted = DesktopState::default();
        assert!(super::prepare_install_config(&restarted, &fixture.paths).is_err());
        super::shutdown(&restarted);
        super::finish_database_replacement(&state).unwrap();
        assert!(!fixture.paths.runtime_dir.join("RECOVERY_REQUIRED").exists());
        assert!(!state.inner.lock().unwrap().recovery_required);
    }

    #[test]
    fn malformed_mysql_counts_never_imply_an_empty_database() {
        assert_eq!(super::parse_mysql_count(b"0\n").unwrap(), 0);
        for bytes in [b"".as_slice(), b"broken", b"0\n1", b"-1"] {
            assert!(super::parse_mysql_count(bytes).is_err());
        }
    }

    #[test]
    #[ignore = "requires an isolated MySQL 8 database"]
    fn activation_preserves_business_rows_when_the_users_table_is_empty() {
        let database = std::env::var("RUST_MYSQL_DATABASE").unwrap();
        assert!(database.starts_with("clinic_rust_test_"));
        let port = std::env::var("RUST_MYSQL_PORT").unwrap().parse().unwrap();
        let username = std::env::var("RUST_MYSQL_USER").unwrap();
        let password = std::env::var("RUST_MYSQL_PASSWORD").unwrap();
        run_async_operation(async move {
            let pool = api::db::connect_database("127.0.0.1", port, &database, &username, &password).await?;
            api::schema::reset(&pool).await?;
            let mut connection = pool.acquire().await.map_err(|error| error.to_string())?;
            sqlx::query("SET FOREIGN_KEY_CHECKS=0").execute(&mut *connection).await.map_err(|error| error.to_string())?;
            sqlx::query("INSERT INTO patients (name, phone, age, gender, created_by, is_active, created_at, updated_at) VALUES ('مريض محفوظ دون مستخدمين', '0910000000', 35, 'male', 999, 1, NOW(), NOW())")
                .execute(&mut *connection).await.map_err(|error| error.to_string())?;
            sqlx::query("SET FOREIGN_KEY_CHECKS=1").execute(&mut *connection).await.map_err(|error| error.to_string())?;
            drop(connection);
            let error = super::activate_new_database_attempt(&pool, "Test".into(), None, "Admin".into(), "test".into(), "unused-hash".into()).await.unwrap_err();
            assert!(error.contains("تحتوي على سجلات"));
            let patients = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM patients").fetch_one(&pool).await.map_err(|error| error.to_string())?;
            assert_eq!(patients, 1);
            pool.close().await;
            Ok(())
        }, "test activation guard timed out").unwrap();
    }

    #[test]
    fn runtime_errors_are_logged_without_local_credentials() {
        let fixture = RuntimeFilesFixture::new();
        let config = super::default_config();
        let state = DesktopState::default();
        {
            let mut inner = state.inner.lock().unwrap();
            inner.paths = Some(fixture.paths.clone());
            inner.config = Some(config.clone());
        }
        super::record_runtime_error(
            &state,
            &format!(
                "{} {} {}",
                config.root_password, config.app_password, config.app_key
            ),
        );
        let log = fs::read_to_string(fixture.root.join("logs/runtime.log")).unwrap();
        for secret in [&config.root_password, &config.app_password, &config.app_key] {
            assert!(!log.contains(secret));
        }
        assert!(log.contains("[محجوب]"));
    }

    #[test]
    fn sql_values_and_identifiers_are_escaped() {
        assert_eq!(escape_sql_string("a'b\\c"), "a''b\\\\c");
        assert_eq!(quote_identifier("a`b"), "`a``b`");
    }

    #[test]
    fn mysql_log_tail_returns_only_the_latest_lines() {
        let path = std::env::temp_dir().join(format!(
            "clinic-mysql-log-tail-{}.log",
            Uuid::new_v4().simple()
        ));
        let contents = (1..=15)
            .map(|line| format!("line {line}"))
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(&path, contents).expect("write MySQL test log");

        let tail = mysql_log_tail(&path, 12);

        assert_eq!(tail.lines().count(), 12);
        assert!(tail.starts_with("line 4\n"));
        assert!(tail.ends_with("line 15"));
        fs::remove_file(path).expect("remove MySQL test log");
    }

    #[test]
    #[ignore = "requires an isolated MySQL 8 database"]
    fn rust_activation_creates_real_admin_and_settings_once() {
        let database = std::env::var("RUST_MYSQL_DATABASE")
            .expect("RUST_MYSQL_DATABASE must name an isolated MySQL test database");
        assert!(
            database.starts_with("clinic_rust_test_"),
            "refusing to reset a database outside the clinic_rust_test_* namespace"
        );
        let mysql_port = std::env::var("RUST_MYSQL_PORT")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(3306);
        let app_user = std::env::var("RUST_MYSQL_USER").unwrap_or_else(|_| "root".into());
        let app_password = std::env::var("RUST_MYSQL_PASSWORD").unwrap_or_default();
        let context = RuntimeContext {
            config: InstallConfig {
                format_version: 1,
                mysql_port,
                api_port: 0,
                database: database.clone(),
                app_user: app_user.clone(),
                app_password: app_password.clone(),
                root_password: app_password.clone(),
                app_key: "test".into(),
                activated: false,
            },
            paths: RuntimePaths {
                resource_root: PathBuf::new(),
                runtime_dir: PathBuf::new(),
                mysql_data_dir: PathBuf::new(),
                mysql_log: PathBuf::new(),
                config_file: PathBuf::new(),
                root_client_file: PathBuf::new(),
            },
            executables: RuntimeExecutables {
                mysqld: PathBuf::new(),
                mysql: PathBuf::new(),
                mysqldump: PathBuf::new(),
                mysqladmin: PathBuf::new(),
                mysql_base: None,
            },
        };
        let host = std::env::var("RUST_MYSQL_HOST").unwrap_or_else(|_| "127.0.0.1".into());
        let reset_database = database.clone();
        let reset_user = app_user.clone();
        let reset_password = app_password.clone();
        run_async_operation(
            async move {
                let pool = api::db::connect_database(
                    &host,
                    mysql_port,
                    &reset_database,
                    &reset_user,
                    &reset_password,
                )
                .await?;
                api::schema::reset(&pool).await?;
                pool.close().await;
                Ok(())
            },
            "test reset timed out",
        )
        .expect("reset Rust activation test database");

        let request = NewActivationRequest {
            clinic_name: "عيادة الاختبار".into(),
            clinic_subtitle: Some("للعلاج الطبيعي".into()),
            admin_name: "مدير الاختبار".into(),
            admin_username: "admin-rust".into(),
            admin_password: "Clinic-test-123".into(),
        };
        activate_new_database(&context, request.clone()).expect("activate with Rust");

        let verify_database = database.clone();
        let verify_user = app_user.clone();
        let verify_password = app_password.clone();
        let host = std::env::var("RUST_MYSQL_HOST").unwrap_or_else(|_| "127.0.0.1".into());
        let (
            role,
            password_hash,
            clinic_name,
            settings_count,
            subscription,
            payment_amount,
            debt_amount,
        ) = run_async_operation(
            async move {
                let pool = api::db::connect_database(
                    &host,
                    mysql_port,
                    &verify_database,
                    &verify_user,
                    &verify_password,
                )
                .await?;
                let user = sqlx::query_as::<_, (u64, String, String)>(
                    "SELECT id, role, password FROM users WHERE username = 'admin-rust'",
                )
                .fetch_one(&pool)
                .await
                .map_err(|error| error.to_string())?;
                let setting = sqlx::query_as::<_, (String, i64)>(
                    "SELECT clinic_name, (SELECT COUNT(*) FROM settings) FROM settings LIMIT 1",
                )
                .fetch_one(&pool)
                .await
                .map_err(|error| error.to_string())?;
                let patient_id = sqlx::query(
                    "INSERT INTO patients \
                     (name, phone, age, gender, is_active, created_by, created_at, updated_at) \
                     VALUES ('مريض التهيئة', '0911111111', 30, 'male', 1, ?, NOW(), NOW())",
                )
                .bind(user.0)
                .execute(&pool)
                .await
                .map_err(|error| error.to_string())?
                .last_insert_id();
                let service_id = sqlx::query(
                    "INSERT INTO services \
                     (name, price, is_active, created_by, created_at, updated_at) \
                     VALUES ('خدمة التهيئة', 50, 1, ?, NOW(), NOW())",
                )
                .bind(user.0)
                .execute(&pool)
                .await
                .map_err(|error| error.to_string())?
                .last_insert_id();
                let mut transaction = pool.begin().await.map_err(|error| error.to_string())?;
                let created = api::services::subscriptions::create(
                    &mut transaction,
                    &api::services::subscriptions::CreateInput {
                        patient_id,
                        name: "اشتراك بعد التهيئة".into(),
                        service_ids: vec![service_id],
                        sessions_count: 6,
                        total_price: Decimal::from(300),
                        paid_amount: Decimal::from(100),
                        method: Some("cash".into()),
                    },
                    user.0,
                )
                .await
                .map_err(|error| format!("{}: {:?}", error.message, error.errors))?;
                transaction
                    .commit()
                    .await
                    .map_err(|error| error.to_string())?;
                let payment_amount = created.payment.expect("created payment").amount;
                let debt_amount = created.debt.expect("created debt").amount;
                pool.close().await;
                Ok((
                    user.1,
                    user.2,
                    setting.0,
                    setting.1,
                    created.subscription,
                    payment_amount,
                    debt_amount,
                ))
            },
            "test verification timed out",
        )
        .expect("read Rust activation rows");
        assert_eq!(role, "admin");
        assert!(verify("Clinic-test-123", &password_hash).expect("verify bcrypt hash"));
        assert_eq!(clinic_name, "عيادة الاختبار");
        assert_eq!(settings_count, 1);
        assert_eq!(subscription["name"], "اشتراك بعد التهيئة");
        assert_eq!(subscription["sessions_count"], 6);
        assert_eq!(subscription["total_price"], "300.000");
        assert_eq!(subscription["received_amount"], "100.000");
        assert_eq!(subscription["outstanding_amount"], "200.000");
        assert_eq!(payment_amount, Decimal::from(100));
        assert_eq!(debt_amount, Decimal::from(200));

        let error = activate_new_database(&context, request)
            .expect_err("second activation must preserve the existing database");
        assert!(error.contains("تحتوي على مستخدمين"));
    }

    #[test]
    #[ignore = "requires an isolated MySQL 8 database"]
    fn rust_api_server_starts_serves_cors_and_stops_without_php() {
        let database = std::env::var("RUST_MYSQL_DATABASE")
            .expect("RUST_MYSQL_DATABASE must name an isolated MySQL test database");
        assert!(database.starts_with("clinic_rust_test_"));
        let mysql_port = std::env::var("RUST_MYSQL_PORT")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(3306);
        let app_user = std::env::var("RUST_MYSQL_USER").unwrap_or_else(|_| "root".into());
        let app_password = std::env::var("RUST_MYSQL_PASSWORD").unwrap_or_default();
        let api_port = available_port().expect("reserve API port");
        let runtime_dir =
            std::env::temp_dir().join(format!("clinic-rust-api-test-{}", Uuid::new_v4().simple()));
        fs::create_dir_all(&runtime_dir).expect("create API test runtime directory");

        let state = DesktopState::default();
        {
            let mut inner = state.inner.lock().expect("lock desktop test state");
            inner.config = Some(InstallConfig {
                format_version: 1,
                mysql_port,
                api_port,
                database,
                app_user,
                app_password: app_password.clone(),
                root_password: app_password,
                app_key: "test".into(),
                activated: true,
            });
            inner.paths = Some(RuntimePaths {
                resource_root: PathBuf::new(),
                runtime_dir: runtime_dir.clone(),
                mysql_data_dir: runtime_dir.join("mysql-data"),
                mysql_log: runtime_dir.join("mysql.log"),
                config_file: runtime_dir.join("install.json"),
                root_client_file: runtime_dir.join("mysql-root.cnf"),
            });
            inner.executables = Some(RuntimeExecutables {
                mysqld: PathBuf::new(),
                mysql: PathBuf::new(),
                mysqldump: PathBuf::new(),
                mysqladmin: PathBuf::new(),
                mysql_base: None,
            });
        }

        start_rust_api(&state).expect("start embedded Rust API");
        assert!(health_request(api_port).expect("call Rust health route"));
        let info = runtime_info(&state).expect("read runtime info");
        assert!(info.ready);
        assert_eq!(
            info.api_base_url,
            format!("http://127.0.0.1:{api_port}/api/v1")
        );

        let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, api_port))
            .expect("connect to Rust API for CORS preflight");
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("set CORS response timeout");
        stream
            .write_all(
                format!(
                    "OPTIONS /api/v1/health HTTP/1.1\r\nHost: 127.0.0.1:{api_port}\r\nOrigin: http://tauri.localhost\r\nAccess-Control-Request-Method: GET\r\nConnection: close\r\n\r\n"
                )
                .as_bytes(),
            )
            .expect("send CORS preflight");
        let mut response = String::new();
        stream
            .read_to_string(&mut response)
            .expect("read CORS preflight");
        let response = response.to_ascii_lowercase();
        assert!(response.starts_with("http/1.1 200") || response.starts_with("http/1.1 204"));
        assert!(response.contains("access-control-allow-origin: *"));

        stop_rust_api(&state).expect("stop embedded Rust API");
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if TcpListener::bind((Ipv4Addr::LOCALHOST, api_port)).is_ok() {
                break;
            }
            assert!(Instant::now() < deadline, "Rust API port did not close");
            thread::sleep(Duration::from_millis(25));
        }
        fs::remove_dir_all(runtime_dir).expect("remove API test runtime directory");
    }
}
