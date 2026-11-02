<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Facades\Schema;

return new class extends Migration
{
    /**
     * Run the migrations.
     */
    public function up(): void
    {
        Schema::table('services', function (Blueprint $table): void {
            $table->unsignedSmallInteger('included_units')->nullable()->after('price');
            $table->string('unit_name', 50)->nullable()->after('included_units');
            $table->decimal('extra_unit_price', 12, 3)->unsigned()->nullable()->after('unit_name');
        });

        Schema::table('subscriptions', function (Blueprint $table): void {
            $table->string('plan_type', 32)->default('legacy_package')->after('sponsor_id')->index();
            $table->date('end_date')->nullable()->after('expected_end_date');
        });

        Schema::table('session_services', function (Blueprint $table): void {
            $table->decimal('base_price', 12, 3)->unsigned()->nullable()->after('price');
            $table->unsignedSmallInteger('included_units')->default(0)->after('base_price');
            $table->unsignedSmallInteger('extra_units')->default(0)->after('included_units');
            $table->string('unit_name', 50)->nullable()->after('extra_units');
            $table->decimal('extra_unit_price', 12, 3)->unsigned()->nullable()->after('unit_name');
            $table->string('item_type', 32)->default('service')->after('extra_unit_price');
            $table->boolean('is_addition')->default(false)->after('item_type');
            $table->timestamp('added_at')->nullable()->after('is_addition');
        });

        DB::table('session_services')->update([
            'base_price' => DB::raw('price'),
        ]);
    }

    /**
     * Reverse the migrations.
     */
    public function down(): void
    {
        Schema::table('session_services', function (Blueprint $table): void {
            $table->dropColumn([
                'base_price',
                'included_units',
                'extra_units',
                'unit_name',
                'extra_unit_price',
                'item_type',
                'is_addition',
                'added_at',
            ]);
        });

        Schema::table('subscriptions', function (Blueprint $table): void {
            $table->dropIndex(['plan_type']);
            $table->dropColumn(['plan_type', 'end_date']);
        });

        Schema::table('services', function (Blueprint $table): void {
            $table->dropColumn(['included_units', 'unit_name', 'extra_unit_price']);
        });
    }
};
