<?php

namespace App\Services;

use App\Enums\CoverageType;
use App\Enums\DebtKind;
use App\Enums\DebtStatus;
use App\Enums\PaymentMethod;
use App\Enums\PaymentType;
use App\Enums\SessionServiceItemType;
use App\Enums\SessionStatus;
use App\Enums\SessionType;
use App\Enums\SubscriptionPlanType;
use App\Enums\SubscriptionStatus;
use App\Enums\SubscriptionUsageType;
use App\Enums\UserRole;
use App\Exceptions\ApiException;
use App\Models\Debt;
use App\Models\Doctor;
use App\Models\Patient;
use App\Models\Payment;
use App\Models\Service;
use App\Models\Session;
use App\Models\SessionAddition;
use App\Models\SessionService as SessionServiceModel;
use App\Models\Subscription;
use App\Models\SubscriptionUsage;
use App\Models\User;
use App\Models\WalletTransaction;
use App\Support\Money;
use Illuminate\Support\Arr;
use Illuminate\Support\Collection;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Str;

class SessionService
{
    public function __construct(
        private AuditLogger $auditLogger,
        private WalletService $walletService,
        private PaymentService $paymentService,
    ) {}

    /** @param array<string, mixed> $data */
    public function create(array $data, User $actor): Session
    {
        return DB::transaction(function () use ($data, $actor): Session {
            [$attributes, $services] = $this->prepareOpenSession($data);

            $session = Session::create([
                ...$attributes,
                'code' => 'pending-'.Str::uuid(),
                'status' => SessionStatus::Open,
                'opened_at' => now(),
                'created_by' => $actor->id,
            ]);
            $session->update(['code' => 'S-'.str_pad((string) (1000 + $session->id), 4, '0', STR_PAD_LEFT)]);
            $this->replaceSnapshots($session, $services, $actor);
            $this->auditLogger->log($actor, 'session_created', $session, $session->id, null, $this->auditValues($session));

            return $this->loadSession($session);
        });
    }

    /**
     * @param  array<string, mixed>  $data
     * @return array{session: Session, effects: array<string, mixed>, replayed: bool}
     */
    public function createAndClose(array $data, User $actor): array
    {
        return DB::transaction(function () use ($data, $actor): array {
            $session = $this->create($data, $actor);

            return $this->close($session, $actor);
        });
    }

    /** @param array<string, mixed> $data */
    public function update(Session $session, array $data, User $actor): Session
    {
        return DB::transaction(function () use ($session, $data, $actor): Session {
            $locked = Session::query()->lockForUpdate()->findOrFail($session->id);

            if ($locked->status !== SessionStatus::Open) {
                if ($actor->role === UserRole::Receptionist && $locked->status === SessionStatus::Closed) {
                    throw ApiException::forbidden('لا يستطيع موظف الاستقبال تعديل جلسة مغلقة.');
                }

                throw ApiException::conflict('يمكن تعديل الجلسة المفتوحة فقط.');
            }

            $old = $this->auditValues($this->loadSession($locked));
            [$attributes, $services] = $this->prepareOpenSession($this->mergeOpenSessionData($locked, $data));
            $locked->update($attributes);
            $this->replaceSnapshots($locked, $services, $actor);
            $fresh = $this->loadSession($locked);
            $this->auditLogger->log($actor, 'session_updated', $fresh, $fresh->id, $old, $this->auditValues($fresh));

            return $fresh;
        });
    }

