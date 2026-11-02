<?php

namespace App\Http\Resources;

use App\Enums\DebtStatus;
use App\Support\Money;
use Illuminate\Http\Request;
use Illuminate\Http\Resources\Json\JsonResource;

class DebtResource extends JsonResource
{
    /**
     * Transform the resource into an array.
     *
     * @return array<string, mixed>
     */
    public function toArray(Request $request): array
    {
        return [
            'id' => $this->resource->id,
            'patient_id' => $this->resource->patient_id,
            'sponsor_id' => $this->resource->sponsor_id,
            'session_id' => $this->resource->session_id,
            'session_addition_id' => $this->resource->session_addition_id,
            'subscription_id' => $this->resource->subscription_id,
            'kind' => $this->resource->kind->value,
            'amount' => Money::normalize($this->resource->amount),
            'paid_amount' => Money::normalize($this->resource->paid_amount),
            'remaining' => $this->resource->status === DebtStatus::Void
                ? '0.000'
                : Money::subtract($this->resource->amount, $this->resource->paid_amount),
            'status' => $this->resource->status->value,
            'voided_at' => $this->resource->voided_at?->toIso8601String(),
            'void_reason' => $this->resource->void_reason,
            'note' => $this->resource->note,
            'created_by' => $this->resource->created_by,
            'created_at' => $this->resource->created_at?->toIso8601String(),
        ];
    }
}
