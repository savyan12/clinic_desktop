<?php

namespace Tests\Feature;

use App\Models\Doctor;
use App\Models\Patient;
use App\Models\Payment;
use App\Models\Service;
use App\Models\Sponsor;
use App\Models\User;
use App\Models\WalletTransaction;
use App\Services\WalletService;
use Illuminate\Foundation\Testing\LazilyRefreshDatabase;
use Illuminate\Support\Str;
use Laravel\Sanctum\Sanctum;
use Tests\TestCase;

class WalletDebtApiTest extends TestCase
{
    use LazilyRefreshDatabase;

    public function test_wallet_topup_creates_receipt_and_movement_once_for_repeated_key(): void
    {
        [$admin, $receptionist, $patient] = $this->context();
        Sanctum::actingAs($receptionist);
        $key = (string) Str::uuid();
        $payload = ['amount' => '100.000', 'method' => 'cash', 'note' => 'شحن رصيد'];

        $first = $this->withHeader('Idempotency-Key', $key)
            ->postJson("/api/v1/patients/{$patient->id}/wallet/topups", $payload);
        $second = $this->withHeader('Idempotency-Key', $key)
            ->postJson("/api/v1/patients/{$patient->id}/wallet/topups", $payload);

        $first->assertCreated()->assertJsonPath('data.balance', '100.000');
        $second->assertCreated();
        $this->assertEquals($first->json(), $second->json());
        $this->assertDatabaseCount('wallet_transactions', 1);
        $this->assertDatabaseHas('payments', ['patient_id' => $patient->id, 'type' => 'wallet_topup', 'amount' => 100]);
        $this->assertDatabaseHas('audit_logs', ['action' => 'payment_recorded', 'entity_type' => 'payment']);
        $this->assertDatabaseHas('audit_logs', ['action' => 'wallet_topped_up', 'entity_type' => 'wallet_transaction']);
    }

    public function test_one_payment_can_settle_two_debts_partially_and_overallocation_is_rejected(): void
    {
        [$admin, $receptionist, $patient] = $this->context();
        $sponsor = Sponsor::factory()->create(['created_by' => $admin->id]);
        Sanctum::actingAs($admin);
        $firstDebtId = $this->withHeader('Idempotency-Key', (string) Str::uuid())
            ->postJson("/api/v1/patients/{$patient->id}/opening-debts", [
                'amount' => '120.000', 'sponsor_id' => $sponsor->id, 'note' => 'دين أول',
            ])->assertSuccessful()->json('data.id');
        $secondDebtId = $this->withHeader('Idempotency-Key', (string) Str::uuid())
            ->postJson("/api/v1/patients/{$patient->id}/opening-debts", [
                'amount' => '80.000', 'sponsor_id' => $sponsor->id, 'note' => 'دين ثان',
            ])->assertSuccessful()->json('data.id');
        Sanctum::actingAs($receptionist);

        $this->withHeader('Idempotency-Key', (string) Str::uuid())
            ->postJson('/api/v1/debts/settlements', [
                'amount' => '100.000',
                'method' => 'cash',
                'allocations' => [
                    ['debt_id' => $firstDebtId, 'amount' => '60.000'],
                    ['debt_id' => $secondDebtId, 'amount' => '40.000'],
                ],
            ])->assertCreated()->assertJsonCount(2, 'data.allocations');

        $this->assertDatabaseHas('debts', ['id' => $firstDebtId, 'paid_amount' => 60, 'status' => 'partial']);
        $this->assertDatabaseHas('debts', ['id' => $secondDebtId, 'paid_amount' => 40, 'status' => 'partial']);
        $paymentsBefore = Payment::query()->count();

        $this->withHeader('Idempotency-Key', (string) Str::uuid())
            ->postJson('/api/v1/debts/settlements', [
                'amount' => '70.000',
                'method' => 'cash',
                'allocations' => [['debt_id' => $firstDebtId, 'amount' => '70.000']],
            ])->assertUnprocessable()->assertJsonValidationErrors(['allocations.0.amount']);
        $this->assertSame($paymentsBefore, Payment::query()->count());
    }

