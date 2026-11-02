<?php

namespace Database\Factories;

use App\Models\Service;
use App\Models\User;
use Illuminate\Database\Eloquent\Factories\Factory;

/**
 * @extends Factory<Service>
 */
class ServiceFactory extends Factory
{
    /**
     * Define the model's default state.
     *
     * @return array<string, mixed>
     */
    public function definition(): array
    {
        return [
            'name' => fake()->unique()->words(2, true),
            'price' => fake()->randomElement(['40.000', '60.000', '80.000', '100.000']),
            'included_units' => null,
            'unit_name' => null,
            'extra_unit_price' => null,
            'is_active' => true,
            'created_by' => User::factory(),
        ];
    }
}
