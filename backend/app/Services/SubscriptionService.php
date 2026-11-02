<?php

namespace App\Services;

use App\Enums\DebtKind;
use App\Enums\DebtStatus;
use App\Enums\PaymentMethod;
use App\Enums\PaymentType;
use App\Enums\SessionStatus;
use App\Enums\SubscriptionPlanType;
use App\Enums\SubscriptionStatus;
use App\Exceptions\ApiException;
use App\Models\Debt;
use App\Models\Patient;
use App\Models\Payment;
use App\Models\PaymentAllocation;
use App\Models\Service;
use App\Models\Subscription;
use App\Models\SubscriptionService as SubscriptionServiceModel;
use App\Models\User;
use App\Support\Money;
use Carbon\CarbonImmutable;
use Illuminate\Support\Arr;
use Illuminate\Support\Facades\DB;

class SubscriptionService
{
    public function __construct(private AuditLogger $auditLogger) {}

    /**
     * @param  array<string, mixed>  $data
     * @return array{subscription: Subscription, payment: ?Payment, debt: ?Debt}
     */
    public function create(array $data, User $actor): array
    {
        return DB::transaction(function () use ($data, $actor): array {
            $patient = Patient::query()->where('is_active', true)->lockForUpdate()->findOrFail($data['patient_id']);
            $planType = SubscriptionPlanType::from($data['plan_type'] ?? SubscriptionPlanType::Monthly->value);

            return $planType === SubscriptionPlanType::Monthly
                ? $this->createMonthly($patient, $data, $actor)
                : $this->createLegacyPackage($patient, $data, $actor);
        });
    }

    public function expireMonthlySubscriptions(): void
    {
        Subscription::query()
            ->where('plan_type', SubscriptionPlanType::Monthly)
            ->where('status', SubscriptionStatus::Active)
            ->whereDate('end_date', '<', today())
            ->update(['status' => SubscriptionStatus::Completed]);
    }

    /**
     * @param  array<string, mixed>  $data
     * @return array{subscription: Subscription, payment: ?Payment, debt: ?Debt}
     */
    private function createMonthly(Patient $patient, array $data, User $actor): array
    {
        $startDate = CarbonImmutable::parse($data['start_date'])->startOfDay();
        $endDate = CarbonImmutable::parse($data['end_date'])->startOfDay();

        if ($endDate->lt($startDate)) {
            throw ApiException::validation('تاريخ نهاية الاشتراك يجب ألا يسبق تاريخ البداية.', [
                'end_date' => ['اختر تاريخ نهاية يساوي تاريخ البداية أو يأتي بعده.'],
            ]);
        }

        $amount = Money::normalize($data['amount']);
        $paidAmount = $this->validatePayment($amount, $data);
        $subscription = Subscription::create([
            'patient_id' => $patient->id,
            'sponsor_id' => $data['sponsor_id'] ?? null,
            'plan_type' => SubscriptionPlanType::Monthly,
            'sessions_count' => 0,
            'sessions_used' => 0,
            'base_total' => $amount,
            'discount' => '0.000',
            'net_total' => $amount,
            'paid_amount' => $paidAmount,
            'start_date' => $startDate->toDateString(),
            'expected_end_date' => null,
            'end_date' => $endDate->toDateString(),
            'status' => SubscriptionStatus::Active,
            'created_by' => $actor->id,
        ]);
        [$payment, $debt] = $this->recordInitialFinancials($subscription, $patient, $data, $paidAmount, $actor);

        $this->auditLogger->log($actor, 'subscription_created', $subscription, $subscription->id, null, [
            ...$subscription->toArray(),
            'payment_id' => $payment?->id,
            'debt_id' => $debt?->id,
        ]);

        return compact('subscription', 'payment', 'debt');
    }

