<?php

namespace App\Enums;

enum DebtKind: string
{
    case Session = 'session';
    case Subscription = 'subscription';
    case Opening = 'opening';
}
