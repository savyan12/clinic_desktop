# Customer Installation Guide — Clinic Desktop

Last checked against the source: 2026-11-02.

Audience: the technician or AI assistant preparing delivery, installing the application, and explaining it to the customer. Explain customer-facing steps in Arabic and use the Arabic button labels below. This document covers this standalone desktop repository only.

## 1. Read this before giving installation advice

This is the offline Windows desktop edition, not the sibling web application. Tauri packages the Vue interface, runs the Rust API inside the application process, and starts its own MySQL 8 process. Both the API and MySQL listen only on `127.0.0.1`, on locally selected ports. Closing the application stops its local services.

- The customer installs a complete Windows NSIS `.exe` installer, not the source folder, `dist/`, or the application executable alone.
- The compiled Rust backend, MySQL, fonts, and application assets must be inside the delivery package. WebView2 and Microsoft Visual C++ runtime installers are also bundled by the Windows build configuration.
- Routine use and first activation are designed to work without internet once the complete package and prerequisites are installed. Building the installer normally needs internet on the developer's machine.
- No hosting, domain, public API server, XAMPP, WAMP, IIS, Apache, Docker, or separately installed MySQL service is needed on the customer's PC.
- This is one local dataset per Windows user. Installing it on another PC or using another Windows account does **not** synchronize or share the clinic's data. Do not offer LAN or multi-PC access as an existing feature.
- Activation means creating the clinic database and administrator, or restoring a backup. The current activation screen does not request an online license key.

### Release status and bundled runtime

The current application version in `src-tauri/tauri.conf.json` and `src-tauri/Cargo.toml` is `1.0.0`; the application identifier is `com.ettizan.clinic`. Recheck these files for later releases.

**The Rust application has been exercised locally through the Tauri UI, but the Windows installer has not yet completed clean Windows 10/11 acceptance.** Before customer delivery, the release owner must build the installer and complete the tests in section 10. Frontend or Linux test success alone is not proof that the Windows installer works.

The release contains no PHP, Composer, or Laravel runtime. The Rust API is compiled into the Tauri executable. The only separately bundled server runtime is the pinned MySQL 8 x64 distribution; exact CI pins and the separate Windows 10/11 acceptance process are documented in the [GitHub Actions build guide](GITHUB_ACTIONS_WINDOWS_BUILD.md).

## 2. What must be available on the customer's PC?

| Item | Customer requirement | Installation responsibility |
| --- | --- | --- |
| Windows | Build targets are Windows 10 x64 and Windows 11 x64; Windows 11 is recommended. Validate each exact OS/build against the bundled runtime versions. | Customer/technician. Neither OS is certified by build success alone; ARM64 and 32-bit Windows are outside the build target. |
| Microsoft Edge WebView2 Runtime | Required to display the interface. An ordinary browser installation is not the acceptance check. | The configured installer includes the offline WebView2 installer. Verify installation succeeds, including on an offline clean PC. |
| Microsoft Visual C++ x64 Redistributable | Required by the packaged native runtimes. | The NSIS post-install hook runs the bundled `VC_redist.x64.exe`. Administrator approval may be needed for prerequisites. |
| Embedded Rust API | Required internally, with no separate customer installation. | Compiled into the Tauri application executable. |
| MySQL Community Server 8 and client tools | Required internally; no customer database setup or database password entry. | Bundled. The app initializes its own data directory and generates credentials. |
| Windows PowerShell (`powershell.exe`) and CIM access | Used to discover removable USB drives. | Normally provided by Windows; enterprise policies must allow the existing USB-discovery operation. Installing PowerShell 7 alone is not a substitute for `powershell.exe`. |
| Writable local user profile and adequate disk space | Required for the database, logs, temporary restore databases, and backup verification. | Technician checks before activation. |
| USB flash drive | Required for the documented backup and PC-transfer workflow. | Provide a reliable, writable drive recognized by Windows as removable storage. |
| Printer and driver | Only if paper receipts/statements are required. | Install the printer's Windows driver and verify a Windows test page. |

The customer does **not** need Node.js, npm, Rust, PHP, Composer, Visual Studio Build Tools, Git, a terminal left open, or a manually started API/database server.