    /**
     * @param  array<string, mixed>  $data
     * @return array{subscription: Subscription, payment: ?Payment, debt: ?Debt}
     */
    private function createLegacyPackage(Patient $patient, array $data, User $actor): array
    {
        $services = Service::query()
            ->whereIn('id', $data['service_ids'])
            ->where('is_active', true)
            ->orderBy('id')
            ->lockForUpdate()
            ->get();

        if ($services->count() !== count(array_unique($data['service_ids']))) {
            throw ApiException::validation('إحدى الخدمات غير متاحة للاشتراكات الجديدة.', [
                'service_ids' => ['اختر خدمات نشطة فقط.'],
            ]);
        }

        $sessionsCount = (int) $data['sessions_count'];
        $servicesPerSession = Money::sum($services->pluck('price'));
        $baseTotal = Money::multiply($servicesPerSession, (string) $sessionsCount);
        $discount = Money::normalize($data['discount'] ?? '0');

        if (Money::compare($discount, $baseTotal) > 0) {
            throw ApiException::validation('الخصم لا يمكن أن يتجاوز السعر الأساسي للاشتراك.', [
                'discount' => ['قيمة الخصم أكبر من السعر الأساسي.'],
            ]);
        }

        $netTotal = Money::subtract($baseTotal, $discount);
        $paidAmount = $this->validatePayment($netTotal, $data);
        $subscription = Subscription::create([
            'patient_id' => $patient->id,
            'sponsor_id' => $data['sponsor_id'] ?? null,
            'plan_type' => SubscriptionPlanType::LegacyPackage,
            'sessions_count' => $sessionsCount,
            'sessions_used' => 0,
            'base_total' => $baseTotal,
            'discount' => $discount,
            'net_total' => $netTotal,
            'paid_amount' => $paidAmount,
            'start_date' => $data['start_date'],
            'expected_end_date' => $data['expected_end_date'] ?? null,
            'end_date' => null,
            'status' => SubscriptionStatus::Active,
            'created_by' => $actor->id,
        ]);

        foreach ($services as $service) {
            SubscriptionServiceModel::create([
                'subscription_id' => $subscription->id,
                'service_id' => $service->id,
                'service_name' => $service->name,
                'price' => Money::normalize($service->price),
                'created_by' => $actor->id,
            ]);
        }

        [$payment, $debt] = $this->recordInitialFinancials($subscription, $patient, $data, $paidAmount, $actor);
        $this->auditLogger->log($actor, 'subscription_created', $subscription, $subscription->id, null, [
            ...$subscription->toArray(),
            'service_ids' => $services->pluck('id')->all(),
            'payment_id' => $payment?->id,
            'debt_id' => $debt?->id,
        ]);

        return compact('subscription', 'payment', 'debt');
    }

    /** @param array<string, mixed> $data */
    private function validatePayment(string $netTotal, array $data): string
    {
        $paidAmount = Money::normalize($data['paid_amount'] ?? '0');

        if (Money::compare($paidAmount, $netTotal) > 0) {
            throw ApiException::validation('المبلغ المدفوع يتجاوز قيمة الاشتراك.', [
                'paid_amount' => ['أقصى مبلغ مسموح هو '.$netTotal.'.'],
            ]);
        }

        if (Money::isPositive($paidAmount) && empty($data['method'])) {
            throw ApiException::validation('طريقة الدفع مطلوبة عند تسجيل مبلغ مدفوع.', [
                'method' => ['اختر نقدًا أو تحويلًا مصرفيًا.'],
            ]);
        }

        return $paidAmount;
    }

