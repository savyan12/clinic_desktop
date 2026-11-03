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
use App\Models\Setting;
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
        $columns = $this->reportColumns($type);
        $writer->addRow(Row::fromValues([$this->reportTitle($type)]));
        $writer->addRow(Row::fromValues(['الفترة', $this->periodLabel($report)]));
        $writer->addRow(Row::fromValues(array_values($columns)));

        foreach ($rows as $row) {
            $writer->addRow(Row::fromValues(array_map(
                fn ($value, $key) => $this->displayValue($type, $key, $value),
                array_values($row),
                array_keys($row),
            )));
        }

        $writer->addRow(Row::fromValues(['الإجماليات']));

        foreach ($report['totals'] as $key => $value) {
            $writer->addRow(Row::fromValues([
                $this->totalLabel($type, $key),
                $this->displayValue($type, $key, $value),
            ]));
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
            'orientation' => 'P',
            'tempDir' => sys_get_temp_dir(),
            'autoScriptToLang' => true,
            'autoLangToFont' => true,
            'margin_left' => 11,
            'margin_right' => 11,
            'margin_top' => 10,
            'margin_bottom' => 18,
        ]);
        $mpdf->SetDirectionality('rtl');
        $mpdf->SetTitle($this->reportTitle($type));
        $rows = $this->rowsForExport($type, $report['rows']);
        $columns = $this->reportColumns($type);
        $headerHtml = implode('', array_map(fn (string $label): string => '<th>'.e($label).'</th>', array_values($columns)));
        $bodyHtml = implode('', array_map(function (array $row) use ($type): string {
            $cells = '';

            foreach ($row as $key => $value) {
                $cells .= '<td>'.$this->pdfValueHtml($type, $key, $value).'</td>';
            }

            return '<tr>'.$cells.'</tr>';
        }, $rows));
        if ($bodyHtml === '') {
            $bodyHtml = '<tr><td class="empty" colspan="'.count($columns).'">لا توجد بيانات ضمن الفترة المحددة</td></tr>';
        }

        $summaryItems = $this->summaryItems($type, $report['totals']);
        $summaryWidth = $summaryItems === [] ? 100 : 100 / count($summaryItems);
        $summaryHtml = implode('', array_map(function (array $item) use ($type, $summaryWidth): string {
            return '<td style="width:'.$summaryWidth.'%"><span>'.e($item['label']).'</span><strong>'.$this->pdfValueHtml($type, $item['key'], $item['value']).'</strong></td>';
        }, $summaryItems));

        $settings = Setting::query()->first();
        $title = e($this->reportTitle($type));
        $clinicName = e($settings?->clinic_name ?: 'عيادة عين الحياة');
        $clinicSubtitle = e($settings?->clinic_subtitle ?: 'للعلاج الطبيعي والتأهيل');
        $period = e($this->periodLabel($report));
        $generatedAt = e($this->dateTimeLabel($report['meta']['generated_at'] ?? now()->toIso8601String()));
        $address = e($settings?->address ?: '—');
        $phone = e($settings?->phone ?: '—');
        $receiptFooter = e($settings?->receipt_footer ?: '');
        $logoHtml = $this->reportLogoHtml((bool) ($settings?->show_logo ?? true));
        $logoCellHtml = $logoHtml === '' ? '' : '<td class="logo-cell">'.$logoHtml.'</td>';
        $rowCount = count($rows);

        $mpdf->SetHTMLFooter('<div style="border-top:1px solid #c9d4e2;padding-top:5px;color:#566277;font-size:8px;text-align:center"><table style="width:100%;border-collapse:collapse"><tr><td style="text-align:right">'.$address.'</td><td style="text-align:center">'.$receiptFooter.'</td><td style="text-align:left" dir="ltr">'.$phone.'</td></tr></table><div style="margin-top:3px">صفحة {PAGENO} من {nbpg}</div></div>');

        $mpdf->WriteHTML(<<<HTML
            <html lang="ar" dir="rtl">
            <style>
                body { font-family: dejavusans; font-size: 9px; color: #080b09; }
                .header, .meta-table, .summary-table, .report { width: 100%; border-collapse: collapse; }
                .header td { vertical-align: middle; padding: 0 0 7px; }
                .brand-cell { width: 62%; text-align: right; }
                .brand-wrap { width: 100%; border-collapse: collapse; }
                .brand-wrap td { padding: 0; vertical-align: middle; }
                .logo-cell { width: 17mm; }
                .logo { width: 14mm; height: 14mm; }
                .brand-name { margin: 0; color: #00694b; font-size: 20px; font-weight: bold; }
                .brand-subtitle { margin-top: 2px; color: #566277; font-size: 10px; font-weight: bold; }
                .title-cell { width: 38%; text-align: left; }
                .title { margin: 0; font-size: 17px; font-weight: bold; }
                .generated { margin-top: 3px; color: #566277; font-size: 8px; }
                .brand-rule { height: 2px; margin-bottom: 8px; background: #00784f; }
                .meta-table { margin-bottom: 8px; border: 1px solid #c9d4e2; }
                .meta-table td { padding: 7px 9px; border-left: 1px solid #c9d4e2; vertical-align: top; }
                .meta-table td:last-child { border-left: 0; }
                .label, .summary-table span { display: block; color: #566277; font-size: 8px; font-weight: bold; }
                .meta-value { display: block; margin-top: 2px; font-size: 10px; font-weight: bold; }
                .summary-table { margin-bottom: 12px; border: 1px solid #c9d4e2; background: #eaf4ef; }
                .summary-table td { padding: 8px 9px; border-left: 1px solid #c9d4e2; vertical-align: top; }
                .summary-table td:last-child { border-left: 0; }
                .summary-table strong { display: block; margin-top: 3px; font-size: 12px; }
                .section-title { margin: 0; font-size: 14px; font-weight: bold; }
                .section-rule { height: 2px; margin: 3px 0 7px; background: #00784f; }
                .report { table-layout: fixed; border: 1px solid #c9d4e2; }
                .report thead { display: table-header-group; }
                .report tr { page-break-inside: avoid; }
                .report th, .report td { padding: 6px 4px; border-bottom: 1px solid #c9d4e2; border-left: 1px solid #c9d4e2; text-align: center; vertical-align: middle; overflow-wrap: break-word; }
                .report th:last-child, .report td:last-child { border-left: 0; }
                .report th { background: #00784f; color: #fff; font-size: 8px; font-weight: bold; }
                .report td { font-size: 8px; }
                .empty { padding: 18px !important; color: #566277; }
                .latin { direction: ltr; unicode-bidi: embed; font-family: dejavusans; }
                .report-note { margin: 8px 0 0; color: #566277; font-size: 8px; }
            </style>
            <body>
                <table class="header"><tr>
                    <td class="brand-cell"><table class="brand-wrap"><tr>{$logoCellHtml}<td><div class="brand-name">{$clinicName}</div><div class="brand-subtitle">{$clinicSubtitle}</div></td></tr></table></td>
                    <td class="title-cell"><div class="title">{$title}</div><div class="generated">أُعد في {$generatedAt}</div></td>
                </tr></table>
                <div class="brand-rule"></div>
                <table class="meta-table"><tr>
                    <td style="width:45%"><span class="label">الفترة</span><strong class="meta-value">{$period}</strong></td>
                    <td style="width:35%"><span class="label">نوع التقرير</span><strong class="meta-value">{$title}</strong></td>
                    <td style="width:20%"><span class="label">عدد النتائج</span><strong class="meta-value latin">{$rowCount}</strong></td>
                </tr></table>
                <table class="summary-table"><tr>{$summaryHtml}</tr></table>
                <h2 class="section-title">بيانات التقرير</h2>
                <div class="section-rule"></div>
                <table class="report"><thead><tr>{$headerHtml}</tr></thead><tbody>{$bodyHtml}</tbody></table>
                <p class="report-note">هذا التقرير يعرض كل مفهوم مالي بصورة مستقلة، ولا يُعد تقرير أرباح.</p>
            </body>
            </html>
            HTML);
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
        $columns = array_keys($this->reportColumns($type));

        return collect($rows)
            ->map(function (array $row) use ($columns): array {
                $selected = [];

                foreach ($columns as $column) {
                    $selected[$column] = match ($column) {
                        'subscription_end' => $row['end_date'] ?? $row['expected_end_date'] ?? null,
                        'subscription_remaining' => ($row['plan_type'] ?? null) === 'monthly'
                            ? ($row['remaining_days'] ?? 0).' يوم'
                            : ($row['remaining'] ?? 0).' جلسة',
                        default => $row[$column] ?? null,
                    };
                }

                return $selected;
            })
            ->all();
    }

    /** @return array<string, string> */
    private function reportColumns(string $type): array
    {
        return match ($type) {
            'sessions' => [
                'opened_at' => 'التاريخ',
                'patient' => 'المريض',
                'doctor' => 'الطبيب',
                'services' => 'الخدمات',
                'net_total' => 'القيمة',
                'status' => 'الحالة',
            ],
            'collections' => [
                'created_at' => 'التاريخ',
                'type' => 'النوع',
                'patient' => 'المريض',
                'method' => 'الطريقة',
                'amount' => 'المبلغ',
            ],
            'debts' => [
                'id' => 'المرجع',
                'patient' => 'المريض',
                'sponsor' => 'المسؤول',
                'amount' => 'الأصل',
                'paid_amount' => 'المسدد',
                'remaining' => 'المتبقي',
                'status' => 'الحالة',
            ],
            'doctor-dues' => [
                'name' => 'الطبيب',
                'sessions_count' => 'الجلسات',
                'earned' => 'المكتسب',
                'paid' => 'المصروف',
                'remaining' => 'المتبقي',
            ],
            'subscriptions' => [
                'patient' => 'المريض',
                'plan_type' => 'النوع',
                'start_date' => 'البداية',
                'subscription_end' => 'النهاية',
                'subscription_remaining' => 'المتبقي',
                'net_total' => 'القيمة',
                'status' => 'الحالة',
            ],
            default => [],
        };
    }

    private function totalLabel(string $type, string $key): string
    {
        return match (true) {
            $type === 'sessions' && $key === 'count' => 'عدد الجلسات',
            $type === 'subscriptions' && $key === 'count' => 'عدد الاشتراكات',
            $key === 'service_value' => 'قيمة الخدمات',
            $key === 'received' => 'إجمالي المقبوضات',
            $key === 'refunds' => 'إجمالي الاسترجاعات',
            $key === 'net' => 'صافي المقبوضات',
            $key === 'original' => 'إجمالي الديون',
            $type === 'doctor-dues' && $key === 'paid' => 'إجمالي المصروف',
            $key === 'paid' => 'إجمالي المسدد',
            $key === 'remaining' => 'إجمالي المتبقي',
            $key === 'earned' => 'إجمالي المستحق المكتسب',
            $key === 'active' => 'الاشتراكات النشطة',
            $key === 'remaining_sessions' => 'الجلسات المتبقية',
            $key === 'net_total' => 'قيمة الاشتراكات',
            default => $key,
        };
    }

    private function displayValue(string $type, string $key, mixed $value): string
    {
        if (in_array($key, ['patient', 'doctor', 'sponsor'], true)) {
            if ($key === 'sponsor' && $value === null) {
                return 'المريض';
            }

            return is_array($value) ? ($value['name'] ?? '—') : '—';
        }

        if ($key === 'services') {
            $services = is_array($value) ? collect($value)->pluck('name')->filter()->implode('، ') : '';

            return $services !== '' ? $services : '—';
        }

        if ($value === null || $value === '') {
            return '—';
        }

        if (is_bool($value)) {
            return $value ? 'نعم' : 'لا';
        }

        if (! is_scalar($value)) {
            return '—';
        }

        $raw = (string) $value;

        if (in_array($key, ['opened_at', 'created_at', 'start_date', 'end_date', 'subscription_end'], true)) {
            return $this->dateLabel($raw);
        }

        if ($this->isMoneyKey($key)) {
            return $this->moneyLabel($value);
        }

        if ($key === 'id') {
            return '#'.$raw;
        }

        return $this->enumLabel($type, $key, $raw) ?? $raw;
    }

    private function enumLabel(string $type, string $key, string $value): ?string
    {
        return match (true) {
            $key === 'status' && $value === 'open' && $type === 'sessions' => 'مفتوحة',
            $key === 'status' && $value === 'open' => 'مفتوح',
            $key === 'status' && $value === 'closed' => 'مغلقة',
            $key === 'status' && $value === 'cancelled' && $type === 'subscriptions' => 'ملغي',
            $key === 'status' && $value === 'cancelled' => 'ملغاة',
            $key === 'status' && $value === 'active' => 'نشط',
            $key === 'status' && $value === 'completed' => 'مكتمل',
            $key === 'status' && $value === 'stopped' => 'موقوف',
            $key === 'status' && $value === 'partial' => 'جزئي',
            $key === 'status' && $value === 'paid' => 'مسدد',
            $key === 'status' && $value === 'void' => 'ملغى',
            $key === 'plan_type' && $value === 'monthly' => 'شهري',
            $key === 'plan_type' && $value === 'legacy_package' => 'حزمة جلسات',
            $key === 'method' && $value === 'cash' => 'نقدًا',
            $key === 'method' && $value === 'transfer' => 'تحويل مصرفي',
            $key === 'type' && $value === 'session_payment' => 'دفعة جلسة',
            $key === 'type' && $value === 'subscription_payment' => 'دفعة اشتراك',
            $key === 'type' && $value === 'wallet_topup' => 'شحن رصيد',
            $key === 'type' && $value === 'debt_payment' => 'تسديد دين',
            $key === 'type' && $value === 'refund' => 'استرجاع',
            $key === 'type' && $value === 'adjustment' => 'تسوية',
            default => null,
        };
    }

    private function isMoneyKey(string $key): bool
    {
        return in_array($key, [
            'service_value', 'received', 'refunds', 'net', 'original', 'paid', 'remaining',
            'earned', 'net_total', 'amount', 'paid_amount',
        ], true);
    }

    private function numberLabel(mixed $value): string
    {
        if (! is_numeric($value)) {
            return (string) $value;
        }

        return rtrim(rtrim(number_format((float) $value, 3, '.', ''), '0'), '.');
    }

    private function moneyLabel(mixed $value): string
    {
        return $this->numberLabel($value).' د.ل';
    }

    private function dateLabel(string $value): string
    {
        try {
            $date = new \DateTimeImmutable($value);
        } catch (\Exception) {
            return $value;
        }

        $months = [
            1 => 'يناير', 'فبراير', 'مارس', 'أبريل', 'مايو', 'يونيو',
            'يوليو', 'أغسطس', 'سبتمبر', 'أكتوبر', 'نوفمبر', 'ديسمبر',
        ];

        return ((int) $date->format('j')).' '.$months[(int) $date->format('n')].' '.$date->format('Y');
    }

    private function dateTimeLabel(string $value): string
    {
        try {
            $date = new \DateTimeImmutable($value);
        } catch (\Exception) {
            return $value;
        }

        return $this->dateLabel($value).'، '.$date->format('H:i');
    }

    private function pdfValueHtml(string $type, string $key, mixed $value): string
    {
        if ($this->isMoneyKey($key)) {
            return '<span class="latin" dir="ltr">'.e($this->numberLabel($value)).'</span>&nbsp;د.ل';
        }

        if ($key === 'id') {
            return '#<span class="latin" dir="ltr">'.e((string) $value).'</span>';
        }

        return '<span dir="auto">'.e($this->displayValue($type, $key, $value)).'</span>';
    }

    /**
     * @param  array<string, mixed>  $totals
     * @return array<int, array{key: string, label: string, value: mixed}>
     */
    private function summaryItems(string $type, array $totals): array
    {
        return collect($this->totalKeys($type))
            ->filter(fn (string $key): bool => array_key_exists($key, $totals))
            ->map(fn (string $key): array => [
                'key' => $key,
                'label' => $this->totalLabel($type, $key),
                'value' => $totals[$key],
            ])
            ->values()
            ->all();
    }

    /** @return array<int, string> */
    private function totalKeys(string $type): array
    {
        return match ($type) {
            'sessions' => ['count', 'service_value'],
            'collections' => ['received', 'refunds', 'net'],
            'debts' => ['original', 'paid', 'remaining'],
            'doctor-dues' => ['earned', 'paid', 'remaining'],
            'subscriptions' => ['count', 'active', 'remaining_sessions', 'net_total'],
            default => [],
        };
    }

    private function reportLogoHtml(bool $showLogo): string
    {
        $logoPath = base_path('../public/clinic-logo-128.png');

        if (! $showLogo || ! is_file($logoPath)) {
            return '';
        }

        return '<img class="logo" alt="" src="data:image/png;base64,'.base64_encode((string) file_get_contents($logoPath)).'">';
    }

    /** @param array<string, mixed> $report */
    private function periodLabel(array $report): string
    {
        $from = $report['meta']['from'] ?? null;
        $to = $report['meta']['to'] ?? null;

        if ($from && $to) {
            return 'من '.$this->dateLabel($from).' إلى '.$this->dateLabel($to);
        }

        if ($from) {
            return 'من '.$this->dateLabel($from);
        }

        if ($to) {
            return 'حتى '.$this->dateLabel($to);
        }

        return 'كل الفترات';
    }
}
