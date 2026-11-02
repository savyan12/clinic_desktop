<?php

namespace App\Http\Resources;

use App\Support\Money;
use Illuminate\Http\Request;
use Illuminate\Http\Resources\Json\JsonResource;

class DoctorPayoutResource extends JsonResource
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
            'doctor_id' => $this->resource->doctor_id,
            'amount' => Money::normalize($this->resource->amount),
            'date' => $this->resource->date?->format('Y-m-d'),
            'method' => $this->resource->method->value,
            'note' => $this->resource->note,
            'created_by' => $this->resource->created_by,
            'created_at' => $this->resource->created_at?->toIso8601String(),
        ];
    }
}
