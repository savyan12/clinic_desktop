<?php

namespace App\Services;

use App\Enums\PaymentMethod;
use App\Enums\SessionStatus;
use App\Exceptions\ApiException;
use App\Models\Doctor;
use App\Models\DoctorPayout;
use App\Models\Session;
use App\Models\User;
use App\Support\Money;
use Illuminate\Support\Facades\DB;

class DoctorPayoutService
{
    public function __construct(private AuditLogger $auditLogger) {}

    public function remaining(Doctor|int $doctor): string
    {
        $doctorId = $doctor instanceof Doctor ? $doctor->id : $doctor;
        $earned = Money::normalize(Session::query()
            ->where('doctor_id', $doctorId)
            ->where('status', SessionStatus::Closed)
            ->sum('commission_amount'));
        $paid = Money::normalize(DoctorPayout::query()->where('doctor_id', $doctorId)->sum('amount'));

        return Money::subtract($earned, $paid);
    }

    public function create(
        Doctor $doctor,
        string $amount,
        string $date,
        PaymentMethod $method,
        ?string $note,
        User $actor,
    ): DoctorPayout {
        return DB::transaction(function () use ($doctor, $amount, $date, $method, $note, $actor): DoctorPayout {
            $lockedDoctor = Doctor::query()->lockForUpdate()->findOrFail($doctor->id);
            $normalizedAmount = Money::normalize($amount);
            $remaining = $this->remaining($lockedDoctor);

            if (Money::compare($normalizedAmount, $remaining) > 0) {
                throw ApiException::validation('مبلغ الصرف يتجاوز مستحقات الطبيب المتبقية.', [
                    'amount' => ['المستحق المتبقي هو '.$remaining.'.'],
                ]);
            }

            $payout = DoctorPayout::create([
                'doctor_id' => $lockedDoctor->id,
                'amount' => $normalizedAmount,
                'date' => $date,
                'method' => $method,
                'note' => $note,
                'created_by' => $actor->id,
            ]);

            $this->auditLogger->log($actor, 'doctor_payout_created', $payout, $payout->id, null, $payout->toArray());

            return $payout;
        });
    }
}
