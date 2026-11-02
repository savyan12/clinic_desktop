<?php

namespace Tests\Feature;

use App\Enums\PaymentMethod;
use App\Enums\PaymentType;
use App\Enums\SessionStatus;
use App\Models\Doctor;
use App\Models\Patient;
use App\Models\Payment;
use App\Models\Service;
use App\Models\Session;
use App\Models\SessionAddition;
use App\Models\User;
use App\Services\WalletService;
use App\Support\Money;
use Illuminate\Foundation\Testing\LazilyRefreshDatabase;
use Illuminate\Support\Str;
use Illuminate\Testing\TestResponse;
use Laravel\Sanctum\Sanctum;
use Tests\TestCase;

class SessionLifecycleApiTest extends TestCase
{
    use LazilyRefreshDatabase;

    public function test_new_session_is_closed_immediately_and_financial_effects_are_atomic(): void
    {
        $context = $this->context();
        Sanctum::actingAs($context['receptionist']);

        $response = $this->createIndividualSession($context, [
            'paid_amount' => '80.000',
            'wallet_amount' => '0.000',
            'debt_amount' => '0.000',
            'method' => 'cash',
        ]);

        $response->assertCreated()
            ->assertJsonPath('data.session.status', SessionStatus::Closed->value)
            ->assertJsonPath('data.session.closed_by', $context['receptionist']->id)
            ->assertJsonPath('data.effects.payment.amount', '80.000')
            ->assertJsonPath('data.replayed', false);

        $sessionId = $response->json('data.session.id');
        $this->assertDatabaseHas('sessions', ['id' => $sessionId, 'status' => SessionStatus::Closed->value]);
        $this->assertDatabaseHas('payments', [
            'session_id' => $sessionId,
            'type' => PaymentType::SessionPayment->value,
            'amount' => 80,
        ]);
        $this->assertDatabaseHas('audit_logs', ['action' => 'session_closed', 'entity_id' => $sessionId]);
    }

    public function test_service_price_snapshot_remains_immutable_after_master_price_change(): void
    {
        $context = $this->context();
        Sanctum::actingAs($context['receptionist']);
        $sessionId = $this->createIndividualSession($context, [
            'paid_amount' => '80.000',
            'wallet_amount' => '0.000',
            'debt_amount' => '0.000',
            'method' => 'cash',
        ])->json('data.session.id');

        Sanctum::actingAs($context['admin']);
        $this->patchJson("/api/v1/services/{$context['service']->id}", [
            'name' => $context['service']->name,
            'price' => '120.000',
            'included_units' => null,
            'unit_name' => null,
            'extra_unit_price' => null,
            'is_active' => true,
            'reason' => 'تحديث قائمة الأسعار',
        ])->assertOk();

        $this->getJson("/api/v1/sessions/{$sessionId}")
            ->assertOk()
            ->assertJsonPath('data.services.0.price', '80.000')
            ->assertJsonPath('data.net_total', '80.000');
    }

    public function test_session_total_includes_configured_service_extra_units(): void
    {
        $context = $this->context();
        $cupping = Service::factory()->create([
            'name' => 'الحجامة',
            'price' => '30.000',
            'included_units' => 7,
            'unit_name' => 'كوب',
            'extra_unit_price' => '5.000',
            'created_by' => $context['admin']->id,
        ]);
        Sanctum::actingAs($context['receptionist']);

        $this->withHeader('Idempotency-Key', (string) Str::uuid())
            ->postJson('/api/v1/sessions', [
                'patient_id' => $context['patient']->id,
                'doctor_id' => $context['doctor']->id,
                'type' => 'individual',
                'services' => [['service_id' => $cupping->id, 'extra_units' => 2]],
                'discount' => '0.000',
                'settlement' => [
                    'paid_amount' => '40.000',
                    'wallet_amount' => '0.000',
                    'debt_amount' => '0.000',
                    'method' => 'cash',
                ],
            ])->assertCreated()
            ->assertJsonPath('data.session.services_total', '40.000')
            ->assertJsonPath('data.session.services.0.base_price', '30.000')
            ->assertJsonPath('data.session.services.0.included_units', 7)
            ->assertJsonPath('data.session.services.0.extra_units', 2)
            ->assertJsonPath('data.session.services.0.total_units', 9)
            ->assertJsonPath('data.session.services.0.unit_name', 'كوب');
    }

