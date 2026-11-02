<?php

namespace App\Http\Requests\Sessions;

use App\Enums\CoverageType;
use App\Enums\PaymentMethod;
use App\Enums\SessionServiceItemType;
use App\Enums\SessionType;
use Illuminate\Contracts\Validation\ValidationRule;
use Illuminate\Foundation\Http\FormRequest;
use Illuminate\Validation\Rule;

class StoreSessionRequest extends FormRequest
{
    /**
     * Determine if the user is authorized to make this request.
     */
    public function authorize(): bool
    {
        return $this->user() !== null;
    }

    /**
     * Get the validation rules that apply to the request.
     *
     * @return array<string, ValidationRule|array<mixed>|string>
     */
    public function rules(): array
    {
        $isSubscription = $this->string('type')->toString() === SessionType::Subscription->value;

        return [
            'patient_id' => ['required', 'integer', Rule::exists('patients', 'id')->where('is_active', true)],
            'doctor_id' => ['required', 'integer', Rule::exists('doctors', 'id')->where('is_active', true)],
            'type' => ['required', Rule::enum(SessionType::class)],
            'subscription_id' => ['nullable', 'required_if:type,subscription', 'integer', 'exists:subscriptions,id'],
            'service_ids' => ['nullable', 'array', 'min:1'],
            'service_ids.*' => ['integer', 'distinct', Rule::exists('services', 'id')->where('is_active', true)],
            'services' => ['nullable', 'array', 'min:1'],
            'services.*.service_id' => ['required', 'integer', Rule::exists('services', 'id')->where('is_active', true)],
            'services.*.item_type' => ['nullable', Rule::enum(SessionServiceItemType::class)],
            'services.*.extra_units' => ['nullable', 'integer', 'min:0', 'max:10000'],
            'discount' => ['nullable', 'numeric', 'decimal:0,3', 'min:0'],
            'settlement' => [
                'nullable',
                'required_if:type,individual',
                'array:coverage,wallet_amount,paid_amount,debt_amount,method,sponsor_id',
            ],
            'settlement.coverage' => ['nullable', Rule::enum(CoverageType::class)],
            'settlement.wallet_amount' => ['nullable', 'numeric', 'decimal:0,3', 'min:0'],
            'settlement.paid_amount' => ['nullable', 'numeric', 'decimal:0,3', 'min:0'],
            'settlement.debt_amount' => ['nullable', 'numeric', 'decimal:0,3', 'min:0'],
            'settlement.method' => ['nullable', Rule::enum(PaymentMethod::class)],
            'settlement.sponsor_id' => ['nullable', 'integer', Rule::exists('sponsors', 'id')->where('is_active', true)],
        ];
    }
}
