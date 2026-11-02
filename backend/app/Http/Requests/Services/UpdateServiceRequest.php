<?php

namespace App\Http\Requests\Services;

use Illuminate\Contracts\Validation\ValidationRule;

class UpdateServiceRequest extends StoreServiceRequest
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
        return [
            ...parent::rules(),
            'reason' => ['nullable', 'string', 'max:1000'],
        ];
    }
}
