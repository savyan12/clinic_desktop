<?php

namespace App\Http\Controllers\Api\V1;

use App\Enums\UserRole;
use App\Http\Controllers\Controller;
use App\Http\Requests\Users\ResetPasswordRequest;
use App\Http\Requests\Users\StoreUserRequest;
use App\Http\Requests\Users\UpdateUserRequest;
use App\Http\Resources\UserResource;
use App\Models\User;
use App\Services\AuditLogger;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\DB;

class UserController extends Controller
{
    public function __construct(private AuditLogger $auditLogger) {}

    public function index(Request $request): JsonResponse
    {
        $query = User::query()->where('role', UserRole::Receptionist->value);

        if ($search = $request->string('search')->trim()->toString()) {
            $query->where(fn ($builder) => $builder
                ->where('name', 'like', "%{$search}%")
                ->orWhere('username', 'like', "%{$search}%"));
        }

        if ($request->has('is_active')) {
            $query->where('is_active', $request->boolean('is_active'));
        }

        $paginator = $query->latest('id')->paginate(min(100, max(1, $request->integer('per_page', 15))));

        return response()->json([
            'data' => UserResource::collection($paginator->getCollection())->resolve($request),
            'meta' => [
                'current_page' => $paginator->currentPage(),
                'last_page' => $paginator->lastPage(),
                'per_page' => $paginator->perPage(),
                'total' => $paginator->total(),
            ],
        ]);
    }

    public function store(StoreUserRequest $request): UserResource
    {
        $user = DB::transaction(function () use ($request): User {
            $user = User::create([
                ...$request->safe()->only(['name', 'username', 'password', 'is_active']),
                'role' => UserRole::Receptionist,
            ]);
            $this->auditLogger->log($request->user(), 'user_created', $user, $user->id, null, [
                'name' => $user->name,
                'username' => $user->username,
                'role' => $user->role->value,
                'is_active' => $user->is_active,
            ]);

            return $user;
        });

        return UserResource::make($user);
    }

    public function update(UpdateUserRequest $request, User $user): UserResource|JsonResponse
    {
        if ($user->role !== UserRole::Receptionist) {
            return response()->json(['message' => 'تعذر العثور على السجل المطلوب.'], 404);
        }

        DB::transaction(function () use ($request, $user): void {
            $old = $user->only(['name', 'username', 'is_active']);
            $user->update($request->validated());

            if (! $user->is_active) {
                $user->tokens()->delete();
            }

            $this->auditLogger->log(
                $request->user(),
                'user_updated',
                $user,
                $user->id,
                $old,
                $user->fresh()->only(['name', 'username', 'is_active']),
            );
        });

        return UserResource::make($user->fresh());
    }

    public function resetPassword(ResetPasswordRequest $request, User $user): JsonResponse
    {
        if ($user->role !== UserRole::Receptionist) {
            return response()->json(['message' => 'تعذر العثور على السجل المطلوب.'], 404);
        }

        DB::transaction(function () use ($request, $user): void {
            $user->update(['password' => $request->string('password')->toString()]);
            $user->tokens()->delete();
            $this->auditLogger->log(
                $request->user(),
                'user_password_reset',
                $user,
                $user->id,
                null,
                ['tokens_revoked' => true],
            );
        });

        return response()->json(['data' => ['message' => 'تمت إعادة تعيين كلمة المرور بنجاح.']]);
    }
}
