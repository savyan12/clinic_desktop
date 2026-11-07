# Subscription Financial Repair Progress

## Overall objectives

Stabilize the Rust/Tauri subscription financial lifecycle without losing historical data or breaking individual-session behavior. Each repair is implemented and verified independently in the strict order defined by `docs/اصلاحات.md`.

## Ordered repair items

1. **Completed** — Isolate subscription sessions from individual-session financial workflows.
2. **Completed** — Treat the session doctor as a subscription-session supervisor with no new commission.
3. **Completed** — Prohibit adding services to subscription sessions.
4. **Pending, requirements updated** — Align the subscription contract with the confirmed name, fixed session count, total price, start date, and used-session history model.
5. **Pending** — Enforce one session per subscription per attendance date.
6. **Pending** — Remove subscription pause/stop functionality while preserving history.
7. **Pending** — Correct refund and payment reconciliation.
8. **Pending** — Correct subscription paid and remaining amounts.
9. **Pending** — Protect subscription lifecycle and historical records.
10. **Pending** — Add final database integrity protections and consistency checks.

## Current repair item

### Repair 4 — Align the Subscription Contract Model

Repair 3 is complete and verified. Repair 4 is next and has not been started in this execution cycle, in accordance with the required one-item workflow. It will replace the date-ended monthly model with the confirmed fixed-session contract: subscription name, session count, total price, start date, and completion after the final session.

Confirmed subscription rules received on 2026-11-06:

- A subscription has a name, fixed session count, total price, and start date.
- Used sessions are recorded against the subscription.
- A supervising doctor is selected separately for each subscription session.
- The supervising doctor receives no commission from a subscription session.
- The doctor remains visible in session details and subscription reports as operational supervision data.
- A subscription does not have selected attendance weekdays. It can be used on any valid day allowed by the subscription period or remaining-session rules.

### Attendance Weekday Removal — Completed

This focused business-rule adjustment was completed before starting Repair 3:

- Removed attendance-day selection, validation, previews, and correction controls from the Vue subscription UI.
- Removed weekday labels from subscription lists, details, and the new-session subscription selector.
- New monthly subscription requests no longer send or require `scheduled_weekdays`.
- Monthly usability now depends only on active status and the start/end validity range; any day inside that range is accepted.
- Patient subscription filters and usable-subscription loaders no longer query the stored weekday JSON.
- Monthly cancellation previews now count all calendar days inclusively.
- Mock behavior and regression coverage were updated to match the Rust API.
- The existing `scheduled_weekdays` database column and Rust row field remain in place only for backward compatibility and historical audit data. New records store `NULL`, and operational API/report resources expose no weekday schedule.

Files modified for the attendance-weekday removal:

- `src/components/subscriptions/CreateSubscriptionDialog.vue`
- `src/views/subscriptions/SubscriptionDetailsView.vue`
- `src/views/subscriptions/SubscriptionsView.vue`
- `src/components/sessions/NewSessionDialog.vue`
- `src-tauri/src/api/handlers/subscriptions.rs`
- `src-tauri/src/api/services/subscriptions.rs`
- `src-tauri/src/api/services/sessions.rs`
- `src-tauri/src/api/resources.rs`
- `src-tauri/src/api/loaders.rs`
- `src-tauri/src/api/handlers/patients.rs`
- `src-tauri/src/api/services/reports.rs`
- `src-tauri/src/api/http_tests.rs`
- `src/mocks/data.js`
- `src/mocks/handlers.js`
- `PROGRESS.md`

Verification on 2026-11-06:

- `cargo fmt --check`: passed.
- `cargo test --lib`: passed; 13 passed, 0 failed, 14 ignored for documented integration requirements.
- `npm run lint`: passed with zero warnings.
- `npm run build`: passed; only the existing Vite large-chunk advisory was emitted.
- `npm run verify:mock`: passed the complete mock walkthrough.
- `git diff --check`: passed.
- No PHP/Laravel file was read or modified.

## Completed items

### Repair 3 — Prohibit Adding Services to Subscription Sessions

Implemented changes:

