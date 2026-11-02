<?php

namespace Tests\Feature;

use App\Enums\SessionStatus;
use App\Enums\SubscriptionPlanType;
use App\Enums\SubscriptionStatus;
use App\Models\Doctor;
use App\Models\Patient;
use App\Models\Payment;
use App\Models\Service;
use App\Models\Sponsor;
use App\Models\Subscription;
use App\Models\SubscriptionUsage;
use App\Models\User;
use Illuminate\Foundation\Testing\LazilyRefreshDatabase;
use Illuminate\Support\Str;
use Laravel\Sanctum\Sanctum;
use Tests\TestCase;

class SubscriptionLifecycleApiTest extends TestCase
{
    use LazilyRefreshDatabase;

    public function test_monthly_subscription_uses_user_entered_amount_and_date_range_without_services_or_session_quota(): void
    {
        $context = $this->context();
        $sponsor = Sponsor::factory()->create(['created_by' => $context['admin']->id]);
        Sanctum::actingAs($context['receptionist']);

        $this->withHeader('Idempotency-Key', (string) Str::uuid())
            ->postJson('/api/v1/subscriptions', [
                ...$this->monthlyPayload($context),
                'amount' => '650.000',
                'paid_amount' => '250.000',
                'method' => 'cash',
                'sponsor_id' => $sponsor->id,
            ])->assertCreated()
            ->assertJsonPath('data.subscription.plan_type', SubscriptionPlanType::Monthly->value)
            ->assertJsonPath('data.subscription.sessions_count', 0)
            ->assertJsonPath('data.subscription.remaining', null)
            ->assertJsonPath('data.subscription.base_total', '650.000')
            ->assertJsonPath('data.subscription.net_total', '650.000')
            ->assertJsonPath('data.subscription.discount', '0.000')
            ->assertJsonPath('data.subscription.remaining_amount', '400.000')
            ->assertJsonPath('data.subscription.start_date', today()->toDateString())
            ->assertJsonPath('data.subscription.end_date', today()->addMonth()->toDateString());

        $this->assertDatabaseCount('subscription_services', 0);
        $this->assertDatabaseHas('payments', ['type' => 'subscription_payment', 'amount' => 250]);
        $this->assertDatabaseHas('debts', ['kind' => 'subscription', 'amount' => 400]);
    }

    public function test_monthly_subscription_session_closes_immediately_without_consuming_a_quota_or_creating_receipt(): void
    {
        $context = $this->context();
        Sanctum::actingAs($context['receptionist']);
        $subscriptionId = $this->createMonthlySubscription($context);

        $sessionId = $this->withHeader('Idempotency-Key', (string) Str::uuid())
            ->postJson('/api/v1/sessions', [
                'patient_id' => $context['patient']->id,
                'doctor_id' => $context['doctor']->id,
                'type' => 'subscription',
                'subscription_id' => $subscriptionId,
                'service_ids' => [$context['firstService']->id, $context['secondService']->id],
            ])->assertCreated()
            ->assertJsonPath('data.session.status', SessionStatus::Closed->value)
            ->assertJsonPath('data.session.net_total', '120.000')
            ->assertJsonPath('data.effects.subscription_usage', null)
            ->assertJsonPath('data.effects.payment', null)
            ->json('data.session.id');

        $subscription = Subscription::query()->findOrFail($subscriptionId);
        $this->assertSame(0, $subscription->sessions_used);
        $this->assertSame(SubscriptionStatus::Active, $subscription->status);
        $this->assertSame(0, SubscriptionUsage::query()->where('session_id', $sessionId)->count());
        $this->assertSame(0, Payment::query()->where('session_id', $sessionId)->count());
    }

    public function test_monthly_subscription_can_only_be_used_inside_its_active_date_range(): void
    {
        $context = $this->context();
        Sanctum::actingAs($context['receptionist']);
        $subscriptionId = $this->withHeader('Idempotency-Key', (string) Str::uuid())
            ->postJson('/api/v1/subscriptions', [
                ...$this->monthlyPayload($context),
                'start_date' => today()->addDay()->toDateString(),
                'end_date' => today()->addMonth()->toDateString(),
            ])->assertCreated()
            ->json('data.subscription.id');

        $this->withHeader('Idempotency-Key', (string) Str::uuid())
            ->postJson('/api/v1/sessions', [
                'patient_id' => $context['patient']->id,
                'doctor_id' => $context['doctor']->id,
                'type' => 'subscription',
                'subscription_id' => $subscriptionId,
                'service_ids' => [$context['firstService']->id],
            ])->assertUnprocessable()
            ->assertJsonValidationErrors(['subscription_id']);
    }

    public function test_monthly_subscription_is_completed_automatically_after_end_date(): void
    {
        $context = $this->context();
        $subscription = Subscription::factory()->monthly()->create([
            'patient_id' => $context['patient']->id,
            'created_by' => $context['admin']->id,
            'start_date' => today()->subMonth(),
            'end_date' => today()->subDay(),
            'status' => SubscriptionStatus::Active,
        ]);
        Sanctum::actingAs($context['receptionist']);

        $this->getJson("/api/v1/subscriptions/{$subscription->id}")
            ->assertOk()
            ->assertJsonPath('data.status', SubscriptionStatus::Completed->value)
            ->assertJsonPath('data.is_usable', false);
    }