    /**
     * @param  array<string, mixed>  $data
     * @return array{0: ?Payment, 1: ?Debt}
     */
    private function recordInitialFinancials(
        Subscription $subscription,
        Patient $patient,
        array $data,
        string $paidAmount,
        User $actor,
    ): array {
        $payment = null;

        if (Money::isPositive($paidAmount)) {
            $payment = Payment::create([
                'patient_id' => $patient->id,
                'subscription_id' => $subscription->id,
                'type' => PaymentType::SubscriptionPayment,
                'method' => PaymentMethod::from($data['method']),
                'amount' => $paidAmount,
                'created_by' => $actor->id,
            ]);
            $this->auditLogger->log($actor, 'payment_recorded', $payment, $payment->id, null, [
                'subscription_id' => $subscription->id,
                'type' => PaymentType::SubscriptionPayment->value,
                'method' => $payment->method->value,
                'amount' => $paidAmount,
            ]);
        }

        $debt = null;
        $remaining = Money::subtract($subscription->net_total, $paidAmount);

        if (Money::isPositive($remaining)) {
            $debt = Debt::create([
                'patient_id' => $patient->id,
                'sponsor_id' => $data['sponsor_id'] ?? null,
                'subscription_id' => $subscription->id,
                'kind' => DebtKind::Subscription,
                'amount' => $remaining,
                'paid_amount' => '0.000',
                'status' => DebtStatus::Open,
                'created_by' => $actor->id,
            ]);
        }

        return [$payment, $debt];
    }

    public function stop(Subscription $subscription, string $reason, User $actor): Subscription
    {
        $this->expireMonthlySubscriptions();

        return DB::transaction(function () use ($subscription, $reason, $actor): Subscription {
            $locked = Subscription::query()->lockForUpdate()->findOrFail($subscription->id);

            if ($locked->status !== SubscriptionStatus::Active) {
                throw ApiException::conflict('لا يمكن إيقاف اشتراك غير نشط.');
            }

            $old = $locked->toArray();
            $locked->update(['status' => SubscriptionStatus::Stopped]);
            $this->auditLogger->log($actor, 'subscription_stopped', $locked, $locked->id, $old, $locked->fresh()->toArray(), $reason);

            return $locked->fresh();
        });
    }

    public function reactivate(Subscription $subscription, ?string $reason, User $actor): Subscription
    {
        return DB::transaction(function () use ($subscription, $reason, $actor): Subscription {
            $locked = Subscription::query()->lockForUpdate()->findOrFail($subscription->id);

            if ($locked->status !== SubscriptionStatus::Stopped) {
                throw ApiException::conflict('يمكن إعادة تفعيل الاشتراك الموقوف فقط.');
            }

            if ($locked->isMonthly() && $locked->end_date?->lt(today())) {
                throw ApiException::validation('انتهت مدة الاشتراك ولا يمكن إعادة تفعيله.', [
                    'subscription' => ['أنشئ اشتراكًا شهريًا جديدًا للمريض.'],
                ]);
            }

            if (! $locked->isMonthly() && $locked->sessions_used >= $locked->sessions_count) {
                throw ApiException::validation('لا توجد جلسات متبقية لإعادة تفعيل الاشتراك.', [
                    'subscription' => ['أنشئ اشتراكًا جديدًا للمريض بدلًا من إعادة التفعيل.'],
                ]);
            }

            $old = $locked->toArray();
            $locked->update(['status' => SubscriptionStatus::Active]);
            $this->auditLogger->log($actor, 'subscription_reactivated', $locked, $locked->id, $old, $locked->fresh()->toArray(), $reason);

            return $locked->fresh();
        });
    }

    /** @return array<string, mixed> */
    public function cancellationPreview(Subscription $subscription): array
    {
        $this->expireMonthlySubscriptions();

        return DB::transaction(function () use ($subscription): array {
            $locked = Subscription::query()->lockForUpdate()->findOrFail($subscription->id);

            return $this->publicCancellationPreview($this->cancellationState($locked));
        });
    }