- The Rust service-addition endpoint now rejects every subscription session before parsing or writing an addition, whether the session is open or closed.
- Updating an open subscription session now rejects `services`, `service_ids`, and changing `subscription_id`, preventing service mutation through the generic session-update path.
- Closed subscription-session correction remains restricted to changing the supervising doctor only.
- The Vue session-details page no longer shows or mounts the add-services dialog for subscription sessions.
- Historical subscription-session additions remain visible and are not deleted or rewritten.
- Individual closed sessions retain their existing service-addition workflow, settlement effects, and doctor-commission adjustment.
- Frontend mocks now enforce the same subscription prohibition and include a working individual-session addition path.

Files modified for Repair 3:

- `src-tauri/src/api/services/sessions.rs`
- `src-tauri/src/api/http_tests.rs`
- `src/views/sessions/SessionDetailsView.vue`
- `src/mocks/handlers.js`
- `scripts/verify-mock.mjs`
- `PROGRESS.md`

Database or migration changes for Repair 3:

- None. Historical additions and service snapshots were preserved.
- No PHP/Laravel file was read or modified.

Repair 3 verification on 2026-11-06:

- `cargo fmt --check`: passed.
- `cargo test --lib`: passed; 14 passed, 0 failed, 14 ignored for documented integration requirements.
- New unit test `subscription_session_rejects_service_mutation_payloads`: passed.
- MySQL assertions were added for rejecting a post-close subscription-session addition and preserving zero addition rows; they remain ignored without an isolated MySQL test database.
- The existing isolated-MySQL individual-session addition lifecycle coverage remains unchanged and ignored for the same infrastructure requirement.
- `npm run lint`: passed with zero warnings.
- `npm run build`: passed; only the existing Vite large-chunk advisory was emitted.
- `npm run verify:mock`: passed, including rejection of open and closed subscription-session service mutations and successful individual-session service addition.
- `git diff --check`: passed.

### Repair 2 — Subscription Session Doctor Is Supervisory Only

The former subscription-level commission proposal was superseded by the confirmed business rule above. There is no subscription or subscription-session commission. The doctor selected for a subscription session is a supervising doctor only.

Implemented changes:

- Closing a new subscription session now stores `NULL` for its commission rate, basis, and amount.
- Individual-session commission calculation and snapshots remain unchanged.
- Adding data to, cancelling, or correcting a subscription session does not generate or recalculate a commission; changing the doctor updates only the supervising doctor.
- Existing non-null historical commission snapshots are not deleted. Doctor earning and payout reconciliation continues to use recorded non-null snapshots so prior payouts cannot become inconsistent or negative merely because the policy changed.
- Session details identify the subscription-session doctor as the supervising doctor and show that no commission is created. A pre-existing historical commission, if present, is labelled explicitly as historical.
- Subscription report rows now include each closed subscription session and its supervising doctor. Reports can be filtered by supervising doctor.
- PDF/XLSX subscription exports include the session-to-supervising-doctor mapping.
- Frontend mock behavior follows the same commission and reporting rules.

Files modified for Repair 2:

- `src-tauri/src/api/services/sessions.rs`
- `src-tauri/src/api/handlers/dashboard.rs`
- `src-tauri/src/api/handlers/doctors.rs`
- `src-tauri/src/api/services/ledger.rs`
- `src-tauri/src/api/services/reports.rs`
- `src-tauri/src/api/exports.rs`
- `src-tauri/src/api/http_tests.rs`
- `src/constants/reports.js`
- `src/views/reports/ReportsView.vue`
- `src/views/sessions/SessionDetailsView.vue`
- `src/mocks/data.js`
- `src/mocks/handlers.js`
- `PROGRESS.md`

Database or migration changes for Repair 2:

- None. Existing historical financial rows were not rewritten.
- No PHP/Laravel file was read or modified.

Repair 2 tests executed on 2026-11-06:

- `cargo fmt --check`: passed.
- `cargo test --lib`: passed; 15 passed, 0 failed, 14 ignored for documented isolated-MySQL/local-infrastructure requirements.
- New unit test `subscription_sessions_do_not_create_doctor_commission`: passed.
- Export coverage for subscription-session supervising doctors: passed.
- `npm run lint`: passed with zero warnings.
- `npm run build`: passed; only the existing Vite large-chunk advisory was emitted.
- `npm run verify:mock`: passed the complete mock walkthrough.
- `git diff --check`: passed.

