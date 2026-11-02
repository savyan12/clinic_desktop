# Clinic Desktop Application

This standalone repository contains the offline desktop edition of the physiotherapy clinic system. It keeps the Vue 3/Vuetify interface while Tauri starts a private local MySQL 8 server and an embedded loopback-only Rust API built with Axum and SQLx. The retained `backend/` directory is a migration reference and rollback source; it is not packaged or required at runtime.

For customer prerequisites, installation, activation, backup/restore, and the release checklist, see [Customer Installation Guide](docs/CUSTOMER_INSTALLATION.md). For automated builds, version tags, runtime pins, and separate Windows 10/11 acceptance, see [GitHub Actions Windows Builds](docs/GITHUB_ACTIONS_WINDOWS_BUILD.md).

## Runtime behavior

- The application binds MySQL and the Rust API to `127.0.0.1` on generated local ports.
- Database credentials are generated per installation and stored in the current user's private application-data directory.
- Operational data is stored under the current Windows user's local application-data directory, not inside the installation folder.
- The first launch offers either a clean activation or a verified restore from USB.
- A clean activation creates one administrator and no demonstration data.

## Run locally on Linux for development

Install Node.js/npm, Rust, MySQL 8 server/client tools, and the Linux WebKit dependencies required by Tauri. PHP and Composer are not required. Then run:

```bash
npm ci
npm run desktop:dev
```

Keep that terminal open while using the application. The development runtime searches `PATH` and standard Linux locations such as `/usr/sbin/mysqld`, so MySQL can still start when a graphical session omits system-administration directories from `PATH`.

## Build a Windows NSIS installer

Build on 64-bit Windows with Node.js, npm, Rust stable (MSVC toolchain), Visual Studio C++ Build Tools, and PowerShell 7.4 or later (`pwsh`).

Download these official 64-bit packages first:

1. MySQL Community Server 8 x64 ZIP archive.
2. Microsoft Visual C++ 2015–2022 Redistributable (`VC_redist.x64.exe`).

Then run:

```powershell
npm ci
pwsh -NoProfile -ExecutionPolicy Bypass -File scripts/prepare-windows-runtime.ps1 `
  -MySqlArchive "C:\Downloads\mysql.zip" `
  -VcRedistInstaller "C:\Downloads\VC_redist.x64.exe"
npm run desktop:build:windows
```

The installer is produced under `src-tauri\target\release\bundle\nsis\`. It bundles the Rust backend inside the Tauri executable, MySQL, WebView2's offline installer, and the Visual C++ runtime installer.

The release validator checks the MySQL x64 binaries and verifies that the bundle resource mapping contains no PHP, Composer, Laravel, or backend runtime directory. GitHub Actions uses the exact verified runtime versions in `scripts/windows-runtime.lock.json` and uploads the installer, its SHA-256, and a runtime manifest.

## USB backup guarantees

The settings screen writes backups to:

```text
ClinicBackups/YYYY-MM-DD/HH-mm-ss/
```

Each accepted folder contains the SQL dump, a manifest with its SHA-256 digest, and a completion marker. Before a backup is reported as successful, the application:

1. creates a transaction-consistent MySQL dump;
2. verifies that the local dump restores into a temporary database;
3. copies it to USB and flushes the files;
4. reads it back and compares SHA-256;
5. restores the USB copy into another temporary database;
6. publishes the folder atomically only after every check succeeds.

Incomplete folders are removed and are never listed as restorable backups. Restore first validates the manifest, size, checksum, application version, and a staging migration. If an existing database is being replaced, an internal safety dump is verified first and used for rollback if replacement fails.

## Verification

```bash
npm run lint
npm run build
npm run verify:ui
npm run desktop:prepare
cd src-tauri && cargo fmt --check && cargo check && cargo test
```

The final NSIS installer must still be built and smoke-tested on Windows because Linux cannot validate the packaged Windows MySQL/runtime binaries or execute the installer.

`npm run verify:ui` checks picker caching, dialog lifecycle, patient selection, and out-of-order responses using the real Vue setup/watchers and a simulated HTTP adapter. It does not connect to the clinic database or measure browser animation performance.
