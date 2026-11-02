<?php

namespace App\Http\Requests\Subscriptions;

use Illuminate\Contracts\Validation\ValidationRule;
use Illuminate\Foundation\Http\FormRequest;
use Illuminate\Validation\Rule;

class CorrectSubscriptionRequest extends FormRequest
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
            'changes' => ['required', 'array:expected_end_date,end_date,sponsor_id', 'min:1'],
            'changes.expected_end_date' => ['sometimes', 'nullable', 'date'],
            'changes.end_date' => ['sometimes', 'date'],
            'changes.sponsor_id' => ['sometimes', 'nullable', 'integer', Rule::exists('sponsors', 'id')->where('is_active', true)],
            'reason' => ['required', 'string', 'max:1000'],
        ];
    }
}