    /**
     * @param  array<string, mixed>  $data
     * @return array{session: Session, addition: SessionAddition, effects: array<string, mixed>}
     */
    public function addServices(Session $session, array $data, User $actor): array
    {
        return DB::transaction(function () use ($session, $data, $actor): array {
            $locked = Session::query()->lockForUpdate()->findOrFail($session->id);

            if ($locked->status !== SessionStatus::Closed) {
                throw ApiException::conflict('يمكن إضافة خدمات إلى جلسة مغلقة فقط.');
            }

            $old = $this->auditValues($this->loadSession($locked));
            $items = $this->resolveServiceItems($data, true);
            $additionTotal = Money::sum($items->pluck('price'));
            $walletAmount = '0.000';
            $paidAmount = '0.000';
            $debtAmount = '0.000';
            $paymentMethod = null;
            $sponsorId = null;
            $coverage = CoverageType::Subscription;
            $patient = Patient::query()->lockForUpdate()->findOrFail($locked->patient_id);

            if ($locked->type === SessionType::Subscription) {
                $subscription = Subscription::query()->lockForUpdate()->findOrFail($locked->subscription_id);

                if ($subscription->plan_type !== SubscriptionPlanType::Monthly) {
                    throw ApiException::validation('إضافة خدمات بعد الإغلاق متاحة للاشتراكات الشهرية فقط.', [
                        'services' => ['استخدم تصحيح المدير للاشتراكات القديمة المعتمدة على عدد الجلسات.'],
                    ]);
                }

                if ($subscription->status !== SubscriptionStatus::Active
                    || $subscription->start_date?->gt(today())
                    || $subscription->end_date?->lt(today())) {
                    throw ApiException::validation('الاشتراك الشهري لم يعد صالحًا.', [
                        'subscription_id' => ['لا يمكن إضافة خدمات مغطاة باشتراك خارج مدته.'],
                    ]);
                }
            } else {
                $settlement = $data['settlement'] ?? [];
                $walletAmount = Money::normalize($settlement['wallet_amount'] ?? '0');
                $paidAmount = Money::normalize($settlement['paid_amount'] ?? '0');
                $debtAmount = Money::normalize($settlement['debt_amount'] ?? '0');

                if (Money::compare(Money::sum([$walletAmount, $paidAmount, $debtAmount]), $additionTotal) !== 0) {
                    throw ApiException::validation('مجموع تسوية الخدمات الإضافية يجب أن يساوي قيمتها.', [
                        'settlement' => ['راجع مبالغ الرصيد والمدفوع والدين.'],
                    ]);
                }

                if (Money::isPositive($paidAmount) && empty($settlement['method'])) {
                    throw ApiException::validation('طريقة الدفع مطلوبة عند تسجيل مبلغ مدفوع.', [
                        'settlement.method' => ['اختر نقدًا أو تحويلًا مصرفيًا.'],
                    ]);
                }

                $paymentMethod = ! empty($settlement['method'])
                    ? PaymentMethod::from($settlement['method'])
                    : null;
                $sponsorId = $settlement['sponsor_id'] ?? null;
                $coverage = $this->coverage($walletAmount, $paidAmount, $debtAmount, $paymentMethod);
            }

            $addition = SessionAddition::create([
                'session_id' => $locked->id,
                'services_total' => $additionTotal,
                'net_total' => $additionTotal,
                'coverage' => $coverage,
                'wallet_amount' => $walletAmount,
                'paid_amount' => $paidAmount,
                'debt_amount' => $debtAmount,
                'payment_method' => $paymentMethod,
                'sponsor_id' => $sponsorId,
                'created_by' => $actor->id,
            ]);

            foreach ($items as $item) {
                SessionServiceModel::create([
                    'session_id' => $locked->id,
                    'session_addition_id' => $addition->id,
                    'service_id' => $item['service_id'],
                    'service_name' => $item['service_name'],
                    'price' => $item['price'],
                    'base_price' => $item['base_price'],
                    'included_units' => $item['included_units'],
                    'extra_units' => $item['extra_units'],
                    'unit_name' => $item['unit_name'],
                    'extra_unit_price' => $item['extra_unit_price'],
                    'item_type' => $item['item_type'],
                    'is_addition' => true,
                    'added_at' => now(),
                    'created_by' => $actor->id,
                ]);
            }

            $effects = $locked->type === SessionType::Subscription
                ? [
                    'subscription_usage' => null,
                    'wallet_transaction' => null,
                    'payment' => null,
                    'debt' => null,
                ]
                : $this->applySettlementEffects(
                    $locked,
                    $patient,
                    $walletAmount,
                    $paidAmount,
                    $debtAmount,
                    $paymentMethod,
                    $sponsorId,
                    $actor,
                    PaymentType::Adjustment,
                    $addition,
                );

            $newWalletAmount = Money::add($locked->wallet_amount, $walletAmount);
            $newPaidAmount = Money::add($locked->paid_amount, $paidAmount);
            $newDebtAmount = Money::add($locked->debt_amount, $debtAmount);
            $newPaymentMethod = $locked->payment_method;

            if (Money::isPositive($paidAmount)) {
                $newPaymentMethod = Money::isZero($locked->paid_amount) || $locked->payment_method === $paymentMethod
                    ? $paymentMethod
                    : null;
            }

            $commissionDelta = Money::percentage($additionTotal, $locked->commission_rate ?? '0');
            $locked->update([
                'services_total' => Money::add($locked->services_total, $additionTotal),
                'net_total' => Money::add($locked->net_total, $additionTotal),
                'wallet_amount' => $newWalletAmount,
                'paid_amount' => $newPaidAmount,
                'debt_amount' => $newDebtAmount,
                'coverage' => $locked->type === SessionType::Subscription
                    ? CoverageType::Subscription
                    : $this->coverage($newWalletAmount, $newPaidAmount, $newDebtAmount, $newPaymentMethod),
                'payment_method' => $newPaymentMethod,
                'commission_base_amount' => Money::add($locked->commission_base_amount ?? '0', $additionTotal),
                'commission_amount' => Money::add($locked->commission_amount ?? '0', $commissionDelta),
            ]);

            $fresh = $this->loadSession($locked);
            $this->auditLogger->log($actor, 'session_services_added', $fresh, $fresh->id, $old, [
                ...$this->auditValues($fresh),
                'addition_id' => $addition->id,
            ]);

            return [
                'session' => $fresh,
                'addition' => $addition->fresh(['services']),
                'effects' => $effects,
            ];
        });
    }

