<?php

namespace App\Services;

use App\Enums\DebtStatus;
use App\Enums\PaymentType;
use App\Enums\SessionStatus;
use App\Enums\SubscriptionPlanType;
use App\Enums\SubscriptionStatus;
use App\Exceptions\ApiException;
use App\Models\Debt;
use App\Models\Doctor;
use App\Models\Payment;
use App\Models\Session;
use App\Models\Subscription;
use App\Support\Money;
use Mpdf\Mpdf;
use OpenSpout\Common\Entity\Row;
use OpenSpout\Writer\XLSX\Writer;

class ReportService
{
    /** @param array<string, mixed> $filters */
    public function report(string $type, array $filters): array
    {
        $report = match ($type) {
            'sessions' => $this->sessions($filters),
            'collections' => $this->collections($filters),
            'debts' => $this->debts($filters),
            'doctor-dues' => $this->doctorDues($filters),
            'subscriptions' => $this->subscriptions($filters),
            default => throw new ApiException('تعذر العثور على التقرير المطلوب.', 404),
        };

        $report['meta'] = [
            'type' => $type,
            'from' => $filters['from'] ?? null,
            'to' => $filters['to'] ?? null,
            'generated_at' => now()->toIso8601String(),
        ];

        return $report;
    }

    /**
     * @param  array<string, mixed>  $report
     * @return array{path: string, name: string, mime: string}
     */
    public function export(string $type, string $format, array $report): array
    {
        return match ($format) {
            'xlsx' => $this->exportXlsx($type, $report),
            'pdf' => $this->exportPdf($type, $report),
            default => throw ApiException::validation('صيغة التصدير غير مدعومة.', [
                'format' => ['اختر xlsx أو pdf.'],
            ]),
        };
    }

    /** @param array<string, mixed> $filters */
    private function sessions(array $filters): array
    {
        $query = Session::query()->with(['patient', 'doctor', 'services']);
        $this->period($query, $filters, 'opened_at');

        foreach (['doctor_id', 'patient_id', 'status'] as $field) {
            if (! empty($filters[$field])) {
                $query->where($field, $filters[$field]);
            }
        }

        $sessions = $query->latest('opened_at')->get();
        $rows = $sessions->map(fn (Session $session) => [
            'id' => $session->id,
            'code' => $session->code,
            'patient_id' => $session->patient_id,
            'patient' => [
                'id' => $session->patient->id,
                'name' => $session->patient->name,
                'phone' => $session->patient->phone,
            ],
            'doctor_id' => $session->doctor_id,
            'doctor' => [
                'id' => $session->doctor->id,
                'name' => $session->doctor->name,
            ],
            'services' => $session->services->map(fn ($service) => [
                'service_id' => $service->service_id,
                'name' => $service->service_name,
                'price' => Money::normalize($service->price),
            ])->values()->all(),
            'type' => $session->type->value,
            'status' => $session->status->value,
            'opened_at' => $session->opened_at?->toIso8601String(),
            'closed_at' => $session->closed_at?->toIso8601String(),
            'net_total' => Money::normalize($session->net_total),
        ])->values()->all();

        return ['rows' => $rows, 'totals' => [
            'count' => count($rows),
            'service_value' => Money::sum($sessions
                ->where('status', SessionStatus::Closed)
                ->pluck('net_total')),
        ]];
    }