    /**
     * @return array{
     *     subscription: Subscription,
     *     preview: array<string, mixed>,
     *     refund: ?Payment,
     *     voided_debt_ids: array<int, int>,
     *     replacement_debt: ?Debt
     * }
     */
    public function cancel(Subscription $subscription, string $reason, User $actor): array
    {
        $this->expireMonthlySubscriptions();

        return DB::transaction(function () use ($subscription, $reason, $actor): array {
            $locked = Subscription::query()->lockForUpdate()->findOrFail($subscription->id);
            Patient::query()->lockForUpdate()->findOrFail($locked->patient_id);
            $state = $this->cancellationState($locked);
            $preview = $this->publicCancellationPreview($state);
            $old = $locked->toArray();
            $refund = null;

            if (Money::isPositive($preview['refund_amount'])) {
                $sourcePaymentIds = $state['source_payment_ids'];
                $refund = Payment::create([
                    'patient_id' => $locked->patient_id,
                    'subscription_id' => $locked->id,
                    'type' => PaymentType::Refund,
                    'method' => PaymentMethod::Cash,
                    'amount' => '-'.$preview['refund_amount'],
                    'reverses_payment_id' => count($sourcePaymentIds) === 1 ? $sourcePaymentIds[0] : null,
                    'note' => $reason,
                    'created_by' => $actor->id,
                ]);
                $this->auditLogger->log($actor, 'payment_refunded', $refund, $refund->id, null, [
                    'subscription_id' => $locked->id,
                    'amount' => '-'.$preview['refund_amount'],
                    'method' => PaymentMethod::Cash->value,
                ], $reason);
            }

            $voidedDebtIds = [];

            foreach ($state['debts_to_void'] as $debt) {
                $debt->update([
                    'status' => DebtStatus::Void,
                    'voided_at' => now(),
                    'voided_by' => $actor->id,
                    'void_reason' => $reason,
                ]);
                $voidedDebtIds[] = $debt->id;
            }

            $replacementDebt = null;

            if (Money::isPositive($preview['replacement_debt_amount'])) {
                $replacementDebt = Debt::create([
                    'patient_id' => $locked->patient_id,
                    'sponsor_id' => $locked->sponsor_id,
                    'subscription_id' => $locked->id,
                    'kind' => DebtKind::Subscription,
                    'amount' => $preview['replacement_debt_amount'],
                    'paid_amount' => '0.000',
                    'status' => DebtStatus::Open,
                    'note' => 'دين معدل بعد إلغاء الاشتراك #'.$locked->id,
                    'created_by' => $actor->id,
                ]);
            }

            $locked->update([
                'status' => SubscriptionStatus::Cancelled,
                'cancelled_at' => now(),
                'cancelled_by' => $actor->id,
                'cancel_reason' => $reason,
                'refund_amount' => $preview['refund_amount'],
            ]);
            $fresh = $locked->fresh(['patient', 'sponsor', 'services']);
            $this->auditLogger->log($actor, 'subscription_cancelled', $fresh, $fresh->id, $old, [
                ...$fresh->toArray(),
                'preview' => $preview,
                'refund_payment_id' => $refund?->id,
                'voided_debt_ids' => $voidedDebtIds,
                'replacement_debt_id' => $replacementDebt?->id,
            ], $reason);

            return [
                'subscription' => $fresh,
                'preview' => $preview,
                'refund' => $refund,
                'voided_debt_ids' => $voidedDebtIds,
                'replacement_debt' => $replacementDebt,
            ];
        });
    }

