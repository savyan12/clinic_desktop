<?php

namespace App\Models;

use App\Enums\SubscriptionUsageType;
use Illuminate\Database\Eloquent\Attributes\Fillable;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;

#[Fillable(['subscription_id', 'session_id', 'type', 'reverses_usage_id', 'created_by'])]
class SubscriptionUsage extends Model
{
    protected function casts(): array
    {
        return ['type' => SubscriptionUsageType::class];
    }

    public function subscription(): BelongsTo
    {
        return $this->belongsTo(Subscription::class);
    }

    public function session(): BelongsTo
    {
        return $this->belongsTo(Session::class);
    }

    public function reversedUsage(): BelongsTo
    {
        return $this->belongsTo(self::class, 'reverses_usage_id');
    }
}
