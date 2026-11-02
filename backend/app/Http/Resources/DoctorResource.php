<?php

namespace App\Http\Resources;

use App\Support\Money;
use Illuminate\Http\Request;
use Illuminate\Http\Resources\Json\JsonResource;

class DoctorResource extends JsonResource
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
            'commission_rate' => Money::normalize($this->resource->commission_rate),
            'is_active' => $this->resource->is_active,
            'sessions_count' => (int) ($this->resource->getAttribute('sessions_count') ?? 0),
            'earned' => Money::normalize($this->resource->getAttribute('earned')),
            'paid' => Money::normalize($this->resource->getAttribute('paid')),
            'remaining' => Money::normalize($this->resource->getAttribute('remaining')),
            'created_at' => $this->resource->created_at?->toIso8601String(),
        ];
    }
}
