<?php

namespace App\Http\Controllers\Api\V1;

use App\Http\Controllers\Controller;
use App\Http\Requests\Auth\LoginRequest;
use App\Http\Resources\UserResource;
use App\Models\User;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Facades\Hash;

class AuthController extends Controller
{
    public function login(LoginRequest $request): JsonResponse
    {
        $user = User::query()->where('username', $request->string('username'))->first();

        if (! $user || ! Hash::check($request->string('password'), $user->password)) {
            return response()->json([
                'message' => 'اسم المستخدم أو كلمة المرور غير صحيحة.',
                'errors' => ['username' => ['تعذر تسجيل الدخول بهذه البيانات.']],
            ], 422);
        }

        if (! $user->is_active) {
            return response()->json(['message' => 'هذا الحساب معطل. راجع مدير النظام.'], 403);
        }

        $token = DB::transaction(function () use ($user): string {
            $user->update(['last_login_at' => now()]);

            return $user->createToken('clinic-api')->plainTextToken;
        });

        return response()->json(['data' => [
            'token' => $token,
            'user' => UserResource::make($user->fresh())->resolve($request),
        ]]);
    }

    public function logout(Request $request): JsonResponse
    {
        $request->user()->currentAccessToken()?->delete();

        return response()->json(['data' => ['message' => 'تم تسجيل الخروج بنجاح.']]);
    }

    public function me(Request $request): UserResource
    {
        return UserResource::make($request->user());
    }
}
