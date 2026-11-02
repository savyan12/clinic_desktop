<?php

namespace Tests\Feature;

use App\Models\Patient;
use App\Models\User;
use Illuminate\Foundation\Testing\LazilyRefreshDatabase;
use Illuminate\Support\Facades\Route;
use Illuminate\Support\Str;
use Laravel\Sanctum\Sanctum;
use Tests\TestCase;

class RouteSafetyTest extends TestCase
{
    use LazilyRefreshDatabase;

    public function test_api_has_no_delete_routes_for_domain_or_financial_records(): void
    {
        $deleteRoutes = collect(Route::getRoutes()->getRoutes())
            ->filter(fn ($route) => str_starts_with($route->uri(), 'api/v1/'))
            ->filter(fn ($route) => in_array('DELETE', $route->methods(), true));

        $this->assertCount(0, $deleteRoutes);
    }

    public function test_audit_log_has_read_route_only(): void
    {
        $auditRoutes = collect(Route::getRoutes()->getRoutes())
            ->filter(fn ($route) => $route->uri() === 'api/v1/audit-log');

        $this->assertCount(1, $auditRoutes);
        $this->assertSame(['GET', 'HEAD'], $auditRoutes->first()->methods());
    }

    public function test_unauthenticated_api_request_returns_consistent_401_json(): void
    {
        $this->getJson('/api/v1/patients')
            ->assertUnauthorized()
            ->assertJsonPath('message', 'انتهت جلسة الدخول، يرجى تسجيل الدخول مجددًا.');
    }

    public function test_money_route_requires_uuid_idempotency_key_and_rejects_key_reuse_with_new_payload(): void
    {
        $admin = User::factory()->admin()->create();
        $receptionist = User::factory()->create();
        $patient = Patient::factory()->create(['created_by' => $admin->id]);
        Sanctum::actingAs($receptionist);

        $this->postJson("/api/v1/patients/{$patient->id}/wallet/topups", [
            'amount' => '10.000', 'method' => 'cash',
        ])->assertUnprocessable();

        $key = (string) Str::uuid();
        $this->withHeader('Idempotency-Key', $key)
            ->postJson("/api/v1/patients/{$patient->id}/wallet/topups", [
                'amount' => '10.000', 'method' => 'cash',
            ])->assertCreated();
        $this->withHeader('Idempotency-Key', $key)
            ->postJson("/api/v1/patients/{$patient->id}/wallet/topups", [
                'amount' => '20.000', 'method' => 'cash',
            ])->assertConflict()
            ->assertJsonPath('message', 'مفتاح العملية مستخدم لطلب مختلف.');
    }

    public function test_idempotency_key_is_scoped_to_the_concrete_resource_path(): void
    {
        $admin = User::factory()->admin()->create();
        $receptionist = User::factory()->create();
        $firstPatient = Patient::factory()->create(['created_by' => $admin->id]);
        $secondPatient = Patient::factory()->create(['created_by' => $admin->id]);
        Sanctum::actingAs($receptionist);
        $key = (string) Str::uuid();
        $payload = ['amount' => '10.000', 'method' => 'cash'];

        $this->withHeader('Idempotency-Key', $key)
            ->postJson("/api/v1/patients/{$firstPatient->id}/wallet/topups", $payload)
            ->assertCreated()
            ->assertJsonPath('data.balance', '10.000');
        $this->withHeader('Idempotency-Key', $key)
            ->postJson("/api/v1/patients/{$secondPatient->id}/wallet/topups", $payload)
            ->assertCreated()
            ->assertJsonPath('data.balance', '10.000');

        $this->assertDatabaseCount('wallet_transactions', 2);
    }
}
