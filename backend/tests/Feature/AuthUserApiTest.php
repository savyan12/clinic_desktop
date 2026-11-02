<?php

namespace Tests\Feature;

use App\Enums\UserRole;
use App\Models\AuditLog;
use App\Models\User;
use Illuminate\Foundation\Testing\LazilyRefreshDatabase;
use Illuminate\Support\Facades\Hash;
use Laravel\Sanctum\Sanctum;
use Tests\TestCase;

class AuthUserApiTest extends TestCase
{
    use LazilyRefreshDatabase;

    public function test_valid_credentials_return_token_and_update_last_login(): void
    {
        $user = User::factory()->create(['username' => 'reception', 'password' => 'Reception123!']);

        $response = $this->postJson('/api/v1/auth/login', [
            'username' => 'reception',
            'password' => 'Reception123!',
        ]);

        $response->assertOk()
            ->assertJsonPath('data.user.username', 'reception')
            ->assertJsonStructure(['data' => ['token', 'user']]);
        $this->assertNotNull($user->fresh()->last_login_at);
    }

    public function test_inactive_account_returns_403(): void
    {
        User::factory()->create([
            'username' => 'inactive',
            'password' => 'Reception123!',
            'is_active' => false,
        ]);

        $this->postJson('/api/v1/auth/login', [
            'username' => 'inactive',
            'password' => 'Reception123!',
        ])->assertForbidden()->assertJsonPath('message', 'هذا الحساب معطل. راجع مدير النظام.');
    }

    public function test_admin_creates_receptionist_and_user_change_is_audited(): void
    {
        $admin = User::factory()->admin()->create();
        Sanctum::actingAs($admin);

        $response = $this->postJson('/api/v1/users', [
            'name' => 'موظف استقبال',
            'username' => 'new_reception',
            'password' => 'Reception123!',
            'password_confirmation' => 'Reception123!',
            'is_active' => true,
        ]);

        $response->assertSuccessful()->assertJsonPath('data.role', UserRole::Receptionist->value);
        $this->assertDatabaseHas('users', ['username' => 'new_reception', 'role' => UserRole::Receptionist->value]);
        $this->assertDatabaseHas('audit_logs', ['action' => 'user_created', 'user_id' => $admin->id]);
    }

    public function test_receptionist_cannot_access_user_management(): void
    {
        Sanctum::actingAs(User::factory()->create());

        $this->getJson('/api/v1/users')->assertForbidden();
    }

    public function test_deactivation_revokes_existing_tokens(): void
    {
        $admin = User::factory()->admin()->create();
        $receptionist = User::factory()->create();
        $receptionist->createToken('first');
        $receptionist->createToken('second');
        Sanctum::actingAs($admin);

        $this->patchJson("/api/v1/users/{$receptionist->id}", [
            'name' => $receptionist->name,
            'username' => $receptionist->username,
            'is_active' => false,
        ])->assertOk();

        $this->assertDatabaseCount('personal_access_tokens', 0);
        $this->assertFalse($receptionist->fresh()->is_active);
        $this->assertSame(1, AuditLog::query()->where('action', 'user_updated')->count());
    }

    public function test_password_reset_hashes_password_and_revokes_tokens(): void
    {
        $admin = User::factory()->admin()->create();
        $receptionist = User::factory()->create();
        $receptionist->createToken('existing');
        Sanctum::actingAs($admin);

        $this->postJson("/api/v1/users/{$receptionist->id}/reset-password", [
            'password' => 'Updated123!',
            'password_confirmation' => 'Updated123!',
        ])->assertOk();

        $this->assertTrue(Hash::check('Updated123!', $receptionist->fresh()->password));
        $this->assertDatabaseCount('personal_access_tokens', 0);
    }
}
