<?php

namespace App\Enums;

enum DebtStatus: string
{
    case Open = 'open';
    case Partial = 'partial';
    case Paid = 'paid';
    case Void = 'void';
}