Practical planning recommendations, **not measured minimum requirements**: 8 GB RAM, an SSD, at least 10 GB initially free with additional space as records/backups grow, and a 1920 × 1080 display. The configured application window has a minimum size of 1024 × 720; check Windows display scaling and usable desktop space as well. A UPS and two rotating backup drives are advisable.

## 3. Pre-installation checklist

1. Confirm whether this is a new clinic installation, a replacement PC with an existing backup, or an update of an already activated installation. Never choose new activation merely because existing data is not visible.
2. Obtain the release owner's tested x64 installer, exact version, SHA-256 digest, and support contact. Verify the file against the digest supplied through a trusted channel. A matching digest is not a publisher signature.
3. Confirm the intended daily Windows account. The installer uses `currentUser` mode; installation and first launch should be performed for that account, not under a technician's separate account.
4. Check Windows version/architecture, free space, clock, time zone, power stability, prerequisite-installation permissions, and antivirus/application-control policies.
5. If the clinic already has data, make a verified backup first and retain the existing PC/installation. Never erase it to make installation easier.
6. For recovery, obtain the complete backup folder and the existing application's login credentials. Do not ask the customer to send their password to an AI conversation.
7. Plan a quiet period for activation, updates, or transfer. Stop data entry on the old installation before the final transfer backup.

Example read-only package checks in PowerShell; replace the example filename with the actual delivered file:

```powershell
Get-FileHash -LiteralPath "C:\ClinicDelivery\Clinic-setup.exe" -Algorithm SHA256
Get-AuthenticodeSignature -LiteralPath "C:\ClinicDelivery\Clinic-setup.exe"
```

The source configuration does not establish that a release is signed. If Windows reports an unknown publisher or blocks the file, verify its origin with the release owner. Do not disable SmartScreen, antivirus, or the firewall as a routine installation step.

## 4. Install the delivered package

1. Sign in to the intended Windows account and close any running copy of the clinic application.
2. Copy the verified installer to a local folder and open it. Choose Arabic or English in the installer language selector.
3. Keep the offered installation directory unless there is an approved reason to change it. Do not use a USB drive or a shared/synchronized network folder for the live installation/data.
4. Allow the included WebView2 and Visual C++ prerequisites to install. If a trusted prerequisite asks for administrator approval, have the authorized administrator approve it.
5. Wait for completion. If prerequisite installation fails, stop and record the message; do not treat a partially installed application as ready.
6. Restart Windows if a prerequisite requests it. The VC++ hook marks exit code `3010` as restart required but uses `/norestart`, so it will not force a restart. An equal/newer x64 runtime is accepted after checking its installed version; failed installation or an unverified `1638` result stops the installer with an error.
7. Launch **`منظومة عيادة عين الحياة`** from the installed shortcut/Start menu. The configured Start menu folder is **`عيادة عين الحياة`**; use the actual release's name if branding changes.
8. Continue with section 5 for a genuinely new clinic, or section 7 for a backup transfer. An already activated installation should start its existing database and go to login, not require new activation.

No customer `.env` editing, database creation command, port forwarding, or public firewall rule is part of these steps. Ports such as `5173` and `8000` are development examples, not fixed ports of the installed desktop application.

## 5. First activation of a new clinic

Only use this path when a new empty dataset is intended. The clean-activation implementation resets the selected local database before creating its initial records; it is **not** a repair or migration method for existing clinic data.

1. In **`تهيئة منظومة العيادة`**, select **`تفعيل جديد`**.
2. Enter **`اسم العيادة`**, optional **`الوصف المختصر (اختياري)`**, **`اسم مدير النظام`**, and **`اسم المستخدم`**. The suggested username `admin` is editable; it is not a universal recovery account.
3. Let the clinic owner enter **`كلمة المرور`** and **`تأكيد كلمة المرور`** privately. The application requires at least 8 characters; use a unique longer password and store it safely outside this document.
4. Select **`إنشاء وتفعيل النظام`** once. Wait without closing the application, powering off, or repeatedly launching another copy.
5. The app initializes its local MySQL database, applies migrations, and creates one administrator and the clinic settings. It does not add demonstration patients, services, or doctors.
6. At **`تسجيل الدخول`**, use the administrator credentials just created.
7. Through the normal administrative screens, configure the real clinic details/printing preferences, services and prices, doctors and rates, reception users, and sponsors where needed. Blank lists on a new installation are expected.
8. Create and verify the initial USB backup using section 6, then complete section 8 before handover.

