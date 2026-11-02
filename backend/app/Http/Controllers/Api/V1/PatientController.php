<?php

namespace App\Http\Controllers\Api\V1;

use App\Enums\DebtStatus;
use App\Enums\SubscriptionPlanType;
use App\Enums\SubscriptionStatus;
use App\Exceptions\ApiException;
use App\Http\Controllers\Controller;
use App\Http\Requests\Patients\StorePatientRequest;
use App\Http\Requests\Patients\UpdatePatientRequest;
use App\Http\Resources\DebtResource;
use App\Http\Resources\PatientResource;
use App\Http\Resources\PaymentAllocationResource;
use App\Http\Resources\PaymentResource;
use App\Http\Resources\SessionResource;
use App\Http\Resources\SubscriptionResource;
use App\Http\Resources\WalletTransactionResource;
use App\Models\Patient;
use App\Services\AuditLogger;
use App\Services\SubscriptionService;
use App\Support\Money;
use Illuminate\Database\Eloquent\Builder;
use Illuminate\Database\Eloquent\Relations\HasMany;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use Illuminate\Support\Arr;
use Illuminate\Support\Facades\DB;

class PatientController extends Controller
{
    public function __construct(
        private AuditLogger $auditLogger,
        private SubscriptionService $subscriptionService,
    ) {}

    public function index(Request $request): JsonResponse
    {
        $this->subscriptionService->expireMonthlySubscriptions();
        $query = Patient::query()
            ->with('sponsor')
            ->withSum('walletTransactions as wallet_balance', 'amount')
            ->withSum(['debts as debt_amount_sum' => fn ($query) => $query->where('status', '!=', DebtStatus::Void)], 'amount')
            ->withSum(['debts as debt_paid_sum' => fn ($query) => $query->where('status', '!=', DebtStatus::Void)], 'paid_amount')
            ->with(['subscriptions' => fn ($query) => $this->usableSubscriptions($query)]);

        if ($search = $request->string('search')->trim()->toString()) {
            $query->where(fn ($builder) => $builder
                ->where('name', 'like', "%{$search}%")
                ->orWhere('phone', 'like', "%{$search}%"));
        }

        if ($request->string('subscription_status')->toString() === 'active') {
            $query->whereHas('subscriptions', fn ($builder) => $this->usableSubscriptions($builder));
        } elseif ($request->string('subscription_status')->toString() === 'none') {
            $query->whereDoesntHave('subscriptions', fn ($builder) => $this->usableSubscriptions($builder));
        }

        $paginator = $query->latest('id')->paginate($this->perPage($request));
        $paginator->getCollection()->each(fn (Patient $patient) => $this->setFinancialAttributes($patient));

        return $this->paginated($paginator, PatientResource::class, $request);
    }

    public function store(StorePatientRequest $request): PatientResource
    {
        $data = $request->validated();
        $patient = DB::transaction(function () use ($data, $request): Patient {
            if (Patient::query()->where('phone', $data['phone'])->exists() && ! ($data['confirm_duplicate'] ?? false)) {
                throw ApiException::validation('يوجد مريض مسجل برقم الهاتف نفسه. أكد الحفظ إذا كان هذا سجلًا مختلفًا.', [
                    'phone' => ['رقم الهاتف مستخدم في سجل آخر.'],
                    'confirm_duplicate' => ['يلزم تأكيد إنشاء سجل مكرر.'],
                ]);
            }

            $patient = Patient::create([
                ...Arr::except($data, ['confirm_duplicate']),
                'created_by' => $request->user()->id,
            ]);
            $this->auditLogger->log($request->user(), 'patient_created', $patient, $patient->id, null, $patient->toArray());

            return $patient;
        });

        return PatientResource::make($this->loadSummary($patient));
    }

    public function show(Patient $patient, Request $request): JsonResponse
    {
        $this->subscriptionService->expireMonthlySubscriptions();
        $patient->load([
            'sponsor',
            'sessions' => fn ($query) => $query->with(['doctor', 'services'])->latest('opened_at'),
            'subscriptions' => fn ($query) => $query->with('services')->latest('id'),
            'walletTransactions' => fn ($query) => $query->latest('id'),
            'debts' => fn ($query) => $query->latest('id'),
            'payments' => fn ($query) => $query->latest('id'),
        ]);
        $patient->setAttribute('wallet_balance', Money::sum($patient->walletTransactions->pluck('amount')));
        $patient->setAttribute('debt_total', Money::sum($patient->debts
            ->where('status', '!=', DebtStatus::Void)
            ->map(fn ($debt) => Money::subtract($debt->amount, $debt->paid_amount))));
        $patient->setAttribute('subscription_status', $patient->subscriptions->contains('status', SubscriptionStatus::Active) ? 'active' : null);

        return response()->json(['data' => [
            ...PatientResource::make($patient)->resolve($request),
            'sessions' => SessionResource::collection($patient->sessions)->resolve($request),
            'subscriptions' => SubscriptionResource::collection($patient->subscriptions)->resolve($request),
            'wallet_movements' => WalletTransactionResource::collection($patient->walletTransactions)->resolve($request),
            'debts' => DebtResource::collection($patient->debts)->resolve($request),
            'payments' => PaymentResource::collection($patient->payments)->resolve($request),
        ]]);
    }

