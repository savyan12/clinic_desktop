use crate::models::InstallConfig;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Serialize, Deserialize)]
struct ConfigSnapshot {
    checksum: String,
    config: InstallConfig,
}

pub struct LoadedConfig {
    pub config: InstallConfig,
    pub recovered: bool,
}

fn snapshot_path(path: &Path) -> PathBuf {
    path.with_extension("json.backup")
}

pub fn validate_config(config: &InstallConfig) -> Result<(), String> {
    if config.format_version != 1 {
        return Err("إصدار إعدادات التشغيل المحلي غير مدعوم.".into());
    }
    if config.mysql_port == 0 || config.api_port == 0 || config.mysql_port == config.api_port {
        return Err("منافذ التشغيل المحلي غير صالحة.".into());
    }
    for value in [&config.database, &config.app_user] {
        if value.is_empty()
            || value.len() > 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        {
            return Err("اسم قاعدة البيانات أو حساب التشغيل المحلي غير صالح.".into());
        }
    }
    for value in [&config.root_password, &config.app_password] {
        if value.is_empty() || value.contains(['\0', '\r', '\n']) {
            return Err("بيانات اتصال التشغيل المحلي غير صالحة.".into());
        }
    }
    let key = config
        .app_key
        .strip_prefix("base64:")
        .and_then(|value| BASE64.decode(value).ok());
    if !key.is_some_and(|bytes| bytes.len() == 32) {
        return Err("مفتاح إعدادات التشغيل المحلي غير صالح.".into());
    }
    Ok(())
}

fn config_checksum(config: &InstallConfig) -> Result<String, String> {
    let bytes =
        serde_json::to_vec(config).map_err(|_| "تعذر التحقق من إعدادات التشغيل.".to_string())?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn decode_config(bytes: &[u8]) -> Result<InstallConfig, String> {
    let config = serde_json::from_slice(bytes)
        .map_err(|_| "ملف إعدادات التشغيل المحلي غير صالح.".to_string())?;
    validate_config(&config)?;
    Ok(config)
}

pub fn load_config(path: &Path) -> Result<Option<LoadedConfig>, String> {
    let primary = fs::read(path);
    if let Ok(bytes) = &primary {
        if let Ok(config) = decode_config(bytes) {
            // Establish or repair the protected copy before using these credentials.
            save_config(path, &config)?;
            return Ok(Some(LoadedConfig {
                config,
                recovered: false,
            }));
        }
    }
    let backup_path = snapshot_path(path);
    let backup = fs::read(&backup_path);
    if primary
        .as_ref()
        .is_err_and(|error| error.kind() == io::ErrorKind::NotFound)
        && backup
            .as_ref()
            .is_err_and(|error| error.kind() == io::ErrorKind::NotFound)
    {
        return Ok(None);
    }
    let recovered = (|| {
        let bytes = backup.map_err(|_| ())?;
        let snapshot: ConfigSnapshot = serde_json::from_slice(&bytes).map_err(|_| ())?;
        validate_config(&snapshot.config).map_err(|_| ())?;
        if config_checksum(&snapshot.config).map_err(|_| ())? != snapshot.checksum {
            return Err(());
        }
        Ok(snapshot.config)
    })()
    .map_err(|()| "تعذر قراءة إعدادات التشغيل ولم توجد نسخة إعدادات سليمة. بيانات العيادة محفوظة؛ يلزم استرداد إعدادات التشغيل بواسطة المسؤول قبل فتح النظام.".to_string())?;
    if let Ok(bytes) = primary {
        let archive = path.with_extension(format!("json.corrupt-{}", Uuid::new_v4().simple()));
        atomic_write(&archive, &bytes)?;
    }
    save_config(path, &recovered)?;
    Ok(Some(LoadedConfig {
        config: recovered,
        recovered: true,
    }))
}

pub fn save_config(path: &Path, config: &InstallConfig) -> Result<(), String> {
    validate_config(config)?;
    let snapshot = ConfigSnapshot {
        checksum: config_checksum(config)?,
        config: config.clone(),
    };
    let protected = serde_json::to_vec_pretty(&snapshot)
        .map_err(|_| "تعذر تجهيز نسخة إعدادات التشغيل.".to_string())?;
    let primary = serde_json::to_vec_pretty(config)
        .map_err(|_| "تعذر تجهيز إعدادات التشغيل المحلي.".to_string())?;
    // A durable, checked candidate remains available if replacing the primary fails.
    atomic_write(&snapshot_path(path), &protected)?;
    atomic_write(path, &primary)
}

pub fn acquire_instance_lock(runtime_dir: &Path) -> Result<File, String> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(runtime_dir.join("instance.lock"))
        .map_err(|error| format!("تعذر حجز مجلد تشغيل النظام: {error}"))?;
    file.try_lock().map_err(|_| {
        "نسخة أخرى من النظام تستخدم مجلد البيانات نفسه. أغلق النسخة الأخرى ثم أعد المحاولة."
            .to_string()
    })?;
    Ok(file)
}

pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if fs::read(path).is_ok_and(|existing| existing == bytes) {
        return OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .and_then(|file| file.sync_all())
            .map_err(|error| format!("تعذر تثبيت ملف التشغيل {}: {error}", path.display()));
    }
    let temporary = path.with_extension(format!("tmp-{}", Uuid::new_v4().simple()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(fs::Permissions::from_mode(0o600))?;
        }
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        replace_path(&temporary, path)?;
        sync_parent(path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result.map_err(|error: io::Error| {
        format!("تعذر حفظ ملف التشغيل {} بأمان: {error}", path.display())
    })
}

#[cfg(windows)]
pub fn replace_path(source: &Path, destination: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };
    let source = source
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let destination = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    // Both paths are on the same volume; never expose a truncated destination.
    let result = unsafe {
        MoveFileExW(
            source.as_ptr(),
            destination.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if result == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
pub fn replace_path(source: &Path, destination: &Path) -> io::Result<()> {
    fs::rename(source, destination)
}

fn sync_parent(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    if let Some(parent) = path.parent() {
        File::open(parent)?.sync_all()?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestDir(PathBuf);
    impl TestDir {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "clinic-runtime-files-test-{}",
                Uuid::new_v4().simple()
            ));
            fs::create_dir(&root).unwrap();
            Self(root)
        }
        fn config_path(&self) -> PathBuf {
            self.0.join("install.json")
        }
    }
    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn config() -> InstallConfig {
        InstallConfig {
            format_version: 1,
            mysql_port: 33123,
            api_port: 18123,
            database: "clinic_system".into(),
            app_user: "clinic_app".into(),
            root_password: "Root-test-only!".into(),
            app_password: "App-test-only!".into(),
            app_key: format!("base64:{}", BASE64.encode([42_u8; 32])),
            activated: true,
        }
    }

    #[test]
    fn zeroed_and_truncated_primary_recovers_the_same_credentials() {
        for bytes in [vec![0; 390], b"{\"format_version\":1,".to_vec()] {
            let dir = TestDir::new();
            let original = config();
            save_config(&dir.config_path(), &original).unwrap();
            fs::write(dir.config_path(), &bytes).unwrap();
            let loaded = load_config(&dir.config_path()).unwrap().unwrap();
            assert!(loaded.recovered);
            assert_eq!(loaded.config.root_password, original.root_password);
            assert_eq!(loaded.config.app_password, original.app_password);
            assert!(loaded.config.activated);
            assert!(fs::read_dir(&dir.0).unwrap().any(|entry| entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .contains("corrupt-")));
            assert!(!load_config(&dir.config_path()).unwrap().unwrap().recovered);
        }
    }

    #[test]
    fn missing_primary_recovers_from_the_checked_snapshot() {
        let dir = TestDir::new();
        save_config(&dir.config_path(), &config()).unwrap();
        fs::remove_file(dir.config_path()).unwrap();
        assert!(load_config(&dir.config_path()).unwrap().unwrap().recovered);
    }

    #[test]
    fn corrupt_copies_are_not_replaced_with_new_credentials() {
        let dir = TestDir::new();
        fs::write(dir.config_path(), [0; 390]).unwrap();
        fs::write(snapshot_path(&dir.config_path()), [0; 162]).unwrap();
        assert!(load_config(&dir.config_path()).is_err());
        assert_eq!(fs::read(dir.config_path()).unwrap(), vec![0; 390]);
    }

    #[test]
    fn snapshot_checksum_rejects_changed_credentials() {
        let dir = TestDir::new();
        save_config(&dir.config_path(), &config()).unwrap();
        let backup = snapshot_path(&dir.config_path());
        let mut snapshot: ConfigSnapshot =
            serde_json::from_slice(&fs::read(&backup).unwrap()).unwrap();
        snapshot.config.root_password = "Different-test-only!".into();
        fs::write(&backup, serde_json::to_vec(&snapshot).unwrap()).unwrap();
        fs::write(dir.config_path(), []).unwrap();
        assert!(load_config(&dir.config_path()).is_err());
    }

    #[test]
    fn invalid_ports_and_key_are_rejected_before_saving() {
        let dir = TestDir::new();
        let mut candidate = config();
        candidate.mysql_port = candidate.api_port;
        assert!(save_config(&dir.config_path(), &candidate).is_err());
        candidate = config();
        candidate.app_key = "broken".into();
        assert!(save_config(&dir.config_path(), &candidate).is_err());
        assert!(!dir.config_path().exists());
    }

    #[test]
    fn unchanged_credentials_do_not_rewrite_the_files() {
        let dir = TestDir::new();
        save_config(&dir.config_path(), &config()).unwrap();
        let modified = fs::metadata(dir.config_path()).unwrap().modified().unwrap();
        save_config(&dir.config_path(), &config()).unwrap();
        assert_eq!(
            fs::metadata(dir.config_path()).unwrap().modified().unwrap(),
            modified
        );
    }

    #[test]
    fn failed_replacement_preserves_the_destination_and_cleans_its_scratch_file() {
        let dir = TestDir::new();
        let destination = dir.0.join("existing-directory");
        fs::create_dir(&destination).unwrap();
        fs::write(destination.join("keep.txt"), "preserve").unwrap();
        assert!(atomic_write(&destination, b"replacement").is_err());
        assert_eq!(fs::read(destination.join("keep.txt")).unwrap(), b"preserve");
        assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 1);
    }

    #[test]
    fn instance_lock_blocks_a_second_owner_and_releases_on_close() {
        let dir = TestDir::new();
        let owner = acquire_instance_lock(&dir.0).unwrap();
        assert!(acquire_instance_lock(&dir.0).is_err());
        drop(owner);
        assert!(acquire_instance_lock(&dir.0).is_ok());
    }

    #[test]
    fn instance_lock_is_released_after_the_owning_process_is_terminated() {
        use std::process::{Child, Command, Stdio};
        use std::thread;
        use std::time::{Duration, Instant};

        struct OwnedChild(Child);
        impl Drop for OwnedChild {
            fn drop(&mut self) {
                if self.0.try_wait().ok().flatten().is_none() {
                    crate::native_process::terminate_owned_child(&mut self.0);
                }
            }
        }

        let dir = TestDir::new();
        let mut command = Command::new(std::env::current_exe().unwrap());
        crate::runtime::hide_console_window(&mut command);
        let mut child = OwnedChild(
            command
                .args([
                    "--ignored",
                    "--exact",
                    "runtime_files::tests::child_instance_lock_helper",
                ])
                .env("CLINIC_TEST_INSTANCE_LOCK_DIR", &dir.0)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap(),
        );
        let deadline = Instant::now() + Duration::from_secs(5);
        while !dir.0.join("lock-ready").exists() {
            assert!(
                child.0.try_wait().unwrap().is_none(),
                "lock helper exited before acquiring the lock"
            );
            assert!(
                Instant::now() < deadline,
                "lock helper did not become ready"
            );
            thread::sleep(Duration::from_millis(20));
        }
        assert!(acquire_instance_lock(&dir.0).is_err());
        crate::native_process::terminate_owned_child(&mut child.0);
        assert!(acquire_instance_lock(&dir.0).is_ok());
    }

    #[test]
    #[ignore = "subprocess fixture; invoked only by the instance-lock regression test"]
    fn child_instance_lock_helper() {
        let directory = PathBuf::from(std::env::var_os("CLINIC_TEST_INSTANCE_LOCK_DIR").unwrap());
        let _owner = acquire_instance_lock(&directory).unwrap();
        atomic_write(&directory.join("lock-ready"), b"ready").unwrap();
        std::thread::sleep(std::time::Duration::from_secs(30));
    }
}
