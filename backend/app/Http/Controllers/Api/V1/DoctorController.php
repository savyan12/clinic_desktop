<?php

namespace App\Http\Controllers\Api\V1;

use App\Enums\SessionStatus;
use App\Exceptions\ApiException;
use App\Http\Controllers\Controller;
use App\Http\Requests\Doctors\StoreDoctorRequest;
use App\Http\Requests\Doctors\UpdateDoctorRequest;
use App\Http\Resources\DoctorPayoutResource;
use App\Http\Resources\DoctorResource;
use App\Http\Resources\SessionResource;
use App\Models\Doctor;
use App\Services\AuditLogger;
use App\Support\Money;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use Illuminate\Support\Arr;
use Illuminate\Support\Facades\DB;

class DoctorController extends Controller
{
    public function __construct(private AuditLogger $auditLogger) {}

    public function index(Request $request): JsonResponse
    {
        $from = $request->date('from');
        $to = $request->date('to');
        $query = Doctor::query()->with([
            'sessions' => fn ($builder) => $builder
                ->where('status', SessionStatus::Closed)
                ->when($from, fn ($query) => $query->whereDate('closed_at', '>=', $from))
                ->when($to, fn ($query) => $query->whereDate('closed_at', '<=', $to)),
            'payouts' => fn ($builder) => $builder
                ->when($from, fn ($query) => $query->whereDate('date', '>=', $from))
                ->when($to, fn ($query) => $query->whereDate('date', '<=', $to)),
        ]);

        if ($search = $request->string('search')->trim()->toString()) {
            $query->where('name', 'like', "%{$search}%");
        }

        if ($request->has('is_active')) {
            $query->where('is_active', $request->boolean('is_active'));
        }

        $paginator = $query->latest('id')->paginate($this->perPage($request));
        $paginator->getCollection()->each(fn (Doctor $doctor) => $this->setDues($doctor));

        return $this->paginated($paginator, DoctorResource::class, $request);
    }

    public function store(StoreDoctorRequest $request): DoctorResource
    {
        $doctor = DB::transaction(function () use ($request): Doctor {
            $doctor = Doctor::create([...$request->validated(), 'created_by' => $request->user()->id]);
            $this->auditLogger->log($request->user(), 'doctor_created', $doctor, $doctor->id, null, $doctor->toArray());

            return $doctor;
        });

        return DoctorResource::make($doctor);
    }

    public function show(Doctor $doctor, Request $request): JsonResponse
    {
        $this->loadPeriod($doctor, $request);
        $this->setDues($doctor);

        return response()->json(['data' => [
            ...DoctorResource::make($doctor)->resolve($request),
            'sessions' => SessionResource::collection($doctor->sessions)->resolve($request),
            'payouts' => DoctorPayoutResource::collection($doctor->payouts)->resolve($request),
        ]]);
    }

    public function update(UpdateDoctorRequest $request, Doctor $doctor): DoctorResource
    {
        $data = $request->validated();
        $rateChanged = array_key_exists('commission_rate', $data)
            && Money::compare($data['commission_rate'], $doctor->commission_rate) !== 0;

        if ($rateChanged && empty($data['reason'])) {
            throw ApiException::validation('سبب تغيير نسبة الطبيب مطلوب.', [
                'reason' => ['اكتب سبب تغيير النسبة.'],
            ]);
        }

        DB::transaction(function () use ($data, $doctor, $rateChanged, $request): void {
            $old = $doctor->toArray();
            $doctor->update(Arr::except($data, ['reason']));
            $this->auditLogger->log(
                $request->user(),
                $rateChanged ? 'doctor_rate_changed' : 'doctor_updated',
                $doctor,
                $doctor->id,
                $old,
                $doctor->fresh()->toArray(),
                $data['reason'] ?? null,
            );
        });

        return DoctorResource::make($doctor->fresh());
    }

    public function dues(Doctor $doctor, Request $request): JsonResponse
    {
        $this->loadPeriod($doctor, $request);
        $this->setDues($doctor);
        $allTimeEarned = Money::normalize($doctor->sessions()->where('status', SessionStatus::Closed)->sum('commission_amount'));
        $allTimePaid = Money::normalize($doctor->payouts()->sum('amount'));

        return response()->json(['data' => [
            'doctor' => DoctorResource::make($doctor)->resolve($request),
            'sessions' => SessionResource::collection($doctor->sessions)->resolve($request),
            'payouts' => DoctorPayoutResource::collection($doctor->payouts)->resolve($request),
            'all_time_remaining' => Money::subtract($allTimeEarned, $allTimePaid),
        ]]);
    }

    private function loadPeriod(Doctor $doctor, Request $request): void
    {
        $from = $request->date('from');
        $to = $request->date('to');
        $doctor->load([
            'sessions' => fn ($builder) => $builder
                ->where('status', SessionStatus::Closed)
                ->with(['patient', 'doctor', 'services'])
                ->when($from, fn ($query) => $query->whereDate('closed_at', '>=', $from))
                ->when($to, fn ($query) => $query->whereDate('closed_at', '<=', $to))
                ->latest('closed_at'),
            'payouts' => fn ($builder) => $builder
                ->when($from, fn ($query) => $query->whereDate('date', '>=', $from))
                ->when($to, fn ($query) => $query->whereDate('date', '<=', $to))
                ->latest('date'),
        ]);
    }

    private function setDues(Doctor $doctor): void
    {
        $earned = Money::sum($doctor->sessions->pluck('commission_amount'));
        $paid = Money::sum($doctor->payouts->pluck('amount'));
        $doctor->setAttribute('sessions_count', $doctor->sessions->count());
        $doctor->setAttribute('earned', $earned);
        $doctor->setAttribute('paid', $paid);
        $doctor->setAttribute('remaining', Money::subtract($earned, $paid));
    }
}