    public function update(UpdatePatientRequest $request, Patient $patient): PatientResource
    {
        $data = $request->validated();
        DB::transaction(function () use ($data, $patient, $request): void {
            if (Patient::query()->where('id', '!=', $patient->id)->where('phone', $data['phone'])->exists() && ! ($data['confirm_duplicate'] ?? false)) {
                throw ApiException::validation('يوجد مريض مسجل برقم الهاتف نفسه. أكد الحفظ إذا كان هذا سجلًا مختلفًا.', [
                    'phone' => ['رقم الهاتف مستخدم في سجل آخر.'],
                    'confirm_duplicate' => ['يلزم تأكيد حفظ سجل مكرر.'],
                ]);
            }

            $old = $patient->toArray();
            $patient->update(Arr::except($data, ['confirm_duplicate']));
            $this->auditLogger->log($request->user(), 'patient_updated', $patient, $patient->id, $old, $patient->fresh()->toArray());
        });

        return PatientResource::make($this->loadSummary($patient->fresh()));
    }

    public function statement(Patient $patient, Request $request): JsonResponse
    {
        $from = $request->date('from');
        $to = $request->date('to');
        $period = fn ($query) => $query
            ->when($from, fn ($builder) => $builder->whereDate('created_at', '>=', $from))
            ->when($to, fn ($builder) => $builder->whereDate('created_at', '<=', $to));
        $patient->load([
            'walletTransactions' => $period,
            'debts' => fn ($query) => $period($query)->with('allocations'),
            'payments' => fn ($query) => $period($query)->with('allocations'),
        ]);

        return response()->json(['data' => [
            'patient' => ['id' => $patient->id, 'name' => $patient->name, 'phone' => $patient->phone],
            'wallet_balance' => Money::normalize($patient->walletTransactions()->sum('amount')),
            'wallet_movements' => WalletTransactionResource::collection($patient->walletTransactions)->resolve($request),
            'debts' => DebtResource::collection($patient->debts)->resolve($request),
            'payments' => PaymentResource::collection($patient->payments)->resolve($request),
            'allocations' => PaymentAllocationResource::collection(
                $patient->payments->flatMap->allocations->values(),
            )->resolve($request),
            'totals' => [
                'debt_original' => Money::sum($patient->debts->pluck('amount')),
                'debt_paid' => Money::sum($patient->debts->pluck('paid_amount')),
            ],
        ]]);
    }

    private function loadSummary(Patient $patient): Patient
    {
        $patient->load([
            'sponsor',
            'subscriptions' => fn (HasMany $query): HasMany => $this->usableSubscriptions($query),
        ]);
        $patient->setAttribute('wallet_balance', Money::normalize($patient->walletTransactions()->sum('amount')));
        $patient->setAttribute('debt_total', Money::sum($patient->debts()
            ->where('status', '!=', DebtStatus::Void)
            ->get()
            ->map(fn ($debt) => Money::subtract($debt->amount, $debt->paid_amount))));
        $patient->setAttribute('subscription_status', $patient->subscriptions->isNotEmpty() ? 'active' : null);

        return $patient;
    }

    private function setFinancialAttributes(Patient $patient): void
    {
        $patient->setAttribute('wallet_balance', Money::normalize($patient->getAttribute('wallet_balance')));
        $patient->setAttribute('debt_total', Money::subtract(
            $patient->getAttribute('debt_amount_sum') ?? '0',
            $patient->getAttribute('debt_paid_sum') ?? '0',
        ));
        $patient->setAttribute('subscription_status', $patient->subscriptions->isNotEmpty() ? 'active' : null);
    }

    private function usableSubscriptions(Builder|HasMany $query): Builder|HasMany
    {
        return $query
            ->where('status', SubscriptionStatus::Active)
            ->where(function ($builder): void {
                $builder->where('plan_type', SubscriptionPlanType::LegacyPackage)
                    ->orWhere(function ($monthly): void {
                        $monthly->where('plan_type', SubscriptionPlanType::Monthly)
                            ->whereDate('start_date', '<=', today())
                            ->whereDate('end_date', '>=', today());
                    });
            });
    }
}
