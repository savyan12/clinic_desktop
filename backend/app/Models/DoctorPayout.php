<?php

namespace App\Models;

use App\Enums\PaymentMethod;
use Illuminate\Database\Eloquent\Attributes\Fillable;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;

#[Fillable(['doctor_id', 'amount', 'date', 'method', 'note', 'created_by'])]
class DoctorPayout extends Model
{
    protected function casts(): array
    {
        return ['amount' => 'decimal:3', 'date' => 'date:Y-m-d', 'method' => PaymentMethod::class];
    }

    public function doctor(): BelongsTo
    {
        return $this->belongsTo(Doctor::class);
    }
}
