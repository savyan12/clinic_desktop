<?php

namespace App\Http\Controllers\Api\V1;

use App\Http\Controllers\Controller;
use App\Http\Requests\Sessions\AddSessionServicesRequest;
use App\Http\Requests\Sessions\CancelSessionRequest;
use App\Http\Requests\Sessions\CloseSessionRequest;
use App\Http\Requests\Sessions\CorrectSessionRequest;
use App\Http\Requests\Sessions\StoreSessionRequest;
use App\Http\Requests\Sessions\UpdateSessionRequest;
use App\Http\Resources\DebtResource;
use App\Http\Resources\PaymentResource;
use App\Http\Resources\SessionResource;
use App\Http\Resources\SubscriptionUsageResource;
use App\Http\Resources\WalletTransactionResource;
use App\Models\Debt;
use App\Models\Payment;
use App\Models\Session;
use App\Models\SubscriptionUsage;
use App\Models\WalletTransaction;
use App\Services\SessionService;
use App\Services\WalletService;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;

class SessionController extends Controller
{
    public function __construct(
        private SessionService $sessionService,
        private WalletService $walletService,
    ) {}

    public function index(Request $request): JsonResponse
    {
        $query = Session::query()->with(['patient', 'doctor', 'services']);

        if (! $request->user()->isAdmin()) {
            $query->whereDate('opened_at', today());
        }

        if ($search = $request->string('search')->trim()->toString()) {
            $query->whereHas('patient', fn ($builder) => $builder
                ->where('name', 'like', "%{$search}%")
                ->orWhere('phone', 'like', "%{$search}%"));
        }

        if ($doctorId = $request->integer('doctor_id')) {
            $query->where('doctor_id', $doctorId);
        }

        if ($status = $request->string('status')->toString()) {
            $query->where('status', $status);
        }

        if ($request->user()->isAdmin()) {
            if ($date = $request->date('date')) {
                $query->whereDate('opened_at', $date);
            }

            if ($from = $request->date('from')) {
                $query->whereDate('opened_at', '>=', $from);
            }

            if ($to = $request->date('to')) {
                $query->whereDate('opened_at', '<=', $to);
            }
        }

        return $this->paginated(
            $query->latest('opened_at')->latest('id')->paginate($this->perPage($request)),
            SessionResource::class,
            $request,
        );
    }

    public function store(StoreSessionRequest $request): JsonResponse
    {
        $result = $this->sessionService->createAndClose($request->validated(), $request->user());

        return response()->json(['data' => [
            'session' => SessionResource::make($result['session'])->resolve($request),
            'effects' => $this->serializeEffects($result['effects'], $request),
            'replayed' => $result['replayed'],
        ]], 201);
    }

    public function show(Session $session): SessionResource
    {
        $session->load(['patient', 'doctor', 'services', 'additions', 'creator', 'closedBy', 'cancelledBy']);
        $session->patient->setAttribute('wallet_balance', $this->walletService->balance($session->patient));

        return SessionResource::make($session);
    }

    public function update(UpdateSessionRequest $request, Session $session): SessionResource
    {
        return SessionResource::make($this->sessionService->update($session, $request->validated(), $request->user()));
    }

    public function close(CloseSessionRequest $request, Session $session): JsonResponse
    {
        $result = $this->sessionService->close($session, $request->user());

        return response()->json(['data' => [
            'session' => SessionResource::make($result['session'])->resolve($request),
            'effects' => $this->serializeEffects($result['effects'], $request),
            'replayed' => $result['replayed'],
        ]]);
    }

    public function addServices(AddSessionServicesRequest $request, Session $session): JsonResponse
    {
        $result = $this->sessionService->addServices($session, $request->validated(), $request->user());

        return response()->json(['data' => [
            'session' => SessionResource::make($result['session'])->resolve($request),
            'addition' => [
                'id' => $result['addition']->id,
                'services_total' => $result['addition']->services_total,
                'net_total' => $result['addition']->net_total,
                'created_at' => $result['addition']->created_at?->toIso8601String(),
            ],
            'effects' => $this->serializeEffects($result['effects'], $request),
        ]], 201);
    }

    public function cancel(CancelSessionRequest $request, Session $session): JsonResponse
    {
        $result = $this->sessionService->cancel(
            $session,
            $request->string('reason')->toString(),
            $request->user(),
        );

        return response()->json(['data' => [
            'session' => SessionResource::make($result['session'])->resolve($request),
            'reversals' => $this->serializeNested($result['reversals'], $request),
        ]]);
    }

    public function correct(CorrectSessionRequest $request, Session $session): JsonResponse
    {
        $result = $this->sessionService->correct(
            $session,
            $request->validated(),
            $request->string('reason')->toString(),
            $request->user(),
        );

        return response()->json(['data' => [
            'session' => SessionResource::make($result['session'])->resolve($request),
            'financial_impact' => $this->serializeNested($result['financial_impact'], $request),
        ]]);
    }

    /** @param array<string, mixed> $effects */
    private function serializeEffects(array $effects, Request $request): array
    {
        return [
            'subscription_usage' => $effects['subscription_usage']
                ? SubscriptionUsageResource::make($effects['subscription_usage'])->resolve($request)
                : null,
            'wallet_transaction' => $effects['wallet_transaction']
                ? WalletTransactionResource::make($effects['wallet_transaction'])->resolve($request)
                : null,
            'payment' => $effects['payment']
                ? PaymentResource::make($effects['payment'])->resolve($request)
                : null,
            'debt' => $effects['debt']
                ? DebtResource::make($effects['debt'])->resolve($request)
                : null,
        ];
    }

    private function serializeNested(mixed $value, Request $request): mixed
    {
        if ($value instanceof SubscriptionUsage) {
            return SubscriptionUsageResource::make($value)->resolve($request);
        }

        if ($value instanceof WalletTransaction) {
            return WalletTransactionResource::make($value)->resolve($request);
        }

        if ($value instanceof Payment) {
            return PaymentResource::make($value)->resolve($request);
        }

        if ($value instanceof Debt) {
            return DebtResource::make($value)->resolve($request);
        }

        if ($value instanceof Model) {
            return $value->toArray();
        }

        if (is_array($value)) {
            return collect($value)->map(fn ($item) => $this->serializeNested($item, $request))->all();
        }

        return $value;
    }
}
