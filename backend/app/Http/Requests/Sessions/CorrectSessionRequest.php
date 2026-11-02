<?php

namespace App\Http\Requests\Sessions;

use App\Enums\CoverageType;
use App\Enums\PaymentMethod;
use Illuminate\Contracts\Validation\ValidationRule;
use Illuminate\Foundation\Http\FormRequest;
use Illuminate\Validation\Rule;
use Illuminate\Validation\Validator;

class CorrectSessionRequest extends FormRequest
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
            'changes' => ['nullable', 'array:patient_id,doctor_id,service_ids,discount,settlement', 'min:1'],
            'changes.patient_id' => ['sometimes', 'integer', Rule::exists('patients', 'id')->where('is_active', true)],
            'changes.doctor_id' => ['sometimes', 'integer', Rule::exists('doctors', 'id')->where('is_active', true)],
            'changes.service_ids' => ['sometimes', 'array', 'min:1'],
            'changes.service_ids.*' => ['integer', 'distinct', Rule::exists('services', 'id')->where('is_active', true)],
            'changes.discount' => ['sometimes', 'numeric', 'decimal:0,3', 'min:0'],
            'changes.settlement' => ['sometimes', 'array:coverage,wallet_amount,paid_amount,debt_amount,method,sponsor_id'],
            'changes.settlement.coverage' => ['sometimes', Rule::enum(CoverageType::class)],
            'changes.settlement.wallet_amount' => ['sometimes', 'numeric', 'decimal:0,3', 'min:0'],
            'changes.settlement.paid_amount' => ['sometimes', 'numeric', 'decimal:0,3', 'min:0'],
            'changes.settlement.debt_amount' => ['sometimes', 'numeric', 'decimal:0,3', 'min:0'],
            'changes.settlement.method' => ['sometimes', 'nullable', Rule::enum(PaymentMethod::class)],
            'changes.settlement.sponsor_id' => ['sometimes', 'nullable', 'integer', Rule::exists('sponsors', 'id')->where('is_active', true)],
            'patient_id' => ['sometimes', 'integer', Rule::exists('patients', 'id')->where('is_active', true)],
            'doctor_id' => ['sometimes', 'integer', Rule::exists('doctors', 'id')->where('is_active', true)],
            'service_ids' => ['sometimes', 'array', 'min:1'],
            'service_ids.*' => ['integer', 'distinct', Rule::exists('services', 'id')->where('is_active', true)],
            'discount' => ['sometimes', 'numeric', 'decimal:0,3', 'min:0'],
            'settlement' => ['sometimes', 'array:coverage,wallet_amount,paid_amount,debt_amount,method,sponsor_id'],
            'settlement.coverage' => ['sometimes', Rule::enum(CoverageType::class)],
            'settlement.wallet_amount' => ['sometimes', 'numeric', 'decimal:0,3', 'min:0'],
            'settlement.paid_amount' => ['sometimes', 'numeric', 'decimal:0,3', 'min:0'],
            'settlement.debt_amount' => ['sometimes', 'numeric', 'decimal:0,3', 'min:0'],
            'settlement.method' => ['sometimes', 'nullable', Rule::enum(PaymentMethod::class)],
            'settlement.sponsor_id' => ['sometimes', 'nullable', 'integer', Rule::exists('sponsors', 'id')->where('is_active', true)],
            'reason' => ['required', 'string', 'max:1000'],
        ];
    }

    /** @return array<int, callable(Validator): void> */
    public function after(): array
    {
        return [
            function (Validator $validator): void {
                if ($validator->errors()->isNotEmpty()) {
                    return;
                }

                if ($this->filled('changes') || $this->hasAny([
                    'patient_id',
                    'doctor_id',
                    'service_ids',
                    'discount',
                    'settlement',
                ])) {
                    return;
                }

                $validator->errors()->add('changes', 'أرسل تغييرًا واحدًا على الأقل.');
            },
        ];
    }
}