    /**
     * @return array{session: Session, effects: array<string, mixed>, replayed: bool}
     */
    public function close(Session $session, User $actor): array
    {
        return DB::transaction(function () use ($session, $actor): array {
            $locked = Session::query()->lockForUpdate()->findOrFail($session->id);

            if ($locked->status === SessionStatus::Closed) {
                return [
                    'session' => $this->loadSession($locked),
                    'effects' => $this->existingEffects($locked),
                    'replayed' => true,
                ];
            }

            if ($locked->status === SessionStatus::Cancelled) {
                throw ApiException::conflict('لا يمكن إغلاق جلسة ملغاة.');
            }

            $patient = Patient::query()->lockForUpdate()->findOrFail($locked->patient_id);
            $effects = [
                'subscription_usage' => null,
                'wallet_transaction' => null,
                'payment' => null,
                'debt' => null,
            ];

            if ($locked->type === SessionType::Subscription) {
                $effects['subscription_usage'] = $this->consumeSubscription($locked, $actor);
            } else {
                $effects = $this->applyIndividualEffects($locked, $patient, $actor, PaymentType::SessionPayment);
            }

            $doctor = Doctor::query()->findOrFail($locked->doctor_id);
            $commissionBase = config('clinic.commission_base') === 'gross'
                ? Money::normalize($locked->services_total)
                : Money::normalize($locked->net_total);

            $locked->update([
                'status' => SessionStatus::Closed,
                'closed_at' => now(),
                'closed_by' => $actor->id,
                'commission_rate' => Money::normalize($doctor->commission_rate),
                'commission_base_amount' => $commissionBase,
                'commission_amount' => Money::percentage($commissionBase, $doctor->commission_rate),
            ]);

            $fresh = $this->loadSession($locked);
            $this->auditLogger->log($actor, 'session_closed', $fresh, $fresh->id, ['status' => SessionStatus::Open->value], $this->auditValues($fresh));

            return ['session' => $fresh, 'effects' => $effects, 'replayed' => false];
        });
    }

    /** @return array{session: Session, reversals: array<string, mixed>} */
    public function cancel(Session $session, string $reason, User $actor): array
    {
        return DB::transaction(function () use ($session, $reason, $actor): array {
            $locked = Session::query()->lockForUpdate()->findOrFail($session->id);

            if ($locked->status === SessionStatus::Cancelled) {
                throw ApiException::conflict('تم إلغاء الجلسة بالفعل.');
            }

            if ($locked->status === SessionStatus::Closed && $actor->role !== UserRole::Admin) {
                throw ApiException::forbidden('إلغاء جلسة مغلقة متاح لمدير النظام فقط.');
            }

            $old = $this->auditValues($this->loadSession($locked));
            $wasClosed = $locked->status === SessionStatus::Closed;
            $reversals = $wasClosed
                ? $this->reverseClosedEffects($locked, $reason, $actor)
                : [];

            if ($wasClosed) {
                $reversals['commission_removed'] = [
                    'rate' => Money::normalize($locked->commission_rate),
                    'base_amount' => Money::normalize($locked->commission_base_amount),
                    'amount' => Money::normalize($locked->commission_amount),
                ];
            }

            $locked->update([
                'status' => SessionStatus::Cancelled,
                'cancelled_at' => now(),
                'cancelled_by' => $actor->id,
                'cancel_reason' => $reason,
                'commission_rate' => $wasClosed ? null : $locked->commission_rate,
                'commission_base_amount' => $wasClosed ? null : $locked->commission_base_amount,
                'commission_amount' => $wasClosed ? null : $locked->commission_amount,
            ]);

            $fresh = $this->loadSession($locked);
            $this->auditLogger->log($actor, 'session_cancelled', $fresh, $fresh->id, $old, $this->auditValues($fresh), $reason);

            return ['session' => $fresh, 'reversals' => $reversals];
        });
    }

    /**
     * @param  array<string, mixed>  $changes
     * @return array{session: Session, financial_impact: array<string, mixed>}
     */
    public function correct(Session $session, array $changes, string $reason, User $actor): array
    {
        return DB::transaction(function () use ($session, $changes, $reason, $actor): array {
            $locked = Session::query()->lockForUpdate()->findOrFail($session->id);

            if ($locked->status !== SessionStatus::Closed) {
                throw ApiException::conflict('يمكن تصحيح الجلسات المغلقة فقط.');
            }

            $changes = [...($changes['changes'] ?? []), ...Arr::except($changes, ['changes', 'reason'])];
            $old = $this->auditValues($this->loadSession($locked));
            $oldNetTotal = Money::normalize($locked->net_total);
            $oldCommissionAmount = Money::normalize($locked->commission_amount);
            $oldCommissionRate = Money::normalize($locked->commission_rate);
            $oldDoctorId = $locked->doctor_id;

            if ($locked->type === SessionType::Subscription) {
                if (array_diff(array_keys($changes), ['doctor_id'])) {
                    throw ApiException::validation('تصحيح جلسة الاشتراك يقتصر على الطبيب دون تغيير الاستهلاك أو القيمة.', [
                        'changes' => ['ألغِ الجلسة المغلقة ثم افتح جلسة صحيحة لتغيير الاشتراك أو الخدمات.'],
                    ]);
                }

                if (isset($changes['doctor_id'])) {
                    $doctor = Doctor::query()->where('is_active', true)->findOrFail($changes['doctor_id']);
                    $commissionBase = config('clinic.commission_base') === 'gross'
                        ? Money::normalize($locked->services_total)
                        : Money::normalize($locked->net_total);
                    $locked->update([
                        'doctor_id' => $doctor->id,
                        'commission_rate' => Money::normalize($doctor->commission_rate),
                        'commission_base_amount' => $commissionBase,
                        'commission_amount' => Money::percentage($commissionBase, $doctor->commission_rate),
                    ]);
                }

                $impact = ['commission_adjusted' => true];
            } else {
                if ($locked->additions()->exists()) {
                    throw ApiException::conflict('لا يمكن استبدال جلسة مغلقة تحتوي على إضافات. ألغِ الجلسة وأنشئ جلسة صحيحة للحفاظ على السجل المالي.');
                }

                $correctionData = $this->mergeOpenSessionData($locked, $changes);
                $correctionData['type'] = SessionType::Individual->value;

                $impact = $this->reverseClosedEffects($locked, 'تصحيح جلسة مغلقة: '.$reason, $actor);
                [$attributes, $services] = $this->prepareOpenSession($correctionData);
                $locked->update($attributes);
                $this->replaceSnapshots($locked, $services, $actor);
                $patient = Patient::query()->lockForUpdate()->findOrFail($locked->patient_id);
                $newEffects = $this->applyIndividualEffects($locked, $patient, $actor, PaymentType::Adjustment);
                $commissionRate = $oldCommissionRate;

                if ($locked->doctor_id !== $oldDoctorId) {
                    $commissionRate = Money::normalize(
                        Doctor::query()->findOrFail($locked->doctor_id)->commission_rate,
                    );
                }

                $commissionBase = config('clinic.commission_base') === 'gross'
                    ? Money::normalize($locked->services_total)
                    : Money::normalize($locked->net_total);
                $locked->update([
                    'commission_rate' => $commissionRate,
                    'commission_base_amount' => $commissionBase,
                    'commission_amount' => Money::percentage($commissionBase, $commissionRate),
                ]);
                $impact['replacement'] = $newEffects;
            }

            $fresh = $this->loadSession($locked);
            $impact['net_delta'] = Money::subtract($fresh->net_total, $oldNetTotal);
            $impact['commission_delta'] = Money::subtract($fresh->commission_amount ?? '0', $oldCommissionAmount);
            $this->auditLogger->log($actor, 'session_corrected', $fresh, $fresh->id, $old, $this->auditValues($fresh), $reason);

            return ['session' => $fresh, 'financial_impact' => $impact];
        });
    }

