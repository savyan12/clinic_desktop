use crate::drives::{ensure_path_is_on_drive, resolve_drive};
use crate::models::{
    BackupManifest, BackupRecord, BackupResult, DesktopState, RemovableDrive, RuntimeContext,
    RuntimeInfo,
};
use crate::runtime;
use chrono::{Local, SecondsFormat};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{BufReader, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Output, Stdio};
use uuid::Uuid;

const BACKUP_FORMAT_VERSION: u32 = 1;
const BACKUP_ROOT: &str = "ClinicBackups";
const DUMP_FILE: &str = "clinic-database.sql";
const MANIFEST_FILE: &str = "manifest.json";
const COMPLETE_FILE: &str = "COMPLETE.sha256";

pub fn create_backup(state: &DesktopState, drive_id: &str) -> Result<BackupResult, String> {
    let _operation = state
        .operation_lock
        .lock()
        .map_err(|_| "تعذر بدء عملية النسخ الاحتياطي.".to_string())?;
    let context = runtime::ensure_mysql_running(state)?;
    let (drive, drive_root) = resolve_drive(drive_id)?;
    create_backup_on_drive(&context, &drive, &drive_root)
}

fn create_backup_on_drive(
    context: &RuntimeContext,
    drive: &RemovableDrive,
    drive_root: &Path,
) -> Result<BackupResult, String> {
    let work_dir = context
        .paths
        .runtime_dir
        .join("backup-work")
        .join(Uuid::new_v4().simple().to_string());
    fs::create_dir_all(&work_dir)
        .map_err(|error| format!("تعذر إنشاء مساحة النسخة الاحتياطية: {error}"))?;
    let local_dump = work_dir.join(DUMP_FILE);
    let mut incomplete_folder = None;

    let result = (|| {
        dump_database(&context, &context.config.database, &local_dump)?;
        let dump_size = fs::metadata(&local_dump)
            .map_err(|error| format!("تعذر قراءة حجم النسخة الاحتياطية: {error}"))?
            .len();
        if dump_size == 0 {
            return Err("ملف النسخة الاحتياطية فارغ، ولم يتم حفظه.".to_string());
        }
        if let Some(free_bytes) = drive.free_bytes {
            let required = dump_size.saturating_mul(2).saturating_add(10 * 1024 * 1024);
            if free_bytes < required {
                return Err(
                    "المساحة المتاحة على وحدة USB لا تكفي لإنشاء نسخة آمنة والتحقق منها.".to_string(),
                );
            }
        }

        let local_checksum = sha256_file(&local_dump)?;
        verify_dump_can_restore(&context, &local_dump)?;

        let now = Local::now();
        let date_folder = drive_root
            .join(BACKUP_ROOT)
            .join(now.format("%Y-%m-%d").to_string());
        fs::create_dir_all(&date_folder)
            .map_err(|error| format!("تعذر إنشاء مجلد النسخة على USB: {error}"))?;
        let date_folder = ensure_path_is_on_drive(&date_folder, &drive_root)?;
        let final_folder = unique_backup_folder(&date_folder, &now.format("%H-%M-%S").to_string());
        let partial_folder = date_folder.join(format!(
            ".{}.partial-{}",
            now.format("%H-%M-%S"),
            Uuid::new_v4().simple()
        ));
        fs::create_dir(&partial_folder)
            .map_err(|error| format!("تعذر بدء النسخ إلى USB: {error}"))?;
        incomplete_folder = Some(partial_folder.clone());
        let usb_dump = partial_folder.join(DUMP_FILE);
        copy_and_sync(&local_dump, &usb_dump)?;
        let read_back_checksum = sha256_file(&usb_dump)?;
        if read_back_checksum != local_checksum {
            return Err("فشل التحقق من النسخة المقروءة من USB؛ لم تُعتمد النسخة.".to_string());
        }
        verify_dump_can_restore(&context, &usb_dump)?;

        let manifest = BackupManifest {
            format_version: BACKUP_FORMAT_VERSION,
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            created_at: now.to_rfc3339_opts(SecondsFormat::Secs, false),
            database: context.config.database.clone(),
            dump_file: DUMP_FILE.to_string(),
            dump_sha256: read_back_checksum.clone(),
            dump_size,
            verified_restore: true,
        };
        let manifest_path = partial_folder.join(MANIFEST_FILE);
        let manifest_bytes = serde_json::to_vec_pretty(&manifest)
            .map_err(|error| format!("تعذر تجهيز وصف النسخة الاحتياطية: {error}"))?;
        write_and_sync(&manifest_path, &manifest_bytes)?;
        let manifest_checksum = sha256_file(&manifest_path)?;
        write_and_sync(
            &partial_folder.join(COMPLETE_FILE),
            format!("{manifest_checksum}  {MANIFEST_FILE}\n").as_bytes(),
        )?;
        sync_directory(&partial_folder)?;
        validate_backup_folder(&partial_folder)?;
        fs::rename(&partial_folder, &final_folder)
            .map_err(|error| format!("تعذر اعتماد مجلد النسخة الاحتياطية: {error}"))?;
        sync_directory(&date_folder)?;
        incomplete_folder = None;

        Ok(BackupResult {
            folder: final_folder.to_string_lossy().to_string(),
            created_at: manifest.created_at,
            dump_size,
            checksum: read_back_checksum,
            verified: true,
        })
    })();

    let _ = fs::remove_dir_all(&work_dir);
    if let Some(folder) = incomplete_folder {
        let _ = fs::remove_dir_all(folder);
    }
    result
}

