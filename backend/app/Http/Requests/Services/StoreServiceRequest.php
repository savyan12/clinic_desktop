<?php

namespace App\Http\Requests\Services;

use Illuminate\Contracts\Validation\ValidationRule;
use Illuminate\Foundation\Http\FormRequest;
use Illuminate\Validation\Validator;

class StoreServiceRequest extends FormRequest
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
            'name' => ['required', 'string', 'max:255'],
            'price' => ['required', 'numeric', 'decimal:0,3', 'min:0'],
            'included_units' => ['nullable', 'integer', 'min:1', 'max:10000'],
            'unit_name' => ['nullable', 'string', 'max:50'],
            'extra_unit_price' => ['nullable', 'numeric', 'decimal:0,3', 'min:0'],
            'is_active' => ['sometimes', 'boolean'],
        ];
    }

    /** @return array<int, callable(Validator): void> */
    public function after(): array
    {
        return [function (Validator $validator): void {
            $addonFields = [
                $this->input('included_units'),
                $this->input('unit_name'),
                $this->input('extra_unit_price'),
            ];
            $configured = collect($addonFields)->filter(fn (mixed $value): bool => $value !== null && $value !== '')->count();

            if ($configured > 0 && $configured < count($addonFields)) {
                $validator->errors()->add('included_units', 'أدخل عدد الوحدات المشمولة واسم الوحدة وسعر الوحدة الإضافية معًا.');
            }
        }];
    }
}
