<?php

namespace App\Http\Resources;

use App\Support\Money;
use Illuminate\Http\Request;
use Illuminate\Http\Resources\Json\JsonResource;

class SessionResource extends JsonResource
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
            'code' => $this->resource->code,
            'patient_id' => $this->resource->patient_id,
            'patient' => $this->whenLoaded('patient', function (): array {
                $patient = [
                    'id' => $this->resource->patient->id,
                    'name' => $this->resource->patient->name,
                    'phone' => $this->resource->patient->phone,
                ];

                if (array_key_exists('wallet_balance', $this->resource->patient->getAttributes())) {
                    $patient['wallet_balance'] = Money::normalize($this->resource->patient->getAttribute('wallet_balance'));
                }

                return $patient;
            }),
            'doctor_id' => $this->resource->doctor_id,
            'doctor' => $this->whenLoaded('doctor', fn () => [
                'id' => $this->resource->doctor->id,
                'name' => $this->resource->doctor->name,
            ]),
            'services' => $this->whenLoaded('services', fn () => $this->resource->services->map(fn ($service) => [
                'id' => $service->id,
                'service_id' => $service->service_id,
                'name' => $service->service_name,
                'price' => Money::normalize($service->price),
                'base_price' => Money::normalize($service->base_price ?? $service->price),
                'included_units' => $service->included_units,
                'extra_units' => $service->extra_units,
                'total_units' => $service->included_units + $service->extra_units,
                'unit_name' => $service->unit_name,
                'extra_unit_price' => $service->extra_unit_price === null
                    ? null
                    : Money::normalize($service->extra_unit_price),
                'item_type' => $service->item_type->value,
                'is_addition' => $service->is_addition,
                'added_at' => $service->added_at?->toIso8601String(),
                'session_addition_id' => $service->session_addition_id,
            ])->values()),
            'service_ids' => $this->whenLoaded('services', fn () => $this->resource->services->pluck('service_id')->values()),
            'type' => $this->resource->type->value,
            'subscription_id' => $this->resource->subscription_id,
            'status' => $this->resource->status->value,
            'opened_at' => $this->resource->opened_at?->toIso8601String(),
            'closed_at' => $this->resource->closed_at?->toIso8601String(),
            'cancelled_at' => $this->resource->cancelled_at?->toIso8601String(),
            'cancel_reason' => $this->resource->cancel_reason,
            'services_total' => Money::normalize($this->resource->services_total),
            'discount' => Money::normalize($this->resource->discount),
            'net_total' => Money::normalize($this->resource->net_total),
            'settlement' => [
                'coverage' => $this->resource->coverage->value,
                'wallet_amount' => Money::normalize($this->resource->wallet_amount),
                'paid_amount' => Money::normalize($this->resource->paid_amount),
                'debt_amount' => Money::normalize($this->resource->debt_amount),
                'method' => $this->resource->payment_method?->value,
                'sponsor_id' => $this->resource->sponsor_id,
            ],
            'commission_rate' => $this->resource->commission_rate === null ? null : Money::normalize($this->resource->commission_rate),
            'commission_base_amount' => $this->resource->commission_base_amount === null ? null : Money::normalize($this->resource->commission_base_amount),
            'commission_amount' => $this->resource->commission_amount === null ? null : Money::normalize($this->resource->commission_amount),
            'created_by' => $this->resource->created_by,
            'created_by_user' => $this->whenLoaded('creator', fn () => [
                'id' => $this->resource->creator->id,
                'name' => $this->resource->creator->name,
            ]),
            'closed_by' => $this->resource->closed_by,
            'closed_by_user' => $this->whenLoaded('closedBy', fn () => $this->resource->closedBy ? [
                'id' => $this->resource->closedBy->id,
                'name' => $this->resource->closedBy->name,
            ] : null),
            'cancelled_by' => $this->resource->cancelled_by,
            'cancelled_by_user' => $this->whenLoaded('cancelledBy', fn () => $this->resource->cancelledBy ? [
                'id' => $this->resource->cancelledBy->id,
                'name' => $this->resource->cancelledBy->name,
            ] : null),
            'additions' => $this->whenLoaded('additions', fn () => $this->resource->additions->map(fn ($addition) => [
                'id' => $addition->id,
                'services_total' => Money::normalize($addition->services_total),
                'net_total' => Money::normalize($addition->net_total),
                'coverage' => $addition->coverage->value,
                'wallet_amount' => Money::normalize($addition->wallet_amount),
                'paid_amount' => Money::normalize($addition->paid_amount),
                'debt_amount' => Money::normalize($addition->debt_amount),
                'method' => $addition->payment_method?->value,
                'sponsor_id' => $addition->sponsor_id,
                'created_by' => $addition->created_by,
                'created_at' => $addition->created_at?->toIso8601String(),
            ])->values()),
        ];
    }
}
