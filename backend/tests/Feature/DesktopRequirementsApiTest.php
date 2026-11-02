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

class DesktopRequirementsApiTest extends TestCase
{
    use LazilyRefreshDatabase;

    public function test_patient_name_and_phone_are_the_only_required_profile_fields(): void
    {
        $admin = User::factory()->admin()->create();
        $receptionist = User::factory()->create();
        Sanctum::actingAs($receptionist);

        $patientId = $this->postJson('/api/v1/patients', [
            'name' => 'مريض جديد',
            'phone' => '0912345678',
        ])->assertCreated()
            ->assertJsonPath('data.age', null)
            ->assertJsonPath('data.gender', null)
            ->json('data.id');

        $this->assertDatabaseHas('patients', [
            'id' => $patientId,
            'name' => 'مريض جديد',
            'phone' => '0912345678',
            'age' => null,
            'gender' => null,
            'created_by' => $receptionist->id,
        ]);
        $this->assertNotNull($admin->id);
    }

    public function test_receptionist_session_index_is_restricted_to_today_while_admin_can_see_history(): void
    {
        $admin = User::factory()->admin()->create();
        $receptionist = User::factory()->create();
        $patient = Patient::factory()->create(['created_by' => $admin->id]);
        $doctor = Doctor::factory()->create(['created_by' => $admin->id]);
        $service = Service::factory()->create(['price' => '80.000', 'created_by' => $admin->id]);
        Session::factory()->create([
            'patient_id' => $patient->id,
            'doctor_id' => $doctor->id,
            'created_by' => $receptionist->id,
            'status' => 'closed',
            'opened_at' => now()->subDay(),
            'closed_at' => now()->subDay(),
            'commission_rate' => '30.000',
            'commission_base_amount' => '80.000',
            'commission_amount' => '24.000',
        ]);

        Sanctum::actingAs($receptionist);
        $todaySessionId = $this->withHeader('Idempotency-Key', (string) Str::uuid())
            ->postJson('/api/v1/sessions', [
                'patient_id' => $patient->id,
                'doctor_id' => $doctor->id,
                'type' => 'individual',
                'service_ids' => [$service->id],
                'discount' => '0.000',
                'settlement' => [
                    'paid_amount' => '80.000',
                    'wallet_amount' => '0.000',
                    'debt_amount' => '0.000',
                    'method' => 'cash',
                ],
            ])->assertCreated()->json('data.session.id');

        $this->getJson('/api/v1/sessions?date='.now()->subDay()->toDateString())
            ->assertOk()
            ->assertJsonCount(1, 'data')
            ->assertJsonPath('data.0.id', $todaySessionId);

        Sanctum::actingAs($admin);
        $this->getJson('/api/v1/sessions')
            ->assertOk()
            ->assertJsonCount(2, 'data');
    }
}
