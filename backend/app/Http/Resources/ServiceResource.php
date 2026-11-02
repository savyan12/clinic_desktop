<?php

namespace App\Http\Resources;

use App\Support\Money;
use Illuminate\Http\Request;
use Illuminate\Http\Resources\Json\JsonResource;

class ServiceResource extends JsonResource
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
            'price' => Money::normalize($this->resource->price),
            'included_units' => $this->resource->included_units,
            'unit_name' => $this->resource->unit_name,
            'extra_unit_price' => $this->resource->extra_unit_price === null
                ? null
                : Money::normalize($this->resource->extra_unit_price),
            'is_active' => $this->resource->is_active,
            'updated_at' => $this->resource->updated_at?->toIso8601String(),
        ];
    }
}