    /** @param array<string, mixed> $filters */
    private function collections(array $filters): array
    {
        $query = Payment::query()->with('patient');
        $this->period($query, $filters, 'created_at');

        foreach (['patient_id', 'type', 'method'] as $field) {
            if (! empty($filters[$field])) {
                $query->where($field, $filters[$field]);
            }
        }

        if (! empty($filters['sponsor_id'])) {
            $query->where(fn ($builder) => $builder
                ->whereHas('allocations.debt', fn ($debtQuery) => $debtQuery->where('sponsor_id', $filters['sponsor_id']))
                ->orWhereHas('session', fn ($sessionQuery) => $sessionQuery->where('sponsor_id', $filters['sponsor_id']))
                ->orWhereHas('subscription', fn ($subscriptionQuery) => $subscriptionQuery->where('sponsor_id', $filters['sponsor_id']))
                ->orWhereHas('reversedPayment.allocations.debt', fn ($debtQuery) => $debtQuery->where('sponsor_id', $filters['sponsor_id']))
                ->orWhereHas('reversedPayment.session', fn ($sessionQuery) => $sessionQuery->where('sponsor_id', $filters['sponsor_id']))
                ->orWhereHas('reversedPayment.subscription', fn ($subscriptionQuery) => $subscriptionQuery->where('sponsor_id', $filters['sponsor_id'])));
        }

        $payments = $query->latest('id')->get();
        $received = Money::sum($payments->where('type', '!=', PaymentType::Refund)->pluck('amount'));
        $refunds = Money::sum($payments->where('type', PaymentType::Refund)->map(fn ($payment) => ltrim($payment->amount, '-')));
        $rows = $payments->map(fn (Payment $payment) => [
            'id' => $payment->id,
            'patient_id' => $payment->patient_id,
            'patient' => [
                'id' => $payment->patient->id,
                'name' => $payment->patient->name,
                'phone' => $payment->patient->phone,
            ],
            'session_id' => $payment->session_id,
            'subscription_id' => $payment->subscription_id,
            'type' => $payment->type->value,
            'method' => $payment->method->value,
            'amount' => Money::normalize($payment->amount),
            'created_at' => $payment->created_at?->toIso8601String(),
        ])->values()->all();

        return ['rows' => $rows, 'totals' => [
            'received' => $received,
            'refunds' => $refunds,
            'net' => Money::subtract($received, $refunds),
        ]];
    }

    /** @param array<string, mixed> $filters */
    private function debts(array $filters): array
    {
        $query = Debt::query()->with(['patient', 'sponsor']);
        $this->period($query, $filters, 'created_at');

        foreach (['patient_id', 'sponsor_id', 'status', 'kind'] as $field) {
            if (! empty($filters[$field])) {
                $query->where($field, $filters[$field]);
            }
        }

        $debts = $query->latest('id')->get();
        $rows = $debts->map(fn (Debt $debt) => [
            'id' => $debt->id,
            'patient_id' => $debt->patient_id,
            'patient' => [
                'id' => $debt->patient->id,
                'name' => $debt->patient->name,
                'phone' => $debt->patient->phone,
            ],
            'sponsor_id' => $debt->sponsor_id,
            'sponsor' => $debt->sponsor ? [
                'id' => $debt->sponsor->id,
                'name' => $debt->sponsor->name,
            ] : null,
            'kind' => $debt->kind->value,
            'status' => $debt->status->value,
            'amount' => Money::normalize($debt->amount),
            'paid_amount' => Money::normalize($debt->paid_amount),
            'remaining' => $debt->status === DebtStatus::Void ? '0.000' : Money::subtract($debt->amount, $debt->paid_amount),
            'created_at' => $debt->created_at?->toIso8601String(),
        ])->values()->all();

        return ['rows' => $rows, 'totals' => [
            'original' => Money::sum($debts->pluck('amount')),
            'paid' => Money::sum($debts->pluck('paid_amount')),
            'remaining' => Money::sum(array_column($rows, 'remaining')),
        ]];
    }

    /** @param array<string, mixed> $filters */
    private function doctorDues(array $filters): array
    {
        $doctors = Doctor::query()
            ->when($filters['doctor_id'] ?? null, fn ($query, $doctorId) => $query->where('id', $doctorId))
            ->with([
                'sessions' => function ($query) use ($filters): void {
                    $query->where('status', SessionStatus::Closed);
                    $this->period($query, $filters, 'closed_at');
                },
                'payouts' => function ($query) use ($filters): void {
                    $this->period($query, $filters, 'date');
                },
            ])->orderBy('name')->get();
        $rows = $doctors->map(function (Doctor $doctor): array {
            $earned = Money::sum($doctor->sessions->pluck('commission_amount'));
            $paid = Money::sum($doctor->payouts->pluck('amount'));

            return [
                'id' => $doctor->id,
                'name' => $doctor->name,
                'phone' => $doctor->phone,
                'commission_rate' => Money::normalize($doctor->commission_rate),
                'is_active' => $doctor->is_active,
                'sessions_count' => $doctor->sessions->count(),
                'earned' => $earned,
                'paid' => $paid,
                'remaining' => Money::subtract($earned, $paid),
            ];
        })->values()->all();

        return ['rows' => $rows, 'totals' => [
            'earned' => Money::sum(array_column($rows, 'earned')),
            'paid' => Money::sum(array_column($rows, 'paid')),
            'remaining' => Money::sum(array_column($rows, 'remaining')),
        ]];
    }

