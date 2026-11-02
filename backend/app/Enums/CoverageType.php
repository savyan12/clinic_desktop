<?php

namespace App\Enums;

enum CoverageType: string
{
    case Subscription = 'subscription';
    case Cash = 'cash';
    case Transfer = 'transfer';
    case Wallet = 'wallet';
    case Debt = 'debt';
    case Mixed = 'mixed';
}
