<?php

namespace App\Http\Controllers\Api\V1;

use App\Exceptions\ApiException;
use App\Http\Controllers\Controller;
use App\Http\Requests\Services\StoreServiceRequest;
use App\Http\Requests\Services\UpdateServiceRequest;
use App\Http\Resources\ServiceResource;
use App\Models\Service;
use App\Services\AuditLogger;
use App\Support\Money;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use Illuminate\Support\Arr;
use Illuminate\Support\Facades\DB;

class ServiceController extends Controller
{
    public function __construct(private AuditLogger $auditLogger) {}

    public function index(Request $request): JsonResponse
    {
        $query = Service::query();

        if ($search = $request->string('search')->trim()->toString()) {
            $query->where('name', 'like', "%{$search}%");
        }

        if ($request->has('is_active')) {
            $query->where('is_active', $request->boolean('is_active'));
        }

        return $this->paginated($query->latest('id')->paginate($this->perPage($request)), ServiceResource::class, $request);
    }

    public function store(StoreServiceRequest $request): ServiceResource
    {
        $service = DB::transaction(function () use ($request): Service {
            $service = Service::create([...$request->validated(), 'created_by' => $request->user()->id]);
            $this->auditLogger->log($request->user(), 'service_created', $service, $service->id, null, $service->toArray());

            return $service;
        });

        return ServiceResource::make($service);
    }

    public function update(UpdateServiceRequest $request, Service $service): ServiceResource
    {
        $data = $request->validated();
        $priceChanged = Money::compare($data['price'], $service->price) !== 0;

        if ($priceChanged && empty($data['reason'])) {
            throw ApiException::validation('سبب تغيير سعر الخدمة مطلوب.', [
                'reason' => ['اكتب سبب تغيير السعر.'],
            ]);
        }

        DB::transaction(function () use ($data, $priceChanged, $request, $service): void {
            $old = $service->toArray();
            $service->update(Arr::except($data, ['reason']));
            $this->auditLogger->log(
                $request->user(),
                $priceChanged ? 'service_price_changed' : 'service_updated',
                $service,
                $service->id,
                $old,
                $service->fresh()->toArray(),
                $data['reason'] ?? null,
            );
        });

        return ServiceResource::make($service->fresh());
    }
}
