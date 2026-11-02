<?php

namespace App\Http\Middleware;

use App\Exceptions\ApiException;
use App\Models\IdempotencyKey;
use Closure;
use Illuminate\Database\UniqueConstraintViolationException;
use Illuminate\Http\Request;
use Illuminate\Support\Arr;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Str;
use Symfony\Component\HttpFoundation\Response;

class IdempotencyMiddleware
{
    /**
     * Handle an incoming request.
     *
     * @param  Closure(Request): (Response)  $next
     */
    public function handle(Request $request, Closure $next): Response
    {
        $key = $request->header('Idempotency-Key');

        if (! is_string($key) || ! Str::isUuid($key)) {
            throw ApiException::validation(
                'مفتاح العملية مطلوب ويجب أن يكون UUID صالحًا.',
                ['idempotency_key' => ['أرسل ترويسة Idempotency-Key صالحة.']],
            );
        }

        $route = $request->path();
        $requestHash = hash('sha256', json_encode([
            'method' => $request->method(),
            'route' => $route,
            'payload' => $this->sortRecursively($request->all()),
        ], JSON_UNESCAPED_UNICODE | JSON_UNESCAPED_SLASHES | JSON_THROW_ON_ERROR));

        try {
            return DB::transaction(function () use ($request, $next, $key, $route, $requestHash): Response {
                $record = IdempotencyKey::query()
                    ->where('user_id', $request->user()->id)
                    ->where('route', $route)
                    ->where('key', $key)
                    ->lockForUpdate()
                    ->first();

                if ($record) {
                    if (! hash_equals($record->request_hash, $requestHash)) {
                        throw ApiException::conflict('مفتاح العملية مستخدم لطلب مختلف.');
                    }

                    if ($record->status === null || $record->response === null) {
                        throw ApiException::conflict('العملية نفسها قيد التنفيذ، حاول بعد قليل.');
                    }

                    return response()->json($record->response, $record->status);
                }

                $record = IdempotencyKey::create([
                    'user_id' => $request->user()->id,
                    'route' => $route,
                    'key' => $key,
                    'request_hash' => $requestHash,
                ]);

                $response = $next($request);

                if ($response->getStatusCode() >= 500) {
                    throw new \RuntimeException('Idempotent operation failed with a server error.');
                }

                $decoded = json_decode($response->getContent(), true, flags: JSON_THROW_ON_ERROR);
                $record->update([
                    'status' => $response->getStatusCode(),
                    'response' => $decoded,
                ]);

                return $response;
            }, 3);
        } catch (UniqueConstraintViolationException $exception) {
            $record = IdempotencyKey::query()
                ->where('user_id', $request->user()->id)
                ->where('route', $route)
                ->where('key', $key)
                ->first();

            if (! $record) {
                throw $exception;
            }

            if (! hash_equals($record->request_hash, $requestHash)) {
                throw ApiException::conflict('مفتاح العملية مستخدم لطلب مختلف.');
            }

            if ($record->status === null || $record->response === null) {
                throw ApiException::conflict('العملية نفسها قيد التنفيذ، حاول بعد قليل.');
            }

            return response()->json($record->response, $record->status);
        }
    }

    /**
     * @param  array<string|int, mixed>  $value
     * @return array<string|int, mixed>
     */
    private function sortRecursively(array $value): array
    {
        foreach ($value as $key => $item) {
            if (is_array($item)) {
                $value[$key] = $this->sortRecursively($item);
            }
        }

        if (Arr::isAssoc($value)) {
            ksort($value);
        }

        return $value;
    }
}
