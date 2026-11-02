<?php

namespace App\Http\Requests\Subscriptions;

use App\Enums\PaymentMethod;
use App\Enums\SubscriptionPlanType;
use Illuminate\Contracts\Validation\ValidationRule;
use Illuminate\Foundation\Http\FormRequest;
use Illuminate\Validation\Rule;

class StoreSubscriptionRequest extends FormRequest
{
    protected function prepareForValidation(): void
    {
        if (! $this->has('plan_type')) {
            $this->merge([
                'plan_type' => $this->has('service_ids') || $this->has('sessions_count')
                    ? SubscriptionPlanType::LegacyPackage->value
                    : SubscriptionPlanType::Monthly->value,
            ]);
        }
    }

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
        $isMonthly = $this->string('plan_type')->toString() === SubscriptionPlanType::Monthly->value;

        return [
            'patient_id' => ['required', 'integer', Rule::exists('patients', 'id')->where('is_active', true)],
            'plan_type' => ['required', Rule::enum(SubscriptionPlanType::class)],
            'amount' => [Rule::requiredIf($isMonthly), Rule::excludeIf(! $isMonthly), 'numeric', 'decimal:0,3', 'min:0'],
            'service_ids' => [Rule::requiredIf(! $isMonthly), Rule::excludeIf($isMonthly), 'array', 'min:1'],
            'service_ids.*' => [Rule::excludeIf($isMonthly), 'integer', 'distinct', Rule::exists('services', 'id')->where('is_active', true)],
            'sessions_count' => [Rule::requiredIf(! $isMonthly), Rule::excludeIf($isMonthly), 'integer', 'min:1'],
            'discount' => [Rule::excludeIf($isMonthly), 'nullable', 'numeric', 'decimal:0,3', 'min:0'],
            'paid_amount' => ['nullable', 'numeric', 'decimal:0,3', 'min:0'],
            'method' => ['nullable', Rule::enum(PaymentMethod::class)],
            'start_date' => ['required', 'date'],
            'end_date' => [Rule::requiredIf($isMonthly), Rule::excludeIf(! $isMonthly), 'date', 'after_or_equal:start_date'],
            'expected_end_date' => [Rule::excludeIf($isMonthly), 'nullable', 'date'],
            'sponsor_id' => ['nullable', 'integer', Rule::exists('sponsors', 'id')->where('is_active', true)],
        ];
    }
}
