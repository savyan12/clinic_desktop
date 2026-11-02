<?php

namespace App\Http\Controllers\Api\V1;

use App\Enums\SubscriptionPlanType;
use App\Enums\SubscriptionStatus;
use App\Exceptions\ApiException;
use App\Http\Controllers\Controller;
use App\Http\Requests\Subscriptions\CancelSubscriptionRequest;
use App\Http\Requests\Subscriptions\CorrectSubscriptionRequest;
use App\Http\Requests\Subscriptions\StoreSubscriptionRequest;
use App\Http\Requests\Subscriptions\SubscriptionReasonRequest;
use App\Http\Resources\DebtResource;
use App\Http\Resources\PaymentResource;
use App\Http\Resources\SessionResource;
use App\Http\Resources\SubscriptionResource;
use App\Http\Resources\SubscriptionUsageResource;
use App\Models\Subscription;
use App\Services\SubscriptionService;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;

class SubscriptionController extends Controller
{
    public function __construct(private SubscriptionService $subscriptionService) {}

    public function index(Request $request): JsonResponse
    {
        $this->subscriptionService->expireMonthlySubscriptions();
        $query = Subscription::query()->with(['patient', 'sponsor', 'services']);

        if ($search = $request->string('search')->trim()->toString()) {
            $query->whereHas('patient', fn ($builder) => $builder
                ->where('name', 'like', "%{$search}%")
                ->orWhere('phone', 'like', "%{$search}%"));
        }

        if ($status = $request->string('status')->toString()) {
            $query->where('status', $status);
        }

        if ($request->boolean('near_end')) {
            $sessionsThreshold = (int) config('clinic.near_end_sessions');
            $dateThreshold = today()->addDays((int) config('clinic.near_end_days'))->toDateString();
            $query->where('status', SubscriptionStatus::Active)
                ->where(fn ($builder) => $builder
                    ->where(fn ($legacy) => $legacy
                        ->where('plan_type', SubscriptionPlanType::LegacyPackage)
                        ->where(fn ($criteria) => $criteria
                            ->whereRaw('(sessions_count - sessions_used) <= ?', [$sessionsThreshold])
                            ->orWhereDate('expected_end_date', '<=', $dateThreshold)))
                    ->orWhere(fn ($monthly) => $monthly
                        ->where('plan_type', SubscriptionPlanType::Monthly)
                        ->whereDate('end_date', '<=', $dateThreshold)));
        }

        return $this->paginated(
            $query->latest('id')->paginate($this->perPage($request)),
            SubscriptionResource::class,
            $request,
        );
    }

    public function store(StoreSubscriptionRequest $request): JsonResponse
    {
        $result = $this->subscriptionService->create($request->validated(), $request->user());
        $subscription = $result['subscription']->load(['patient', 'sponsor', 'services']);

        return response()->json(['data' => [
            'subscription' => SubscriptionResource::make($subscription)->resolve($request),
            'payment' => $result['payment'] ? PaymentResource::make($result['payment'])->resolve($request) : null,
            'debt' => $result['debt'] ? DebtResource::make($result['debt'])->resolve($request) : null,
        ]], 201);
    }

    public function show(Subscription $subscription, Request $request): JsonResponse
    {
        $this->subscriptionService->expireMonthlySubscriptions();
        $subscription->refresh();
        $subscription->load([
            'patient', 'sponsor', 'services',
            'usages' => fn ($query) => $query->latest('id'),
            'sessions' => fn ($query) => $query->with(['patient', 'doctor', 'services'])->latest('opened_at'),
            'payments' => fn ($query) => $query->latest('id'),
            'debts' => fn ($query) => $query->latest('id'),
        ]);

        return response()->json(['data' => [
            ...SubscriptionResource::make($subscription)->resolve($request),
            'usages' => SubscriptionUsageResource::collection($subscription->usages)->resolve($request),
            'sessions' => SessionResource::collection($subscription->sessions)->resolve($request),
            'payments' => PaymentResource::collection($subscription->payments)->resolve($request),
            'debts' => DebtResource::collection($subscription->debts)->resolve($request),
        ]]);
    }

    public function usage(Subscription $subscription, Request $request): JsonResponse
    {
        $paginator = $subscription->usages()->latest('id')->paginate($this->perPage($request));

        return $this->paginated($paginator, SubscriptionUsageResource::class, $request);
    }

    public function stop(SubscriptionReasonRequest $request, Subscription $subscription): SubscriptionResource
    {
        if (! $request->filled('reason')) {
            throw ApiException::validation('سبب إيقاف الاشتراك مطلوب.', ['reason' => ['اكتب سبب الإيقاف.']]);
        }

        return SubscriptionResource::make($this->subscriptionService->stop(
            $subscription,
            $request->string('reason')->toString(),
            $request->user(),
        )->load(['patient', 'sponsor', 'services']));
    }

    public function reactivate(SubscriptionReasonRequest $request, Subscription $subscription): SubscriptionResource
    {
        return SubscriptionResource::make($this->subscriptionService->reactivate(
            $subscription,
            $request->string('reason')->toString() ?: null,
            $request->user(),
        )->load(['patient', 'services']));
    }

    public function cancelPreview(Subscription $subscription): JsonResponse
    {
        return response()->json(['data' => $this->subscriptionService->cancellationPreview($subscription)]);
    }

    public function cancel(CancelSubscriptionRequest $request, Subscription $subscription): JsonResponse
    {
        $result = $this->subscriptionService->cancel(
            $subscription,
            $request->string('reason')->toString(),
            $request->user(),
        );

        return response()->json(['data' => [
            'subscription' => SubscriptionResource::make($result['subscription'])->resolve($request),
            'preview' => $result['preview'],
            'refund' => $result['refund'] ? PaymentResource::make($result['refund'])->resolve($request) : null,
            'voided_debt_ids' => $result['voided_debt_ids'],
            'replacement_debt' => $result['replacement_debt'] ? DebtResource::make($result['replacement_debt'])->resolve($request) : null,
        ]]);
    }

    public function correct(CorrectSubscriptionRequest $request, Subscription $subscription): JsonResponse
    {
        $corrected = $this->subscriptionService->correct(
            $subscription,
            $request->validated('changes'),
            $request->string('reason')->toString(),
            $request->user(),
        )->load(['patient', 'sponsor', 'services']);

        return response()->json(['data' => [
            'subscription' => SubscriptionResource::make($corrected)->resolve($request),
            'impact' => ['historical_financial_rows_changed' => false],
        ]]);
    }
}
