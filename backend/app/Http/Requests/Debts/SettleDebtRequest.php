<?php

namespace App\Http\Requests\Debts;

use App\Enums\PaymentMethod;
use Illuminate\Contracts\Validation\ValidationRule;
use Illuminate\Foundation\Http\FormRequest;
use Illuminate\Validation\Rule;

class SettleDebtRequest extends FormRequest
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
        return [
            'amount' => ['required', 'numeric', 'decimal:0,3', 'gt:0'],
            'method' => ['required', Rule::enum(PaymentMethod::class)],
            'allocations' => ['required', 'array', 'min:1'],
            'allocations.*.debt_id' => ['required', 'integer', 'distinct', 'exists:debts,id'],
            'allocations.*.amount' => ['required', 'numeric', 'decimal:0,3', 'gt:0'],
        ];
    }
}
