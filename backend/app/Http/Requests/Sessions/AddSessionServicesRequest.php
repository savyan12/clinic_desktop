<?php

namespace App\Http\Requests\Sessions;

use App\Enums\PaymentMethod;
use App\Enums\SessionServiceItemType;
use App\Enums\SessionType;
use App\Models\Session;
use Illuminate\Contracts\Validation\ValidationRule;
use Illuminate\Foundation\Http\FormRequest;
use Illuminate\Validation\Rule;

class AddSessionServicesRequest extends FormRequest
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
        $session = $this->route('session');
        $isSubscription = $session instanceof Session && $session->type === SessionType::Subscription;

        return [
            'services' => ['required', 'array', 'min:1'],
            'services.*.service_id' => ['required', 'integer', Rule::exists('services', 'id')->where('is_active', true)],
            'services.*.item_type' => ['nullable', Rule::enum(SessionServiceItemType::class)],
            'services.*.extra_units' => ['nullable', 'integer', 'min:0', 'max:10000'],
            'settlement' => [Rule::requiredIf(! $isSubscription), 'nullable', 'array:wallet_amount,paid_amount,debt_amount,method,sponsor_id'],
            'settlement.wallet_amount' => ['nullable', 'numeric', 'decimal:0,3', 'min:0'],
            'settlement.paid_amount' => ['nullable', 'numeric', 'decimal:0,3', 'min:0'],
            'settlement.debt_amount' => ['nullable', 'numeric', 'decimal:0,3', 'min:0'],
            'settlement.method' => ['nullable', Rule::enum(PaymentMethod::class)],
            'settlement.sponsor_id' => ['nullable', 'integer', Rule::exists('sponsors', 'id')->where('is_active', true)],
        ];
    }
}
