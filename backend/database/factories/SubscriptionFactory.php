<?php

namespace Database\Factories;

use App\Enums\SubscriptionPlanType;
use App\Models\Patient;
use App\Models\Subscription;
use App\Models\User;
use Illuminate\Database\Eloquent\Factories\Factory;

/**
 * @extends Factory<Subscription>
 */
class SubscriptionFactory extends Factory
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
            'sponsor_id' => null,
            'plan_type' => SubscriptionPlanType::LegacyPackage,
            'sessions_count' => 10,
            'sessions_used' => 0,
            'base_total' => '800.000',
            'discount' => '50.000',
            'net_total' => '750.000',
            'paid_amount' => '750.000',
            'start_date' => today(),
            'expected_end_date' => today()->addMonth(),
            'end_date' => null,
            'status' => 'active',
            'created_by' => User::factory(),
        ];
    }

    public function monthly(): static
    {
        return $this->state(fn (): array => [
            'plan_type' => SubscriptionPlanType::Monthly,
            'sessions_count' => 0,
            'sessions_used' => 0,
            'base_total' => '600.000',
            'discount' => '0.000',
            'net_total' => '600.000',
            'paid_amount' => '600.000',
            'start_date' => today(),
            'expected_end_date' => null,
            'end_date' => today()->addMonth(),
        ]);
    }
}