    public function test_admin_can_correct_monthly_end_date_without_changing_financial_history(): void
    {
        $context = $this->context();
        Sanctum::actingAs($context['receptionist']);
        $subscriptionId = $this->createMonthlySubscription($context);
        Sanctum::actingAs($context['admin']);
        $newEndDate = today()->addMonths(2)->toDateString();

        $this->withHeader('Idempotency-Key', (string) Str::uuid())
            ->postJson("/api/v1/subscriptions/{$subscriptionId}/correct", [
                'changes' => ['end_date' => $newEndDate],
                'reason' => 'تمديد المدة بموافقة الإدارة',
            ])->assertOk()
            ->assertJsonPath('data.subscription.end_date', $newEndDate)
            ->assertJsonPath('data.impact.historical_financial_rows_changed', false);

        $this->assertDatabaseHas('audit_logs', [
            'action' => 'subscription_corrected',
            'entity_id' => $subscriptionId,
        ]);
    }

    public function test_patient_can_have_multiple_monthly_subscriptions(): void
    {
        $context = $this->context();
        Sanctum::actingAs($context['receptionist']);

        $firstId = $this->createMonthlySubscription($context);
        $secondId = $this->createMonthlySubscription($context);

        $this->assertNotSame($firstId, $secondId);
        $this->assertDatabaseCount('subscriptions', 2);
    }

    public function test_legacy_session_package_remains_readable_and_consumable_after_restore(): void
    {
        $context = $this->context();
        Sanctum::actingAs($context['receptionist']);
        $subscriptionId = $this->withHeader('Idempotency-Key', (string) Str::uuid())
            ->postJson('/api/v1/subscriptions', [
                'patient_id' => $context['patient']->id,
                'plan_type' => SubscriptionPlanType::LegacyPackage->value,
                'service_ids' => [$context['firstService']->id, $context['secondService']->id],
                'sessions_count' => 10,
                'discount' => '100.000',
                'paid_amount' => '1100.000',
                'method' => 'cash',
                'start_date' => today()->toDateString(),
                'expected_end_date' => null,
            ])->assertCreated()
            ->assertJsonPath('data.subscription.plan_type', SubscriptionPlanType::LegacyPackage->value)
            ->json('data.subscription.id');

        $this->withHeader('Idempotency-Key', (string) Str::uuid())
            ->postJson('/api/v1/sessions', [
                'patient_id' => $context['patient']->id,
                'doctor_id' => $context['doctor']->id,
                'type' => 'subscription',
                'subscription_id' => $subscriptionId,
            ])->assertCreated()
            ->assertJsonPath('data.session.status', SessionStatus::Closed->value)
            ->assertJsonCount(2, 'data.session.services')
            ->assertJsonPath('data.session.net_total', '110.000');

        $this->assertSame(1, Subscription::query()->findOrFail($subscriptionId)->sessions_used);
        $this->assertDatabaseCount('subscription_usages', 1);
    }

    public function test_subscription_renew_route_does_not_exist(): void
    {
        $context = $this->context();
        Sanctum::actingAs($context['receptionist']);
        $subscriptionId = $this->createMonthlySubscription($context);
        Sanctum::actingAs($context['admin']);

        $this->withHeader('Idempotency-Key', (string) Str::uuid())
            ->postJson("/api/v1/subscriptions/{$subscriptionId}/renew", $this->monthlyPayload($context))
            ->assertNotFound();
    }

    /** @return array{admin: User, receptionist: User, patient: Patient, doctor: Doctor, firstService: Service, secondService: Service} */
    private function context(): array
    {
        $admin = User::factory()->admin()->create();
        $receptionist = User::factory()->create();
        $patient = Patient::factory()->create(['created_by' => $admin->id]);
        $doctor = Doctor::factory()->create(['created_by' => $admin->id]);
        $firstService = Service::factory()->create(['price' => '80.000', 'created_by' => $admin->id]);
        $secondService = Service::factory()->create(['price' => '40.000', 'created_by' => $admin->id]);

        return compact('admin', 'receptionist', 'patient', 'doctor', 'firstService', 'secondService');
    }

    /** @param array<string, mixed> $context */
    private function createMonthlySubscription(array $context): int
    {
        return $this->withHeader('Idempotency-Key', (string) Str::uuid())
            ->postJson('/api/v1/subscriptions', $this->monthlyPayload($context))
            ->assertCreated()
            ->json('data.subscription.id');
    }

    /** @param array<string, mixed> $context */
    private function monthlyPayload(array $context): array
    {
        return [
            'patient_id' => $context['patient']->id,
            'plan_type' => SubscriptionPlanType::Monthly->value,
            'amount' => '600.000',
            'paid_amount' => '600.000',
            'method' => 'cash',
            'start_date' => today()->toDateString(),
            'end_date' => today()->addMonth()->toDateString(),
            'sponsor_id' => null,
        ];
    }
}