pub fn list_backups(drive_id: &str) -> Result<Vec<BackupRecord>, String> {
    let (_, drive_root) = resolve_drive(drive_id)?;
    list_backups_on_drive(&drive_root)
}

fn list_backups_on_drive(drive_root: &Path) -> Result<Vec<BackupRecord>, String> {
    let root = drive_root.join(BACKUP_ROOT);
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut records = Vec::new();
    for date_entry in
        fs::read_dir(&root).map_err(|error| format!("تعذر قراءة مجلد النسخ الاحتياطية: {error}"))?
    {
        let date_entry =
            date_entry.map_err(|error| format!("تعذر قراءة نسخة احتياطية: {error}"))?;
        if !date_entry
            .file_type()
            .map_err(|error| format!("تعذر فحص مجلد النسخة: {error}"))?
            .is_dir()
        {
            continue;
        }
        for time_entry in fs::read_dir(date_entry.path())
            .map_err(|error| format!("تعذر قراءة نسخ اليوم: {error}"))?
        {
            let time_entry =
                time_entry.map_err(|error| format!("تعذر قراءة نسخة احتياطية: {error}"))?;
            if !time_entry
                .file_type()
                .map_err(|error| format!("تعذر فحص النسخة: {error}"))?
                .is_dir()
                || time_entry.file_name().to_string_lossy().starts_with('.')
            {
                continue;
            }
            if let Ok(manifest) = validate_backup_folder(&time_entry.path()) {
                let id = format!(
                    "{}/{}",
                    date_entry.file_name().to_string_lossy(),
                    time_entry.file_name().to_string_lossy()
                );
                records.push(BackupRecord {
                    id,
                    folder: time_entry.path().to_string_lossy().to_string(),
                    created_at: manifest.created_at,
                    dump_size: manifest.dump_size,
                    checksum: manifest.dump_sha256,
                    verified: manifest.verified_restore,
                });
            }
        }
    }
    records.sort_by(|left, right| right.created_at.cmp(&left.created_at));
    Ok(records)
}

pub fn restore_backup(
    state: &DesktopState,
    drive_id: &str,
    backup_id: &str,
) -> Result<RuntimeInfo, String> {
    let _operation = state
        .operation_lock
        .lock()
        .map_err(|_| "تعذر بدء عملية استعادة النسخة الاحتياطية.".to_string())?;
    validate_backup_id(backup_id)?;
    let context = runtime::ensure_mysql_running(state)?;
    let (_, drive_root) = resolve_drive(drive_id)?;
    restore_backup_from_drive(state, &context, &drive_root, backup_id)
}

