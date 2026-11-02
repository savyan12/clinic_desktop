<?php

namespace Tests\Feature;

use App\Enums\SubscriptionStatus;
use App\Models\AuditLog;
use App\Models\Debt;
use App\Models\Doctor;
use App\Models\Patient;
use App\Models\Payment;
use App\Models\Service;
use App\Models\Session;
use App\Models\Subscription;
use App\Models\User;
use App\Services\SessionService;
use Illuminate\Foundation\Testing\LazilyRefreshDatabase;
use Illuminate\Support\Str;
use Laravel\Sanctum\Sanctum;
use Tests\TestCase;

class SubscriptionCancellationApiTest extends TestCase
{
    use LazilyRefreshDatabase;

    public function test_fully_paid_subscription_refunds_remaining_value_once_and_is_reported(): void
    {
        $context = $this->context();
        Sanctum::actingAs($context['admin']);
        $subscriptionId = $this->createSubscription($context, '1000.000');
        Subscription::query()->whereKey($subscriptionId)->update(['sessions_used' => 4]);

        $this->getJson("/api/v1/subscriptions/{$subscriptionId}/cancel-preview")
            ->assertOk()
            ->assertJsonPath('data.sessions_used', 4)
            ->assertJsonPath('data.sessions_remaining', 6)
            ->assertJsonPath('data.consumed_value', '400.000')
            ->assertJsonPath('data.paid_total', '1000.000')
            ->assertJsonPath('data.refund_amount', '600.000');

        $key = (string) Str::uuid();
        $first = $this->withHeader('Idempotency-Key', $key)
            ->postJson("/api/v1/subscriptions/{$subscriptionId}/cancel", ['reason' => 'إلغاء بطلب المريض'])
            ->assertOk()
            ->assertJsonPath('data.subscription.status', SubscriptionStatus::Cancelled->value)
            ->assertJsonPath('data.subscription.refund_amount', '600.000')
            ->assertJsonPath('data.refund.amount', '-600.000')
            ->assertJsonPath('data.refund.method', 'cash');
        $this->withHeader('Idempotency-Key', $key)
            ->postJson("/api/v1/subscriptions/{$subscriptionId}/cancel", ['reason' => 'إلغاء بطلب المريض'])
            ->assertOk()
            ->assertExactJson($first->json());

        $originalPayment = Payment::query()->where('subscription_id', $subscriptionId)->where('type', 'subscription_payment')->firstOrFail();
        $refund = Payment::query()->where('subscription_id', $subscriptionId)->where('type', 'refund')->firstOrFail();
        $this->assertSame($originalPayment->id, $refund->reverses_payment_id);
        $this->assertSame(1, Payment::query()->where('subscription_id', $subscriptionId)->where('type', 'refund')->count());
        $this->assertDatabaseHas('audit_logs', [
            'action' => 'subscription_cancelled',
            'entity_type' => 'subscription',
            'entity_id' => $subscriptionId,
            'reason' => 'إلغاء بطلب المريض',
        ]);
        $this->assertSame(1, AuditLog::query()->where('action', 'subscription_cancelled')->where('entity_id', $subscriptionId)->count());

        $this->getJson('/api/v1/reports/collections')
            ->assertOk()
            ->assertJsonPath('data.totals.received', '1000.000')
            ->assertJsonPath('data.totals.refunds', '600.000')
            ->assertJsonPath('data.totals.net', '400.000');
        $this->getJson('/api/v1/reports/subscriptions?status=cancelled')
            ->assertOk()
            ->assertJsonPath('data.rows.0.status', SubscriptionStatus::Cancelled->value);

        Sanctum::actingAs($context['receptionist']);
        $this->withHeader('Idempotency-Key', (string) Str::uuid())
            ->postJson('/api/v1/sessions', [
                'patient_id' => $context['patient']->id,
                'doctor_id' => $context['doctor']->id,
                'type' => 'subscription',
                'subscription_id' => $subscriptionId,
            ])->assertUnprocessable()->assertJsonValidationErrors(['subscription_id']);
    }

