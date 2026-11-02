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
        Schema::create('subscriptions', function (Blueprint $table) {
            $table->id();
            $table->foreignId('patient_id')->constrained()->restrictOnDelete();
            $table->foreignId('sponsor_id')->nullable()->constrained()->restrictOnDelete();
            $table->unsignedInteger('sessions_count');
            $table->unsignedInteger('sessions_used')->default(0);
            $table->decimal('base_total', 12, 3)->unsigned();
            $table->decimal('discount', 12, 3)->unsigned()->default(0);
            $table->decimal('net_total', 12, 3)->unsigned();
            $table->decimal('paid_amount', 12, 3)->unsigned()->default(0);
            $table->date('start_date');
            $table->date('expected_end_date')->nullable();
            $table->enum('status', ['active', 'completed', 'stopped'])->default('active');
            $table->foreignId('created_by')->constrained('users')->restrictOnDelete();
            $table->timestamps();

            $table->index(['patient_id', 'status']);
            $table->index(['status', 'start_date']);
        });
    }

    /**
     * Reverse the migrations.
     */
    public function down(): void
    {
        Schema::dropIfExists('subscriptions');
    }
};
