<?php

namespace App\Http\Resources;

use App\Support\Money;
use Illuminate\Http\Request;
use Illuminate\Http\Resources\Json\JsonResource;

class WalletTransactionResource extends JsonResource
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
            'session_id' => $this->resource->session_id,
            'session_addition_id' => $this->resource->session_addition_id,
            'payment_id' => $this->resource->payment_id,
            'type' => $this->resource->type->value,
            'amount' => Money::normalize($this->resource->amount),
            'balance_after' => Money::normalize($this->resource->balance_after),
            'reverses_transaction_id' => $this->resource->reverses_transaction_id,
            'note' => $this->resource->note,
            'created_by' => $this->resource->created_by,
            'created_at' => $this->resource->created_at?->toIso8601String(),
        ];
    }
}
