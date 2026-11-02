<?php

return [
    'commission_base' => env('CLINIC_COMMISSION_BASE', 'net'),
    'near_end_sessions' => (int) env('CLINIC_NEAR_END_SESSIONS', 2),
    'near_end_days' => (int) env('CLINIC_NEAR_END_DAYS', 7),
    'payment_methods' => ['cash', 'transfer'],
];