    public function test_receptionist_cannot_refund_and_admin_refund_is_negative_and_audited(): void
    {
        [$admin, $receptionist, $patient] = $this->context();
        Sanctum::actingAs($receptionist);
        $paymentId = $this->withHeader('Idempotency-Key', (string) Str::uuid())
            ->postJson("/api/v1/patients/{$patient->id}/wallet/topups", [
                'amount' => '100.000', 'method' => 'cash',
            ])->assertCreated()->json('data.payment.id');
        $payload = [
            'patient_id' => $patient->id,
            'payment_id' => $paymentId,
            'amount' => '25.000',
            'method' => 'cash',
            'reason' => 'استرجاع معتمد',
        ];

        $this->withHeader('Idempotency-Key', (string) Str::uuid())
            ->postJson('/api/v1/refunds', $payload)
            ->assertForbidden();
        Sanctum::actingAs($admin);

        $this->withHeader('Idempotency-Key', (string) Str::uuid())
            ->postJson('/api/v1/refunds', $payload)
            ->assertCreated()
            ->assertJsonPath('data.amount', '-25.000');
        $this->assertDatabaseHas('payments', ['type' => 'refund', 'amount' => -25, 'reverses_payment_id' => $paymentId]);
        $this->assertDatabaseHas('audit_logs', ['action' => 'payment_refunded']);
    }

    public function test_refunding_a_wallet_topup_debits_the_prepaid_balance_in_the_same_operation(): void
    {
        [$admin, $receptionist, $patient] = $this->context();
        Sanctum::actingAs($receptionist);
        $paymentId = $this->withHeader('Idempotency-Key', (string) Str::uuid())
            ->postJson("/api/v1/patients/{$patient->id}/wallet/topups", [
                'amount' => '100.000', 'method' => 'cash',
            ])->assertCreated()->json('data.payment.id');
        Sanctum::actingAs($admin);

        $this->withHeader('Idempotency-Key', (string) Str::uuid())
            ->postJson('/api/v1/refunds', [
                'patient_id' => $patient->id,
                'payment_id' => $paymentId,
                'amount' => '100.000',
                'method' => 'cash',
                'reason' => 'استرجاع شحن الرصيد',
            ])->assertCreated()
            ->assertJsonPath('data.amount', '-100.000');

        $this->assertSame('0.000', app(WalletService::class)->balance($patient));
        $this->assertSame(2, WalletTransaction::query()->where('patient_id', $patient->id)->count());
        $this->assertDatabaseHas('wallet_transactions', [
            'patient_id' => $patient->id,
            'type' => 'debit',
            'amount' => -100,
            'balance_after' => 0,
        ]);
    }

    public function test_partial_wallet_topup_refund_is_idempotent_and_collections_count_only_the_payment_refund(): void
    {
        [$admin, $receptionist, $patient] = $this->context();
        Sanctum::actingAs($receptionist);
        $paymentId = $this->withHeader('Idempotency-Key', (string) Str::uuid())
            ->postJson("/api/v1/patients/{$patient->id}/wallet/topups", [
                'amount' => '100.000', 'method' => 'cash',
            ])->assertCreated()->json('data.payment.id');
        Sanctum::actingAs($admin);
        $key = (string) Str::uuid();
        $payload = [
            'patient_id' => $patient->id,
            'payment_id' => $paymentId,
            'amount' => '25.000',
            'method' => 'cash',
            'reason' => 'استرجاع جزئي',
        ];

        $first = $this->withHeader('Idempotency-Key', $key)->postJson('/api/v1/refunds', $payload);
        $second = $this->withHeader('Idempotency-Key', $key)->postJson('/api/v1/refunds', $payload);

        $first->assertCreated()->assertJsonPath('data.amount', '-25.000');
        $second->assertCreated();
        $this->assertSame($first->json(), $second->json());
        $this->assertSame('75.000', app(WalletService::class)->balance($patient));
        $this->assertSame(1, Payment::query()->where('type', 'refund')->where('reverses_payment_id', $paymentId)->count());
        $this->assertSame(2, WalletTransaction::query()->where('patient_id', $patient->id)->count());
        $this->assertDatabaseHas('audit_logs', ['action' => 'wallet_topup_refunded']);

        $this->getJson('/api/v1/reports/collections')
            ->assertOk()
            ->assertJsonPath('data.totals.received', '100.000')
            ->assertJsonPath('data.totals.refunds', '25.000')
            ->assertJsonPath('data.totals.net', '75.000');
    }

