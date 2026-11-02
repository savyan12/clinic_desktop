<?php

namespace Tests\Feature;

use App\Models\Doctor;
use App\Models\Patient;
use App\Models\Service;
use App\Models\Session;
use App\Models\User;
use Illuminate\Foundation\Testing\LazilyRefreshDatabase;
use Illuminate\Support\Str;
use Laravel\Sanctum\Sanctum;
use Tests\TestCase;

class DoctorPayoutApiTest extends TestCase
{
    use LazilyRefreshDatabase;

    public function test_commission_rate_and_base_are_snapshotted_and_rate_change_affects_only_future_sessions(): void
    {
        $context = $this->context();
        Sanctum::actingAs($context['receptionist']);
        $firstSessionId = $this->closeCashSession($context);
        $this->assertDatabaseHas('sessions', [
            'id' => $firstSessionId,
            'commission_rate' => 30,
            'commission_base_amount' => 80,
            'commission_amount' => 24,
        ]);
        Sanctum::actingAs($context['admin']);

        $this->patchJson("/api/v1/doctors/{$context['doctor']->id}", [
            'name' => $context['doctor']->name,
            'phone' => $context['doctor']->phone,
            'commission_rate' => '50.000',
            'is_active' => true,
            'reason' => 'اتفاق جديد',
        ])->assertOk();

        $this->withHeader('Idempotency-Key', (string) Str::uuid())
            ->postJson("/api/v1/sessions/{$firstSessionId}/correct", [
                'changes' => [
                    'discount' => '10.000',
                    'settlement' => [
                        'paid_amount' => '70.000',
                        'wallet_amount' => '0.000',
                        'debt_amount' => '0.000',
                        'method' => 'cash',
                    ],
                ],
                'reason' => 'تصحيح الخصم بعد تغيير النسبة',
            ])->assertOk()
            ->assertJsonPath('data.session.commission_rate', '30.000')
            ->assertJsonPath('data.session.commission_amount', '21.000');
        Sanctum::actingAs($context['receptionist']);
        $secondSessionId = $this->closeCashSession($context);

        $this->assertSame('21.000', Session::query()->findOrFail($firstSessionId)->commission_amount);
        $this->assertSame('40.000', Session::query()->findOrFail($secondSessionId)->commission_amount);
    }

    public function test_payout_reduces_remaining_and_cannot_exceed_earned_due(): void
    {
        $context = $this->context();
        Sanctum::actingAs($context['receptionist']);
        $this->closeCashSession($context);

        $this->withHeader('Idempotency-Key', (string) Str::uuid())
            ->postJson("/api/v1/doctors/{$context['doctor']->id}/payouts", [
                'amount' => '5.000', 'date' => today()->toDateString(), 'method' => 'cash',
            ])->assertForbidden();
        Sanctum::actingAs($context['admin']);

        $this->withHeader('Idempotency-Key', (string) Str::uuid())
            ->postJson("/api/v1/doctors/{$context['doctor']->id}/payouts", [
                'amount' => '10.000', 'date' => today()->toDateString(), 'method' => 'transfer', 'note' => 'دفعة أولى',
            ])->assertCreated();
        $this->getJson("/api/v1/doctors/{$context['doctor']->id}/dues")
            ->assertOk()
            ->assertJsonPath('data.all_time_remaining', '14.000');

        $this->withHeader('Idempotency-Key', (string) Str::uuid())
            ->postJson("/api/v1/doctors/{$context['doctor']->id}/payouts", [
                'amount' => '15.000', 'date' => today()->toDateString(), 'method' => 'cash',
            ])->assertUnprocessable();
        $this->assertDatabaseCount('doctor_payouts', 1);
    }

    /** @return array<string, mixed> */
    private function context(): array
    {
        $admin = User::factory()->admin()->create();
        $receptionist = User::factory()->create();
        $doctor = Doctor::factory()->create(['commission_rate' => '30.000', 'created_by' => $admin->id]);
        $patient = Patient::factory()->create(['created_by' => $admin->id]);
        $service = Service::factory()->create(['price' => '80.000', 'created_by' => $admin->id]);

        return compact('admin', 'receptionist', 'doctor', 'patient', 'service');
    }

    private function closeCashSession(array $context): int
    {
        return $this->withHeader('Idempotency-Key', (string) Str::uuid())
            ->postJson('/api/v1/sessions', [
                'patient_id' => $context['patient']->id,
                'doctor_id' => $context['doctor']->id,
                'type' => 'individual',
                'service_ids' => [$context['service']->id],
                'discount' => '0.000',
                'settlement' => [
                    'paid_amount' => '80.000', 'wallet_amount' => '0.000', 'debt_amount' => '0.000', 'method' => 'cash',
                ],
            ])->assertCreated()->json('data.session.id');
    }
}
