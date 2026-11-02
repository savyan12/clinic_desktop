<?php

namespace App\Models;

use App\Enums\SubscriptionPlanType;
use App\Enums\SubscriptionStatus;
use Database\Factories\SubscriptionFactory;
use Illuminate\Database\Eloquent\Attributes\Fillable;
use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;
use Illuminate\Database\Eloquent\Relations\HasMany;

#[Fillable(['patient_id', 'sponsor_id', 'plan_type', 'sessions_count', 'sessions_used', 'base_total', 'discount', 'net_total', 'paid_amount', 'start_date', 'expected_end_date', 'end_date', 'status', 'cancelled_at', 'cancelled_by', 'cancel_reason', 'refund_amount', 'created_by'])]
class Subscription extends Model
{
    /** @use HasFactory<SubscriptionFactory> */
    use HasFactory;

    protected function casts(): array
    {
        return [
            'sessions_count' => 'integer',
            'sessions_used' => 'integer',
            'plan_type' => SubscriptionPlanType::class,
            'base_total' => 'decimal:3',
            'discount' => 'decimal:3',
            'net_total' => 'decimal:3',
            'paid_amount' => 'decimal:3',
            'start_date' => 'date:Y-m-d',
            'expected_end_date' => 'date:Y-m-d',
            'end_date' => 'date:Y-m-d',
            'status' => SubscriptionStatus::class,
            'cancelled_at' => 'datetime',
            'refund_amount' => 'decimal:3',
        ];
    }

    public function isMonthly(): bool
    {
        return $this->plan_type === SubscriptionPlanType::Monthly;
    }

    public function patient(): BelongsTo
    {
        return $this->belongsTo(Patient::class);
    }

    public function sponsor(): BelongsTo
    {
        return $this->belongsTo(Sponsor::class);
    }

    public function creator(): BelongsTo
    {
        return $this->belongsTo(User::class, 'created_by');
    }

    public function cancelledBy(): BelongsTo
    {
        return $this->belongsTo(User::class, 'cancelled_by');
    }

    public function services(): HasMany
    {
        return $this->hasMany(SubscriptionService::class);
    }

    public function sessions(): HasMany
    {
        return $this->hasMany(Session::class);
    }

    public function usages(): HasMany
    {
        return $this->hasMany(SubscriptionUsage::class);
    }

    public function payments(): HasMany
    {
        return $this->hasMany(Payment::class);
    }

    public function debts(): HasMany
    {
        return $this->hasMany(Debt::class);
    }
}
