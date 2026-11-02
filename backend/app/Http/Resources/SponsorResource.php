<?php

namespace App\Http\Resources;

use App\Support\Money;
use Illuminate\Http\Request;
use Illuminate\Http\Resources\Json\JsonResource;

class SponsorResource extends JsonResource
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
            'address' => $this->resource->address,
            'is_active' => $this->resource->is_active,
            'patients_count' => (int) ($this->resource->getAttribute('patients_count') ?? 0),
            'total_due' => Money::normalize($this->resource->getAttribute('total_due')),
            'total_paid' => Money::normalize($this->resource->getAttribute('total_paid')),
            'remaining' => Money::normalize($this->resource->getAttribute('remaining')),
            'created_at' => $this->resource->created_at?->toIso8601String(),
        ];
    }
}