fn restore_backup_from_drive(
    state: &DesktopState,
    context: &RuntimeContext,
    drive_root: &Path,
    backup_id: &str,
) -> Result<RuntimeInfo, String> {
    let backup_folder = drive_root.join(BACKUP_ROOT).join(backup_id);
    let backup_folder = ensure_path_is_on_drive(&backup_folder, &drive_root)?;
    let manifest = validate_backup_folder(&backup_folder)?;
    if !manifest.verified_restore {
        return Err("هذه النسخة لم تُسجل كنسخة مجرّبة الاستعادة.".to_string());
    }
    let source_dump = backup_folder.join(&manifest.dump_file);

    let work_dir = context
        .paths
        .runtime_dir
        .join("restore-work")
        .join(Uuid::new_v4().simple().to_string());
    fs::create_dir_all(&work_dir).map_err(|error| format!("تعذر إنشاء مساحة الاستعادة: {error}"))?;
    let normalized_dump = work_dir.join("restored-and-migrated.sql");
    let safety_dump = work_dir.join("pre-restore-safety.sql");
    let staging_database = format!("clinic_restore_{}", Uuid::new_v4().simple());

    let result = (|| {
        create_database(&context, &staging_database)?;
        let staging_result = (|| {
            import_database(&context, &staging_database, &source_dump)?;
            runtime::migrate_database(&context, &staging_database, true)?;
            validate_database(&context, &staging_database)?;
            dump_database(&context, &staging_database, &normalized_dump)
        })();
        let _ = drop_database(&context, &staging_database);
        staging_result?;

        let had_existing_data = database_has_tables(&context, &context.config.database)?;
        if had_existing_data {
            dump_database(&context, &context.config.database, &safety_dump)?;
            verify_dump_can_restore(&context, &safety_dump)?;
        }

        runtime::stop_rust_api(state)?;
        let replacement = replace_database(&context, &normalized_dump).and_then(|_| {
            runtime::migrate_database(&context, &context.config.database, false)?;
            validate_database(&context, &context.config.database)
        });

        if let Err(error) = replacement {
            if had_existing_data {
                let rollback = replace_database(&context, &safety_dump);
                if let Err(rollback_error) = rollback {
                    return Err(format!(
                        "فشلت الاستعادة ({error}) وفشل الرجوع إلى النسخة الداخلية ({rollback_error}). احتفظ بمجلد النسخة ولا تشغّل النظام قبل الصيانة."
                    ));
                }
                let _ = runtime::start_rust_api(state);
                return Err(format!(
                    "فشلت الاستعادة وتمت إعادة قاعدة البيانات السابقة بأمان: {error}"
                ));
            }
            return Err(error);
        }

        runtime::mark_activated_and_start_api(state)
    })();

    let _ = drop_database(&context, &staging_database);
    let _ = fs::remove_dir_all(&work_dir);
    result
}

fn validate_backup_folder(folder: &Path) -> Result<BackupManifest, String> {
    let manifest_path = folder.join(MANIFEST_FILE);
    let complete_path = folder.join(COMPLETE_FILE);
    let manifest_bytes = fs::read(&manifest_path)
        .map_err(|_| "النسخة غير مكتملة: ملف الوصف غير موجود.".to_string())?;
    let manifest: BackupManifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| format!("ملف وصف النسخة غير صالح: {error}"))?;
    if manifest.format_version != BACKUP_FORMAT_VERSION || !manifest.verified_restore {
        return Err("إصدار النسخة غير مدعوم أو لم يكتمل التحقق منها.".to_string());
    }
    if version_is_newer(&manifest.app_version, env!("CARGO_PKG_VERSION"))? {
        return Err(
            "هذه النسخة أُنشئت بإصدار أحدث من التطبيق. حدّث التطبيق قبل الاستعادة.".to_string(),
        );
    }
    if Path::new(&manifest.dump_file)
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err("اسم ملف قاعدة البيانات داخل النسخة غير آمن.".to_string());
    }
    let complete = fs::read_to_string(&complete_path)
        .map_err(|_| "النسخة غير مكتملة: علامة الاكتمال غير موجودة.".to_string())?;
    let manifest_checksum = sha256_bytes(&manifest_bytes);
    if complete.trim() != format!("{manifest_checksum}  {MANIFEST_FILE}") {
        return Err("فشل التحقق من سلامة ملف وصف النسخة.".to_string());
    }
    let dump_path = folder.join(&manifest.dump_file);
    let metadata = fs::metadata(&dump_path)
        .map_err(|_| "ملف قاعدة البيانات غير موجود داخل النسخة.".to_string())?;
    if metadata.len() != manifest.dump_size {
        return Err("حجم ملف النسخة لا يطابق الوصف المحفوظ.".to_string());
    }
    if sha256_file(&dump_path)? != manifest.dump_sha256 {
        return Err("بصمة ملف قاعدة البيانات لا تطابق النسخة المحفوظة.".to_string());
    }
    Ok(manifest)
}

