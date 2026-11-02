<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Attributes\Fillable;
use Illuminate\Database\Eloquent\Model;

#[Fillable(['clinic_name', 'clinic_subtitle', 'phone', 'address', 'receipt_footer', 'currency', 'print_size', 'show_logo', 'body_font_size', 'body_font_weight', 'table_font_size', 'table_font_weight', 'heading_font_size', 'heading_font_weight', 'section_font_size', 'section_font_weight', 'updated_by'])]
class Setting extends Model
{
    protected function casts(): array
    {
        return [
            'show_logo' => 'boolean',
            'body_font_size' => 'integer',
            'body_font_weight' => 'integer',
            'table_font_size' => 'integer',
            'table_font_weight' => 'integer',
            'heading_font_size' => 'integer',
            'heading_font_weight' => 'integer',
            'section_font_size' => 'integer',
            'section_font_weight' => 'integer',
        ];
    }
}
