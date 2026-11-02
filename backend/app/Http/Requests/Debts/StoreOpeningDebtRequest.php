<?php

namespace App\Http\Requests\Debts;

use Illuminate\Contracts\Validation\ValidationRule;
use Illuminate\Foundation\Http\FormRequest;
use Illuminate\Validation\Rule;

class StoreOpeningDebtRequest extends FormRequest
{
    /**
     * Determine if the user is authorized to make this request.
     */
    public function authorize(): bool
    {
        return $this->user()?->isAdmin() ?? false;
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
            'sponsor_id' => ['nullable', 'integer', Rule::exists('sponsors', 'id')->where('is_active', true)],
            'note' => ['nullable', 'string', 'max:1000'],
        ];
    }
}