fn verify_dump_can_restore(
    context: &crate::models::RuntimeContext,
    dump_path: &Path,
) -> Result<(), String> {
    let database = format!("clinic_verify_{}", Uuid::new_v4().simple());
    create_database(context, &database)?;
    let result = import_database(context, &database, dump_path)
        .and_then(|_| validate_database(context, &database));
    let _ = drop_database(context, &database);
    result.map_err(|error| format!("فشل اختبار استعادة النسخة: {error}"))
}

fn dump_database(
    context: &crate::models::RuntimeContext,
    database: &str,
    destination: &Path,
) -> Result<(), String> {
    let file = File::create(destination)
        .map_err(|error| format!("تعذر إنشاء ملف النسخة الاحتياطية: {error}"))?;
    let output = runtime::mysqldump_command(context)
        .args([
            "--single-transaction",
            "--quick",
            "--routines",
            "--triggers",
            "--events",
            "--hex-blob",
            "--set-gtid-purged=OFF",
            "--no-tablespaces",
            "--default-character-set=utf8mb4",
            database,
        ])
        .stdout(Stdio::from(file))
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| format!("تعذر تشغيل mysqldump: {error}"))?;
    ensure_success(output, "فشل إنشاء نسخة قاعدة البيانات")?;
    sync_file(destination)
}

fn import_database(
    context: &crate::models::RuntimeContext,
    database: &str,
    source: &Path,
) -> Result<(), String> {
    let input =
        File::open(source).map_err(|error| format!("تعذر فتح ملف النسخة للاستعادة: {error}"))?;
    let output = runtime::mysql_command(context)
        .args(["--default-character-set=utf8mb4", "--database", database])
        .stdin(Stdio::from(input))
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| format!("تعذر تشغيل استعادة MySQL: {error}"))?;
    ensure_success(output, "فشل استيراد قاعدة البيانات")
}

fn replace_database(context: &crate::models::RuntimeContext, source: &Path) -> Result<(), String> {
    drop_database(context, &context.config.database)?;
    create_database(context, &context.config.database)?;
    import_database(context, &context.config.database, source)
}

fn create_database(context: &crate::models::RuntimeContext, database: &str) -> Result<(), String> {
    runtime::mysql_output(
        context,
        &[
            "-e".to_string(),
            format!(
                "CREATE DATABASE {} CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci",
                runtime::quote_identifier(database)
            ),
        ],
    )?;
    Ok(())
}

fn drop_database(context: &crate::models::RuntimeContext, database: &str) -> Result<(), String> {
    runtime::mysql_output(
        context,
        &[
            "-e".to_string(),
            format!(
                "DROP DATABASE IF EXISTS {}",
                runtime::quote_identifier(database)
            ),
        ],
    )?;
    Ok(())
}

fn validate_database(
    context: &crate::models::RuntimeContext,
    database: &str,
) -> Result<(), String> {
    let query = format!(
        "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = '{}' AND table_name IN ('migrations','users','settings')",
        database.replace('\'', "''")
    );
    let output = runtime::mysql_output(
        context,
        &[
            "--batch".to_string(),
            "--skip-column-names".to_string(),
            "-e".to_string(),
            query,
        ],
    )?;
    let count = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<u8>()
        .unwrap_or(0);
    if count < 2 {
        return Err("قاعدة البيانات المستعادة لا تحتوي على جداول النظام الأساسية.".to_string());
    }
    Ok(())
}

fn database_has_tables(
    context: &crate::models::RuntimeContext,
    database: &str,
) -> Result<bool, String> {
    let query = format!(
        "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = '{}'",
        database.replace('\'', "''")
    );
    let output = runtime::mysql_output(
        context,
        &[
            "--batch".to_string(),
            "--skip-column-names".to_string(),
            "-e".to_string(),
            query,
        ],
    )?;
    Ok(String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<u64>()
        .unwrap_or(0)
        > 0)
}

fn validate_backup_id(value: &str) -> Result<(), String> {
    let components = Path::new(value).components().collect::<Vec<_>>();
    if components.len() != 2
        || components
            .iter()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err("معرّف النسخة الاحتياطية غير صالح.".to_string());
    }
    Ok(())
}

