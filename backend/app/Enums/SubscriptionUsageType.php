<?php

namespace App\Enums;

enum SubscriptionUsageType: string
{
    case Consume = 'consume';
    case Restore = 'restore';
}
