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
        Schema::create('session_additions', function (Blueprint $table): void {
            $table->id();
            $table->foreignId('session_id')->constrained()->restrictOnDelete();
            $table->decimal('services_total', 12, 3)->unsigned();
            $table->decimal('net_total', 12, 3)->unsigned();
            $table->enum('coverage', ['subscription', 'cash', 'transfer', 'wallet', 'debt', 'mixed']);
            $table->decimal('wallet_amount', 12, 3)->unsigned()->default(0);
            $table->decimal('paid_amount', 12, 3)->unsigned()->default(0);
            $table->decimal('debt_amount', 12, 3)->unsigned()->default(0);
            $table->enum('payment_method', ['cash', 'transfer'])->nullable();
            $table->foreignId('sponsor_id')->nullable()->constrained()->restrictOnDelete();
            $table->foreignId('created_by')->constrained('users')->restrictOnDelete();
            $table->timestamps();

            $table->index(['session_id', 'created_at']);
        });

        Schema::table('session_services', function (Blueprint $table): void {
            $table->index('session_id');
        });

        Schema::table('session_services', function (Blueprint $table): void {
            $table->dropUnique(['session_id', 'service_id']);
            $table->foreignId('session_addition_id')
                ->nullable()
                ->after('session_id')
                ->constrained('session_additions')
                ->restrictOnDelete();
            $table->index(['session_id', 'service_id']);
        });

        Schema::table('session_services', function (Blueprint $table): void {
            $table->dropIndex(['session_id']);
        });

        Schema::table('payments', function (Blueprint $table): void {
            $table->foreignId('session_addition_id')
                ->nullable()
                ->after('session_id')
                ->constrained('session_additions')
                ->restrictOnDelete();
        });

        Schema::table('debts', function (Blueprint $table): void {
            $table->foreignId('session_addition_id')
                ->nullable()
                ->after('session_id')
                ->constrained('session_additions')
                ->restrictOnDelete();
        });

        Schema::table('wallet_transactions', function (Blueprint $table): void {
            $table->foreignId('session_addition_id')
                ->nullable()
                ->after('session_id')
                ->constrained('session_additions')
                ->restrictOnDelete();
        });
    }

    /**
     * Reverse the migrations.
     */
    public function down(): void
    {
        DB::table('session_services')->whereNotNull('session_addition_id')->delete();

        Schema::table('wallet_transactions', function (Blueprint $table): void {
            $table->dropConstrainedForeignId('session_addition_id');
        });

        Schema::table('debts', function (Blueprint $table): void {
            $table->dropConstrainedForeignId('session_addition_id');
        });

        Schema::table('payments', function (Blueprint $table): void {
            $table->dropConstrainedForeignId('session_addition_id');
        });

        Schema::table('session_services', function (Blueprint $table): void {
            $table->index('session_id');
        });

        Schema::table('session_services', function (Blueprint $table): void {
            $table->dropIndex(['session_id', 'service_id']);
            $table->dropConstrainedForeignId('session_addition_id');
            $table->unique(['session_id', 'service_id']);
        });

        Schema::table('session_services', function (Blueprint $table): void {
            $table->dropIndex(['session_id']);
        });

        Schema::dropIfExists('session_additions');
    }
};