    /** @param array<string, mixed> $filters */
    private function subscriptions(array $filters): array
    {
        $query = Subscription::query()->with(['patient', 'services']);
        $this->period($query, $filters, 'start_date');

        foreach (['patient_id', 'status'] as $field) {
            if (! empty($filters[$field])) {
                $query->where($field, $filters[$field]);
            }
        }

        if (! empty($filters['near_end'])) {
            $dateThreshold = today()->addDays((int) config('clinic.near_end_days'))->toDateString();
            $query->where('status', SubscriptionStatus::Active)
                ->where(fn ($builder) => $builder
                    ->where(fn ($legacy) => $legacy
                        ->where('plan_type', SubscriptionPlanType::LegacyPackage)
                        ->where(fn ($criteria) => $criteria
                            ->whereRaw('(sessions_count - sessions_used) <= ?', [(int) config('clinic.near_end_sessions')])
                            ->orWhereDate('expected_end_date', '<=', $dateThreshold)))
                    ->orWhere(fn ($monthly) => $monthly
                        ->where('plan_type', SubscriptionPlanType::Monthly)
                        ->whereDate('end_date', '<=', $dateThreshold)));
        }

        $subscriptions = $query->latest('id')->get();
        $rows = $subscriptions->map(function (Subscription $subscription): array {
            $isMonthly = $subscription->plan_type === SubscriptionPlanType::Monthly;

            return [
                'id' => $subscription->id,
                'patient_id' => $subscription->patient_id,
                'patient' => [
                    'id' => $subscription->patient->id,
                    'name' => $subscription->patient->name,
                    'phone' => $subscription->patient->phone,
                ],
                'plan_type' => $subscription->plan_type->value,
                'services' => $subscription->services->map(fn ($service) => [
                    'service_id' => $service->service_id,
                    'name' => $service->service_name,
                    'price' => Money::normalize($service->price),
                ])->values()->all(),
                'sessions_count' => $isMonthly ? null : $subscription->sessions_count,
                'sessions_used' => $isMonthly ? null : $subscription->sessions_used,
                'remaining' => $isMonthly ? null : max(0, $subscription->sessions_count - $subscription->sessions_used),
                'remaining_days' => $isMonthly && $subscription->end_date
                    ? max(0, today()->diffInDays($subscription->end_date, false) + 1)
                    : null,
                'net_total' => Money::normalize($subscription->net_total),
                'discount' => Money::normalize($subscription->discount),
                'status' => $subscription->status->value,
                'start_date' => $subscription->start_date?->format('Y-m-d'),
                'expected_end_date' => $subscription->expected_end_date?->format('Y-m-d'),
                'end_date' => $subscription->end_date?->format('Y-m-d'),
            ];
        })->values()->all();

        return ['rows' => $rows, 'totals' => [
            'count' => count($rows),
            'active' => $subscriptions->where('status', SubscriptionStatus::Active)->count(),
            'remaining_sessions' => collect($rows)->sum(fn (array $row): int => (int) ($row['remaining'] ?? 0)),
            'net_total' => Money::sum($subscriptions->pluck('net_total')),
        ]];
    }

    /** @param array<string, mixed> $filters */
    private function period($query, array $filters, string $column): void
    {
        if (! empty($filters['from'])) {
            $query->whereDate($column, '>=', $filters['from']);
        }

        if (! empty($filters['to'])) {
            $query->whereDate($column, '<=', $filters['to']);
        }
    }

    /** @param array<string, mixed> $report */
    private function exportXlsx(string $type, array $report): array
    {
        $path = tempnam(sys_get_temp_dir(), 'clinic-report-');
        $writer = new Writer;
        $writer->openToFile($path);
        $rows = $this->rowsForExport($type, $report['rows']);
        $writer->addRow(Row::fromValues([$this->reportTitle($type)]));
        $writer->addRow(Row::fromValues(['الفترة', $this->periodLabel($report)]));

        if ($rows !== []) {
            $writer->addRow(Row::fromValues(array_map(fn ($heading) => $this->headingLabel($heading), array_keys($rows[0]))));

            foreach ($rows as $row) {
                $writer->addRow(Row::fromValues(array_map(
                    fn ($value, $key) => $this->displayValue($key, $value),
                    array_values($row),
                    array_keys($row),
                )));
            }
        }

        $writer->addRow(Row::fromValues(['الإجماليات']));

        foreach ($report['totals'] as $key => $value) {
            $writer->addRow(Row::fromValues([$this->headingLabel($key), $this->displayValue($key, $value)]));
        }

        $writer->close();

        return [
            'path' => $path,
            'name' => "تقرير-{$type}.xlsx",
            'mime' => 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
        ];
    }