    public function test_wallet_topup_refund_cannot_exceed_current_wallet_balance_and_writes_nothing(): void
    {
        [$admin, $receptionist, $patient] = $this->context();
        $doctor = Doctor::factory()->create(['created_by' => $admin->id]);
        $service = Service::factory()->create(['price' => '60.000', 'created_by' => $admin->id]);
        Sanctum::actingAs($receptionist);
        $paymentId = $this->withHeader('Idempotency-Key', (string) Str::uuid())
            ->postJson("/api/v1/patients/{$patient->id}/wallet/topups", [
                'amount' => '100.000', 'method' => 'cash',
            ])->assertCreated()->json('data.payment.id');
        $this->withHeader('Idempotency-Key', (string) Str::uuid())
            ->postJson('/api/v1/sessions', [
                'patient_id' => $patient->id,
                'doctor_id' => $doctor->id,
                'type' => 'individual',
                'service_ids' => [$service->id],
                'discount' => '0.000',
                'settlement' => [
                    'wallet_amount' => '60.000',
                    'paid_amount' => '0.000',
                    'debt_amount' => '0.000',
                ],
            ])->assertCreated();
        $paymentCount = Payment::query()->count();
        $movementCount = WalletTransaction::query()->count();
        Sanctum::actingAs($admin);

        $this->withHeader('Idempotency-Key', (string) Str::uuid())
            ->postJson('/api/v1/refunds', [
                'patient_id' => $patient->id,
                'payment_id' => $paymentId,
                'amount' => '50.000',
                'method' => 'cash',
                'reason' => 'يتجاوز الرصيد الحالي',
            ])->assertUnprocessable()
            ->assertJsonPath('message', 'رصيد المريض الحالي لا يكفي لاسترجاع هذا المبلغ.')
            ->assertJsonValidationErrors(['amount']);

        $this->assertSame('40.000', app(WalletService::class)->balance($patient));
        $this->assertSame($paymentCount, Payment::query()->count());
        $this->assertSame($movementCount, WalletTransaction::query()->count());
    }

    public function test_wallet_topup_refund_repair_command_is_dry_run_by_default_and_idempotent_when_applied(): void
    {
        [$admin, $receptionist, $patient] = $this->context();
        Sanctum::actingAs($receptionist);
        $paymentId = $this->withHeader('Idempotency-Key', (string) Str::uuid())
            ->postJson("/api/v1/patients/{$patient->id}/wallet/topups", [
                'amount' => '100.000', 'method' => 'cash',
            ])->assertCreated()->json('data.payment.id');
        $refund = Payment::query()->create([
            'patient_id' => $patient->id,
            'type' => 'refund',
            'method' => 'cash',
            'amount' => '-100.000',
            'reverses_payment_id' => $paymentId,
            'note' => 'سجل قديم يحتاج إصلاحًا',
            'created_by' => $admin->id,
        ]);

        $this->artisan('clinic:repair-wallet-topup-refunds')
            ->expectsOutputToContain('Found 1 wallet top-up refund(s) missing wallet debit movements.')
            ->expectsOutputToContain('Dry run only.')
            ->assertSuccessful();
        $this->assertSame(1, WalletTransaction::query()->where('patient_id', $patient->id)->count());

        $this->artisan('clinic:repair-wallet-topup-refunds', ['--apply' => true])
            ->expectsOutputToContain('Applied 1 wallet debit repair(s).')
            ->assertSuccessful();
        $this->assertSame('0.000', app(WalletService::class)->balance($patient));
        $this->assertDatabaseHas('wallet_transactions', [
            'payment_id' => $refund->id,
            'type' => 'debit',
            'amount' => -100,
            'balance_after' => 0,
        ]);
        $this->assertDatabaseHas('audit_logs', [
            'action' => 'wallet_topup_refunded',
            'entity_type' => 'wallet_transaction',
        ]);

        $movementCount = WalletTransaction::query()->count();
        $this->artisan('clinic:repair-wallet-topup-refunds', ['--apply' => true])
            ->expectsOutput('No wallet top-up refunds are missing wallet debit movements.')
            ->assertSuccessful();
        $this->assertSame($movementCount, WalletTransaction::query()->count());
    }