    /** @param array<string, mixed> $changes */
    public function correct(Subscription $subscription, array $changes, string $reason, User $actor): Subscription
    {
        return DB::transaction(function () use ($subscription, $changes, $reason, $actor): Subscription {
            $locked = Subscription::query()->lockForUpdate()->findOrFail($subscription->id);
            $allowed = Arr::only(
                $changes,
                $locked->isMonthly()
                    ? ['end_date', 'sponsor_id']
                    : ['expected_end_date', 'sponsor_id'],
            );

            if ($locked->isMonthly() && array_key_exists('end_date', $allowed)) {
                $endDate = CarbonImmutable::parse($allowed['end_date'])->startOfDay();

                if ($endDate->lt(CarbonImmutable::parse($locked->start_date)->startOfDay())) {
                    throw ApiException::validation('تاريخ نهاية الاشتراك يجب ألا يسبق تاريخ البداية.', [
                        'changes.end_date' => ['اختر تاريخ نهاية صحيحًا.'],
                    ]);
                }
            }

            if (array_key_exists('sponsor_id', $allowed) && $locked->debts()->exists()) {
                throw ApiException::validation('لا يمكن تغيير الجهة الراعية بعد تسجيل دين على الاشتراك.', [
                    'changes.sponsor_id' => ['أنشئ تصحيحًا ماليًا مستقلًا يحافظ على السجل الأصلي.'],
                ]);
            }

            $old = Arr::only($locked->toArray(), array_keys($allowed));
            $locked->update($allowed);
            $this->auditLogger->log($actor, 'subscription_corrected', $locked, $locked->id, $old, Arr::only($locked->fresh()->toArray(), array_keys($allowed)), $reason);

            return $locked->fresh();
        });
    }

