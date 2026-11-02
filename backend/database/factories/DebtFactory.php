<?php

namespace Database\Factories;

use App\Models\Debt;
use App\Models\Patient;
use App\Models\User;
use Illuminate\Database\Eloquent\Factories\Factory;

/**
 * @extends Factory<Debt>
 */
class DebtFactory extends Factory
{
    /**
     * Define the model's default state.
     *
     * @return array<string, mixed>
     */
    public function definition(): array
    {
        return [
            'patient_id' => Patient::factory(),
            'kind' => 'opening',
            'amount' => '100.000',
            'paid_amount' => '0.000',
            'status' => 'open',
            'created_by' => User::factory(),
        ];
    }
}
