<?php

namespace App\Services;

use App\Models\AuditLog;
use App\Models\Setting;
use App\Models\User;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Support\Str;

class AuditLogger
{
    /**
     * @param  array<string, mixed>|null  $oldValues
     * @param  array<string, mixed>|null  $newValues
     */
    public function log(
        User $user,
        string $action,
        Model|string $entity,
        int $entityId,
        ?array $oldValues = null,
        ?array $newValues = null,
        ?string $reason = null,
    ): AuditLog {
        return AuditLog::create([
            'user_id' => $user->id,
            'action' => $action,
            'entity_type' => $this->entityType($entity),
            'entity_id' => $entityId,
            'old_values' => $oldValues,
            'new_values' => $newValues,
            'reason' => $reason,
            'created_at' => now(),
        ]);
    }

    private function entityType(Model|string $entity): string
    {
        if (is_string($entity)) {
            return $entity;
        }

        return $entity instanceof Setting
            ? 'settings'
            : Str::snake(class_basename($entity));
    }
}
