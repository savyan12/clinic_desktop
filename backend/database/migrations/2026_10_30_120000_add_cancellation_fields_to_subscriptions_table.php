<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Facades\Schema;

return new class extends Migration
{
    public function up(): void
    {
        if (DB::getDriverName() === 'mysql') {
            DB::statement("ALTER TABLE subscriptions MODIFY status ENUM('active', 'completed', 'stopped', 'cancelled') NOT NULL DEFAULT 'active'");
        }

        Schema::table('subscriptions', function (Blueprint $table): void {
            $table->timestamp('cancelled_at')->nullable()->after('status');
            $table->foreignId('cancelled_by')->nullable()->after('cancelled_at')->constrained('users')->restrictOnDelete();
            $table->text('cancel_reason')->nullable()->after('cancelled_by');
            $table->decimal('refund_amount', 12, 3)->unsigned()->default(0)->after('cancel_reason');
        });
    }

    public function down(): void
    {
        Schema::table('subscriptions', function (Blueprint $table): void {
            $table->dropConstrainedForeignId('cancelled_by');
            $table->dropColumn(['cancelled_at', 'cancel_reason', 'refund_amount']);
        });

        if (DB::getDriverName() === 'mysql') {
            DB::table('subscriptions')->where('status', 'cancelled')->update(['status' => 'stopped']);
            DB::statement("ALTER TABLE subscriptions MODIFY status ENUM('active', 'completed', 'stopped') NOT NULL DEFAULT 'active'");
        }
    }
};
