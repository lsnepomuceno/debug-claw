<?php

declare(strict_types=1);

namespace App;

final class Order
{
    /** @param list<array{sku: string, price: float}> $items */
    public function __construct(
        public readonly int $customerId,
        public readonly array $items,
        public float $total = 0.0,
    ) {
    }
}
