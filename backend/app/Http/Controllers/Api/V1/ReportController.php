<?php

namespace App\Http\Controllers\Api\V1;

use App\Exceptions\ApiException;
use App\Http\Controllers\Controller;
use App\Services\ReportService;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use Symfony\Component\HttpFoundation\BinaryFileResponse;

class ReportController extends Controller
{
    public function __construct(private ReportService $reportService) {}

    public function show(string $type, Request $request): JsonResponse
    {
        return response()->json(['data' => $this->reportService->report($type, $this->filters($request))]);
    }

    public function export(string $type, Request $request): BinaryFileResponse
    {
        $filters = $this->filters($request);
        $file = $this->reportService->export(
            $type,
            $request->string('format')->toString(),
            $this->reportService->report($type, $filters),
        );

        return response()->download($file['path'], $file['name'], ['Content-Type' => $file['mime']])->deleteFileAfterSend();
    }

    /** @return array<string, mixed> */
    private function filters(Request $request): array
    {
        $from = $request->date('from');
        $to = $request->date('to');

        if ($from && $to && $from->isAfter($to)) {
            throw ApiException::validation('تاريخ البداية يجب أن يسبق تاريخ النهاية.', [
                'from' => ['الفترة الزمنية غير صحيحة.'],
            ]);
        }

        return array_filter([
            'from' => $from?->toDateString(),
            'to' => $to?->toDateString(),
            'doctor_id' => $request->integer('doctor_id') ?: null,
            'patient_id' => $request->integer('patient_id') ?: null,
            'sponsor_id' => $request->integer('sponsor_id') ?: null,
            'status' => $request->string('status')->toString() ?: null,
            'kind' => $request->string('kind')->toString() ?: null,
            'type' => $request->string('type')->toString() ?: null,
            'method' => $request->string('method')->toString() ?: null,
            'near_end' => $request->boolean('near_end') ?: null,
        ], fn ($value) => $value !== null);
    }
}
