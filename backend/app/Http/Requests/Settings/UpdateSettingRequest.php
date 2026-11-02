<?php

namespace App\Http\Requests\Settings;

use Illuminate\Contracts\Validation\ValidationRule;
use Illuminate\Foundation\Http\FormRequest;

class UpdateSettingRequest extends FormRequest
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
            'clinic_name' => ['required', 'string', 'max:255'],
            'clinic_subtitle' => ['nullable', 'string', 'max:255'],
            'phone' => ['nullable', 'string', 'max:30'],
            'address' => ['nullable', 'string', 'max:255'],
            'receipt_footer' => ['nullable', 'string', 'max:1000'],
            'currency' => ['required', 'string', 'max:10'],
            'print_size' => ['required', 'in:A4,A5'],
            'show_logo' => ['required', 'boolean'],
            'body_font_size' => ['sometimes', 'integer', 'between:14,20'],
            'body_font_weight' => ['sometimes', 'integer', 'in:400,600'],
            'table_font_size' => ['sometimes', 'integer', 'between:13,18'],
            'table_font_weight' => ['sometimes', 'integer', 'in:400,600'],
            'heading_font_size' => ['sometimes', 'integer', 'between:24,40'],
            'heading_font_weight' => ['sometimes', 'integer', 'in:400,600'],
            'section_font_size' => ['sometimes', 'integer', 'between:18,28'],
            'section_font_weight' => ['sometimes', 'integer', 'in:400,600'],
        ];
    }
}
