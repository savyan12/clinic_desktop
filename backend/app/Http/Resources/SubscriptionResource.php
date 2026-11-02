<?php

namespace App\Http\Resources;

use App\Enums\SubscriptionPlanType;
use App\Support\Money;
use Carbon\CarbonImmutable;
use Illuminate\Http\Request;
use Illuminate\Http\Resources\Json\JsonResource;

class SubscriptionResource extends JsonResource
{
    /**
     * Transform the resource into an array.
     *
     * @return array<string, mixed>
     */
    public function toArray(Request $request): array
    {
        $isMonthly = $this->resource->plan_type === SubscriptionPlanType::Monthly;
        $today = CarbonImmutable::today(config('app.timezone'));
        $endDate = $this->resource->end_date
            ? CarbonImmutable::parse($this->resource->end_date)->startOfDay()
            : null;
        $remainingDays = $isMonthly && $this->resource->end_date
            ? max(0, $today->diffInDays($endDate, false) + 1)
            : null;
        $isUsable = $isMonthly
            ? $this->resource->status->value === 'active'
                && $this->resource->start_date?->lte($today)
                && $this->resource->end_date?->gte($today)
            : $this->resource->status->value === 'active'
                && $this->resource->sessions_used < $this->resource->sessions_count;

        return [
            'id' => $this->resource->id,
            'patient_id' => $this->resource->patient_id,
            'patient' => $this->whenLoaded('patient', fn () => [
                'id' => $this->resource->patient->id,
                'name' => $this->resource->patient->name,
                'phone' => $this->resource->patient->phone,
            ]),
            'sponsor' => $this->whenLoaded('sponsor', fn () => $this->resource->sponsor ? [
                'id' => $this->resource->sponsor->id,
                'name' => $this->resource->sponsor->name,
            ] : null),
            'services' => $this->whenLoaded('services', fn () => $this->resource->services->map(fn ($service) => [
                'service_id' => $service->service_id,
                'name' => $service->service_name,
                'price' => Money::normalize($service->price),
            ])->values()),
            'plan_type' => $this->resource->plan_type->value,
            'sessions_count' => $this->resource->sessions_count,
            'sessions_used' => $this->resource->sessions_used,
            'remaining' => $isMonthly ? null : max(0, $this->resource->sessions_count - $this->resource->sessions_used),
            'remaining_days' => $remainingDays,
            'is_usable' => $isUsable,
            'base_total' => Money::normalize($this->resource->base_total),
            'discount' => Money::normalize($this->resource->discount),
            'net_total' => Money::normalize($this->resource->net_total),
            'paid_amount' => Money::normalize($this->resource->paid_amount),
            'remaining_amount' => Money::subtract($this->resource->net_total, $this->resource->paid_amount),
            'start_date' => $this->resource->start_date?->format('Y-m-d'),
            'expected_end_date' => $this->resource->expected_end_date?->format('Y-m-d'),
            'end_date' => $this->resource->end_date?->format('Y-m-d'),
            'status' => $this->resource->status->value,
            'cancelled_at' => $this->resource->cancelled_at?->toIso8601String(),
            'cancelled_by' => $this->resource->cancelled_by,
            'cancel_reason' => $this->resource->cancel_reason,
            'refund_amount' => Money::normalize($this->resource->refund_amount),
            'sponsor_id' => $this->resource->sponsor_id,
            'created_by' => $this->resource->created_by,
        ];
    }
}
