<?php

namespace App\Http\Requests\Sessions;

use App\Enums\CoverageType;
use App\Enums\PaymentMethod;
use App\Enums\SessionServiceItemType;
use App\Enums\SessionType;
use App\Models\Session;
use Illuminate\Contracts\Validation\ValidationRule;
use Illuminate\Validation\Rule;

class UpdateSessionRequest extends StoreSessionRequest
{
    /**
     * Determine if the user is authorized to make this request.
     */
    public function authorize(): bool
    {
        return parent::authorize();
    }

    /**
     * Get the validation rules that apply to the request.
     *
     * @return array<string, ValidationRule|array<mixed>|string>
     */
    public function rules(): array
    {
        $routeSession = $this->route('session');
        $targetType = $this->input('type');

        if ($targetType === null && $routeSession instanceof Session) {
            $targetType = $routeSession->type->value;
        }

        $isSubscription = $targetType === SessionType::Subscription->value;

        return [
            'patient_id' => ['sometimes', 'integer', Rule::exists('patients', 'id')->where('is_active', true)],
            'doctor_id' => ['sometimes', 'integer', Rule::exists('doctors', 'id')->where('is_active', true)],
            'type' => ['sometimes', Rule::enum(SessionType::class)],
            'subscription_id' => ['sometimes', 'nullable', 'integer', 'exists:subscriptions,id'],
            'service_ids' => ['sometimes', 'array', 'min:1'],
            'service_ids.*' => ['integer', 'distinct', Rule::exists('services', 'id')->where('is_active', true)],
            'services' => ['sometimes', 'array', 'min:1'],
            'services.*.service_id' => ['required', 'integer', Rule::exists('services', 'id')->where('is_active', true)],
            'services.*.item_type' => ['nullable', Rule::enum(SessionServiceItemType::class)],
            'services.*.extra_units' => ['nullable', 'integer', 'min:0', 'max:10000'],
            'discount' => ['sometimes', 'numeric', 'decimal:0,3', 'min:0'],
            'settlement' => ['sometimes', 'array:coverage,wallet_amount,paid_amount,debt_amount,method,sponsor_id'],
            'settlement.coverage' => ['sometimes', Rule::enum(CoverageType::class)],
            'settlement.wallet_amount' => ['sometimes', 'numeric', 'decimal:0,3', 'min:0'],
            'settlement.paid_amount' => ['sometimes', 'numeric', 'decimal:0,3', 'min:0'],
            'settlement.debt_amount' => ['sometimes', 'numeric', 'decimal:0,3', 'min:0'],
            'settlement.method' => ['sometimes', 'nullable', Rule::enum(PaymentMethod::class)],
            'settlement.sponsor_id' => ['sometimes', 'nullable', 'integer', Rule::exists('sponsors', 'id')->where('is_active', true)],
        ];
    }
}
