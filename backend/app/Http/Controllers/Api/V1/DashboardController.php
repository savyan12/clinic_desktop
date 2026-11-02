<?php

namespace App\Http\Controllers\Api\V1;

use App\Enums\DebtStatus;
use App\Enums\SessionStatus;
use App\Enums\SubscriptionPlanType;
use App\Enums\SubscriptionStatus;
use App\Enums\UserRole;
use App\Http\Controllers\Controller;
use App\Http\Resources\PaymentResource;
use App\Http\Resources\SessionResource;
use App\Http\Resources\SubscriptionResource;
use App\Models\Debt;
use App\Models\DoctorPayout;
use App\Models\Payment;
use App\Models\Session;
use App\Models\Subscription;
use App\Services\SubscriptionService;
use App\Support\Money;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;

class DashboardController extends Controller
{
    public function __construct(private SubscriptionService $subscriptionService) {}

    public function __invoke(Request $request): JsonResponse
    {
        $this->subscriptionService->expireMonthlySubscriptions();
        $date = $request->date('date')?->toDateString() ?? today()->toDateString();
        $sessionsToday = Session::query()->whereDate('opened_at', $date);
        $todaySessions = Session::query()
            ->whereDate('opened_at', $date)
            ->with(['patient', 'doctor', 'services'])
            ->latest('opened_at')
            ->get();
        $openSessions = Session::query()
            ->where('status', SessionStatus::Open)
            ->with(['patient', 'doctor', 'services'])
            ->oldest('opened_at')
            ->get();
        $nearSubscriptions = Subscription::query()
            ->where('status', SubscriptionStatus::Active)
            ->where(fn ($query) => $query
                ->where(fn ($legacy) => $legacy
                    ->where('plan_type', SubscriptionPlanType::LegacyPackage)
                    ->where(fn ($criteria) => $criteria
                        ->whereRaw('(sessions_count - sessions_used) <= ?', [(int) config('clinic.near_end_sessions')])
                        ->orWhereDate('expected_end_date', '<=', today()->addDays((int) config('clinic.near_end_days')))))
                ->orWhere(fn ($monthly) => $monthly
                    ->where('plan_type', SubscriptionPlanType::Monthly)
                    ->whereDate('end_date', '<=', today()->addDays((int) config('clinic.near_end_days')))))
            ->with(['patient', 'services'])
            ->latest('id')
            ->get();
        $baseStats = [
            [
                'key' => 'sessions_today',
                'label' => 'جلسات اليوم',
                'value' => (clone $sessionsToday)->count(),
                'money' => false,
                'to' => "/reports/sessions?from={$date}&to={$date}",
            ],
            [
                'key' => 'patients_today',
                'label' => 'المرضى الحاضرون اليوم',
                'value' => (clone $sessionsToday)->distinct()->count('patient_id'),
                'money' => false,
                'to' => "/reports/sessions?from={$date}&to={$date}",
            ],
        ];

        if ($request->user()->role === UserRole::Receptionist) {
            $stats = [...$baseStats, [
                'key' => 'near_subscriptions',
                'label' => 'الاشتراكات القريبة من الانتهاء',
                'value' => $nearSubscriptions->count(),
                'money' => false,
                'to' => '/subscriptions?near_end=1',
            ]];

            return response()->json(['data' => [
                'stats' => $stats,
                'open_sessions' => SessionResource::collection($openSessions)->resolve($request),
                'today_sessions' => SessionResource::collection($todaySessions)->resolve($request),
                'near_subscriptions' => SubscriptionResource::collection($nearSubscriptions)->resolve($request),
            ]]);
        }

        $serviceValue = Money::normalize((clone $sessionsToday)
            ->where('status', SessionStatus::Closed)
            ->sum('net_total'));
        $collections = Money::normalize(Payment::query()->whereDate('created_at', $date)->sum('amount'));
        $debtsRemaining = Money::sum(Debt::query()
            ->where('status', '!=', DebtStatus::Void)
            ->get()
            ->map(fn ($debt) => Money::subtract($debt->amount, $debt->paid_amount)));
        $doctorEarned = Money::normalize(Session::query()->where('status', SessionStatus::Closed)->sum('commission_amount'));
        $doctorPaid = Money::normalize(DoctorPayout::query()->sum('amount'));
        $doctorRemaining = Money::subtract($doctorEarned, $doctorPaid);
        $stats = [
            ...$baseStats,
            ['key' => 'service_value_today', 'label' => 'قيمة جلسات اليوم', 'value' => $serviceValue, 'money' => true, 'to' => "/reports/sessions?from={$date}&to={$date}"],
            ['key' => 'collections_today', 'label' => 'المقبوضات اليوم', 'value' => $collections, 'money' => true, 'to' => "/reports/collections?from={$date}&to={$date}"],
            ['key' => 'debts_remaining', 'label' => 'إجمالي الديون', 'value' => $debtsRemaining, 'money' => true, 'to' => '/reports/debts'],
            ['key' => 'near_subscriptions', 'label' => 'الاشتراكات القريبة', 'value' => $nearSubscriptions->count(), 'money' => false, 'to' => '/subscriptions?near_end=1'],
            ['key' => 'doctor_dues_remaining', 'label' => 'مستحقات الأطباء غير المصروفة', 'value' => $doctorRemaining, 'money' => true, 'to' => '/reports/doctor-dues'],
        ];
        $recentMovements = Payment::query()->with('patient')->latest('id')->limit(10)->get();

        return response()->json(['data' => [
            'stats' => $stats,
            'open_sessions' => SessionResource::collection($openSessions)->resolve($request),
            'today_sessions' => SessionResource::collection($todaySessions)->resolve($request),
            'near_subscriptions' => SubscriptionResource::collection($nearSubscriptions)->resolve($request),
            'financial_summary' => [
                'collections_today' => $collections,
                'debts_remaining' => $debtsRemaining,
                'doctor_dues_remaining' => $doctorRemaining,
            ],
            'recent_movements' => PaymentResource::collection($recentMovements)->resolve($request),
        ]]);
    }
}
