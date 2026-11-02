<?php

namespace Tests\Unit;

use App\Support\Money;
use PHPUnit\Framework\TestCase;

class MoneyTest extends TestCase
{
    public function test_decimal_operations_never_require_floats(): void
    {
        $this->assertSame('80.000', Money::normalize('80'));
        $this->assertSame('100.125', Money::add('60.100', '40.025'));
        $this->assertSame('33.333', Money::divide('100.000', 3));
        $this->assertSame('24.000', Money::percentage('80.000', '30.000'));
        $this->assertSame(1, Money::compare('1.001', '1.000'));
    }
}
