<?php

namespace App\Http\Resources;

use Illuminate\Http\Request;
use Illuminate\Http\Resources\Json\JsonResource;

class AuditLogResource extends JsonResource
{
    /**
     * Transform the resource into an array.
     *
     * @return array<string, mixed>
     */
    public function toArray(Request $request): array
    {
        return [
            'id' => $this->resource->id,
            'user_id' => $this->resource->user_id,
            'user' => $this->whenLoaded('user', fn () => [
                'id' => $this->resource->user->id,
                'name' => $this->resource->user->name,
            ]),
            'action' => $this->resource->action,
            'entity_type' => $this->resource->entity_type,
            'entity_id' => $this->resource->entity_id,
            'old_values' => $this->resource->old_values,
            'new_values' => $this->resource->new_values,
            'reason' => $this->resource->reason,
            'created_at' => $this->resource->created_at?->toIso8601String(),
        ];
    }
}
