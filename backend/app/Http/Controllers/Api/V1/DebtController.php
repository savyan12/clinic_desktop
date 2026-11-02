<?php

namespace App\Http\Controllers\Api\V1;

use App\Enums\DebtStatus;
use App\Enums\PaymentMethod;
use App\Enums\UserRole;
use App\Exceptions\ApiException;
use App\Http\Controllers\Controller;
use App\Http\Requests\Debts\SettleDebtRequest;
use App\Http\Requests\Debts\StoreOpeningDebtRequest;
use App\Http\Resources\DebtResource;
use App\Http\Resources\PaymentAllocationResource;
use App\Http\Resources\PaymentResource;
use App\Models\Debt;
use App\Models\Patient;
use App\Services\DebtService;
use App\Support\Money;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;

class DebtController extends Controller
{
    public function __construct(private DebtService $debtService) {}

    public function index(Request $request): JsonResponse
    {
        if ($request->user()->role === UserRole::Receptionist && ! $request->integer('patient_id')) {
            throw ApiException::forbidden('حدد مريضًا لعرض ديونه.');
        }

        $query = Debt::query();

        foreach (['patient_id', 'sponsor_id'] as $field) {
            if ($value = $request->integer($field)) {
                $query->where($field, $value);
            }
        }

        foreach (['status', 'kind'] as $field) {
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
        $response = $this->paginated($paginator, DebtResource::class, $request);
        $payload = $response->getData(true);
        $payload['totals'] = [
            'original' => Money::sum($paginator->getCollection()->pluck('amount')),
            'paid' => Money::sum($paginator->getCollection()->pluck('paid_amount')),
            'remaining' => Money::sum($paginator->getCollection()->map(
                fn ($debt) => $debt->status === DebtStatus::Void
                    ? '0.000'
                    : Money::subtract($debt->amount, $debt->paid_amount),
            )),
        ];

        return response()->json($payload);
    }

    public function patientDebts(Patient $patient, Request $request): JsonResponse
    {
        $request->merge(['patient_id' => $patient->id]);

        return $this->index($request);
    }

    public function openingDebt(StoreOpeningDebtRequest $request, Patient $patient): DebtResource
    {
        return DebtResource::make($this->debtService->createOpeningDebt(
            $patient,
            $request->string('amount')->toString(),
            $request->integer('sponsor_id') ?: null,
            $request->string('note')->toString() ?: null,
            $request->user(),
        ));
    }

    public function settle(SettleDebtRequest $request): JsonResponse
    {
        $result = $this->debtService->settle(
            $request->string('amount')->toString(),
            PaymentMethod::from($request->string('method')->toString()),
            $request->validated('allocations'),
            $request->user(),
        );

        return response()->json(['data' => [
            'payment' => PaymentResource::make($result['payment'])->resolve($request),
            'allocations' => PaymentAllocationResource::collection(collect($result['allocations']))->resolve($request),
        ]], 201);
    }
}
