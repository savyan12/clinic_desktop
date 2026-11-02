<?php

namespace App\Enums;

enum PaymentType: string
{
    case SessionPayment = 'session_payment';
    case SubscriptionPayment = 'subscription_payment';
    case WalletTopup = 'wallet_topup';
    case DebtPayment = 'debt_payment';
    case Refund = 'refund';
    case Adjustment = 'adjustment';
}