    public function test_partially_paid_subscription_voids_debt_and_refunds_only_the_overpayment(): void
    {
        $context = $this->context();
        Sanctum::actingAs($context['admin']);
        $subscriptionId = $this->createSubscription($context, '300.000');
        $debt = Debt::query()->where('subscription_id', $subscriptionId)->firstOrFail();
        $this->withHeader('Idempotency-Key', (string) Str::uuid())
            ->postJson('/api/v1/debts/settlements', [
                'amount' => '200.000',
                'method' => 'cash',
                'allocations' => [['debt_id' => $debt->id, 'amount' => '200.000']],
            ])->assertCreated();
        Subscription::query()->whereKey($subscriptionId)->update([
            'sessions_used' => 4,
            'status' => SubscriptionStatus::Stopped,
        ]);

        $this->withHeader('Idempotency-Key', (string) Str::uuid())
            ->postJson("/api/v1/subscriptions/{$subscriptionId}/cancel", ['reason' => 'إلغاء الاشتراك الموقوف'])
            ->assertOk()
            ->assertJsonPath('data.preview.paid_total', '500.000')
            ->assertJsonPath('data.preview.consumed_value', '400.000')
            ->assertJsonPath('data.preview.refund_amount', '100.000')
            ->assertJsonPath('data.preview.debt_voided_amount', '500.000')
            ->assertJsonPath('data.preview.replacement_debt_amount', '0.000')
            ->assertJsonPath('data.refund.amount', '-100.000')
            ->assertJsonPath('data.refund.reverses_payment_id', null)
            ->assertJsonPath('data.replacement_debt', null);

        $this->assertDatabaseHas('debts', ['id' => $debt->id, 'status' => 'void', 'paid_amount' => 200]);
        $this->assertSame(1, Debt::query()->where('subscription_id', $subscriptionId)->count());
    }

    public function test_used_value_above_paid_total_replaces_the_old_debt_with_the_consumed_balance(): void
    {
        $context = $this->context();
        Sanctum::actingAs($context['admin']);
        $subscriptionId = $this->createSubscription($context, '200.000');
        $oldDebt = Debt::query()->where('subscription_id', $subscriptionId)->firstOrFail();
        Subscription::query()->whereKey($subscriptionId)->update(['sessions_used' => 4]);

        $this->withHeader('Idempotency-Key', (string) Str::uuid())
            ->postJson("/api/v1/subscriptions/{$subscriptionId}/cancel", ['reason' => 'تسوية القيمة المستهلكة'])
            ->assertOk()
            ->assertJsonPath('data.preview.paid_total', '200.000')
            ->assertJsonPath('data.preview.consumed_value', '400.000')
            ->assertJsonPath('data.preview.refund_amount', '0.000')
            ->assertJsonPath('data.preview.replacement_debt_amount', '200.000')
            ->assertJsonPath('data.refund', null)
            ->assertJsonPath('data.replacement_debt.amount', '200.000')
            ->assertJsonPath('data.replacement_debt.status', 'open');

        $this->assertDatabaseHas('debts', ['id' => $oldDebt->id, 'status' => 'void']);
        $this->assertDatabaseHas('debts', [
            'subscription_id' => $subscriptionId,
            'amount' => 200,
            'paid_amount' => 0,
            'status' => 'open',
        ]);
        $this->assertSame(0, Payment::query()->where('subscription_id', $subscriptionId)->where('type', 'refund')->count());
    }

    public function test_cancellation_is_blocked_by_an_open_linked_session(): void
    {
        $context = $this->context();
        Sanctum::actingAs($context['admin']);
        $subscriptionId = $this->createSubscription($context, '1000.000');
        app(SessionService::class)->create([
            'patient_id' => $context['patient']->id,
            'doctor_id' => $context['doctor']->id,
            'type' => 'subscription',
            'subscription_id' => $subscriptionId,
        ], $context['admin']);

        $this->withHeader('Idempotency-Key', (string) Str::uuid())
            ->postJson("/api/v1/subscriptions/{$subscriptionId}/cancel", ['reason' => 'محاولة الإلغاء'])
            ->assertConflict()
            ->assertJsonPath('message', 'توجد جلسة مفتوحة مرتبطة بالاشتراك. أغلقها أو ألغها أولًا.');

        $this->assertSame(SubscriptionStatus::Active, Subscription::query()->findOrFail($subscriptionId)->status);
        $this->assertSame(0, Payment::query()->where('subscription_id', $subscriptionId)->where('type', 'refund')->count());
    }

