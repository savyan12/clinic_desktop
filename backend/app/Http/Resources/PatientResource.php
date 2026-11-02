<?php

namespace App\Http\Resources;

use App\Support\Money;
use Illuminate\Http\Request;
use Illuminate\Http\Resources\Json\JsonResource;

class PatientResource extends JsonResource
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
            'name' => $this->resource->name,
            'phone' => $this->resource->phone,
            'age' => $this->resource->age,
            'gender' => $this->resource->gender?->value,
            'area' => $this->resource->area,
            'emergency_phone' => $this->resource->emergency_phone,
            'sponsor_id' => $this->resource->sponsor_id,
            'sponsor' => $this->whenLoaded('sponsor', fn () => $this->resource->sponsor ? [
                'id' => $this->resource->sponsor->id,
                'name' => $this->resource->sponsor->name,
            ] : null),
            'wallet_balance' => Money::normalize($this->resource->getAttribute('wallet_balance')),
            'debt_total' => Money::normalize($this->resource->getAttribute('debt_total')),
            'subscription_status' => $this->resource->getAttribute('subscription_status'),
            'is_active' => $this->resource->is_active,
            'created_at' => $this->resource->created_at?->toIso8601String(),
        ];
    }
}
