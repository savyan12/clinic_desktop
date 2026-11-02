<?php

namespace App\Http\Resources;

use App\Support\Money;
use Illuminate\Http\Request;
use Illuminate\Http\Resources\Json\JsonResource;

class PaymentResource extends JsonResource
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
            'patient' => $this->whenLoaded('patient', fn () => [
                'id' => $this->resource->patient->id,
                'name' => $this->resource->patient->name,
                'phone' => $this->resource->patient->phone,
            ]),
            'session_id' => $this->resource->session_id,
            'session_addition_id' => $this->resource->session_addition_id,
            'subscription_id' => $this->resource->subscription_id,
            'type' => $this->resource->type->value,
            'method' => $this->resource->method->value,
            'amount' => Money::normalize($this->resource->amount),
            'reverses_payment_id' => $this->resource->reverses_payment_id,
            'note' => $this->resource->note,
            'created_by' => $this->resource->created_by,
            'created_at' => $this->resource->created_at?->toIso8601String(),
        ];
    }
}