Do not run development seeders, `migrate:fresh`, or database reset commands on a customer's installation. Do not reuse passwords shown in mock/demo documentation.

## 6. Back up the clinic's data

The existing UI provides **manual** USB backups. It does not currently configure an automatic daily schedule. Agree who will make the backup at the end of each working day and before every update or PC transfer.

1. Finish current data entry, sign in as an administrator, and connect a trusted USB flash drive.
2. Open **`إعدادات النظام`** → **`نسخة احتياطية على USB`**.
3. Click **`تحديث الوحدات`** if needed, then choose the correct drive under **`اختر وحدة USB`**. Check its label, drive letter, and free space.
4. Click **`إنشاء النسخة والتحقق منها`** once. Do not remove the drive, close the app, or shut down Windows while it is working.
5. Wait for **`تم إنشاء نسخة موثقة ومجرّبة الاستعادة`** and the displayed backup path. File presence by itself is not a success signal.
6. Record the backup date/path, safely eject the drive, and store it securely away from the PC. Keep more than one dated backup and rotate drives.

The app creates this structure at the USB drive root (drive letter and date/time are examples):

```text
E:\ClinicBackups\2026-11-01\18-30-00\
    clinic-database.sql
    manifest.json
    COMPLETE.sha256
```

Preserve all three files and the complete `ClinicBackups` directory structure. Do not edit the manifest/checksums, copy only the SQL file, or rename an incomplete `.partial-*` directory to make it appear complete.

The implementation makes a transaction-consistent dump, tests importing the local dump, copies it to USB and flushes the writes, compares SHA-256 after reading it back, and tests importing the USB copy before publishing the completed folder. Those checks establish file integrity and basic database restorability; they do not replace the operator's restored-data checks in section 8.

Important limits:

- The backup is a **database dump**, including clinic records, application users, password hashes, and database settings. It is not a full Windows image, application installer, printer-driver backup, or archive of all local files.
- Backups are **not encrypted by the application**. SHA-256 detects integrity problems; it does not hide patient data or prove who created a backup. Use trusted media, restricted physical access, and approved encrypted storage where available. Restore only trusted clinic backups.
- The USB free-space check requires at least twice the dump size plus 10 MiB. The local PC also needs room for dumps and temporary MySQL databases; this USB check is not a local disk-space guarantee.
- Some external USB SSDs identify as fixed disks and will not appear: Windows discovery currently accepts `DriveType=2` removable drives. Use a recognized flash drive; do not repartition/format customer storage as an automatic workaround.

## 7. Transfer to a replacement PC or restore a backup

The supported customer-facing restore path is **during first activation**. Settings currently offers backup creation, not an in-place restore button. An activated installation with a runtime error offers retry, not a reset/restore wizard.

1. If the old installation works, stop further data entry, make a final verified USB backup, and retain the old PC untouched until the transfer is accepted.
2. Install a tested application release on the replacement PC under its intended Windows account. Use the same application version as the backup, or a newer release whose restore compatibility has been tested. Backups from a newer app version are rejected by an older app.
3. On the first activation screen, select **`استكمال من نسخة احتياطية`**, not **`تفعيل جديد`**.
4. Connect the drive, click **`تحديث`**, and choose **`وحدة USB`**.
5. Select the intended **`النسخة الاحتياطية`** after checking its date/time and size. Only accepted complete backups are listed. If none appears, use section 11; do not manufacture the completion files.
6. Click **`التحقق والاستعادة والتفعيل`** once and wait. Do not remove the drive or interrupt power.
7. The app validates the backup, imports it into a temporary database, applies migrations there, checks it, and installs the restored database. The destination generates its own local service credentials.
8. Log in using an application username/password that existed **at the time of the backup**, not a new default password. Restoring an older backup also restores the users/password hashes from that time.
9. Compare recent patients, sessions, subscriptions, wallet/debt balances, and financial totals with the old installation or agreed reports. Confirm the backup cutoff; entries created after it are not included.
10. Verify printing, make a new verified backup from the replacement PC, and designate it as the sole live installation before resuming work.

