use crate::{
    api::{self, config::ApiConfig, AppState},
    models::{
        ApiServerProcess, DesktopState, InstallConfig, NewActivationRequest, RuntimeContext,
        RuntimeExecutables, RuntimeInfo, RuntimePaths,
    },
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use bcrypt::{hash, DEFAULT_COST};
use sha2::{Digest, Sha256};
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

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

pub(crate) fn hide_console_window(command: &mut Command) {
    #[cfg(target_os = "windows")]
    command.creation_flags(CREATE_NO_WINDOW);

    #[cfg(not(target_os = "windows"))]
    let _ = command;
}

pub fn initialize(app: &AppHandle, state: &DesktopState) -> Result<(), String> {
    let paths = resolve_paths(app)?;
    let config = match load_config(&paths.config_file)? {
        Some(config) => config,
        None => {
            let config = default_config();
            save_config(&paths.config_file, &config)?;
            config
        }
    };

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

    if config.activated {
        if let Err(error) = start_existing(state) {
            record_runtime_error(state, &error);
            return Err(error);
        }
    }

    Ok(())
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
        let config = inner.config.clone().unwrap_or_else(default_config);
        (inner.ready, config.api_port)
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
    let config = inner.config.clone().unwrap_or_else(default_config);

    Ok(RuntimeInfo {
        desktop: true,
        ready: inner.ready,
        activated: inner.config.as_ref().is_some_and(|value| value.activated),
        api_base_url: format!("http://127.0.0.1:{}/api/v1", config.api_port),
        mysql_port: config.mysql_port,
        api_port: config.api_port,
        error: inner.last_error.clone(),
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
    let mut config = inner.config.clone().unwrap_or_else(default_config);

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

    save_config(&paths.config_file, &config)?;
    write_mysql_client_file(&paths.root_client_file, &config)?;

    let mut context = RuntimeContext {
        config: config.clone(),
        paths: paths.clone(),
        executables: executables.clone(),
    };

    if port_is_in_use(config.mysql_port) {
        if mysql_credentials_work(&context) {
            inner.config = Some(config);
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
    if initialized {
        write_mysql_bootstrap_file(&bootstrap_file, &config)?;
    }

    let child = spawn_mysql(
        &paths,
        &executables,
        &config,
        bootstrap_file.exists().then_some(bootstrap_file.as_path()),
    )?;
    inner.mysql_child = Some(child);
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
            let pool = api::db::connect(&config).await?;
            let users_table_exists = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM information_schema.tables \
                 WHERE table_schema = DATABASE() AND table_name = 'users'",
            )
            .fetch_one(&pool)
            .await
            .map_err(|error| format!("تعذر فحص قاعدة بيانات التفعيل: {error}"))?
                > 0;
            if users_table_exists {
                let users_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM users")
                    .fetch_one(&pool)
                    .await
                    .map_err(|error| format!("تعذر فحص مستخدمي النظام: {error}"))?;
                if users_count > 0 {
                    return Err(
                        "قاعدة البيانات المحلية تحتوي على مستخدمين ولا يمكن تهيئتها كتثبيت جديد."
                            .to_string(),
                    );
                }
            }

            api::schema::reset(&pool).await?;
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
            pool.close().await;
            Ok(())
        },
        "انتهت مهلة تهيئة قاعدة بيانات العيادة.",
    )
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
    tauri::async_runtime::spawn(async move {
        let _ = sender.send(future.await);
    });
    receiver
        .recv_timeout(STARTUP_TIMEOUT)
        .map_err(|_| timeout_message.to_string())?
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
        let config = inner
            .config
            .as_mut()
            .ok_or_else(|| "إعدادات التشغيل المحلي غير موجودة.".to_string())?;
        config.activated = true;
        save_config(&paths.config_file, config)?;
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

    match startup_receiver.recv_timeout(STARTUP_TIMEOUT) {
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
        }
    }

    Ok(())
}

fn stop_mysql_child(state: &DesktopState) {
    if let Ok(mut inner) = state.inner.lock() {
        if let Some(mut child) = inner.mysql_child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

pub fn shutdown(state: &DesktopState) {
    let _operation = state.operation_lock.lock().ok();
    let context = context(state).ok();
    let _ = stop_rust_api(state);

    if let Some(context) = context {
        let mut command = Command::new(&context.executables.mysqladmin);
        hide_console_window(&mut command);
        command
            .arg(format!(
                "--defaults-extra-file={}",
                context.paths.root_client_file.display()
            ))
            .arg("shutdown");
        let _ = command.output();
    }

    if let Ok(mut inner) = state.inner.lock() {
        if let Some(mut child) = inner.mysql_child.take() {
            let deadline = Instant::now() + Duration::from_secs(8);
            while Instant::now() < deadline {
                if child.try_wait().ok().flatten().is_some() {
                    return;
                }
                thread::sleep(Duration::from_millis(100));
            }
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

pub fn mysql_output(context: &RuntimeContext, args: &[String]) -> Result<Output, String> {
    let output = mysql_command(context)
        .args(args)
        .output()
        .map_err(|error| format!("تعذر الاتصال بقاعدة البيانات المحلية: {error}"))?;

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
        "--defaults-extra-file={}",
        context.paths.root_client_file.display()
    ));
    command
}

pub fn mysqldump_command(context: &RuntimeContext) -> Command {
    let mut command = Command::new(&context.executables.mysqldump);
    hide_console_window(&mut command);
    command.arg(format!(
        "--defaults-extra-file={}",
        context.paths.root_client_file.display()
    ));
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

fn load_config(path: &Path) -> Result<Option<InstallConfig>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let bytes =
        fs::read(path).map_err(|error| format!("تعذر قراءة إعدادات التشغيل المحلي: {error}"))?;
    let config: InstallConfig = serde_json::from_slice(&bytes)
        .map_err(|error| format!("ملف إعدادات التشغيل المحلي غير صالح: {error}"))?;
    if config.format_version != CONFIG_FORMAT_VERSION {
        return Err("إصدار إعدادات التشغيل المحلي غير مدعوم.".to_string());
    }
    Ok(Some(config))
}

fn save_config(path: &Path, config: &InstallConfig) -> Result<(), String> {
    let temporary = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(config)
        .map_err(|error| format!("تعذر تجهيز إعدادات التشغيل المحلي: {error}"))?;
    fs::write(&temporary, bytes)
        .map_err(|error| format!("تعذر حفظ إعدادات التشغيل المحلي: {error}"))?;
    set_private_permissions(&temporary)?;
    fs::rename(&temporary, path)
        .map_err(|error| format!("تعذر تثبيت إعدادات التشغيل المحلي: {error}"))?;
    set_private_permissions(path)
}

fn write_mysql_client_file(path: &Path, config: &InstallConfig) -> Result<(), String> {
    let contents = format!(
        "[client]\nuser=root\npassword={}\nhost=127.0.0.1\nport={}\nprotocol=tcp\ndefault-character-set=utf8mb4\n",
        config.root_password, config.mysql_port
    );
    fs::write(path, contents).map_err(|error| format!("تعذر حفظ اتصال قاعدة البيانات: {error}"))?;
    set_private_permissions(path)
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
    fs::write(path, sql).map_err(|error| format!("تعذر تجهيز حساب MySQL المحلي: {error}"))?;
    set_private_permissions(path)
}

fn mysql_credentials_work(context: &RuntimeContext) -> bool {
    mysql_command(context)
        .args(["--batch", "--skip-column-names", "-e", "SELECT 1"])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn initialize_mysql_data(
    paths: &RuntimePaths,
    executables: &RuntimeExecutables,
) -> Result<bool, String> {
    if paths.mysql_data_dir.join("mysql").exists() {
        return Ok(false);
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
    let output = command
        .output()
        .map_err(|error| format!("تعذر تهيئة MySQL المحلي: {error}"))?;
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
        let output = command
            .args([
                "--protocol=tcp",
                &format!("--host={host}"),
                &format!("--port={}", context.config.mysql_port),
                "--user=root",
                "--skip-password",
                "-e",
                &sql,
            ])
            .output()
            .map_err(|error| format!("تعذر تأمين حساب MySQL المحلي: {error}"))?;
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
    let deadline = Instant::now() + STARTUP_TIMEOUT;
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
    child.try_wait().ok().flatten().is_none()
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

pub fn record_runtime_error(state: &DesktopState, error: &str) {
    if let Ok(mut inner) = state.inner.lock() {
        inner.ready = false;
        inner.last_error = Some(error.to_string());
    }
}

#[cfg(unix)]
fn set_private_permissions(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let permissions = fs::Permissions::from_mode(0o600);
    fs::set_permissions(path, permissions)
        .map_err(|error| format!("تعذر حماية ملف التشغيل المحلي: {error}"))
}

#[cfg(not(unix))]
fn set_private_permissions(_path: &Path) -> Result<(), String> {
    Ok(())
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
