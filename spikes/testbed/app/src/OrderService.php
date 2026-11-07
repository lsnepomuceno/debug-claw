<?php

declare(strict_types=1);

namespace App;

use InvalidArgumentException;

/**
 * The scenario used by the MCP contract reviewer: POST /orders fails with
 * HTTP 500 for some customers, because their discount brings the total to
 * zero before charge() is called.
 */
final class OrderService
{
    public function place(int $customerId): array
    {
        $order = new Order($customerId, [
            ['sku' => 'book', 'price' => 30.0],
            ['sku' => 'pen', 'price' => 5.0],
        ]);

        $order->total = $this->total($order);
        $this->charge($order);

        return ['customer_id' => $customerId, 'total' => $order->total, 'status' => 'charged'];
    }

    private function total(Order $order): float
    {
        $subtotal = array_sum(array_column($order->items, 'price'));

        return $subtotal * (1 - $this->discount($order->customerId));
    }

    /** Every third customer has a full discount. */
    private function discount(int $customerId): float
    {
        return $customerId % 3 === 0 ? 1.0 : 0.1;
    }

    private function charge(Order $order): void
    {
        if ($order->total <= 0) {
            throw new InvalidArgumentException("Cannot charge an order with total {$order->total}");
        }
    }
}
