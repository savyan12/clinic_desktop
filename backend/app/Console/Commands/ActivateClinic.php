<?php

namespace App\Console\Commands;

use App\Enums\UserRole;
use App\Models\Setting;
use App\Models\User;
use Illuminate\Console\Attributes\Description;
use Illuminate\Console\Attributes\Signature;
use Illuminate\Console\Command;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Facades\Schema;

#[Signature('clinic:activate
    {--new : Initialize a brand-new local database}
    {--seed : Seed development data when an existing database is empty}
    {--clinic-name= : Clinic name for a new installation}
    {--clinic-subtitle= : Optional clinic subtitle for a new installation}
    {--admin-name= : Administrator display name for a new installation}
    {--admin-username= : Administrator username for a new installation}
    {--admin-password= : Administrator password for a new installation}')]
#[Description('Prepare the local clinic database for first activation or restored data')]
class ActivateClinic extends Command
{
    /**
     * Execute the console command.
     */
    public function handle(): int
    {
        if ($this->option('new')) {
            if (Schema::hasTable('users') && User::query()->exists()) {
                $this->components->error('The local database already contains users and cannot be initialized as new.');

                return self::FAILURE;
            }

            $required = collect(['clinic-name', 'admin-name', 'admin-username', 'admin-password'])
                ->filter(fn (string $option): bool => blank($this->option($option)));

            if ($required->isNotEmpty()) {
                $this->components->error('New activation requires clinic and administrator credentials.');

                return self::FAILURE;
            }

            if (mb_strlen((string) $this->option('admin-password')) < 8) {
                $this->components->error('The administrator password must contain at least 8 characters.');

                return self::FAILURE;
            }

            if ($this->call('migrate:fresh', ['--force' => true]) !== self::SUCCESS) {
                $this->components->error('The local database migrations failed.');

                return self::FAILURE;
            }
            DB::transaction(function (): void {
                $admin = User::query()->create([
                    'name' => trim((string) $this->option('admin-name')),
                    'username' => trim((string) $this->option('admin-username')),
                    'password' => (string) $this->option('admin-password'),
                    'role' => UserRole::Admin,
                    'is_active' => true,
                ]);

                Setting::query()->create([
                    'clinic_name' => trim((string) $this->option('clinic-name')),
                    'clinic_subtitle' => filled($this->option('clinic-subtitle'))
                        ? trim((string) $this->option('clinic-subtitle'))
                        : null,
                    'currency' => 'د.ل',
                    'print_size' => 'A4',
                    'show_logo' => true,
                    'body_font_size' => 16,
                    'body_font_weight' => 400,
                    'table_font_size' => 15,
                    'table_font_weight' => 400,
                    'heading_font_size' => 32,
                    'heading_font_weight' => 600,
                    'section_font_size' => 22,
                    'section_font_weight' => 600,
                    'updated_by' => $admin->id,
                ]);
            });
            $this->components->info('The new clinic database was initialized successfully.');

            return self::SUCCESS;
        }

        if ($this->call('migrate', ['--force' => true]) !== self::SUCCESS) {
            $this->components->error('The local database migrations failed.');

            return self::FAILURE;
        }

        if ($this->option('seed') && (! Schema::hasTable('users') || User::query()->doesntExist())) {
            if ($this->call('db:seed', ['--force' => true]) !== self::SUCCESS) {
                $this->components->error('The local development seed failed.');

                return self::FAILURE;
            }
        }

        $this->components->info('The clinic database is ready.');

        return self::SUCCESS;
    }
}