    /**
     * @param  array<string, mixed>  $data
     * @return array{0: array<string, mixed>, 1: iterable<int, Service|object>}
     */
    private function prepareOpenSession(array $data): array
    {
        $patientQuery = Patient::query()->lockForUpdate();

        if (($data['_existing_patient_id'] ?? null) !== $data['patient_id']) {
            $patientQuery->where('is_active', true);
        }

        $doctorQuery = Doctor::query();

        if (($data['_existing_doctor_id'] ?? null) !== $data['doctor_id']) {
            $doctorQuery->where('is_active', true);
        }

        $patient = $patientQuery->findOrFail($data['patient_id']);
        $doctor = $doctorQuery->findOrFail($data['doctor_id']);
        $type = SessionType::from($data['type']);

        if ($type === SessionType::Subscription) {
            if (empty($data['subscription_id'])) {
                throw ApiException::validation('الاشتراك مطلوب للجلسة من نوع اشتراك.', [
                    'subscription_id' => ['اختر اشتراكًا نشطًا للمريض.'],
                ]);
            }

            $subscription = Subscription::query()->with('services')->lockForUpdate()->findOrFail($data['subscription_id']);

            if ($subscription->patient_id !== $patient->id || $subscription->status !== SubscriptionStatus::Active) {
                throw ApiException::validation('الاشتراك المحدد غير صالح لهذا المريض.', [
                    'subscription_id' => ['اختر اشتراكًا نشطًا يخص المريض.'],
                ]);
            }

            if ($subscription->plan_type === SubscriptionPlanType::Monthly) {
                if ($subscription->start_date?->gt(today()) || $subscription->end_date?->lt(today())) {
                    throw ApiException::validation('الاشتراك الشهري خارج مدة الصلاحية.', [
                        'subscription_id' => ['يجب أن يكون تاريخ اليوم بين بداية الاشتراك ونهايته.'],
                    ]);
                }

                $services = $this->resolveServiceItems($data);
                $servicesTotal = Money::sum($services->pluck('price'));

                return [[
                    'patient_id' => $patient->id,
                    'doctor_id' => $doctor->id,
                    'subscription_id' => $subscription->id,
                    'type' => SessionType::Subscription,
                    'services_total' => $servicesTotal,
                    'discount' => '0.000',
                    'net_total' => $servicesTotal,
                    'coverage' => CoverageType::Subscription,
                    'wallet_amount' => '0.000',
                    'paid_amount' => '0.000',
                    'debt_amount' => '0.000',
                    'payment_method' => null,
                    'sponsor_id' => null,
                ], $services];
            }

            if ($subscription->sessions_used >= $subscription->sessions_count) {
                throw ApiException::validation('لا توجد جلسات متبقية في الاشتراك.', [
                    'subscription_id' => ['الاشتراك مكتمل.'],
                ]);
            }

            $services = $subscription->services->map(fn ($service): array => [
                'service_id' => $service->service_id,
                'service_name' => $service->service_name,
                'price' => Money::normalize($service->price),
                'base_price' => Money::normalize($service->price),
                'included_units' => 0,
                'extra_units' => 0,
                'unit_name' => null,
                'extra_unit_price' => null,
                'item_type' => SessionServiceItemType::Service,
            ]);
            $servicesTotal = Money::sum($services->pluck('price'));
            $netTotal = Money::divide($subscription->net_total, $subscription->sessions_count);

            return [[
                'patient_id' => $patient->id,
                'doctor_id' => $doctor->id,
                'subscription_id' => $subscription->id,
                'type' => SessionType::Subscription,
                'services_total' => $servicesTotal,
                'discount' => Money::subtract($servicesTotal, $netTotal),
                'net_total' => $netTotal,
                'coverage' => CoverageType::Subscription,
                'wallet_amount' => '0.000',
                'paid_amount' => '0.000',
                'debt_amount' => '0.000',
                'payment_method' => null,
                'sponsor_id' => null,
            ], $services];
        }

        if (empty($data['service_ids']) && empty($data['services']) && empty($data['_service_snapshots'])) {
            throw ApiException::validation('اختر خدمة واحدة على الأقل للجلسة الفردية.', [
                'service_ids' => ['حقل الخدمات مطلوب.'],
            ]);
        }

        $services = $this->resolveServiceItems($data);

        $servicesTotal = Money::sum($services->pluck('price'));
        $discount = Money::normalize($data['discount'] ?? '0');

        if (Money::compare($discount, $servicesTotal) > 0) {
            throw ApiException::validation('الخصم لا يمكن أن يتجاوز قيمة الخدمات.', [
                'discount' => ['قيمة الخصم أكبر من إجمالي الخدمات.'],
            ]);
        }

        $netTotal = Money::subtract($servicesTotal, $discount);
        $settlement = $data['settlement'] ?? [];
        $walletAmount = Money::normalize($settlement['wallet_amount'] ?? '0');
        $paidAmount = Money::normalize($settlement['paid_amount'] ?? '0');
        $debtAmount = Money::normalize($settlement['debt_amount'] ?? '0');

        if (Money::compare(Money::sum([$walletAmount, $paidAmount, $debtAmount]), $netTotal) !== 0) {
            throw ApiException::validation('مجموع أجزاء التسوية يجب أن يساوي صافي قيمة الجلسة.', [
                'settlement' => ['راجع مبالغ الرصيد والمدفوع والدين.'],
            ]);
        }

        if (Money::isPositive($paidAmount) && empty($settlement['method'])) {
            throw ApiException::validation('طريقة الدفع مطلوبة عند تسجيل مبلغ مدفوع.', [
                'settlement.method' => ['اختر نقدًا أو تحويلًا مصرفيًا.'],
            ]);
        }

        if (Money::isPositive($walletAmount) && Money::compare($this->walletService->balance($patient), $walletAmount) < 0) {
            throw ApiException::validation('الرصيد غير كافٍ. اشحن رصيد المريض أولًا ثم أعد المحاولة.', [
                'settlement.wallet_amount' => ['الرصيد المتاح أقل من المبلغ المطلوب.'],
            ]);
        }

        $method = ! empty($settlement['method']) ? PaymentMethod::from($settlement['method']) : null;

        return [[
            'patient_id' => $patient->id,
            'doctor_id' => $doctor->id,
            'subscription_id' => null,
            'type' => SessionType::Individual,
            'services_total' => $servicesTotal,
            'discount' => $discount,
            'net_total' => $netTotal,
            'coverage' => $this->coverage($walletAmount, $paidAmount, $debtAmount, $method),
            'wallet_amount' => $walletAmount,
            'paid_amount' => $paidAmount,
            'debt_amount' => $debtAmount,
            'payment_method' => $method,
            'sponsor_id' => $settlement['sponsor_id'] ?? null,
        ], $services];
    }

