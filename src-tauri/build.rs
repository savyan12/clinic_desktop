use std::path::PathBuf;

fn main() {
    let target = std::env::var("TARGET").unwrap_or_default();

    if target.contains("windows") {
        let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
        let runtime = manifest_dir.join("resources/runtime");
        let required = [
            runtime.join("mysql/bin/mysqld.exe"),
            runtime.join("mysql/bin/mysql.exe"),
            runtime.join("mysql/bin/mysqldump.exe"),
            runtime.join("mysql/bin/mysqladmin.exe"),
            runtime.join("vcredist/VC_redist.x64.exe"),
        ];

        for path in required {
            if !path.exists() {
                panic!(
                    "missing Windows desktop runtime resource: {}. Run scripts/prepare-windows-runtime.ps1 first.",
                    path.display()
                );
            }
        }
    }

    tauri_build::build();
}
