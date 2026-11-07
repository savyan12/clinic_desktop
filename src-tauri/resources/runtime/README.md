# Desktop runtime staging directory

The Rust backend is compiled into the Tauri executable. On Windows,
`prepare-windows-runtime.ps1` extracts the official MySQL archive into `mysql/`
and stages the verified Visual C++ prerequisite in `vcredist/`.

Generated runtime files and vendor binaries are intentionally excluded from Git.
The Tauri resource allowlist also rejects PHP, Composer, Laravel, and legacy
backend runtime resources.
