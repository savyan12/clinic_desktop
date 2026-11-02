# Desktop Delivery Progress

## Current state

- The desktop edition now runs Vue inside Tauri, starts a private loopback MySQL 8 instance, and serves `/api/v1` from the embedded Rust process using Axum and SQLx.
- PHP, Composer, and Laravel are not required or packaged at runtime. The existing `backend/` directory remains intact as a rollback/reference implementation until Windows acceptance is complete.
- The Rust backend preserves the existing MySQL schema and migration names, customer data, Sanctum-compatible bearer tokens, bcrypt passwords, decimal money precision, row locking, financial ledgers, audit logs, idempotency records, Arabic errors, role rules, and frontend API contract.
- Operational data belongs to the daily Windows user. Verified backup and first-activation restore remain mandatory parts of delivery.
- Desktop migration implementation is complete. The remaining release gate is Windows packaging and acceptance, not missing backend functionality.
- The desktop application is maintained as a standalone repository; sibling web applications are outside its source tree and remain unchanged.

## Done

- Inventoried all Laravel routes, controllers, requests, resources, models, migrations, services, tests, Vue API callers, Tauri startup, activation, and backup/restore paths.
- Implemented the desktop API in Rust with Axum, SQLx/MySQL, `rust_decimal`, bcrypt, and Sanctum-compatible token hashing/storage.
- Implemented authentication and login throttling, admin/receptionist authorization, patients, sessions, subscriptions, services, doctors, sponsors, users, wallet, debts, payments, refunds, doctor payouts, settings, audit log, dashboard, reports, and PDF/XLSX exports.
- Preserved financial transaction boundaries, locks, ledger reversals, commission snapshots, debt allocations, subscription consumption/restoration, and idempotent replay behavior.
- Kept the existing MySQL engine, business schema, data, and Laravel migration history. Fresh activation, existing-schema validation, migration, backup, restore, and API restart use Rust without PHP.
- Integrated the Rust API with Tauri startup/shutdown, local MySQL lifecycle, activation, runtime error handling, and verified backup/restore.
- Added local Arabic-capable PDF fonts and Rust-generated PDF/XLSX exports.
- Updated runtime staging and the Windows build workflow so legacy PHP, Composer, Laravel, and staged backend resources are rejected from the packaged application.
- Updated `README.md`, `CUSTOMER_INSTALLATION.md`, `GITHUB_ACTIONS_WINDOWS_BUILD.md`, runtime pins, validation scripts, artifact collection, and NSIS preparation for the Rust-only runtime.
- Preserved unloaded Laravel relationship behavior by omitting unloaded fields instead of returning `null`, and added contract assertions for payment and relationship shapes.
- Restored the configurable commission base through `CLINIC_COMMISSION_BASE=net|gross`, defaulting to `net` as required by D-01.
- Removed Rust development warnings by trimming unused row projections and limiting test-only helpers to test builds; no global warning suppression was added.
- Added transaction retry parity with Laravel: idempotent money operations retry MySQL deadlock/lock-timeout failures up to three times. Concurrent session close remains single-effect and returns the existing result to the losing request.
- Matched Laravel's empty-query normalization for the sessions and audit-log filters, so values such as `date=` and `user_id=` are treated as absent instead of producing an Axum 400 response.
- User smoke-tested the application through the Tauri UI and confirmed it works correctly.
- Prepared the desktop source as a standalone repository with root-level Windows CI paths and exclusions for generated runtimes, build output, local secrets, backups, and customer databases.

## Verification completed

- Frontend lint and production build passed.
- UI verification passed: 11/11 checks.
- Runtime staging tests passed.
- `cargo fmt --check` and `cargo check --no-default-features --locked` pass without warnings.
- Normal Rust tests passed: 8 passed, 14 intentionally ignored because they require isolated MySQL/runtime processes.
- Full isolated MySQL/runtime suite passed: 14/14, including API contract, concurrent/idempotent close, financial lifecycle, Laravel-schema compatibility, activation, backup -> mutation -> restore, and Rust API restart.
- Targeted MySQL HTTP contract verification passed for the exact empty-filter URLs sent by the sessions and audit-log Vue screens.
- `git diff --check` passed after the latest fixes.

## Remaining release acceptance

- Run the hosted Windows workflow and produce the complete x64 NSIS installer.
- Install on clean Windows 10 x64 and Windows 11 x64 machines/profiles without Node.js, Rust, PHP, Composer, or MySQL preinstalled.
- Verify prerequisites, clean activation, both roles, representative clinical/financial flows, printing, PDF/XLSX exports, offline operation, close/reopen, and persistence after Windows restart.
- Verify backup to a physical removable USB drive and first-run restore on a separate clean installation, comparing records and financial totals.
- Verify upgrade from the previous delivered release with representative data and a retained pre-upgrade backup.
- Record installer filename/version/hash and the separate Windows 10/11 acceptance results. Do not describe the release as Windows-certified until these checks pass.

## Decisions

- RUST-01: Implement the desktop API inside the existing Rust/Tauri package with Axum + SQLx/MySQL, while retaining `backend/` as a non-packaged rollback/reference implementation through final Windows acceptance.
- RUST-02: Keep the existing MySQL engine and business tables unchanged. Continue using Laravel's existing `migrations` table and migration names so no second migration-history schema is introduced.
- RUST-03: Preserve Sanctum token storage/format compatibility and existing password hashes across the cutover.
- RUST-04: Package only the compiled Rust API; do not delete the retained Laravel source during the safe migration period.
- RUST-05: Match Laravel's three-attempt database transaction retry behavior for MySQL deadlocks and lock timeouts.
- DOC-01: Keep desktop delivery documentation inside this standalone repository's `docs/` directory; sibling web applications remain separate and unchanged.
- DOC-02: Treat hardware figures as planning recommendations, not benchmarked minimums, and distinguish bundled customer runtimes from developer build tools.
- DOC-03: Document only the available UI: manual backup in Settings and restore during first activation. Do not invent an in-place restore button or automatic scheduler.
- CI-01: Build only the desktop Vue/Tauri/Rust application and fail staging if any legacy PHP/Laravel runtime resource is mapped into the package.
- CI-02: Keep NSIS and the current-user installation model. Validate official runtime sources, exact pins where supported, and the offline WebView2/VC++/MySQL payloads.
- CI-03: Normal/manual workflow runs upload artifacts only. Only a successful version-tag push may publish a release.

## Questions for the client

- Final deployment must confirm the target Windows version/architecture, intended daily Windows account, installer version, and whether the installation is new, an update, or a restore of existing clinic data.
