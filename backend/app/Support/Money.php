<?php

namespace App\Support;

use InvalidArgumentException;

class Money
{
    public const int SCALE = 3;

    public static function normalize(int|float|string|null $amount): string
    {
        if ($amount === null || $amount === '') {
            return '0.000';
        }

        $value = (string) $amount;

        if (! preg_match('/^-?\d+(?:\.\d+)?$/', $value)) {
            throw new InvalidArgumentException('Invalid decimal money value.');
        }

        return bcround($value, self::SCALE);
    }

    public static function add(int|float|string $left, int|float|string $right): string
    {
        return bcadd(self::normalize($left), self::normalize($right), self::SCALE);
    }

    public static function subtract(int|float|string $left, int|float|string $right): string
    {
        return bcsub(self::normalize($left), self::normalize($right), self::SCALE);
    }

    public static function multiply(int|float|string $left, int|float|string $right): string
    {
        return self::normalize(bcmul(self::normalize($left), self::normalize($right), self::SCALE + 6));
    }

    public static function divide(int|float|string $amount, int $divisor): string
    {
        if ($divisor <= 0) {
            throw new InvalidArgumentException('Money divisor must be positive.');
        }

        return self::normalize(bcdiv(self::normalize($amount), (string) $divisor, self::SCALE + 6));
    }

    public static function percentage(int|float|string $amount, int|float|string $rate): string
    {
        $product = bcmul(self::normalize($amount), self::normalize($rate), self::SCALE + 6);

        return self::normalize(bcdiv($product, '100', self::SCALE + 6));
    }

    public static function compare(int|float|string $left, int|float|string $right): int
    {
        return bccomp(self::normalize($left), self::normalize($right), self::SCALE);
    }

    public static function isZero(int|float|string|null $amount): bool
    {
        return self::compare($amount ?? '0', '0') === 0;
    }

    public static function isPositive(int|float|string $amount): bool
    {
        return self::compare($amount, '0') === 1;
    }

    /**
     * @param  iterable<int, int|float|string|null>  $amounts
     */
    public static function sum(iterable $amounts): string
    {
        $total = '0.000';

        foreach ($amounts as $amount) {
            $total = self::add($total, $amount ?? '0');
        }

        return $total;
    }
}
