# Desktop runtime staging directory

The Rust backend is compiled into the Tauri executable. On Windows,
`prepare-windows-runtime.ps1` extracts the official MySQL archive into `mysql/`
and stages the verified Visual C++ prerequisite in `vcredist/`.

Generated runtime files and vendor binaries are intentionally excluded from Git.
Legacy `backend/` or `php/` folders may remain locally during migration rollback
testing, but the Tauri resource allowlist does not package them.
