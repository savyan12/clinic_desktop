<?php

namespace App\Services;

use App\Enums\PaymentMethod;
use App\Enums\PaymentType;
use App\Exceptions\ApiException;
use App\Models\Patient;
use App\Models\Payment;
use App\Models\User;
use App\Support\Money;
use Illuminate\Support\Facades\DB;

class PaymentService
{
    public function __construct(
        private AuditLogger $auditLogger,
        private WalletService $walletService,
    ) {}

    public function refund(
        Payment $payment,
        string $amount,
        PaymentMethod $method,
        string $reason,
        User $actor,
    ): Payment {
        return DB::transaction(function () use ($payment, $amount, $method, $reason, $actor): Payment {
            $original = Payment::query()->lockForUpdate()->findOrFail($payment->id);

            if ($original->type === PaymentType::Refund || ! Money::isPositive($original->amount)) {
                throw ApiException::validation('لا يمكن استرجاع هذه الدفعة.', [
                    'payment_id' => ['الدفعة المحددة ليست دفعة قابلة للاسترجاع.'],
                ]);
            }

            $alreadyRefunded = Money::normalize(Payment::query()
                ->where('reverses_payment_id', $original->id)
                ->where('type', PaymentType::Refund)
                ->sum(DB::raw('ABS(amount)')));
            $available = Money::subtract($original->amount, $alreadyRefunded);
            $normalizedAmount = Money::normalize($amount);

            if (Money::compare($normalizedAmount, $available) > 0) {
                throw ApiException::validation('مبلغ الاسترجاع يتجاوز المبلغ المتاح.', [
                    'amount' => ['الحد المتاح للاسترجاع هو '.$available.'.'],
                ]);
            }

            $lockedPatient = $original->type === PaymentType::WalletTopup
                ? Patient::query()->lockForUpdate()->findOrFail($original->patient_id)
                : null;

            if ($lockedPatient && Money::compare($this->walletService->balance($lockedPatient), $normalizedAmount) < 0) {
                throw ApiException::validation('رصيد المريض الحالي لا يكفي لاسترجاع هذا المبلغ.', [
                    'amount' => ['رصيد المريض الحالي لا يكفي لاسترجاع هذا المبلغ.'],
                ]);
            }

            $refund = Payment::create([
                'patient_id' => $original->patient_id,
                'session_id' => $original->session_id,
                'subscription_id' => $original->subscription_id,
                'type' => PaymentType::Refund,
                'method' => $method,
                'amount' => '-'.$normalizedAmount,
                'reverses_payment_id' => $original->id,
                'note' => $reason,
                'created_by' => $actor->id,
            ]);

            $this->auditLogger->log($actor, 'payment_refunded', $refund, $refund->id, null, [
                'payment_id' => $original->id,
                'amount' => '-'.$normalizedAmount,
            ], $reason);

            if ($lockedPatient) {
                $this->walletService->debitRefund($refund, $lockedPatient, $normalizedAmount, $reason, $actor);
            }

            return $refund;
        });
    }
}