    public function test_session_creation_is_idempotent_for_the_same_key_and_payload(): void
    {
        $context = $this->context();
        Sanctum::actingAs($context['receptionist']);
        $key = (string) Str::uuid();
        $payload = $this->individualPayload($context, [
            'paid_amount' => '80.000',
            'wallet_amount' => '0.000',
            'debt_amount' => '0.000',
            'method' => 'cash',
        ]);

        $first = $this->withHeader('Idempotency-Key', $key)->postJson('/api/v1/sessions', $payload);
        $second = $this->withHeader('Idempotency-Key', $key)->postJson('/api/v1/sessions', $payload);

        $first->assertCreated();
        $second->assertCreated()->assertExactJson($first->json());
        $this->assertDatabaseCount('sessions', 1);
        $this->assertDatabaseCount('payments', 1);
    }

    public function test_insufficient_wallet_returns_422_without_creating_session_or_debit(): void
    {
        $context = $this->context();
        app(WalletService::class)->topUp($context['patient'], '10.000', PaymentMethod::Cash, null, $context['receptionist']);
        Sanctum::actingAs($context['receptionist']);

        $this->withHeader('Idempotency-Key', (string) Str::uuid())
            ->postJson('/api/v1/sessions', $this->individualPayload($context, [
                'paid_amount' => '0.000',
                'wallet_amount' => '80.000',
                'debt_amount' => '0.000',
            ]))
            ->assertUnprocessable()
            ->assertJsonPath('message', 'الرصيد غير كافٍ. اشحن رصيد المريض أولًا ثم أعد المحاولة.');

        $this->assertDatabaseCount('sessions', 0);
        $this->assertDatabaseCount('wallet_transactions', 1);
    }

    public function test_receptionist_cannot_edit_or_cancel_closed_session_and_admin_correction_is_audited(): void
    {
        $context = $this->context();
        Sanctum::actingAs($context['receptionist']);
        $sessionId = $this->createIndividualSession($context, [
            'paid_amount' => '80.000',
            'wallet_amount' => '0.000',
            'debt_amount' => '0.000',
            'method' => 'cash',
        ])->json('data.session.id');

        $this->patchJson("/api/v1/sessions/{$sessionId}", $this->individualPayload($context, [
            'paid_amount' => '80.000',
            'wallet_amount' => '0.000',
            'debt_amount' => '0.000',
            'method' => 'cash',
        ]))->assertForbidden();
        $this->withHeader('Idempotency-Key', (string) Str::uuid())
            ->postJson("/api/v1/sessions/{$sessionId}/cancel", ['reason' => 'محاولة غير مسموحة'])
            ->assertForbidden();

        Sanctum::actingAs($context['admin']);
        $this->withHeader('Idempotency-Key', (string) Str::uuid())
            ->postJson("/api/v1/sessions/{$sessionId}/correct", [
                'changes' => [
                    'discount' => '10.000',
                    'settlement' => [
                        'paid_amount' => '70.000',
                        'wallet_amount' => '0.000',
                        'debt_amount' => '0.000',
                        'method' => 'cash',
                    ],
                ],
                'reason' => 'تصحيح خصم معتمد',
            ])->assertOk()
            ->assertJsonPath('data.session.net_total', '70.000')
            ->assertJsonPath('data.financial_impact.net_delta', '-10.000');

        $this->assertDatabaseHas('audit_logs', [
            'action' => 'session_corrected',
            'entity_type' => 'session',
            'entity_id' => $sessionId,
        ]);
    }