    public function test_cancellation_does_not_change_closed_sessions_or_doctor_commission(): void
    {
        $context = $this->context();
        Sanctum::actingAs($context['admin']);
        $subscriptionId = $this->createSubscription($context, '1000.000');
        $sessionId = $this->withHeader('Idempotency-Key', (string) Str::uuid())
            ->postJson('/api/v1/sessions', [
                'patient_id' => $context['patient']->id,
                'doctor_id' => $context['doctor']->id,
                'type' => 'subscription',
                'subscription_id' => $subscriptionId,
            ])->assertCreated()->json('data.session.id');
        $closedSession = Session::query()->findOrFail($sessionId);
        $commissionAmount = $closedSession->commission_amount;

        $this->withHeader('Idempotency-Key', (string) Str::uuid())
            ->postJson("/api/v1/subscriptions/{$subscriptionId}/cancel", ['reason' => 'إلغاء بعد جلسة منفذة'])
            ->assertOk()
            ->assertJsonPath('data.preview.consumed_value', '100.000')
            ->assertJsonPath('data.preview.refund_amount', '900.000');

        $closedSession->refresh();
        $this->assertSame('closed', $closedSession->status->value);
        $this->assertSame($commissionAmount, $closedSession->commission_amount);
    }

    public function test_completed_and_already_cancelled_subscriptions_are_blocked(): void
    {
        $context = $this->context();
        Sanctum::actingAs($context['admin']);
        $completedId = $this->createSubscription($context, '1000.000');
        Subscription::query()->whereKey($completedId)->update([
            'sessions_used' => 10,
            'status' => SubscriptionStatus::Completed,
        ]);

        $this->withHeader('Idempotency-Key', (string) Str::uuid())
            ->postJson("/api/v1/subscriptions/{$completedId}/cancel", ['reason' => 'غير مسموح'])
            ->assertConflict()
            ->assertJsonPath('message', 'لا يمكن إلغاء اشتراك مكتمل.');

        $cancelledId = $this->createSubscription($context, '1000.000');
        $this->withHeader('Idempotency-Key', (string) Str::uuid())
            ->postJson("/api/v1/subscriptions/{$cancelledId}/cancel", ['reason' => 'الإلغاء الأول'])
            ->assertOk();
        $this->withHeader('Idempotency-Key', (string) Str::uuid())
            ->postJson("/api/v1/subscriptions/{$cancelledId}/cancel", ['reason' => 'الإلغاء الثاني'])
            ->assertConflict()
            ->assertJsonPath('message', 'تم إلغاء الاشتراك بالفعل.');
    }

    public function test_receptionist_cannot_preview_or_cancel_subscription(): void
    {
        $context = $this->context();
        Sanctum::actingAs($context['admin']);
        $subscriptionId = $this->createSubscription($context, '1000.000');
        Sanctum::actingAs($context['receptionist']);

        $this->getJson("/api/v1/subscriptions/{$subscriptionId}/cancel-preview")->assertForbidden();
        $this->withHeader('Idempotency-Key', (string) Str::uuid())
            ->postJson("/api/v1/subscriptions/{$subscriptionId}/cancel", ['reason' => 'غير مصرح'])
            ->assertForbidden();
    }

    public function test_cancellation_reason_is_required(): void
    {
        $context = $this->context();
        Sanctum::actingAs($context['admin']);
        $subscriptionId = $this->createSubscription($context, '1000.000');

        $this->withHeader('Idempotency-Key', (string) Str::uuid())
            ->postJson("/api/v1/subscriptions/{$subscriptionId}/cancel", ['reason' => ''])
            ->assertUnprocessable()
            ->assertJsonValidationErrors(['reason']);

        $this->assertSame(SubscriptionStatus::Active, Subscription::query()->findOrFail($subscriptionId)->status);
    }

    /** @return array<string, User|Patient|Doctor|Service> */
    private function context(): array
    {
        $admin = User::factory()->admin()->create();
        $receptionist = User::factory()->create();
        $patient = Patient::factory()->create(['created_by' => $admin->id]);
        $doctor = Doctor::factory()->create(['created_by' => $admin->id]);
        $service = Service::factory()->create(['price' => '100.000', 'created_by' => $admin->id]);

        return compact('admin', 'receptionist', 'patient', 'doctor', 'service');
    }

    /** @param array<string, User|Patient|Doctor|Service> $context */
    private function createSubscription(array $context, string $paidAmount): int
    {
        return $this->withHeader('Idempotency-Key', (string) Str::uuid())
            ->postJson('/api/v1/subscriptions', [
                'patient_id' => $context['patient']->id,
                'service_ids' => [$context['service']->id],
                'sessions_count' => 10,
                'discount' => '0.000',
                'paid_amount' => $paidAmount,
                'method' => $paidAmount === '0.000' ? null : 'cash',
                'start_date' => today()->toDateString(),
                'expected_end_date' => null,
                'sponsor_id' => null,
            ])->assertCreated()->json('data.subscription.id');
    }
}