fn unique_backup_folder(parent: &Path, base_name: &str) -> PathBuf {
    let direct = parent.join(base_name);
    if !direct.exists() {
        return direct;
    }
    for suffix in 2..=9999 {
        let candidate = parent.join(format!("{base_name}-{suffix}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    parent.join(format!("{base_name}-{}", Uuid::new_v4().simple()))
}

fn copy_and_sync(source: &Path, destination: &Path) -> Result<(), String> {
    let mut reader = BufReader::new(
        File::open(source).map_err(|error| format!("تعذر فتح ملف النسخة المحلية: {error}"))?,
    );
    let mut writer = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(destination)
        .map_err(|error| format!("تعذر إنشاء ملف النسخة على USB: {error}"))?;
    std::io::copy(&mut reader, &mut writer)
        .map_err(|error| format!("تعذر نسخ قاعدة البيانات إلى USB: {error}"))?;
    writer
        .flush()
        .and_then(|_| writer.sync_all())
        .map_err(|error| format!("تعذر تثبيت بيانات النسخة على USB: {error}"))
}

fn write_and_sync(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .map_err(|error| format!("تعذر إنشاء ملف {}: {error}", path.display()))?;
    file.write_all(bytes)
        .and_then(|_| file.flush())
        .and_then(|_| file.sync_all())
        .map_err(|error| format!("تعذر تثبيت ملف {}: {error}", path.display()))
}

fn sync_file(path: &Path) -> Result<(), String> {
    OpenOptions::new()
        .read(true)
        .open(path)
        .and_then(|file| file.sync_all())
        .map_err(|error| format!("تعذر تثبيت ملف النسخة: {error}"))
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> Result<(), String> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| format!("تعذر تثبيت مجلد النسخة على USB: {error}"))
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> Result<(), String> {
    Ok(())
}

fn version_is_newer(candidate: &str, current: &str) -> Result<bool, String> {
    fn parse(value: &str) -> Option<[u64; 3]> {
        let stable = value.split_once('-').map_or(value, |(version, _)| version);
        let parts = stable
            .split('.')
            .map(str::parse::<u64>)
            .collect::<Result<Vec<_>, _>>()
            .ok()?;

        (parts.len() == 3).then(|| [parts[0], parts[1], parts[2]])
    }

    let candidate = parse(candidate)
        .ok_or_else(|| "إصدار التطبيق داخل النسخة الاحتياطية غير صالح.".to_string())?;
    let current = parse(current).ok_or_else(|| "إصدار التطبيق الحالي غير صالح.".to_string())?;

    Ok(candidate > current)
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let mut reader = BufReader::new(
        File::open(path).map_err(|error| format!("تعذر فتح الملف للتحقق: {error}"))?,
    );
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| format!("تعذر قراءة الملف للتحقق: {error}"))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn ensure_success(output: Output, prefix: &str) -> Result<(), String> {
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    Err(if stderr.is_empty() {
        format!("{prefix}: {}", output.status)
    } else {
        format!("{prefix}: {stderr}")
    })
}

#[cfg(test)]
mod tests {
    use super::{
        create_backup_on_drive, list_backups_on_drive, restore_backup_from_drive,
        validate_backup_id, version_is_newer,
    };
    use crate::{
        models::{
            DesktopState, InstallConfig, NewActivationRequest, RemovableDrive, RuntimeExecutables,
            RuntimePaths,
        },
        runtime,
    };
    use std::{fs, net::TcpListener, path::PathBuf};
    use uuid::Uuid;

    struct RuntimeCleanup {
        state: DesktopState,
        root: PathBuf,
    }

    impl Drop for RuntimeCleanup {
        fn drop(&mut self) {
            runtime::shutdown(&self.state);
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn backup_identifier_requires_exact_date_and_time_folders() {
        assert!(validate_backup_id("2026-11-01/14-30-00").is_ok());
        assert!(validate_backup_id("../secret").is_err());
        assert!(validate_backup_id("2026-11-01/../../secret").is_err());
        assert!(validate_backup_id("one").is_err());
    }

    #[test]
    fn newer_backup_versions_are_rejected() {
        assert!(version_is_newer("1.1.0", "1.0.9").unwrap());
        assert!(!version_is_newer("1.0.0", "1.0.0").unwrap());
        assert!(!version_is_newer("0.9.9", "1.0.0").unwrap());
        assert!(version_is_newer("future", "1.0.0").is_err());
    }

    #[test]
    #[ignore = "requires local MySQL 8 server and client executables"]
    fn backup_round_trip_restores_data_and_restarts_rust_api() {
        let root = std::env::temp_dir().join(format!(
            "clinic-backup-round-trip-{}",
            Uuid::new_v4().simple()
        ));
        let runtime_dir = root.join("runtime");
        let mysql_data_dir = root.join("mysql-data");
        let logs_dir = root.join("logs");
        let drive_root = root.join("usb");
        for directory in [&runtime_dir, &mysql_data_dir, &logs_dir, &drive_root] {
            fs::create_dir_all(directory).expect("create backup integration directory");
        }

        let mysql_port = free_port();
        let mut api_port = free_port();
        while api_port == mysql_port {
            api_port = free_port();
        }
        let state = DesktopState::default();
        {
            let mut inner = state.inner.lock().expect("lock backup test state");
            inner.config = Some(InstallConfig {
                format_version: 1,
                mysql_port,
                api_port,
                database: "clinic_backup_test".into(),
                app_user: "clinic_backup".into(),
                app_password: "AppBackup123!".into(),
                root_password: "RootBackup123!".into(),
                app_key: "test".into(),
                activated: false,
            });
            inner.paths = Some(RuntimePaths {
                resource_root: PathBuf::new(),
                runtime_dir: runtime_dir.clone(),
                mysql_data_dir,
                mysql_log: logs_dir.join("mysql.log"),
                config_file: runtime_dir.join("install.json"),
                root_client_file: runtime_dir.join("mysql-root.cnf"),
            });
            inner.executables = Some(RuntimeExecutables {
                mysqld: PathBuf::from("/usr/sbin/mysqld"),
                mysql: PathBuf::from("/usr/bin/mysql"),
                mysqldump: PathBuf::from("/usr/bin/mysqldump"),
                mysqladmin: PathBuf::from("/usr/bin/mysqladmin"),
                mysql_base: None,
            });
        }
        let _cleanup = RuntimeCleanup {
            state: state.clone(),
            root,
        };

        runtime::activate_new(
            &state,
            NewActivationRequest {
                clinic_name: "عيادة النسخ الأصلية".into(),
                clinic_subtitle: Some("اختبار الاستعادة".into()),
                admin_name: "مدير النسخ".into(),
                admin_username: "admin".into(),
                admin_password: "ClinicBackup123!".into(),
            },
        )
        .expect("activate isolated backup runtime");
        let context = runtime::context(&state).expect("read backup runtime context");
        let database = runtime::quote_identifier(&context.config.database);
        runtime::mysql_output(
            &context,
            &[
                "--default-character-set=utf8mb4".into(),
                "-e".into(),
                format!(
                    "INSERT INTO {database}.patients \
                     (name, phone, age, gender, is_active, created_by, created_at, updated_at) \
                     SELECT 'المريض الأصلي', '0911111111', 35, 'male', 1, id, NOW(), NOW() \
                     FROM {database}.users ORDER BY id LIMIT 1"
                ),
            ],
        )
        .expect("insert source data before backup");

        let drive = RemovableDrive {
            id: "test-usb".into(),
            label: "Test USB".into(),
            path: drive_root.to_string_lossy().to_string(),
            free_bytes: None,
        };
        let backup = create_backup_on_drive(&context, &drive, &drive_root)
            .expect("create and verify real backup");
        assert!(backup.verified);
        assert!(backup.dump_size > 0);
        let backups = list_backups_on_drive(&drive_root).expect("list verified backup");
        assert_eq!(backups.len(), 1);
        assert_eq!(backups[0].checksum, backup.checksum);

        runtime::mysql_output(
            &context,
            &[
                "--default-character-set=utf8mb4".into(),
                "-e".into(),
                format!(
                    "UPDATE {database}.patients SET name = 'المريض المعدل'; \
                     UPDATE {database}.settings SET clinic_name = 'عيادة معدلة بعد النسخ'"
                ),
            ],
        )
        .expect("mutate data after backup");

        let info = restore_backup_from_drive(&state, &context, &drive_root, &backups[0].id)
            .expect("restore verified backup");
        assert!(info.ready);
        assert!(info.activated);
        let restored_context = runtime::context(&state).expect("read restored runtime context");
        let output = runtime::mysql_output(
            &restored_context,
            &[
                "--batch".into(),
                "--skip-column-names".into(),
                "--default-character-set=utf8mb4".into(),
                "-e".into(),
                format!(
                    "SELECT CONCAT(p.name, '|', s.clinic_name) \
                     FROM {database}.patients p CROSS JOIN {database}.settings s LIMIT 1"
                ),
            ],
        )
        .expect("read restored data");
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            "المريض الأصلي|عيادة النسخ الأصلية"
        );
    }

    fn free_port() -> u16 {
        TcpListener::bind(("127.0.0.1", 0))
            .expect("reserve test port")
            .local_addr()
            .expect("read test port")
            .port()
    }
}
