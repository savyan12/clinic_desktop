<?php

namespace App\Http\Controllers\Api\V1;

use App\Enums\PaymentMethod;
use App\Http\Controllers\Controller;
use App\Http\Requests\Wallet\TopUpWalletRequest;
use App\Http\Resources\PaymentResource;
use App\Http\Resources\WalletTransactionResource;
use App\Models\Patient;
use App\Services\WalletService;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;

class WalletController extends Controller
{
    public function __construct(private WalletService $walletService) {}

    public function movements(Patient $patient, Request $request): JsonResponse
    {
        $query = $patient->walletTransactions();

        if ($type = $request->string('type')->toString()) {
            $query->where('type', $type);
        }

        if ($from = $request->date('from')) {
            $query->whereDate('created_at', '>=', $from);
        }

        if ($to = $request->date('to')) {
            $query->whereDate('created_at', '<=', $to);
        }

        $paginator = $query->latest('id')->paginate($this->perPage($request));
        $response = $this->paginated($paginator, WalletTransactionResource::class, $request);
        $payload = $response->getData(true);
        $payload['balance'] = $this->walletService->balance($patient);

        return response()->json($payload);
    }

    public function topUp(TopUpWalletRequest $request, Patient $patient): JsonResponse
    {
        $result = $this->walletService->topUp(
            $patient,
            $request->string('amount')->toString(),
            PaymentMethod::from($request->string('method')->toString()),
            $request->string('note')->toString() ?: null,
            $request->user(),
        );

        return response()->json(['data' => [
            'balance' => $result['balance'],
            'movement' => WalletTransactionResource::make($result['movement'])->resolve($request),
            'payment' => PaymentResource::make($result['payment'])->resolve($request),
        ]], 201);
    }
}
