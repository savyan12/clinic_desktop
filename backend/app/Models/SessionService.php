<?php

namespace App\Models;

use App\Enums\SessionServiceItemType;
use Illuminate\Database\Eloquent\Attributes\Fillable;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;

#[Fillable(['session_id', 'session_addition_id', 'service_id', 'service_name', 'price', 'base_price', 'included_units', 'extra_units', 'unit_name', 'extra_unit_price', 'item_type', 'is_addition', 'added_at', 'created_by'])]
class SessionService extends Model
{
    protected function casts(): array
    {
        return [
            'price' => 'decimal:3',
            'base_price' => 'decimal:3',
            'included_units' => 'integer',
            'extra_units' => 'integer',
            'extra_unit_price' => 'decimal:3',
            'item_type' => SessionServiceItemType::class,
            'is_addition' => 'boolean',
            'added_at' => 'datetime',
        ];
    }

    public function session(): BelongsTo
    {
        return $this->belongsTo(Session::class);
    }

    public function service(): BelongsTo
    {
        return $this->belongsTo(Service::class);
    }

    public function addition(): BelongsTo
    {
        return $this->belongsTo(SessionAddition::class, 'session_addition_id');
    }
}
