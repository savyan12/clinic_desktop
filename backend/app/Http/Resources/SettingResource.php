<?php

namespace App\Http\Resources;

use Illuminate\Http\Request;
use Illuminate\Http\Resources\Json\JsonResource;

class SettingResource extends JsonResource
{
    /**
     * Transform the resource into an array.
     *
     * @return array<string, mixed>
     */
    public function toArray(Request $request): array
    {
        return [
            'clinic_name' => $this->resource->clinic_name,
            'clinic_subtitle' => $this->resource->clinic_subtitle,
            'phone' => $this->resource->phone,
            'address' => $this->resource->address,
            'receipt_footer' => $this->resource->receipt_footer,
            'currency' => $this->resource->currency,
            'print_size' => $this->resource->print_size,
            'show_logo' => $this->resource->show_logo,
            'body_font_size' => $this->resource->body_font_size,
            'body_font_weight' => $this->resource->body_font_weight,
            'table_font_size' => $this->resource->table_font_size,
            'table_font_weight' => $this->resource->table_font_weight,
            'heading_font_size' => $this->resource->heading_font_size,
            'heading_font_weight' => $this->resource->heading_font_weight,
            'section_font_size' => $this->resource->section_font_size,
            'section_font_weight' => $this->resource->section_font_weight,
        ];
    }
}
