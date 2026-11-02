<?php

namespace App\Enums;

enum SubscriptionPlanType: string
{
    case LegacyPackage = 'legacy_package';
    case Monthly = 'monthly';
}
