<?php

namespace App\Models;

use App\Enums\DebtKind;
use App\Enums\DebtStatus;
use Database\Factories\DebtFactory;
use Illuminate\Database\Eloquent\Attributes\Fillable;
use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;
use Illuminate\Database\Eloquent\Relations\HasMany;

#[Fillable(['patient_id', 'sponsor_id', 'session_id', 'session_addition_id', 'subscription_id', 'kind', 'amount', 'paid_amount', 'status', 'voided_at', 'voided_by', 'void_reason', 'note', 'created_by'])]
class Debt extends Model
{
    /** @use HasFactory<DebtFactory> */
    use HasFactory;

    protected function casts(): array
    {
        return [
            'kind' => DebtKind::class,
            'status' => DebtStatus::class,
            'amount' => 'decimal:3',
            'paid_amount' => 'decimal:3',
            'voided_at' => 'datetime',
        ];
    }

    public function patient(): BelongsTo
    {
        return $this->belongsTo(Patient::class);
    }

    public function sponsor(): BelongsTo
    {
        return $this->belongsTo(Sponsor::class);
    }

    public function session(): BelongsTo
    {
        return $this->belongsTo(Session::class);
    }

    public function sessionAddition(): BelongsTo
    {
        return $this->belongsTo(SessionAddition::class);
    }

    public function subscription(): BelongsTo
    {
        return $this->belongsTo(Subscription::class);
    }

    public function allocations(): HasMany
    {
        return $this->hasMany(PaymentAllocation::class);
    }
}
