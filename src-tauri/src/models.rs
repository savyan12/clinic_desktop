use serde::{Deserialize, Serialize};
use std::fs::File;
use std::path::PathBuf;
use std::process::Child;
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot;

pub struct ApiServerProcess {
    pub shutdown: oneshot::Sender<()>,
    pub task: tauri::async_runtime::JoinHandle<()>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallConfig {
    pub format_version: u32,
    pub mysql_port: u16,
    pub api_port: u16,
    pub database: String,
    pub app_user: String,
    pub app_password: String,
    pub root_password: String,
    pub app_key: String,
    pub activated: bool,
}

#[derive(Debug, Clone)]
pub struct RuntimePaths {
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    pub resource_root: PathBuf,
    pub runtime_dir: PathBuf,
    pub mysql_data_dir: PathBuf,
    pub mysql_log: PathBuf,
    pub config_file: PathBuf,
    pub root_client_file: PathBuf,
}

#[derive(Debug, Clone)]
pub struct RuntimeExecutables {
    pub mysqld: PathBuf,
    pub mysql: PathBuf,
    pub mysqldump: PathBuf,
    pub mysqladmin: PathBuf,
    pub mysql_base: Option<PathBuf>,
}

#[derive(Default)]
pub struct RuntimeProcessState {
    pub config: Option<InstallConfig>,
    pub paths: Option<RuntimePaths>,
    pub executables: Option<RuntimeExecutables>,
    pub mysql_child: Option<Child>,
    pub api_server: Option<ApiServerProcess>,
    pub ready: bool,
    pub last_error: Option<String>,
    pub instance_lock: Option<File>,
    pub recovery_required: bool,
    pub recovery_notice: Option<String>,
    pub mysql_managed: bool,
}

#[derive(Clone, Default)]
pub struct DesktopState {
    pub inner: Arc<Mutex<RuntimeProcessState>>,
    pub operation_lock: Arc<Mutex<()>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeInfo {
    pub desktop: bool,
    pub ready: bool,
    pub activated: bool,
    pub api_base_url: String,
    pub mysql_port: u16,
    pub api_port: u16,
    pub error: Option<String>,
    pub recovery_required: bool,
    pub recovery_notice: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NewActivationRequest {
    pub clinic_name: String,
    pub clinic_subtitle: Option<String>,
    pub admin_name: String,
    pub admin_username: String,
    pub admin_password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemovableDrive {
    pub id: String,
    pub label: String,
    pub path: String,
    pub free_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupManifest {
    pub format_version: u32,
    pub app_version: String,
    pub created_at: String,
    pub database: String,
    pub dump_file: String,
    pub dump_sha256: String,
    pub dump_size: u64,
    pub verified_restore: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct BackupRecord {
    pub id: String,
    pub folder: String,
    pub created_at: String,
    pub dump_size: u64,
    pub checksum: String,
    pub verified: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct BackupResult {
    pub folder: String,
    pub created_at: String,
    pub dump_size: u64,
    pub checksum: String,
    pub verified: bool,
}

#[derive(Debug, Clone)]
pub struct RuntimeContext {
    pub config: InstallConfig,
    pub paths: RuntimePaths,
    pub executables: RuntimeExecutables,
}
