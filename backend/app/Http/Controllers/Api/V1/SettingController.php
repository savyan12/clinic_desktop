<?php

namespace App\Http\Controllers\Api\V1;

use App\Http\Controllers\Controller;
use App\Http\Requests\Settings\UpdateSettingRequest;
use App\Http\Resources\SettingResource;
use App\Models\Setting;
use App\Services\AuditLogger;
use Illuminate\Support\Facades\DB;

class SettingController extends Controller
{
    public function __construct(private AuditLogger $auditLogger) {}

    public function show(): SettingResource
    {
        return SettingResource::make($this->settings());
    }

    public function update(UpdateSettingRequest $request): SettingResource
    {
        $settings = $this->settings();
        DB::transaction(function () use ($request, $settings): void {
            $old = $settings->toArray();
            $settings->update([...$request->validated(), 'updated_by' => $request->user()->id]);
            $this->auditLogger->log($request->user(), 'settings_updated', $settings, $settings->id, $old, $settings->fresh()->toArray());
        });

        return SettingResource::make($settings->fresh());
    }

    private function settings(): Setting
    {
        return Setting::query()->firstOrCreate([], [
            'clinic_name' => 'عيادة عين الحياة',
            'clinic_subtitle' => 'للعلاج الطبيعي والتأهيل',
            'currency' => 'د.ل',
            'print_size' => 'A4',
            'show_logo' => true,
            'body_font_size' => 16,
            'body_font_weight' => 400,
            'table_font_size' => 15,
            'table_font_weight' => 400,
            'heading_font_size' => 32,
            'heading_font_weight' => 600,
            'section_font_size' => 22,
            'section_font_weight' => 600,
        ]);
    }
}
