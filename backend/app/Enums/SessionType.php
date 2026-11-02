<?php

namespace App\Enums;

enum SessionType: string
{
    case Individual = 'individual';
    case Subscription = 'subscription';
}
