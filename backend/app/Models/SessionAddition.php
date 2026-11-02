<?php

namespace App\Models;

use App\Enums\CoverageType;
use App\Enums\PaymentMethod;
use Database\Factories\SessionAdditionFactory;
use Illuminate\Database\Eloquent\Attributes\Fillable;
use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;
use Illuminate\Database\Eloquent\Relations\HasMany;

#[Fillable(['session_id', 'services_total', 'net_total', 'coverage', 'wallet_amount', 'paid_amount', 'debt_amount', 'payment_method', 'sponsor_id', 'created_by'])]
class SessionAddition extends Model
{
    /** @use HasFactory<SessionAdditionFactory> */
    use HasFactory;

    protected function casts(): array
    {
        return [
            'services_total' => 'decimal:3',
            'net_total' => 'decimal:3',
            'coverage' => CoverageType::class,
            'wallet_amount' => 'decimal:3',
            'paid_amount' => 'decimal:3',
            'debt_amount' => 'decimal:3',
            'payment_method' => PaymentMethod::class,
        ];
    }

    public function session(): BelongsTo
    {
        return $this->belongsTo(Session::class);
    }

    public function sponsor(): BelongsTo
    {
        return $this->belongsTo(Sponsor::class);
    }

    public function creator(): BelongsTo
    {
        return $this->belongsTo(User::class, 'created_by');
    }

    public function services(): HasMany
    {
        return $this->hasMany(SessionService::class);
    }

    public function payments(): HasMany
    {
        return $this->hasMany(Payment::class);
    }

    public function debts(): HasMany
    {
        return $this->hasMany(Debt::class);
    }

    public function walletTransactions(): HasMany
    {
        return $this->hasMany(WalletTransaction::class);
    }
}
