<?php

namespace App\Enums;

enum SubscriptionStatus: string
{
    case Active = 'active';
    case Completed = 'completed';
    case Stopped = 'stopped';
    case Cancelled = 'cancelled';
}
