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
        Schema::create('sessions', function (Blueprint $table) {
            $table->id();
            $table->string('code')->unique();
            $table->foreignId('patient_id')->constrained()->restrictOnDelete();
            $table->foreignId('doctor_id')->constrained()->restrictOnDelete();
            $table->foreignId('subscription_id')->nullable()->constrained()->restrictOnDelete();
            $table->enum('type', ['individual', 'subscription']);
            $table->enum('status', ['open', 'closed', 'cancelled'])->default('open');
            $table->timestamp('opened_at');
            $table->timestamp('closed_at')->nullable();
            $table->timestamp('cancelled_at')->nullable();
            $table->foreignId('created_by')->constrained('users')->restrictOnDelete();
            $table->foreignId('closed_by')->nullable()->constrained('users')->restrictOnDelete();
            $table->foreignId('cancelled_by')->nullable()->constrained('users')->restrictOnDelete();
            $table->text('cancel_reason')->nullable();
            $table->decimal('services_total', 12, 3)->unsigned();
            $table->decimal('discount', 12, 3)->unsigned()->default(0);
            $table->decimal('net_total', 12, 3)->unsigned();
            $table->enum('coverage', ['subscription', 'cash', 'transfer', 'wallet', 'debt', 'mixed']);
            $table->decimal('wallet_amount', 12, 3)->unsigned()->default(0);
            $table->decimal('paid_amount', 12, 3)->unsigned()->default(0);
            $table->decimal('debt_amount', 12, 3)->unsigned()->default(0);
            $table->enum('payment_method', ['cash', 'transfer'])->nullable();
            $table->foreignId('sponsor_id')->nullable()->constrained()->restrictOnDelete();
            $table->decimal('commission_rate', 6, 3)->unsigned()->nullable();
            $table->decimal('commission_base_amount', 12, 3)->unsigned()->nullable();
            $table->decimal('commission_amount', 12, 3)->unsigned()->nullable();
            $table->timestamps();

            $table->index(['status', 'opened_at']);
            $table->index(['patient_id', 'opened_at']);
            $table->index(['doctor_id', 'opened_at']);
        });
    }

    /**
     * Reverse the migrations.
     */
    public function down(): void
    {
        Schema::dropIfExists('sessions');
    }
};
