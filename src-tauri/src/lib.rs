mod api;
mod backup;
mod drives;
mod models;
mod runtime;

use models::{
    BackupRecord, BackupResult, DesktopState, NewActivationRequest, RemovableDrive, RuntimeInfo,
};
use tauri::{AppHandle, Manager, RunEvent, State};

#[tauri::command]
fn get_runtime_info(state: State<'_, DesktopState>) -> Result<RuntimeInfo, String> {
    runtime::runtime_info(state.inner())
}

#[tauri::command]
async fn activate_new_installation(
    state: State<'_, DesktopState>,
    request: NewActivationRequest,
) -> Result<RuntimeInfo, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || runtime::activate_new(&state, request))
        .await
        .map_err(|error| format!("تعذر إكمال التفعيل: {error}"))?
}

#[tauri::command]
fn list_removable_drives() -> Result<Vec<RemovableDrive>, String> {
    drives::list_removable_drives()
}

#[tauri::command]
async fn list_drive_backups(drive_id: String) -> Result<Vec<BackupRecord>, String> {
    tauri::async_runtime::spawn_blocking(move || backup::list_backups(&drive_id))
        .await
        .map_err(|error| format!("تعذر قراءة النسخ الاحتياطية: {error}"))?
}

#[tauri::command]
async fn create_usb_backup(
    state: State<'_, DesktopState>,
    drive_id: String,
) -> Result<BackupResult, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || backup::create_backup(&state, &drive_id))
        .await
        .map_err(|error| format!("تعذر إنشاء النسخة الاحتياطية: {error}"))?
}

#[tauri::command]
async fn activate_from_backup(
    state: State<'_, DesktopState>,
    drive_id: String,
    backup_id: String,
) -> Result<RuntimeInfo, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        backup::restore_backup(&state, &drive_id, &backup_id)
    })
    .await
    .map_err(|error| format!("تعذر استكمال النسخة الاحتياطية: {error}"))?
}

#[tauri::command]
async fn restart_local_runtime(state: State<'_, DesktopState>) -> Result<RuntimeInfo, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        runtime::start_existing(&state)?;
        runtime::runtime_info(&state)
    })
    .await
    .map_err(|error| format!("تعذر إعادة تشغيل الخدمات المحلية: {error}"))?
}

pub fn run() {
    let state = DesktopState::default();
    let managed_state = state.clone();
    let app = tauri::Builder::default()
        .manage(managed_state)
        .setup(move |app| {
            if let Err(error) = runtime::initialize(app.handle(), &state) {
                runtime::record_runtime_error(&state, &error);
                eprintln!("Desktop runtime initialization failed: {error}");
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_runtime_info,
            activate_new_installation,
            list_removable_drives,
            list_drive_backups,
            create_usb_backup,
            activate_from_backup,
            restart_local_runtime,
        ])
        .build(tauri::generate_context!())
        .expect("error while building the desktop application");

    app.run(|app_handle: &AppHandle, event| {
        if matches!(event, RunEvent::Exit | RunEvent::ExitRequested { .. }) {
            runtime::shutdown(app_handle.state::<DesktopState>().inner());
        }
    });
}
