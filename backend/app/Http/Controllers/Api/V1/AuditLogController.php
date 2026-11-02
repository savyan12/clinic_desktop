<?php

namespace App\Http\Controllers\Api\V1;

use App\Http\Controllers\Controller;
use App\Http\Resources\AuditLogResource;
use App\Models\AuditLog;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;

class AuditLogController extends Controller
{
    public function index(Request $request): JsonResponse
    {
        $query = AuditLog::query()->with('user');

        foreach (['user_id', 'entity_id'] as $field) {
            if ($value = $request->integer($field)) {
                $query->where($field, $value);
            }
        }

        foreach (['action', 'entity_type'] as $field) {
            if ($value = $request->string($field)->toString()) {
                $query->where($field, $value);
            }
        }

        if ($date = $request->date('date')) {
            $query->whereDate('created_at', $date);
        }

        if ($from = $request->date('from')) {
            $query->whereDate('created_at', '>=', $from);
        }

        if ($to = $request->date('to')) {
            $query->whereDate('created_at', '<=', $to);
        }

        return $this->paginated(
            $query->latest('id')->paginate($this->perPage($request)),
            AuditLogResource::class,
            $request,
        );
    }
}
