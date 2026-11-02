<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

return new class extends Migration
{
    /**
     * Run the migrations.
     */
    public function up(): void
    {
        Schema::table('settings', function (Blueprint $table): void {
            $table->unsignedTinyInteger('body_font_size')->default(16)->after('show_logo');
            $table->unsignedSmallInteger('body_font_weight')->default(400)->after('body_font_size');
            $table->unsignedTinyInteger('table_font_size')->default(15)->after('body_font_weight');
            $table->unsignedSmallInteger('table_font_weight')->default(400)->after('table_font_size');
            $table->unsignedTinyInteger('heading_font_size')->default(32)->after('table_font_weight');
            $table->unsignedSmallInteger('heading_font_weight')->default(600)->after('heading_font_size');
            $table->unsignedTinyInteger('section_font_size')->default(22)->after('heading_font_weight');
            $table->unsignedSmallInteger('section_font_weight')->default(600)->after('section_font_size');
        });
    }

    /**
     * Reverse the migrations.
     */
    public function down(): void
    {
        Schema::table('settings', function (Blueprint $table): void {
            $table->dropColumn([
                'body_font_size',
                'body_font_weight',
                'table_font_size',
                'table_font_weight',
                'heading_font_size',
                'heading_font_weight',
                'section_font_size',
                'section_font_weight',
            ]);
        });
    }
};
