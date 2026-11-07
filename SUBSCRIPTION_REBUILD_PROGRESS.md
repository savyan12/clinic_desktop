# Subscription Rebuild Progress

## Current objective

Replace the monthly/legacy split with one fixed-session subscription model across the Rust API, MySQL schema, Vue UI, reports, mocks, and tests. The embedded Rust API is the only application backend.

## Final domain design

- A subscription has a patient, required name, covered services, one shared session allowance, a manually entered total price, a backend-generated start date, and `active`, `completed`, or `cancelled` status.
- Every recorded subscription session selects its own supervising doctor and covered services, closes immediately, consumes exactly one shared allowance, and stores zero service value and zero commission.
- Cancelling a closed subscription session creates one restoration usage, restores one allowance, and reactivates a completed subscription when appropriate.
- Received money is derived from direct subscription payments plus debt-payment allocations. Refunds are separate negative payment rows with `payment_refund_allocations` linking them to their attributable source payments.
- Subscription cancellation accepts an explicit refund amount and method, rejects amounts above net retained receipts, voids open subscription debt, and leaves all original payments intact.
- Generic refunds reject payments attributed to subscriptions; subscription refunds must use the cancellation workflow.
- Admin correction may change only total sessions and total price, preserves consumed sessions and previous payments, reconciles the remaining debt, and writes an audit record.
- Doctor dues and payout queries explicitly include only individual sessions, so historical subscription commission snapshots cannot create liabilities.

## Completed stages

1. **Schema and Rust contract — complete**
   - Rebuilt subscription tables and row/resource contracts around fixed sessions and manual price.
   - Added refund allocations and removed monthly, end-date, sponsor, schedule, stop/reactivate, discount, and stale financial snapshot fields.
2. **Session consumption and cancellation — complete**
   - Added covered-service selection, selectable session date, row locking, one-session consumption, idempotent restoration, and zero financial/commission effects.
3. **Financial integration — complete**
   - Added ledger-derived received/refunded/outstanding/retained totals, debt-allocation attribution, explicit cancellation refunds, debt reconciliation, and generic-refund protection.
4. **Vue workflow — complete**
   - Rebuilt create, list, details, correction, cancellation, session, patient, dashboard, and status views for the unified model.
5. **Reports, exports, dashboard, and mocks — complete**
   - Reports use actual payments/refunds, filter nested supervised sessions correctly, and report supervision separately from doctor commission.
   - Mock API now matches the Rust response and lifecycle contracts.
6. **Tests and regression verification — complete**
   - Added fixed-session financial lifecycle, explicit-refund, generic-refund rejection, allowance restoration, and concurrent over-consumption tests.
   - Removed obsolete Laravel-schema compatibility tests.
7. **First-run schema regression — complete**
   - Added a uniquely named Rust migration that upgrades databases which already recorded the original subscriptions-table migration.
   - The migration backfills required subscription names and manual total prices, normalizes lifecycle status, removes obsolete monthly/legacy columns, and updates covered-service rows to the fixed-session contract.
   - Fresh initialization and legacy-schema upgrade now both validate `subscriptions.name` as a required `VARCHAR(255)` column.
8. **Session-number search — complete**
   - Session list search now matches the displayed session code as well as patient name and phone, including numeric input such as `1005` for `S-1005`.

## Main modified areas

- Rust: `src-tauri/src/api/schema.rs`, subscription/session/ledger/report services and handlers, loaders/resources/rows, dashboard, exports, routing, runtime config, and HTTP tests.
- Vue: subscription dialogs/views/API, session creation/details, patient/dashboard/report views, status/label/audit/report constants.
- Verification: `src/mocks/data.js`, `src/mocks/handlers.js`, `scripts/verify-mock.mjs`, `scripts/verify-real-api.mjs`, and `scripts/verify-ui.mjs`.
- Deleted obsolete `src/constants/subscriptions.js` schedule helpers.

## Tests executed

- `cargo check` — passed.
- `cargo test --no-run` — passed.
- `cargo test --lib` — 13 passed, 14 ignored because they require isolated MySQL, local MySQL, or visual artifact generation.
- `npm run build` — passed.
- `npm run lint` — passed.
- `npm run verify:ui` — 15 checks passed.
- `npm run verify:mock` — passed full subscription and individual-finance walkthrough.
- `npm run verify:form-flow` — 8 checks passed.
- `node --check scripts/verify-real-api.mjs` — passed.
- `git diff --check` — passed.
- Fresh-schema and recorded-legacy-schema MySQL integration tests — passed.
- First-run Rust activation followed by fixed-session subscription creation, payment, and debt creation — passed.
- Subscription financial lifecycle through the Rust API — passed.
- Full fresh MySQL activation, backup, restore, and Rust API restart round trip — passed.
- Session-number search through the Rust API and mock API — passed.

## Environment note

The database integration checks were executed against an isolated MySQL 8.4 server and database under `/tmp`; no application database was modified.

## Existing worktree note

The session began with unrelated uncommitted files and a legacy Laravel migration. The migration was later reviewed as obsolete and removed with the retired Laravel source. The development app data backup remains at `~/.local/share/com.ettizan.clinic-before-first-run-backup-test-20261106`.
