<?php

namespace Database\Factories;

use App\Models\Session;
use App\Models\SessionAddition;
use App\Models\User;
use Illuminate\Database\Eloquent\Factories\Factory;

/**
 * @extends Factory<SessionAddition>
 */
class SessionAdditionFactory extends Factory
{
    /**
     * Define the model's default state.
     *
     * @return array<string, mixed>
     */
    public function definition(): array
    {
        return [
            'session_id' => Session::factory(),
            'services_total' => '20.000',
            'net_total' => '20.000',
            'coverage' => 'cash',
            'wallet_amount' => '0.000',
            'paid_amount' => '20.000',
            'debt_amount' => '0.000',
            'payment_method' => 'cash',
            'sponsor_id' => null,
            'created_by' => User::factory(),
        ];
    }
}
