<?php

use App\Http\Controllers\Api\V1\AuditLogController;
use App\Http\Controllers\Api\V1\AuthController;
use App\Http\Controllers\Api\V1\DashboardController;
use App\Http\Controllers\Api\V1\DebtController;
use App\Http\Controllers\Api\V1\DoctorController;
use App\Http\Controllers\Api\V1\DoctorPayoutController;
use App\Http\Controllers\Api\V1\PatientController;
use App\Http\Controllers\Api\V1\PaymentController;
use App\Http\Controllers\Api\V1\ReportController;
use App\Http\Controllers\Api\V1\ServiceController;
use App\Http\Controllers\Api\V1\SessionController;
use App\Http\Controllers\Api\V1\SettingController;
use App\Http\Controllers\Api\V1\SponsorController;
use App\Http\Controllers\Api\V1\SubscriptionController;
use App\Http\Controllers\Api\V1\UserController;
use App\Http\Controllers\Api\V1\WalletController;
use Illuminate\Support\Facades\Route;

Route::prefix('v1')->group(function (): void {
    Route::get('/health', fn () => response()->json([
        'status' => 'ok',
        'time' => now()->toIso8601String(),
    ]));
    Route::post('/auth/login', [AuthController::class, 'login'])->middleware('throttle:login');

    Route::middleware('auth:sanctum')->group(function (): void {
        Route::post('/auth/logout', [AuthController::class, 'logout']);
        Route::get('/auth/me', [AuthController::class, 'me']);

        Route::get('/dashboard', DashboardController::class);

        Route::get('/patients', [PatientController::class, 'index']);
        Route::post('/patients', [PatientController::class, 'store']);
        Route::get('/patients/{patient}', [PatientController::class, 'show']);
        Route::patch('/patients/{patient}', [PatientController::class, 'update']);
        Route::get('/patients/{patient}/statement', [PatientController::class, 'statement']);
        Route::get('/patients/{patient}/wallet/movements', [WalletController::class, 'movements']);
        Route::post('/patients/{patient}/wallet/topups', [WalletController::class, 'topUp'])->middleware('idempotency');
        Route::get('/patients/{patient}/debts', [DebtController::class, 'patientDebts']);
        Route::get('/patients/{patient}/payments', [PaymentController::class, 'patientPayments']);

        Route::get('/sponsors', [SponsorController::class, 'index']);
        Route::get('/doctors', [DoctorController::class, 'index']);
        Route::get('/services', [ServiceController::class, 'index']);

        Route::get('/sessions', [SessionController::class, 'index']);
        Route::post('/sessions', [SessionController::class, 'store'])->middleware('idempotency');
        Route::get('/sessions/{session}', [SessionController::class, 'show']);
        Route::patch('/sessions/{session}', [SessionController::class, 'update']);
        Route::post('/sessions/{session}/close', [SessionController::class, 'close'])->middleware('idempotency');
        Route::post('/sessions/{session}/services', [SessionController::class, 'addServices'])->middleware('idempotency');
        Route::post('/sessions/{session}/cancel', [SessionController::class, 'cancel'])->middleware('idempotency');

        Route::get('/subscriptions', [SubscriptionController::class, 'index']);
        Route::post('/subscriptions', [SubscriptionController::class, 'store'])->middleware('idempotency');
        Route::get('/subscriptions/{subscription}', [SubscriptionController::class, 'show']);
        Route::get('/subscriptions/{subscription}/usage', [SubscriptionController::class, 'usage']);

        Route::get('/debts', [DebtController::class, 'index']);
        Route::post('/debts/settlements', [DebtController::class, 'settle'])->middleware('idempotency');
        Route::get('/payments', [PaymentController::class, 'index']);
        Route::get('/settings', [SettingController::class, 'show']);

        Route::middleware('role:admin')->group(function (): void {
            Route::get('/users', [UserController::class, 'index']);
            Route::post('/users', [UserController::class, 'store']);
            Route::patch('/users/{user}', [UserController::class, 'update']);
            Route::post('/users/{user}/reset-password', [UserController::class, 'resetPassword']);

            Route::post('/patients/{patient}/opening-debts', [DebtController::class, 'openingDebt'])->middleware('idempotency');

            Route::post('/sponsors', [SponsorController::class, 'store']);
            Route::get('/sponsors/{sponsor}', [SponsorController::class, 'show']);
            Route::patch('/sponsors/{sponsor}', [SponsorController::class, 'update']);
            Route::get('/sponsors/{sponsor}/statement', [SponsorController::class, 'statement']);

            Route::post('/doctors', [DoctorController::class, 'store']);
            Route::get('/doctors/{doctor}', [DoctorController::class, 'show']);
            Route::patch('/doctors/{doctor}', [DoctorController::class, 'update']);
            Route::get('/doctors/{doctor}/dues', [DoctorController::class, 'dues']);
            Route::post('/doctors/{doctor}/payouts', [DoctorPayoutController::class, 'store'])->middleware('idempotency');

            Route::post('/services', [ServiceController::class, 'store']);
            Route::patch('/services/{service}', [ServiceController::class, 'update']);

            Route::post('/sessions/{session}/correct', [SessionController::class, 'correct'])->middleware('idempotency');
            Route::post('/subscriptions/{subscription}/stop', [SubscriptionController::class, 'stop'])->middleware('idempotency');
            Route::post('/subscriptions/{subscription}/reactivate', [SubscriptionController::class, 'reactivate'])->middleware('idempotency');
            Route::get('/subscriptions/{subscription}/cancel-preview', [SubscriptionController::class, 'cancelPreview']);
            Route::post('/subscriptions/{subscription}/cancel', [SubscriptionController::class, 'cancel'])->middleware('idempotency');
            Route::post('/subscriptions/{subscription}/correct', [SubscriptionController::class, 'correct'])->middleware('idempotency');

            Route::post('/refunds', [PaymentController::class, 'refund'])->middleware('idempotency');
            Route::get('/reports/{type}', [ReportController::class, 'show']);
            Route::get('/reports/{type}/export', [ReportController::class, 'export']);
            Route::get('/audit-log', [AuditLogController::class, 'index']);
            Route::put('/settings', [SettingController::class, 'update']);
        });
    });
});
