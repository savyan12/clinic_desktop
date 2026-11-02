<?php

namespace App\Services;

use App\Enums\DebtKind;
use App\Enums\DebtStatus;
use App\Enums\PaymentMethod;
use App\Enums\PaymentType;
use App\Exceptions\ApiException;
use App\Models\Debt;
use App\Models\Patient;
use App\Models\Payment;
use App\Models\PaymentAllocation;
use App\Models\User;
use App\Support\Money;
use Illuminate\Support\Facades\DB;

class DebtService
{
    public function __construct(private AuditLogger $auditLogger) {}

    public function createOpeningDebt(
        Patient $patient,
        string $amount,
        ?int $sponsorId,
        ?string $note,
        User $actor,
    ): Debt {
        return DB::transaction(function () use ($patient, $amount, $sponsorId, $note, $actor): Debt {
            Patient::query()->lockForUpdate()->findOrFail($patient->id);

            $debt = Debt::create([
                'patient_id' => $patient->id,
                'sponsor_id' => $sponsorId,
                'kind' => DebtKind::Opening,
                'amount' => Money::normalize($amount),
                'paid_amount' => '0.000',
                'status' => DebtStatus::Open,
                'note' => $note,
                'created_by' => $actor->id,
            ]);

            $this->auditLogger->log($actor, 'opening_debt_created', $debt, $debt->id, null, $debt->toArray());

            return $debt;
        });
    }

    /**
     * @param  array<int, array{debt_id: int, amount: string}>  $allocations
     * @return array{payment: Payment, allocations: array<int, PaymentAllocation>}
     */
    public function settle(string $amount, PaymentMethod $method, array $allocations, User $actor): array
    {
        return DB::transaction(function () use ($amount, $method, $allocations, $actor): array {
            $paymentAmount = Money::normalize($amount);
            $allocationTotal = Money::sum(array_column($allocations, 'amount'));

            if (Money::compare($allocationTotal, $paymentAmount) !== 0) {
                throw ApiException::validation('يجب تخصيص كامل مبلغ الدفعة على الديون.', [
                    'allocations' => ['مجموع التخصيصات يجب أن يساوي مبلغ الدفعة.'],
                ]);
            }

            $debtIds = array_column($allocations, 'debt_id');
            $debts = Debt::query()->whereIn('id', $debtIds)->orderBy('id')->lockForUpdate()->get()->keyBy('id');

            if ($debts->count() !== count($debtIds)) {
                throw ApiException::validation('تعذر العثور على أحد الديون المحددة.', [
                    'allocations' => ['أحد الديون غير موجود.'],
                ]);
            }

            $patientIds = $debts->pluck('patient_id')->unique();

            if ($patientIds->count() !== 1) {
                throw ApiException::validation('يجب أن تخص الدفعة مريضًا واحدًا.', [
                    'allocations' => ['لا يمكن جمع ديون مرضى مختلفين في دفعة واحدة.'],
                ]);
            }

            foreach ($allocations as $index => $allocation) {
                $debt = $debts->get($allocation['debt_id']);
                $remaining = Money::subtract($debt->amount, $debt->paid_amount);

                if ($debt->status === DebtStatus::Void || $debt->status === DebtStatus::Paid) {
                    throw ApiException::validation('أحد الديون غير متاح للتسوية.', [
                        "allocations.{$index}.debt_id" => ['الدين مسدد أو ملغى.'],
                    ]);
                }

                if (Money::compare($allocation['amount'], $remaining) > 0) {
                    throw ApiException::validation('مبلغ التخصيص يتجاوز المتبقي على الدين.', [
                        "allocations.{$index}.amount" => ['المتبقي على الدين هو '.$remaining.'.'],
                    ]);
                }
            }

            $payment = Payment::create([
                'patient_id' => $patientIds->first(),
                'type' => PaymentType::DebtPayment,
                'method' => $method,
                'amount' => $paymentAmount,
                'created_by' => $actor->id,
            ]);

            $createdAllocations = [];

            foreach ($allocations as $allocation) {
                $debt = $debts->get($allocation['debt_id']);
                $normalizedAmount = Money::normalize($allocation['amount']);
                $paidAmount = Money::add($debt->paid_amount, $normalizedAmount);

                $createdAllocations[] = PaymentAllocation::create([
                    'payment_id' => $payment->id,
                    'debt_id' => $debt->id,
                    'amount' => $normalizedAmount,
                    'created_by' => $actor->id,
                ]);

                $debt->update([
                    'paid_amount' => $paidAmount,
                    'status' => Money::compare($paidAmount, $debt->amount) === 0
                        ? DebtStatus::Paid
                        : DebtStatus::Partial,
                ]);
            }

            $this->auditLogger->log($actor, 'debt_payment_recorded', $payment, $payment->id, null, [
                'amount' => $paymentAmount,
                'allocations' => $allocations,
            ]);

            return ['payment' => $payment, 'allocations' => $createdAllocations];
        });
    }
}