MySQL integration assertions were added for null subscription-session commissions, unchanged individual commission totals, and supervising-doctor report output. They remain ignored because no isolated `RUST_MYSQL_*` database is configured.

### Repair 1 — Isolate Subscription Sessions from Individual Sessions

Initial inspection found that the close path already avoids calling the individual settlement writer for a subscription session. However:

- Direct API payloads can still contain individual settlement fields and those fields are silently ignored instead of being rejected.
- Revenue summaries add `sessions.net_total` for all closed sessions, which treats the nominal value of subscription-session services as new service revenue.
- The session type and nullable `subscription_id` relationship are enforced during normal creation, but financial operations do not consistently assert that invariant when loading an existing row.
- Doctor commission is still generated from each subscription session. This is intentionally deferred to Repair 2 because the ordered plan requires the replacement subscription-level commission policy to be implemented separately.

Implemented changes:

- Subscription-session requests now reject any independent settlement object and reject non-zero discounts.
- Open sessions cannot be converted between individual and subscription operational types.
- Individual-session requests reject a non-null `subscription_id`.
- Existing session rows are checked before update, close, service-addition, cancellation, and correction operations: subscription sessions must have a parent subscription and individual sessions must not.
- Subscription-session close continues to create only the appropriate subscription usage effect and does not call individual settlement creation.
- Dashboard and session-report service revenue totals now count closed individual sessions only. Subscription-session service snapshots remain visible as nominal/historical values.
- Frontend mock revenue totals now use the same meaning as the Rust backend.
- Regression tests cover direct settlement rejection and verify that subscription sessions create no payment, debt, or wallet transaction.

## Files modified by completed item

- `src-tauri/src/api/services/sessions.rs`
- `src-tauri/src/api/handlers/dashboard.rs`
- `src-tauri/src/api/services/reports.rs`
- `src-tauri/src/api/http_tests.rs`
- `src/mocks/handlers.js`
- `PROGRESS.md`

These files already contained unrelated uncommitted user changes. The Repair 1 edits were kept targeted and did not discard those changes.

## Database or migration changes

None for Repair 1. No PHP/Laravel file or migration was used or modified.

## Tests executed

- `cargo fmt --check`: passed after applying `cargo fmt` to the Rust changes.
- `cargo test --lib`: passed; 14 tests passed, 0 failed, and 14 were ignored because they require isolated MySQL databases or local integration infrastructure.
- New non-database unit test `subscription_session_rejects_individual_financial_payloads`: passed.
- `npm run lint`: passed with zero warnings.
- `npm run build`: passed. Vite emitted only the existing large-chunk advisory.
- `git diff --check`: passed.

Re-verification on 2026-11-06 after resuming from `docs/اصلاحات.md`:

- Inspected the actual Repair 1 paths for session creation/update/close/cancellation/correction, settlement creation, subscription usage, dashboard totals, and session-report totals.
- `cargo test --lib`: passed again; 14 passed, 0 failed, 14 ignored for the documented isolated-MySQL/local-infrastructure requirements.
- `npm run lint`: passed again with zero warnings.
- `npm run build`: passed again; only the existing Vite large-chunk advisory was emitted.
- `git diff --check`: passed before this documentation-only update.
- No PHP/Laravel file was read or modified during this resumed verification.

The new MySQL regression assertions were added but could not be executed because no `RUST_MYSQL_*` isolated test environment is configured in this workspace.

## Known issues and unresolved decisions

- Historical non-null subscription-session commission snapshots remain part of historical doctor earnings to keep already-recorded payouts reconcilable. Removing them retroactively would require an explicit financial reconciliation policy and is not part of this repair.
- The workspace still contains pre-existing monthly schedules and service-selection changes that do not match the newly confirmed fixed-session subscription model. Subscription naming and the full contract-model alignment remain pending under Repair 4; they were not mixed into Repair 2.
- The isolated MySQL integration tests require a database named with the `clinic_rust_test_*` prefix and may remain ignored when that environment is unavailable.

## Risks and incomplete work

- The MySQL integration assertions for financial effects, commission totals, and subscription supervision reporting still need execution in an isolated database before production release.
- Repair 3 and all later items remain pending and were not started in this execution cycle.