    /** @param array<string, mixed> $report */
    private function exportPdf(string $type, array $report): array
    {
        $path = tempnam(sys_get_temp_dir(), 'clinic-report-');
        $mpdf = new Mpdf([
            'mode' => 'utf-8',
            'format' => 'A4',
            'tempDir' => sys_get_temp_dir(),
            'autoScriptToLang' => true,
            'autoLangToFont' => true,
        ]);
        $mpdf->SetDirectionality('rtl');
        $rows = $this->rowsForExport($type, $report['rows']);
        $headings = $rows === [] ? [] : array_keys($rows[0]);
        $headerHtml = implode('', array_map(fn ($heading) => '<th>'.e($this->headingLabel($heading)).'</th>', $headings));
        $bodyHtml = implode('', array_map(function (array $row): string {
            $cells = '';

            foreach ($row as $key => $value) {
                $cells .= '<td>'.e((string) $this->displayValue($key, $value)).'</td>';
            }

            return '<tr>'.$cells.'</tr>';
        }, $rows));
        $totalsHtml = '';

        foreach ($report['totals'] as $key => $value) {
            $totalsHtml .= '<tr><th>'.e($this->headingLabel($key)).'</th><td>'.e((string) $this->displayValue($key, $value)).'</td></tr>';
        }

        $title = e($this->reportTitle($type));
        $period = e($this->periodLabel($report));
        $generatedAt = e(now()->format('Y-m-d H:i'));
        $mpdf->WriteHTML("<html lang=\"ar\" dir=\"rtl\"><style>body{font-family:dejavusans;font-size:10px;color:#1d1d1f}h1{font-size:22px;margin:0 0 8px}.meta{color:#555;margin:0 0 16px}.report{border-collapse:collapse;width:100%}.report th,.report td{border:1px solid #ddd;padding:6px;text-align:right;vertical-align:top}.report th{background:#f5f5f7;font-weight:bold}.totals{border-collapse:collapse;margin-top:16px;width:45%}.totals th,.totals td{border:1px solid #ddd;padding:7px;text-align:right}.totals th{background:#f5f5f7}</style><body><h1>{$title}</h1><p class=\"meta\">الفترة: {$period} | تاريخ الإعداد: {$generatedAt}</p><table class=\"report\"><thead><tr>{$headerHtml}</tr></thead><tbody>{$bodyHtml}</tbody></table><h2>الإجماليات</h2><table class=\"totals\">{$totalsHtml}</table></body></html>");
        $mpdf->Output($path, 'F');

        return ['path' => $path, 'name' => "تقرير-{$type}.pdf", 'mime' => 'application/pdf'];
    }

    private function reportTitle(string $type): string
    {
        return match ($type) {
            'sessions' => 'تقرير الجلسات',
            'collections' => 'تقرير المقبوضات',
            'debts' => 'تقرير الديون',
            'doctor-dues' => 'تقرير مستحقات الأطباء',
            'subscriptions' => 'تقرير الاشتراكات',
            default => 'تقرير',
        };
    }

    /**
     * @param  array<int, array<string, mixed>>  $rows
     * @return array<int, array<string, mixed>>
     */
    private function rowsForExport(string $type, array $rows): array
    {
        $columns = match ($type) {
            'sessions' => ['opened_at', 'patient', 'doctor', 'services', 'net_total', 'status'],
            'collections' => ['created_at', 'type', 'patient', 'method', 'amount'],
            'debts' => ['id', 'patient', 'sponsor', 'amount', 'paid_amount', 'remaining', 'status'],
            'doctor-dues' => ['name', 'sessions_count', 'earned', 'paid', 'remaining'],
            'subscriptions' => ['patient', 'plan_type', 'start_date', 'end_date', 'sessions_count', 'sessions_used', 'remaining', 'remaining_days', 'net_total', 'status'],
            default => [],
        };

        return collect($rows)
            ->map(function (array $row) use ($columns, $type): array {
                $selected = collect($columns)->mapWithKeys(
                    fn (string $column): array => [$column => $row[$column] ?? null],
                )->all();

                if ($type === 'subscriptions' && ($selected['status'] ?? null) === 'cancelled') {
                    $selected['status'] = 'subscription_cancelled';
                }

                return $selected;
            })
            ->all();
    }

