<?php

namespace App\Models;

use App\Enums\CoverageType;
use App\Enums\PaymentMethod;
use App\Enums\SessionStatus;
use App\Enums\SessionType;
use Database\Factories\SessionFactory;
use Illuminate\Database\Eloquent\Attributes\Fillable;
use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;
use Illuminate\Database\Eloquent\Relations\HasMany;

#[Fillable(['code', 'patient_id', 'doctor_id', 'subscription_id', 'type', 'status', 'opened_at', 'closed_at', 'cancelled_at', 'created_by', 'closed_by', 'cancelled_by', 'cancel_reason', 'services_total', 'discount', 'net_total', 'coverage', 'wallet_amount', 'paid_amount', 'debt_amount', 'payment_method', 'sponsor_id', 'commission_rate', 'commission_base_amount', 'commission_amount'])]
class Session extends Model
{
    /** @use HasFactory<SessionFactory> */
    use HasFactory;

    protected function casts(): array
    {
        return [
            'type' => SessionType::class,
            'status' => SessionStatus::class,
            'coverage' => CoverageType::class,
            'payment_method' => PaymentMethod::class,
            'opened_at' => 'datetime',
            'closed_at' => 'datetime',
            'cancelled_at' => 'datetime',
            'services_total' => 'decimal:3',
            'discount' => 'decimal:3',
            'net_total' => 'decimal:3',
            'wallet_amount' => 'decimal:3',
            'paid_amount' => 'decimal:3',
            'debt_amount' => 'decimal:3',
            'commission_rate' => 'decimal:3',
            'commission_base_amount' => 'decimal:3',
            'commission_amount' => 'decimal:3',
        ];
    }

    public function patient(): BelongsTo
    {
        return $this->belongsTo(Patient::class);
    }

    public function doctor(): BelongsTo
    {
        return $this->belongsTo(Doctor::class);
    }

    public function subscription(): BelongsTo
    {
        return $this->belongsTo(Subscription::class);
    }

    public function sponsor(): BelongsTo
    {
        return $this->belongsTo(Sponsor::class);
    }

    public function creator(): BelongsTo
    {
        return $this->belongsTo(User::class, 'created_by');
    }

    public function closedBy(): BelongsTo
    {
        return $this->belongsTo(User::class, 'closed_by');
    }

    public function cancelledBy(): BelongsTo
    {
        return $this->belongsTo(User::class, 'cancelled_by');
    }

    public function services(): HasMany
    {
        return $this->hasMany(SessionService::class);
    }

    public function additions(): HasMany
    {
        return $this->hasMany(SessionAddition::class);
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

    public function subscriptionUsages(): HasMany
    {
        return $this->hasMany(SubscriptionUsage::class);
    }
}
