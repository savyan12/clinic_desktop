<?php

namespace App\Http\Controllers\Api\V1;

use App\Enums\DebtStatus;
use App\Http\Controllers\Controller;
use App\Http\Requests\Sponsors\StoreSponsorRequest;
use App\Http\Requests\Sponsors\UpdateSponsorRequest;
use App\Http\Resources\DebtResource;
use App\Http\Resources\PaymentAllocationResource;
use App\Http\Resources\PaymentResource;
use App\Http\Resources\SponsorResource;
use App\Models\Debt;
use App\Models\Patient;
use App\Models\Payment;
use App\Models\Session;
use App\Models\Sponsor;
use App\Services\AuditLogger;
use App\Support\Money;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use Illuminate\Support\Collection;
use Illuminate\Support\Facades\DB;

class SponsorController extends Controller
{
    public function __construct(private AuditLogger $auditLogger) {}

    public function index(Request $request): JsonResponse
    {
        $query = Sponsor::query()->withCount('patients')->with('debts');

        if ($search = $request->string('search')->trim()->toString()) {
            $query->where('name', 'like', "%{$search}%");
        }

        if ($request->has('is_active')) {
            $query->where('is_active', $request->boolean('is_active'));
        }

        $paginator = $query->latest('id')->paginate($this->perPage($request));
        $paginator->getCollection()->each(fn (Sponsor $sponsor) => $this->setTotals($sponsor));

        return $this->paginated($paginator, SponsorResource::class, $request);
    }

    public function store(StoreSponsorRequest $request): SponsorResource
    {
        $sponsor = DB::transaction(function () use ($request): Sponsor {
            $sponsor = Sponsor::create([...$request->validated(), 'created_by' => $request->user()->id]);
            $this->auditLogger->log($request->user(), 'sponsor_created', $sponsor, $sponsor->id, null, $sponsor->toArray());

            return $sponsor;
        });

        return SponsorResource::make($sponsor);
    }

    public function show(Sponsor $sponsor, Request $request): JsonResponse
    {
        $sponsor->load([
            'patients',
            'debts.allocations.payment',
        ])->loadCount('patients');
        $patientIds = $sponsor->patients->pluck('id')
            ->merge($sponsor->debts->pluck('patient_id'))
            ->unique()
            ->values();
        $patients = Patient::query()->whereIn('id', $patientIds)->orderBy('name')->get(['id', 'name', 'phone']);
        $sessions = $this->sessionsForDebts($sponsor->debts);
        $payments = $this->paymentsForDebts($sponsor->debts);
        $this->setTotals($sponsor);

        return response()->json(['data' => [
            ...SponsorResource::make($sponsor)->resolve($request),
            'patients' => $this->patientSummaries($patients),
            'sessions' => $this->sessionSummaries($sessions),
            'debts' => DebtResource::collection($sponsor->debts)->resolve($request),
            'payments' => PaymentResource::collection($payments)->resolve($request),
        ]]);
    }

    public function update(UpdateSponsorRequest $request, Sponsor $sponsor): SponsorResource
    {
        DB::transaction(function () use ($request, $sponsor): void {
            $old = $sponsor->toArray();
            $sponsor->update($request->validated());
            $this->auditLogger->log($request->user(), 'sponsor_updated', $sponsor, $sponsor->id, $old, $sponsor->fresh()->toArray());
        });

        return SponsorResource::make($sponsor->fresh());
    }

