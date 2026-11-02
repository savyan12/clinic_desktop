<?php

namespace App\Console\Commands;

use App\Enums\PaymentType;
use App\Enums\WalletTransactionType;
use App\Models\Patient;
use App\Models\Payment;
use App\Models\User;
use App\Models\WalletTransaction;
use App\Services\WalletService;
use App\Support\Money;
use Illuminate\Console\Command;
use Illuminate\Database\Eloquent\Collection;
use Illuminate\Support\Facades\DB;
use RuntimeException;

class RepairWalletTopupRefunds extends Command
{
    protected $signature = 'clinic:repair-wallet-topup-refunds {--apply : Insert missing wallet debit movements}';

    protected $description = 'Find wallet top-up refunds that are missing their matching prepaid-balance debit';

    public function __construct(private WalletService $walletService)
    {
        parent::__construct();
    }

    public function handle(): int
    {
        $refunds = $this->missingRefunds();

        if ($refunds->isEmpty()) {
            $this->info('No wallet top-up refunds are missing wallet debit movements.');

            return self::SUCCESS;
        }

        $this->info("Found {$refunds->count()} wallet top-up refund(s) missing wallet debit movements.");
        $projectedBalances = [];
        $issues = [];

        foreach ($refunds as $refund) {
            $issue = $this->validateFinding($refund);

            if ($issue !== null) {
                $issues[] = "Refund #{$refund->id}: {$issue}";

                continue;
            }

            $amount = $this->refundAmount($refund);
            $balance = $projectedBalances[$refund->patient_id]
                ?? $this->walletService->balance($refund->patient_id);

            if (Money::compare($balance, $amount) < 0) {
                $issues[] = "Refund #{$refund->id}: current wallet balance {$balance} is less than {$amount}.";

                continue;
            }

            $projectedBalances[$refund->patient_id] = Money::subtract($balance, $amount);
            $this->line(
                "Refund #{$refund->id} | top-up #{$refund->reverses_payment_id} | patient #{$refund->patient_id} | debit -{$amount} | projected balance {$projectedBalances[$refund->patient_id]}",
            );
        }

        if ($issues !== []) {
            foreach ($issues as $issue) {
                $this->error($issue);
            }

            $this->error('No changes were written because at least one finding was not safe to repair automatically.');

            return self::FAILURE;
        }

        if (! $this->option('apply')) {
            $this->warn('Dry run only. Re-run with --apply to insert the missing debit movements and audit entries.');

            return self::SUCCESS;
        }

        $applied = DB::transaction(function () use ($refunds): array {
            $movements = [];

            foreach ($refunds as $refund) {
                $lockedRefund = Payment::query()
                    ->with('reversedPayment')
                    ->lockForUpdate()
                    ->findOrFail($refund->id);

                if ($this->hasMatchingDebit($lockedRefund)) {
                    continue;
                }

                if ($this->validateFinding($lockedRefund) !== null) {
                    throw new RuntimeException("Refund #{$lockedRefund->id} is no longer safe to repair.");
                }

                $lockedPatient = Patient::query()->lockForUpdate()->findOrFail($lockedRefund->patient_id);
                $actor = User::query()->findOrFail($lockedRefund->created_by);
                $reason = "Repair: inserted missing wallet debit for wallet top-up refund #{$lockedRefund->id}.";
                $movements[] = $this->walletService->debitRefund(
                    $lockedRefund,
                    $lockedPatient,
                    $this->refundAmount($lockedRefund),
                    $reason,
                    $actor,
                );
            }

            return $movements;
        });

        $this->info('Applied '.count($applied).' wallet debit repair(s).');

        return self::SUCCESS;
    }

    /** @return Collection<int, Payment> */
    private function missingRefunds(): Collection
    {
        return Payment::query()
            ->with('reversedPayment')
            ->where('type', PaymentType::Refund)
            ->whereHas('reversedPayment', fn ($query) => $query->where('type', PaymentType::WalletTopup))
            ->orderBy('id')
            ->get()
            ->reject(fn (Payment $refund): bool => $this->hasMatchingDebit($refund))
            ->values();
    }

    private function hasMatchingDebit(Payment $refund): bool
    {
        return WalletTransaction::query()
            ->where('payment_id', $refund->id)
            ->where('patient_id', $refund->patient_id)
            ->where('type', WalletTransactionType::Debit)
            ->get()
            ->contains(fn (WalletTransaction $movement): bool => Money::compare($movement->amount, $refund->amount) === 0);
    }

    private function validateFinding(Payment $refund): ?string
    {
        $original = $refund->reversedPayment;

        if ($refund->type !== PaymentType::Refund || Money::compare($refund->amount, '0') >= 0) {
            return 'the refund payment is not a negative refund ledger row.';
        }

        if (! $original || $original->type !== PaymentType::WalletTopup) {
            return 'the reversed payment is not a wallet top-up.';
        }

        if ($refund->patient_id !== $original->patient_id) {
            return 'the refund and original top-up belong to different patients.';
        }

        if (WalletTransaction::query()->where('payment_id', $refund->id)->exists()) {
            return 'a non-matching wallet movement is already linked to the refund.';
        }

        if (! User::query()->whereKey($refund->created_by)->exists()) {
            return 'the refund creator no longer exists.';
        }

        $refundedTotal = Money::normalize(Payment::query()
            ->where('reverses_payment_id', $original->id)
            ->where('type', PaymentType::Refund)
            ->sum(DB::raw('ABS(amount)')));

        if (Money::compare($refundedTotal, $original->amount) > 0) {
            return 'refunds exceed the original wallet top-up amount.';
        }

        return null;
    }

    private function refundAmount(Payment $refund): string
    {
        return ltrim(Money::normalize($refund->amount), '-');
    }
}
