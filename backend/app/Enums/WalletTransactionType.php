<?php

namespace App\Enums;

enum WalletTransactionType: string
{
    case Topup = 'topup';
    case Debit = 'debit';
    case Reversal = 'reversal';
}
