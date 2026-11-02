<?php

namespace Database\Factories;

use App\Models\Patient;
use App\Models\User;
use Illuminate\Database\Eloquent\Factories\Factory;

/**
 * @extends Factory<Patient>
 */
class PatientFactory extends Factory
{
    /**
     * Define the model's default state.
     *
     * @return array<string, mixed>
     */
    public function definition(): array
    {
        return [
            'name' => fake()->name(),
            'phone' => fake()->unique()->numerify('09########'),
            'age' => fake()->numberBetween(12, 80),
            'gender' => fake()->randomElement(['male', 'female']),
            'area' => fake()->city(),
            'emergency_phone' => fake()->optional()->numerify('09########'),
            'sponsor_id' => null,
            'is_active' => true,
            'created_by' => User::factory(),
        ];
    }
}