    public function test_sponsor_statement_includes_patients_sessions_allocations_and_totals(): void
    {
        [$admin, $receptionist, $patient] = $this->context();
        $sponsor = Sponsor::factory()->create(['created_by' => $admin->id]);
        $doctor = Doctor::factory()->create(['created_by' => $admin->id]);
        $service = Service::factory()->create([
            'name' => 'علاج يدوي',
            'price' => '80.000',
            'created_by' => $admin->id,
        ]);
        Sanctum::actingAs($receptionist);
        $response = $this->withHeader('Idempotency-Key', (string) Str::uuid())
            ->postJson('/api/v1/sessions', [
                'patient_id' => $patient->id,
                'doctor_id' => $doctor->id,
                'type' => 'individual',
                'service_ids' => [$service->id],
                'discount' => '0.000',
                'settlement' => [
                    'paid_amount' => '0.000',
                    'wallet_amount' => '0.000',
                    'debt_amount' => '80.000',
                    'sponsor_id' => $sponsor->id,
                ],
            ])->assertCreated();
        $sessionId = $response->json('data.session.id');
        $debtId = $response
            ->json('data.effects.debt.id');
        $this->withHeader('Idempotency-Key', (string) Str::uuid())
            ->postJson('/api/v1/debts/settlements', [
                'amount' => '30.000',
                'method' => 'cash',
                'allocations' => [['debt_id' => $debtId, 'amount' => '30.000']],
            ])->assertCreated();
        Sanctum::actingAs($admin);

        $this->getJson("/api/v1/sponsors/{$sponsor->id}/statement")
            ->assertOk()
            ->assertJsonPath('data.patients.0.id', $patient->id)
            ->assertJsonMissingPath('data.sessions.0.doctor_id')
            ->assertJsonMissingPath('data.sessions.0.doctor')
            ->assertJsonPath('data.sessions.0.services.0.name', 'علاج يدوي')
            ->assertJsonPath('data.payments.0.type', 'debt_payment')
            ->assertJsonCount(1, 'data.allocations')
            ->assertJsonPath('data.totals.due', '80.000')
            ->assertJsonPath('data.totals.paid', '30.000')
            ->assertJsonPath('data.totals.remaining', '50.000');

        $this->getJson("/api/v1/sponsors/{$sponsor->id}")
            ->assertOk()
            ->assertJsonPath('data.total_due', '80.000')
            ->assertJsonPath('data.total_paid', '30.000')
            ->assertJsonPath('data.remaining', '50.000');

        $this->withHeader('Idempotency-Key', (string) Str::uuid())
            ->postJson("/api/v1/sessions/{$sessionId}/cancel", ['reason' => 'إلغاء إداري'])
            ->assertOk();

        $this->getJson("/api/v1/sponsors/{$sponsor->id}/statement")
            ->assertOk()
            ->assertJsonPath('data.totals.due', '0.000')
            ->assertJsonPath('data.totals.paid', '0.000')
            ->assertJsonPath('data.totals.remaining', '0.000');
    }

    /** @return array{User, User, Patient} */
    private function context(): array
    {
        $admin = User::factory()->admin()->create();
        $receptionist = User::factory()->create();
        $patient = Patient::factory()->create(['created_by' => $admin->id]);

        return [$admin, $receptionist, $patient];
    }
}
