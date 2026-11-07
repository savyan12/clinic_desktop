# Subscription System Analysis

The desktop app runs **Vue → Rust/Axum API → local MySQL**. The Rust implementation is authoritative; the retired Laravel source has been removed. This review covers the working tree at the time of inspection, including existing uncommitted edits; database findings come from schema, migrations, and queries, not live records.

## 1. Creation and management

The frontend creates **monthly subscriptions** using a patient, fixed amount, covered services, start/end dates, initial payment, and optional sponsor. The API also accepts **legacy packages**, priced as the selected service prices × session count − discount. Service names and prices are saved as snapshots.

Administrators can stop, reactivate, cancel, or correct subscriptions. Corrections allow the end date and sponsor; sponsor changes are blocked once any subscription debt exists. Stopping changes status without extending dates or changing finances. Monthly subscriptions become completed after their end date when certain endpoints run; legacy packages complete when their allowance is consumed.

The code does not enforce a calendar-month duration, full payment before use, or one active subscription per patient.

## 2. Session recording and tracking

The session form selects the patient, subscription, supervising doctor, and—on monthly plans—covered services. Saving creates and immediately closes the session.

- **Monthly:** Visits are stored in `sessions` and `session_services`, linked through `subscription_id`. Today must fall within the subscription dates. No `subscription_usages` entry or allowance decrement occurs. No daily visit limit is implemented.
- **Legacy:** Each closed visit records a `consume` entry and increments `sessions_used`. Cancelling that visit records a linked `restore` entry and decrements the counter.

Subscription sessions cannot receive later service additions; closed-session corrections are limited to changing the doctor. Monthly session values use current service prices, while legacy session values divide the package’s net price by its session count.

## 3. Payments and doctor commissions

Initial receipts become `subscription_payment` records, using cash or transfer. Any unpaid balance becomes a subscription debt, optionally assigned to a sponsor. Later payments settle debts through `payment_allocations`.

Subscription visits themselves create **no additional receipt, wallet deduction, debt, or commission** in the current Rust implementation.

Cancellation calculates consumed value using inclusive elapsed calendar days for monthly plans—including today—or used sessions for legacy packages. It compares that value with actual payments, records any refund as a **negative cash payment**, voids outstanding original debt, and creates replacement debt if consumed value exceeds payments.

New subscription visits store commission fields as `NULL`. Individual visits snapshot the doctor’s percentage against net value by default, or gross value when configured. Doctor payouts are separate records, capped at earned commissions minus previous payouts. Historical subscription sessions with non-null commissions still qualify for dues calculations.

## 4. Responsible files, functions, tables, and reports

| Responsibility | Files and functions | Tables |
|---|---|---|
| Subscription UI | [CreateSubscriptionDialog.vue](../src/components/subscriptions/CreateSubscriptionDialog.vue), [SubscriptionDetailsView.vue](../src/views/subscriptions/SubscriptionDetailsView.vue), [subscriptions API wrapper](../src/api/subscriptions.js) | — |
| Subscription lifecycle | [handlers/subscriptions.rs](../src-tauri/src/api/handlers/subscriptions.rs): parsing, endpoints, `expire_monthly`; [services/subscriptions.rs](../src-tauri/src/api/services/subscriptions.rs): `create`, `stop`, `reactivate`, `correct`, `cancellation_state`, `cancel` | `subscriptions`, `subscription_services` |
| Session tracking | [NewSessionDialog.vue](../src/components/sessions/NewSessionDialog.vue); [services/sessions.rs](../src-tauri/src/api/services/sessions.rs): `create_and_close`, `consume_subscription`, `reverse_closed_effects` | `sessions`, `session_services`, `subscription_usages` |
| Money and payouts | [services/ledger.rs](../src-tauri/src/api/services/ledger.rs): `insert_payment`, `settle_debts`, `refund`, `doctor_payout` | `payments`, `debts`, `payment_allocations`, `doctor_payouts`, `wallet_transactions` |
| Reports and exports | [services/reports.rs](../src-tauri/src/api/services/reports.rs), [exports.rs](../src-tauri/src/api/exports.rs), [ReportsView.vue](../src/views/reports/ReportsView.vue), [report definitions](../src/constants/reports.js) | Subscription, session, payment, debt, and payout tables |
| Schema and safeguards | [schema.rs](../src-tauri/src/api/schema.rs), [idempotency.rs](../src-tauri/src/api/idempotency.rs), [audit.rs](../src-tauri/src/api/audit.rs) | Also `patients`, `services`, `doctors`, `sponsors`, `users`, `audit_logs`, `idempotency_keys`, `migrations` |

The subscription report filters by **subscription start date** and includes closed visits with their supervising doctors. Collections reports show receipts/refunds; debt reports show outstanding balances. The sessions report displays subscription session values but totals only closed individual sessions.

## 5. Bugs, inconsistencies, and unclear behavior

- **Displayed payment balances become stale.** Debt settlements update debts, but never `subscriptions.paid_amount`; `remaining_amount` remains original net price minus initial payment. Subscription details also omit later debt payments because they query only direct `payments.subscription_id` links. See the [resource calculation](../src-tauri/src/api/resources.rs#L124) and [payment query](../src-tauri/src/api/handlers/subscriptions.rs#L157).
- **Refund attribution has financial edge cases.** Cancellation refunds funded by multiple payments have no `reverses_payment_id`, so ordinary refund limits do not account for them. Conversely, refunds against a payment shared across several debts are subtracted in full from each subscription’s contribution, potentially understating another subscription’s paid total. See [cancellation calculations](../src-tauri/src/api/services/subscriptions.rs#L534) and [refund creation](../src-tauri/src/api/services/subscriptions.rs#L626).
- **The report’s “near expiry” filter has incompatible types.** The frontend sends `near_end=1`; Rust expects a boolean parsed as `true`/`false`, so the request is rejected. See the [frontend](../src/views/reports/ReportsView.vue#L82) and [backend](../src-tauri/src/api/handlers/reports.rs#L30).
- **Extending a completed monthly subscription does not restore usability.** Correction changes its date but leaves status `completed`; reactivation accepts only `stopped`. Reports also do not perform the expiry update themselves, allowing stale statuses. See [correction](../src-tauri/src/api/services/subscriptions.rs#L345).
- **Report labels can mislead.** Visible session values do not necessarily sum to the displayed total. Subscription date/doctor filters select subscriptions, but their nested visit lists still include all dates and doctors. These are actual query behaviors; the intended reporting rules are unclear.
- **Migration behavior differs from current scheduling behavior.** A migration assigns weekdays and adds every active service to existing monthly subscriptions. Current creation stores no schedule, responses return `scheduled_weekdays: null`, and usage ignores weekdays. Whether this historical coverage expansion was intended is unclear. See the [migration implementation](../src-tauri/src/api/schema.rs#L392).
The analysis was read-only. No application code or data was changed, and no tests or migrations were run. This Markdown file records that analysis without applying fixes or assuming intended business rules.