    /**
     * @param  array<string, mixed>  $data
     * @return Collection<int, array<string, mixed>>
     */
    private function resolveServiceItems(array $data, bool $allowExtraOnly = false): Collection
    {
        if (isset($data['_service_snapshots'])) {
            return collect($data['_service_snapshots'])->map(fn ($service): array => [
                'service_id' => $service->service_id,
                'service_name' => $service->service_name,
                'price' => Money::normalize($service->price),
                'base_price' => Money::normalize($service->base_price ?? $service->price),
                'included_units' => (int) ($service->included_units ?? 0),
                'extra_units' => (int) ($service->extra_units ?? 0),
                'unit_name' => $service->unit_name,
                'extra_unit_price' => $service->extra_unit_price === null
                    ? null
                    : Money::normalize($service->extra_unit_price),
                'item_type' => $service->item_type ?? SessionServiceItemType::Service,
            ])->values();
        }

        $rawItems = collect($data['services'] ?? collect($data['service_ids'] ?? [])->map(
            fn ($serviceId): array => ['service_id' => $serviceId],
        )->all());

        if ($rawItems->isEmpty()) {
            throw ApiException::validation('اختر خدمة واحدة على الأقل.', [
                'services' => ['حقل الخدمات مطلوب.'],
            ]);
        }

        $serviceIds = $rawItems->pluck('service_id')->map(fn ($id): int => (int) $id)->unique()->values();
        $services = Service::query()
            ->whereIn('id', $serviceIds)
            ->where('is_active', true)
            ->get()
            ->keyBy('id');

        if ($services->count() !== $serviceIds->count()) {
            throw ApiException::validation('إحدى الخدمات غير متاحة للجلسات الجديدة.', [
                'services' => ['اختر خدمات نشطة فقط.'],
            ]);
        }

        $seen = [];

        return $rawItems->map(function (array $item) use ($allowExtraOnly, $services, &$seen): array {
            $service = $services->get((int) $item['service_id']);
            $itemType = SessionServiceItemType::from($item['item_type'] ?? SessionServiceItemType::Service->value);
            $extraUnits = (int) ($item['extra_units'] ?? 0);
            $dedupeKey = $service->id.':'.$itemType->value;

            if (isset($seen[$dedupeKey])) {
                throw ApiException::validation('لا يمكن تكرار بند الخدمة نفسه في العملية الواحدة.', [
                    'services' => ['ادمج الكمية الإضافية في بند واحد.'],
                ]);
            }
            $seen[$dedupeKey] = true;

            if ($itemType === SessionServiceItemType::ExtraUnits && ! $allowExtraOnly) {
                throw ApiException::validation('الوحدات الإضافية المنفردة متاحة عند الإضافة إلى جلسة مغلقة فقط.', [
                    'services' => ['اختر الخدمة الأساسية عند إنشاء الجلسة.'],
                ]);
            }

            if ($extraUnits > 0 && ($service->included_units === null || $service->extra_unit_price === null)) {
                throw ApiException::validation('الخدمة المحددة لا تدعم وحدات إضافية.', [
                    'services' => ['راجع إعدادات ملحقات الخدمة.'],
                ]);
            }

            if ($itemType === SessionServiceItemType::ExtraUnits && $extraUnits < 1) {
                throw ApiException::validation('أدخل عدد الوحدات الإضافية.', [
                    'services' => ['عدد الوحدات الإضافية يجب أن يكون 1 على الأقل.'],
                ]);
            }

            $basePrice = $itemType === SessionServiceItemType::Service
                ? Money::normalize($service->price)
                : '0.000';
            $extraUnitPrice = $service->extra_unit_price === null
                ? null
                : Money::normalize($service->extra_unit_price);
            $extrasTotal = $extraUnitPrice === null
                ? '0.000'
                : Money::multiply($extraUnitPrice, (string) $extraUnits);

            return [
                'service_id' => $service->id,
                'service_name' => $service->name,
                'price' => Money::add($basePrice, $extrasTotal),
                'base_price' => $basePrice,
                'included_units' => $itemType === SessionServiceItemType::Service
                    ? (int) ($service->included_units ?? 0)
                    : 0,
                'extra_units' => $extraUnits,
                'unit_name' => $service->unit_name,
                'extra_unit_price' => $extraUnitPrice,
                'item_type' => $itemType,
            ];
        })->values();
    }