    public function test_receptionist_can_append_services_and_extra_units_to_closed_session_once(): void
    {
        $context = $this->context();
        $cupping = Service::factory()->create([
            'name' => 'الحجامة',
            'price' => '30.000',
            'included_units' => 7,
            'unit_name' => 'كوب',
            'extra_unit_price' => '5.000',
            'created_by' => $context['admin']->id,
        ]);
        Sanctum::actingAs($context['receptionist']);
        $sessionId = $this->createIndividualSession($context, [
            'paid_amount' => '80.000',
            'wallet_amount' => '0.000',
            'debt_amount' => '0.000',
            'method' => 'cash',
        ])->json('data.session.id');
        $key = (string) Str::uuid();
        $payload = [
            'services' => [[
                'service_id' => $cupping->id,
                'item_type' => 'extra_units',
                'extra_units' => 3,
            ]],
            'settlement' => [
                'paid_amount' => '15.000',
                'wallet_amount' => '0.000',
                'debt_amount' => '0.000',
                'method' => 'cash',
            ],
        ];

        $first = $this->withHeader('Idempotency-Key', $key)
            ->postJson("/api/v1/sessions/{$sessionId}/services", $payload)
            ->assertCreated()
            ->assertJsonPath('data.session.net_total', '95.000')
            ->assertJsonPath('data.session.services.1.item_type', 'extra_units')
            ->assertJsonPath('data.session.services.1.extra_units', 3)
            ->assertJsonPath('data.addition.net_total', '15.000');
        $this->withHeader('Idempotency-Key', $key)
            ->postJson("/api/v1/sessions/{$sessionId}/services", $payload)
            ->assertCreated()
            ->assertExactJson($first->json());

        $this->assertSame(1, SessionAddition::query()->where('session_id', $sessionId)->count());
        $this->assertSame(1, Payment::query()
            ->where('session_id', $sessionId)
            ->where('type', PaymentType::Adjustment)
            ->count());
        $this->assertSame('28.500', Session::query()->findOrFail($sessionId)->commission_amount);
    }

    public function test_admin_cancel_closed_wallet_session_restores_balance_and_removes_due(): void
    {
        $context = $this->context();
        $walletService = app(WalletService::class);
        $walletService->topUp($context['patient'], '100.000', PaymentMethod::Cash, null, $context['receptionist']);
        Sanctum::actingAs($context['receptionist']);
        $sessionId = $this->createIndividualSession($context, [
            'paid_amount' => '0.000',
            'wallet_amount' => '80.000',
            'debt_amount' => '0.000',
        ])->json('data.session.id');
        $this->assertSame('20.000', $walletService->balance($context['patient']));

        Sanctum::actingAs($context['admin']);
        $this->withHeader('Idempotency-Key', (string) Str::uuid())
            ->postJson("/api/v1/sessions/{$sessionId}/cancel", ['reason' => 'إلغاء إداري'])
            ->assertOk()
            ->assertJsonPath('data.session.status', SessionStatus::Cancelled->value)
            ->assertJsonPath('data.session.commission_amount', null)
            ->assertJsonPath('data.reversals.commission_removed.amount', '24.000');

        $this->assertSame('100.000', $walletService->balance($context['patient']));
        $this->assertSame('0.000', Money::normalize(Session::query()->where('status', SessionStatus::Closed)->sum('commission_amount')));
    }

    /** @return array{admin: User, receptionist: User, doctor: Doctor, patient: Patient, service: Service} */
    private function context(): array
    {
        $admin = User::factory()->admin()->create();
        $receptionist = User::factory()->create();
        $doctor = Doctor::factory()->create(['commission_rate' => '30.000', 'created_by' => $admin->id]);
        $patient = Patient::factory()->create(['created_by' => $admin->id]);
        $service = Service::factory()->create(['price' => '80.000', 'created_by' => $admin->id]);

        return compact('admin', 'receptionist', 'doctor', 'patient', 'service');
    }

    /** @param array<string, string> $settlement */
    private function createIndividualSession(array $context, array $settlement): TestResponse
    {
        return $this->withHeader('Idempotency-Key', (string) Str::uuid())
            ->postJson('/api/v1/sessions', $this->individualPayload($context, $settlement));
    }

    /** @param array<string, string> $settlement */
    private function individualPayload(array $context, array $settlement): array
    {
        return [
            'patient_id' => $context['patient']->id,
            'doctor_id' => $context['doctor']->id,
            'type' => 'individual',
            'service_ids' => [$context['service']->id],
            'discount' => '0.000',
            'settlement' => $settlement,
        ];
    }
}