Restore replaces a dataset; it does not merge two PCs' work. For recovery on an **already activated** PC, stop and request a technician-approved recovery plan with a verified external backup. Do not delete `install.json`, the Windows profile, or `mysql-data` to expose the activation screen. The internal restore routine has safety/rollback logic, but that is not a customer-facing permission to overwrite an existing installation.

## 8. Installation acceptance and handover

Record actual results; do not mark an item passed based only on this guide.

- [ ] Application starts from its shortcut under the intended Windows account, without developer tools or an open terminal.
- [ ] The tested release works with internet disconnected, including local login and data access.
- [ ] Arabic text, RTL layout, fonts, display scaling, and navigation/dialogs are usable on the actual screen.
- [ ] New installation has the expected empty dataset, or restored records/totals match the chosen backup.
- [ ] Administrator and reception logins work, with their expected permissions.
- [ ] Real clinic settings, services, doctors, and user accounts have been reviewed by the owner.
- [ ] Printer test page and an application print/PDF output are readable in Arabic; Excel export opens if required. An Excel-compatible viewer is optional software for viewing exports, not for running the clinic app.
- [ ] App closes and reopens normally; Windows restart preserves the records.
- [ ] A USB backup completes successfully, and a restore has been rehearsed on a separate test PC/profile using the delivered release.
- [ ] The owner knows the backup routine, data location, support contact, and how to close the app before shutdown.

Perform any dummy patient/session/payment end-to-end tests on an isolated acceptance installation. Do not create fake financial history in the live clinic database. A live transaction test needs the clinic owner's explicit approval.

Handover record: installer filename/version/hash, Windows version/architecture/account, installation date, prerequisite versions, printer result, latest verified backup path/date, restore-test result, unresolved issues, and support contact. **Exclude passwords, tokens, application keys, and database credentials.**

## 9. Data location, updates, and removal

With the current identifier, Windows operational data is expected under:

```text
%LOCALAPPDATA%\com.ettizan.clinic\
    runtime\install.json
    runtime\mysql-root.cnf
    mysql-data\
    logs\mysql.log
```

The authority is Tauri's `app_local_data_dir()` plus the application identifier, not the display name or installation directory. Check the actual account/path if the identifier changes in a later release. Rust API startup errors are shown by the activation/runtime screen; MySQL process output is written to `logs\mysql.log`.

`install.json` contains generated ports, database credentials, and `APP_KEY`; `mysql-root.cnf` contains database root connection credentials. Treat both as secrets. Do not paste them into AI chats/support tickets. Protect the Windows account and disk; this application does not establish database-at-rest encryption merely by using login passwords.

For an update:

1. Obtain the release owner's tested upgrade package and compatibility notes, including the bundled MySQL version. Do not swap a different MySQL binary into an existing data directory yourself.
2. Stop data entry and make a verified external backup **before** installing or opening the new release. Startup can apply database migrations automatically.
3. Close the application normally, then install the approved newer package for the same Windows account. Retain the previous installer and backup until acceptance is complete.
4. Launch, log in to the existing dataset, repeat the relevant section 8 checks, and make a post-update backup. There is no configured automatic updater in this source.

Downgrades are disabled in the Windows bundle configuration. Do not force an older executable against a migrated database. Rollback requires a tested recovery plan using the pre-update backup and a compatible release.

Before uninstalling, reinstalling Windows, deleting a user profile, or retiring a PC, obtain and test an external backup. Application files and operational data are separate, but **uninstaller data retention has not been verified here**; never rely on uninstall/reinstall as a backup or reset strategy. Do not copy the live `mysql-data` directory while MySQL is running.

## 10. Release-owner appendix: prepare the Windows installer

This section runs on the **developer/build machine**, not on the customer's PC. Run all project commands from the standalone repository root.

### Build-machine requirements

- Windows x64 and PowerShell 7.4 or later (`pwsh`) for the release scripts. The customer's built-in `powershell.exe` remains sufficient for removable-drive discovery at runtime.
- Node.js compatible with the installed Vite version (currently `^20.19.0 || >=22.12.0`) and npm. Prefer a maintained compatible LTS release.
- Rust stable with the `x86_64-pc-windows-msvc` host/toolchain, Visual Studio C++ Build Tools with desktop C++ workload, and a Windows SDK.
- Official MySQL Community Server 8 x64 ZIP and compatible official `VC_redist.x64.exe`. Record exact versions and hashes; do not silently replace MySQL with MariaDB or an untested major version.
- Install the VC++ runtime on the build machine too: merely copying its installer into resources is not sufficient for running the MySQL validation commands.
- Internet for npm/Cargo and the Tauri packaging dependencies, including obtaining the offline WebView2 installer. This does not imply a customer runtime internet requirement.

