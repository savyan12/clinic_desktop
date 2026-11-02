<?php

namespace Tests\Feature;

use App\Models\Doctor;
use App\Models\Patient;
use App\Models\Service;
use App\Models\Setting;
use App\Models\Sponsor;
use App\Models\User;
use Illuminate\Foundation\Testing\LazilyRefreshDatabase;
use Illuminate\Support\Str;
use Laravel\Sanctum\Sanctum;
use Tests\TestCase;

class ReportsSettingsApiTest extends TestCase
{
    use LazilyRefreshDatabase;

    public function test_admin_reports_separate_service_value_collections_debts_and_dues(): void
    {
        $context = $this->context();
        Sanctum::actingAs($context['receptionist']);
        $this->closeSession($context);

        $this->getJson('/api/v1/reports/sessions')->assertForbidden();
        Sanctum::actingAs($context['admin']);

        $this->getJson('/api/v1/reports/sessions')
            ->assertOk()
            ->assertJsonPath('data.totals.service_value', '80.000');
        $this->getJson('/api/v1/reports/collections')
            ->assertOk()
            ->assertJsonPath('data.totals.net', '80.000');
        $this->getJson('/api/v1/reports/doctor-dues')
            ->assertOk()
            ->assertJsonPath('data.totals.earned', '24.000');
    }

    public function test_pdf_and_xlsx_exports_return_real_files(): void
    {
        $context = $this->context();
        Sanctum::actingAs($context['receptionist']);
        $this->closeSession($context);
        Sanctum::actingAs($context['admin']);

        $pdf = $this->get('/api/v1/reports/sessions/export?format=pdf');
        $pdf->assertOk()->assertHeader('content-type', 'application/pdf');
        $this->assertStringStartsWith('%PDF-', $pdf->streamedContent());

        $xlsx = $this->get('/api/v1/reports/sessions/export?format=xlsx');
        $xlsx->assertOk()->assertHeader('content-type', 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet');
        $this->assertStringStartsWith('PK', $xlsx->streamedContent());
    }

    public function test_report_rows_match_the_frontend_contract(): void
    {
        $context = $this->context();
        $sponsor = Sponsor::factory()->create(['created_by' => $context['admin']->id]);
        Sanctum::actingAs($context['receptionist']);
        $this->closeSession($context);
        $this->withHeader('Idempotency-Key', (string) Str::uuid())
            ->postJson('/api/v1/subscriptions', [
                'patient_id' => $context['patient']->id,
                'plan_type' => 'monthly',
                'amount' => '160.000',
                'paid_amount' => '80.000',
                'method' => 'cash',
                'start_date' => today()->toDateString(),
                'end_date' => today()->addMonth()->toDateString(),
                'sponsor_id' => $sponsor->id,
            ])->assertCreated();
        Sanctum::actingAs($context['admin']);

        $this->getJson('/api/v1/reports/sessions')
            ->assertOk()
            ->assertJsonPath('data.rows.0.patient.name', $context['patient']->name)
            ->assertJsonPath('data.rows.0.doctor.name', $context['doctor']->name)
            ->assertJsonPath('data.rows.0.services.0.name', $context['service']->name)
            ->assertJsonPath('data.rows.0.net_total', '80.000')
            ->assertJsonPath('data.totals.service_value', '80.000');
        $this->getJson('/api/v1/reports/collections')
            ->assertOk()
            ->assertJsonPath('data.rows.0.patient.name', $context['patient']->name)
            ->assertJsonPath('data.rows.0.amount', '80.000');
        $this->getJson("/api/v1/reports/collections?sponsor_id={$sponsor->id}")
            ->assertOk()
            ->assertJsonCount(1, 'data.rows')
            ->assertJsonPath('data.rows.0.type', 'subscription_payment');
        $this->getJson('/api/v1/reports/debts')
            ->assertOk()
            ->assertJsonPath('data.rows.0.patient.name', $context['patient']->name)
            ->assertJsonPath('data.rows.0.paid_amount', '0.000');
        $this->getJson('/api/v1/reports/doctor-dues')
            ->assertOk()
            ->assertJsonPath('data.rows.0.id', $context['doctor']->id)
            ->assertJsonPath('data.rows.0.name', $context['doctor']->name);
        $this->getJson('/api/v1/reports/subscriptions')
            ->assertOk()
            ->assertJsonPath('data.rows.0.patient.name', $context['patient']->name)
            ->assertJsonPath('data.rows.0.plan_type', 'monthly')
            ->assertJsonPath('data.rows.0.services', [])
            ->assertJsonPath('data.rows.0.remaining', null)
            ->assertJsonPath('data.rows.0.end_date', today()->addMonth()->toDateString());
    }

    public function test_settings_are_readable_by_both_roles_and_only_admin_can_update(): void
    {
        $context = $this->context();
        Setting::create([
            'clinic_name' => 'عيادة اتزان',
            'currency' => 'د.ل',
            'print_size' => 'A4',
            'show_logo' => true,
            'updated_by' => $context['admin']->id,
        ]);
        $payload = [
            'clinic_name' => 'مركز الاتزان',
            'clinic_subtitle' => 'للعلاج الطبيعي',
            'phone' => '0910000000',
            'address' => 'طرابلس',
            'receipt_footer' => 'سلامتكم',
            'currency' => 'د.ل',
            'print_size' => 'A5',
            'show_logo' => false,
            'body_font_size' => 17,
            'body_font_weight' => 400,
            'table_font_size' => 16,
            'table_font_weight' => 600,
            'heading_font_size' => 34,
            'heading_font_weight' => 600,
            'section_font_size' => 24,
            'section_font_weight' => 600,
        ];
        Sanctum::actingAs($context['receptionist']);
        $this->getJson('/api/v1/settings')->assertOk();
        $this->putJson('/api/v1/settings', $payload)->assertForbidden();
        Sanctum::actingAs($context['admin']);

        $this->putJson('/api/v1/settings', $payload)
            ->assertOk()
            ->assertJsonPath('data.clinic_name', 'مركز الاتزان')
            ->assertJsonPath('data.print_size', 'A5')
            ->assertJsonPath('data.table_font_weight', 600);
        $this->putJson('/api/v1/settings', [...$payload, 'body_font_weight' => 500])
            ->assertUnprocessable()
            ->assertJsonValidationErrors(['body_font_weight']);
        $this->assertDatabaseHas('audit_logs', ['action' => 'settings_updated']);
    }

    public function test_dashboard_returns_role_specific_stat_count(): void
    {
        $context = $this->context();
        Sanctum::actingAs($context['receptionist']);
        $this->closeSession($context);
        $this->getJson('/api/v1/dashboard')->assertOk()->assertJsonCount(3, 'data.stats');
        Sanctum::actingAs($context['admin']);
        $this->getJson('/api/v1/dashboard')
            ->assertOk()
            ->assertJsonCount(7, 'data.stats')
            ->assertJsonPath('data.recent_movements.0.patient.name', $context['patient']->name);
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

    private function closeSession(array $context): void
    {
        $this->withHeader('Idempotency-Key', (string) Str::uuid())
            ->postJson('/api/v1/sessions', [
                'patient_id' => $context['patient']->id,
                'doctor_id' => $context['doctor']->id,
                'type' => 'individual',
                'service_ids' => [$context['service']->id],
                'discount' => '0.000',
                'settlement' => [
                    'paid_amount' => '80.000', 'wallet_amount' => '0.000', 'debt_amount' => '0.000', 'method' => 'cash',
                ],
            ])->assertCreated();
    }
}
