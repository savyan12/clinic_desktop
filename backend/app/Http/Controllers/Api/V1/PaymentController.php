<?php

namespace App\Http\Controllers\Api\V1;

use App\Enums\PaymentMethod;
use App\Enums\UserRole;
use App\Exceptions\ApiException;
use App\Http\Controllers\Controller;
use App\Http\Requests\Payments\StoreRefundRequest;
use App\Http\Resources\PaymentResource;
use App\Models\Patient;
use App\Models\Payment;
use App\Services\PaymentService;
use App\Support\Money;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;

class PaymentController extends Controller
{
    public function __construct(private PaymentService $paymentService) {}

    public function index(Request $request): JsonResponse
    {
        if ($request->user()->role === UserRole::Receptionist && ! $request->integer('patient_id')) {
            throw ApiException::forbidden('حدد مريضًا لعرض مدفوعاته.');
        }

        $query = Payment::query();

        if ($patientId = $request->integer('patient_id')) {
            $query->where('patient_id', $patientId);
        }

        if ($sponsorId = $request->integer('sponsor_id')) {
            $query->where(fn ($builder) => $builder
                ->whereHas('allocations.debt', fn ($debtQuery) => $debtQuery->where('sponsor_id', $sponsorId))
                ->orWhereHas('session', fn ($sessionQuery) => $sessionQuery->where('sponsor_id', $sponsorId))
                ->orWhereHas('subscription', fn ($subscriptionQuery) => $subscriptionQuery->where('sponsor_id', $sponsorId))
                ->orWhereHas('reversedPayment.allocations.debt', fn ($debtQuery) => $debtQuery->where('sponsor_id', $sponsorId))
                ->orWhereHas('reversedPayment.session', fn ($sessionQuery) => $sessionQuery->where('sponsor_id', $sponsorId))
                ->orWhereHas('reversedPayment.subscription', fn ($subscriptionQuery) => $subscriptionQuery->where('sponsor_id', $sponsorId)));
        }

        foreach (['type', 'method'] as $field) {
            if ($value = $request->string($field)->toString()) {
                $query->where($field, $value);
            }
        }

        if ($from = $request->date('from')) {
            $query->whereDate('created_at', '>=', $from);
        }

        if ($to = $request->date('to')) {
            $query->whereDate('created_at', '<=', $to);
        }

        $paginator = $query->latest('id')->paginate($this->perPage($request));
        $response = $this->paginated($paginator, PaymentResource::class, $request);
        $payload = $response->getData(true);
        $payload['totals'] = ['amount' => Money::sum($paginator->getCollection()->pluck('amount'))];

        return response()->json($payload);
    }

    public function patientPayments(Patient $patient, Request $request): JsonResponse
    {
        $request->merge(['patient_id' => $patient->id]);

        return $this->index($request);
    }

    public function refund(StoreRefundRequest $request): JsonResponse
    {
        $payment = Payment::query()
            ->where('patient_id', $request->integer('patient_id'))
            ->findOrFail($request->integer('payment_id'));
        $refund = $this->paymentService->refund(
            $payment,
            $request->string('amount')->toString(),
            PaymentMethod::from($request->string('method')->toString()),
            $request->string('reason')->toString(),
            $request->user(),
        );

        return response()->json(['data' => PaymentResource::make($refund)->resolve($request)], 201);
    }
}
