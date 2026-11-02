<?php

namespace Database\Factories;

use App\Models\Doctor;
use App\Models\Patient;
use App\Models\Session;
use App\Models\User;
use Illuminate\Database\Eloquent\Factories\Factory;

/**
 * @extends Factory<Session>
 */
class SessionFactory extends Factory
{
    /**
     * Define the model's default state.
     *
     * @return array<string, mixed>
     */
    public function definition(): array
    {
        return [
            'code' => 'S-'.fake()->unique()->numerify('#####'),
            'patient_id' => Patient::factory(),
            'doctor_id' => Doctor::factory(),
            'subscription_id' => null,
            'type' => 'individual',
            'status' => 'open',
            'opened_at' => now(),
            'created_by' => User::factory(),
            'services_total' => '80.000',
            'discount' => '0.000',
            'net_total' => '80.000',
            'coverage' => 'cash',
            'wallet_amount' => '0.000',
            'paid_amount' => '80.000',
            'debt_amount' => '0.000',
            'payment_method' => 'cash',
        ];
    }
}
