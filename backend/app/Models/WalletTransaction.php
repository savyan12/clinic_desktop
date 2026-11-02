<?php

namespace App\Models;

use App\Enums\WalletTransactionType;
use Illuminate\Database\Eloquent\Attributes\Fillable;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;

#[Fillable(['patient_id', 'session_id', 'session_addition_id', 'payment_id', 'type', 'amount', 'balance_after', 'reverses_transaction_id', 'note', 'created_by'])]
class WalletTransaction extends Model
{
    protected function casts(): array
    {
        return ['type' => WalletTransactionType::class, 'amount' => 'decimal:3', 'balance_after' => 'decimal:3'];
    }

    public function patient(): BelongsTo
    {
        return $this->belongsTo(Patient::class);
    }

    public function session(): BelongsTo
    {
        return $this->belongsTo(Session::class);
    }

    public function sessionAddition(): BelongsTo
    {
        return $this->belongsTo(SessionAddition::class);
    }

    public function payment(): BelongsTo
    {
        return $this->belongsTo(Payment::class);
    }

    public function reversedTransaction(): BelongsTo
    {
        return $this->belongsTo(self::class, 'reverses_transaction_id');
    }
}