    private function headingLabel(string $key): string
    {
        return [
            'id' => 'الرقم', 'code' => 'الرمز', 'date' => 'التاريخ', 'opened_at' => 'تاريخ الجلسة',
            'closed_at' => 'تاريخ الإغلاق', 'created_at' => 'التاريخ', 'patient' => 'المريض',
            'patient_id' => 'رقم المريض', 'doctor' => 'الطبيب', 'doctor_id' => 'رقم الطبيب', 'services' => 'الخدمات',
            'status' => 'الحالة', 'service_value' => 'قيمة الخدمات', 'type' => 'النوع',
            'method' => 'طريقة الدفع', 'amount' => 'المبلغ', 'sponsor' => 'الجهة الراعية',
            'sponsor_id' => 'رقم الجهة', 'kind' => 'نوع الدين', 'paid' => 'المدفوع',
            'paid_amount' => 'المدفوع', 'remaining' => 'المتبقي', 'name' => 'الاسم',
            'sessions_count' => 'عدد الجلسات', 'sessions_used' => 'الجلسات المستخدمة',
            'remaining_sessions' => 'الجلسات المتبقية', 'remaining_days' => 'الأيام المتبقية',
            'plan_type' => 'نوع الاشتراك', 'start_date' => 'تاريخ البداية', 'end_date' => 'تاريخ النهاية',
            'net_total' => 'القيمة الصافية',
            'discount' => 'الخصم', 'count' => 'العدد', 'received' => 'المقبوضات',
            'refunds' => 'الاسترجاعات', 'net' => 'الصافي', 'original' => 'القيمة الأصلية',
            'earned' => 'المستحق المكتسب', 'active' => 'الاشتراكات النشطة',
        ][$key] ?? $key;
    }

    private function displayValue(string $key, mixed $value): string|int|float|bool|null
    {
        if (in_array($key, ['patient', 'doctor', 'sponsor'], true) && is_array($value)) {
            return $value['name'] ?? '';
        }

        if ($key === 'services' && is_array($value)) {
            return collect($value)->pluck('name')->implode('، ');
        }

        if (! is_scalar($value) && $value !== null) {
            return json_encode($value, JSON_UNESCAPED_UNICODE);
        }

        $labels = [
            'open' => 'مفتوحة', 'closed' => 'مغلقة', 'cancelled' => 'ملغاة',
            'active' => 'نشط', 'completed' => 'مكتمل', 'stopped' => 'موقوف',
            'subscription_cancelled' => 'ملغي',
            'monthly' => 'شهري', 'legacy_package' => 'حزمة جلسات قديمة',
            'partial' => 'جزئي', 'paid' => 'مسدد', 'void' => 'ملغى',
            'cash' => 'نقدًا', 'transfer' => 'تحويل مصرفي',
            'session_payment' => 'دفعة جلسة', 'subscription_payment' => 'دفعة اشتراك',
            'wallet_topup' => 'شحن رصيد', 'debt_payment' => 'تسديد دين',
            'refund' => 'استرجاع', 'adjustment' => 'تصحيح',
            'session' => 'جلسة', 'subscription' => 'اشتراك', 'opening' => 'دين افتتاحي',
        ];

        return is_string($value) && isset($labels[$value]) ? $labels[$value] : $value;
    }

    /** @param array<string, mixed> $report */
    private function periodLabel(array $report): string
    {
        $from = $report['meta']['from'] ?? null;
        $to = $report['meta']['to'] ?? null;

        if ($from && $to) {
            return "من {$from} إلى {$to}";
        }

        if ($from) {
            return "من {$from}";
        }

        if ($to) {
            return "حتى {$to}";
        }

        return 'كل الفترات';
    }
}