    /**
     * @param  array<string, mixed>  $data
     * @return array<string, mixed>
     */
    private function mergeOpenSessionData(Session $session, array $data): array
    {
        $serviceSnapshots = $session->services()->get();
        $currentServiceIds = $serviceSnapshots->pluck('service_id')->map(fn ($id) => (int) $id)->sort()->values()->all();
        $settlement = [
            'coverage' => $session->coverage->value,
            'wallet_amount' => $session->wallet_amount,
            'paid_amount' => $session->paid_amount,
            'debt_amount' => $session->debt_amount,
            'method' => $session->payment_method?->value,
            'sponsor_id' => $session->sponsor_id,
        ];
        $merged = [
            'patient_id' => $session->patient_id,
            'doctor_id' => $session->doctor_id,
            'type' => $session->type->value,
            'subscription_id' => $session->subscription_id,
            'service_ids' => $currentServiceIds,
            'discount' => $session->discount,
            'settlement' => $settlement,
            '_existing_patient_id' => $session->patient_id,
            '_existing_doctor_id' => $session->doctor_id,
            ...$data,
        ];

        if (isset($data['settlement'])) {
            $merged['settlement'] = [...$settlement, ...$data['settlement']];
        }

        $requestedServiceIds = collect($merged['service_ids'] ?? [])
            ->map(fn ($id) => (int) $id)
            ->sort()
            ->values()
            ->all();
        $targetType = SessionType::from($merged['type']);

        if ($session->type === SessionType::Individual
            && $targetType === SessionType::Individual
            && $requestedServiceIds === $currentServiceIds) {
            $merged['_service_snapshots'] = $serviceSnapshots;
        }

        return $merged;
    }

    /** @param iterable<int, array<string, mixed>> $services */
    private function replaceSnapshots(Session $session, iterable $services, User $actor): void
    {
        $session->services()->delete();

        foreach ($services as $service) {
            SessionServiceModel::create([
                'session_id' => $session->id,
                'service_id' => $service['service_id'],
                'service_name' => $service['service_name'],
                'price' => Money::normalize($service['price']),
                'base_price' => Money::normalize($service['base_price']),
                'included_units' => $service['included_units'],
                'extra_units' => $service['extra_units'],
                'unit_name' => $service['unit_name'],
                'extra_unit_price' => $service['extra_unit_price'],
                'item_type' => $service['item_type'],
                'is_addition' => false,
                'created_by' => $actor->id,
            ]);
        }
    }