Use trusted vendor sources: [MySQL Community Server](https://dev.mysql.com/downloads/mysql/), [Microsoft Visual C++ Redistributable documentation](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist), and [WebView2](https://developer.microsoft.com/en-us/microsoft-edge/webview2/). Download runtime versions compatible with this release, not simply every latest major version.

### Build steps

Use a clean release checkout without customer data. Replace these example local paths. Check every command's exit status and stop on an error; do not paste through failures.

```powershell
Set-Location "C:\Projects\clinic_desktop"
node --version
npm --version
rustc --version
cargo --version

npm ci

pwsh -NoProfile -ExecutionPolicy Bypass -File scripts/prepare-windows-runtime.ps1 `
  -MySqlArchive "C:\Downloads\mysql-8-x64.zip" `
  -VcRedistInstaller "C:\Downloads\VC_redist.x64.exe"

npm run lint
npm run build
npm run verify:ui
npm run desktop:prepare
node scripts/validate-windows-runtime.mjs
Push-Location src-tauri
cargo fmt --check
cargo check --locked
cargo test --locked
Pop-Location
npm run desktop:build:windows
```

The shown execution-policy override applies to this PowerShell process running the reviewed project script, not a permanent Windows policy change. Follow the organization's software-execution policy; do not bypass a managed prohibition.

The preparation script rebuilds the **generated** `src-tauri/resources/runtime/mysql` and `vcredist` folders. The JavaScript preparation step validates that required runtime files are present and that the Tauri bundle mapping contains no legacy PHP/Laravel resources. Never keep customer data in these staging folders.

Before bundling, confirm all expected files exist:

```text
src-tauri/resources/runtime/
    mysql/bin/mysqld.exe
    mysql/bin/mysql.exe
    mysql/bin/mysqldump.exe
    mysql/bin/mysqladmin.exe
    vcredist/VC_redist.x64.exe
```

Runtime versions and hashes live in `scripts/windows-runtime.lock.json`. The validator checks the MySQL version and x64 architecture, required server/client/backup executables, error-message assets, VC++ installer, and Tauri resource allowlist. It rejects any PHP, Composer, Laravel, or staged-backend resource mapping so the delivered application cannot silently depend on the retired runtime.

The default x64-host build writes the installer under **`src-tauri\target\release\bundle\nsis\`**. Use the actual generated filename and verify architecture; custom Cargo target/output settings may change the location. The final package must include the offline WebView2 installer, the runtime tree, and the configured VC++ post-install hook. `npm run build` alone only produces frontend assets and is not an installer.

Inspect staged content for accidental customer files, SQL dumps, credentials, development environment files, or symbolic links. The release runtime should contain only the approved MySQL and VC++ trees; the Rust backend is part of the compiled executable. The validator is not a general secret scanner for arbitrary source-code contents. Do not ship a copied working customer's installation as a release package.

### Required release gate

On a clean disposable Windows x64 VM/PC **without** Node, PHP, Composer, MySQL, or Rust installed:

1. Test the exact installer with internet disconnected, including missing-prerequisite installation and a restart if requested.
2. Test fresh activation, both roles, representative clinical/financial flows on dummy data, printing, PDF/Excel exports, exit/reopen, and Windows restart.
3. Test actual removable USB backup and first-run restoration on another clean installation. Compare records and totals, not only success messages.
4. Test upgrade from the previous delivered version with representative data and its bundled MySQL version. Keep the old verified backup intact.
5. Record OS/build/runtime versions, release hash, test results, and remaining limits. Deliver only after unresolved install/runtime failures are fixed and re-tested.

Rust database/API tests remain part of release verification in an isolated development/test environment. Do not run migration/reset/testing commands against the customer's local database. None of the Windows release-gate steps is claimed complete by this document.

## 11. Troubleshooting without risking clinic data

| Symptom | Safe first checks / next step |
| --- | --- |
| No installer, only source files or `dist/` | Ask the release owner to complete section 10. Do not install a development toolchain on the customer's PC as a substitute. |
| Visual C++ prerequisite fails or a runtime DLL is missing | Record the exact message; verify trusted x64 VC++ package installation, permissions, and pending restart. Never download individual DLLs from third-party sites. |
| Blank window or WebView2 error | Verify WebView2 Runtime installation and the delivered package; check Windows application-control policy. Repair using a trusted Microsoft/package installer, not by deleting clinic data. |
| `تعذر تشغيل الخدمات المحلية` | Click `إعادة المحاولة` once. If it persists, close normally, check disk space, app files, security quarantine, and the logs in section 9. Escalate with redacted error details. |
| Rust API does not start | Close duplicate application windows, retry once, and record the complete redacted message. Verify that the delivered executable matches the installer hash; do not install PHP or run the retained Laravel source as a customer workaround. |
| Local port/process conflict | Close duplicate application windows and retry normal startup. Ports are generated and may change. Do not kill every MySQL process or stop unrelated Windows services. |
| USB is not listed | Confirm it appears in File Explorer, is unlocked/writable, and is classified as removable. Refresh the list. Check `powershell.exe`/CIM policy. Do not format it automatically. |
| No backup is listed | Verify the correct drive and `ClinicBackups/date/time` hierarchy with all three files. Incomplete, damaged, or newer-version backups may be omitted. Use another verified copy or contact support. |
| Backup verification fails / insufficient space | Leave the current database and older backups intact. Check local and USB capacity, write protection, and drive health; retry on a known-good drive. Do not claim success from a partial file. |
| Application appears empty after changing Windows account | Return to the intended account and verify its data location. Do not activate a new database over missing/unidentified data. |
| Forgotten administrator password | Contact the authorized maintainer with proof of ownership. There is no documented self-service reset here; do not suggest a demo password or delete/reseed the database. |
| Incorrect print layout or Arabic output | Check Windows printer driver, paper size, app print settings, and the exact installed release; test on an isolated document. Do not alter financial records to fix printing. |
| Existing data recovery or failed restore | Stop writes, retain the original backup and existing data, and request a recovery plan. Never repeatedly reset or reinitialize the database. |

For support, provide the app version, Windows version/architecture, affected step, exact redacted error, and backup date/status. Logs can contain personal information or secrets; review/redact them locally. Do not upload a patient database, `install.json`, or `mysql-root.cnf` to an AI service.

## 12. Instructions for the next AI assistant

Start by asking only what changes the path: Is this a new installation, an update, or recovery? Is the tested `.exe` available? Which Windows version/architecture and daily Windows account will be used? Is there existing data or a verified backup?

Then explain only the applicable customer steps in Arabic. Separate required software from software already bundled, and separate client installation from developer packaging. Do not execute build, activation, restore, or update steps merely because this guide is present; obtain the user's task authorization first. Any replacement of existing data requires an explicit recovery decision and backup.

Do not promise cloud sync, automatic scheduled backups, encryption, a reset/restore button in Settings, an online license flow, or a working Windows installer unless the current release actually provides and verifies them. Do not change unrelated source code to make the instructions fit.

Source map for rechecking a later release (paths relative to this document):

- [Desktop overview](../README.md) and [package scripts](../package.json).
- [Installer configuration](../src-tauri/tauri.conf.json), [Rust version/dependencies](../src-tauri/Cargo.toml), and [VC++ installer hook](../src-tauri/windows/installer-hooks.nsh).
- [Windows runtime preparation](../scripts/prepare-windows-runtime.ps1), [runtime staging](../scripts/prepare-runtime.mjs), and [runtime pins](../scripts/windows-runtime.lock.json).
- [Runtime paths, activation, startup, shutdown, and credentials](../src-tauri/src/runtime.rs), [Rust API](../src-tauri/src/api/mod.rs), and [schema migration](../src-tauri/src/api/schema.rs).
- [Backup format/validation/restore](../src-tauri/src/backup.rs) and [USB discovery](../src-tauri/src/drives.rs).
- [Activation screen and exact labels](../src/views/activation/ActivationView.vue), [Settings backup UI](../src/views/settings/SettingsView.vue), and [activation routing](../src/router/index.js).
