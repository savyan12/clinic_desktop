<?php

namespace App\Http\Controllers\Api\V1;

use App\Enums\PaymentMethod;
use App\Http\Controllers\Controller;
use App\Http\Requests\Doctors\StoreDoctorPayoutRequest;
use App\Http\Resources\DoctorPayoutResource;
use App\Http\Resources\DoctorResource;
use App\Models\Doctor;
use App\Services\DoctorPayoutService;
use Illuminate\Http\JsonResponse;

class DoctorPayoutController extends Controller
{
    public function __construct(private DoctorPayoutService $payoutService) {}

    public function store(StoreDoctorPayoutRequest $request, Doctor $doctor): JsonResponse
    {
        $payout = $this->payoutService->create(
            $doctor,
            $request->string('amount')->toString(),
            $request->string('date')->toString(),
            PaymentMethod::from($request->string('method')->toString()),
            $request->string('note')->toString() ?: null,
            $request->user(),
        );
        $doctor->setAttribute('remaining', $this->payoutService->remaining($doctor));

        return response()->json(['data' => [
            'payout' => DoctorPayoutResource::make($payout)->resolve($request),
            'doctor' => DoctorResource::make($doctor)->resolve($request),
        ]], 201);
    }
}
