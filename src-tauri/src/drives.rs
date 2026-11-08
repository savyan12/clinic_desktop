use crate::models::RemovableDrive;
#[cfg(target_os = "windows")]
use crate::runtime;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn list_removable_drives() -> Result<Vec<RemovableDrive>, String> {
    #[cfg(target_os = "windows")]
    {
        list_windows_drives()
    }

    #[cfg(not(target_os = "windows"))]
    {
        list_unix_drives()
    }
}

pub fn resolve_drive(drive_id: &str) -> Result<(RemovableDrive, PathBuf), String> {
    let drive = list_removable_drives()?
        .into_iter()
        .find(|drive| drive.id == drive_id)
        .ok_or_else(|| "وحدة التخزين المحددة لم تعد متصلة. أعد تحديث القائمة.".to_string())?;
    let root = PathBuf::from(&drive.path);

    if !root.exists() || !root.is_dir() {
        return Err("تعذر الوصول إلى وحدة التخزين المحددة.".to_string());
    }

    Ok((drive, root))
}

pub fn ensure_path_is_on_drive(path: &Path, drive_root: &Path) -> Result<PathBuf, String> {
    let canonical_root = drive_root
        .canonicalize()
        .map_err(|error| format!("تعذر التحقق من وحدة التخزين: {error}"))?;
    let canonical_path = path
        .canonicalize()
        .map_err(|error| format!("تعذر الوصول إلى النسخة الاحتياطية: {error}"))?;

    if !canonical_path.starts_with(&canonical_root) {
        return Err("مسار النسخة الاحتياطية لا ينتمي إلى وحدة USB المحددة.".to_string());
    }

    Ok(canonical_path)
}

#[cfg(target_os = "windows")]
fn list_windows_drives() -> Result<Vec<RemovableDrive>, String> {
    let script = concat!(
        "[Console]::OutputEncoding=[Text.UTF8Encoding]::UTF8; ",
        "@(Get-CimInstance Win32_LogicalDisk -Filter 'DriveType=2' | ",
        "Select-Object DeviceID,VolumeName,FreeSpace) | ConvertTo-Json -Compress"
    );
    let mut command = Command::new("powershell.exe");
    runtime::hide_console_window(&mut command);
    let output = command
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            script,
        ])
        .output()
        .map_err(|error| format!("تعذر قراءة وحدات USB: {error}"))?;

    if !output.status.success() {
        return Err("تعذر قراءة وحدات USB من Windows.".to_string());
    }

    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if text.is_empty() {
        return Ok(Vec::new());
    }
    let value: Value = serde_json::from_str(&text)
        .map_err(|error| format!("تعذر تحليل قائمة وحدات USB: {error}"))?;
    let rows = match value {
        Value::Array(rows) => rows,
        Value::Object(_) => vec![value],
        _ => Vec::new(),
    };

    Ok(rows
        .into_iter()
        .filter_map(|row| {
            let id = row.get("DeviceID")?.as_str()?.to_string();
            let label = row
                .get("VolumeName")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .unwrap_or("USB")
                .to_string();
            let free_bytes = row
                .get("FreeSpace")
                .and_then(|value| value.as_u64().or_else(|| value.as_str()?.parse().ok()));
            Some(RemovableDrive {
                id: id.clone(),
                label,
                path: format!("{id}\\"),
                free_bytes,
            })
        })
        .collect())
}

#[cfg(not(target_os = "windows"))]
fn list_unix_drives() -> Result<Vec<RemovableDrive>, String> {
    let output = Command::new("lsblk")
        .args([
            "--json",
            "--bytes",
            "--output",
            "NAME,PATH,LABEL,RM,TRAN,MOUNTPOINTS,FSAVAIL",
        ])
        .output()
        .map_err(|error| format!("تعذر قراءة وحدات التخزين: {error}"))?;

    if !output.status.success() {
        return Err("تعذر قراءة وحدات التخزين القابلة للإزالة.".to_string());
    }

    let value: Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("تعذر تحليل وحدات التخزين: {error}"))?;
    let mut drives = Vec::new();
    collect_unix_devices(
        value.get("blockdevices").and_then(Value::as_array),
        false,
        &mut drives,
    );
    drives.sort_by(|left, right| left.path.cmp(&right.path));
    drives.dedup_by(|left, right| left.path == right.path);
    Ok(drives)
}

#[cfg(not(target_os = "windows"))]
fn collect_unix_devices(
    rows: Option<&Vec<Value>>,
    parent_removable: bool,
    drives: &mut Vec<RemovableDrive>,
) {
    let Some(rows) = rows else {
        return;
    };

    for row in rows {
        let removable = parent_removable
            || row.get("rm").and_then(Value::as_bool).unwrap_or(false)
            || row.get("rm").and_then(Value::as_u64) == Some(1)
            || row.get("tran").and_then(Value::as_str) == Some("usb");
        let mountpoints = row
            .get("mountpoints")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        if removable {
            for mountpoint in mountpoints
                .into_iter()
                .filter_map(|value| value.as_str().map(str::to_string))
            {
                if mountpoint.is_empty() {
                    continue;
                }
                let id = row
                    .get("path")
                    .and_then(Value::as_str)
                    .unwrap_or(&mountpoint)
                    .to_string();
                let label = row
                    .get("label")
                    .and_then(Value::as_str)
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or("USB")
                    .to_string();
                let free_bytes = row
                    .get("fsavail")
                    .and_then(|value| value.as_u64().or_else(|| value.as_str()?.parse().ok()));
                drives.push(RemovableDrive {
                    id,
                    label,
                    path: mountpoint,
                    free_bytes,
                });
            }
        }

        collect_unix_devices(
            row.get("children").and_then(Value::as_array),
            removable,
            drives,
        );
    }
}