    /** @return array<string, mixed> */
    private function cancellationState(Subscription $subscription): array
    {
        if ($subscription->status === SubscriptionStatus::Cancelled) {
            throw ApiException::conflict('تم إلغاء الاشتراك بالفعل.');
        }

        if ($subscription->status === SubscriptionStatus::Completed) {
            throw ApiException::conflict('لا يمكن إلغاء اشتراك مكتمل.');
        }

        if (! in_array($subscription->status, [SubscriptionStatus::Active, SubscriptionStatus::Stopped], true)) {
            throw ApiException::conflict('لا يمكن إلغاء الاشتراك في حالته الحالية.');
        }

        $remainingSessions = null;
        $periodDays = null;
        $elapsedDays = null;
        $remainingDays = null;

        if ($subscription->isMonthly()) {
            $startDate = CarbonImmutable::parse($subscription->start_date)->startOfDay();
            $endDate = CarbonImmutable::parse($subscription->end_date)->startOfDay();
            $today = CarbonImmutable::today(config('app.timezone'));
            $periodDays = $startDate->diffInDays($endDate) + 1;
            $elapsedDays = $today->lt($startDate)
                ? 0
                : min($periodDays, $startDate->diffInDays($today->min($endDate)) + 1);
            $remainingDays = max(0, $periodDays - $elapsedDays);
        } else {
            $remainingSessions = max(0, $subscription->sessions_count - $subscription->sessions_used);

            if ($remainingSessions === 0) {
                throw ApiException::validation('لا توجد جلسات متبقية لإلغاء الاشتراك.', [
                    'subscription' => ['يجب أن يحتوي الاشتراك على جلسات متبقية.'],
                ]);
            }
        }

        if ($subscription->sessions()->where('status', SessionStatus::Open)->lockForUpdate()->exists()) {
            throw ApiException::conflict('توجد جلسة مفتوحة مرتبطة بالاشتراك. أغلقها أو ألغها أولًا.');
        }

        $debts = Debt::query()
            ->where('subscription_id', $subscription->id)
            ->orderBy('id')
            ->lockForUpdate()
            ->get();
        $debtIds = $debts->pluck('id');
        $allocations = $debtIds->isEmpty()
            ? collect()
            : PaymentAllocation::query()->whereIn('debt_id', $debtIds)->lockForUpdate()->get();
        $directPayments = Payment::query()
            ->where('subscription_id', $subscription->id)
            ->where('type', PaymentType::SubscriptionPayment)
            ->where('amount', '>', 0)
            ->lockForUpdate()
            ->get();
        $sourcePaymentIds = $directPayments->pluck('id')
            ->merge($allocations->pluck('payment_id'))
            ->unique()
            ->values();
        $sourcePayments = $sourcePaymentIds->isEmpty()
            ? collect()
            : Payment::query()->whereIn('id', $sourcePaymentIds)->lockForUpdate()->get()->keyBy('id');
        $refundsByPayment = $sourcePaymentIds->isEmpty()
            ? collect()
            : Payment::query()
                ->whereIn('reverses_payment_id', $sourcePaymentIds)
                ->where('type', PaymentType::Refund)
                ->lockForUpdate()
                ->get()
                ->groupBy('reverses_payment_id');
        $contributions = [];

        foreach ($directPayments as $payment) {
            $contributions[$payment->id] = Money::add($contributions[$payment->id] ?? '0', $payment->amount);
        }

        foreach ($allocations as $allocation) {
            $payment = $sourcePayments->get($allocation->payment_id);

            if ($payment && Money::isPositive($payment->amount)) {
                $contributions[$payment->id] = Money::add($contributions[$payment->id] ?? '0', $allocation->amount);
            }
        }

        $paidTotal = '0.000';
        $netSourceIds = [];

        foreach ($contributions as $paymentId => $contribution) {
            $alreadyRefunded = Money::sum(($refundsByPayment->get($paymentId) ?? collect())->map(
                fn (Payment $payment) => ltrim($payment->amount, '-'),
            ));
            $netContribution = Money::compare($alreadyRefunded, $contribution) >= 0
                ? '0.000'
                : Money::subtract($contribution, $alreadyRefunded);

            if (Money::isPositive($netContribution)) {
                $paidTotal = Money::add($paidTotal, $netContribution);
                $netSourceIds[] = (int) $paymentId;
            }
        }

        $consumedValue = $subscription->isMonthly()
            ? Money::divide(
                Money::multiply($subscription->net_total, (string) $elapsedDays),
                (int) $periodDays,
            )
            : Money::divide(
                Money::multiply($subscription->net_total, (string) $subscription->sessions_used),
                $subscription->sessions_count,
            );
        $refundAmount = Money::compare($paidTotal, $consumedValue) > 0
            ? Money::subtract($paidTotal, $consumedValue)
            : '0.000';
        $replacementDebtAmount = Money::compare($consumedValue, $paidTotal) > 0
            ? Money::subtract($consumedValue, $paidTotal)
            : '0.000';
        $debtsToVoid = $debts->filter(fn (Debt $debt) => in_array($debt->status, [DebtStatus::Open, DebtStatus::Partial], true));

        return [
            'plan_type' => $subscription->plan_type->value,
            'sessions_count' => $subscription->isMonthly() ? null : $subscription->sessions_count,
            'sessions_used' => $subscription->isMonthly() ? null : $subscription->sessions_used,
            'sessions_remaining' => $remainingSessions,
            'period_days' => $periodDays,
            'elapsed_days' => $elapsedDays,
            'remaining_days' => $remainingDays,
            'per_session_value' => $subscription->isMonthly()
                ? null
                : Money::divide($subscription->net_total, $subscription->sessions_count),
            'consumed_value' => $consumedValue,
            'paid_total' => $paidTotal,
            'refund_amount' => $refundAmount,
            'debt_voided_amount' => Money::sum($debtsToVoid->map(fn (Debt $debt) => Money::subtract($debt->amount, $debt->paid_amount))),
            'replacement_debt_amount' => $replacementDebtAmount,
            'source_payment_ids' => $netSourceIds,
            'debts_to_void' => $debtsToVoid,
        ];
    }

    /** @param array<string, mixed> $state */
    private function publicCancellationPreview(array $state): array
    {
        return Arr::only($state, [
            'plan_type',
            'sessions_count',
            'sessions_used',
            'sessions_remaining',
            'period_days',
            'elapsed_days',
            'remaining_days',
            'per_session_value',
            'consumed_value',
            'paid_total',
            'refund_amount',
            'debt_voided_amount',
            'replacement_debt_amount',
        ]);
    }
}