    private function consumeSubscription(Session $session, User $actor): ?SubscriptionUsage
    {
        $subscription = Subscription::query()->lockForUpdate()->findOrFail($session->subscription_id);

        if ($subscription->status !== SubscriptionStatus::Active) {
            throw ApiException::validation('الاشتراك لم يعد صالحًا للاستخدام.', [
                'subscription_id' => ['الاشتراك موقوف أو مكتمل أو ملغي.'],
            ]);
        }

        if ($subscription->plan_type === SubscriptionPlanType::Monthly) {
            if ($subscription->start_date?->gt(today()) || $subscription->end_date?->lt(today())) {
                throw ApiException::validation('الاشتراك الشهري خارج مدة الصلاحية.', [
                    'subscription_id' => ['يجب أن يكون تاريخ اليوم بين بداية الاشتراك ونهايته.'],
                ]);
            }

            return null;
        }

        if ($subscription->sessions_used >= $subscription->sessions_count) {
            throw ApiException::validation('الاشتراك لم يعد صالحًا للاستخدام.', [
                'subscription_id' => ['لا توجد جلسات متبقية.'],
            ]);
        }

        $coveredServiceIds = $subscription->services()->pluck('service_id')->sort()->values()->all();
        $sessionServiceIds = $session->services()->pluck('service_id')->sort()->values()->all();

        if ($coveredServiceIds !== $sessionServiceIds) {
            throw ApiException::validation('خدمات الجلسة لا تطابق خدمات الاشتراك.', [
                'subscription_id' => ['أعد فتح الجلسة من الاشتراك الصحيح.'],
            ]);
        }

        $usage = SubscriptionUsage::create([
            'subscription_id' => $subscription->id,
            'session_id' => $session->id,
            'type' => SubscriptionUsageType::Consume,
            'created_by' => $actor->id,
        ]);

        $used = $subscription->sessions_used + 1;
        $subscription->update([
            'sessions_used' => $used,
            'status' => $used >= $subscription->sessions_count
                ? SubscriptionStatus::Completed
                : SubscriptionStatus::Active,
        ]);

        return $usage;
    }

    /** @return array{subscription_usage: null, wallet_transaction: ?WalletTransaction, payment: ?Payment, debt: ?Debt} */
    private function applyIndividualEffects(Session $session, Patient $patient, User $actor, PaymentType $paymentType): array
    {
        return $this->applySettlementEffects(
            $session,
            $patient,
            Money::normalize($session->wallet_amount),
            Money::normalize($session->paid_amount),
            Money::normalize($session->debt_amount),
            $session->payment_method,
            $session->sponsor_id,
            $actor,
            $paymentType,
        );
    }

    /** @return array{subscription_usage: null, wallet_transaction: ?WalletTransaction, payment: ?Payment, debt: ?Debt} */
    private function applySettlementEffects(
        Session $session,
        Patient $patient,
        string $walletAmount,
        string $paidAmount,
        string $debtAmount,
        ?PaymentMethod $paymentMethod,
        ?int $sponsorId,
        User $actor,
        PaymentType $paymentType,
        ?SessionAddition $addition = null,
    ): array {
        $walletTransaction = $this->walletService->debit($session, $patient, $walletAmount, $actor, $addition);
        $payment = null;

        if (Money::isPositive($paidAmount)) {
            $payment = Payment::create([
                'patient_id' => $patient->id,
                'session_id' => $session->id,
                'session_addition_id' => $addition?->id,
                'type' => $paymentType,
                'method' => $paymentMethod,
                'amount' => $paidAmount,
                'created_by' => $actor->id,
            ]);
            $this->auditLogger->log($actor, 'payment_recorded', $payment, $payment->id, null, [
                'session_id' => $session->id,
                'type' => $paymentType->value,
                'method' => $payment->method->value,
                'amount' => Money::normalize($payment->amount),
            ]);
        }

        $debt = null;

        if (Money::isPositive($debtAmount)) {
            $debt = Debt::create([
                'patient_id' => $patient->id,
                'sponsor_id' => $sponsorId,
                'session_id' => $session->id,
                'session_addition_id' => $addition?->id,
                'kind' => DebtKind::Session,
                'amount' => $debtAmount,
                'paid_amount' => '0.000',
                'status' => DebtStatus::Open,
                'created_by' => $actor->id,
            ]);
        }

        return [
            'subscription_usage' => null,
            'wallet_transaction' => $walletTransaction,
            'payment' => $payment,
            'debt' => $debt,
        ];
    }