    public function statement(Sponsor $sponsor, Request $request): JsonResponse
    {
        $from = $request->date('from');
        $to = $request->date('to');
        $debts = $sponsor->debts()
            ->with(['patient', 'session.services', 'allocations.payment'])
            ->when($request->integer('patient_id'), fn ($query, $patientId) => $query->where('patient_id', $patientId))
            ->when($from, fn ($query) => $query->whereDate('created_at', '>=', $from))
            ->when($to, fn ($query) => $query->whereDate('created_at', '<=', $to))
            ->latest('id')
            ->get();
        $patients = Patient::query()
            ->whereIn('id', $debts->pluck('patient_id')->unique())
            ->orderBy('name')
            ->get(['id', 'name', 'phone']);
        $sessions = $this->sessionsForDebts($debts);
        $payments = $this->paymentsForDebts($debts);
        $allocations = $debts->flatMap->allocations->values();
        $activeDebts = $debts->filter(fn (Debt $debt) => $debt->status !== DebtStatus::Void);

        return response()->json(['data' => [
            'sponsor' => ['id' => $sponsor->id, 'name' => $sponsor->name],
            'patients' => $this->patientSummaries($patients),
            'sessions' => $this->sessionSummaries($sessions),
            'debts' => DebtResource::collection($debts)->resolve($request),
            'payments' => PaymentResource::collection($payments)->resolve($request),
            'allocations' => PaymentAllocationResource::collection($allocations)->resolve($request),
            'totals' => [
                'due' => Money::sum($activeDebts->pluck('amount')),
                'paid' => Money::sum($activeDebts->pluck('paid_amount')),
                'remaining' => Money::sum($activeDebts->map(
                    fn ($debt) => Money::subtract($debt->amount, $debt->paid_amount),
                )),
            ],
        ]]);
    }

    private function setTotals(Sponsor $sponsor): void
    {
        $debts = $sponsor->relationLoaded('debts') ? $sponsor->debts : $sponsor->debts()->get();
        $activeDebts = $debts->filter(fn (Debt $debt) => $debt->status !== DebtStatus::Void);
        $sponsor->setAttribute('total_due', Money::sum($activeDebts->pluck('amount')));
        $sponsor->setAttribute('total_paid', Money::sum($activeDebts->pluck('paid_amount')));
        $sponsor->setAttribute('remaining', Money::sum($activeDebts->map(
            fn ($debt) => Money::subtract($debt->amount, $debt->paid_amount),
        )));
    }

    /** @param Collection<int, Debt> $debts */
    private function sessionsForDebts(Collection $debts): Collection
    {
        $sessionIds = $debts->pluck('session_id')->filter()->unique()->values();
        $subscriptionIds = $debts->pluck('subscription_id')->filter()->unique()->values();

        if ($sessionIds->isEmpty() && $subscriptionIds->isEmpty()) {
            return collect();
        }

        return Session::query()
            ->where(function ($query) use ($sessionIds, $subscriptionIds): void {
                if ($sessionIds->isNotEmpty()) {
                    $query->whereIn('id', $sessionIds);
                }

                if ($subscriptionIds->isNotEmpty()) {
                    $sessionIds->isNotEmpty()
                        ? $query->orWhereIn('subscription_id', $subscriptionIds)
                        : $query->whereIn('subscription_id', $subscriptionIds);
                }
            })
            ->with(['patient', 'services'])
            ->latest('opened_at')
            ->get();
    }

    /** @param Collection<int, Debt> $debts */
    private function paymentsForDebts(Collection $debts): Collection
    {
        $paymentIds = $debts->flatMap->allocations->pluck('payment_id')->unique()->values();

        if ($paymentIds->isEmpty()) {
            return collect();
        }

        return Payment::query()
            ->where(fn ($query) => $query
                ->whereIn('id', $paymentIds)
                ->orWhereIn('reverses_payment_id', $paymentIds))
            ->latest('id')
            ->get();
    }

    /** @param Collection<int, Patient> $patients */
    private function patientSummaries(Collection $patients): array
    {
        return $patients->map(fn (Patient $patient) => [
            'id' => $patient->id,
            'name' => $patient->name,
            'phone' => $patient->phone,
        ])->values()->all();
    }

    /** @param Collection<int, Session> $sessions */
    private function sessionSummaries(Collection $sessions): array
    {
        return $sessions->map(fn (Session $session) => [
            'id' => $session->id,
            'code' => $session->code,
            'patient_id' => $session->patient_id,
            'patient' => [
                'id' => $session->patient->id,
                'name' => $session->patient->name,
                'phone' => $session->patient->phone,
            ],
            'services' => $session->services->map(fn ($service) => [
                'service_id' => $service->service_id,
                'name' => $service->service_name,
                'price' => Money::normalize($service->price),
            ])->values()->all(),
            'status' => $session->status->value,
            'opened_at' => $session->opened_at?->toIso8601String(),
            'closed_at' => $session->closed_at?->toIso8601String(),
            'net_total' => Money::normalize($session->net_total),
        ])->values()->all();
    }
}
