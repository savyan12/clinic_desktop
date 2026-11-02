<?php

namespace App\Services;

use App\Enums\PaymentMethod;
use App\Enums\PaymentType;
use App\Enums\WalletTransactionType;
use App\Exceptions\ApiException;
use App\Models\Patient;
use App\Models\Payment;
use App\Models\Session;
use App\Models\SessionAddition;
use App\Models\User;
use App\Models\WalletTransaction;
use App\Support\Money;
use Illuminate\Support\Facades\DB;

class WalletService
{
    public function __construct(private AuditLogger $auditLogger) {}

    public function balance(Patient|int $patient): string
    {
        $patientId = $patient instanceof Patient ? $patient->id : $patient;

        return Money::normalize(WalletTransaction::query()->where('patient_id', $patientId)->sum('amount'));
    }

    /** @return array{balance: string, movement: WalletTransaction, payment: Payment} */
    public function topUp(Patient $patient, string $amount, PaymentMethod $method, ?string $note, User $actor): array
    {
        return DB::transaction(function () use ($patient, $amount, $method, $note, $actor): array {
            $lockedPatient = Patient::query()->lockForUpdate()->findOrFail($patient->id);
            $normalizedAmount = Money::normalize($amount);
            $balance = Money::add($this->balance($lockedPatient), $normalizedAmount);

            $payment = Payment::create([
                'patient_id' => $lockedPatient->id,
                'type' => PaymentType::WalletTopup,
                'method' => $method,
                'amount' => $normalizedAmount,
                'note' => $note,
                'created_by' => $actor->id,
            ]);
            $this->auditLogger->log($actor, 'payment_recorded', $payment, $payment->id, null, [
                'patient_id' => $lockedPatient->id,
                'type' => PaymentType::WalletTopup->value,
                'method' => $method->value,
                'amount' => $normalizedAmount,
            ]);

            $movement = WalletTransaction::create([
                'patient_id' => $lockedPatient->id,
                'payment_id' => $payment->id,
                'type' => WalletTransactionType::Topup,
                'amount' => $normalizedAmount,
                'balance_after' => $balance,
                'note' => $note,
                'created_by' => $actor->id,
            ]);

            $this->auditLogger->log($actor, 'wallet_topped_up', $movement, $movement->id, null, [
                'patient_id' => $lockedPatient->id,
                'amount' => $normalizedAmount,
                'balance_after' => $balance,
                'payment_id' => $payment->id,
            ]);

            return compact('balance', 'movement', 'payment');
        });
    }

    public function debit(
        Session $session,
        Patient $lockedPatient,
        string $amount,
        User $actor,
        ?SessionAddition $addition = null,
    ): ?WalletTransaction {
        if (Money::isZero($amount)) {
            return null;
        }

        $balance = $this->balance($lockedPatient);

        if (Money::compare($balance, $amount) < 0) {
            throw ApiException::validation(
                'الرصيد غير كافٍ. اشحن رصيد المريض أولًا ثم أعد المحاولة.',
                ['settlement.wallet_amount' => ['الرصيد المتاح أقل من المبلغ المطلوب.']],
            );
        }

        return WalletTransaction::create([
            'patient_id' => $lockedPatient->id,
            'session_id' => $session->id,
            'session_addition_id' => $addition?->id,
            'type' => WalletTransactionType::Debit,
            'amount' => '-'.Money::normalize($amount),
            'balance_after' => Money::subtract($balance, $amount),
            'created_by' => $actor->id,
        ]);
    }

    public function debitRefund(
        Payment $refund,
        Patient $lockedPatient,
        string $amount,
        string $reason,
        User $actor,
    ): WalletTransaction {
        $balance = $this->balance($lockedPatient);
        $normalizedAmount = Money::normalize($amount);

        if (Money::compare($balance, $normalizedAmount) < 0) {
            throw ApiException::validation('رصيد المريض الحالي لا يكفي لاسترجاع هذا المبلغ.', [
                'amount' => ['رصيد المريض الحالي لا يكفي لاسترجاع هذا المبلغ.'],
            ]);
        }

        $movement = WalletTransaction::create([
            'patient_id' => $lockedPatient->id,
            'payment_id' => $refund->id,
            'type' => WalletTransactionType::Debit,
            'amount' => '-'.$normalizedAmount,
            'balance_after' => Money::subtract($balance, $normalizedAmount),
            'note' => $reason,
            'created_by' => $actor->id,
        ]);

        $this->auditLogger->log($actor, 'wallet_topup_refunded', $movement, $movement->id, null, [
            'patient_id' => $lockedPatient->id,
            'refund_payment_id' => $refund->id,
            'amount' => '-'.$normalizedAmount,
            'balance_after' => Money::subtract($balance, $normalizedAmount),
        ], $reason);

        return $movement;
    }

    public function reverse(WalletTransaction $debit, User $actor, string $note): WalletTransaction
    {
        $existing = WalletTransaction::query()
            ->where('reverses_transaction_id', $debit->id)
            ->first();

        if ($existing) {
            return $existing;
        }

        $patient = Patient::query()->lockForUpdate()->findOrFail($debit->patient_id);
        $amount = ltrim(Money::normalize($debit->amount), '-');

        return WalletTransaction::create([
            'patient_id' => $patient->id,
            'session_id' => $debit->session_id,
            'session_addition_id' => $debit->session_addition_id,
            'type' => WalletTransactionType::Reversal,
            'amount' => $amount,
            'balance_after' => Money::add($this->balance($patient), $amount),
            'reverses_transaction_id' => $debit->id,
            'note' => $note,
            'created_by' => $actor->id,
        ]);
    }
}