    /** @return array<string, mixed> */
    private function reverseClosedEffects(Session $session, string $reason, User $actor): array
    {
        $reversals = ['subscription_usage' => null, 'wallet' => [], 'refunds' => [], 'voided_debts' => []];

        if ($session->type === SessionType::Subscription) {
            $consume = SubscriptionUsage::query()
                ->where('session_id', $session->id)
                ->where('type', SubscriptionUsageType::Consume)
                ->lockForUpdate()
                ->first();

            if ($consume && ! SubscriptionUsage::query()->where('reverses_usage_id', $consume->id)->exists()) {
                $subscription = Subscription::query()->lockForUpdate()->findOrFail($consume->subscription_id);
                $restore = SubscriptionUsage::create([
                    'subscription_id' => $subscription->id,
                    'session_id' => $session->id,
                    'type' => SubscriptionUsageType::Restore,
                    'reverses_usage_id' => $consume->id,
                    'created_by' => $actor->id,
                ]);
                $subscription->update([
                    'sessions_used' => max(0, $subscription->sessions_used - 1),
                    'status' => in_array($subscription->status, [SubscriptionStatus::Stopped, SubscriptionStatus::Cancelled], true)
                        ? $subscription->status
                        : SubscriptionStatus::Active,
                ]);
                $reversals['subscription_usage'] = $restore;
            }

            return $reversals;
        }

        $debits = WalletTransaction::query()
            ->where('session_id', $session->id)
            ->where('amount', '<', 0)
            ->lockForUpdate()
            ->get();

        foreach ($debits as $debit) {
            if (! WalletTransaction::query()->where('reverses_transaction_id', $debit->id)->exists()) {
                $reversals['wallet'][] = $this->walletService->reverse($debit, $actor, $reason);
            }
        }

        $payments = Payment::query()
            ->where('session_id', $session->id)
            ->whereIn('type', [PaymentType::SessionPayment, PaymentType::Adjustment])
            ->where('amount', '>', 0)
            ->lockForUpdate()
            ->get();

        foreach ($payments as $payment) {
            $refunded = Money::normalize(Payment::query()
                ->where('reverses_payment_id', $payment->id)
                ->where('type', PaymentType::Refund)
                ->sum(DB::raw('ABS(amount)')));
            $refundable = Money::subtract($payment->amount, $refunded);

            if (Money::isPositive($refundable)) {
                $reversals['refunds'][] = $this->paymentService->refund(
                    $payment,
                    $refundable,
                    $payment->method,
                    $reason,
                    $actor,
                );
            }
        }

        $debts = Debt::query()
            ->where('session_id', $session->id)
            ->where('status', '!=', DebtStatus::Void)
            ->with('allocations.payment')
            ->lockForUpdate()
            ->get();

        foreach ($debts as $debt) {
            foreach ($debt->allocations as $allocation) {
                $payment = $allocation->payment;
                $alreadyRefunded = Money::normalize(Payment::query()
                    ->where('reverses_payment_id', $payment->id)
                    ->where('type', PaymentType::Refund)
                    ->sum(DB::raw('ABS(amount)')));
                $available = Money::subtract($payment->amount, $alreadyRefunded);
                $refundAmount = Money::compare($allocation->amount, $available) <= 0 ? $allocation->amount : $available;

                if (Money::isPositive($refundAmount)) {
                    $reversals['refunds'][] = $this->paymentService->refund(
                        $payment,
                        $refundAmount,
                        $payment->method,
                        $reason,
                        $actor,
                    );
                }
            }

            $debt->update([
                'status' => DebtStatus::Void,
                'voided_at' => now(),
                'voided_by' => $actor->id,
                'void_reason' => $reason,
            ]);
            $reversals['voided_debts'][] = $debt->fresh();
        }

        return $reversals;
    }

    private function coverage(string $wallet, string $paid, string $debt, ?PaymentMethod $method): CoverageType
    {
        $used = collect([
            'wallet' => Money::isPositive($wallet),
            'paid' => Money::isPositive($paid),
            'debt' => Money::isPositive($debt),
        ])->filter()->keys();

        if ($used->count() > 1) {
            return CoverageType::Mixed;
        }

        return match ($used->first()) {
            'wallet' => CoverageType::Wallet,
            'debt' => CoverageType::Debt,
            'paid' => $method === PaymentMethod::Transfer ? CoverageType::Transfer : CoverageType::Cash,
            default => CoverageType::Cash,
        };
    }

    /** @return array<string, mixed> */
    private function existingEffects(Session $session): array
    {
        return [
            'subscription_usage' => $session->subscriptionUsages()->where('type', SubscriptionUsageType::Consume)->first(),
            'wallet_transaction' => $session->walletTransactions()->where('amount', '<', 0)->latest('id')->first(),
            'payment' => $session->payments()->where('amount', '>', 0)->latest('id')->first(),
            'debt' => $session->debts()->where('status', '!=', DebtStatus::Void)->latest('id')->first(),
        ];
    }

    private function loadSession(Session $session): Session
    {
        return $session->fresh(['patient', 'doctor', 'services', 'additions', 'creator', 'closedBy', 'cancelledBy']);
    }

    /** @return array<string, mixed> */
    private function auditValues(Session $session): array
    {
        return [
            ...Arr::only($session->toArray(), [
                'patient_id', 'doctor_id', 'subscription_id', 'type', 'status', 'services_total',
                'discount', 'net_total', 'coverage', 'wallet_amount', 'paid_amount', 'debt_amount',
                'payment_method', 'sponsor_id', 'commission_rate', 'commission_base_amount',
                'commission_amount', 'closed_at', 'cancelled_at', 'cancel_reason',
            ]),
            'services' => $session->services->map(fn ($service) => [
                'service_id' => $service->service_id,
                'name' => $service->service_name,
                'price' => Money::normalize($service->price),
                'base_price' => Money::normalize($service->base_price ?? $service->price),
                'included_units' => $service->included_units,
                'extra_units' => $service->extra_units,
                'unit_name' => $service->unit_name,
                'extra_unit_price' => $service->extra_unit_price === null
                    ? null
                    : Money::normalize($service->extra_unit_price),
                'item_type' => $service->item_type->value,
                'is_addition' => $service->is_addition,
            ])->all(),
        ];
    }
}
